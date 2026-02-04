//! Production-ready circuit breaker implementation for failure prevention

use std::{
    sync::{Arc, atomic::{AtomicU64, AtomicU32, Ordering}},
    time::{Duration, Instant},
    collections::HashMap,
    future::Future,
};
use tokio::sync::{RwLock, Mutex};
use serde::{Serialize, Deserialize};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Failing - requests are rejected
    Open,
    /// Testing recovery - limited requests allowed
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
    /// Time to wait before transitioning from open to half-open
    pub reset_timeout: Duration,
    /// Maximum requests allowed in half-open state
    pub half_open_max_calls: u32,
    /// Time window for failure counting
    pub failure_window: Duration,
    /// Minimum number of calls before evaluating
    pub min_calls: u32,
    /// Failure rate threshold (0.0 - 1.0)
    pub failure_rate_threshold: f64,
    /// Slow call duration threshold
    pub slow_call_duration: Duration,
    /// Slow call rate threshold (0.0 - 1.0)
    pub slow_call_rate_threshold: f64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            reset_timeout: Duration::from_secs(30),
            half_open_max_calls: 3,
            failure_window: Duration::from_secs(60),
            min_calls: 10,
            failure_rate_threshold: 0.5,
            slow_call_duration: Duration::from_secs(5),
            slow_call_rate_threshold: 0.5,
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone, Serialize)]
pub struct CircuitBreakerMetrics {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub rejected_calls: u64,
    pub slow_calls: u64,
    pub state_transitions: u64,
    pub last_failure_time: Option<DateTime<Utc>>,
    pub last_success_time: Option<DateTime<Utc>>,
    pub current_failure_rate: f64,
    pub current_slow_call_rate: f64,
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    metrics: Arc<CircuitBreakerMetrics>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    half_open_calls: AtomicU32,
    last_state_change: Arc<Mutex<Instant>>,
    recent_calls: Arc<RwLock<Vec<CallRecord>>>,
}

#[derive(Debug, Clone)]
struct CallRecord {
    timestamp: Instant,
    duration: Duration,
    success: bool,
}

impl CircuitBreaker {
    /// Create new circuit breaker
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            metrics: Arc::new(CircuitBreakerMetrics {
                total_calls: 0,
                successful_calls: 0,
                failed_calls: 0,
                rejected_calls: 0,
                slow_calls: 0,
                state_transitions: 0,
                last_failure_time: None,
                last_success_time: None,
                current_failure_rate: 0.0,
                current_slow_call_rate: 0.0,
            }),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            half_open_calls: AtomicU32::new(0),
            last_state_change: Arc::new(Mutex::new(Instant::now())),
            recent_calls: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Execute operation with circuit breaker protection
    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        // Check if we should allow the call
        let state = self.get_current_state().await;
        
        match state {
            CircuitState::Open => {
                self.increment_rejected_calls();
                return Err(CircuitBreakerError::CircuitOpen);
            }
            CircuitState::HalfOpen => {
                let current_calls = self.half_open_calls.fetch_add(1, Ordering::SeqCst);
                if current_calls >= self.config.half_open_max_calls {
                    self.half_open_calls.fetch_sub(1, Ordering::SeqCst);
                    self.increment_rejected_calls();
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            CircuitState::Closed => {}
        }
        
        // Execute the operation
        let start = Instant::now();
        let result = operation().await;
        let duration = start.elapsed();
        
        // Record the call
        self.record_call(duration, result.is_ok()).await;
        
        // Handle the result
        match result {
            Ok(value) => {
                self.on_success(duration).await;
                Ok(value)
            }
            Err(error) => {
                self.on_failure(duration).await;
                Err(CircuitBreakerError::OperationFailed(error))
            }
        }
    }
    
    /// Get current circuit state
    async fn get_current_state(&self) -> CircuitState {
        let mut state = self.state.write().await;
        
        // Check if we should transition from Open to HalfOpen
        if *state == CircuitState::Open {
            let last_change = *self.last_state_change.lock().await;
            if last_change.elapsed() >= self.config.reset_timeout {
                self.transition_to_half_open(&mut state).await;
            }
        }
        
        *state
    }
    
    /// Handle successful call
    async fn on_success(&self, duration: Duration) {
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.successful_calls += 1;
        metrics.last_success_time = Some(Utc::now());
        
        if duration > self.config.slow_call_duration {
            metrics.slow_calls += 1;
        }
        
        let state = *self.state.read().await;
        
        match state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                
                if success_count >= self.config.success_threshold {
                    let mut state = self.state.write().await;
                    self.transition_to_closed(&mut state).await;
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but log it
                warn!("Successful call recorded in Open state for circuit {}", self.name);
            }
        }
    }
    
    /// Handle failed call
    async fn on_failure(&self, duration: Duration) {
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.failed_calls += 1;
        metrics.last_failure_time = Some(Utc::now());
        
        if duration > self.config.slow_call_duration {
            metrics.slow_calls += 1;
        }
        
        let state = *self.state.read().await;
        
        match state {
            CircuitState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                
                // Check if we should open the circuit
                if self.should_open_circuit().await {
                    let mut state = self.state.write().await;
                    self.transition_to_open(&mut state).await;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                let mut state = self.state.write().await;
                self.transition_to_open(&mut state).await;
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }
    
    /// Check if circuit should be opened based on metrics
    async fn should_open_circuit(&self) -> bool {
        let recent_calls = self.recent_calls.read().await;
        
        // Clean old records
        let window_start = Instant::now() - self.config.failure_window;
        let valid_calls: Vec<_> = recent_calls.iter()
            .filter(|call| call.timestamp > window_start)
            .cloned()
            .collect();
        
        if valid_calls.len() < self.config.min_calls as usize {
            return false;
        }
        
        // Calculate failure rate
        let failed_calls = valid_calls.iter().filter(|call| !call.success).count();
        let failure_rate = failed_calls as f64 / valid_calls.len() as f64;
        
        // Calculate slow call rate
        let slow_calls = valid_calls.iter()
            .filter(|call| call.duration > self.config.slow_call_duration)
            .count();
        let slow_call_rate = slow_calls as f64 / valid_calls.len() as f64;
        
        // Update metrics
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.current_failure_rate = failure_rate;
        metrics.current_slow_call_rate = slow_call_rate;
        
        // Check thresholds
        failure_rate > self.config.failure_rate_threshold ||
        slow_call_rate > self.config.slow_call_rate_threshold
    }
    
    /// Record a call
    async fn record_call(&self, duration: Duration, success: bool) {
        let mut recent_calls = self.recent_calls.write().await;
        
        // Add new record
        recent_calls.push(CallRecord {
            timestamp: Instant::now(),
            duration,
            success,
        });
        
        // Clean old records
        let window_start = Instant::now() - self.config.failure_window;
        recent_calls.retain(|call| call.timestamp > window_start);
        
        // Update total calls metric
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.total_calls += 1;
    }
    
    /// Transition to Open state
    async fn transition_to_open(&self, state: &mut CircuitState) {
        *state = CircuitState::Open;
        *self.last_state_change.lock().await = Instant::now();
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
        
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.state_transitions += 1;
        
        error!(
            "Circuit breaker '{}' opened due to failures. Will retry in {:?}",
            self.name, self.config.reset_timeout
        );
    }
    
    /// Transition to Closed state
    async fn transition_to_closed(&self, state: &mut CircuitState) {
        *state = CircuitState::Closed;
        *self.last_state_change.lock().await = Instant::now();
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
        
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.state_transitions += 1;
        
        info!("Circuit breaker '{}' closed after successful recovery", self.name);
    }
    
    /// Transition to HalfOpen state
    async fn transition_to_half_open(&self, state: &mut CircuitState) {
        *state = CircuitState::HalfOpen;
        *self.last_state_change.lock().await = Instant::now();
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
        
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.state_transitions += 1;
        
        info!(
            "Circuit breaker '{}' half-open, testing with {} calls",
            self.name, self.config.half_open_max_calls
        );
    }
    
    /// Increment rejected calls counter
    fn increment_rejected_calls(&self) {
        let metrics = unsafe { &mut *(Arc::as_ptr(&self.metrics) as *mut CircuitBreakerMetrics) };
        metrics.rejected_calls += 1;
    }
    
    /// Get current state
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }
    
    /// Get metrics
    pub fn metrics(&self) -> &CircuitBreakerMetrics {
        &self.metrics
    }
    
    /// Reset circuit breaker
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        self.transition_to_closed(&mut state).await;
        
        // Clear recent calls
        self.recent_calls.write().await.clear();
    }
}

/// Circuit breaker error types
#[derive(Debug, Clone)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open, request rejected
    CircuitOpen,
    /// Operation failed with error
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::OperationFailed(e) => write!(f, "Operation failed: {}", e),
        }
    }
}

impl<E: std::error::Error> std::error::Error for CircuitBreakerError<E> {}

/// Circuit breaker manager for multiple breakers
pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create new circuit breaker manager
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config,
        }
    }
    
    /// Get or create circuit breaker
    pub async fn get_or_create(&self, name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write().await;
        
        if let Some(breaker) = breakers.get(name) {
            breaker.clone()
        } else {
            let breaker = Arc::new(CircuitBreaker::new(name, self.default_config.clone()));
            breakers.insert(name.to_string(), breaker.clone());
            breaker
        }
    }
    
    /// Get circuit breaker if exists
    pub async fn get(&self, name: &str) -> Option<Arc<CircuitBreaker>> {
        self.breakers.read().await.get(name).cloned()
    }
    
    /// Get all circuit breakers
    pub async fn all(&self) -> Vec<(String, Arc<CircuitBreaker>)> {
        self.breakers.read().await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    
    /// Get metrics for all breakers
    pub async fn all_metrics(&self) -> HashMap<String, CircuitBreakerMetrics> {
        let breakers = self.breakers.read().await;
        breakers.iter()
            .map(|(name, breaker)| (name.clone(), breaker.metrics().clone()))
            .collect()
    }
    
    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        let breakers = self.breakers.read().await;
        for breaker in breakers.values() {
            breaker.reset().await;
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    
    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            min_calls: 3,
            failure_rate_threshold: 0.5,
            ..Default::default()
        };
        
        let breaker = CircuitBreaker::new("test", config);
        
        // Fail 3 times
        for _ in 0..3 {
            let _ = breaker.call(|| async {
                Result::<(), &'static str>::Err("fail")
            }).await;
        }
        
        // Circuit should be open
        assert_eq!(breaker.state().await, CircuitState::Open);
        
        // Next call should be rejected
        let result = breaker.call(|| async {
            Result::<(), &'static str>::Ok(())
        }).await;
        
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            reset_timeout: Duration::from_millis(100),
            half_open_max_calls: 3,
            min_calls: 2,
            ..Default::default()
        };
        
        let breaker = CircuitBreaker::new("test", config);
        
        // Open the circuit
        for _ in 0..2 {
            let _ = breaker.call(|| async {
                Result::<(), &'static str>::Err("fail")
            }).await;
        }
        
        assert_eq!(breaker.state().await, CircuitState::Open);
        
        // Wait for reset timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should be half-open now, succeed twice
        for _ in 0..2 {
            let result = breaker.call(|| async {
                Result::<(), &'static str>::Ok(())
            }).await;
            assert!(result.is_ok());
        }
        
        // Should be closed now
        assert_eq!(breaker.state().await, CircuitState::Closed);
    }
}