//! Polymarket API Rate Limiting Implementation
//! 
//! Enforces rate limits per specification:
//! - Markets: 50 requests per 10 seconds
//! - Orders: 500 requests per 10 seconds

use solana_program::{
    clock::Clock,
    program_error::ProgramError,
    sysvar::Sysvar,
    msg,
};
use std::collections::VecDeque;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::error::BettingPlatformError;

/// Rate limiter for API requests
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RateLimiter {
    /// Market request timestamps
    market_requests: VecDeque<i64>,
    /// Order request timestamps
    order_requests: VecDeque<i64>,
}

impl RateLimiter {
    /// Market request limit per window
    pub const MARKET_LIMIT: usize = 50;
    
    /// Order request limit per window
    pub const ORDER_LIMIT: usize = 500;
    
    /// Time window in seconds
    pub const WINDOW_SECONDS: i64 = 10;
    
    /// Create new rate limiter
    pub fn new() -> Self {
        Self {
            market_requests: VecDeque::with_capacity(Self::MARKET_LIMIT),
            order_requests: VecDeque::with_capacity(Self::ORDER_LIMIT),
        }
    }
    
    /// Check if market request is allowed
    pub fn check_market_limit(&mut self) -> Result<(), ProgramError> {
        let now = Clock::get()?.unix_timestamp;
        Self::cleanup_old_requests(&mut self.market_requests, now);
        
        if self.market_requests.len() >= Self::MARKET_LIMIT {
            msg!("Market rate limit exceeded: {} requests in last {}s", 
                self.market_requests.len(), Self::WINDOW_SECONDS);
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }
        
        self.market_requests.push_back(now);
        msg!("Market request allowed: {}/{} in window", 
            self.market_requests.len(), Self::MARKET_LIMIT);
        Ok(())
    }
    
    /// Check if order request is allowed
    pub fn check_order_limit(&mut self) -> Result<(), ProgramError> {
        let now = Clock::get()?.unix_timestamp;
        Self::cleanup_old_requests(&mut self.order_requests, now);
        
        if self.order_requests.len() >= Self::ORDER_LIMIT {
            msg!("Order rate limit exceeded: {} requests in last {}s", 
                self.order_requests.len(), Self::WINDOW_SECONDS);
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }
        
        self.order_requests.push_back(now);
        msg!("Order request allowed: {}/{} in window", 
            self.order_requests.len(), Self::ORDER_LIMIT);
        Ok(())
    }
    
    /// Clean up requests older than the window
    fn cleanup_old_requests(requests: &mut VecDeque<i64>, now: i64) {
        let cutoff = now - Self::WINDOW_SECONDS;
        
        // Remove all requests older than the window
        while let Some(&front) = requests.front() {
            if front < cutoff {
                requests.pop_front();
            } else {
                break;
            }
        }
    }
    
    /// Get current usage stats
    pub fn get_usage(&mut self) -> (usize, usize) {
        let now = Clock::get().unwrap().unix_timestamp;
        Self::cleanup_old_requests(&mut self.market_requests, now);
        Self::cleanup_old_requests(&mut self.order_requests, now);
        
        (self.market_requests.len(), self.order_requests.len())
    }
    
    /// Get current requests count
    pub fn get_current_requests(&self) -> usize {
        self.market_requests.len() + self.order_requests.len()
    }
    
    /// Get requests per window limit
    pub fn get_requests_per_window(&self) -> usize {
        Self::MARKET_LIMIT + Self::ORDER_LIMIT
    }
    
    /// Reset rate limiter
    pub fn reset(&mut self) {
        self.market_requests.clear();
        self.order_requests.clear();
    }
}

/// Global rate limiter state (would be stored in PDA in production)
pub struct RateLimiterState {
    pub authority: solana_program::pubkey::Pubkey,
    pub market_request_count: u32,
    pub order_request_count: u32,
    pub window_start: i64,
    pub total_requests: u64,
    pub total_rejections: u64,
}

impl RateLimiterState {
    pub const SIZE: usize = 32 + 4 + 4 + 8 + 8 + 8;
    
    /// Check and update rate limit
    pub fn check_and_update(
        &mut self,
        is_market_request: bool,
    ) -> Result<(), ProgramError> {
        let now = Clock::get()?.unix_timestamp;
        
        // Reset window if expired
        if now - self.window_start >= RateLimiter::WINDOW_SECONDS {
            self.window_start = now;
            self.market_request_count = 0;
            self.order_request_count = 0;
        }
        
        // Check appropriate limit
        if is_market_request {
            if self.market_request_count >= RateLimiter::MARKET_LIMIT as u32 {
                self.total_rejections += 1;
                return Err(BettingPlatformError::RateLimitExceeded.into());
            }
            self.market_request_count += 1;
        } else {
            if self.order_request_count >= RateLimiter::ORDER_LIMIT as u32 {
                self.total_rejections += 1;
                return Err(BettingPlatformError::RateLimitExceeded.into());
            }
            self.order_request_count += 1;
        }
        
        self.total_requests += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new();
        
        // Should allow up to limit
        for _ in 0..RateLimiter::MARKET_LIMIT {
            assert!(limiter.check_market_limit().is_ok());
        }
        
        // Should reject after limit
        assert!(limiter.check_market_limit().is_err());
    }
}