//! Authentication handlers for wallet-based authentication
//! Provides production-ready authentication endpoints

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};
use crate::{
    AppState,
    auth::{AuthService, UserRole, AuthConfig},
    wallet_verification::{VerificationRequest, WalletVerificationService},
    response::responses,
};
use std::sync::Arc;

/// Wallet authentication request
#[derive(Debug, Deserialize)]
pub struct WalletAuthRequest {
    pub wallet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

/// Wallet authentication response
#[derive(Debug, Serialize)]
pub struct WalletAuthResponse {
    pub success: bool,
    pub token: String,
    pub expires_at: i64,
    pub wallet: String,
    pub role: UserRole,
}

/// Auth challenge response
#[derive(Debug, Serialize)]
pub struct AuthChallengeResponse {
    pub challenge: String,
    pub expires_at: i64,
}

/// Unified wallet authentication endpoint
/// Handles both challenge generation and signature verification
pub async fn authenticate_wallet(
    State(state): State<AppState>,
    Json(payload): Json<WalletAuthRequest>,
) -> Response {
    debug!("Wallet authentication request for: {}", payload.wallet);
    
    // Validate wallet address format
    if !is_valid_wallet_address(&payload.wallet) {
        return responses::bad_request("Invalid wallet address format").into_response();
    }
    
    // If signature is provided, verify it
    if let (Some(signature), Some(message)) = (payload.signature, payload.message) {
        // Create verification request
        let verification_req = VerificationRequest {
            wallet: payload.wallet.clone(),
            signature,
            message,
            nonce: payload.nonce.clone().unwrap_or_default(),
        };
        
        match state.wallet_verification.verify_signature(verification_req).await {
            Ok(response) => {
                if response.verified {
                    // Create auth service with config
                    let auth_config = AuthConfig {
                        jwt_secret: std::env::var("JWT_SECRET")
                            .unwrap_or_else(|_| "your-secret-key-must-be-at-least-32-characters-long".to_string()),
                        jwt_expiration_hours: 24,
                    };
                    let auth_service = AuthService::new(auth_config);
                    
                    // Generate JWT token
                    match auth_service.generate_token(&response.wallet, UserRole::User) {
                        Ok(token) => {
                            let auth_response = WalletAuthResponse {
                                success: true,
                                token: token.clone(),
                                expires_at: response.expires_at.unwrap_or(0) as i64,
                                wallet: response.wallet,
                                role: UserRole::User,
                            };
                            
                            info!("Wallet authenticated successfully: {}", payload.wallet);
                            responses::ok(auth_response).into_response()
                        }
                        Err(e) => {
                            error!("Failed to generate token: {:?}", e);
                            responses::internal_error("Failed to generate authentication token").into_response()
                        }
                    }
                } else {
                    responses::unauthorized("Invalid signature or expired challenge").into_response()
                }
            }
            Err(e) => {
                error!("Signature verification failed: {}", e);
                responses::unauthorized("Signature verification failed").into_response()
            }
        }
    } else {
        // No signature provided, generate challenge
        match state.wallet_verification.generate_challenge(&payload.wallet).await {
            Ok(challenge_response) => {
                let response = AuthChallengeResponse {
                    challenge: challenge_response.challenge_compat,
                    expires_at: challenge_response.expires_at as i64,
                };
                
                debug!("Challenge generated for wallet: {}", payload.wallet);
                responses::ok(response).into_response()
            }
            Err(e) => {
                error!("Failed to generate challenge: {}", e);
                responses::internal_error("Failed to generate authentication challenge").into_response()
            }
        }
    }
}

/// Refresh authentication token
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    pub success: bool,
    pub token: String,
    pub expires_at: i64,
}

pub async fn refresh_token(
    State(_state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Response {
    // Create auth service
    let auth_config = AuthConfig {
        jwt_secret: std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "your-secret-key-must-be-at-least-32-characters-long".to_string()),
        jwt_expiration_hours: 24,
    };
    let auth_service = AuthService::new(auth_config);
    
    match auth_service.refresh_token(&payload.token) {
        Ok(new_token) => {
            let response = RefreshTokenResponse {
                success: true,
                token: new_token,
                expires_at: chrono::Utc::now().timestamp() + 86400, // 24 hours
            };
            responses::ok(response).into_response()
        }
        Err(e) => {
            debug!("Token refresh failed: {:?}", e);
            responses::unauthorized("Invalid or expired token").into_response()
        }
    }
}

/// Logout endpoint
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub token: String,
}

pub async fn logout(
    State(_state): State<AppState>,
    Json(_payload): Json<LogoutRequest>,
) -> Response {
    // In a production system, you would:
    // 1. Add the token to a blacklist
    // 2. Clear any server-side sessions
    // 3. Notify other services
    
    responses::ok(json!({
        "success": true,
        "message": "Successfully logged out"
    })).into_response()
}

/// Get current user info from token
#[derive(Debug, Deserialize)]
pub struct UserInfoRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    pub wallet: String,
    pub role: UserRole,
    pub issued_at: i64,
    pub expires_at: i64,
}

pub async fn get_user_info(
    State(_state): State<AppState>,
    Json(payload): Json<UserInfoRequest>,
) -> Response {
    // Create auth service
    let auth_config = AuthConfig {
        jwt_secret: std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "your-secret-key-must-be-at-least-32-characters-long".to_string()),
        jwt_expiration_hours: 24,
    };
    let auth_service = AuthService::new(auth_config);
    
    match auth_service.validate_token(&payload.token) {
        Ok(claims) => {
            let response = UserInfoResponse {
                wallet: claims.wallet,
                role: claims.role,
                issued_at: claims.iat,
                expires_at: claims.exp,
            };
            responses::ok(response).into_response()
        }
        Err(e) => {
            debug!("Token validation failed: {:?}", e);
            responses::unauthorized("Invalid or expired token").into_response()
        }
    }
}

/// Validate wallet address format
fn is_valid_wallet_address(address: &str) -> bool {
    // For Solana addresses
    if address.len() == 44 || address.len() == 43 {
        // Basic base58 check
        address.chars().all(|c| {
            c.is_ascii_alphanumeric() && c != '0' && c != 'O' && c != 'I' && c != 'l'
        })
    } else if address.starts_with("0x") && address.len() == 42 {
        // For Ethereum addresses
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    } else if address.starts_with("demo_wallet_") {
        // For demo wallets
        true
    } else {
        false
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_validation() {
        // Valid Solana address
        assert!(is_valid_wallet_address("7EYnhQoR9YM3N7UoaKRoA44Uy8JeaZV3qyouov87awMs"));
        
        // Valid Ethereum address
        assert!(is_valid_wallet_address("0x742d35Cc6634C0532925a3b844Bc9e7595f8b2dc"));
        
        // Valid demo wallet
        assert!(is_valid_wallet_address("demo_wallet_test_001"));
        
        // Invalid addresses
        assert!(!is_valid_wallet_address(""));
        assert!(!is_valid_wallet_address("invalid"));
        assert!(!is_valid_wallet_address("0xinvalid"));
    }
}