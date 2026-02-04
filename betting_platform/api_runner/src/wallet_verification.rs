//! Wallet signature verification for secure authentication

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Signature, Signer},
    signer::keypair::Keypair,
};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Challenge nonce for wallet verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub nonce: String,
    pub message: String,
    pub expires_at: u64,
    pub created_at: u64,
}

/// Verification request from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub wallet: String,
    pub signature: String,
    #[serde(alias = "challenge")]
    pub message: String,
    #[serde(default)]
    pub nonce: String,
}

/// Verification response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResponse {
    pub verified: bool,
    pub wallet: String,
    pub token: Option<String>,
    pub expires_at: Option<u64>,
}

/// Challenge response for client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub nonce: String,
    pub message: String,
    pub expires_at: u64,
    #[serde(rename = "challenge")]
    pub challenge_compat: String, // For test compatibility
}

/// Wallet verification service
pub struct WalletVerificationService {
    challenges: Arc<RwLock<HashMap<String, Challenge>>>,
    verified_wallets: Arc<RwLock<HashMap<String, u64>>>, // wallet -> expires_at
    challenge_duration: u64, // seconds
    token_duration: u64, // seconds
}

impl WalletVerificationService {
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            verified_wallets: Arc::new(RwLock::new(HashMap::new())),
            challenge_duration: 300, // 5 minutes
            token_duration: 3600, // 1 hour
        }
    }

    /// Generate a challenge for wallet verification
    pub async fn generate_challenge(&self, wallet: &str) -> Result<ChallengeResponse> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let nonce = uuid::Uuid::new_v4().to_string();
        let message = format!(
            "Sign this message to verify your wallet ownership.\nWallet: {}\nNonce: {}\nTimestamp: {}",
            wallet, nonce, now
        );

        let challenge = Challenge {
            nonce: nonce.clone(),
            message: message.clone(),
            expires_at: now + self.challenge_duration,
            created_at: now,
        };

        // Store challenge
        let mut challenges = self.challenges.write().await;
        challenges.insert(wallet.to_string(), challenge);

        // Clean up expired challenges
        let expired_keys: Vec<_> = challenges
            .iter()
            .filter(|(_, c)| c.expires_at < now)
            .map(|(k, _)| k.clone())
            .collect();
        
        for key in expired_keys {
            challenges.remove(&key);
        }

        Ok(ChallengeResponse {
            nonce: nonce.clone(),
            message: message.clone(),
            challenge_compat: message,
            expires_at: (now + self.challenge_duration) * 1000, // Convert to milliseconds for JS Date compatibility
        })
    }

    /// Verify wallet signature
    pub async fn verify_signature(&self, request: VerificationRequest) -> Result<VerificationResponse> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Check if wallet is demo wallet
        if request.wallet.starts_with("demo-") || 
           request.wallet.starts_with("advanced-") || 
           request.wallet.starts_with("pro-") {
            info!("Demo wallet verification for: {}", request.wallet);
            
            // For demo wallets, accept any signature and generate token
            let token = self.generate_token(&request.wallet, now + self.token_duration);
            
            // Store verification
            let mut verified = self.verified_wallets.write().await;
            verified.insert(request.wallet.clone(), now + self.token_duration);
            
            return Ok(VerificationResponse {
                verified: true,
                wallet: request.wallet,
                token: Some(token),
                expires_at: Some(now + self.token_duration),
            });
        }

        // Get and validate challenge
        let challenges = self.challenges.read().await;
        let challenge = match challenges.get(&request.wallet) {
            Some(c) => c,
            None => {
                warn!("No challenge found for wallet: {}", request.wallet);
                return Ok(VerificationResponse {
                    verified: false,
                    wallet: request.wallet,
                    token: None,
                    expires_at: None,
                });
            }
        };

        // Check if challenge expired
        if challenge.expires_at < now {
            warn!("Expired challenge for wallet: {}", request.wallet);
            return Ok(VerificationResponse {
                verified: false,
                wallet: request.wallet,
                token: None,
                expires_at: None,
            });
        }

        // Verify nonce matches if provided
        if !request.nonce.is_empty() && challenge.nonce != request.nonce {
            warn!("Invalid nonce for wallet: {}", request.wallet);
            return Ok(VerificationResponse {
                verified: false,
                wallet: request.wallet,
                token: None,
                expires_at: None,
            });
        }

        // Verify message matches
        if challenge.message != request.message {
            warn!("Message mismatch for wallet: {}", request.wallet);
            return Ok(VerificationResponse {
                verified: false,
                wallet: request.wallet,
                token: None,
                expires_at: None,
            });
        }

        // Verify Solana signature
        let verification_result = self.verify_solana_signature(
            &request.wallet,
            &request.message,
            &request.signature,
        ).await;

        match verification_result {
            Ok(true) => {
                info!("Successfully verified wallet signature: {}", request.wallet);
                
                // Generate token
                let token = self.generate_token(&request.wallet, now + self.token_duration);
                
                // Store verification
                let mut verified = self.verified_wallets.write().await;
                verified.insert(request.wallet.clone(), now + self.token_duration);
                
                // Clean up challenge
                drop(challenges);
                let mut challenges_mut = self.challenges.write().await;
                challenges_mut.remove(&request.wallet);
                
                Ok(VerificationResponse {
                    verified: true,
                    wallet: request.wallet,
                    token: Some(token),
                    expires_at: Some(now + self.token_duration),
                })
            }
            Ok(false) => {
                warn!("Invalid signature for wallet: {}", request.wallet);
                Ok(VerificationResponse {
                    verified: false,
                    wallet: request.wallet,
                    token: None,
                    expires_at: None,
                })
            }
            Err(e) => {
                error!("Signature verification error for wallet {}: {}", request.wallet, e);
                Ok(VerificationResponse {
                    verified: false,
                    wallet: request.wallet,
                    token: None,
                    expires_at: None,
                })
            }
        }
    }

    /// Verify Solana signature
    async fn verify_solana_signature(
        &self,
        wallet_str: &str,
        message: &str,
        signature_str: &str,
    ) -> Result<bool> {
        // Parse wallet address
        let wallet_pubkey = Pubkey::from_str(wallet_str)?;
        
        // Parse signature
        let signature = Signature::from_str(signature_str)?;
        
        // Convert message to bytes
        let message_bytes = message.as_bytes();
        
        // Verify signature
        let verified = signature.verify(wallet_pubkey.as_ref(), message_bytes);
        
        Ok(verified)
    }

    /// Check if wallet is verified
    pub async fn is_wallet_verified(&self, wallet: &str) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let verified = self.verified_wallets.read().await;
        
        if let Some(&expires_at) = verified.get(wallet) {
            expires_at > now
        } else {
            false
        }
    }

    /// Validate token
    pub async fn validate_token(&self, token: &str) -> Option<String> {
        // Simple token format: wallet|expires_at|hash
        let parts: Vec<&str> = token.split('|').collect();
        if parts.len() != 3 {
            return None;
        }

        let wallet = parts[0];
        let expires_at: u64 = parts[1].parse().ok()?;
        let provided_hash = parts[2];

        // Check expiration
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        if expires_at < now {
            return None;
        }

        // Verify hash
        let expected_hash = self.hash_token(wallet, expires_at);
        if provided_hash != expected_hash {
            return None;
        }

        Some(wallet.to_string())
    }

    /// Generate verification token
    fn generate_token(&self, wallet: &str, expires_at: u64) -> String {
        let hash = self.hash_token(wallet, expires_at);
        format!("{}|{}|{}", wallet, expires_at, hash)
    }

    /// Hash token components
    fn hash_token(&self, wallet: &str, expires_at: u64) -> String {
        use sha2::{Sha256, Digest};
        
        let secret = "betting_platform_secret_key"; // In production, use env var
        let data = format!("{}|{}|{}", wallet, expires_at, secret);
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Clean up expired data
    pub async fn cleanup_expired(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Clean up challenges
        let mut challenges = self.challenges.write().await;
        challenges.retain(|_, challenge| challenge.expires_at > now);
        
        // Clean up verified wallets
        let mut verified = self.verified_wallets.write().await;
        verified.retain(|_, &mut expires_at| expires_at > now);
        
        info!("Cleaned up expired wallet verification data");
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(service: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                service.cleanup_expired().await;
            }
        });
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_wallet_verification() {
        let service = WalletVerificationService::new();
        
        let request = VerificationRequest {
            wallet: "demo-test123".to_string(),
            signature: "fake_signature".to_string(),
            message: "test message".to_string(),
            nonce: "test_nonce".to_string(),
        };
        
        let response = service.verify_signature(request).await.unwrap();
        assert!(response.verified);
        assert!(response.token.is_some());
    }

    #[tokio::test]
    async fn test_challenge_generation() {
        let service = WalletVerificationService::new();
        let wallet = "11111111111111111111111111111112";
        
        let challenge = service.generate_challenge(wallet).await.unwrap();
        assert!(!challenge.nonce.is_empty());
        assert!(challenge.message.contains(wallet));
    }

    #[tokio::test]
    async fn test_token_validation() {
        let service = WalletVerificationService::new();
        let wallet = "test_wallet";
        let expires_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600;
        
        let token = service.generate_token(wallet, expires_at);
        let validated_wallet = service.validate_token(&token).await;
        
        assert_eq!(validated_wallet, Some(wallet.to_string()));
    }
}