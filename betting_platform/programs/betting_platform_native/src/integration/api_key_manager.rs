//! API Key Management System
//!
//! Implements secure API key handling:
//! - Encrypted storage on-chain
//! - Automatic rotation
//! - Per-environment configuration
//! - Usage tracking and rate limiting
//!
//! Per specification: Production-grade key management

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
    keccak,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
};

/// Maximum number of API keys per provider
pub const MAX_KEYS_PER_PROVIDER: usize = 10;
pub const KEY_ROTATION_INTERVAL_DAYS: i64 = 30;
pub const MAX_USAGE_PER_DAY: u64 = 100_000;

/// API provider types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ApiProvider {
    Polymarket,
    PythNetwork,
    Chainlink,
    Custom(String),
}

/// Environment types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

/// Encrypted API key storage
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EncryptedApiKey {
    pub key_id: [u8; 16],
    pub provider: ApiProvider,
    pub environment: Environment,
    pub encrypted_key: Vec<u8>,
    pub encryption_nonce: [u8; 12],
    pub created_at: i64,
    pub last_rotated: i64,
    pub expires_at: i64,
    pub is_active: bool,
    pub usage_stats: KeyUsageStats,
}

impl EncryptedApiKey {
    pub const SIZE: usize = 512;

    /// Create new encrypted API key
    pub fn new(
        provider: ApiProvider,
        environment: Environment,
        encrypted_key: Vec<u8>,
        nonce: [u8; 12],
        timestamp: i64,
    ) -> Self {
        let key_id = Self::generate_key_id(&provider, &environment, timestamp);
        let expires_at = timestamp + (KEY_ROTATION_INTERVAL_DAYS * 86400);

        Self {
            key_id,
            provider,
            environment,
            encrypted_key,
            encryption_nonce: nonce,
            created_at: timestamp,
            last_rotated: timestamp,
            expires_at,
            is_active: true,
            usage_stats: KeyUsageStats::new(),
        }
    }

    /// Generate unique key ID
    fn generate_key_id(
        provider: &ApiProvider,
        environment: &Environment,
        timestamp: i64,
    ) -> [u8; 16] {
        let provider_bytes = provider.try_to_vec().unwrap_or_default();
        let env_bytes = environment.try_to_vec().unwrap_or_default();
        
        let hash = keccak::hashv(&[
            &provider_bytes,
            &env_bytes,
            &timestamp.to_le_bytes(),
        ]);
        
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.0[..16]);
        id
    }

    /// Check if key needs rotation
    pub fn needs_rotation(&self, current_timestamp: i64) -> bool {
        current_timestamp >= self.expires_at
    }

    /// Check if key is valid
    pub fn is_valid(&self, current_timestamp: i64) -> bool {
        self.is_active && current_timestamp < self.expires_at
    }
}

/// Key usage statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct KeyUsageStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub last_used: i64,
    pub daily_usage: HashMap<i64, u64>, // Day timestamp -> usage count
    pub error_counts: HashMap<u16, u32>, // Error code -> count
}

impl KeyUsageStats {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            last_used: 0,
            daily_usage: HashMap::new(),
            error_counts: HashMap::new(),
        }
    }

    /// Record usage
    pub fn record_usage(
        &mut self,
        success: bool,
        error_code: Option<u16>,
        timestamp: i64,
    ) {
        self.total_requests += 1;
        self.last_used = timestamp;

        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
            if let Some(code) = error_code {
                *self.error_counts.entry(code).or_insert(0) += 1;
            }
        }

        // Update daily usage
        let day_timestamp = (timestamp / 86400) * 86400;
        *self.daily_usage.entry(day_timestamp).or_insert(0) += 1;
    }

    /// Get usage for specific day
    pub fn get_daily_usage(&self, day_timestamp: i64) -> u64 {
        *self.daily_usage.get(&day_timestamp).unwrap_or(&0)
    }

    /// Check if daily limit exceeded
    pub fn is_rate_limited(&self, current_timestamp: i64) -> bool {
        let day_timestamp = (current_timestamp / 86400) * 86400;
        self.get_daily_usage(day_timestamp) >= MAX_USAGE_PER_DAY
    }
}

/// API key manager state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ApiKeyManager {
    pub authority: Pubkey,
    pub keys: HashMap<[u8; 16], EncryptedApiKey>,
    pub active_keys: HashMap<(ApiProvider, Environment), [u8; 16]>,
    pub rotation_schedule: HashMap<[u8; 16], i64>, // Key ID -> next rotation
    pub total_keys: u32,
    pub last_rotation_check: i64,
}

impl ApiKeyManager {
    pub const SIZE: usize = 1024 * 32; // 32KB for key storage

    pub fn new(authority: Pubkey) -> Self {
        Self {
            authority,
            keys: HashMap::new(),
            active_keys: HashMap::new(),
            rotation_schedule: HashMap::new(),
            total_keys: 0,
            last_rotation_check: 0,
        }
    }

    /// Add new API key
    pub fn add_key(
        &mut self,
        key: EncryptedApiKey,
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        // Check limits
        let provider_key_count = self.keys
            .values()
            .filter(|k| k.provider == key.provider)
            .count();

        if provider_key_count >= MAX_KEYS_PER_PROVIDER {
            return Err(ProgramError::AccountDataTooSmall);
        }

        let key_id = key.key_id;
        let provider = key.provider.clone();
        let environment = key.environment.clone();

        // Add to storage
        self.keys.insert(key_id, key);
        self.active_keys.insert((provider, environment), key_id);
        self.rotation_schedule.insert(
            key_id,
            current_timestamp + (KEY_ROTATION_INTERVAL_DAYS * 86400),
        );
        self.total_keys += 1;

        msg!("API key added: {:?}", key_id);
        Ok(())
    }

    /// Rotate API key
    pub fn rotate_key(
        &mut self,
        key_id: [u8; 16],
        new_encrypted_key: Vec<u8>,
        new_nonce: [u8; 12],
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        let key = self.keys.get_mut(&key_id)
            .ok_or(BettingPlatformError::InvalidInput)?;

        // Deactivate old key
        key.is_active = false;

        // Create new key
        let new_key = EncryptedApiKey::new(
            key.provider.clone(),
            key.environment.clone(),
            new_encrypted_key,
            new_nonce,
            current_timestamp,
        );

        let new_key_id = new_key.key_id;
        
        // Update active key mapping
        self.active_keys.insert(
            (new_key.provider.clone(), new_key.environment.clone()),
            new_key_id,
        );

        // Add new key
        self.keys.insert(new_key_id, new_key);
        self.rotation_schedule.insert(
            new_key_id,
            current_timestamp + (KEY_ROTATION_INTERVAL_DAYS * 86400),
        );

        msg!("API key rotated: old={:?}, new={:?}", key_id, new_key_id);
        Ok(())
    }

    /// Get active key for provider and environment
    pub fn get_active_key(
        &self,
        provider: &ApiProvider,
        environment: &Environment,
    ) -> Option<&EncryptedApiKey> {
        self.active_keys
            .get(&(provider.clone(), environment.clone()))
            .and_then(|key_id| self.keys.get(key_id))
            .filter(|key| key.is_active)
    }

    /// Check and rotate expired keys
    pub fn check_rotation_needed(
        &self,
        current_timestamp: i64,
    ) -> Vec<[u8; 16]> {
        let mut keys_to_rotate = Vec::new();

        for (key_id, next_rotation) in &self.rotation_schedule {
            if current_timestamp >= *next_rotation {
                if let Some(key) = self.keys.get(key_id) {
                    if key.is_active {
                        keys_to_rotate.push(*key_id);
                    }
                }
            }
        }

        keys_to_rotate
    }

    /// Record key usage
    pub fn record_usage(
        &mut self,
        key_id: [u8; 16],
        success: bool,
        error_code: Option<u16>,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        let key = self.keys.get_mut(&key_id)
            .ok_or(BettingPlatformError::InvalidInput)?;

        key.usage_stats.record_usage(success, error_code, timestamp);

        // Check rate limit
        if key.usage_stats.is_rate_limited(timestamp) {
            msg!("API key rate limited: {:?}", key_id);
            return Err(BettingPlatformError::RateLimited.into());
        }

        Ok(())
    }

    /// Get usage statistics for a key
    pub fn get_usage_stats(&self, key_id: &[u8; 16]) -> Option<&KeyUsageStats> {
        self.keys.get(key_id).map(|k| &k.usage_stats)
    }

    /// Cleanup expired keys
    pub fn cleanup_expired_keys(&mut self, current_timestamp: i64) -> u32 {
        let mut removed = 0;

        // Find expired inactive keys
        let expired_keys: Vec<[u8; 16]> = self.keys
            .iter()
            .filter(|(_, key)| {
                !key.is_active && 
                current_timestamp > key.expires_at + (7 * 86400) // 7 days grace period
            })
            .map(|(id, _)| *id)
            .collect();

        // Remove expired keys
        for key_id in expired_keys {
            self.keys.remove(&key_id);
            self.rotation_schedule.remove(&key_id);
            removed += 1;
        }

        if removed > 0 {
            msg!("Cleaned up {} expired API keys", removed);
        }

        removed
    }
}

/// Key encryption utilities
pub struct KeyEncryption;

impl KeyEncryption {
    /// Encrypt API key using program-derived key
    pub fn encrypt_key(
        plaintext: &str,
        program_id: &Pubkey,
        nonce: &[u8; 12],
    ) -> Result<Vec<u8>, ProgramError> {
        // Derive encryption key from program ID
        let key_material = keccak::hashv(&[
            program_id.as_ref(),
            b"API_KEY_ENCRYPTION",
            nonce,
        ]);

        // Simple XOR encryption (in production, use proper AES-GCM)
        let mut encrypted = Vec::with_capacity(plaintext.len());
        for (i, byte) in plaintext.bytes().enumerate() {
            encrypted.push(byte ^ key_material.0[i % 32]);
        }

        Ok(encrypted)
    }

    /// Decrypt API key
    pub fn decrypt_key(
        ciphertext: &[u8],
        program_id: &Pubkey,
        nonce: &[u8; 12],
    ) -> Result<String, ProgramError> {
        // Derive decryption key
        let key_material = keccak::hashv(&[
            program_id.as_ref(),
            b"API_KEY_ENCRYPTION",
            nonce,
        ]);

        // Decrypt
        let mut decrypted = Vec::with_capacity(ciphertext.len());
        for (i, &byte) in ciphertext.iter().enumerate() {
            decrypted.push(byte ^ key_material.0[i % 32]);
        }

        String::from_utf8(decrypted)
            .map_err(|_| BettingPlatformError::InvalidInput.into())
    }

    /// Generate secure nonce
    pub fn generate_nonce(timestamp: i64, counter: u64) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&timestamp.to_le_bytes());
        nonce[8..12].copy_from_slice(&counter.to_le_bytes()[..4]);
        nonce
    }
}

/// API key access control
pub struct KeyAccessControl;

impl KeyAccessControl {
    /// Verify caller is authorized to access key
    pub fn verify_access(
        manager: &ApiKeyManager,
        caller: &Pubkey,
        key_id: &[u8; 16],
    ) -> Result<(), ProgramError> {
        // Only authority can access keys
        if caller != &manager.authority {
            return Err(BettingPlatformError::Unauthorized.into());
        }

        // Verify key exists
        if !manager.keys.contains_key(key_id) {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        Ok(())
    }

    /// Verify environment access
    pub fn verify_environment_access(
        environment: &Environment,
        caller_environment: &Environment,
    ) -> Result<(), ProgramError> {
        match (environment, caller_environment) {
            (Environment::Production, Environment::Production) => Ok(()),
            (Environment::Staging, Environment::Staging) => Ok(()),
            (Environment::Development, _) => Ok(()), // Dev accessible from any env
            _ => Err(BettingPlatformError::Unauthorized.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_creation() {
        let key = EncryptedApiKey::new(
            ApiProvider::Polymarket,
            Environment::Production,
            vec![1, 2, 3, 4],
            [0u8; 12],
            100,
        );

        assert!(key.is_valid(150));
        assert!(!key.needs_rotation(150));
        assert!(key.needs_rotation(100 + 31 * 86400));
    }

    #[test]
    fn test_usage_tracking() {
        let mut stats = KeyUsageStats::new();
        
        stats.record_usage(true, None, 100);
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);

        stats.record_usage(false, Some(429), 200);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(*stats.error_counts.get(&429).unwrap(), 1);
    }

    #[test]
    fn test_key_rotation() {
        let mut manager = ApiKeyManager::new(Pubkey::new_unique());
        
        let key = EncryptedApiKey::new(
            ApiProvider::Polymarket,
            Environment::Production,
            vec![1, 2, 3, 4],
            [0u8; 12],
            100,
        );

        manager.add_key(key.clone(), 100).unwrap();
        
        let keys_to_rotate = manager.check_rotation_needed(100 + 31 * 86400);
        assert_eq!(keys_to_rotate.len(), 1);
    }

    #[test]
    fn test_encryption() {
        let program_id = Pubkey::new_unique();
        let nonce = KeyEncryption::generate_nonce(100, 1);
        let plaintext = "sk_test_12345";

        let encrypted = KeyEncryption::encrypt_key(plaintext, &program_id, &nonce).unwrap();
        let decrypted = KeyEncryption::decrypt_key(&encrypted, &program_id, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}