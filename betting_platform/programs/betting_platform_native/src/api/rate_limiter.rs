//! Rate Limiter Implementation
//!
//! Token bucket algorithm for 100 req/s rate limiting per specification

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use solana_program::pubkey::Pubkey;
use serde::{Deserialize, Serialize};

/// Rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Maximum requests per second
    pub requests_per_second: u32,
    /// Burst capacity (max tokens)
    pub burst_capacity: u32,
    /// Refill interval
    pub refill_interval: Duration,
    /// Enable per-user limits
    pub per_user_limits: bool,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100, // 100 req/s as per specification
            burst_capacity: 200,       // Allow 2x burst
            refill_interval: Duration::from_millis(10), // Refill every 10ms
            per_user_limits: true,
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    /// Current token count
    tokens: f64,
    /// Maximum tokens
    capacity: f64,
    /// Tokens per refill
    refill_rate: f64,
    /// Last refill time
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume tokens
    pub fn try_consume(&mut self, tokens: f64) -> Result<(), RateLimitError> {
        self.refill();
        
        if self.tokens >= tokens {
            self.tokens -= tokens;
            Ok(())
        } else {
            let wait_time = ((tokens - self.tokens) / self.refill_rate * 1000.0) as u64;
            Err(RateLimitError::RateLimitExceeded { 
                retry_after_ms: wait_time 
            })
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        
        if elapsed.as_millis() > 0 {
            let tokens_to_add = self.refill_rate * elapsed.as_secs_f64();
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// Get available tokens
    pub fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }

    /// Get time until next token available
    pub fn time_until_available(&self) -> Duration {
        if self.tokens >= 1.0 {
            Duration::from_secs(0)
        } else {
            let needed = 1.0 - self.tokens;
            Duration::from_secs_f64(needed / self.refill_rate)
        }
    }
}

/// Rate limiter with per-user buckets
pub struct RateLimiter {
    /// Global rate limiter
    global_bucket: Arc<Mutex<TokenBucket>>,
    /// Per-user buckets
    user_buckets: Arc<Mutex<HashMap<Pubkey, TokenBucket>>>,
    /// Configuration
    config: RateLimiterConfig,
    /// Metrics
    metrics: Arc<Mutex<RateLimiterMetrics>>,
}

impl RateLimiter {
    /// Create new rate limiter with default config (100 req/s)
    pub fn new() -> Self {
        Self::with_config(RateLimiterConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: RateLimiterConfig) -> Self {
        let refill_rate = config.requests_per_second as f64 * 
                         config.refill_interval.as_secs_f64();
        
        let global_bucket = TokenBucket::new(config.burst_capacity, refill_rate);
        
        Self {
            global_bucket: Arc::new(Mutex::new(global_bucket)),
            user_buckets: Arc::new(Mutex::new(HashMap::new())),
            config,
            metrics: Arc::new(Mutex::new(RateLimiterMetrics::default())),
        }
    }

    /// Check if request is allowed
    pub fn check_request(&self, user: Option<&Pubkey>) -> Result<(), RateLimitError> {
        // Check global limit first
        {
            let mut global = self.global_bucket.lock().unwrap();
            global.try_consume(1.0)?;
        }

        // Check per-user limit if enabled
        if self.config.per_user_limits {
            if let Some(user_key) = user {
                let mut buckets = self.user_buckets.lock().unwrap();
                
                let refill_rate = (self.config.requests_per_second / 10) as f64 * 
                                 self.config.refill_interval.as_secs_f64();
                
                let bucket = buckets.entry(*user_key)
                    .or_insert_with(|| TokenBucket::new(
                        self.config.burst_capacity / 10, // 10 req/s per user
                        refill_rate
                    ));
                
                bucket.try_consume(1.0)?;
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.total_requests += 1;
            metrics.allowed_requests += 1;
        }

        Ok(())
    }

    /// Check multiple requests at once
    pub fn check_batch(&self, user: Option<&Pubkey>, count: u32) -> Result<(), RateLimitError> {
        // Check global limit
        {
            let mut global = self.global_bucket.lock().unwrap();
            global.try_consume(count as f64)?;
        }

        // Check per-user limit
        if self.config.per_user_limits {
            if let Some(user_key) = user {
                let mut buckets = self.user_buckets.lock().unwrap();
                
                let refill_rate = (self.config.requests_per_second / 10) as f64 * 
                                 self.config.refill_interval.as_secs_f64();
                
                let bucket = buckets.entry(*user_key)
                    .or_insert_with(|| TokenBucket::new(
                        self.config.burst_capacity / 10,
                        refill_rate
                    ));
                
                bucket.try_consume(count as f64)?;
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.total_requests += count as u64;
            metrics.allowed_requests += count as u64;
        }

        Ok(())
    }

    /// Get current status
    pub fn get_status(&self, user: Option<&Pubkey>) -> RateLimiterStatus {
        let global_tokens = {
            let mut global = self.global_bucket.lock().unwrap();
            global.available_tokens()
        };

        let user_tokens = if let Some(user_key) = user {
            let mut buckets = self.user_buckets.lock().unwrap();
            buckets.get_mut(user_key)
                .map(|b| b.available_tokens())
                .unwrap_or(10.0) // Default user capacity
        } else {
            0.0
        };

        let metrics = self.metrics.lock().unwrap().clone();

        RateLimiterStatus {
            global_available: global_tokens as u32,
            user_available: user_tokens as u32,
            requests_per_second: self.config.requests_per_second,
            metrics,
        }
    }

    /// Reset rate limiter
    pub fn reset(&self) {
        {
            let mut global = self.global_bucket.lock().unwrap();
            global.tokens = global.capacity;
            global.last_refill = Instant::now();
        }

        {
            let mut buckets = self.user_buckets.lock().unwrap();
            buckets.clear();
        }

        {
            let mut metrics = self.metrics.lock().unwrap();
            *metrics = RateLimiterMetrics::default();
        }
    }

    /// Clean up old user buckets
    pub fn cleanup_old_buckets(&self, max_age: Duration) {
        let mut buckets = self.user_buckets.lock().unwrap();
        let now = Instant::now();
        
        buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) < max_age
        });
    }
}

/// Rate limiter status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterStatus {
    pub global_available: u32,
    pub user_available: u32,
    pub requests_per_second: u32,
    pub metrics: RateLimiterMetrics,
}

/// Rate limiter metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimiterMetrics {
    pub total_requests: u64,
    pub allowed_requests: u64,
    pub rejected_requests: u64,
    pub last_reset: Option<u64>,
}

/// Rate limit errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitError {
    RateLimitExceeded { retry_after_ms: u64 },
    InternalError(String),
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::RateLimitExceeded { retry_after_ms } => {
                write!(f, "Rate limit exceeded. Retry after {} ms", retry_after_ms)
            }
            RateLimitError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for RateLimitError {}

/// Rate limit middleware for API endpoints
pub struct RateLimitMiddleware {
    limiter: Arc<RateLimiter>,
}

impl RateLimitMiddleware {
    pub fn new(limiter: Arc<RateLimiter>) -> Self {
        Self { limiter }
    }

    /// Check rate limit for request
    pub fn check(&self, user: Option<&Pubkey>) -> Result<(), RateLimitError> {
        self.limiter.check_request(user)
    }

    /// Get rate limit headers
    pub fn get_headers(&self, user: Option<&Pubkey>) -> RateLimitHeaders {
        let status = self.limiter.get_status(user);
        
        RateLimitHeaders {
            x_rate_limit: status.requests_per_second,
            x_rate_limit_remaining: status.global_available.min(status.user_available),
            x_rate_limit_reset: (Instant::now() + Duration::from_secs(1)).elapsed().as_secs(),
        }
    }
}

/// Rate limit headers
#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitHeaders {
    #[serde(rename = "X-RateLimit-Limit")]
    pub x_rate_limit: u32,
    #[serde(rename = "X-RateLimit-Remaining")]
    pub x_rate_limit_remaining: u32,
    #[serde(rename = "X-RateLimit-Reset")]
    pub x_rate_limit_reset: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(10, 1.0);
        
        // Should allow 10 requests initially
        for _ in 0..10 {
            assert!(bucket.try_consume(1.0).is_ok());
        }
        
        // 11th request should fail
        assert!(bucket.try_consume(1.0).is_err());
        
        // Wait for refill
        thread::sleep(Duration::from_millis(1100));
        
        // Should have ~1 token now
        assert!(bucket.try_consume(1.0).is_ok());
    }

    #[test]
    fn test_rate_limiter_100_rps() {
        let limiter = RateLimiter::new(); // 100 req/s by default
        let user = Pubkey::new_unique();
        
        // Should allow burst
        for _ in 0..100 {
            assert!(limiter.check_request(Some(&user)).is_ok());
        }
        
        // Check status
        let status = limiter.get_status(Some(&user));
        assert_eq!(status.requests_per_second, 100);
    }

    #[test]
    fn test_per_user_limits() {
        let limiter = RateLimiter::new();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();
        
        // User 1 uses their quota
        for _ in 0..10 {
            assert!(limiter.check_request(Some(&user1)).is_ok());
        }
        
        // User 2 should still have quota
        assert!(limiter.check_request(Some(&user2)).is_ok());
    }

    #[test]
    fn test_batch_requests() {
        let limiter = RateLimiter::new();
        
        // Should allow batch of 50
        assert!(limiter.check_batch(None, 50).is_ok());
        
        // Should allow another 50 (within burst capacity)
        assert!(limiter.check_batch(None, 50).is_ok());
        
        // Should fail for large batch exceeding capacity
        assert!(limiter.check_batch(None, 200).is_err());
    }
}