//! Security logging framework for production use

use axum::{
    extract::{ConnectInfo, MatchedPath},
    http::{Request, Response, StatusCode},
    middleware::Next,
};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    sync::RwLock,
};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Security event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityEventType {
    // Authentication events
    LoginAttempt,
    LoginSuccess,
    LoginFailure,
    LogoutSuccess,
    TokenRefresh,
    TokenExpired,
    InvalidToken,
    
    // Authorization events
    UnauthorizedAccess,
    ForbiddenAccess,
    ElevatedPrivilegeUsed,
    
    // Rate limiting events
    RateLimitExceeded,
    DdosAttemptDetected,
    IpBlocked,
    IpUnblocked,
    
    // Input validation events
    SqlInjectionAttempt,
    XssAttempt,
    PathTraversalAttempt,
    InvalidInputRejected,
    
    // API security events
    InvalidApiKey,
    ApiKeyRevoked,
    SuspiciousRequest,
    MalformedRequest,
    
    // Data security events
    SensitiveDataAccessed,
    DataExportAttempt,
    BulkDataRequest,
    
    // System security events
    SecurityConfigChanged,
    AuditLogAccessed,
    SystemIntegrityCheck,
    
    // Wallet/crypto events
    WalletConnected,
    WalletDisconnected,
    SignatureVerificationFailed,
    TransactionSigned,
    SuspiciousTransaction,
}

/// Security event severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SecuritySeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Security log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityLogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: SecurityEventType,
    pub severity: SecuritySeverity,
    pub ip_address: Option<String>,
    pub user_id: Option<String>,
    pub wallet_address: Option<String>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
    pub status_code: Option<u16>,
    pub user_agent: Option<String>,
    pub details: HashMap<String, serde_json::Value>,
    pub risk_score: f32,
    pub flagged: bool,
}

impl SecurityLogEntry {
    pub fn new(event_type: SecurityEventType, severity: SecuritySeverity) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type,
            severity,
            ip_address: None,
            user_id: None,
            wallet_address: None,
            request_path: None,
            request_method: None,
            status_code: None,
            user_agent: None,
            details: HashMap::new(),
            risk_score: 0.0,
            flagged: false,
        }
    }
    
    pub fn with_ip(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }
    
    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    pub fn with_wallet(mut self, wallet: String) -> Self {
        self.wallet_address = Some(wallet);
        self
    }
    
    pub fn with_request_info(mut self, path: String, method: String) -> Self {
        self.request_path = Some(path);
        self.request_method = Some(method);
        self
    }
    
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.details.insert(key.into(), serde_json::to_value(value).unwrap_or(serde_json::Value::Null));
        self
    }
    
    pub fn calculate_risk_score(&mut self) {
        let mut score: f32 = 0.0;
        
        // Base score by severity
        score += match self.severity {
            SecuritySeverity::Info => 0.0,
            SecuritySeverity::Low => 0.2,
            SecuritySeverity::Medium => 0.4,
            SecuritySeverity::High => 0.7,
            SecuritySeverity::Critical => 1.0,
        };
        
        // Additional score by event type
        score += match &self.event_type {
            SecurityEventType::SqlInjectionAttempt |
            SecurityEventType::XssAttempt |
            SecurityEventType::PathTraversalAttempt => 0.8,
            
            SecurityEventType::DdosAttemptDetected |
            SecurityEventType::SuspiciousTransaction => 0.9,
            
            SecurityEventType::UnauthorizedAccess |
            SecurityEventType::InvalidToken => 0.5,
            
            SecurityEventType::LoginFailure => 0.3,
            
            _ => 0.1,
        };
        
        self.risk_score = score.min(1.0);
        self.flagged = score >= 0.7;
    }
}

/// Security logger configuration
#[derive(Clone)]
pub struct SecurityLoggerConfig {
    /// Log file path
    pub log_file_path: String,
    
    /// Maximum log file size in bytes
    pub max_file_size: u64,
    
    /// Log rotation enabled
    pub rotation_enabled: bool,
    
    /// Days to retain logs
    pub retention_days: u32,
    
    /// Enable real-time alerts
    pub alerts_enabled: bool,
    
    /// Alert threshold (risk score)
    pub alert_threshold: f32,
    
    /// Rate limit for similar events (per minute)
    pub event_rate_limit: u32,
}

impl Default for SecurityLoggerConfig {
    fn default() -> Self {
        Self {
            log_file_path: "logs/security.log".to_string(),
            max_file_size: 100 * 1024 * 1024, // 100MB
            rotation_enabled: true,
            retention_days: 90,
            alerts_enabled: true,
            alert_threshold: 0.7,
            event_rate_limit: 100,
        }
    }
}

/// Security event aggregator for pattern detection
struct EventAggregator {
    events: Vec<SecurityLogEntry>,
    ip_counts: HashMap<String, u32>,
    user_counts: HashMap<String, u32>,
    event_type_counts: HashMap<SecurityEventType, u32>,
    window_start: Instant,
}

impl EventAggregator {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            ip_counts: HashMap::new(),
            user_counts: HashMap::new(),
            event_type_counts: HashMap::new(),
            window_start: Instant::now(),
        }
    }
    
    fn add_event(&mut self, event: &SecurityLogEntry) {
        self.events.push(event.clone());
        
        if let Some(ip) = &event.ip_address {
            *self.ip_counts.entry(ip.clone()).or_insert(0) += 1;
        }
        
        if let Some(user) = &event.user_id {
            *self.user_counts.entry(user.clone()).or_insert(0) += 1;
        }
        
        *self.event_type_counts.entry(event.event_type.clone()).or_insert(0) += 1;
    }
    
    fn detect_patterns(&self) -> Vec<SecurityAlert> {
        let mut alerts = Vec::new();
        let window_duration = Instant::now().duration_since(self.window_start);
        
        // Detect rapid login failures
        if let Some(count) = self.event_type_counts.get(&SecurityEventType::LoginFailure) {
            if *count > 10 && window_duration < Duration::from_secs(60) {
                alerts.push(SecurityAlert {
                    alert_type: "Brute Force Attempt".to_string(),
                    severity: SecuritySeverity::High,
                    details: format!("{} login failures in {:?}", count, window_duration),
                });
            }
        }
        
        // Detect concentrated attacks from single IP
        for (ip, count) in &self.ip_counts {
            if *count > 50 && window_duration < Duration::from_secs(60) {
                alerts.push(SecurityAlert {
                    alert_type: "Suspicious IP Activity".to_string(),
                    severity: SecuritySeverity::High,
                    details: format!("IP {} made {} requests in {:?}", ip, count, window_duration),
                });
            }
        }
        
        // Detect injection attempt patterns
        let injection_events = [
            SecurityEventType::SqlInjectionAttempt,
            SecurityEventType::XssAttempt,
            SecurityEventType::PathTraversalAttempt,
        ];
        
        let injection_count: u32 = injection_events.iter()
            .filter_map(|e| self.event_type_counts.get(e))
            .sum();
        
        if injection_count > 5 {
            alerts.push(SecurityAlert {
                alert_type: "Multiple Injection Attempts".to_string(),
                severity: SecuritySeverity::Critical,
                details: format!("{} injection attempts detected", injection_count),
            });
        }
        
        alerts
    }
    
    fn should_reset(&self) -> bool {
        Instant::now().duration_since(self.window_start) > Duration::from_secs(300) // 5 minutes
    }
}

#[derive(Debug)]
struct SecurityAlert {
    alert_type: String,
    severity: SecuritySeverity,
    details: String,
}

/// Security logger service
pub struct SecurityLogger {
    config: SecurityLoggerConfig,
    aggregator: Arc<RwLock<EventAggregator>>,
    file_lock: Arc<RwLock<()>>,
}

impl SecurityLogger {
    pub fn new(config: SecurityLoggerConfig) -> Self {
        Self {
            config,
            aggregator: Arc::new(RwLock::new(EventAggregator::new())),
            file_lock: Arc::new(RwLock::new(())),
        }
    }
    
    /// Log an authentication-related event
    pub async fn log_auth_event(&self, wallet: &str, event_type: &str, details: Option<&str>) {
        let security_event_type = match event_type {
            "login_success" => SecurityEventType::LoginSuccess,
            "login_failure" => SecurityEventType::LoginFailure,
            "logout" => SecurityEventType::LogoutSuccess,
            "token_refresh" => SecurityEventType::TokenRefresh,
            "role_updated" => SecurityEventType::ElevatedPrivilegeUsed,
            "market_created" => SecurityEventType::SensitiveDataAccessed,
            "system_config_updated" => SecurityEventType::SecurityConfigChanged,
            "permission_granted" => SecurityEventType::ElevatedPrivilegeUsed,
            _ => SecurityEventType::SensitiveDataAccessed,
        };
        
        let severity = match event_type {
            "login_success" | "logout" => SecuritySeverity::Info,
            "login_failure" => SecuritySeverity::Low,
            "token_refresh" => SecuritySeverity::Info,
            "role_updated" | "permission_granted" => SecuritySeverity::Medium,
            "system_config_updated" => SecuritySeverity::High,
            _ => SecuritySeverity::Low,
        };
        
        let mut event = SecurityLogEntry::new(security_event_type, severity)
            .with_wallet(wallet.to_string())
            .with_detail("event_type", event_type);
        
        if let Some(details_str) = details {
            event = event.with_detail("details", details_str);
        }
        
        self.log_event(event).await;
    }
    
    /// Log a security event
    pub async fn log_event(&self, mut event: SecurityLogEntry) {
        // Calculate risk score
        event.calculate_risk_score();
        
        // Add to aggregator
        {
            let mut aggregator = self.aggregator.write().await;
            aggregator.add_event(&event);
            
            // Check if we need to reset the window
            if aggregator.should_reset() {
                *aggregator = EventAggregator::new();
            }
            
            // Detect patterns and generate alerts
            if self.config.alerts_enabled {
                let alerts = aggregator.detect_patterns();
                for alert in alerts {
                    self.send_alert(&alert).await;
                }
            }
        }
        
        // Write to log file
        if let Err(e) = self.write_to_file(&event).await {
            error!("Failed to write security log: {}", e);
        }
        
        // Send alert if needed
        if self.config.alerts_enabled && event.risk_score >= self.config.alert_threshold {
            self.send_event_alert(&event).await;
        }
        
        // Log to tracing
        match event.severity {
            SecuritySeverity::Critical | SecuritySeverity::High => {
                error!(
                    event_type = ?event.event_type,
                    severity = ?event.severity,
                    risk_score = event.risk_score,
                    "Security event: {}",
                    serde_json::to_string(&event).unwrap_or_default()
                );
            }
            SecuritySeverity::Medium => {
                warn!(
                    event_type = ?event.event_type,
                    severity = ?event.severity,
                    risk_score = event.risk_score,
                    "Security event: {}",
                    serde_json::to_string(&event).unwrap_or_default()
                );
            }
            _ => {
                info!(
                    event_type = ?event.event_type,
                    severity = ?event.severity,
                    risk_score = event.risk_score,
                    "Security event: {}",
                    serde_json::to_string(&event).unwrap_or_default()
                );
            }
        }
    }
    
    async fn write_to_file(&self, event: &SecurityLogEntry) -> Result<(), std::io::Error> {
        let _lock = self.file_lock.write().await;
        
        // Create directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(&self.config.log_file_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Check file size and rotate if needed
        if self.config.rotation_enabled {
            if let Ok(metadata) = tokio::fs::metadata(&self.config.log_file_path).await {
                if metadata.len() > self.config.max_file_size {
                    self.rotate_log_file().await?;
                }
            }
        }
        
        // Write event
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_file_path)
            .await?;
        
        let log_line = format!("{}\n", serde_json::to_string(event)?);
        file.write_all(log_line.as_bytes()).await?;
        file.flush().await?;
        
        Ok(())
    }
    
    async fn rotate_log_file(&self) -> Result<(), std::io::Error> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let rotated_path = format!("{}.{}", self.config.log_file_path, timestamp);
        tokio::fs::rename(&self.config.log_file_path, rotated_path).await?;
        Ok(())
    }
    
    async fn send_alert(&self, alert: &SecurityAlert) {
        // In production, this would send to monitoring service
        error!(
            alert_type = %alert.alert_type,
            severity = ?alert.severity,
            details = %alert.details,
            "SECURITY ALERT"
        );
    }
    
    async fn send_event_alert(&self, event: &SecurityLogEntry) {
        // In production, this would send to monitoring service
        error!(
            event_id = %event.id,
            event_type = ?event.event_type,
            risk_score = event.risk_score,
            "HIGH RISK SECURITY EVENT"
        );
    }
    
    /// Get recent security events
    pub async fn get_recent_events(&self, limit: usize) -> Vec<SecurityLogEntry> {
        let aggregator = self.aggregator.read().await;
        aggregator.events.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get security statistics
    pub async fn get_statistics(&self) -> SecurityStatistics {
        let aggregator = self.aggregator.read().await;
        
        let severity_distribution = aggregator.events.iter()
            .fold(HashMap::new(), |mut acc, event| {
                *acc.entry(event.severity).or_insert(0) += 1;
                acc
            });
        
        let event_type_distribution = aggregator.event_type_counts.clone();
        
        let high_risk_events = aggregator.events.iter()
            .filter(|e| e.risk_score >= 0.7)
            .count() as u32;
        
        SecurityStatistics {
            total_events: aggregator.events.len() as u32,
            severity_distribution,
            event_type_distribution,
            high_risk_events,
            unique_ips: aggregator.ip_counts.len() as u32,
            unique_users: aggregator.user_counts.len() as u32,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SecurityStatistics {
    pub total_events: u32,
    pub severity_distribution: HashMap<SecuritySeverity, u32>,
    pub event_type_distribution: HashMap<SecurityEventType, u32>,
    pub high_risk_events: u32,
    pub unique_ips: u32,
    pub unique_users: u32,
}

/// Security logging middleware
pub async fn security_logging_middleware<B>(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    matched_path: Option<MatchedPath>,
    request: Request<B>,
    next: Next<B>,
) -> impl axum::response::IntoResponse
where
    B: axum::body::HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<axum::BoxError>,
{
    let addr = connect_info
        .map(|ConnectInfo(addr)| addr)
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    let start_time = Instant::now();
    let method = request.method().clone();
    let path = matched_path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let _user_agent = request.headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    
    // Extract user info if available (would come from auth middleware)
    let user_id: Option<String> = None;
    
    let response = next.run(request).await;
    
    let duration = start_time.elapsed();
    let status = response.status();
    
    // Log security-relevant events
    if status == StatusCode::UNAUTHORIZED {
        let mut event = SecurityLogEntry::new(
            SecurityEventType::UnauthorizedAccess,
            SecuritySeverity::Medium,
        )
        .with_ip(addr.ip().to_string())
        .with_request_info(path.clone(), method.to_string())
        .with_detail("duration_ms", duration.as_millis() as u64)
        .with_detail("status_code", status.as_u16());
        
        if let Some(uid) = &user_id {
            event = event.with_user(uid.clone());
        }
        
        log_security_event(event).await;
    } else if status == StatusCode::FORBIDDEN {
        let mut event = SecurityLogEntry::new(
            SecurityEventType::ForbiddenAccess,
            SecuritySeverity::Medium,
        )
        .with_ip(addr.ip().to_string())
        .with_request_info(path.clone(), method.to_string())
        .with_detail("duration_ms", duration.as_millis() as u64)
        .with_detail("status_code", status.as_u16());
        
        if let Some(uid) = &user_id {
            event = event.with_user(uid.clone());
        }
        
        log_security_event(event).await;
    }
    
    response
}

use std::sync::OnceLock;

// Global security logger instance
static SECURITY_LOGGER: OnceLock<Arc<SecurityLogger>> = OnceLock::new();

/// Initialize global security logger
pub fn init_security_logger(config: SecurityLoggerConfig) -> Arc<SecurityLogger> {
    let logger = Arc::new(SecurityLogger::new(config));
    SECURITY_LOGGER.get_or_init(|| logger.clone());
    logger
}

/// Get global security logger
pub fn get_security_logger() -> Option<Arc<SecurityLogger>> {
    SECURITY_LOGGER.get().cloned()
}

/// Log a security event using the global logger
pub async fn log_security_event(event: SecurityLogEntry) {
    if let Some(logger) = get_security_logger() {
        logger.log_event(event).await;
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_security_event_creation() {
        let event = SecurityLogEntry::new(
            SecurityEventType::LoginSuccess,
            SecuritySeverity::Info,
        )
        .with_ip("192.168.1.1".to_string())
        .with_user("user123".to_string());
        
        assert_eq!(event.event_type, SecurityEventType::LoginSuccess);
        assert_eq!(event.severity, SecuritySeverity::Info);
        assert_eq!(event.ip_address, Some("192.168.1.1".to_string()));
        assert_eq!(event.user_id, Some("user123".to_string()));
    }
    
    #[tokio::test]
    async fn test_risk_score_calculation() {
        let mut event = SecurityLogEntry::new(
            SecurityEventType::SqlInjectionAttempt,
            SecuritySeverity::High,
        );
        
        event.calculate_risk_score();
        
        assert!(event.risk_score > 0.7);
        assert!(event.flagged);
    }
    
    #[tokio::test]
    async fn test_event_aggregation() {
        let mut aggregator = EventAggregator::new();
        
        for _ in 0..15 {
            let event = SecurityLogEntry::new(
                SecurityEventType::LoginFailure,
                SecuritySeverity::Medium,
            )
            .with_ip("192.168.1.1".to_string());
            
            aggregator.add_event(&event);
        }
        
        let alerts = aggregator.detect_patterns();
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|a| a.alert_type.contains("Brute Force")));
    }
    
    #[tokio::test]
    async fn test_security_logger() {
        let config = SecurityLoggerConfig {
            log_file_path: "/tmp/test_security.log".to_string(),
            ..Default::default()
        };
        
        let logger = SecurityLogger::new(config);
        
        let event = SecurityLogEntry::new(
            SecurityEventType::LoginSuccess,
            SecuritySeverity::Info,
        )
        .with_ip("127.0.0.1".to_string());
        
        logger.log_event(event).await;
        
        let stats = logger.get_statistics().await;
        assert_eq!(stats.total_events, 1);
    }
}
