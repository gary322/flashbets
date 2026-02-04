//! RBAC-protected endpoint examples

use axum::{
    extract::State,
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{
    AppState,
    rbac_authorization::{
        CanCreateMarkets, CanViewAllPositions, CanUpdateSystemConfig,
        RequireRole, Role, Permission
    },
    jwt_validation::AuthenticatedUser,
};

/// Admin-only endpoint to update user role
#[derive(Debug, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub wallet: String,
    pub role: String,
}

pub async fn update_user_role(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins can update roles
    if role != Role::Admin {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Validate new role
    let new_role = Role::from_str(&payload.role)
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    // Log the role change
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "role_updated",
        Some(&format!("Updated {} to role {}", payload.wallet, payload.role)),
    ).await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "wallet": payload.wallet,
        "new_role": payload.role,
        "updated_by": user.claims.wallet,
    })))
}

/// Market maker endpoint to create markets
pub async fn create_market_authorized(
    State(state): State<AppState>,
    CanCreateMarkets { user }: CanCreateMarkets,
    Json(payload): Json<crate::types::CreateMarketRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // User has permission to create markets
    tracing::info!(
        "User {} creating market: {}", 
        user.claims.wallet, 
        payload.question
    );
    
    // Log market creation
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "market_created",
        Some(&payload.question),
    ).await;
    
    // Here you would implement actual market creation
    Ok(Json(serde_json::json!({
        "success": true,
        "market_id": uuid::Uuid::new_v4().to_string(),
        "creator": user.claims.wallet,
        "question": payload.question,
    })))
}

/// Support/Admin endpoint to view all positions
pub async fn view_all_positions(
    _state: State<AppState>,
    CanViewAllPositions { user }: CanViewAllPositions,
) -> impl IntoResponse {
    // User has permission to view all positions
    tracing::info!("User {} viewing all positions", user.claims.wallet);
    
    // Here you would fetch all positions from database
    Json(serde_json::json!({
        "positions": [],
        "total": 0,
        "viewer": user.claims.wallet,
        "viewer_role": user.claims.role,
    }))
}

/// System config update (admin only)
#[derive(Debug, Deserialize, Serialize)]
pub struct SystemConfigUpdate {
    pub min_bet_amount: Option<u64>,
    pub max_bet_amount: Option<u64>,
    pub protocol_fee_rate: Option<u16>,
    pub emergency_mode: Option<bool>,
}

pub async fn update_system_config(
    State(state): State<AppState>,
    CanUpdateSystemConfig { user }: CanUpdateSystemConfig,
    Json(payload): Json<SystemConfigUpdate>,
) -> Result<impl IntoResponse, StatusCode> {
    // Log critical system update
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "system_config_updated",
        Some(&serde_json::to_string(&payload).unwrap_or_default()),
    ).await;
    
    // Here you would update system configuration
    Ok(Json(serde_json::json!({
        "success": true,
        "updated_by": user.claims.wallet,
        "changes": payload,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}

/// Get user permissions
pub async fn get_user_permissions(
    user: AuthenticatedUser,
) -> impl IntoResponse {
    let role = Role::from_str(&user.claims.role)
        .unwrap_or(Role::User);
    
    let permissions: Vec<String> = role.permissions()
        .into_iter()
        .map(|p| format!("{:?}", p))
        .collect();
    
    Json(serde_json::json!({
        "wallet": user.claims.wallet,
        "role": user.claims.role,
        "permissions": permissions,
    }))
}

/// Grant custom permission (admin only)
#[derive(Debug, Deserialize)]
pub struct GrantPermissionRequest {
    pub wallet: String,
    pub permission: String,
}

pub async fn grant_permission(
    State(state): State<AppState>,
    RequireRole { user, role }: RequireRole,
    Json(payload): Json<GrantPermissionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Only admins can grant permissions
    if role != Role::Admin {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Parse permission (in production, you'd have proper parsing)
    let permission = match payload.permission.as_str() {
        "ViewMarkets" => Permission::ViewMarkets,
        "CreateMarkets" => Permission::CreateMarkets,
        "PlaceTrades" => Permission::PlaceTrades,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    // Grant the permission
    state.authorization_service.grant_permission(&payload.wallet, permission);
    
    // Log the grant
    state.security_logger.log_auth_event(
        &user.claims.wallet,
        "permission_granted",
        Some(&format!("Granted {} to {}", payload.permission, payload.wallet)),
    ).await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "wallet": payload.wallet,
        "permission": payload.permission,
        "granted_by": user.claims.wallet,
    })))
}

