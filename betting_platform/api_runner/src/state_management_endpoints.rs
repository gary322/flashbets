//! State Management REST API Endpoints
//! 
//! Provides REST endpoints for interacting with the centralized state management system.

use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    AppState,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    response::ApiResponse,
    middleware::AuthenticatedUser,
    auth::UserRole,
    state_manager::{StateManager, StateSnapshot, StateStats},
};

/// Query parameters for state operations
#[derive(Debug, Deserialize)]
pub struct StateQuery {
    pub prefix: Option<String>,
    pub include_metadata: Option<bool>,
}

/// Request body for state set operation
#[derive(Debug, Deserialize)]
pub struct SetStateRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Request body for compare-and-swap operation
#[derive(Debug, Deserialize)]
pub struct CompareAndSwapRequest {
    pub key: String,
    pub expected: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

/// Response for state get operation
#[derive(Debug, Serialize)]
pub struct StateResponse {
    pub key: String,
    pub value: serde_json::Value,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub version: u64,
}

/// Response for state keys list
#[derive(Debug, Serialize)]
pub struct StateKeysResponse {
    pub keys: Vec<String>,
    pub total: usize,
}

/// Response for compare-and-swap operation
#[derive(Debug, Serialize)]
pub struct CompareAndSwapResponse {
    pub success: bool,
    pub version: u64,
}

/// Get state value by key
pub async fn get_state(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(key): Path<String>,
    Query(params): Query<StateQuery>,
) -> Result<Json<ApiResponse<Option<StateResponse>>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "get_state");
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Get value
    let value: Option<serde_json::Value> = state_manager.get(&key).await?;
    
    let response = match value {
        Some(val) => {
            let metadata = if params.include_metadata.unwrap_or(false) {
                state_manager.get_metadata(&key).await
            } else {
                None
            };
            
            let version = state_manager.get_version().await;
            
            Some(StateResponse {
                key,
                value: val,
                metadata,
                version,
            })
        }
        None => None,
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// Set state value
pub async fn set_state(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<SetStateRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "set_state");
    
    // Only admins can set state
    if user.role != UserRole::Admin {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only administrators can set state",
            context,
        ));
    }
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Set value
    state_manager.set(&req.key, req.value, &user.wallet).await?;
    
    // Set metadata if provided
    if let Some(metadata) = req.metadata {
        state_manager.set_metadata(&req.key, metadata).await;
    }
    
    info!("State value set by user {}: key={}", user.wallet, req.key);
    
    Ok(Json(ApiResponse::success(())))
}

/// Remove state value
pub async fn remove_state(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "remove_state");
    
    // Only admins can remove state
    if user.role != UserRole::Admin {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only administrators can remove state",
            context,
        ));
    }
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Remove value
    state_manager.remove(&key, &user.wallet).await?;
    
    info!("State value removed by user {}: key={}", user.wallet, key);
    
    Ok(Json(ApiResponse::success(())))
}

/// List state keys by prefix
pub async fn list_state_keys(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(params): Query<StateQuery>,
) -> Result<Json<ApiResponse<StateKeysResponse>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "list_state_keys");
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Get keys
    let prefix = params.prefix.as_deref().unwrap_or("");
    let keys = state_manager.get_keys_by_prefix(prefix).await;
    
    Ok(Json(ApiResponse::success(StateKeysResponse {
        total: keys.len(),
        keys,
    })))
}

/// Compare and swap operation
pub async fn compare_and_swap_state(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(req): Json<CompareAndSwapRequest>,
) -> Result<Json<ApiResponse<CompareAndSwapResponse>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "compare_and_swap_state");
    
    // Only admins can perform CAS
    if user.role != UserRole::Admin {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only administrators can perform compare-and-swap",
            context,
        ));
    }
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Perform CAS
    let success = state_manager.compare_and_swap(
        &req.key,
        req.expected,
        req.new_value,
        &user.wallet,
    ).await?;
    
    let version = state_manager.get_version().await;
    
    if success {
        info!("Compare-and-swap succeeded for key {} by user {}", req.key, user.wallet);
    } else {
        warn!("Compare-and-swap failed for key {} by user {}", req.key, user.wallet);
    }
    
    Ok(Json(ApiResponse::success(CompareAndSwapResponse {
        success,
        version,
    })))
}

/// Get state statistics
pub async fn get_state_stats(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<StateStats>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "get_state_stats");
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Get stats
    let stats = state_manager.get_stats().await;
    
    Ok(Json(ApiResponse::success(stats)))
}

/// Create state snapshot
pub async fn create_snapshot(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<StateSnapshot>>, AppError> {
    let context = ErrorContext::new("state_endpoints", "create_snapshot");
    
    // Only admins can create snapshots
    if user.role != UserRole::Admin {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only administrators can create snapshots",
            context,
        ));
    }
    
    // Check if state manager is available
    let state_manager = state
        .state_manager
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "State management service not available",
            context.clone(),
        ))?;
    
    // Create snapshot
    let snapshot = state_manager.create_snapshot().await?;
    
    info!("State snapshot created by user {}", user.wallet);
    
    Ok(Json(ApiResponse::success(snapshot)))
}

/// WebSocket endpoint for state change events
pub async fn state_events_websocket(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    let state_manager = state.state_manager.clone();
    
    ws.on_upgrade(move |socket| async move {
        if let Some(manager) = state_manager {
            handle_state_events_websocket(socket, manager, user).await;
        }
    })
}

/// Handle WebSocket connection for state events
async fn handle_state_events_websocket(
    socket: axum::extract::ws::WebSocket,
    state_manager: Arc<StateManager>,
    user: AuthenticatedUser,
) {
    use axum::extract::ws::{Message, WebSocket};
    use futures_util::{SinkExt, StreamExt};
    
    let (mut tx, mut rx) = socket.split();
    
    // Subscribe to state changes
    let mut event_receiver = state_manager.subscribe();
    
    // Spawn task to forward events to WebSocket
    let user_id = user.wallet.clone();
    tokio::spawn(async move {
        while let Ok(event) = event_receiver.recv().await {
            let msg = serde_json::json!({
                "type": "state_change",
                "data": event,
            });
            
            if tx.send(Message::Text(msg.to_string())).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages (mainly for keepalive)
    while let Some(msg) = rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if text == "ping" {
                    // Ignore ping messages
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }
    
    info!("State events WebSocket closed for user {}", user_id);
}

/// Register state management routes
pub fn register_routes(app: axum::Router<Arc<AppState>>) -> axum::Router<Arc<AppState>> {
    app.nest("/api/v1/state", axum::Router::new()
        .route("/keys", axum::routing::get(list_state_keys))
        .route("/stats", axum::routing::get(get_state_stats))
        .route("/snapshot", axum::routing::post(create_snapshot))
        .route("/cas", axum::routing::post(compare_and_swap_state))
        .route("/events", axum::routing::get(state_events_websocket))
        .route("/:key", axum::routing::get(get_state))
        .route("/:key", axum::routing::put(set_state))
        .route("/:key", axum::routing::delete(remove_state))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_state_endpoints() {
        // Test would go here
    }
}