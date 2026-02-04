//! External API integration service with circuit breakers and retry logic

use anyhow::{Result, anyhow};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, Duration, timeout};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error, warn, debug};
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use backoff::{ExponentialBackoff, backoff::Backoff};
use async_trait::async_trait;

use crate::integration::{
    Platform, ExternalPrice, IntegrationConfig,
    polymarket::PolymarketClient,
    kalshi::KalshiClient,
};

/// External API service with resilience patterns
pub struct ExternalApiService {
    config: IntegrationConfig,
    clients: Arc<RwLock<HashMap<Platform, Box<dyn ExternalApiClient>>>>,
    circuit_breakers: Arc<RwLock<HashMap<Platform, CircuitBreaker>>>,
    health_status: Arc<RwLock<HashMap<Platform, ApiHealth>>>,
    retry_policy: RetryPolicy,
}

/// API health status
#[derive(Debug, Clone, Serialize)]
pub struct ApiHealth {
    pub platform: Platform,
    pub is_healthy: bool,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub latency_ms: Option<u64>,
    pub error_message: Option<String>,
}

/// Circuit breaker state
#[derive(Debug, Clone)]
enum CircuitState {
    Closed,
    Open { opened_at: DateTime<Utc> },
    HalfOpen,
}

/// Circuit breaker for API calls
#[derive(Debug, Clone)]
struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    failure_threshold: u32,
    recovery_timeout: Duration,
    half_open_max_calls: u32,
}

/// Retry policy configuration
#[derive(Debug, Clone)]
struct RetryPolicy {
    max_retries: u32,
    initial_interval: Duration,
    max_interval: Duration,
    multiplier: f64,
}

/// External API client trait
#[async_trait::async_trait]
trait ExternalApiClient: Send + Sync {
    async fn fetch_markets(&self, limit: usize) -> Result<Vec<MarketData>>;
    async fn fetch_prices(&self, market_ids: Vec<String>) -> Result<Vec<PriceData>>;
    async fn health_check(&self) -> Result<()>;
    fn platform(&self) -> Platform;
}

/// Generic market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub outcomes: Vec<String>,
    pub end_time: Option<DateTime<Utc>>,
    pub volume: f64,
    pub liquidity: f64,
    pub active: bool,
}

/// Generic price data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub market_id: String,
    pub prices: Vec<f64>,
    pub timestamp: DateTime<Utc>,
    pub volume_24h: f64,
    pub liquidity: f64,
}

impl ExternalApiService {
    /// Create new external API service
    pub fn new(config: IntegrationConfig) -> Self {
        let retry_policy = RetryPolicy {
            max_retries: 3,
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
        };
        
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            health_status: Arc::new(RwLock::new(HashMap::new())),
            retry_policy,
        }
    }
    
    /// Initialize API clients
    pub async fn initialize(&self) -> Result<()> {
        let mut clients = self.clients.write().await;
        let mut breakers = self.circuit_breakers.write().await;
        let mut health = self.health_status.write().await;
        
        // Initialize Polymarket client
        if self.config.polymarket_enabled {
            let client = PolymarketApiClient::new(
                self.config.polymarket_api_key.clone(),
                self.config.polymarket_webhook_secret.clone(),
            )?;
            
            clients.insert(Platform::Polymarket, Box::new(client));
            
            breakers.insert(Platform::Polymarket, CircuitBreaker::new(
                5, // failure threshold
                Duration::from_secs(30), // recovery timeout
                3, // half-open max calls
            ));
            
            health.insert(Platform::Polymarket, ApiHealth {
                platform: Platform::Polymarket,
                is_healthy: true,
                last_check: Utc::now(),
                consecutive_failures: 0,
                latency_ms: None,
                error_message: None,
            });
        }
        
        // Initialize Kalshi client
        if self.config.kalshi_enabled {
            let client = KalshiApiClient::new(
                self.config.kalshi_api_key.clone(),
                self.config.kalshi_api_secret.clone(),
            )?;
            
            clients.insert(Platform::Kalshi, Box::new(client));
            
            breakers.insert(Platform::Kalshi, CircuitBreaker::new(
                5, // failure threshold
                Duration::from_secs(30), // recovery timeout
                3, // half-open max calls
            ));
            
            health.insert(Platform::Kalshi, ApiHealth {
                platform: Platform::Kalshi,
                is_healthy: true,
                last_check: Utc::now(),
                consecutive_failures: 0,
                latency_ms: None,
                error_message: None,
            });
        }
        
        info!("External API service initialized with {} clients", clients.len());
        Ok(())
    }
    
    /// Fetch markets from all enabled platforms
    pub async fn fetch_all_markets(&self, limit: usize) -> HashMap<Platform, Result<Vec<MarketData>>> {
        let clients = self.clients.read().await;
        let mut results = HashMap::new();
        
        for (platform, _) in clients.iter() {
            let platform_val = *platform;
            let result = self.fetch_markets_for_platform(platform_val, limit).await;
            results.insert(platform_val, result);
        }
        
        results
    }
    
    /// Helper to fetch markets for a specific platform
    async fn fetch_markets_for_platform(&self, platform: Platform, limit: usize) -> Result<Vec<MarketData>> {
        self.fetch_with_resilience(
            platform,
            |c| Box::pin(async move { c.fetch_markets(limit).await }),
        ).await
    }
    
    /// Fetch prices for specific markets
    pub async fn fetch_prices(
        &self,
        platform: Platform,
        market_ids: Vec<String>,
    ) -> Result<Vec<PriceData>> {
        self.fetch_with_resilience(
            platform,
            |c| {
                let ids = market_ids.clone();
                Box::pin(async move { c.fetch_prices(ids).await })
            },
        ).await
    }
    
    /// Fetch with circuit breaker and retry logic
    async fn fetch_with_resilience<F, T>(
        &self,
        platform: Platform,
        operation: F,
    ) -> Result<T>
    where
        F: for<'a> Fn(&'a dyn ExternalApiClient) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + 'a>>,
    {
        // Check circuit breaker
        let breaker_state = {
            let breakers = self.circuit_breakers.read().await;
            breakers.get(&platform).map(|b| b.state.clone())
        };
        
        match breaker_state {
            Some(CircuitState::Open { opened_at }) => {
                let elapsed = Utc::now().signed_duration_since(opened_at);
                let recovery_timeout = {
                    let breakers = self.circuit_breakers.read().await;
                    breakers.get(&platform).unwrap().recovery_timeout
                };
                
                if elapsed.to_std().unwrap() < recovery_timeout {
                    return Err(anyhow!("Circuit breaker open for {:?}", platform));
                } else {
                    // Transition to half-open
                    self.transition_to_half_open(platform).await;
                }
            }
            _ => {}
        }
        
        // Get client
        let clients = self.clients.read().await;
        let client = clients.get(&platform)
            .ok_or_else(|| anyhow!("Client not found for {:?}", platform))?;
        
        // Execute with retry
        let mut backoff = ExponentialBackoff {
            initial_interval: self.retry_policy.initial_interval,
            max_interval: self.retry_policy.max_interval,
            multiplier: self.retry_policy.multiplier,
            ..Default::default()
        };
        
        let mut last_error = None;
        
        for attempt in 0..=self.retry_policy.max_retries {
            let start = std::time::Instant::now();
            
            match timeout(Duration::from_secs(30), operation(client.as_ref())).await {
                Ok(Ok(result)) => {
                    let latency = start.elapsed().as_millis() as u64;
                    self.record_success(platform, latency).await;
                    return Ok(result);
                }
                Ok(Err(e)) => {
                    last_error = Some(e.to_string());
                    self.record_failure(platform, e.to_string()).await;
                    
                    if attempt < self.retry_policy.max_retries {
                        if let Some(interval) = backoff.next_backoff() {
                            warn!(
                                "Request to {:?} failed (attempt {}), retrying in {:?}",
                                platform, attempt + 1, interval
                            );
                            tokio::time::sleep(interval).await;
                        }
                    }
                }
                Err(_) => {
                    last_error = Some("Request timeout".to_string());
                    self.record_failure(platform, "Request timeout".to_string()).await;
                }
            }
        }
        
        Err(anyhow!(
            "All retries exhausted for {:?}: {}",
            platform,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
    }
    
    /// Record successful API call
    async fn record_success(&self, platform: Platform, latency_ms: u64) {
        let mut breakers = self.circuit_breakers.write().await;
        let mut health = self.health_status.write().await;
        
        if let Some(breaker) = breakers.get_mut(&platform) {
            breaker.on_success();
        }
        
        if let Some(status) = health.get_mut(&platform) {
            status.is_healthy = true;
            status.consecutive_failures = 0;
            status.latency_ms = Some(latency_ms);
            status.last_check = Utc::now();
            status.error_message = None;
        }
    }
    
    /// Record failed API call
    async fn record_failure(&self, platform: Platform, error: String) {
        let mut breakers = self.circuit_breakers.write().await;
        let mut health = self.health_status.write().await;
        
        if let Some(breaker) = breakers.get_mut(&platform) {
            breaker.on_failure();
        }
        
        if let Some(status) = health.get_mut(&platform) {
            status.consecutive_failures += 1;
            status.last_check = Utc::now();
            status.error_message = Some(error);
            
            if status.consecutive_failures >= 3 {
                status.is_healthy = false;
            }
        }
    }
    
    /// Transition circuit breaker to half-open state
    async fn transition_to_half_open(&self, platform: Platform) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&platform) {
            breaker.state = CircuitState::HalfOpen;
            breaker.success_count = 0;
            info!("Circuit breaker for {:?} transitioned to half-open", platform);
        }
    }
    
    /// Get health status for all platforms
    pub async fn get_health_status(&self) -> HashMap<Platform, ApiHealth> {
        let health = self.health_status.read().await;
        health.clone()
    }
    
    /// Start health check monitoring
    pub async fn start_health_monitoring(&self) {
        let service = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let clients = service.clients.read().await;
                for (platform, client) in clients.iter() {
                    let platform = *platform;
                    let client = client.as_ref();
                    
                    let start = std::time::Instant::now();
                    match timeout(Duration::from_secs(10), client.health_check()).await {
                        Ok(Ok(())) => {
                            let latency = start.elapsed().as_millis() as u64;
                            service.record_success(platform, latency).await;
                            debug!("{:?} health check passed ({}ms)", platform, latency);
                        }
                        Ok(Err(e)) => {
                            service.record_failure(platform, e.to_string()).await;
                            warn!("{:?} health check failed: {}", platform, e);
                        }
                        Err(_) => {
                            service.record_failure(platform, "Health check timeout".to_string()).await;
                            warn!("{:?} health check timeout", platform);
                        }
                    }
                }
            }
        });
        
        info!("Health monitoring started");
    }
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, recovery_timeout: Duration, half_open_max_calls: u32) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold,
            recovery_timeout,
            half_open_max_calls,
        }
    }
    
    fn on_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.half_open_max_calls {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                    info!("Circuit breaker closed after successful recovery");
                }
            }
            _ => {}
        }
    }
    
    fn on_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open { opened_at: Utc::now() };
                    error!("Circuit breaker opened after {} failures", self.failure_count);
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open { opened_at: Utc::now() };
                self.failure_count = 0;
                self.success_count = 0;
                warn!("Circuit breaker reopened after failure in half-open state");
            }
            _ => {}
        }
    }
}

/// Polymarket API client adapter
struct PolymarketApiClient {
    inner: PolymarketClient,
}

impl PolymarketApiClient {
    fn new(api_key: Option<String>, webhook_secret: Option<String>) -> Result<Self> {
        Ok(Self {
            inner: PolymarketClient::new(api_key, webhook_secret)?,
        })
    }
}

#[async_trait::async_trait]
impl ExternalApiClient for PolymarketApiClient {
    async fn fetch_markets(&self, limit: usize) -> Result<Vec<MarketData>> {
        let markets = self.inner.get_markets(limit).await?;
        
        Ok(markets.into_iter().map(|m| MarketData {
            id: m.condition_id,
            title: m.question,
            description: m.description,
            outcomes: m.tokens.iter().map(|t| t.outcome.clone()).collect(),
            end_time: m.end_date_iso.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
            volume: m.tokens.iter().map(|t| t.price * 1000.0).sum(), // Rough estimate
            liquidity: m.minimum_order_size * 1000.0, // Rough estimate
            active: m.active && !m.closed,
        }).collect())
    }
    
    async fn fetch_prices(&self, market_ids: Vec<String>) -> Result<Vec<PriceData>> {
        let mut prices = Vec::new();
        
        for market_id in market_ids {
            match self.inner.get_market(&market_id).await {
                Ok(market) => {
                    prices.push(PriceData {
                        market_id: market.condition_id.clone(),
                        prices: market.tokens.iter().map(|t| t.price).collect(),
                        timestamp: Utc::now(),
                        volume_24h: market.tokens.iter().map(|t| t.price * 1000.0).sum(),
                        liquidity: market.minimum_order_size * 1000.0,
                    });
                }
                Err(e) => {
                    warn!("Failed to fetch price for market {}: {}", market_id, e);
                }
            }
        }
        
        Ok(prices)
    }
    
    async fn health_check(&self) -> Result<()> {
        // Try to fetch a small number of markets
        self.inner.get_markets(1).await?;
        Ok(())
    }
    
    fn platform(&self) -> Platform {
        Platform::Polymarket
    }
}

/// Kalshi API client adapter
struct KalshiApiClient {
    inner: KalshiClient,
}

impl KalshiApiClient {
    fn new(api_key: Option<String>, api_secret: Option<String>) -> Result<Self> {
        Ok(Self {
            inner: KalshiClient::new(api_key, api_secret)?,
        })
    }
}

#[async_trait::async_trait]
impl ExternalApiClient for KalshiApiClient {
    async fn fetch_markets(&self, limit: usize) -> Result<Vec<MarketData>> {
        let markets = self.inner.get_markets(limit, "active").await?;
        
        Ok(markets.into_iter().map(|m| MarketData {
            id: m.ticker.clone(),
            title: m.title,
            description: Some(m.subtitle),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            end_time: DateTime::parse_from_rfc3339(&m.close_time).ok().map(|dt| dt.with_timezone(&Utc)),
            volume: m.volume_24h as f64,
            liquidity: m.open_interest as f64,
            active: m.status == "active",
        }).collect())
    }
    
    async fn fetch_prices(&self, market_ids: Vec<String>) -> Result<Vec<PriceData>> {
        let mut prices = Vec::new();
        
        for ticker in market_ids {
            match self.inner.get_market(&ticker).await {
                Ok(market) => {
                    let yes_price = market.last_price as f64 / 100.0;
                    let no_price = 1.0 - yes_price;
                    
                    prices.push(PriceData {
                        market_id: market.ticker.clone(),
                        prices: vec![yes_price, no_price],
                        timestamp: Utc::now(),
                        volume_24h: market.volume_24h as f64,
                        liquidity: market.open_interest as f64,
                    });
                }
                Err(e) => {
                    warn!("Failed to fetch price for market {}: {}", ticker, e);
                }
            }
        }
        
        Ok(prices)
    }
    
    async fn health_check(&self) -> Result<()> {
        // Try to fetch a small number of markets
        self.inner.get_markets(1, "active").await?;
        Ok(())
    }
    
    fn platform(&self) -> Platform {
        Platform::Kalshi
    }
}

impl Clone for ExternalApiService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            clients: self.clients.clone(),
            circuit_breakers: self.circuit_breakers.clone(),
            health_status: self.health_status.clone(),
            retry_policy: self.retry_policy.clone(),
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_circuit_breaker() {
        let mut breaker = CircuitBreaker::new(3, Duration::from_secs(10), 2);
        
        // Initial state should be closed
        assert!(matches!(breaker.state, CircuitState::Closed));
        
        // Record failures
        breaker.on_failure();
        breaker.on_failure();
        assert!(matches!(breaker.state, CircuitState::Closed));
        
        // Third failure should open the circuit
        breaker.on_failure();
        assert!(matches!(breaker.state, CircuitState::Open { .. }));
        
        // Success in open state should not change anything
        breaker.on_success();
        assert!(matches!(breaker.state, CircuitState::Open { .. }));
    }
}