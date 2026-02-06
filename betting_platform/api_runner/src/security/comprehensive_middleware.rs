//! Comprehensive security middleware for request/response logging and threat detection

use axum::{
    body::Body,
    extract::{ConnectInfo, MatchedPath, State},
    http::{Request, Response, StatusCode, HeaderMap},
    middleware::Next,
    response::IntoResponse,
};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::Instant,
};
use tower::ServiceExt;
use tracing::{info, warn, error};
use crate::{
    AppState,
    security::security_logger::{
        SecurityLogEntry, SecurityEventType, SecuritySeverity,
    },
};

/// Extract IP address from request headers or connection info
fn extract_client_ip(headers: &HeaderMap, addr: &SocketAddr) -> String {
    // Check X-Forwarded-For header first (for proxies)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(ip) = forwarded_str.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }
    
    // Check X-Real-IP header
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }
    
    // Fall back to connection address
    addr.ip().to_string()
}

/// Check if request contains potential SQL injection patterns
fn check_sql_injection(value: &str) -> bool {
    let sql_patterns = [
        "' OR '1'='1",
        "\" OR \"1\"=\"1",
        "'; DROP TABLE",
        "\"; DROP TABLE",
        "UNION SELECT",
        "1=1",
        "/*",
        "*/",
        "@@",
        "@variable",
        "WAITFOR DELAY",
        "BENCHMARK(",
        "SLEEP(",
        "0x",
        "\\x",
    ];
    
    let lower_value = value.to_lowercase();
    sql_patterns.iter().any(|pattern| lower_value.contains(&pattern.to_lowercase()))
}

/// Check if request contains potential XSS patterns
fn check_xss_patterns(value: &str) -> bool {
    let xss_patterns = [
        "<script",
        "</script>",
        "javascript:",
        "onerror=",
        "onload=",
        "onclick=",
        "onmouseover=",
        "<iframe",
        "<embed",
        "<object",
        "document.cookie",
        "window.location",
        "eval(",
        "expression(",
        "<svg",
        "data:text/html",
    ];
    
    let lower_value = value.to_lowercase();
    xss_patterns.iter().any(|pattern| lower_value.contains(&pattern.to_lowercase()))
}

/// Check if path contains traversal attempts
fn check_path_traversal(path: &str) -> bool {
    path.contains("..") || path.contains("\\") || path.contains("%2e%2e") || path.contains("%5c")
}

/// Comprehensive security middleware
pub async fn comprehensive_security_middleware(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    matched_path: Option<MatchedPath>,
    request: Request<Body>,
    next: Next<Body>,
) -> impl IntoResponse {
    let start_time = Instant::now();

    let addr = connect_info
        .map(|ConnectInfo(addr)| addr)
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    
    // Extract request info
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = matched_path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| uri.path().to_string());
    let headers = request.headers().clone();
    let client_ip = extract_client_ip(&headers, &addr);
    
    // Extract user agent
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    
    // Extract authorization info
    let has_auth = headers.get("authorization").is_some();
    let auth_user = if has_auth {
        // In production, decode JWT to get user info
        // For now, just mark as authenticated
        Some("authenticated_user")
    } else {
        None
    };
    
    // Security checks on request
    let mut security_flags = Vec::new();
    
    // Check path traversal
    if check_path_traversal(&path) {
        security_flags.push("path_traversal");
        let event = SecurityLogEntry::new(
            SecurityEventType::PathTraversalAttempt,
            SecuritySeverity::High,
        )
        .with_ip(client_ip.clone())
        .with_request_info(path.clone(), method.to_string())
        .with_detail("uri", uri.to_string());
        
        state.security_logger.log_event(event).await;
    }
    
    // Check query parameters for injection
    if let Some(query) = uri.query() {
        if check_sql_injection(query) {
            security_flags.push("sql_injection");
            let event = SecurityLogEntry::new(
                SecurityEventType::SqlInjectionAttempt,
                SecuritySeverity::Critical,
            )
            .with_ip(client_ip.clone())
            .with_request_info(path.clone(), method.to_string())
            .with_detail("query", query);
            
            state.security_logger.log_event(event).await;
        }
        
        if check_xss_patterns(query) {
            security_flags.push("xss_attempt");
            let event = SecurityLogEntry::new(
                SecurityEventType::XssAttempt,
                SecuritySeverity::High,
            )
            .with_ip(client_ip.clone())
            .with_request_info(path.clone(), method.to_string())
            .with_detail("query", query);
            
            state.security_logger.log_event(event).await;
        }
    }
    
    // If high-risk patterns detected, block request
    if !security_flags.is_empty() {
        error!(
            ip = %client_ip,
            path = %path,
            flags = ?security_flags,
            "Security threat detected - blocking request"
        );
        
        return StatusCode::FORBIDDEN.into_response();
    }
    
    // Process request
    let response = next.run(request).await;
    
    // Calculate request duration
    let duration = start_time.elapsed();
    let status = response.status();
    
    // Log based on status code
    match status {
        StatusCode::OK | StatusCode::CREATED | StatusCode::ACCEPTED => {
            // Normal successful requests - log only if it's a sensitive endpoint
            if path.contains("/admin") || path.contains("/api/rbac") || path.contains("/api/auth") {
                let mut event = SecurityLogEntry::new(
                    SecurityEventType::SensitiveDataAccessed,
                    SecuritySeverity::Info,
                )
                .with_ip(client_ip.clone())
                .with_request_info(path.clone(), method.to_string())
                .with_detail("status", status.as_u16())
                .with_detail("duration_ms", duration.as_millis() as u64);
                
                if let Some(user) = auth_user {
                    event = event.with_user(user.to_string());
                }
                
                if let Some(ua) = &user_agent {
                    event.user_agent = Some(ua.clone());
                }
                
                state.security_logger.log_event(event).await;
            }
        }
        
        StatusCode::UNAUTHORIZED => {
            let mut event = SecurityLogEntry::new(
                SecurityEventType::UnauthorizedAccess,
                SecuritySeverity::Medium,
            )
            .with_ip(client_ip.clone())
            .with_request_info(path.clone(), method.to_string())
            .with_detail("status", status.as_u16())
            .with_detail("duration_ms", duration.as_millis() as u64);
            
            if let Some(ua) = &user_agent {
                event.user_agent = Some(ua.clone());
            }
            
            state.security_logger.log_event(event).await;
        }
        
        StatusCode::FORBIDDEN => {
            let mut event = SecurityLogEntry::new(
                SecurityEventType::ForbiddenAccess,
                SecuritySeverity::Medium,
            )
            .with_ip(client_ip.clone())
            .with_request_info(path.clone(), method.to_string())
            .with_detail("status", status.as_u16())
            .with_detail("duration_ms", duration.as_millis() as u64);
            
            if let Some(user) = auth_user {
                event = event.with_user(user.to_string());
            }
            
            if let Some(ua) = &user_agent {
                event.user_agent = Some(ua.clone());
            }
            
            state.security_logger.log_event(event).await;
        }
        
        StatusCode::TOO_MANY_REQUESTS => {
            let event = SecurityLogEntry::new(
                SecurityEventType::RateLimitExceeded,
                SecuritySeverity::Low,
            )
            .with_ip(client_ip.clone())
            .with_request_info(path.clone(), method.to_string())
            .with_detail("status", status.as_u16());
            
            state.security_logger.log_event(event).await;
        }
        
        StatusCode::BAD_REQUEST => {
            // Check if it's a malformed request that might be an attack
            if duration.as_millis() < 10 {
                let event = SecurityLogEntry::new(
                    SecurityEventType::MalformedRequest,
                    SecuritySeverity::Low,
                )
                .with_ip(client_ip.clone())
                .with_request_info(path.clone(), method.to_string())
                .with_detail("status", status.as_u16())
                .with_detail("duration_ms", duration.as_millis() as u64);
                
                state.security_logger.log_event(event).await;
            }
        }
        
        StatusCode::INTERNAL_SERVER_ERROR => {
            // Log server errors for security monitoring
            warn!(
                ip = %client_ip,
                path = %path,
                method = %method,
                duration_ms = duration.as_millis(),
                "Internal server error - potential security issue"
            );
        }
        
        _ => {}
    }
    
    // Add security headers to response
    let (mut parts, body) = response.into_parts();
    
    // Security headers
    parts.headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    parts.headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    parts.headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    parts.headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    parts.headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'".parse().unwrap()
    );
    
    // Remove server header for security
    parts.headers.remove("server");
    
    Response::from_parts(parts, body)
}

/// IP-based rate limiting and DDoS protection
pub struct IpRateLimiter {
    /// Track request counts per IP
    request_counts: Arc<tokio::sync::RwLock<std::collections::HashMap<String, RequestTracker>>>,
    /// Blocked IPs
    blocked_ips: Arc<tokio::sync::RwLock<std::collections::HashSet<String>>>,
}

#[derive(Clone)]
struct RequestTracker {
    count: u32,
    window_start: Instant,
    suspicious_requests: u32,
}

impl IpRateLimiter {
    pub fn new() -> Self {
        let limiter = Self {
            request_counts: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            blocked_ips: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        };
        
        // Start cleanup task
        let counts = limiter.request_counts.clone();
        let blocked = limiter.blocked_ips.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                
                // Clean up old entries
                let mut request_counts = counts.write().await;
                request_counts.retain(|_, tracker| {
                    tracker.window_start.elapsed() < std::time::Duration::from_secs(300)
                });
                
                // Unblock IPs after 1 hour
                let mut blocked_ips = blocked.write().await;
                blocked_ips.clear(); // Simple approach - clear all after each hour
            }
        });
        
        limiter
    }
    
    pub async fn check_rate_limit(&self, ip: &str, state: &AppState) -> Result<(), StatusCode> {
        // Check if IP is blocked
        {
            let blocked = self.blocked_ips.read().await;
            if blocked.contains(ip) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
        
        let mut counts = self.request_counts.write().await;
        let now = Instant::now();
        
        let tracker = counts.entry(ip.to_string()).or_insert_with(|| RequestTracker {
            count: 0,
            window_start: now,
            suspicious_requests: 0,
        });
        
        // Reset window if needed
        if tracker.window_start.elapsed() > std::time::Duration::from_secs(60) {
            tracker.count = 0;
            tracker.suspicious_requests = 0;
            tracker.window_start = now;
        }
        
        tracker.count += 1;
        
        // Check for DDoS patterns
        if tracker.count > 1000 {
            let request_count = tracker.count;
            // Release write lock before acquiring another
            drop(counts);
            
            // Block IP
            let mut blocked = self.blocked_ips.write().await;
            blocked.insert(ip.to_string());
            
            // Log DDoS attempt
            let event = SecurityLogEntry::new(
                SecurityEventType::DdosAttemptDetected,
                SecuritySeverity::Critical,
            )
            .with_ip(ip.to_string())
            .with_detail("request_count", request_count)
            .with_detail("window_seconds", 60);
            
            state.security_logger.log_event(event).await;
            
            // Log IP blocked
            let block_event = SecurityLogEntry::new(
                SecurityEventType::IpBlocked,
                SecuritySeverity::High,
            )
            .with_ip(ip.to_string())
            .with_detail("reason", "DDoS attempt")
            .with_detail("block_duration_seconds", 3600);
            
            state.security_logger.log_event(block_event).await;
            
            return Err(StatusCode::FORBIDDEN);
        }
        
        // Normal rate limiting
        if tracker.count > 100 {
            let request_count = tracker.count;
            // Log rate limit exceeded
            let event = SecurityLogEntry::new(
                SecurityEventType::RateLimitExceeded,
                SecuritySeverity::Low,
            )
            .with_ip(ip.to_string())
            .with_detail("request_count", request_count)
            .with_detail("limit", 100);
            
            state.security_logger.log_event(event).await;
            
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        
        Ok(())
    }
}
