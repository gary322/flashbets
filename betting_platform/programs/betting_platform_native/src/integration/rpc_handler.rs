use solana_program::{
    account_info::AccountInfo,
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
    events::{emit_event, EventType},
};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// RPC handling constants from Part 7 spec
pub const TARGET_RPC_PER_SECOND: u32 = 10_000; // 10k req/s from spec
pub const RPC_BATCH_SIZE: usize = 100; // Process in batches
pub const MAX_CONCURRENT_REQUESTS: usize = 1000; // Max in-flight requests
pub const REQUEST_TIMEOUT_MS: u64 = 5000; // 5 second timeout
pub const RETRY_ATTEMPTS: u8 = 3; // Number of retries
pub const RATE_LIMIT_WINDOW_MS: u64 = 1000; // 1 second window
pub const MAX_REQUEST_QUEUE_SIZE: usize = 50_000; // Buffer for bursts

/// RPC request types
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Debug)]
pub enum RpcRequestType {
    GetAccountInfo { pubkey: Pubkey },
    GetMultipleAccounts { pubkeys: Vec<Pubkey> },
    GetProgramAccounts { program_id: Pubkey, filters: Vec<RpcFilter> },
    GetSlot,
    GetBlockHeight,
    GetTransaction { signature: [u8; 64] },
    SendTransaction { data: Vec<u8> },
    SimulateTransaction { data: Vec<u8> },
    GetHealth,
    GetVersion,
}

/// RPC filter for program accounts
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Debug)]
pub struct RpcFilter {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

/// RPC request with metadata
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RpcRequest {
    pub id: u64,
    pub request_type: RpcRequestType,
    pub priority: RequestPriority,
    pub created_at: i64,
    pub retry_count: u8,
    pub callback: Option<Pubkey>, // Account to notify on completion
}

/// Request priority levels
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Debug)]
pub enum RequestPriority {
    Critical = 3,   // Liquidations, urgent trades
    High = 2,       // Normal trades, oracle updates
    Normal = 1,     // Market data, analytics
    Low = 0,        // Historical data, background tasks
}

/// RPC response
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RpcResponse {
    pub request_id: u64,
    pub success: bool,
    pub data: Vec<u8>,
    pub error: Option<String>,
    pub latency_ms: u32,
}

/// Rate limiter using token bucket algorithm
#[derive(BorshSerialize, BorshDeserialize)]
pub struct RateLimiter {
    pub tokens: u32,
    pub max_tokens: u32,
    pub refill_rate: u32, // Tokens per second
    pub last_refill: i64,
}

impl RateLimiter {
    pub fn new(max_tokens: u32, refill_rate: u32) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Clock::get().unwrap().unix_timestamp,
        }
    }

    /// Try to consume tokens, returns true if successful
    pub fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Clock::get().unwrap().unix_timestamp;
        let elapsed = (now - self.last_refill) as u32;
        
        if elapsed > 0 {
            let new_tokens = elapsed * self.refill_rate;
            self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
            self.last_refill = now;
        }
    }
}

/// Request queue with priority ordering
#[derive(BorshSerialize, BorshDeserialize)]
pub struct PriorityRequestQueue {
    pub queues: [VecDeque<RpcRequest>; 4], // One queue per priority level
    pub total_size: usize,
}

impl PriorityRequestQueue {
    pub fn new() -> Self {
        Self {
            queues: [
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
            ],
            total_size: 0,
        }
    }

    /// Add request to appropriate priority queue
    pub fn push(&mut self, request: RpcRequest) -> Result<(), ProgramError> {
        if self.total_size >= MAX_REQUEST_QUEUE_SIZE {
            return Err(BettingPlatformError::RequestQueueFull.into());
        }

        let priority_index = request.priority as usize;
        self.queues[priority_index].push_back(request);
        self.total_size += 1;
        Ok(())
    }

    /// Get next highest priority request
    pub fn pop(&mut self) -> Option<RpcRequest> {
        // Check from highest to lowest priority
        for i in (0..4).rev() {
            if let Some(request) = self.queues[i].pop_front() {
                self.total_size -= 1;
                return Some(request);
            }
        }
        None
    }

    /// Get batch of requests respecting priority
    pub fn get_batch(&mut self, max_size: usize) -> Vec<RpcRequest> {
        let mut batch = Vec::new();
        
        while batch.len() < max_size {
            if let Some(request) = self.pop() {
                batch.push(request);
            } else {
                break;
            }
        }
        
        batch
    }
}

/// RPC handler state
#[derive(BorshSerialize, BorshDeserialize)]
pub struct RpcHandler {
    pub rate_limiter: RateLimiter,
    pub request_queue: PriorityRequestQueue,
    pub in_flight_requests: HashMap<u64, RpcRequest>,
    pub total_requests: u64,
    pub total_responses: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: u32,
    pub peak_rps: u32,
    pub current_rps: u32,
    pub last_rps_update: i64,
}

impl RpcHandler {
    pub const SIZE: usize = 
        32 + // rate_limiter
        4 + (MAX_REQUEST_QUEUE_SIZE * 100) + // request_queue (approx)
        4 + (MAX_CONCURRENT_REQUESTS * 100) + // in_flight_requests (approx)
        8 + // total_requests
        8 + // total_responses
        8 + // failed_requests
        4 + // avg_latency_ms
        4 + // peak_rps
        4 + // current_rps
        8; // last_rps_update

    /// Initialize new RPC handler
    pub fn new() -> Self {
        Self {
            rate_limiter: RateLimiter::new(TARGET_RPC_PER_SECOND, TARGET_RPC_PER_SECOND),
            request_queue: PriorityRequestQueue::new(),
            in_flight_requests: HashMap::new(),
            total_requests: 0,
            total_responses: 0,
            failed_requests: 0,
            avg_latency_ms: 0,
            peak_rps: 0,
            current_rps: 0,
            last_rps_update: Clock::get().unwrap().unix_timestamp,
        }
    }

    /// Submit new RPC request
    pub fn submit_request(
        &mut self,
        request_type: RpcRequestType,
        priority: RequestPriority,
    ) -> Result<u64, ProgramError> {
        // Check rate limit
        if !self.rate_limiter.try_consume(1) {
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }

        let request_id = self.generate_request_id();
        let request = RpcRequest {
            id: request_id,
            request_type,
            priority,
            created_at: Clock::get().unwrap().unix_timestamp,
            retry_count: 0,
            callback: None,
        };

        // Add to queue
        self.request_queue.push(request)?;
        self.total_requests += 1;
        self.update_rps();

        Ok(request_id)
    }

    /// Process batch of requests
    pub fn process_batch(&mut self) -> Result<ProcessBatchResult, ProgramError> {
        let batch = self.request_queue.get_batch(RPC_BATCH_SIZE);
        let batch_size = batch.len();
        
        if batch_size == 0 {
            return Ok(ProcessBatchResult {
                processed: 0,
                in_flight: self.in_flight_requests.len() as u32,
                queue_size: self.request_queue.total_size as u32,
            });
        }

        // Check concurrent limit
        let available_slots = MAX_CONCURRENT_REQUESTS.saturating_sub(self.in_flight_requests.len());
        let to_process = batch_size.min(available_slots);

        // Process requests
        for request in batch.into_iter().take(to_process) {
            self.in_flight_requests.insert(request.id, request.clone());
            
            // In production, this would trigger actual RPC call
            msg!("Processing RPC request {} with priority {:?}", request.id, request.priority);
        }

        Ok(ProcessBatchResult {
            processed: to_process as u32,
            in_flight: self.in_flight_requests.len() as u32,
            queue_size: self.request_queue.total_size as u32,
        })
    }

    /// Handle RPC response
    pub fn handle_response(
        &mut self,
        request_id: u64,
        success: bool,
        data: Vec<u8>,
        latency_ms: u32,
    ) -> Result<(), ProgramError> {
        let request = self.in_flight_requests.remove(&request_id)
            .ok_or(BettingPlatformError::RequestNotFound)?;

        if success {
            self.total_responses += 1;
            self.update_avg_latency(latency_ms);
            
            // Process successful response
            msg!("RPC request {} completed in {}ms", request_id, latency_ms);
        } else {
            // Handle failure
            if request.retry_count < RETRY_ATTEMPTS {
                // Retry with increased count
                let retry_count = request.retry_count + 1;
                let mut retry_request = request;
                retry_request.retry_count = retry_count;
                self.request_queue.push(retry_request)?;
                msg!("Retrying RPC request {} (attempt {})", request_id, retry_count);
            } else {
                self.failed_requests += 1;
                msg!("RPC request {} failed after {} attempts", request_id, RETRY_ATTEMPTS);
            }
        }

        Ok(())
    }

    /// Update average latency
    fn update_avg_latency(&mut self, new_latency: u32) {
        if self.avg_latency_ms == 0 {
            self.avg_latency_ms = new_latency;
        } else {
            // Exponential moving average
            self.avg_latency_ms = (self.avg_latency_ms * 9 + new_latency) / 10;
        }
    }

    /// Update requests per second metrics
    fn update_rps(&mut self) {
        let now = Clock::get().unwrap().unix_timestamp;
        let elapsed = now - self.last_rps_update;
        
        if elapsed >= 1 {
            self.current_rps = ((self.total_requests as i64 - self.last_rps_update) / elapsed) as u32;
            if self.current_rps > self.peak_rps {
                self.peak_rps = self.current_rps;
            }
            self.last_rps_update = now;
        }
    }

    /// Generate unique request ID
    fn generate_request_id(&self) -> u64 {
        let clock = Clock::get().unwrap();
        ((clock.unix_timestamp as u64) << 32) | (self.total_requests & 0xFFFFFFFF)
    }

    /// Get handler statistics
    pub fn get_stats(&self) -> RpcHandlerStats {
        let success_rate = if self.total_requests > 0 {
            ((self.total_responses as f64) / (self.total_requests as f64)) * 100.0
        } else {
            0.0
        };

        RpcHandlerStats {
            total_requests: self.total_requests,
            total_responses: self.total_responses,
            failed_requests: self.failed_requests,
            in_flight: self.in_flight_requests.len() as u32,
            queue_size: self.request_queue.total_size as u32,
            avg_latency_ms: self.avg_latency_ms,
            current_rps: self.current_rps,
            peak_rps: self.peak_rps,
            success_rate,
            rate_limit_available: self.rate_limiter.tokens,
        }
    }

    /// Batch submit multiple requests
    pub fn batch_submit(
        &mut self,
        requests: Vec<(RpcRequestType, RequestPriority)>,
    ) -> Result<Vec<u64>, ProgramError> {
        let mut request_ids = Vec::new();
        
        // Check if we can handle the batch
        let tokens_needed = requests.len() as u32;
        if !self.rate_limiter.try_consume(tokens_needed) {
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }

        for (request_type, priority) in requests {
            let request_id = self.generate_request_id();
            let request = RpcRequest {
                id: request_id,
                request_type,
                priority,
                created_at: Clock::get().unwrap().unix_timestamp,
                retry_count: 0,
                callback: None,
            };

            self.request_queue.push(request)?;
            request_ids.push(request_id);
            self.total_requests += 1;
        }

        self.update_rps();
        
        msg!("Batch submitted {} RPC requests", request_ids.len());
        Ok(request_ids)
    }
}

/// Process batch result
#[derive(Debug)]
pub struct ProcessBatchResult {
    pub processed: u32,
    pub in_flight: u32,
    pub queue_size: u32,
}

/// RPC handler statistics
#[derive(Debug)]
pub struct RpcHandlerStats {
    pub total_requests: u64,
    pub total_responses: u64,
    pub failed_requests: u64,
    pub in_flight: u32,
    pub queue_size: u32,
    pub avg_latency_ms: u32,
    pub current_rps: u32,
    pub peak_rps: u32,
    pub success_rate: f64,
    pub rate_limit_available: u32,
}

/// Circuit breaker for endpoint health
#[derive(BorshSerialize, BorshDeserialize)]
pub struct CircuitBreaker {
    pub endpoint: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure: i64,
    pub last_state_change: i64,
    pub failure_threshold: u32,
    pub recovery_timeout: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Debug)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing recovery
}

impl CircuitBreaker {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: 0,
            last_state_change: Clock::get().unwrap().unix_timestamp,
            failure_threshold: 5,
            recovery_timeout: 30, // 30 seconds
        }
    }

    /// Record request outcome
    pub fn record_outcome(&mut self, success: bool) {
        let now = Clock::get().unwrap().unix_timestamp;
        
        match self.state {
            CircuitState::Closed => {
                if success {
                    self.success_count += 1;
                    self.failure_count = 0; // Reset on success
                } else {
                    self.failure_count += 1;
                    self.last_failure = now;
                    
                    if self.failure_count >= self.failure_threshold {
                        self.state = CircuitState::Open;
                        self.last_state_change = now;
                        msg!("Circuit breaker OPEN for endpoint {}", self.endpoint);
                    }
                }
            }
            CircuitState::Open => {
                // Check if we should try recovery
                if now - self.last_state_change >= self.recovery_timeout {
                    self.state = CircuitState::HalfOpen;
                    self.last_state_change = now;
                    msg!("Circuit breaker HALF-OPEN for endpoint {}", self.endpoint);
                }
            }
            CircuitState::HalfOpen => {
                if success {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 1;
                    self.last_state_change = now;
                    msg!("Circuit breaker CLOSED for endpoint {}", self.endpoint);
                } else {
                    self.state = CircuitState::Open;
                    self.failure_count += 1;
                    self.last_failure = now;
                    self.last_state_change = now;
                    msg!("Circuit breaker OPEN again for endpoint {}", self.endpoint);
                }
            }
        }
    }

    /// Check if requests are allowed
    pub fn is_request_allowed(&self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => true, // Allow test request
        }
    }
}

/// Initialize RPC handler
pub fn initialize_rpc_handler(
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing RPC handler");
    msg!("Target RPS: {}", TARGET_RPC_PER_SECOND);
    msg!("Max concurrent requests: {}", MAX_CONCURRENT_REQUESTS);
    msg!("Request timeout: {}ms", REQUEST_TIMEOUT_MS);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(100, 10);
        
        // Should allow initial burst
        assert!(limiter.try_consume(50));
        assert_eq!(limiter.tokens, 50);
        
        // Should allow more within limit
        assert!(limiter.try_consume(50));
        assert_eq!(limiter.tokens, 0);
        
        // Should reject when exhausted
        assert!(!limiter.try_consume(1));
    }

    #[test]
    fn test_priority_queue() {
        let mut queue = PriorityRequestQueue::new();
        
        // Add requests with different priorities
        let req1 = RpcRequest {
            id: 1,
            request_type: RpcRequestType::GetSlot,
            priority: RequestPriority::Low,
            created_at: 0,
            retry_count: 0,
            callback: None,
        };
        
        let req2 = RpcRequest {
            id: 2,
            request_type: RpcRequestType::GetSlot,
            priority: RequestPriority::Critical,
            created_at: 0,
            retry_count: 0,
            callback: None,
        };
        
        queue.push(req1.clone()).unwrap();
        queue.push(req2.clone()).unwrap();
        
        // Should get critical request first
        let next = queue.pop().unwrap();
        assert_eq!(next.id, 2);
        assert_eq!(next.priority, RequestPriority::Critical);
    }

    #[test]
    fn test_circuit_breaker() {
        let mut breaker = CircuitBreaker::new("test-endpoint".to_string());
        
        // Should start closed
        assert_eq!(breaker.state, CircuitState::Closed);
        assert!(breaker.is_request_allowed());
        
        // Record failures
        for _ in 0..5 {
            breaker.record_outcome(false);
        }
        
        // Should be open after threshold
        assert_eq!(breaker.state, CircuitState::Open);
        assert!(!breaker.is_request_allowed());
    }

    #[test]
    fn test_batch_processing() {
        let mut handler = RpcHandler::new();
        
        // Submit multiple requests
        let requests = vec![
            (RpcRequestType::GetSlot, RequestPriority::High),
            (RpcRequestType::GetBlockHeight, RequestPriority::Normal),
            (RpcRequestType::GetHealth, RequestPriority::Low),
        ];
        
        let ids = handler.batch_submit(requests).unwrap();
        assert_eq!(ids.len(), 3);
        
        // Process batch
        let result = handler.process_batch().unwrap();
        assert!(result.processed > 0);
    }
}