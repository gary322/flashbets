//! Rate Limiter
//!
//! Production-grade rate limiting for DOS protection and fair resource usage

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::discriminators,
};

/// Rate limit configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window in slots
    pub window_slots: u64,
    /// Burst capacity (for token bucket)
    pub burst_capacity: u32,
    /// Refill rate per slot
    pub refill_per_slot: u32,
    /// Whether to use sliding window
    pub use_sliding_window: bool,
}

impl RateLimitConfig {
    pub fn new(max_requests: u32, window_slots: u64) -> Self {
        Self {
            max_requests,
            window_slots,
            burst_capacity: max_requests * 2,
            refill_per_slot: max_requests / window_slots.max(1) as u32,
            use_sliding_window: true,
        }
    }
    
    /// Create config for different operation types
    pub fn for_operation(operation: OperationType) -> Self {
        match operation {
            OperationType::Trade => Self::new(100, 10), // 100 trades per 10 slots
            OperationType::Liquidation => Self::new(10, 10), // 10 liquidations per 10 slots
            OperationType::Oracle => Self::new(50, 10), // 50 oracle updates per 10 slots
            OperationType::Withdrawal => Self::new(5, 100), // 5 withdrawals per 100 slots
            OperationType::ConfigUpdate => Self::new(2, 1000), // 2 config updates per 1000 slots
            OperationType::Emergency => Self::new(1, 10000), // 1 emergency per 10000 slots
        }
    }
}

/// Operation types for different rate limits
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Trade,
    Liquidation,
    Oracle,
    Withdrawal,
    ConfigUpdate,
    Emergency,
}

/// Rate limiter using token bucket algorithm
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct TokenBucket {
    /// Current tokens available
    pub tokens: u32,
    /// Maximum capacity
    pub capacity: u32,
    /// Refill rate per slot
    pub refill_rate: u32,
    /// Last refill slot
    pub last_refill_slot: u64,
}

impl TokenBucket {
    pub fn new(capacity: u32, initial_tokens: u32, refill_rate: u32) -> Self {
        Self {
            tokens: initial_tokens,
            capacity,
            refill_rate,
            last_refill_slot: 0,
        }
    }
    
    /// Try to consume tokens
    pub fn try_consume(&mut self, amount: u32) -> Result<(), ProgramError> {
        // Check if enough tokens
        if self.tokens < amount {
            msg!("Rate limit exceeded: {} tokens required, {} available", 
                amount, self.tokens);
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }
        
        // Consume tokens
        self.tokens -= amount;
        Ok(())
    }
    
    /// Try to consume tokens with refill
    pub fn try_consume_with_refill(&mut self, amount: u32, current_slot: u64) -> Result<(), ProgramError> {
        // Refill tokens based on elapsed time
        self.refill(current_slot);
        
        // Check if enough tokens
        if self.tokens < amount {
            msg!("Rate limit exceeded: {} tokens required, {} available", 
                amount, self.tokens);
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }
        
        // Consume tokens
        self.tokens -= amount;
        Ok(())
    }
    
    /// Refill tokens based on elapsed time
    pub fn refill(&mut self, current_slot: u64) {
        if current_slot <= self.last_refill_slot {
            return;
        }
        
        let slots_elapsed = current_slot - self.last_refill_slot;
        let tokens_to_add = (slots_elapsed * self.refill_rate as u64) as u32;
        
        self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
        self.last_refill_slot = current_slot;
    }
    
    /// Get current fill percentage
    pub fn fill_percentage(&self) -> u8 {
        ((self.tokens as u64 * 100) / self.capacity as u64) as u8
    }
}

/// Sliding window rate limiter
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SlidingWindow {
    /// Request timestamps (slots)
    pub requests: Vec<u64>,
    /// Maximum requests
    pub max_requests: u32,
    /// Window size in slots
    pub window_slots: u64,
}

impl SlidingWindow {
    pub fn new(window_slots: u64, max_requests: u32) -> Self {
        Self {
            requests: Vec::with_capacity(max_requests as usize),
            max_requests,
            window_slots,
        }
    }
    
    /// Check and add request
    pub fn check_and_add(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        self.try_request(current_slot)
    }
    
    /// Try to record request
    pub fn try_request(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        // Remove old requests outside window
        let cutoff_slot = current_slot.saturating_sub(self.window_slots);
        self.requests.retain(|&slot| slot > cutoff_slot);
        
        // Check if limit reached
        if self.requests.len() >= self.max_requests as usize {
            msg!("Sliding window rate limit exceeded: {} requests in {} slots",
                self.requests.len(), self.window_slots);
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }
        
        // Record request
        self.requests.push(current_slot);
        Ok(())
    }
    
    /// Get current request count
    pub fn current_count(&self, current_slot: u64) -> usize {
        let cutoff_slot = current_slot.saturating_sub(self.window_slots);
        self.requests.iter().filter(|&&slot| slot > cutoff_slot).count()
    }
}

/// User rate limiter account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UserRateLimiter {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// User pubkey
    pub user: Pubkey,
    /// Token buckets for different operations
    pub trade_bucket: TokenBucket,
    pub liquidation_bucket: TokenBucket,
    pub withdrawal_bucket: TokenBucket,
    /// Sliding windows for burst protection
    pub trade_window: SlidingWindow,
    /// Violation count
    pub violation_count: u32,
    /// Suspension end slot (0 = not suspended)
    pub suspended_until: u64,
    /// Last activity slot
    pub last_activity_slot: u64,
}

impl UserRateLimiter {
    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::RATE_LIMITER,
            user,
            trade_bucket: TokenBucket::new(200, 200, 20), // 200 burst, 200 initial, 20/slot refill
            liquidation_bucket: TokenBucket::new(20, 20, 2), // 20 burst, 20 initial, 2/slot refill
            withdrawal_bucket: TokenBucket::new(10, 10, 1), // 10 burst, 10 initial, 1/slot refill
            trade_window: SlidingWindow::new(10, 100), // 100 trades per 10 slots
            violation_count: 0,
            suspended_until: 0,
            last_activity_slot: 0,
        }
    }
    
    /// Check rate limit for operation
    pub fn check_rate_limit(
        &mut self,
        operation: OperationType,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check if suspended
        if self.suspended_until > 0 && current_slot < self.suspended_until {
            msg!("User suspended until slot {}", self.suspended_until);
            return Err(BettingPlatformError::UserSuspended.into());
        }
        
        // Clear suspension if expired
        if self.suspended_until > 0 && current_slot >= self.suspended_until {
            self.suspended_until = 0;
            self.violation_count = 0;
        }
        
        // Check rate limit based on operation
        let result = match operation {
            OperationType::Trade => {
                self.trade_bucket.try_consume_with_refill(1, current_slot)?;
                self.trade_window.try_request(current_slot)
            }
            OperationType::Liquidation => {
                self.liquidation_bucket.try_consume_with_refill(1, current_slot)
            }
            OperationType::Withdrawal => {
                self.withdrawal_bucket.try_consume_with_refill(1, current_slot)
            }
            _ => Ok(()), // Other operations handled globally
        };
        
        // Handle violations
        if let Err(e) = result {
            self.record_violation(current_slot);
            return Err(e);
        }
        
        self.last_activity_slot = current_slot;
        Ok(())
    }
    
    /// Record rate limit violation
    fn record_violation(&mut self, current_slot: u64) {
        self.violation_count += 1;
        
        // Progressive suspension based on violations
        let suspension_duration = match self.violation_count {
            1..=3 => 10, // 10 slots for first 3 violations
            4..=6 => 100, // 100 slots for next 3
            7..=9 => 1000, // 1000 slots for next 3
            _ => 10000, // 10000 slots for repeat offenders
        };
        
        self.suspended_until = current_slot + suspension_duration;
        msg!("User suspended for {} slots due to {} violations",
            suspension_duration, self.violation_count);
    }
    
    /// Get rate limit status
    pub fn get_status(&self, operation: OperationType) -> RateLimitStatus {
        let (tokens, capacity) = match operation {
            OperationType::Trade => (self.trade_bucket.tokens, self.trade_bucket.capacity),
            OperationType::Liquidation => (self.liquidation_bucket.tokens, self.liquidation_bucket.capacity),
            OperationType::Withdrawal => (self.withdrawal_bucket.tokens, self.withdrawal_bucket.capacity),
            _ => (0, 0),
        };
        
        RateLimitStatus {
            tokens_available: tokens,
            capacity,
            fill_percentage: if capacity > 0 { (tokens * 100 / capacity) as u8 } else { 0 },
            suspended: self.suspended_until > 0,
            suspension_end: self.suspended_until,
            violations: self.violation_count,
        }
    }
}

/// Global rate limiter for protocol-wide limits
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct GlobalRateLimiter {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Total requests in current window
    pub total_requests: u64,
    /// Window start slot
    pub window_start_slot: u64,
    /// Configuration
    pub config: GlobalRateLimitConfig,
    /// Per-operation counters
    pub operation_counts: [u64; 6], // One for each OperationType
    /// Circuit breaker state
    pub circuit_breaker_open: bool,
    /// Circuit breaker cooldown end
    pub circuit_breaker_cooldown: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct GlobalRateLimitConfig {
    /// Maximum total requests per window
    pub max_total_requests: u64,
    /// Window size in slots
    pub window_slots: u64,
    /// Circuit breaker threshold (requests per slot)
    pub circuit_breaker_threshold: u64,
    /// Circuit breaker cooldown slots
    pub circuit_breaker_cooldown: u64,
}

impl GlobalRateLimiter {
    pub fn new(config: GlobalRateLimitConfig) -> Self {
        Self {
            discriminator: discriminators::GLOBAL_RATE_LIMITER,
            total_requests: 0,
            window_start_slot: 0,
            config,
            operation_counts: [0; 6],
            circuit_breaker_open: false,
            circuit_breaker_cooldown: 0,
        }
    }
    
    /// Check global rate limit
    pub fn check_rate_limit(
        &mut self,
        operation: OperationType,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check circuit breaker
        if self.circuit_breaker_open {
            if current_slot < self.circuit_breaker_cooldown {
                msg!("Circuit breaker open until slot {}", self.circuit_breaker_cooldown);
                return Err(BettingPlatformError::CircuitBreakerOpen.into());
            }
            // Reset circuit breaker
            self.circuit_breaker_open = false;
            self.circuit_breaker_cooldown = 0;
        }
        
        // Reset window if needed
        if current_slot >= self.window_start_slot + self.config.window_slots {
            self.total_requests = 0;
            self.window_start_slot = current_slot;
            self.operation_counts = [0; 6];
        }
        
        // Check total limit
        if self.total_requests >= self.config.max_total_requests {
            msg!("Global rate limit exceeded: {} requests", self.total_requests);
            self.trip_circuit_breaker(current_slot);
            return Err(BettingPlatformError::GlobalRateLimitExceeded.into());
        }
        
        // Check requests per slot (burst protection)
        let requests_per_slot = self.total_requests / current_slot.saturating_sub(self.window_start_slot).max(1);
        if requests_per_slot > self.config.circuit_breaker_threshold {
            msg!("Circuit breaker triggered: {} requests/slot", requests_per_slot);
            self.trip_circuit_breaker(current_slot);
            return Err(BettingPlatformError::CircuitBreakerOpen.into());
        }
        
        // Increment counters
        self.total_requests += 1;
        self.operation_counts[operation as usize] += 1;
        
        Ok(())
    }
    
    /// Trip circuit breaker
    fn trip_circuit_breaker(&mut self, current_slot: u64) {
        self.circuit_breaker_open = true;
        self.circuit_breaker_cooldown = current_slot + self.config.circuit_breaker_cooldown;
        msg!("Circuit breaker tripped, cooldown until slot {}", self.circuit_breaker_cooldown);
    }
    
    /// Get current statistics
    pub fn get_stats(&self) -> GlobalRateLimitStats {
        GlobalRateLimitStats {
            total_requests: self.total_requests,
            window_start: self.window_start_slot,
            operation_counts: self.operation_counts,
            circuit_breaker_open: self.circuit_breaker_open,
            utilization_percentage: (self.total_requests * 100 / self.config.max_total_requests.max(1)) as u8,
        }
    }
}

/// Rate limit status
#[derive(Debug)]
pub struct RateLimitStatus {
    pub tokens_available: u32,
    pub capacity: u32,
    pub fill_percentage: u8,
    pub suspended: bool,
    pub suspension_end: u64,
    pub violations: u32,
}

/// Global rate limit statistics
#[derive(Debug)]
pub struct GlobalRateLimitStats {
    pub total_requests: u64,
    pub window_start: u64,
    pub operation_counts: [u64; 6],
    pub circuit_breaker_open: bool,
    pub utilization_percentage: u8,
}

/// Rate limiter combining multiple strategies (for testing)
#[derive(Debug)]
pub struct RateLimiter {
    /// Token bucket for burst control
    pub token_bucket: TokenBucket,
    /// Sliding window for rate limiting
    pub sliding_window: SlidingWindow,
    /// Last activity by user
    pub user_activity: Vec<(Pubkey, u64)>,
}

impl RateLimiter {
    pub fn new(bucket_capacity: u32, window_size: u64, max_requests: u32) -> Self {
        Self {
            token_bucket: TokenBucket::new(bucket_capacity, bucket_capacity, 1),
            sliding_window: SlidingWindow::new(window_size, max_requests),
            user_activity: Vec::new(),
        }
    }
    
    /// Check if user can perform action
    pub fn check_limit(&mut self, user: &Pubkey, current_slot: u64) -> Result<(), ProgramError> {
        // Check token bucket
        self.token_bucket.refill(current_slot);
        self.token_bucket.try_consume(1)?;
        
        // Check sliding window
        self.sliding_window.check_and_add(current_slot)?;
        
        // Update user activity
        if let Some((_, last_slot)) = self.user_activity.iter_mut().find(|(u, _)| u == user) {
            *last_slot = current_slot;
        } else {
            self.user_activity.push((*user, current_slot));
        }
        
        Ok(())
    }
}

/// Apply rate limiting to an operation
pub fn apply_rate_limit(
    user_limiter: &mut UserRateLimiter,
    global_limiter: &mut GlobalRateLimiter,
    operation: OperationType,
    current_slot: u64,
) -> Result<(), ProgramError> {
    // Check global limit first (cheaper)
    global_limiter.check_rate_limit(operation, current_slot)?;
    
    // Then check user limit
    user_limiter.check_rate_limit(operation, current_slot)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(10, 10, 2);
        
        // Initial capacity
        assert_eq!(bucket.tokens, 10);
        
        // Consume tokens
        assert!(bucket.try_consume(5).is_ok());
        assert_eq!(bucket.tokens, 5);
        
        // Refill after 5 slots
        bucket.refill(105);
        assert_eq!(bucket.tokens, 10); // 5 + (5 * 2) = 15, capped at 10
        
        // Cannot exceed capacity
        assert!(bucket.try_consume(11).is_err());
    }

    #[test]
    fn test_sliding_window() {
        let mut window = SlidingWindow::new(10, 3);
        
        // Add requests
        assert!(window.try_request(100).is_ok());
        assert!(window.try_request(102).is_ok());
        assert!(window.try_request(105).is_ok());
        
        // Hit limit
        assert!(window.try_request(107).is_err());
        
        // Old request expires
        assert!(window.try_request(111).is_ok()); // Request at 100 is now outside window
        assert_eq!(window.current_count(111), 3);
    }

    #[test]
    fn test_user_rate_limiter() {
        let user = Pubkey::new_unique();
        let mut limiter = UserRateLimiter::new(user);
        
        // Normal operations
        assert!(limiter.check_rate_limit(OperationType::Trade, 100).is_ok());
        assert!(limiter.check_rate_limit(OperationType::Trade, 101).is_ok());
        
        // Exhaust tokens
        for _ in 0..200 {
            let _ = limiter.check_rate_limit(OperationType::Trade, 102);
        }
        
        // Should be rate limited
        assert!(limiter.check_rate_limit(OperationType::Trade, 103).is_err());
        assert!(limiter.violation_count > 0);
    }

    #[test]
    fn test_circuit_breaker() {
        let config = GlobalRateLimitConfig {
            max_total_requests: 1000,
            window_slots: 100,
            circuit_breaker_threshold: 5,
            circuit_breaker_cooldown: 50,
        };
        let mut global = GlobalRateLimiter::new(config);
        
        // Normal operation
        assert!(global.check_rate_limit(OperationType::Trade, 100).is_ok());
        
        // Trigger circuit breaker with burst
        global.total_requests = 600; // 6 per slot at slot 100
        assert!(global.check_rate_limit(OperationType::Trade, 100).is_err());
        assert!(global.circuit_breaker_open);
        
        // Still blocked during cooldown
        assert!(global.check_rate_limit(OperationType::Trade, 120).is_err());
        
        // Recovered after cooldown
        assert!(global.check_rate_limit(OperationType::Trade, 151).is_ok());
        assert!(!global.circuit_breaker_open);
    }
}