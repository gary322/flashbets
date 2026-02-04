//! State Synchronization Middleware
//! 
//! Provides middleware for state synchronization across requests
//! and WebSocket connections.

use std::{
    sync::Arc,
    time::Duration,
};
use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
    body::Body,
};
use tower::ServiceExt;
use tracing::{debug, error, warn};

use crate::{
    AppState,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    correlation_context::CorrelationContext,
    state_manager::StateManager,
    tracing_logger::CorrelationId,
};

/// State synchronization configuration
#[derive(Debug, Clone)]
pub struct StateSyncConfig {
    /// Enable state tracking for requests
    pub track_requests: bool,
    
    /// Enable automatic state propagation
    pub propagate_state: bool,
    
    /// State key prefix for request tracking
    pub request_prefix: String,
    
    /// State key prefix for user sessions
    pub session_prefix: String,
    
    /// Request state TTL
    pub request_state_ttl: Duration,
    
    /// Session state TTL  
    pub session_state_ttl: Duration,
}

impl Default for StateSyncConfig {
    fn default() -> Self {
        Self {
            track_requests: true,
            propagate_state: true,
            request_prefix: "request:".to_string(),
            session_prefix: "session:".to_string(),
            request_state_ttl: Duration::from_secs(300), // 5 minutes
            session_state_ttl: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// State synchronization middleware
pub async fn state_sync_middleware(
    State(app_state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next<Body>,
) -> Result<Response, AppError> {
    let context = ErrorContext::new("state_sync_middleware", "process_request");
    
    // Check if state manager is available
    let state_manager = match &app_state.state_manager {
        Some(manager) => manager,
        None => {
            // State management not available, proceed without it
            return Ok(next.run(request).await);
        }
    };
    
    // Get correlation ID from request
    let correlation_id = request
        .extensions()
        .get::<CorrelationContext>()
        .map(|ctx| ctx.correlation_id.clone())
        .unwrap_or_else(|| CorrelationId::new());
    
    // Track request start
    let request_key = format!("request:{}", correlation_id);
    let request_data = serde_json::json!({
        "method": request.method().to_string(),
        "uri": request.uri().to_string(),
        "started_at": chrono::Utc::now().to_rfc3339(),
        "headers": extract_safe_headers(&request),
    });
    
    if let Err(e) = state_manager.set(&request_key, request_data, "middleware").await {
        warn!("Failed to track request state: {}", e);
    }
    
    // Get user session if authenticated
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if let Ok(Some(user_id)) = extract_user_id_from_token(&app_state, token).await {
                    // Update session state
                    let session_key = format!("session:{}", user_id);
                    let session_data = serde_json::json!({
                        "last_request": chrono::Utc::now().to_rfc3339(),
                        "correlation_id": correlation_id,
                        "active": true,
                    });
                    
                    if let Err(e) = state_manager.set(&session_key, session_data, "middleware").await {
                        warn!("Failed to update session state: {}", e);
                    }
                    
                    // Add session info to request extensions
                    request.extensions_mut().insert(SessionInfo {
                        user_id: user_id.clone(),
                        session_key,
                    });
                }
            }
        }
    }
    
    // Process request
    let response = next.run(request).await;
    
    // Track request completion
    let completion_data = serde_json::json!({
        "completed_at": chrono::Utc::now().to_rfc3339(),
        "status": response.status().as_u16(),
        "duration_ms": 0, // Would need timing info
    });
    
    if let Err(e) = state_manager.set(
        &format!("{}:completion", request_key),
        completion_data,
        "middleware"
    ).await {
        warn!("Failed to track request completion: {}", e);
    }
    
    Ok(response)
}

/// Extract safe headers for logging
fn extract_safe_headers(request: &Request<Body>) -> serde_json::Value {
    let mut headers = serde_json::Map::new();
    
    // Safe headers to include
    let safe_headers = [
        "content-type",
        "accept",
        "user-agent",
        "referer",
        "origin",
        "x-correlation-id",
        "x-request-id",
    ];
    
    for header_name in &safe_headers {
        if let Some(value) = request.headers().get(*header_name) {
            if let Ok(value_str) = value.to_str() {
                headers.insert(header_name.to_string(), serde_json::Value::String(value_str.to_string()));
            }
        }
    }
    
    serde_json::Value::Object(headers)
}

/// Extract user ID from JWT token
async fn extract_user_id_from_token(
    app_state: &AppState,
    token: &str,
) -> Result<Option<String>, AppError> {
    // Use JWT manager to validate and extract claims
    match app_state.jwt_manager.validate_token(token) {
        Ok(claims) => Ok(Some(claims.sub)),
        Err(_) => Ok(None),
    }
}

/// Session information attached to requests
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub user_id: String,
    pub session_key: String,
}

/// State propagation service for WebSocket connections
pub struct StatePropagationService {
    state_manager: Arc<StateManager>,
    config: StateSyncConfig,
}

impl StatePropagationService {
    pub fn new(state_manager: Arc<StateManager>, config: StateSyncConfig) -> Self {
        Self {
            state_manager,
            config,
        }
    }
    
    /// Start state propagation for a WebSocket connection
    pub async fn start_propagation(
        self: Arc<Self>,
        user_id: String,
        mut sender: tokio::sync::mpsc::Sender<serde_json::Value>,
    ) {
        let mut event_receiver = self.state_manager.subscribe();
        
        loop {
            tokio::select! {
                // Listen for state changes
                Ok(event) = event_receiver.recv() => {
                    // Check if event is relevant to user
                    if self.is_relevant_to_user(&user_id, &event.key) {
                        let message = serde_json::json!({
                            "type": "state_update",
                            "key": event.key,
                            "value": event.new_value,
                            "timestamp": event.timestamp,
                        });
                        
                        if sender.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                
                // Periodic session heartbeat
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let session_key = format!("{}{}", self.config.session_prefix, user_id);
                    let heartbeat = serde_json::json!({
                        "last_heartbeat": chrono::Utc::now().to_rfc3339(),
                        "active": true,
                    });
                    
                    if let Err(e) = self.state_manager.set(&session_key, heartbeat, "propagation").await {
                        error!("Failed to update session heartbeat: {}", e);
                    }
                }
            }
        }
        
        // Mark session as inactive
        let session_key = format!("{}{}", self.config.session_prefix, user_id);
        let inactive = serde_json::json!({
            "active": false,
            "disconnected_at": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Err(e) = self.state_manager.set(&session_key, inactive, "propagation").await {
            error!("Failed to mark session inactive: {}", e);
        }
    }
    
    /// Check if a state key is relevant to a user
    fn is_relevant_to_user(&self, user_id: &str, key: &str) -> bool {
        // User-specific keys
        if key.contains(user_id) {
            return true;
        }
        
        // Global state keys
        if key.starts_with("global:") {
            return true;
        }
        
        // Market updates (example)
        if key.starts_with("market:") {
            return true;
        }
        
        false
    }
}

/// Clean up expired state entries
pub async fn cleanup_expired_state(state_manager: Arc<StateManager>, config: StateSyncConfig) {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
    
    loop {
        interval.tick().await;
        
        // Clean up old request states
        let request_keys = state_manager.get_keys_by_prefix(&config.request_prefix).await;
        let now = chrono::Utc::now();
        
        for key in request_keys {
            if let Ok(Some(value)) = state_manager.get::<serde_json::Value>(&key).await {
                if let Some(started_at) = value.get("started_at").and_then(|v| v.as_str()) {
                    if let Ok(start_time) = chrono::DateTime::parse_from_rfc3339(started_at) {
                        let age = now.signed_duration_since(start_time);
                        if age > chrono::Duration::from_std(config.request_state_ttl).unwrap() {
                            if let Err(e) = state_manager.remove(&key, "cleanup").await {
                                warn!("Failed to clean up expired request state: {}", e);
                            } else {
                                debug!("Cleaned up expired request state: {}", key);
                            }
                        }
                    }
                }
            }
        }
        
        // Clean up inactive sessions
        let session_keys = state_manager.get_keys_by_prefix(&config.session_prefix).await;
        
        for key in session_keys {
            if let Ok(Some(value)) = state_manager.get::<serde_json::Value>(&key).await {
                if let Some(active) = value.get("active").and_then(|v| v.as_bool()) {
                    if !active {
                        if let Some(disconnected_at) = value.get("disconnected_at").and_then(|v| v.as_str()) {
                            if let Ok(disconnect_time) = chrono::DateTime::parse_from_rfc3339(disconnected_at) {
                                let age = now.signed_duration_since(disconnect_time);
                                if age > chrono::Duration::from_std(config.session_state_ttl).unwrap() {
                                    if let Err(e) = state_manager.remove(&key, "cleanup").await {
                                        warn!("Failed to clean up expired session state: {}", e);
                                    } else {
                                        debug!("Cleaned up expired session state: {}", key);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Start background state sync tasks
pub fn start_state_sync_tasks(app_state: Arc<AppState>) {
    if let Some(state_manager) = &app_state.state_manager {
        let config = StateSyncConfig::default();
        
        // Start cleanup task
        let cleanup_manager = state_manager.clone();
        let cleanup_config = config.clone();
        tokio::spawn(async move {
            cleanup_expired_state(cleanup_manager, cleanup_config).await;
        });
        
        debug!("State synchronization background tasks started");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_state_sync_config() {
        let config = StateSyncConfig::default();
        assert!(config.track_requests);
        assert!(config.propagate_state);
        assert_eq!(config.request_prefix, "request:");
        assert_eq!(config.session_prefix, "session:");
    }
}