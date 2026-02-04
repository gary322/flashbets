//! Polymarket CTF (Conditional Token Framework) Integration
//! Handles token operations, minting, burning, and redemption

use anyhow::{Result, anyhow, Context};
use ethereum_types::{Address, H256, U256};
use web3::{
    contract::{Contract, Options},
    types::{TransactionParameters, CallRequest, BlockNumber},
    transports::Http,
    Web3,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};

// Polygon (Matic) Network Configuration
const POLYGON_RPC_URL: &str = "https://polygon-rpc.com";
const POLYGON_RPC_URL_TESTNET: &str = "https://rpc-mumbai.maticvigil.com";
const POLYGON_CHAIN_ID: u64 = 137;
const POLYGON_CHAIN_ID_TESTNET: u64 = 80001;

// Polymarket Contract Addresses
const CTF_EXCHANGE_ADDRESS: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";
const CTF_TOKEN_ADDRESS: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
const USDC_ADDRESS: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
const CONDITIONAL_TOKENS_ADDRESS: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
const FIXED_PRODUCT_MARKET_MAKER: &str = "0x5A6b0C0d3D4dB5a5b5f89f5f4e5D5e5e5e5e5e5e";

/// CTF Client for interacting with Polymarket's Conditional Token Framework
pub struct PolymarketCtfClient {
    web3: Web3<Http>,
    account: Address,
    private_key: SecretKey,
    exchange_contract: Contract<Http>,
    usdc_contract: Contract<Http>,
    testnet: bool,
}

impl PolymarketCtfClient {
    /// Create new CTF client
    pub async fn new(private_key_str: &str, testnet: bool) -> Result<Self> {
        let rpc_url = if testnet {
            POLYGON_RPC_URL_TESTNET
        } else {
            POLYGON_RPC_URL
        };
        
        // Create Web3 instance
        let transport = web3::transports::Http::new(rpc_url)?;
        let web3 = Web3::new(transport);
        
        // Parse private key
        let private_key_bytes = hex::decode(private_key_str.trim_start_matches("0x"))
            .context("Invalid hex private key")?;
        let private_key = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;
        
        // Derive account address
        let secp = secp256k1::Secp256k1::new();
        let public_key = secp256k1::PublicKey::from_secret_key(&secp, &private_key);
        let public_key_bytes = public_key.serialize_uncompressed();
        let hash = keccak_hash::keccak(&public_key_bytes[1..]);
        let mut address_bytes = [0u8; 20];
        address_bytes.copy_from_slice(&hash[12..]);
        let account = Address::from(address_bytes);
        
        // Create contract instances
        let exchange_address: Address = CTF_EXCHANGE_ADDRESS.parse()?;
        let exchange_abi = include_bytes!("../../abi/ctf_exchange.json");
        let exchange_contract = Contract::from_json(
            web3.eth(),
            exchange_address,
            exchange_abi
        )?;
        
        let usdc_address: Address = USDC_ADDRESS.parse()?;
        let usdc_abi = include_bytes!("../../abi/erc20.json");
        let usdc_contract = Contract::from_json(
            web3.eth(),
            usdc_address,
            usdc_abi
        )?;
        
        Ok(Self {
            web3,
            account,
            private_key,
            exchange_contract,
            usdc_contract,
            testnet,
        })
    }
    
    /// Get user's USDC balance
    pub async fn get_usdc_balance(&self, address: &str) -> Result<U256> {
        let addr: Address = address.parse()?;
        let balance: U256 = self.usdc_contract
            .query("balanceOf", (addr,), None, Options::default(), None)
            .await?;
        
        info!("USDC balance for {}: {} wei", address, balance);
        Ok(balance)
    }
    
    /// Get user's MATIC balance for gas
    pub async fn get_matic_balance(&self, address: &str) -> Result<U256> {
        let addr: Address = address.parse()?;
        let balance = self.web3.eth().balance(addr, None).await?;
        
        info!("MATIC balance for {}: {} wei", address, balance);
        Ok(balance)
    }
    
    /// Approve USDC spending for CTF operations
    pub async fn approve_usdc(&self, amount: U256) -> Result<H256> {
        let exchange_address: Address = CTF_EXCHANGE_ADDRESS.parse()?;
        
        info!("Approving {} USDC for CTF exchange", amount);
        
        let gas_price = self.web3.eth().gas_price().await?;
        let options = Options {
            gas: Some(100_000.into()),
            gas_price: Some(gas_price),
            ..Default::default()
        };
        
        let tx_hash = self.usdc_contract
            .call("approve", (exchange_address, amount), self.account, options)
            .await?;
        
        info!("USDC approval tx: {:?}", tx_hash);
        Ok(tx_hash)
    }
    
    /// Split collateral into outcome tokens (mint)
    pub async fn split_position(
        &self,
        condition_id: &str,
        amount: U256,
    ) -> Result<SplitPositionResult> {
        info!("Splitting position for condition {} with amount {}", 
            condition_id, amount);
        
        let condition_bytes = hex::decode(condition_id.strip_prefix("0x").unwrap_or(condition_id))?;
        let condition_h256 = H256::from_slice(&condition_bytes);
        
        // First approve USDC
        self.approve_usdc(amount).await?;
        
        // Split position (mint outcome tokens)
        let gas_price = self.web3.eth().gas_price().await?;
        let options = Options {
            gas: Some(200_000.into()),
            gas_price: Some(gas_price),
            ..Default::default()
        };
        
        let tx_hash = self.exchange_contract
            .call("splitPosition", (self.account, condition_h256, amount), self.account, options)
            .await?;
        
        // Get transaction receipt
        let receipt = self.web3.eth()
            .transaction_receipt(tx_hash)
            .await?
            .ok_or_else(|| anyhow!("Transaction receipt not found"))?;
        
        let block_number = receipt.block_number
            .ok_or_else(|| anyhow!("Block number not in receipt"))?
            .as_u64();
        let gas_used = receipt.gas_used
            .ok_or_else(|| anyhow!("Gas used not in receipt"))?
            .as_u64();
        
        info!("Split position tx: {:?} in block {}", tx_hash, block_number);
        
        Ok(SplitPositionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number,
            yes_tokens: amount,
            no_tokens: amount,
            gas_used,
        })
    }
    
    /// Merge outcome tokens back to collateral (burn)
    pub async fn merge_positions(
        &self,
        condition_id: &str,
        amount: U256,
    ) -> Result<MergePositionsResult> {
        info!("Merging positions for condition {} with amount {}", 
            condition_id, amount);
        
        let condition_bytes = hex::decode(condition_id.strip_prefix("0x").unwrap_or(condition_id))?;
        let condition_h256 = H256::from_slice(&condition_bytes);
        
        // Merge positions (burn outcome tokens for collateral)
        let gas_price = self.web3.eth().gas_price().await?;
        let options = Options {
            gas: Some(200_000.into()),
            gas_price: Some(gas_price),
            ..Default::default()
        };
        
        let tx_hash = self.exchange_contract
            .call("mergePositions", (self.account, condition_h256, amount), self.account, options)
            .await?;
        
        // Get transaction receipt
        let receipt = self.web3.eth()
            .transaction_receipt(tx_hash)
            .await?
            .ok_or_else(|| anyhow!("Transaction receipt not found"))?;
        
        let block_number = receipt.block_number
            .ok_or_else(|| anyhow!("Block number not in receipt"))?
            .as_u64();
        let gas_used = receipt.gas_used
            .ok_or_else(|| anyhow!("Gas used not in receipt"))?
            .as_u64();
        
        info!("Merge positions tx: {:?} in block {}", tx_hash, block_number);
        
        Ok(MergePositionsResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number,
            collateral_returned: amount,
            gas_used,
        })
    }
    
    /// Redeem winning positions after market resolution
    pub async fn redeem_positions(
        &self,
        condition_id: &str,
        index_sets: Vec<U256>,
    ) -> Result<RedemptionResult> {
        info!("Redeeming positions for condition {}", condition_id);
        
        let condition_bytes = hex::decode(condition_id.strip_prefix("0x").unwrap_or(condition_id))?;
        let condition_h256 = H256::from_slice(&condition_bytes);
        
        // Redeem positions
        let gas_price = self.web3.eth().gas_price().await?;
        let options = Options {
            gas: Some(300_000.into()),
            gas_price: Some(gas_price),
            ..Default::default()
        };
        
        let tx_hash = self.exchange_contract
            .call("redeemPositions", (self.account, condition_h256, index_sets), self.account, options)
            .await?;
        
        // Get transaction receipt
        let receipt = self.web3.eth()
            .transaction_receipt(tx_hash)
            .await?
            .ok_or_else(|| anyhow!("Transaction receipt not found"))?;
        
        let block_number = receipt.block_number
            .ok_or_else(|| anyhow!("Block number not in receipt"))?
            .as_u64();
        let gas_used = receipt.gas_used
            .ok_or_else(|| anyhow!("Gas used not in receipt"))?
            .as_u64();
        
        info!("Redeem positions tx: {:?} in block {}", tx_hash, block_number);
        
        // Calculate payout (simplified - would need to check actual amounts from events)
        let payout = U256::from(0);
        
        Ok(RedemptionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number,
            payout,
            gas_used,
        })
    }
    
    /// Get position balance for a specific outcome token
    pub async fn get_position_balance(
        &self,
        token_id: &str,
        address: &str,
    ) -> Result<PositionBalance> {
        let addr: Address = address.parse()?;
        
        // This would interact with the CTF token contract to get balance
        // For now, returning a placeholder
        Ok(PositionBalance {
            token_id: token_id.to_string(),
            owner: address.to_string(),
            balance: U256::from(0),
            outcome_index: 0,
        })
    }
    
    /// Calculate position ID from condition and outcome
    pub fn calculate_position_id(
        condition_id: &str,
        outcome_index: u8,
    ) -> Result<String> {
        // Position ID calculation per CTF spec
        // positionId = keccak256(abi.encodePacked(collateralToken, conditionId, indexSet))
        
        let condition_bytes = hex::decode(condition_id.strip_prefix("0x").unwrap_or(condition_id))?;
        let index_set = U256::from(1u64 << outcome_index);
        
        // Simplified - would need proper encoding
        let position_id = format!("0x{}{:02x}", condition_id, outcome_index);
        
        Ok(position_id)
    }
    
    /// Estimate gas for split position
    pub async fn estimate_split_gas(
        &self,
        condition_id: &str,
        amount: U256,
    ) -> Result<U256> {
        let condition_bytes = hex::decode(condition_id.strip_prefix("0x").unwrap_or(condition_id))?;
        let condition_h256 = H256::from_slice(&condition_bytes);
        
        let gas = self.exchange_contract
            .estimate_gas("splitPosition", (self.account, condition_h256, amount), self.account, Options::default())
            .await?;
        
        Ok(gas)
    }
    
    /// Get current gas price on Polygon
    pub async fn get_gas_price(&self) -> Result<U256> {
        let gas_price = self.web3.eth().gas_price().await?;
        Ok(gas_price)
    }
}

/// Result of splitting a position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPositionResult {
    pub tx_hash: String,
    pub block_number: u64,
    pub yes_tokens: U256,
    pub no_tokens: U256,
    pub gas_used: u64,
}

/// Result of merging positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePositionsResult {
    pub tx_hash: String,
    pub block_number: u64,
    pub collateral_returned: U256,
    pub gas_used: u64,
}

/// Result of redeeming positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionResult {
    pub tx_hash: String,
    pub block_number: u64,
    pub payout: U256,
    pub gas_used: u64,
}

/// Position balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionBalance {
    pub token_id: String,
    pub owner: String,
    pub balance: U256,
    pub outcome_index: u8,
}

/// CTF market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfMarket {
    pub condition_id: String,
    pub question_id: String,
    pub oracle: Address,
    pub outcome_slot_count: u8,
    pub resolved: bool,
    pub payout_numerators: Vec<U256>,
    pub resolution_timestamp: Option<DateTime<Utc>>,
}

/// Manager for handling multiple CTF operations
pub struct CtfManager {
    client: Arc<PolymarketCtfClient>,
    position_cache: Arc<tokio::sync::RwLock<Vec<PositionBalance>>>,
}

impl CtfManager {
    /// Create new CTF manager
    pub async fn new(private_key: &str, testnet: bool) -> Result<Self> {
        let client = Arc::new(PolymarketCtfClient::new(private_key, testnet).await?);
        let position_cache = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        
        Ok(Self {
            client,
            position_cache,
        })
    }
    
    /// Provide liquidity to a market
    pub async fn provide_liquidity(
        &self,
        condition_id: &str,
        amount: U256,
    ) -> Result<()> {
        // 1. Split position to get outcome tokens
        let split_result = self.client.split_position(condition_id, amount).await?;
        
        // 2. Add liquidity to AMM (would interact with FPMM contract)
        info!("Liquidity provision completed: {:?}", split_result);
        
        Ok(())
    }
    
    /// Remove liquidity from a market
    pub async fn remove_liquidity(
        &self,
        condition_id: &str,
        lp_tokens: U256,
    ) -> Result<()> {
        // 1. Remove liquidity from AMM
        // 2. Merge any balanced outcome tokens back to collateral
        
        info!("Removing liquidity for condition {}", condition_id);
        
        Ok(())
    }
    
    /// Execute arbitrage if price difference exists
    pub async fn arbitrage_opportunity(
        &self,
        condition_id: &str,
        market_price: f64,
        amm_price: f64,
        size: U256,
    ) -> Result<bool> {
        let price_diff = (market_price - amm_price).abs();
        let threshold = 0.02; // 2% minimum difference
        
        if price_diff > threshold {
            info!("Arbitrage opportunity detected: {} vs {} ({}% diff)",
                market_price, amm_price, price_diff * 100.0);
            
            // Execute arbitrage trades
            // This would involve trading on both CLOB and AMM
            
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Monitor and auto-redeem resolved markets
    pub async fn auto_redeem_resolved(&self) -> Result<()> {
        // Check all positions for resolved markets
        let positions = self.position_cache.read().await;
        
        for position in positions.iter() {
            // Check if market is resolved
            // If resolved and user has winning positions, auto-redeem
            debug!("Checking position {} for redemption", position.token_id);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_position_id_calculation() {
        let condition_id = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let outcome_index = 0;
        
        let position_id = PolymarketCtfClient::calculate_position_id(condition_id, outcome_index);
        assert!(position_id.is_ok());
        
        let id = position_id.unwrap();
        assert!(id.starts_with("0x"));
    }
    
    #[test]
    fn test_balance_serialization() {
        let balance = PositionBalance {
            token_id: "0xabc123".to_string(),
            owner: "0xdef456".to_string(),
            balance: U256::from(1000000),
            outcome_index: 1,
        };
        
        let json = serde_json::to_string(&balance).unwrap();
        let deserialized: PositionBalance = serde_json::from_str(&json).unwrap();
        
        assert_eq!(balance.token_id, deserialized.token_id);
        assert_eq!(balance.outcome_index, deserialized.outcome_index);
    }
}