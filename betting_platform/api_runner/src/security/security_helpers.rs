//! Helper functions for security logging integration

use super::security_logger::{
    SecurityLogEntry, SecurityEventType, SecuritySeverity, log_security_event
};
use axum::http::StatusCode;

/// Log a successful login
pub async fn log_login_success(user_id: &str, wallet: Option<&str>, ip: &str) {
    let mut event = SecurityLogEntry::new(
        SecurityEventType::LoginSuccess,
        SecuritySeverity::Info,
    )
    .with_user(user_id.to_string())
    .with_ip(ip.to_string());
    
    if let Some(wallet_addr) = wallet {
        event = event.with_wallet(wallet_addr.to_string());
    }
    
    log_security_event(event).await;
}

/// Log a failed login attempt
pub async fn log_login_failure(wallet: Option<&str>, ip: &str, reason: &str) {
    let mut event = SecurityLogEntry::new(
        SecurityEventType::LoginFailure,
        SecuritySeverity::Low,
    )
    .with_ip(ip.to_string())
    .with_detail("reason", reason);
    
    if let Some(wallet_addr) = wallet {
        event = event.with_wallet(wallet_addr.to_string());
    }
    
    log_security_event(event).await;
}

/// Log rate limit exceeded
pub async fn log_rate_limit_exceeded(ip: &str, endpoint: &str, limit_type: &str) {
    let event = SecurityLogEntry::new(
        SecurityEventType::RateLimitExceeded,
        SecuritySeverity::Medium,
    )
    .with_ip(ip.to_string())
    .with_detail("endpoint", endpoint)
    .with_detail("limit_type", limit_type);
    
    log_security_event(event).await;
}

/// Log SQL injection attempt
pub async fn log_sql_injection_attempt(ip: &str, path: &str, payload: &str) {
    let event = SecurityLogEntry::new(
        SecurityEventType::SqlInjectionAttempt,
        SecuritySeverity::High,
    )
    .with_ip(ip.to_string())
    .with_request_info(path.to_string(), "POST".to_string())
    .with_detail("payload_sample", &payload[..payload.len().min(100)]);
    
    log_security_event(event).await;
}

/// Log XSS attempt
pub async fn log_xss_attempt(ip: &str, path: &str, payload: &str) {
    let event = SecurityLogEntry::new(
        SecurityEventType::XssAttempt,
        SecuritySeverity::High,
    )
    .with_ip(ip.to_string())
    .with_request_info(path.to_string(), "POST".to_string())
    .with_detail("payload_sample", &payload[..payload.len().min(100)]);
    
    log_security_event(event).await;
}

/// Log unauthorized access attempt
pub async fn log_unauthorized_access(ip: &str, path: &str, method: &str) {
    let event = SecurityLogEntry::new(
        SecurityEventType::UnauthorizedAccess,
        SecuritySeverity::Medium,
    )
    .with_ip(ip.to_string())
    .with_request_info(path.to_string(), method.to_string());
    
    log_security_event(event).await;
}

/// Log wallet connection
pub async fn log_wallet_connected(wallet: &str, ip: &str) {
    let event = SecurityLogEntry::new(
        SecurityEventType::WalletConnected,
        SecuritySeverity::Info,
    )
    .with_wallet(wallet.to_string())
    .with_ip(ip.to_string());
    
    log_security_event(event).await;
}

/// Log suspicious transaction
pub async fn log_suspicious_transaction(
    user_id: &str,
    wallet: &str,
    ip: &str,
    amount: u64,
    reason: &str,
) {
    let event = SecurityLogEntry::new(
        SecurityEventType::SuspiciousTransaction,
        SecuritySeverity::High,
    )
    .with_user(user_id.to_string())
    .with_wallet(wallet.to_string())
    .with_ip(ip.to_string())
    .with_detail("amount", amount)
    .with_detail("reason", reason);
    
    log_security_event(event).await;
}

/// Log API response with security context
pub async fn log_api_response(
    path: &str,
    method: &str,
    status: StatusCode,
    ip: &str,
    user_id: Option<&str>,
) {
    // Only log security-relevant status codes
    match status {
        StatusCode::UNAUTHORIZED => {
            log_unauthorized_access(ip, path, method).await;
        }
        StatusCode::FORBIDDEN => {
            let mut event = SecurityLogEntry::new(
                SecurityEventType::ForbiddenAccess,
                SecuritySeverity::Medium,
            )
            .with_ip(ip.to_string())
            .with_request_info(path.to_string(), method.to_string());
            
            if let Some(uid) = user_id {
                event = event.with_user(uid.to_string());
            }
            
            log_security_event(event).await;
        }
        StatusCode::TOO_MANY_REQUESTS => {
            log_rate_limit_exceeded(ip, path, "api_endpoint").await;
        }
        _ => {}
    }
}

/// Log DDoS attempt detected
pub async fn log_ddos_attempt(ip: &str, request_count: u32, time_window: u64) {
    let event = SecurityLogEntry::new(
        SecurityEventType::DdosAttemptDetected,
        SecuritySeverity::Critical,
    )
    .with_ip(ip.to_string())
    .with_detail("request_count", request_count)
    .with_detail("time_window_seconds", time_window);
    
    log_security_event(event).await;
}

/// Log IP blocked
pub async fn log_ip_blocked(ip: &str, duration_seconds: u64, reason: &str) {
    let event = SecurityLogEntry::new(
        SecurityEventType::IpBlocked,
        SecuritySeverity::High,
    )
    .with_ip(ip.to_string())
    .with_detail("block_duration_seconds", duration_seconds)
    .with_detail("reason", reason);
    
    log_security_event(event).await;
}

/// Log sensitive data access
pub async fn log_sensitive_data_access(
    user_id: &str,
    ip: &str,
    data_type: &str,
    action: &str,
) {
    let event = SecurityLogEntry::new(
        SecurityEventType::SensitiveDataAccessed,
        SecuritySeverity::Medium,
    )
    .with_user(user_id.to_string())
    .with_ip(ip.to_string())
    .with_detail("data_type", data_type)
    .with_detail("action", action);
    
    log_security_event(event).await;
}

/// Log bulk data request
pub async fn log_bulk_data_request(
    user_id: Option<&str>,
    ip: &str,
    endpoint: &str,
    record_count: u32,
) {
    let mut event = SecurityLogEntry::new(
        SecurityEventType::BulkDataRequest,
        if record_count > 1000 { SecuritySeverity::Medium } else { SecuritySeverity::Low },
    )
    .with_ip(ip.to_string())
    .with_detail("endpoint", endpoint)
    .with_detail("record_count", record_count);
    
    if let Some(uid) = user_id {
        event = event.with_user(uid.to_string());
    }
    
    log_security_event(event).await;
}