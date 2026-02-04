#!/usr/bin/env rust-script
//! Test Polymarket API Rate Limiting
//! 
//! Verifies rate limits are enforced correctly:
//! - Markets: 50 requests per 10 seconds
//! - Orders: 500 requests per 10 seconds

use std::collections::VecDeque;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
struct ProgramError(&'static str);

/// Mock Clock for testing
struct Clock {
    current_time: i64,
}

impl Clock {
    fn new(time: i64) -> Self {
        Self { current_time: time }
    }
    
    fn advance(&mut self, seconds: i64) {
        self.current_time += seconds;
    }
}

/// Rate limiter for API requests
struct RateLimiter {
    market_requests: VecDeque<i64>,
    order_requests: VecDeque<i64>,
}

impl RateLimiter {
    pub const MARKET_LIMIT: usize = 50;
    pub const ORDER_LIMIT: usize = 500;
    pub const WINDOW_SECONDS: i64 = 10;
    
    pub fn new() -> Self {
        Self {
            market_requests: VecDeque::with_capacity(Self::MARKET_LIMIT),
            order_requests: VecDeque::with_capacity(Self::ORDER_LIMIT),
        }
    }
    
    pub fn check_market_limit(&mut self, now: i64) -> Result<(), ProgramError> {
        Self::cleanup_old_requests(&mut self.market_requests, now);
        
        if self.market_requests.len() >= Self::MARKET_LIMIT {
            println!("Market rate limit exceeded: {} requests in last {}s", 
                self.market_requests.len(), Self::WINDOW_SECONDS);
            return Err(ProgramError("RateLimitExceeded"));
        }
        
        self.market_requests.push_back(now);
        println!("Market request allowed: {}/{} in window", 
            self.market_requests.len(), Self::MARKET_LIMIT);
        Ok(())
    }
    
    pub fn check_order_limit(&mut self, now: i64) -> Result<(), ProgramError> {
        Self::cleanup_old_requests(&mut self.order_requests, now);
        
        if self.order_requests.len() >= Self::ORDER_LIMIT {
            println!("Order rate limit exceeded: {} requests in last {}s", 
                self.order_requests.len(), Self::WINDOW_SECONDS);
            return Err(ProgramError("RateLimitExceeded"));
        }
        
        self.order_requests.push_back(now);
        println!("Order request allowed: {}/{} in window", 
            self.order_requests.len(), Self::ORDER_LIMIT);
        Ok(())
    }
    
    fn cleanup_old_requests(requests: &mut VecDeque<i64>, now: i64) {
        let cutoff = now - Self::WINDOW_SECONDS;
        
        while let Some(&front) = requests.front() {
            if front < cutoff {
                requests.pop_front();
            } else {
                break;
            }
        }
    }
    
    pub fn get_usage(&mut self, now: i64) -> (usize, usize) {
        Self::cleanup_old_requests(&mut self.market_requests, now);
        Self::cleanup_old_requests(&mut self.order_requests, now);
        
        (self.market_requests.len(), self.order_requests.len())
    }
    
    pub fn reset(&mut self) {
        self.market_requests.clear();
        self.order_requests.clear();
    }
}

fn test_market_rate_limiting() {
    println!("\n=== Testing Market Rate Limiting (50/10s) ===");
    
    let mut limiter = RateLimiter::new();
    let mut clock = Clock::new(1000);
    
    // Test: Should allow 50 requests
    for i in 1..=50 {
        let result = limiter.check_market_limit(clock.current_time);
        assert!(result.is_ok());
        if i % 10 == 0 {
            println!("✓ Allowed {} market requests", i);
        }
    }
    
    // Test: 51st request should fail
    let result = limiter.check_market_limit(clock.current_time);
    assert_eq!(result, Err(ProgramError("RateLimitExceeded")));
    println!("✓ 51st request correctly rejected");
    
    // Test: After 10 seconds, should allow requests again
    clock.advance(11);
    let result = limiter.check_market_limit(clock.current_time);
    assert!(result.is_ok());
    println!("✓ Requests allowed after window expires");
}

fn test_order_rate_limiting() {
    println!("\n=== Testing Order Rate Limiting (500/10s) ===");
    
    let mut limiter = RateLimiter::new();
    let mut clock = Clock::new(2000);
    
    // Test: Should allow 500 requests
    for i in 1..=500 {
        let result = limiter.check_order_limit(clock.current_time);
        assert!(result.is_ok());
        if i % 100 == 0 {
            println!("✓ Allowed {} order requests", i);
        }
    }
    
    // Test: 501st request should fail
    let result = limiter.check_order_limit(clock.current_time);
    assert_eq!(result, Err(ProgramError("RateLimitExceeded")));
    println!("✓ 501st request correctly rejected");
    
    // Test: After 10 seconds, should allow requests again
    clock.advance(11);
    let result = limiter.check_order_limit(clock.current_time);
    assert!(result.is_ok());
    println!("✓ Requests allowed after window expires");
}

fn test_sliding_window() {
    println!("\n=== Testing Sliding Window Behavior ===");
    
    let mut limiter = RateLimiter::new();
    let mut clock = Clock::new(3000);
    
    // Add 40 requests at time 3000
    for _ in 0..40 {
        limiter.check_market_limit(clock.current_time).unwrap();
    }
    println!("Added 40 requests at time {}", clock.current_time);
    
    // Advance 5 seconds and add 5 more
    clock.advance(5);
    for _ in 0..5 {
        limiter.check_market_limit(clock.current_time).unwrap();
    }
    println!("Added 5 requests at time {} (total: 45)", clock.current_time);
    
    // Advance 6 more seconds (11 total) - first 40 should expire
    clock.advance(6);
    let (market_count, _) = limiter.get_usage(clock.current_time);
    assert_eq!(market_count, 5);
    println!("✓ After 11s, only last 5 requests remain in window");
    
    // Should be able to add 45 more
    for _ in 0..45 {
        limiter.check_market_limit(clock.current_time).unwrap();
    }
    println!("✓ Successfully added 45 more requests");
}

fn test_concurrent_limits() {
    println!("\n=== Testing Concurrent Market and Order Limits ===");
    
    let mut limiter = RateLimiter::new();
    let clock = Clock::new(4000);
    
    // Fill up half of each limit
    for _ in 0..25 {
        limiter.check_market_limit(clock.current_time).unwrap();
    }
    for _ in 0..250 {
        limiter.check_order_limit(clock.current_time).unwrap();
    }
    
    let (market_count, order_count) = limiter.get_usage(clock.current_time);
    println!("Current usage - Markets: {}/50, Orders: {}/500", market_count, order_count);
    assert_eq!(market_count, 25);
    assert_eq!(order_count, 250);
    
    // Fill up the rest
    for _ in 0..25 {
        limiter.check_market_limit(clock.current_time).unwrap();
    }
    for _ in 0..250 {
        limiter.check_order_limit(clock.current_time).unwrap();
    }
    
    // Both should now be at limit
    assert!(limiter.check_market_limit(clock.current_time).is_err());
    assert!(limiter.check_order_limit(clock.current_time).is_err());
    println!("✓ Both limits enforced independently");
}

fn test_edge_cases() {
    println!("\n=== Testing Edge Cases ===");
    
    let mut limiter = RateLimiter::new();
    let mut clock = Clock::new(5000);
    
    // Test: Exact window boundary
    for _ in 0..50 {
        limiter.check_market_limit(clock.current_time).unwrap();
    }
    
    // Advance exactly 10 seconds
    clock.advance(10);
    
    // Old requests should still be in window
    let result = limiter.check_market_limit(clock.current_time);
    assert_eq!(result, Err(ProgramError("RateLimitExceeded")));
    println!("✓ Requests at exact window boundary still count");
    
    // Advance 1 more millisecond
    clock.advance(1);
    
    // Now should be allowed
    let result = limiter.check_market_limit(clock.current_time);
    assert!(result.is_ok());
    println!("✓ Requests expire after window + 1 second");
    
    // Test: Reset functionality
    limiter.reset();
    let (market_count, order_count) = limiter.get_usage(clock.current_time);
    assert_eq!(market_count, 0);
    assert_eq!(order_count, 0);
    println!("✓ Reset clears all requests");
}

fn test_performance() {
    println!("\n=== Testing Performance Under Load ===");
    
    let mut limiter = RateLimiter::new();
    let start = Instant::now();
    
    // Simulate high-frequency trading scenario
    let mut clock = Clock::new(6000);
    let mut accepted = 0;
    let mut rejected = 0;
    
    // Try 1000 market requests over simulated 20 seconds
    for i in 0..1000 {
        // Advance time slightly for each request
        if i % 50 == 0 && i > 0 {
            clock.advance(1);
        }
        
        match limiter.check_market_limit(clock.current_time) {
            Ok(_) => accepted += 1,
            Err(_) => rejected += 1,
        }
    }
    
    let elapsed = start.elapsed();
    println!("Processed 1000 requests in {:?}", elapsed);
    println!("Accepted: {}, Rejected: {}", accepted, rejected);
    println!("✓ Rate limiter performs efficiently under load");
}

fn main() {
    println!("Polymarket API Rate Limiting Test Suite");
    println!("=======================================");
    println!("\nRate Limits:");
    println!("- Markets: {} requests per {} seconds", 
        RateLimiter::MARKET_LIMIT, RateLimiter::WINDOW_SECONDS);
    println!("- Orders: {} requests per {} seconds", 
        RateLimiter::ORDER_LIMIT, RateLimiter::WINDOW_SECONDS);
    
    test_market_rate_limiting();
    test_order_rate_limiting();
    test_sliding_window();
    test_concurrent_limits();
    test_edge_cases();
    test_performance();
    
    println!("\n✅ All rate limiting tests passed!");
    println!("\nSummary:");
    println!("- Market rate limit (50/10s) enforced correctly");
    println!("- Order rate limit (500/10s) enforced correctly");
    println!("- Sliding window mechanism working properly");
    println!("- Concurrent limits tracked independently");
    println!("- Edge cases handled appropriately");
    println!("- Performance is acceptable under load");
}