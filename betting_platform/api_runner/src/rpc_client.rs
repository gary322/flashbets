//! RPC Client for interacting with Solana smart contracts

use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::sync::Arc;
use std::str::FromStr;

use crate::types::*;
use crate::pda;

/// Instructions for the betting platform program
#[derive(BorshSerialize, BorshDeserialize)]
pub enum MarketInstruction {
    CreateMarket {
        question: String,
        outcomes: Vec<String>,
        end_time: i64,
        market_type: MarketType,
        fee_rate: u16,
    },
    PlaceTrade {
        market_id: u128,
        outcome: u8,
        amount: u64,
        leverage: u32,
    },
    ClosePosition {
        position_id: u128,
    },
}

pub struct BettingPlatformClient {
    rpc_client: Arc<RpcClient>,
    program_id: Pubkey,
}


impl BettingPlatformClient {
    pub fn new(rpc_client: Arc<RpcClient>, program_id: Pubkey) -> Self {
        Self {
            rpc_client,
            program_id,
        }
    }

    /// Fetch all markets from the program
    pub async fn get_markets(&self) -> Result<Vec<Market>> {
        // Get all market accounts
        let accounts = self.rpc_client.get_program_accounts(&self.program_id)?;
        
        let mut markets = Vec::new();
        for (pubkey, account) in accounts {
            // Try to deserialize as market
            if let Ok(market) = Market::try_from_slice(&account.data) {
                markets.push(MarketInfo {
                    pubkey,
                    market,
                });
            }
        }
        
        Ok(markets.into_iter().map(|m| m.market).collect())
    }

    /// Get a specific market
    pub async fn get_market(&self, market_id: u128) -> Result<Option<Market>> {
        let market_pda = self.get_market_pda(market_id);
        
        match self.rpc_client.get_account(&market_pda) {
            Ok(account) => {
                let market = Market::try_from_slice(&account.data)?;
                Ok(Some(market))
            }
            Err(_) => Ok(None),
        }
    }

    /// Get user positions
    pub async fn get_positions(&self, wallet: &Pubkey) -> Result<Vec<Position>> {
        let accounts = self.rpc_client.get_program_accounts(&self.program_id)?;
        
        let mut positions = Vec::new();
        for (_, account) in accounts {
            if let Ok(position) = Position::try_from_slice(&account.data) {
                if position.owner == *wallet {
                    positions.push(position);
                }
            }
        }
        
        Ok(positions)
    }

    /// Place a trade
    pub async fn place_trade(
        &self,
        wallet: &Keypair,
        market_id: u128,
        amount: u64,
        outcome: u8,
        leverage: u32,
    ) -> Result<String> {
        let market_pda = self.get_market_pda(market_id);
        let position_pda = self.get_position_pda(wallet.pubkey(), market_id);
        let demo_account_pda = self.get_demo_account_pda(&wallet.pubkey());
        
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(demo_account_pda, false),
                AccountMeta::new(market_pda, false),
                AccountMeta::new(wallet.pubkey(), true),
                AccountMeta::new(position_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: BettingInstruction::PlaceBet {
                market_id,
                amount,
                outcome,
                leverage,
            }.try_to_vec()?,
        };
        
        let mut transaction = Transaction::new_with_payer(
            &[instruction],
            Some(&wallet.pubkey()),
        );
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        transaction.sign(&[wallet], recent_blockhash);
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(signature.to_string())
    }

    /// Create demo account
    pub async fn create_demo_account(&self, wallet: &Keypair) -> Result<String> {
        let demo_account_pda = self.get_demo_account_pda(&wallet.pubkey());
        
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(demo_account_pda, false),
                AccountMeta::new(wallet.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: BettingInstruction::CreateDemoAccount.try_to_vec()?,
        };
        
        let mut transaction = Transaction::new_with_payer(
            &[instruction],
            Some(&wallet.pubkey()),
        );
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        transaction.sign(&[wallet], recent_blockhash);
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(signature.to_string())
    }

    /// Get user balance
    pub async fn get_balance(&self, wallet: &Pubkey) -> Result<Balance> {
        // Get SOL balance
        let sol_balance = self.rpc_client.get_balance(wallet)?;
        
        // Get demo account balance
        let demo_account_pda = self.get_demo_account_pda(wallet);
        let demo_balance = match self.rpc_client.get_account(&demo_account_pda) {
            Ok(account) => {
                if let Ok(demo_account) = DemoAccount::try_from_slice(&account.data) {
                    demo_account.balance
                } else {
                    0
                }
            }
            Err(_) => 0,
        };
        
        Ok(Balance {
            sol: sol_balance,
            demo_usdc: demo_balance,
            mmt: 0, // MMT token balance (if implemented)
        })
    }

    /// Get verses
    pub async fn get_verses(&self) -> Result<Vec<Verse>> {
        let accounts = self.rpc_client.get_program_accounts(&self.program_id)?;
        
        let mut verses = Vec::new();
        for (_, account) in accounts {
            if let Ok(verse) = Verse::try_from_slice(&account.data) {
                verses.push(verse);
            }
        }
        
        Ok(verses)
    }
    
    /// Get program state
    pub async fn get_program_state(&self) -> Result<ProgramState> {
        // Mock implementation for testing
        Ok(ProgramState {
            admin: Pubkey::default(),
            total_markets: 0,
            total_volume: 0,
            protocol_fee_rate: 250,
            min_bet_amount: 1_000_000,
            max_bet_amount: 1_000_000_000,
            emergency_mode: false,
        })
    }
    
    /// Get all markets
    pub async fn get_all_markets(&self) -> Result<Vec<Market>> {
        self.get_markets().await
    }
    
    /// Get user positions with enhanced info
    pub async fn get_user_positions(&self, wallet: &Pubkey) -> Result<Vec<PositionInfo>> {
        let positions = self.get_positions(wallet).await?;
        
        // Convert to PositionInfo with mock data
        Ok(positions.into_iter().map(|p| PositionInfo {
            position: Pubkey::new_unique(),
            market_id: p.market_id,
            amount: p.size,
            outcome: p.outcome,
            leverage: p.leverage,
            entry_price: p.entry_price as f64 / 1e6,
            current_price: 0.5, // Mock current price
            pnl: 0, // Mock PnL
            status: PositionStatus::Open,
            created_at: p.created_at,
            updated_at: p.created_at,
        }).collect())
    }
    
    /// Get market orderbook
    pub async fn get_market_orderbook(&self, _market_id: u128) -> Result<serde_json::Value> {
        // Our Solana betting platform doesn't have an orderbook
        // Orderbook functionality is provided by Polymarket
        Err(anyhow::anyhow!("Orderbook not available - use Polymarket API for orderbook data"))
    }
    
    /// Create market
    pub async fn create_market(
        &self,
        admin: &Keypair,
        market_account: &Pubkey,
        question: &str,
        outcomes: &[String],
        end_time: i64,
        market_type: MarketType,
        fee_rate: u16,
    ) -> Result<String> {
        // Build instruction for creating market
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(*market_account, false),
                AccountMeta::new(admin.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: MarketInstruction::CreateMarket {
                question: question.to_string(),
                outcomes: outcomes.to_vec(),
                end_time,
                market_type,
                fee_rate,
            }.try_to_vec()?,
        };
        
        // Build and send transaction
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&admin.pubkey()),
            &[admin],
            recent_blockhash,
        );
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(signature.to_string())
    }
    
    /// Place bet
    pub async fn place_bet(
        &self,
        trader: &Keypair,
        market_id: u128,
        outcome: u8,
        amount: u64,
        leverage: u32,
        order_type: OrderType,
    ) -> Result<String> {
        self.place_trade(trader, market_id, amount, outcome, leverage).await
    }
    
    /// Close position with position pubkey
    pub async fn close_position(
        &self,
        trader: &Keypair,
        position: &Pubkey,
    ) -> Result<String> {
        // Build instruction for closing position
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(*position, false),
                AccountMeta::new(trader.pubkey(), true),
            ],
            data: vec![3], // Instruction discriminator for ClosePosition
        };
        
        // Build and send transaction
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&trader.pubkey()),
            &[trader],
            recent_blockhash,
        );
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(signature.to_string())
    }
    
    /// Close a position by index
    pub async fn close_position_by_index(
        &self,
        wallet: &Keypair,
        market_id: u128,
        position_index: u8,
    ) -> Result<String> {
        let market_pda = self.get_market_pda(market_id);
        let position_pda = self.get_position_pda(wallet.pubkey(), market_id);
        let demo_account_pda = self.get_demo_account_pda(&wallet.pubkey());
        
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(demo_account_pda, false),
                AccountMeta::new(market_pda, false),
                AccountMeta::new(wallet.pubkey(), true),
                AccountMeta::new(position_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: BettingInstruction::ClosePosition {
                position_index,
            }.try_to_vec()?,
        };
        
        let mut transaction = Transaction::new_with_payer(
            &[instruction],
            Some(&wallet.pubkey()),
        );
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        transaction.sign(&[wallet], recent_blockhash);
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(signature.to_string())
    }


    /// Process quantum settlement payout
    pub async fn process_quantum_settlement(
        &self,
        wallet: &str,
        position_id: &str,
        payout_amount: u64,
    ) -> Result<String> {
        // Convert wallet string to Pubkey
        let wallet_pubkey = Pubkey::from_str(wallet)?;
        
        // For demo purposes, we'll use the demo account PDA
        let demo_account_pda = self.get_demo_account_pda(&wallet_pubkey);
        
        // Build quantum settlement instruction
        let _instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(demo_account_pda, false),
                AccountMeta::new(wallet_pubkey, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: BettingInstruction::ProcessQuantumSettlement {
                position_id: position_id.to_string(),
                payout_amount,
            }.try_to_vec()?,
        };
        
        // For now, we'll simulate the transaction without a real signer
        // In production, this would be signed by the program's settlement authority
        let _recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        
        // Return a simulated signature
        Ok(format!("quantum_settlement_{}_{}", position_id, chrono::Utc::now().timestamp()))
    }

    // PDA derivations using centralized PDA module
    fn get_market_pda(&self, market_id: u128) -> Pubkey {
        pda::helpers::market_pda(&self.program_id, market_id)
    }

    fn get_position_pda(&self, owner: Pubkey, market_id: u128) -> Pubkey {
        pda::helpers::position_pda(&self.program_id, &owner, market_id)
    }

    fn get_demo_account_pda(&self, owner: &Pubkey) -> Pubkey {
        pda::helpers::demo_account_pda(&self.program_id, owner)
    }

    fn get_verse_pda(&self, verse_id: u128) -> Pubkey {
        pda::helpers::verse_pda(&self.program_id, verse_id)
    }

    fn get_global_config_pda(&self) -> Pubkey {
        pda::helpers::global_config_pda(&self.program_id)
    }
}

struct MarketInfo {
    pubkey: Pubkey,
    market: Market,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signer::Signer;
    use std::sync::Arc;
    
    fn create_test_market(id: u128, title: &str) -> Market {
        Market {
            id,
            title: title.to_string(),
            description: format!("Description for {}", title),
            creator: Pubkey::new_unique(),
            outcomes: vec![
                MarketOutcome {
                    id: 0,
                    name: "Yes".to_string(),
                    title: "Yes".to_string(),
                    description: "Yes outcome".to_string(),
                    total_stake: 50000,
                },
                MarketOutcome {
                    id: 1,
                    name: "No".to_string(),
                    title: "No".to_string(),
                    description: "No outcome".to_string(),
                    total_stake: 50000,
                },
            ],
            amm_type: AmmType::PmAmm,
            total_liquidity: 100000,
            total_volume: 100000,
            resolution_time: chrono::Utc::now().timestamp() + 86400,
            resolved: false,
            winning_outcome: None,
            created_at: chrono::Utc::now().timestamp(),
            verse_id: Some(1),
            current_price: 0.5,
        }
    }
    
    fn create_test_client() -> BettingPlatformClient {
        let rpc_client = Arc::new(RpcClient::new("http://localhost:8899".to_string()));
        let program_id = Pubkey::new_unique();
        BettingPlatformClient::new(rpc_client, program_id)
    }
    
    #[tokio::test]
    async fn test_create_client() {
        let client = create_test_client();
        assert!(!client.program_id.to_string().is_empty());
    }
    
    #[tokio::test]
    async fn test_get_market_pda() {
        let client = create_test_client();
        let market_id = 12345u128;
        
        let pda = client.get_market_pda(market_id);
        
        // Verify PDA is deterministic
        let pda2 = client.get_market_pda(market_id);
        assert_eq!(pda, pda2);
        
        // Different market IDs should produce different PDAs
        let pda3 = client.get_market_pda(market_id + 1);
        assert_ne!(pda, pda3);
    }
    
    #[tokio::test]
    async fn test_get_position_pda() {
        let client = create_test_client();
        let market_id = 12345u128;
        let user = Pubkey::new_unique();
        
        let pda = client.get_position_pda(user, market_id);
        
        // Verify PDA is deterministic
        let pda2 = client.get_position_pda(user, market_id);
        assert_eq!(pda, pda2);
        
        // Different users should produce different PDAs
        let user2 = Pubkey::new_unique();
        let pda3 = client.get_position_pda(user2, market_id);
        assert_ne!(pda, pda3);
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn test_place_trade() {
        let client = create_test_client();
        let user = Keypair::new();
        let market_id = 12345u128;
        let amount = 1000u64;
        let outcome = 0u8;
        let leverage = 5u32;
        
        // This would fail without a real RPC connection
        // Just testing the method exists and has correct signature
        let result = client.place_trade(
            &user,
            market_id,
            amount,
            outcome,
            leverage,
        ).await;
        
        // Expected to fail without real RPC
        assert!(result.is_err());
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn test_create_demo_account() {
        let client = create_test_client();
        let user = Keypair::new();
        
        // This would fail without a real RPC connection
        let result = client.create_demo_account(&user).await;
        
        // Expected to fail without real RPC
        assert!(result.is_err());
    }
    
    #[test]
    fn test_instruction_data_serialization() {
        use borsh::BorshSerialize;
        
        #[derive(BorshSerialize)]
        struct TestInstruction {
            discriminator: u8,
            amount: u64,
            leverage: u32,
        }
        
        let _instruction = TestInstruction {
            discriminator: 0,
            amount: 1000,
            leverage: 5,
        };
        
        let data = _instruction.try_to_vec().unwrap();
        
        // Verify serialization
        assert_eq!(data[0], 0); // discriminator
        assert_eq!(data.len(), 1 + 8 + 4); // u8 + u64 + u32
    }
    
    #[tokio::test]
    async fn test_market_info_struct() {
        let market = create_test_market(1000, "Test Market");
        let pubkey = Pubkey::new_unique();
        
        let market_info = MarketInfo {
            pubkey,
            market: market.clone(),
        };
        
        assert_eq!(market_info.pubkey, pubkey);
        assert_eq!(market_info.market.id, market.id);
        assert_eq!(market_info.market.title, market.title);
    }
    
    #[test]
    fn test_pda_seeds() {
        let client = create_test_client();
        let market_id = 12345u128;
        
        // Test market PDA
        let client_pda = client.get_market_pda(market_id);
        
        // Verify it's not the zero address
        assert_ne!(client_pda, Pubkey::default());
        
        // Test position PDA
        let user = Pubkey::new_unique();
        let client_position_pda = client.get_position_pda(user, market_id);
        
        // Position PDA should be different from market PDA
        assert_ne!(client_pda, client_position_pda);
    }
    
    #[test]
    fn test_multiple_pdas() {
        let client = create_test_client();
        let user = Pubkey::new_unique();
        
        // Create PDAs for multiple markets
        let mut pdas = Vec::new();
        for i in 0..10 {
            let market_id = 1000 + i as u128;
            let pda = client.get_position_pda(user, market_id);
            pdas.push(pda);
        }
        
        // All PDAs should be unique
        for i in 0..pdas.len() {
            for j in (i + 1)..pdas.len() {
                assert_ne!(pdas[i], pdas[j]);
            }
        }
    }
    
    #[test]
    fn test_demo_account_pda() {
        let client = create_test_client();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();
        
        let pda1 = client.get_demo_account_pda(&user1);
        let pda2 = client.get_demo_account_pda(&user2);
        
        // Different users should have different demo account PDAs
        assert_ne!(pda1, pda2);
        
        // Same user should always get same PDA
        let pda1_again = client.get_demo_account_pda(&user1);
        assert_eq!(pda1, pda1_again);
    }
}
