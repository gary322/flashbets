//! Comprehensive rate limiting for production use

use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode, HeaderMap, header},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use serde::{Serialize, Deserialize};
use std::{
    collections::HashMap,
    net::{SocketAddr, IpAddr},
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use redis::{AsyncCommands, Client};

/// Rate limit types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitType {
    Global,
    PerIp,
    PerUser,
    PerEndpoint,
    PerApiKey,
}

/// Rate limit tier for different user types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RateLimitTier {
    Free,
    Basic,
    Pro,
    Enterprise,
    Internal,
}

impl RateLimitTier {
    pub fn get_multiplier(&self) -> f64 {
        match self {
            RateLimitTier::Free => 1.0,
            RateLimitTier::Basic => 2.0,
            RateLimitTier::Pro => 5.0,
            RateLimitTier::Enterprise => 10.0,
            RateLimitTier::Internal => 100.0,
        }
    }
}

/// Enhanced rate limit configuration
#[derive(Clone)]
pub struct EnhancedRateLimitConfig {
    // Global limits
    pub global_rps: u32,
    pub global_burst: u32,
    
    // Per-IP limits
    pub per_ip_rps: u32,
    pub per_ip_burst: u32,
    
    // Per-user limits (authenticated)
    pub per_user_rps: u32,
    pub per_user_burst: u32,
    
    // Endpoint-specific limits
    pub endpoint_limits: HashMap<String, EndpointLimit>,
    
    // DDoS protection
    pub ddos_threshold: u32,
    pub ddos_ban_duration: Duration,
    
    // Redis configuration for distributed rate limiting
    pub redis_url: Option<String>,
    pub sync_interval: Duration,
}

#[derive(Clone)]
pub struct EndpointLimit {
    pub path_pattern: String,
    pub rps: u32,
    pub burst: u32,
    pub cost: u32, // Cost multiplier for expensive operations
}

impl Default for EnhancedRateLimitConfig {
    fn default() -> Self {
        let mut endpoint_limits = HashMap::new();
        
        // Critical endpoints with lower limits
        endpoint_limits.insert("/api/auth/login".to_string(), EndpointLimit {
            path_pattern: "/api/auth/login".to_string(),
            rps: 5,
            burst: 10,
            cost: 5,
        });
        
        endpoint_limits.insert("/api/trading/place".to_string(), EndpointLimit {
            path_pattern: "/api/trading/place".to_string(),
            rps: 10,
            burst: 20,
            cost: 3,
        });
        
        endpoint_limits.insert("/api/quantum/create".to_string(), EndpointLimit {
            path_pattern: "/api/quantum/create".to_string(),
            rps: 5,
            burst: 10,
            cost: 10, // Expensive operation
        });
        
        Self {
            global_rps: 10000,
            global_burst: 1000,
            per_ip_rps: 100,
            per_ip_burst: 200,
            per_user_rps: 50,
            per_user_burst: 100,
            endpoint_limits,
            ddos_threshold: 1000,
            ddos_ban_duration: Duration::from_secs(3600), // 1 hour ban
            redis_url: None,
            sync_interval: Duration::from_secs(10),
        }
    }
}

/// Rate limit state
pub struct RateLimitState {
    pub count: u32,
    pub reset_at: Instant,
    pub tier: RateLimitTier,
    pub blocked_until: Option<Instant>,
}

/// Enhanced rate limiter
pub struct EnhancedRateLimiter {
    // In-memory limiters
    global_limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    ip_limiters: Arc<RwLock<HashMap<IpAddr, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>>,
    user_limiters: Arc<RwLock<HashMap<String, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>>,
    endpoint_limiters: Arc<RwLock<HashMap<String, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>>,
    
    // DDoS protection
    ddos_tracker: Arc<RwLock<HashMap<IpAddr, RateLimitState>>>,
    blocked_ips: Arc<RwLock<HashMap<IpAddr, Instant>>>,
    
    // Redis client for distributed limiting
    redis_client: Option<Arc<Client>>,
    
    config: EnhancedRateLimitConfig,
}

impl EnhancedRateLimiter {
    pub async fn new(config: EnhancedRateLimitConfig) -> Self {
        let global_limiter = Arc::new(
            GovernorRateLimiter::direct(
                Quota::per_second(NonZeroU32::new(config.global_rps).unwrap())
                    .allow_burst(NonZeroU32::new(config.global_burst).unwrap())
            )
        );
        
        let redis_client = if let Some(redis_url) = &config.redis_url {
            match Client::open(redis_url.as_str()) {
                Ok(client) => Some(Arc::new(client)),
                Err(e) => {
                    eprintln!("Failed to connect to Redis for rate limiting: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let limiter = Self {
            global_limiter,
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            endpoint_limiters: Arc::new(RwLock::new(HashMap::new())),
            ddos_tracker: Arc::new(RwLock::new(HashMap::new())),
            blocked_ips: Arc::new(RwLock::new(HashMap::new())),
            redis_client,
            config,
        };
        
        // Start cleanup task
        limiter.start_cleanup_task();
        
        limiter
    }
    
    /// Check if request should be rate limited
    pub async fn check_rate_limit(
        &self,
        ip: IpAddr,
        user_id: Option<&str>,
        endpoint: &str,
        tier: RateLimitTier,
    ) -> Result<(), RateLimitError> {
        // Check if IP is blocked
        if self.is_ip_blocked(ip).await {
            return Err(RateLimitError::Blocked);
        }
        
        // Check global rate limit
        if let Err(_) = self.global_limiter.check() {
            return Err(RateLimitError::GlobalLimit);
        }
        
        // Check IP rate limit
        self.check_ip_limit(ip).await?;
        
        // Check user rate limit if authenticated
        if let Some(user_id) = user_id {
            self.check_user_limit(user_id, tier).await?;
        }
        
        // Check endpoint-specific limit
        self.check_endpoint_limit(endpoint).await?;
        
        // Update DDoS tracker
        self.update_ddos_tracker(ip).await;
        
        Ok(())
    }
    
    async fn is_ip_blocked(&self, ip: IpAddr) -> bool {
        let blocked = self.blocked_ips.read().await;
        if let Some(blocked_until) = blocked.get(&ip) {
            if Instant::now() < *blocked_until {
                return true;
            }
        }
        false
    }
    
    async fn check_ip_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let mut limiters = self.ip_limiters.write().await;
        
        let limiter = limiters.entry(ip).or_insert_with(|| {
            Arc::new(GovernorRateLimiter::direct(
                Quota::per_second(NonZeroU32::new(self.config.per_ip_rps).unwrap())
                    .allow_burst(NonZeroU32::new(self.config.per_ip_burst).unwrap())
            ))
        });
        
        limiter.check().map_err(|_| RateLimitError::IpLimit)?;
        Ok(())
    }
    
    async fn check_user_limit(&self, user_id: &str, tier: RateLimitTier) -> Result<(), RateLimitError> {
        let mut limiters = self.user_limiters.write().await;
        
        let multiplier = tier.get_multiplier();
        let user_rps = (self.config.per_user_rps as f64 * multiplier) as u32;
        let user_burst = (self.config.per_user_burst as f64 * multiplier) as u32;
        
        let limiter = limiters.entry(user_id.to_string()).or_insert_with(|| {
            Arc::new(GovernorRateLimiter::direct(
                Quota::per_second(NonZeroU32::new(user_rps).unwrap())
                    .allow_burst(NonZeroU32::new(user_burst).unwrap())
            ))
        });
        
        limiter.check().map_err(|_| RateLimitError::UserLimit)?;
        Ok(())
    }
    
    async fn check_endpoint_limit(&self, endpoint: &str) -> Result<(), RateLimitError> {
        if let Some(endpoint_config) = self.config.endpoint_limits.get(endpoint) {
            let mut limiters = self.endpoint_limiters.write().await;
            
            let limiter = limiters.entry(endpoint.to_string()).or_insert_with(|| {
                Arc::new(GovernorRateLimiter::direct(
                    Quota::per_second(NonZeroU32::new(endpoint_config.rps).unwrap())
                        .allow_burst(NonZeroU32::new(endpoint_config.burst).unwrap())
                ))
            });
            
            // Check multiple times based on cost
            for _ in 0..endpoint_config.cost {
                limiter.check().map_err(|_| RateLimitError::EndpointLimit)?;
            }
        }
        
        Ok(())
    }
    
    async fn update_ddos_tracker(&self, ip: IpAddr) {
        let mut tracker = self.ddos_tracker.write().await;
        let now = Instant::now();
        
        let state = tracker.entry(ip).or_insert(RateLimitState {
            count: 0,
            reset_at: now + Duration::from_secs(60),
            tier: RateLimitTier::Free,
            blocked_until: None,
        });
        
        if now >= state.reset_at {
            state.count = 1;
            state.reset_at = now + Duration::from_secs(60);
        } else {
            state.count += 1;
            
            // Check for DDoS
            if state.count > self.config.ddos_threshold {
                let mut blocked = self.blocked_ips.write().await;
                blocked.insert(ip, now + self.config.ddos_ban_duration);
                
                println!("DDoS protection: Blocked IP {} for {:?}", ip, self.config.ddos_ban_duration);
            }
        }
    }
    
    fn start_cleanup_task(&self) {
        let ip_limiters = self.ip_limiters.clone();
        let user_limiters = self.user_limiters.clone();
        let endpoint_limiters = self.endpoint_limiters.clone();
        let ddos_tracker = self.ddos_tracker.clone();
        let blocked_ips = self.blocked_ips.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                // Clean up old entries
                let now = Instant::now();
                
                // Clean blocked IPs
                let mut blocked = blocked_ips.write().await;
                blocked.retain(|_, blocked_until| now < *blocked_until);
                
                // Clean DDoS tracker
                let mut tracker = ddos_tracker.write().await;
                tracker.retain(|_, state| now < state.reset_at);
                
                // Optionally clean up limiters (if memory is a concern)
                // This is more complex as we need to track last usage
            }
        });
    }
}

#[derive(Debug)]
pub enum RateLimitError {
    GlobalLimit,
    IpLimit,
    UserLimit,
    EndpointLimit,
    Blocked,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::GlobalLimit => write!(f, "Global rate limit exceeded"),
            RateLimitError::IpLimit => write!(f, "IP rate limit exceeded"),
            RateLimitError::UserLimit => write!(f, "User rate limit exceeded"),
            RateLimitError::EndpointLimit => write!(f, "Endpoint rate limit exceeded"),
            RateLimitError::Blocked => write!(f, "IP temporarily blocked due to excessive requests"),
        }
    }
}

/// Rate limit middleware
pub async fn rate_limit_middleware<B>(
    State(limiter): State<Arc<EnhancedRateLimiter>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<B>,
    next: Next<B>,
) -> Response {
    let ip = addr.ip();
    let path = request.uri().path().to_string();
    
    // Extract user ID from JWT if present
    let user_id = extract_user_id_from_request(&request);
    let tier = get_user_tier(&user_id);
    
    match limiter.check_rate_limit(ip, user_id.as_deref(), &path, tier).await {
        Ok(()) => next.run(request).await,
        Err(e) => {
            let mut headers = HeaderMap::new();
            headers.insert("X-RateLimit-Limit", "100".parse().unwrap());
            headers.insert("X-RateLimit-Remaining", "0".parse().unwrap());
            headers.insert("X-RateLimit-Reset", "60".parse().unwrap());
            headers.insert("Retry-After", "60".parse().unwrap());
            
            (
                StatusCode::TOO_MANY_REQUESTS,
                headers,
                Json(serde_json::json!({
                    "error": e.to_string(),
                    "retry_after": 60,
                }))
            ).into_response()
        }
    }
}

fn extract_user_id_from_request<B>(request: &Request<B>) -> Option<String> {
    // Extract from JWT token in Authorization header
    request.headers()
        .get(header::AUTHORIZATION)
        .and_then(|auth| auth.to_str().ok())
        .and_then(|auth| {
            if auth.starts_with("Bearer ") {
                // In production, decode JWT to get user ID
                // For now, return None
                None
            } else {
                None
            }
        })
}

fn get_user_tier(user_id: &Option<String>) -> RateLimitTier {
    // In production, look up user tier from database
    // For now, return Free tier
    RateLimitTier::Free
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::time::{sleep, Duration as TokioDuration};
    
    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let config = EnhancedRateLimitConfig::default();
        let limiter = EnhancedRateLimiter::new(config).await;
        
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        
        // Should allow initial requests
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_ip_rate_limiting() {
        let mut config = EnhancedRateLimitConfig::default();
        config.per_ip_rps = 2;
        config.per_ip_burst = 2;
        
        let limiter = EnhancedRateLimiter::new(config).await;
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Should allow burst
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
        
        // Should be rate limited
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_err());
    }
    
    #[tokio::test]
    async fn test_tier_multipliers() {
        assert_eq!(RateLimitTier::Free.get_multiplier(), 1.0);
        assert_eq!(RateLimitTier::Basic.get_multiplier(), 2.0);
        assert_eq!(RateLimitTier::Pro.get_multiplier(), 5.0);
        assert_eq!(RateLimitTier::Enterprise.get_multiplier(), 10.0);
        assert_eq!(RateLimitTier::Internal.get_multiplier(), 100.0);
    }
    
    #[tokio::test]
    async fn test_endpoint_specific_limits() {
        let config = EnhancedRateLimitConfig::default();
        let limiter = EnhancedRateLimiter::new(config).await;
        let ip = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));
        
        // Login endpoint has stricter limits (5 rps, burst 10)
        let mut success_count = 0;
        for i in 0..15 {
            match limiter.check_rate_limit(ip, None, "/api/auth/login", RateLimitTier::Free).await {
                Ok(()) => success_count += 1,
                Err(_) => {
                    println!("Login endpoint blocked after {} requests", i);
                    break;
                }
            }
        }
        
        // Should allow some requests but not all
        assert!(success_count > 0 && success_count < 15);
    }
    
    #[tokio::test]
    async fn test_ddos_protection() {
        let mut config = EnhancedRateLimitConfig::default();
        config.ddos_threshold = 10;
        config.ddos_ban_duration = Duration::from_secs(1);
        
        let limiter = EnhancedRateLimiter::new(config).await;
        let attacker_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1));
        
        // Simulate DDoS attack
        for _ in 0..15 {
            let _ = limiter.check_rate_limit(
                attacker_ip, 
                None, 
                "/api/test", 
                RateLimitTier::Free
            ).await;
        }
        
        // IP should now be blocked
        sleep(TokioDuration::from_millis(100)).await;
        match limiter.check_rate_limit(attacker_ip, None, "/api/test", RateLimitTier::Free).await {
            Err(RateLimitError::Blocked) => {
                println!("IP correctly blocked for DDoS");
            }
            _ => panic!("IP should have been blocked"),
        }
        
        // Wait for ban to expire
        sleep(TokioDuration::from_secs(1)).await;
        
        // Should be unblocked now
        assert!(limiter.check_rate_limit(attacker_ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
    }
}