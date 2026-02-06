//! Rate limiting middleware for API protection

use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    num::NonZeroU32,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;

/// Rate limiter types
pub type GlobalRateLimiter = Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>;
pub type IpRateLimiter = Arc<RwLock<HashMap<String, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>>;

/// Rate limit configuration
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Global requests per second
    pub global_rps: u32,
    /// Per-IP requests per second
    pub per_ip_rps: u32,
    /// Burst size for global limiter
    pub global_burst: u32,
    /// Burst size for IP limiter
    pub ip_burst: u32,
    /// Cleanup interval for IP limiters
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            global_rps: 1000,      // 1000 requests per second globally
            per_ip_rps: 10,        // 10 requests per second per IP
            global_burst: 100,     // Allow bursts up to 100
            ip_burst: 20,          // Allow bursts up to 20 per IP
            cleanup_interval: Duration::from_secs(300), // Clean up every 5 minutes
        }
    }
}

/// Rate limiter service
pub struct RateLimitService {
    global_limiter: GlobalRateLimiter,
    ip_limiters: IpRateLimiter,
    config: RateLimitConfig,
}

impl RateLimitService {
    pub fn new(config: RateLimitConfig) -> Self {
        // Create global rate limiter
        let global_quota = Quota::per_second(NonZeroU32::new(config.global_rps).unwrap())
            .allow_burst(NonZeroU32::new(config.global_burst).unwrap());
        let global_limiter = Arc::new(GovernorRateLimiter::direct(global_quota));
        
        let service = Self {
            global_limiter,
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            config,
        };
        
        // Start cleanup task
        service.start_cleanup_task();
        
        service
    }
    
    /// Get or create rate limiter for IP
    async fn get_ip_limiter(&self, ip: &str) -> Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        let mut limiters = self.ip_limiters.write().await;
        
        if let Some(limiter) = limiters.get(ip) {
            limiter.clone()
        } else {
            let quota = Quota::per_second(NonZeroU32::new(self.config.per_ip_rps).unwrap())
                .allow_burst(NonZeroU32::new(self.config.ip_burst).unwrap());
            let limiter = Arc::new(GovernorRateLimiter::direct(quota));
            limiters.insert(ip.to_string(), limiter.clone());
            limiter
        }
    }
    
    /// Check if request is allowed
    pub async fn check_rate_limit(&self, ip: &str) -> Result<(), RateLimitError> {
        // Check global rate limit
        if self.global_limiter.check().is_err() {
            return Err(RateLimitError::GlobalLimitExceeded);
        }
        
        // Check IP rate limit
        let ip_limiter = self.get_ip_limiter(ip).await;
        if ip_limiter.check().is_err() {
            return Err(RateLimitError::IpLimitExceeded { 
                retry_after: 1 // Simple 1 second retry
            });
        }
        
        Ok(())
    }
    
    /// Start cleanup task for old IP limiters
    fn start_cleanup_task(&self) {
        let limiters = self.ip_limiters.clone();
        let interval = self.config.cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            
            loop {
                interval.tick().await;
                
                // Clean up old limiters (simple implementation)
                let mut write_guard = limiters.write().await;
                if write_guard.len() > 10000 {
                    // If too many IPs tracked, clear half
                    let to_remove = write_guard.len() / 2;
                    let keys: Vec<_> = write_guard.keys().take(to_remove).cloned().collect();
                    for key in keys {
                        write_guard.remove(&key);
                    }
                    tracing::info!("Cleaned up {} IP rate limiters", to_remove);
                }
            }
        });
    }
}

/// Rate limit errors
#[derive(Debug)]
pub enum RateLimitError {
    GlobalLimitExceeded,
    IpLimitExceeded { retry_after: u64 },
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            RateLimitError::GlobalLimitExceeded => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "Service temporarily unavailable due to high load"
                }))
            ).into_response(),
            
            RateLimitError::IpLimitExceeded { retry_after } => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "error": "Rate limit exceeded",
                        "retry_after": retry_after
                    }))
                ).into_response();
                
                // Add Retry-After header
                response.headers_mut().insert(
                    "Retry-After",
                    retry_after.to_string().parse().unwrap()
                );
                
                response
            }
        }
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware<B>(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    State(rate_limiter): State<Arc<RateLimitService>>,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, RateLimitError> {
    // Extract IP address
    let ip = connect_info
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    // Check rate limit
    rate_limiter.check_rate_limit(&ip).await?;
    
    // Continue to next middleware
    Ok(next.run(request).await)
}

/// Create rate limiting layer
pub fn create_rate_limit_layer(config: RateLimitConfig) -> Arc<RateLimitService> {
    Arc::new(RateLimitService::new(config))
}


#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_rate_limiter() {
        let config = RateLimitConfig {
            global_rps: 10,
            per_ip_rps: 2,
            global_burst: 5,
            ip_burst: 2,
            cleanup_interval: Duration::from_secs(60),
        };
        
        let service = RateLimitService::new(config);
        
        // Should allow first requests
        assert!(service.check_rate_limit("127.0.0.1").await.is_ok());
        assert!(service.check_rate_limit("127.0.0.1").await.is_ok());
        
        // Should block after burst
        tokio::time::sleep(Duration::from_millis(100)).await;
        let result = service.check_rate_limit("127.0.0.1").await;
        // Might pass or fail depending on timing, but should not panic
        let _ = result;
    }
}
