//! Wallet utilities for handling both demo and real wallets

use solana_sdk::pubkey::Pubkey;
use serde::{Serialize, Deserialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletType {
    Demo(String),      // Demo wallet with user ID
    Real(Pubkey),      // Real Solana wallet
}

impl WalletType {
    /// Parse a wallet string into WalletType
    pub fn from_string(wallet: &str) -> Result<Self, String> {
        // Check if it's a demo wallet
        if wallet.starts_with("demo-") || wallet.starts_with("advanced-") || wallet.starts_with("pro-") {
            Ok(WalletType::Demo(wallet.to_string()))
        } else {
            // Try to parse as Solana public key
            match Pubkey::from_str(wallet) {
                Ok(pubkey) => Ok(WalletType::Real(pubkey)),
                Err(_) => Err(format!("Invalid wallet address: {}", wallet)),
            }
        }
    }
    
    /// Get the string representation
    pub fn to_string(&self) -> String {
        match self {
            WalletType::Demo(id) => id.clone(),
            WalletType::Real(pubkey) => pubkey.to_string(),
        }
    }
    
    /// Check if this is a demo wallet
    pub fn is_demo(&self) -> bool {
        matches!(self, WalletType::Demo(_))
    }
    
    /// Get as Pubkey if real wallet, or generate deterministic pubkey for demo
    pub fn as_pubkey(&self) -> Pubkey {
        match self {
            WalletType::Real(pubkey) => *pubkey,
            WalletType::Demo(id) => {
                // Generate deterministic pubkey from demo ID
                // This ensures consistency across requests
                use solana_sdk::hash::hash;
                let hash_bytes = hash(id.as_bytes());
                let mut pubkey_bytes = [0u8; 32];
                pubkey_bytes.copy_from_slice(&hash_bytes.as_ref()[..32]);
                Pubkey::new_from_array(pubkey_bytes)
            }
        }
    }
    
    /// Get user ID from demo wallet
    pub fn get_demo_user_id(&self) -> Option<String> {
        match self {
            WalletType::Demo(id) => {
                // Extract user ID from formats like "demo-user_123", "advanced-user_456", etc.
                id.split('-').nth(1).map(|s| s.to_string())
            }
            WalletType::Real(_) => None,
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_demo_wallet_parsing() {
        let demo = WalletType::from_string("demo-user_123").unwrap();
        assert!(demo.is_demo());
        assert_eq!(demo.get_demo_user_id(), Some("user_123".to_string()));
    }
    
    #[test]
    fn test_real_wallet_parsing() {
        let real_wallet = "11111111111111111111111111111111";
        let wallet = WalletType::from_string(real_wallet).unwrap();
        assert!(!wallet.is_demo());
        assert_eq!(wallet.to_string(), real_wallet);
    }
    
    #[test]
    fn test_deterministic_pubkey() {
        let demo1 = WalletType::from_string("demo-user_123").unwrap();
        let demo2 = WalletType::from_string("demo-user_123").unwrap();
        assert_eq!(demo1.as_pubkey(), demo2.as_pubkey());
    }
}