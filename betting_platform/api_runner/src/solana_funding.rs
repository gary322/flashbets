//! Solana Account Funding and Management for Live Trading

use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingConfig {
    pub airdrop_amount: u64,
    pub min_balance_threshold: u64,
    pub auto_fund_enabled: bool,
    pub funding_source: Option<String>, // Private key or funding account
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStatus {
    pub address: String,
    pub balance_sol: f64,
    pub balance_lamports: u64,
    pub is_funded: bool,
    pub needs_funding: bool,
    pub last_funded: Option<i64>,
}

pub struct SolanaFundingManager {
    rpc_client: RpcClient,
    config: FundingConfig,
}

impl SolanaFundingManager {
    pub fn new(rpc_url: String, config: FundingConfig) -> Self {
        let rpc_client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
        
        Self {
            rpc_client,
            config,
        }
    }

    /// Check account balance and funding status
    pub async fn check_account_status(&self, address: &Pubkey) -> Result<AccountStatus> {
        let balance_lamports = self.rpc_client.get_balance(address)?;
        let balance_sol = balance_lamports as f64 / LAMPORTS_PER_SOL as f64;
        
        let is_funded = balance_lamports > 0;
        let needs_funding = balance_lamports < self.config.min_balance_threshold;
        
        Ok(AccountStatus {
            address: address.to_string(),
            balance_sol,
            balance_lamports,
            is_funded,
            needs_funding,
            last_funded: None, // TODO: Track funding history
        })
    }

    /// Fund account via airdrop (devnet/testnet only)
    pub async fn fund_via_airdrop(&self, address: &Pubkey) -> Result<String> {
        // Check if we're on devnet/testnet
        let cluster = self.detect_cluster().await?;
        
        if cluster != "devnet" && cluster != "testnet" {
            return Err(anyhow!("Airdrops only available on devnet/testnet"));
        }
        
        tracing::info!("Requesting airdrop of {} SOL to {}", 
                      self.config.airdrop_amount as f64 / LAMPORTS_PER_SOL as f64, 
                      address);
        
        let signature = self.rpc_client
            .request_airdrop(address, self.config.airdrop_amount)?;
            
        // Wait for confirmation
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        self.rpc_client
            .confirm_transaction_with_spinner(&signature, &recent_blockhash, CommitmentConfig::confirmed())?;
            
        Ok(signature.to_string())
    }

    /// Transfer SOL from funding source to target account
    pub async fn fund_from_source(&self, target: &Pubkey, amount: u64) -> Result<String> {
        let funding_source = self.get_funding_keypair()?;
        
        // Check funding source balance
        let source_balance = self.rpc_client.get_balance(&funding_source.pubkey())?;
        if source_balance < amount + 5000 { // Include transaction fee
            return Err(anyhow!("Insufficient balance in funding source"));
        }
        
        // Create transfer instruction
        let transfer_instruction = system_instruction::transfer(
            &funding_source.pubkey(),
            target,
            amount,
        );
        
        // Create and sign transaction
        let mut transaction = Transaction::new_with_payer(
            &[transfer_instruction],
            Some(&funding_source.pubkey()),
        );
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        transaction.sign(&[&funding_source], recent_blockhash);
        
        // Send transaction
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        
        tracing::info!("Funded {} with {} lamports, signature: {}", 
                      target, amount, signature);
        
        Ok(signature.to_string())
    }

    /// Auto-fund account if enabled and needed
    pub async fn auto_fund_if_needed(&self, address: &Pubkey) -> Result<Option<String>> {
        if !self.config.auto_fund_enabled {
            return Ok(None);
        }
        
        let status = self.check_account_status(address).await?;
        
        if status.needs_funding {
            tracing::info!("Auto-funding account {} (balance: {} SOL)", 
                          address, status.balance_sol);
            
            // Try airdrop first (for devnet/testnet)
            if let Ok(signature) = self.fund_via_airdrop(address).await {
                return Ok(Some(signature));
            }
            
            // Fallback to funding source transfer
            if self.config.funding_source.is_some() {
                let signature = self.fund_from_source(address, self.config.airdrop_amount).await?;
                return Ok(Some(signature));
            }
        }
        
        Ok(None)
    }

    /// Create a funded keypair for testing
    pub async fn create_funded_keypair(&self) -> Result<(Keypair, String)> {
        let keypair = Keypair::new();
        
        // Fund the new keypair
        let funding_signature = if let Ok(sig) = self.fund_via_airdrop(&keypair.pubkey()).await {
            sig
        } else if self.config.funding_source.is_some() {
            self.fund_from_source(&keypair.pubkey(), self.config.airdrop_amount).await?
        } else {
            return Err(anyhow!("No funding method available"));
        };
        
        tracing::info!("Created and funded new keypair: {}", keypair.pubkey());
        
        Ok((keypair, funding_signature))
    }

    /// Get multiple funded keypairs for testing
    pub async fn create_funded_keypairs(&self, count: usize) -> Result<Vec<(Keypair, String)>> {
        let mut keypairs = Vec::new();
        
        for i in 0..count {
            match self.create_funded_keypair().await {
                Ok(keypair) => {
                    keypairs.push(keypair);
                    tracing::info!("Created funded keypair {}/{}", i + 1, count);
                }
                Err(e) => {
                    tracing::error!("Failed to create keypair {}: {}", i + 1, e);
                    // Continue trying to create others
                }
            }
            
            // Rate limiting to avoid overwhelming the network
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        
        Ok(keypairs)
    }

    /// Detect which Solana cluster we're connected to
    async fn detect_cluster(&self) -> Result<String> {
        // Try to get cluster info to determine if we're on devnet/testnet/mainnet
        match self.rpc_client.get_genesis_hash() {
            Ok(_) => {
                // Simple heuristic: check if we can request airdrops
                let test_keypair = Keypair::new();
                match self.rpc_client.request_airdrop(&test_keypair.pubkey(), 1000) {
                    Ok(_) => Ok("devnet".to_string()),
                    Err(_) => Ok("mainnet".to_string()),
                }
            }
            Err(_) => Ok("testnet".to_string()),
        }
    }

    /// Get funding source keypair from config
    fn get_funding_keypair(&self) -> Result<Keypair> {
        match &self.config.funding_source {
            Some(private_key_str) => {
                // Try to parse as base58 private key
                if let Ok(bytes) = bs58::decode(private_key_str).into_vec() {
                    if bytes.len() == 64 {
                        return Ok(Keypair::from_bytes(&bytes)?);
                    }
                }
                
                // Try to parse as JSON array
                if let Ok(bytes) = serde_json::from_str::<Vec<u8>>(private_key_str) {
                    if bytes.len() == 64 {
                        return Ok(Keypair::from_bytes(&bytes)?);
                    }
                }
                
                Err(anyhow!("Invalid funding source private key format"))
            }
            None => Err(anyhow!("No funding source configured"))
        }
    }

    /// Bulk check multiple accounts
    pub async fn check_multiple_accounts(&self, addresses: &[Pubkey]) -> Result<Vec<AccountStatus>> {
        let mut statuses = Vec::new();
        
        for address in addresses {
            match self.check_account_status(address).await {
                Ok(status) => statuses.push(status),
                Err(e) => {
                    tracing::error!("Failed to check account {}: {}", address, e);
                    // Add error status
                    statuses.push(AccountStatus {
                        address: address.to_string(),
                        balance_sol: 0.0,
                        balance_lamports: 0,
                        is_funded: false,
                        needs_funding: true,
                        last_funded: None,
                    });
                }
            }
        }
        
        Ok(statuses)
    }
}

impl Default for FundingConfig {
    fn default() -> Self {
        Self {
            airdrop_amount: LAMPORTS_PER_SOL, // 1 SOL
            min_balance_threshold: LAMPORTS_PER_SOL / 10, // 0.1 SOL
            auto_fund_enabled: true,
            funding_source: None,
        }
    }
}

/// Enhanced trading client with automatic funding
pub struct FundedTradingClient {
    funding_manager: SolanaFundingManager,
    platform_client: crate::rpc_client::BettingPlatformClient,
}

impl FundedTradingClient {
    pub fn new(
        rpc_url: String, 
        program_id: Pubkey,
        funding_config: FundingConfig
    ) -> Self {
        let funding_manager = SolanaFundingManager::new(rpc_url.clone(), funding_config);
        
        let rpc_client = std::sync::Arc::new(
            RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed())
        );
        let platform_client = crate::rpc_client::BettingPlatformClient::new(rpc_client, program_id);
        
        Self {
            funding_manager,
            platform_client,
        }
    }

    /// Place trade with automatic funding
    pub async fn place_trade_with_funding(
        &self,
        wallet: &Keypair,
        market_id: u128,
        amount: u64,
        outcome: u8,
        leverage: u32,
    ) -> Result<String> {
        // Check and fund account if needed
        if let Some(funding_sig) = self.funding_manager.auto_fund_if_needed(&wallet.pubkey()).await? {
            tracing::info!("Auto-funded account, signature: {}", funding_sig);
            
            // Wait for funding to confirm
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
        
        // Now attempt the trade
        self.platform_client.place_trade(wallet, market_id, amount, outcome, leverage).await
    }

    /// Create demo account with funding
    pub async fn create_funded_demo_account(&self) -> Result<(Keypair, String)> {
        self.funding_manager.create_funded_keypair().await
    }

    /// Get account status
    pub async fn get_account_status(&self, address: &Pubkey) -> Result<AccountStatus> {
        self.funding_manager.check_account_status(address).await
    }
}