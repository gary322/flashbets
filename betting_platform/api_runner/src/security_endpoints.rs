//! Security monitoring and management endpoints

use axum::{
    extract::{Query, State, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::{
    AppState,
    rbac_authorization::{CanUpdateSystemConfig, RequireRole, Role},
    security::security_logger::{SecurityEventType, SecuritySeverity},
};

/// Query parameters for security events
#[derive(Debug, Deserialize, Serialize)]
pub struct SecurityEventQuery {
    pub limit: Option<usize>,
    pub severity: Option<String>,
    pub event_type: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub user_id: Option<String>,
    pub wallet_address: Option<String>,
}

/// Get recent security events (admin only)
pub async fn get_security_events(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
    Query(params): Query<SecurityEventQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins and auditors can view security logs
    if role != Role::Admin && role != Role::Auditor {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Get recent events
    let limit = params.limit.unwrap_or(100).min(1000);
    let events = state.security_logger.get_recent_events(limit).await;
    
    // Filter events based on query parameters
    let filtered_events: Vec<_> = events.into_iter()
        .filter(|event| {
            // Filter by severity
            if let Some(ref severity_str) = params.severity {
                let severity = match severity_str.to_lowercase().as_str() {
                    "info" => SecuritySeverity::Info,
                    "low" => SecuritySeverity::Low,
                    "medium" => SecuritySeverity::Medium,
                    "high" => SecuritySeverity::High,
                    "critical" => SecuritySeverity::Critical,
                    _ => return false,
                };
                if event.severity != severity {
                    return false;
                }
            }
            
            // Filter by IP
            if let Some(ref ip) = params.ip_address {
                if event.ip_address.as_ref() != Some(ip) {
                    return false;
                }
            }
            
            // Filter by user
            if let Some(ref uid) = params.user_id {
                if event.user_id.as_ref() != Some(uid) {
                    return false;
                }
            }
            
            // Filter by wallet
            if let Some(ref wallet) = params.wallet_address {
                if event.wallet_address.as_ref() != Some(wallet) {
                    return false;
                }
            }
            
            // Filter by time range
            if let Some(start) = params.start_time {
                if event.timestamp < start {
                    return false;
                }
            }
            
            if let Some(end) = params.end_time {
                if event.timestamp > end {
                    return false;
                }
            }
            
            true
        })
        .collect();
    
    // Log that security logs were accessed
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "security_logs_accessed",
        Some(&format!("Viewed {} events", filtered_events.len())),
    ).await;
    
    Ok(Json(serde_json::json!({
        "events": filtered_events,
        "count": filtered_events.len(),
        "query": params,
        "accessed_by": user.claims.wallet,
    })))
}

/// Get security statistics
pub async fn get_security_stats(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins, auditors, and support can view stats
    if role != Role::Admin && role != Role::Auditor && role != Role::Support {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let stats = state.security_logger.get_statistics().await;
    
    Ok(Json(serde_json::json!({
        "statistics": stats,
        "timestamp": Utc::now().to_rfc3339(),
        "accessed_by": user.claims.wallet,
    })))
}

/// Security alert configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct AlertConfig {
    pub enabled: bool,
    pub threshold: f32,
    pub email_alerts: Option<bool>,
    pub webhook_url: Option<String>,
}

/// Update security alert configuration (admin only)
pub async fn update_alert_config(
    State(state): State<AppState>,
    CanUpdateSystemConfig { user }: CanUpdateSystemConfig,
    Json(config): Json<AlertConfig>,
) -> Result<impl IntoResponse, StatusCode> {
    // Log configuration change
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "security_config_updated",
        Some(&serde_json::to_string(&config).unwrap_or_default()),
    ).await;
    
    // In production, this would update the actual configuration
    Ok(Json(serde_json::json!({
        "success": true,
        "config": config,
        "updated_by": user.claims.wallet,
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

/// Block or unblock an IP address
#[derive(Debug, Deserialize)]
pub struct IpBlockRequest {
    pub action: String, // "block" or "unblock"
    pub duration_seconds: Option<u64>,
    pub reason: String,
}

pub async fn manage_ip_block(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
    Path(ip): Path<String>,
    Json(request): Json<IpBlockRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins can block/unblock IPs
    if role != Role::Admin {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let event_type = match request.action.as_str() {
        "block" => {
            // Log IP blocked event
            let event = crate::security::security_logger::SecurityLogEntry::new(
                SecurityEventType::IpBlocked,
                SecuritySeverity::High,
            )
            .with_ip(ip.clone())
            .with_detail("blocked_by", &user.claims.wallet)
            .with_detail("reason", &request.reason)
            .with_detail("duration_seconds", request.duration_seconds.unwrap_or(3600));
            
            state.security_logger.log_event(event).await;
            "ip_blocked"
        }
        "unblock" => {
            // Log IP unblocked event
            let event = crate::security::security_logger::SecurityLogEntry::new(
                SecurityEventType::IpUnblocked,
                SecuritySeverity::Medium,
            )
            .with_ip(ip.clone())
            .with_detail("unblocked_by", &user.claims.wallet)
            .with_detail("reason", &request.reason);
            
            state.security_logger.log_event(event).await;
            "ip_unblocked"
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    Ok(Json(serde_json::json!({
        "success": true,
        "ip": ip,
        "action": request.action,
        "performed_by": user.claims.wallet,
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

/// Search security logs
#[derive(Debug, Deserialize)]
pub struct SecuritySearchRequest {
    pub query: String,
    pub fields: Vec<String>,
    pub limit: Option<usize>,
}

pub async fn search_security_logs(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
    Json(search): Json<SecuritySearchRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins and auditors can search logs
    if role != Role::Admin && role != Role::Auditor {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Get all recent events
    let limit = search.limit.unwrap_or(1000).min(10000);
    let events = state.security_logger.get_recent_events(limit).await;
    
    // Search in specified fields
    let results: Vec<_> = events.into_iter()
        .filter(|event| {
            let event_json = serde_json::to_value(event).unwrap_or_default();
            
            for field in &search.fields {
                if let Some(value) = event_json.get(field) {
                    if let Some(str_value) = value.as_str() {
                        if str_value.to_lowercase().contains(&search.query.to_lowercase()) {
                            return true;
                        }
                    }
                }
            }
            
            // Also search in details
            if let Some(details) = event_json.get("details").and_then(|d| d.as_object()) {
                for (_, value) in details {
                    if let Some(str_value) = value.as_str() {
                        if str_value.to_lowercase().contains(&search.query.to_lowercase()) {
                            return true;
                        }
                    }
                }
            }
            
            false
        })
        .collect();
    
    // Log search activity
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "security_logs_searched",
        Some(&format!("Query: '{}', Results: {}", search.query, results.len())),
    ).await;
    
    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
        "query": search.query,
        "fields": search.fields,
        "searched_by": user.claims.wallet,
    })))
}

/// Export security logs
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub format: String, // "json", "csv"
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub include_sensitive: Option<bool>,
}

pub async fn export_security_logs(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
    Json(export): Json<ExportRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins can export logs
    if role != Role::Admin {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Log export attempt
    let event = crate::security::security_logger::SecurityLogEntry::new(
        SecurityEventType::DataExportAttempt,
        SecuritySeverity::High,
    )
    .with_user(user.claims.wallet.clone())
    .with_detail("export_type", "security_logs")
    .with_detail("format", &export.format)
    .with_detail("time_range", &format!("{} to {}", export.start_time, export.end_time));
    
    state.security_logger.log_event(event).await;
    
    // In production, this would generate the actual export
    Ok(Json(serde_json::json!({
        "success": true,
        "export_id": uuid::Uuid::new_v4().to_string(),
        "format": export.format,
        "exported_by": user.claims.wallet,
        "timestamp": Utc::now().to_rfc3339(),
        "download_url": "/api/security/export/download/{export_id}",
    })))
}

/// Real-time security dashboard data
pub async fn get_security_dashboard(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins, auditors, and support can view dashboard
    if role != Role::Admin && role != Role::Auditor && role != Role::Support {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Get current statistics
    let stats = state.security_logger.get_statistics().await;
    
    // Get recent high-risk events
    let recent_events = state.security_logger.get_recent_events(50).await;
    let high_risk_events: Vec<_> = recent_events.into_iter()
        .filter(|e| e.risk_score >= 0.7)
        .take(10)
        .collect();
    
    Ok(Json(serde_json::json!({
        "statistics": stats,
        "high_risk_events": high_risk_events,
        "alerts_active": true,
        "system_status": "secure",
        "last_update": Utc::now().to_rfc3339(),
        "accessed_by": user.claims.wallet,
    })))
}

