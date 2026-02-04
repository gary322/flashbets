//! Authentication endpoints with proper JWT validation

use axum::{
    extract::State,
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{
    AppState,
    jwt_validation::{AuthenticatedUser, JwtError},
    wallet_verification::VerificationRequest,
};

/// Login request structure
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub wallet: String,
    pub signature: String,
    pub message: String,
}

/// Login response with tokens
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: &'static str,
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Login endpoint with wallet signature verification
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Verify wallet signature
    let verification = VerificationRequest {
        wallet: payload.wallet.clone(),
        message: payload.message,
        signature: payload.signature,
        nonce: String::new(), // Add nonce field
    };
    
    match state.wallet_verification.verify_signature(verification).await {
        Ok(response) if response.verified => {
            // Signature is valid, proceed
        }
        _ => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    
    // Generate tokens (default to "user" role for new logins)
    let access_token = state.jwt_manager
        .generate_access_token(&payload.wallet, "user")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let refresh_token = state.jwt_manager
        .generate_refresh_token(&payload.wallet)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Log successful login
    state.security_logger.log_auth_event(
        &payload.wallet,
        "login_success",
        None,
    ).await;
    
    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600, // 1 hour in seconds
        token_type: "Bearer",
    }))
}

/// Refresh token endpoint
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Refresh tokens
    let (access_token, refresh_token) = state.jwt_manager
        .refresh_access_token(&payload.refresh_token)
        .map_err(|e| match e {
            JwtError::TokenExpired => StatusCode::UNAUTHORIZED,
            JwtError::InvalidTokenType => StatusCode::BAD_REQUEST,
            _ => StatusCode::UNAUTHORIZED,
        })?;
    
    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600,
        token_type: "Bearer",
    }))
}

/// Logout endpoint (optional - for token blacklisting in future)
pub async fn logout(
    user: AuthenticatedUser,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // In a production system, you might want to blacklist the token here
    // For now, just return success
    Json(serde_json::json!({
        "message": "Logged out successfully",
        "wallet": user.claims.wallet
    }))
}

/// Get current user info
pub async fn get_user_info(
    user: AuthenticatedUser,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "wallet": user.claims.wallet,
        "role": user.claims.role,
        "token_id": user.claims.jti,
        "issued_at": user.claims.iat,
        "expires_at": user.claims.exp,
    }))
}

/// Validate token endpoint (for external services)
pub async fn validate_token(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Map<String, serde_json::Value>>,
) -> Result<impl IntoResponse, StatusCode> {
    let token = payload.get("token")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let claims = state.jwt_manager
        .validate_token(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    Ok(Json(serde_json::json!({
        "valid": true,
        "wallet": claims.wallet,
        "role": claims.role,
        "expires_at": claims.exp,
    })))
}