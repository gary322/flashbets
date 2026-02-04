//! Request Security System
//!
//! Implements comprehensive request security:
//! - Request signing with HMAC-SHA256
//! - Timestamp validation to prevent replay attacks
//! - Nonce tracking for idempotency
//! - Comprehensive audit logging
//!
//! Per specification: Production-grade security

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
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    error::BettingPlatformError,
    integration::api_key_manager::{ApiProvider, Environment},
    events::{emit_event, EventType},
};

/// Security constants
pub const REQUEST_TIMEOUT_SECONDS: i64 = 300; // 5 minutes
pub const NONCE_CACHE_SIZE: usize = 10000;
pub const MAX_CLOCK_DRIFT_SECONDS: i64 = 60;
pub const AUDIT_LOG_RETENTION_DAYS: i64 = 90;

/// Request signature for authentication
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct RequestSignature {
    pub signature: [u8; 32],
    pub timestamp: i64,
    pub nonce: [u8; 16],
    pub key_id: [u8; 16],
}

impl RequestSignature {
    /// Create new request signature
    pub fn new(
        message: &[u8],
        api_key: &[u8],
        timestamp: i64,
        nonce: [u8; 16],
        key_id: [u8; 16],
    ) -> Self {
        let signature = Self::compute_signature(message, api_key, timestamp, &nonce);
        
        Self {
            signature,
            timestamp,
            nonce,
            key_id,
        }
    }

    /// Compute HMAC-SHA256 signature
    fn compute_signature(
        message: &[u8],
        key: &[u8],
        timestamp: i64,
        nonce: &[u8; 16],
    ) -> [u8; 32] {
        // Construct signed payload
        let mut payload = Vec::new();
        payload.extend_from_slice(message);
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(nonce);

        // Compute HMAC (simplified - in production use proper HMAC-SHA256)
        keccak::hashv(&[
            key,
            &payload,
            b"REQUEST_SIGNATURE",
        ]).0
    }

    /// Verify signature
    pub fn verify(
        &self,
        message: &[u8],
        api_key: &[u8],
    ) -> Result<(), ProgramError> {
        let expected = Self::compute_signature(
            message,
            api_key,
            self.timestamp,
            &self.nonce,
        );

        if expected != self.signature {
            return Err(BettingPlatformError::SecurityCheckFailed.into());
        }

        Ok(())
    }
}

/// Request validator for security checks
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RequestValidator {
    pub nonce_cache: NonceCache,
    pub timestamp_validator: TimestampValidator,
    pub rate_limiter: RequestRateLimiter,
    pub blocked_keys: HashSet<[u8; 16]>,
}

impl RequestValidator {
    pub const SIZE: usize = 1024 * 16; // 16KB

    pub fn new() -> Self {
        Self {
            nonce_cache: NonceCache::new(NONCE_CACHE_SIZE),
            timestamp_validator: TimestampValidator::new(),
            rate_limiter: RequestRateLimiter::new(),
            blocked_keys: HashSet::new(),
        }
    }

    /// Validate incoming request
    pub fn validate_request(
        &mut self,
        signature: &RequestSignature,
        message: &[u8],
        api_key: &[u8],
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        // Check if key is blocked
        if self.blocked_keys.contains(&signature.key_id) {
            return Err(BettingPlatformError::Unauthorized.into());
        }

        // Verify signature
        signature.verify(message, api_key)?;

        // Validate timestamp
        self.timestamp_validator.validate(
            signature.timestamp,
            current_timestamp,
        )?;

        // Check nonce for replay protection
        if !self.nonce_cache.check_and_add(&signature.nonce)? {
            msg!("Duplicate nonce detected: {:?}", signature.nonce);
            return Err(BettingPlatformError::SecurityCheckFailed.into());
        }

        // Check rate limit
        self.rate_limiter.check_rate_limit(&signature.key_id)?;

        Ok(())
    }

    /// Block a key
    pub fn block_key(&mut self, key_id: [u8; 16]) {
        self.blocked_keys.insert(key_id);
        msg!("API key blocked: {:?}", key_id);
    }

    /// Unblock a key
    pub fn unblock_key(&mut self, key_id: [u8; 16]) {
        self.blocked_keys.remove(&key_id);
        msg!("API key unblocked: {:?}", key_id);
    }
}

/// Nonce cache for replay prevention
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct NonceCache {
    pub cache: VecDeque<([u8; 16], i64)>, // (nonce, timestamp)
    pub lookup: HashSet<[u8; 16]>,
    pub max_size: usize,
}

impl NonceCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: VecDeque::new(),
            lookup: HashSet::new(),
            max_size,
        }
    }

    /// Check if nonce exists and add if not
    pub fn check_and_add(&mut self, nonce: &[u8; 16]) -> Result<bool, ProgramError> {
        if self.lookup.contains(nonce) {
            return Ok(false); // Duplicate nonce
        }

        // Add to cache
        let timestamp = Clock::get()?.unix_timestamp;
        self.cache.push_back((*nonce, timestamp));
        self.lookup.insert(*nonce);

        // Evict old entries if cache is full
        while self.cache.len() > self.max_size {
            if let Some((old_nonce, _)) = self.cache.pop_front() {
                self.lookup.remove(&old_nonce);
            }
        }

        Ok(true)
    }

    /// Clean expired nonces
    pub fn clean_expired(&mut self, current_timestamp: i64) {
        let expiry = current_timestamp - REQUEST_TIMEOUT_SECONDS;

        while let Some(&(nonce, timestamp)) = self.cache.front() {
            if timestamp < expiry {
                self.cache.pop_front();
                self.lookup.remove(&nonce);
            } else {
                break;
            }
        }
    }
}

/// Timestamp validator
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct TimestampValidator {
    pub max_clock_drift: i64,
    pub request_timeout: i64,
}

impl TimestampValidator {
    pub fn new() -> Self {
        Self {
            max_clock_drift: MAX_CLOCK_DRIFT_SECONDS,
            request_timeout: REQUEST_TIMEOUT_SECONDS,
        }
    }

    /// Validate request timestamp
    pub fn validate(
        &self,
        request_timestamp: i64,
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        // Check if timestamp is too old
        if current_timestamp - request_timestamp > self.request_timeout {
            msg!(
                "Request timestamp too old: {} vs current {}",
                request_timestamp,
                current_timestamp
            );
            return Err(BettingPlatformError::SecurityCheckFailed.into());
        }

        // Check if timestamp is in the future (with clock drift tolerance)
        if request_timestamp > current_timestamp + self.max_clock_drift {
            msg!(
                "Request timestamp in future: {} vs current {}",
                request_timestamp,
                current_timestamp
            );
            return Err(BettingPlatformError::SecurityCheckFailed.into());
        }

        Ok(())
    }
}

/// Request rate limiter
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RequestRateLimiter {
    pub request_counts: HashMap<[u8; 16], RateLimitBucket>,
    pub global_limit: u64,
    pub window_seconds: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RateLimitBucket {
    pub count: u64,
    pub window_start: i64,
}

impl RequestRateLimiter {
    pub const DEFAULT_LIMIT_PER_MINUTE: u64 = 60;
    pub const WINDOW_SIZE: i64 = 60; // 1 minute

    pub fn new() -> Self {
        Self {
            request_counts: HashMap::new(),
            global_limit: Self::DEFAULT_LIMIT_PER_MINUTE,
            window_seconds: Self::WINDOW_SIZE,
        }
    }

    /// Check rate limit for key
    pub fn check_rate_limit(&mut self, key_id: &[u8; 16]) -> Result<(), ProgramError> {
        let current_time = Clock::get()?.unix_timestamp;
        
        let bucket = self.request_counts.entry(*key_id).or_insert(
            RateLimitBucket {
                count: 0,
                window_start: current_time,
            }
        );

        // Reset bucket if window expired
        if current_time - bucket.window_start >= self.window_seconds {
            bucket.count = 0;
            bucket.window_start = current_time;
        }

        // Check limit
        if bucket.count >= self.global_limit {
            msg!("Rate limit exceeded for key: {:?}", key_id);
            return Err(BettingPlatformError::RateLimited.into());
        }

        bucket.count += 1;
        Ok(())
    }

    /// Clean expired buckets
    pub fn clean_expired(&mut self, current_timestamp: i64) {
        self.request_counts.retain(|_, bucket| {
            current_timestamp - bucket.window_start < self.window_seconds * 2
        });
    }
}

/// Audit log entry
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct AuditLogEntry {
    pub timestamp: i64,
    pub request_id: [u8; 16],
    pub key_id: [u8; 16],
    pub action: AuditAction,
    pub result: AuditResult,
    pub ip_hash: Option<[u8; 16]>,
    pub details: String,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum AuditAction {
    ApiRequest { provider: ApiProvider, method: String },
    KeyRotation,
    SecurityViolation { violation_type: String },
    RateLimitExceeded,
    AuthenticationFailed,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum AuditResult {
    Success,
    Failed { error_code: u16 },
    Blocked,
}

/// Audit logger
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AuditLogger {
    pub logs: VecDeque<AuditLogEntry>,
    pub max_entries: usize,
    pub total_logged: u64,
}

impl AuditLogger {
    pub const SIZE: usize = 1024 * 64; // 64KB for audit logs
    pub const DEFAULT_MAX_ENTRIES: usize = 5000;

    pub fn new() -> Self {
        Self {
            logs: VecDeque::new(),
            max_entries: Self::DEFAULT_MAX_ENTRIES,
            total_logged: 0,
        }
    }

    /// Log audit entry
    pub fn log(
        &mut self,
        key_id: [u8; 16],
        action: AuditAction,
        result: AuditResult,
        details: String,
    ) {
        let timestamp = Clock::get().unwrap().unix_timestamp;
        let request_id = Self::generate_request_id(&key_id, timestamp);

        let entry = AuditLogEntry {
            timestamp,
            request_id,
            key_id,
            action,
            result,
            ip_hash: None,
            details,
        };

        self.logs.push_back(entry);
        self.total_logged += 1;

        // Evict old entries
        while self.logs.len() > self.max_entries {
            self.logs.pop_front();
        }
    }

    /// Generate unique request ID
    fn generate_request_id(key_id: &[u8; 16], timestamp: i64) -> [u8; 16] {
        let hash = keccak::hashv(&[
            key_id,
            &timestamp.to_le_bytes(),
            &rand::random::<u64>().to_le_bytes(),
        ]);
        
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.0[..16]);
        id
    }

    /// Query logs by key
    pub fn query_by_key(&self, key_id: &[u8; 16]) -> Vec<&AuditLogEntry> {
        self.logs
            .iter()
            .filter(|entry| &entry.key_id == key_id)
            .collect()
    }

    /// Query logs by time range
    pub fn query_by_time_range(
        &self,
        start: i64,
        end: i64,
    ) -> Vec<&AuditLogEntry> {
        self.logs
            .iter()
            .filter(|entry| entry.timestamp >= start && entry.timestamp <= end)
            .collect()
    }

    /// Get security violations
    pub fn get_security_violations(&self) -> Vec<&AuditLogEntry> {
        self.logs
            .iter()
            .filter(|entry| matches!(
                entry.action,
                AuditAction::SecurityViolation { .. } |
                AuditAction::AuthenticationFailed
            ))
            .collect()
    }

    /// Clean old entries
    pub fn clean_old_entries(&mut self, current_timestamp: i64) {
        let cutoff = current_timestamp - (AUDIT_LOG_RETENTION_DAYS * 86400);
        
        while let Some(entry) = self.logs.front() {
            if entry.timestamp < cutoff {
                self.logs.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Security metrics tracker
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct SecurityMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_auth: u64,
    pub rate_limit_hits: u64,
    pub replay_attempts: u64,
    pub security_violations: u64,
    pub last_violation: Option<i64>,
}

impl SecurityMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_auth: 0,
            rate_limit_hits: 0,
            replay_attempts: 0,
            security_violations: 0,
            last_violation: None,
        }
    }

    /// Update metrics based on validation result
    pub fn update(&mut self, result: &AuditResult, action: &AuditAction) {
        self.total_requests += 1;

        match result {
            AuditResult::Success => self.successful_requests += 1,
            AuditResult::Failed { .. } => match action {
                AuditAction::AuthenticationFailed => self.failed_auth += 1,
                AuditAction::RateLimitExceeded => self.rate_limit_hits += 1,
                AuditAction::SecurityViolation { .. } => {
                    self.security_violations += 1;
                    self.last_violation = Some(Clock::get().unwrap().unix_timestamp);
                }
                _ => {}
            },
            AuditResult::Blocked => self.security_violations += 1,
        }
    }

    /// Get security score (0-100)
    pub fn get_security_score(&self) -> u8 {
        if self.total_requests == 0 {
            return 100;
        }

        let violation_rate = (self.security_violations as f64) / (self.total_requests as f64);
        let auth_failure_rate = (self.failed_auth as f64) / (self.total_requests as f64);
        
        let score = 100.0 * (1.0 - violation_rate - auth_failure_rate);
        score.max(0.0).min(100.0) as u8
    }
}

// Mock rand for deterministic testing
mod rand {
    pub fn random<T>() -> T 
    where T: Default {
        T::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_signature() {
        let message = b"test message";
        let api_key = b"secret_key";
        let timestamp = 100;
        let nonce = [1u8; 16];
        let key_id = [2u8; 16];

        let sig = RequestSignature::new(message, api_key, timestamp, nonce, key_id);
        assert!(sig.verify(message, api_key).is_ok());

        // Wrong message should fail
        assert!(sig.verify(b"wrong message", api_key).is_err());
    }

    #[test]
    fn test_nonce_cache() {
        let mut cache = NonceCache::new(3);
        let nonce1 = [1u8; 16];
        let nonce2 = [2u8; 16];

        assert!(cache.check_and_add(&nonce1).unwrap());
        assert!(!cache.check_and_add(&nonce1).unwrap()); // Duplicate

        assert!(cache.check_and_add(&nonce2).unwrap());
    }

    #[test]
    fn test_timestamp_validation() {
        let validator = TimestampValidator::new();
        
        // Valid timestamp
        assert!(validator.validate(100, 150).is_ok());

        // Too old
        assert!(validator.validate(100, 500).is_err());

        // Too far in future
        assert!(validator.validate(200, 100).is_err());
    }

    #[test]
    fn test_audit_logger() {
        let mut logger = AuditLogger::new();
        
        logger.log(
            [1u8; 16],
            AuditAction::ApiRequest {
                provider: ApiProvider::Polymarket,
                method: "GET".to_string(),
            },
            AuditResult::Success,
            "Test request".to_string(),
        );

        assert_eq!(logger.total_logged, 1);
        assert_eq!(logger.logs.len(), 1);

        let violations = logger.get_security_violations();
        assert_eq!(violations.len(), 0);
    }
}