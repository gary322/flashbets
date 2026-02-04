//! Rate Limit Edge Case Testing
//! 
//! Tests behavior when Polymarket API rate limits are exhausted

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    integration::rate_limiter::RateLimiter,
    events::{emit_event, EventType},
    integration::events::ComponentHealthUpdateEvent,
};

/// Polymarket rate limits from Part 7
const MARKET_RATE_LIMIT: u32 = 50;
const MARKET_WINDOW_SECONDS: i64 = 10;
const ORDER_RATE_LIMIT: u32 = 500;
const ORDER_WINDOW_SECONDS: i64 = 10;

/// Test market data rate limit exhaustion
pub fn test_market_rate_limit_exhaustion(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let rate_limiter_account = next_account_info(account_iter)?;
    let oracle_cache_account = next_account_info(account_iter)?;
    
    msg!("Testing market data rate limit exhaustion");
    
    // Load rate limiter
    let mut rate_limiter = RateLimiter::deserialize(&mut &rate_limiter_account.data.borrow()[..])?;
    
    // RateLimiter is already configured with internal limits
    
    // Step 1: Simulate rapid market data requests
    msg!("Step 1: Simulating {} rapid market requests", MARKET_RATE_LIMIT + 10);
    
    let mut successful_requests = 0;
    let mut rate_limited_requests = 0;
    
    for i in 0..MARKET_RATE_LIMIT + 10 {
        match rate_limiter.check_market_limit() {
            Ok(_) => {
                successful_requests += 1;
                // Request count is tracked internally
                msg!("Request {} succeeded", i);
            }
            Err(e) => {
                rate_limited_requests += 1;
                msg!("Request {} rate limited", i);
                
                if i < MARKET_RATE_LIMIT {
                    msg!("ERROR: Rate limited before reaching limit!");
                    return Err(e);
                }
            }
        }
    }
    
    msg!("Successful requests: {}", successful_requests);
    msg!("Rate limited requests: {}", rate_limited_requests);
    
    // Verify we hit the limit
    assert_eq!(successful_requests, MARKET_RATE_LIMIT);
    assert_eq!(rate_limited_requests, 10);
    
    // Step 2: Test cache fallback
    msg!("Step 2: Testing cache fallback during rate limit");
    
    // Load oracle cache
    let cache = OracleCache::try_from_slice(&oracle_cache_account.data.borrow())?;
    
    if cache.is_valid() {
        msg!("✓ Cache is valid - using cached prices");
        msg!("  Cached price: {}", cache.last_price);
        msg!("  Cache age: {} seconds", Clock::get()?.unix_timestamp - cache.timestamp);
        
        // Verify cache is recent enough (< 60 seconds)
        if Clock::get()?.unix_timestamp - cache.timestamp > 60 {
            msg!("WARNING: Cache is stale!");
        }
    } else {
        msg!("✗ No valid cache available!");
        return Err(BettingPlatformError::OracleCacheMiss.into());
    }
    
    // Step 3: Test request batching
    msg!("Step 3: Testing request batching optimization");
    
    let batch_size = 10;
    let batched_requests = simulate_batched_requests(&mut rate_limiter, batch_size)?;
    
    msg!("Batched {} requests into {} API calls", batch_size, batched_requests);
    let efficiency = (batch_size as f64 / batched_requests as f64) * 100.0;
    msg!("Batching efficiency: {:.1}%", efficiency);
    
    // Step 4: Test window reset
    msg!("Step 4: Testing rate limit window reset");
    
    // Simulate time passing
    let new_timestamp = Clock::get()?.unix_timestamp + MARKET_WINDOW_SECONDS + 1;
    
    // Rate limiter handles window expiry internally
    msg!("Simulating window expiry");
    rate_limiter.reset();
    
    // Verify we can make requests again
    match rate_limiter.check_market_limit() {
        Ok(_) => msg!("✓ Rate limit reset successful"),
        Err(_) => {
            msg!("✗ ERROR: Still rate limited after window reset!");
            return Err(BettingPlatformError::RateLimitError.into());
        }
    }
    
    // Save rate limiter state
    rate_limiter.serialize(&mut &mut rate_limiter_account.data.borrow_mut()[..])?;
    
    // Emit event
    let component_name = b"market_data_rate_limiter        ";
    let mut name_bytes = [0u8; 32];
    name_bytes.copy_from_slice(&component_name[..32]);
    
    emit_event(EventType::ComponentHealthUpdate, &ComponentHealthUpdateEvent {
        component_name: name_bytes,
        status: 2, // 2 indicates degraded/rate limited
        latency_ms: 0,
        throughput: MARKET_RATE_LIMIT as u64,
    });
    
    msg!("Market rate limit test completed");
    
    Ok(())
}

/// Test order data rate limit with burst handling
pub fn test_order_rate_limit_burst(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let rate_limiter_account = next_account_info(account_iter)?;
    
    msg!("Testing order data rate limit with burst traffic");
    
    // Load rate limiter
    let mut rate_limiter = RateLimiter::deserialize(&mut &rate_limiter_account.data.borrow()[..])?;
    
    // RateLimiter is already configured with internal order limits
    
    // Step 1: Simulate burst of order requests
    msg!("Step 1: Simulating burst of {} order requests", 100);
    
    let burst_size = 100;
    let mut burst_results = Vec::new();
    
    for i in 0..burst_size {
        let result = rate_limiter.check_order_limit();
        burst_results.push(result.is_ok());
        
        if result.is_ok() {
            // Request count is tracked internally
        }
    }
    
    let successful_in_burst = burst_results.iter().filter(|&&x| x).count();
    msg!("Burst results: {}/{} succeeded", successful_in_burst, burst_size);
    
    // Step 2: Test token bucket algorithm
    msg!("Step 2: Testing token bucket rate limiting");
    
    let mut token_bucket = TokenBucket {
        capacity: ORDER_RATE_LIMIT,
        tokens: ORDER_RATE_LIMIT,
        refill_rate: ORDER_RATE_LIMIT / ORDER_WINDOW_SECONDS as u32,
        last_refill: Clock::get()?.unix_timestamp,
    };
    
    // Consume tokens
    let tokens_needed = 50;
    match token_bucket.consume(tokens_needed) {
        Ok(_) => msg!("✓ Consumed {} tokens", tokens_needed),
        Err(_) => msg!("✗ Insufficient tokens"),
    }
    
    msg!("Remaining tokens: {}", token_bucket.tokens);
    
    // Step 3: Test priority queue for high-value requests
    msg!("Step 3: Testing priority queue for rate-limited requests");
    
    let mut priority_queue = RequestPriorityQueue::new();
    
    // Add requests with different priorities
    priority_queue.add_request(RequestPriority::High, "liquidation_check");
    priority_queue.add_request(RequestPriority::Medium, "price_update");
    priority_queue.add_request(RequestPriority::Low, "volume_stats");
    
    // Process queue when rate limit allows
    while let Some(request) = priority_queue.get_next() {
        if rate_limiter.check_order_limit().is_ok() {
            msg!("Processing {} priority request: {}", 
                match request.priority {
                    RequestPriority::High => "HIGH",
                    RequestPriority::Medium => "MEDIUM",
                    RequestPriority::Low => "LOW",
                },
                request.request_type
            );
            // Request count is tracked internally
        } else {
            msg!("Rate limited - deferring request");
            priority_queue.requeue(request);
            break;
        }
    }
    
    // Save state
    rate_limiter.serialize(&mut &mut rate_limiter_account.data.borrow_mut()[..])?;
    
    msg!("Order rate limit burst test completed");
    
    Ok(())
}

/// Test rate limit recovery strategies
pub fn test_rate_limit_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing rate limit recovery strategies");
    
    // Strategy 1: Exponential backoff
    msg!("Strategy 1: Exponential backoff");
    
    let mut backoff = ExponentialBackoff {
        base_delay_ms: 100,
        max_delay_ms: 10000,
        current_attempt: 0,
    };
    
    for i in 0..5 {
        let delay = backoff.next_delay();
        msg!("  Attempt {}: Wait {} ms", i, delay);
    }
    
    // Strategy 2: Circuit breaker pattern
    msg!("Strategy 2: Circuit breaker pattern");
    
    let mut circuit_breaker = ApiCircuitBreaker {
        failure_threshold: 5,
        recovery_timeout: 60,
        failure_count: 0,
        last_failure: None,
        state: CircuitState::Closed,
    };
    
    // Simulate failures
    for _ in 0..6 {
        circuit_breaker.record_failure();
    }
    
    match circuit_breaker.state {
        CircuitState::Open => msg!("  Circuit breaker OPEN - blocking requests"),
        CircuitState::HalfOpen => msg!("  Circuit breaker HALF-OPEN - testing recovery"),
        CircuitState::Closed => msg!("  Circuit breaker CLOSED - normal operation"),
    }
    
    // Strategy 3: Request coalescing
    msg!("Strategy 3: Request coalescing");
    
    let mut coalescer = RequestCoalescer::new();
    
    // Multiple requests for same data
    coalescer.add_request("BTC-USD", Clock::get()?.slot);
    coalescer.add_request("BTC-USD", Clock::get()?.slot + 1);
    coalescer.add_request("BTC-USD", Clock::get()?.slot + 2);
    
    let coalesced = coalescer.get_unique_requests();
    msg!("  Coalesced {} requests into {} unique", 3, coalesced.len());
    
    msg!("Rate limit recovery test completed");
    
    Ok(())
}


/// Oracle cache structure
#[derive(BorshSerialize, BorshDeserialize)]
struct OracleCache {
    last_price: u64,
    timestamp: i64,
    market_id: [u8; 32],
}

impl OracleCache {
    fn is_valid(&self) -> bool {
        Clock::get().unwrap().unix_timestamp - self.timestamp < 300 // 5 minutes
    }
}

/// Token bucket for rate limiting
struct TokenBucket {
    capacity: u32,
    tokens: u32,
    refill_rate: u32,
    last_refill: i64,
}

impl TokenBucket {
    fn consume(&mut self, tokens: u32) -> Result<(), ProgramError> {
        self.refill();
        
        if self.tokens >= tokens {
            self.tokens -= tokens;
            Ok(())
        } else {
            Err(BettingPlatformError::InsufficientTokens.into())
        }
    }
    
    fn refill(&mut self) {
        let current_time = Clock::get().unwrap().unix_timestamp;
        let elapsed = current_time - self.last_refill;
        
        let tokens_to_add = (elapsed as u32 * self.refill_rate).min(self.capacity - self.tokens);
        self.tokens += tokens_to_add;
        self.last_refill = current_time;
    }
}

/// Request priority for queueing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RequestPriority {
    Low,
    Medium,
    High,
}

/// Priority queue for requests
struct RequestPriorityQueue {
    requests: Vec<PrioritizedRequest>,
}

struct PrioritizedRequest {
    priority: RequestPriority,
    request_type: &'static str,
}

impl RequestPriorityQueue {
    fn new() -> Self {
        Self { requests: Vec::new() }
    }
    
    fn add_request(&mut self, priority: RequestPriority, request_type: &'static str) {
        self.requests.push(PrioritizedRequest { priority, request_type });
        self.requests.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
    
    fn get_next(&mut self) -> Option<PrioritizedRequest> {
        self.requests.pop()
    }
    
    fn requeue(&mut self, request: PrioritizedRequest) {
        self.add_request(request.priority, request.request_type);
    }
}

/// Exponential backoff for retries
struct ExponentialBackoff {
    base_delay_ms: u64,
    max_delay_ms: u64,
    current_attempt: u32,
}

impl ExponentialBackoff {
    fn next_delay(&mut self) -> u64 {
        let delay = (self.base_delay_ms * 2u64.pow(self.current_attempt)).min(self.max_delay_ms);
        self.current_attempt += 1;
        delay
    }
}

/// Circuit breaker for API calls
struct ApiCircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: i64,
    failure_count: u32,
    last_failure: Option<i64>,
    state: CircuitState,
}

#[derive(Debug, PartialEq)]
enum CircuitState {
    Closed,    // Normal operation
    Open,      // Blocking requests
    HalfOpen,  // Testing recovery
}

impl ApiCircuitBreaker {
    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Clock::get().unwrap().unix_timestamp);
        
        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
        }
    }
}

/// Request coalescer
struct RequestCoalescer {
    pending: Vec<(String, u64)>,
}

impl RequestCoalescer {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }
    
    fn add_request(&mut self, market: &str, slot: u64) {
        self.pending.push((market.to_string(), slot));
    }
    
    fn get_unique_requests(&self) -> Vec<&str> {
        let mut unique = Vec::new();
        for (market, _) in &self.pending {
            if !unique.contains(&market.as_str()) {
                unique.push(market.as_str());
            }
        }
        unique
    }
}

/// Simulate batched requests
fn simulate_batched_requests(
    rate_limiter: &mut RateLimiter,
    batch_size: u32,
) -> Result<u32, ProgramError> {
    // In real implementation, would batch multiple market requests
    // For simulation, assume we can batch into fewer API calls
    let api_calls_needed = (batch_size + 9) / 10; // Batch up to 10 per call
    
    for _ in 0..api_calls_needed {
        rate_limiter.check_market_limit()?;
    }
    
    Ok(api_calls_needed)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rate_limit_check() {
        let mut limiter = RateLimiter::new();
        
        // Should allow requests initially
        assert!(limiter.get_current_requests() < limiter.get_requests_per_window());
        
        // Fill up to the limit by making requests
        for _ in 0..50 {
            let _ = limiter.check_market_limit();
        }
        
        // Now should be at or near limit
        assert!(limiter.check_market_limit().is_err());
    }
}