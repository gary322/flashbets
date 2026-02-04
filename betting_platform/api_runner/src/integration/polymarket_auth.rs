//! Polymarket Authentication Module
//! Handles both L1 (private key) and L2 (API key) authentication

use anyhow::{Result, anyhow, Context};
use ethereum_types::{Address, H256, U256};
use web3::signing::{Key, SecretKeyRef};
use secp256k1::{SecretKey, PublicKey, Message, Secp256k1};
use keccak_hash::keccak;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};
use hex;

/// Polymarket authentication configuration
#[derive(Debug, Clone)]
pub struct PolymarketAuthConfig {
    /// API Key (UUID format)
    pub api_key: String,
    /// API Secret for HMAC signing
    pub api_secret: String,
    /// API Passphrase
    pub api_passphrase: String,
    /// Polygon private key for L1 authentication
    pub private_key: Option<String>,
    /// Polygon address
    pub address: Address,
}

impl PolymarketAuthConfig {
    /// Create new auth config from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("POLYMARKET_API_KEY")
            .context("POLYMARKET_API_KEY not set")?;
        let api_secret = std::env::var("POLYMARKET_API_SECRET")
            .context("POLYMARKET_API_SECRET not set")?;
        let api_passphrase = std::env::var("POLYMARKET_API_PASSPHRASE")
            .context("POLYMARKET_API_PASSPHRASE not set")?;
        let private_key = std::env::var("POLYMARKET_PRIVATE_KEY").ok();
        
        // Derive address from private key if available
        let address = if let Some(ref pk) = private_key {
            let private_key_bytes = hex::decode(pk.trim_start_matches("0x"))
                .context("Invalid hex private key")?;
            let secret_key = SecretKey::from_slice(&private_key_bytes)
                .map_err(|e| anyhow!("Invalid private key: {}", e))?;
            
            // Derive address from private key
            let secp = Secp256k1::new();
            let public_key = PublicKey::from_secret_key(&secp, &secret_key);
            let public_key_bytes = public_key.serialize_uncompressed();
            
            // Remove the 0x04 prefix and hash
            let hash = keccak(&public_key_bytes[1..]);
            let mut address_bytes = [0u8; 20];
            address_bytes.copy_from_slice(&hash[12..]);
            Address::from(address_bytes)
        } else {
            // Use a placeholder address if no private key
            Address::zero()
        };
        
        Ok(Self {
            api_key,
            api_secret,
            api_passphrase,
            private_key,
            address,
        })
    }
}

/// L1 Authentication - Uses private key to sign messages
pub struct L1Authenticator {
    secret_key: SecretKey,
    address: Address,
}

impl L1Authenticator {
    /// Create new L1 authenticator from private key
    pub fn new(private_key: &str) -> Result<Self> {
        let private_key_bytes = hex::decode(private_key.trim_start_matches("0x"))
            .context("Invalid hex private key")?;
        let secret_key = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;
        
        // Derive address from private key
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_bytes = public_key.serialize_uncompressed();
        
        // Remove the 0x04 prefix and hash
        let hash = keccak(&public_key_bytes[1..]);
        let mut address_bytes = [0u8; 20];
        address_bytes.copy_from_slice(&hash[12..]);
        let address = Address::from(address_bytes);
        
        Ok(Self { secret_key, address })
    }
    
    /// Generate L1 authentication headers
    pub async fn generate_auth_headers(&self) -> Result<L1AuthHeaders> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let nonce = 0u64; // Default nonce, can be incremented for replay protection
        
        // Create the message to sign
        let message_str = format!("{}{}", timestamp, nonce);
        let message_hash = keccak(message_str.as_bytes());
        
        // Sign the message
        let secp = Secp256k1::new();
        let message = Message::from_slice(message_hash.as_bytes())
            .context("Failed to create message")?;
        let (recovery_id, signature_bytes) = secp.sign_ecdsa_recoverable(&message, &self.secret_key)
            .serialize_compact();
        
        // Format signature with recovery id
        let mut sig_with_recovery = [0u8; 65];
        sig_with_recovery[..64].copy_from_slice(&signature_bytes);
        sig_with_recovery[64] = recovery_id.to_i32() as u8;
        
        Ok(L1AuthHeaders {
            address: format!("0x{:x}", self.address),
            signature: format!("0x{}", hex::encode(sig_with_recovery)),
            timestamp: timestamp.to_string(),
            nonce: nonce.to_string(),
        })
    }
    
    /// Sign an order using EIP-712
    pub async fn sign_order(&self, order: &PolymarketOrderData) -> Result<String> {
        // Create EIP-712 hash
        let domain_separator = self.get_domain_separator()?;
        let struct_hash = self.hash_order_struct(order)?;
        
        // Create the final message to sign
        let mut message_bytes = Vec::new();
        message_bytes.push(0x19);
        message_bytes.push(0x01);
        message_bytes.extend_from_slice(&domain_separator);
        message_bytes.extend_from_slice(&struct_hash);
        
        let message_hash = keccak(&message_bytes);
        
        // Sign the message
        let secp = Secp256k1::new();
        let message = Message::from_slice(message_hash.as_bytes())
            .context("Failed to create message")?;
        let (recovery_id, signature_bytes) = secp.sign_ecdsa_recoverable(&message, &self.secret_key)
            .serialize_compact();
        
        // Format signature with recovery id
        let mut sig_with_recovery = [0u8; 65];
        sig_with_recovery[..64].copy_from_slice(&signature_bytes);
        sig_with_recovery[64] = recovery_id.to_i32() as u8 + 27; // EIP-155 recovery id
        
        Ok(format!("0x{}", hex::encode(sig_with_recovery)))
    }
    
    fn get_domain_separator(&self) -> Result<[u8; 32]> {
        // EIP-712 domain separator for Polymarket
        let domain_type_hash = keccak(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");
        let name_hash = keccak(b"Polymarket");
        let version_hash = keccak(b"1");
        let chain_id = U256::from(137); // Polygon mainnet
        let verifying_contract: Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E".parse()?;
        
        let mut encoder = Vec::new();
        encoder.extend_from_slice(domain_type_hash.as_bytes());
        encoder.extend_from_slice(name_hash.as_bytes());
        encoder.extend_from_slice(version_hash.as_bytes());
        let mut chain_id_bytes = [0u8; 32];
        chain_id.to_big_endian(&mut chain_id_bytes);
        encoder.extend_from_slice(&chain_id_bytes);
        encoder.extend_from_slice(&[0u8; 12]);
        encoder.extend_from_slice(&verifying_contract.0);
        
        let hash = keccak(&encoder);
        let mut result = [0u8; 32];
        result.copy_from_slice(hash.as_bytes());
        Ok(result)
    }
    
    fn hash_order_struct(&self, order: &PolymarketOrderData) -> Result<[u8; 32]> {
        // Hash the order struct according to EIP-712
        let order_type_hash = keccak(
            b"Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)"
        );
        
        let mut encoder = Vec::new();
        encoder.extend_from_slice(order_type_hash.as_bytes());
        
        // Encode all order fields
        // This is simplified - in production you'd properly encode each field
        let salt = U256::from_dec_str(&order.salt).unwrap_or_default();
        let mut salt_bytes = [0u8; 32];
        salt.to_big_endian(&mut salt_bytes);
        encoder.extend_from_slice(&salt_bytes);
        
        let hash = keccak(&encoder);
        let mut result = [0u8; 32];
        result.copy_from_slice(hash.as_bytes());
        Ok(result)
    }
}

/// L1 Authentication headers
#[derive(Debug, Clone, Serialize)]
pub struct L1AuthHeaders {
    #[serde(rename = "POLY_ADDRESS")]
    pub address: String,
    #[serde(rename = "POLY_SIGNATURE")]
    pub signature: String,
    #[serde(rename = "POLY_TIMESTAMP")]
    pub timestamp: String,
    #[serde(rename = "POLY_NONCE")]
    pub nonce: String,
}

/// L2 Authentication - Uses API key and HMAC
pub struct L2Authenticator {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
}

impl L2Authenticator {
    /// Create new L2 authenticator
    pub fn new(api_key: String, api_secret: String, api_passphrase: String) -> Self {
        Self {
            api_key,
            api_secret,
            api_passphrase,
        }
    }
    
    /// Generate L2 authentication headers
    pub fn generate_auth_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<L2AuthHeaders> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        
        // Create the message to sign
        let body_str = body.unwrap_or("");
        let message = format!("{}{}{}{}", timestamp, method, path, body_str);
        
        // Generate HMAC signature
        let signature = self.generate_hmac(&message)?;
        
        Ok(L2AuthHeaders {
            api_key: self.api_key.clone(),
            signature,
            timestamp: timestamp.to_string(),
            passphrase: self.api_passphrase.clone(),
        })
    }
    
    /// Generate HMAC-SHA256 signature
    fn generate_hmac(&self, message: &str) -> Result<String> {
        type HmacSha256 = Hmac<Sha256>;
        
        // Decode the base64 secret
        let secret_bytes = base64::decode(&self.api_secret)
            .context("Failed to decode API secret")?;
        
        let mut mac = HmacSha256::new_from_slice(&secret_bytes)
            .context("Invalid HMAC key")?;
        mac.update(message.as_bytes());
        
        let result = mac.finalize();
        let signature = base64::encode(result.into_bytes());
        
        Ok(signature)
    }
}

/// L2 Authentication headers
#[derive(Debug, Clone, Serialize)]
pub struct L2AuthHeaders {
    #[serde(rename = "POLY_API_KEY")]
    pub api_key: String,
    #[serde(rename = "POLY_SIGNATURE")]
    pub signature: String,
    #[serde(rename = "POLY_TIMESTAMP")]
    pub timestamp: String,
    #[serde(rename = "POLY_PASSPHRASE")]
    pub passphrase: String,
}

/// Polymarket order data for signing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketOrderData {
    pub salt: String,
    pub maker: Address,
    pub signer: Address,
    pub taker: Address,
    pub token_id: String,
    pub maker_amount: String,
    pub taker_amount: String,
    pub expiration: String,
    pub nonce: String,
    pub fee_rate_bps: String,
    pub side: u8,
    pub signature_type: u8,
}


/// Combined authenticator supporting both L1 and L2
pub struct PolymarketAuthenticator {
    l1_auth: Option<L1Authenticator>,
    l2_auth: L2Authenticator,
}

impl PolymarketAuthenticator {
    /// Create new authenticator from config
    pub fn new(config: PolymarketAuthConfig) -> Result<Self> {
        let l1_auth = if let Some(private_key) = config.private_key {
            Some(L1Authenticator::new(&private_key)?)
        } else {
            None
        };
        
        let l2_auth = L2Authenticator::new(
            config.api_key,
            config.api_secret,
            config.api_passphrase,
        );
        
        Ok(Self { l1_auth, l2_auth })
    }
    
    /// Get L1 authenticator
    pub fn l1(&self) -> Option<&L1Authenticator> {
        self.l1_auth.as_ref()
    }
    
    /// Get L2 authenticator
    pub fn l2(&self) -> &L2Authenticator {
        &self.l2_auth
    }
    
    /// Generate headers for API request
    pub async fn generate_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        use_l1: bool,
    ) -> Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        
        if use_l1 {
            // Use L1 authentication
            if let Some(l1) = &self.l1_auth {
                let auth_headers = l1.generate_auth_headers().await?;
                headers.insert("POLY_ADDRESS", auth_headers.address.parse()?);
                headers.insert("POLY_SIGNATURE", auth_headers.signature.parse()?);
                headers.insert("POLY_TIMESTAMP", auth_headers.timestamp.parse()?);
                headers.insert("POLY_NONCE", auth_headers.nonce.parse()?);

                // Also include dash-separated variants for compatibility with some clients/docs.
                headers.insert("POLY-ADDRESS", auth_headers.address.parse()?);
                headers.insert("POLY-SIGNATURE", auth_headers.signature.parse()?);
                headers.insert("POLY-TIMESTAMP", auth_headers.timestamp.parse()?);
                headers.insert("POLY-NONCE", auth_headers.nonce.parse()?);
            } else {
                return Err(anyhow!("L1 authentication not configured"));
            }
        } else {
            // Use L2 authentication
            let auth_headers = self.l2_auth.generate_auth_headers(method, path, body)?;
            headers.insert("POLY_API_KEY", auth_headers.api_key.parse()?);
            headers.insert("POLY_SIGNATURE", auth_headers.signature.parse()?);
            headers.insert("POLY_TIMESTAMP", auth_headers.timestamp.parse()?);
            headers.insert("POLY_PASSPHRASE", auth_headers.passphrase.parse()?);

            // Also include dash-separated variants for compatibility with some clients/docs.
            headers.insert("POLY-API-KEY", auth_headers.api_key.parse()?);
            headers.insert("POLY-SIGNATURE", auth_headers.signature.parse()?);
            headers.insert("POLY-TIMESTAMP", auth_headers.timestamp.parse()?);
            headers.insert("POLY-PASSPHRASE", auth_headers.passphrase.parse()?);
        }
        
        // Add common headers
        headers.insert("Content-Type", "application/json".parse()?);
        headers.insert("Accept", "application/json".parse()?);
        
        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hmac_generation() {
        let auth = L2Authenticator::new(
            "test-key".to_string(),
            base64::encode("test-secret"),
            "test-passphrase".to_string(),
        );
        
        let headers = auth.generate_auth_headers("GET", "/orders", None);
        assert!(headers.is_ok());
    }
}
