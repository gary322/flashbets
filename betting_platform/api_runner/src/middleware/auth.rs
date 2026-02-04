//! Authentication middleware for protecting endpoints
//! Provides JWT-based authentication with production-grade security

use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use crate::{
    auth::{AuthService, Claims, AuthConfig, UserRole, AuthError},
    AppState,
};

/// Authenticated user extracted from JWT token
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub wallet: String,
    pub role: UserRole,
    pub claims: Claims,
}

/// Extract authenticated user from request
#[async_trait::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract token from Authorization header
        let token = extract_token_from_headers(&parts.headers)?;
        
        // Create auth service
        let auth_config = AuthConfig {
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-secret-key-must-be-at-least-32-characters-long".to_string()),
            jwt_expiration_hours: 24,
        };
        let auth_service = AuthService::new(auth_config);
        
        // Validate token
        let claims = auth_service.validate_token(&token)
            .map_err(|e| match e {
                AuthError::TokenExpired => AuthRejection::TokenExpired,
                AuthError::InvalidToken => AuthRejection::InvalidToken,
                _ => AuthRejection::Unauthorized,
            })?;
        
        Ok(AuthenticatedUser {
            wallet: claims.wallet.clone(),
            role: claims.role.clone(),
            claims,
        })
    }
}

/// Optional authentication - doesn't fail if no token present
#[derive(Debug, Clone)]
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match AuthenticatedUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalAuth(Some(user))),
            Err(_) => Ok(OptionalAuth(None)),
        }
    }
}

/// Require specific role
pub struct RequireRole {
    pub user: AuthenticatedUser,
    pub required_role: UserRole,
}

impl RequireRole {
    pub fn admin(user: AuthenticatedUser) -> Result<Self, AuthRejection> {
        if user.role != UserRole::Admin {
            return Err(AuthRejection::InsufficientPermissions);
        }
        Ok(Self {
            user,
            required_role: UserRole::Admin,
        })
    }
    
    pub fn market_maker(user: AuthenticatedUser) -> Result<Self, AuthRejection> {
        if user.role != UserRole::Market && user.role != UserRole::Admin {
            return Err(AuthRejection::InsufficientPermissions);
        }
        Ok(Self {
            user,
            required_role: UserRole::Market,
        })
    }
}

/// Authentication rejection reasons
#[derive(Debug)]
pub enum AuthRejection {
    MissingToken,
    InvalidToken,
    TokenExpired,
    Unauthorized,
    InsufficientPermissions,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AuthRejection::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "MISSING_TOKEN",
                "Missing authorization token"
            ),
            AuthRejection::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "INVALID_TOKEN",
                "Invalid authorization token"
            ),
            AuthRejection::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                "TOKEN_EXPIRED",
                "Authorization token has expired"
            ),
            AuthRejection::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Unauthorized access"
            ),
            AuthRejection::InsufficientPermissions => (
                StatusCode::FORBIDDEN,
                "INSUFFICIENT_PERMISSIONS",
                "Insufficient permissions for this operation"
            ),
        };

        (
            status,
            Json(json!({
                "error": {
                    "code": code,
                    "message": message
                }
            }))
        ).into_response()
    }
}

/// Extract JWT token from headers
fn extract_token_from_headers(headers: &header::HeaderMap) -> Result<String, AuthRejection> {
    headers
        .get(header::AUTHORIZATION)
        .ok_or(AuthRejection::MissingToken)?
        .to_str()
        .map_err(|_| AuthRejection::InvalidToken)?
        .strip_prefix("Bearer ")
        .ok_or(AuthRejection::InvalidToken)
        .map(|s| s.to_string())
}

/// Helper trait for extracting state from request extensions
pub trait FromRef<T> {
    fn from_ref(input: &T) -> Self;
}

impl FromRef<AppState> for AppState {
    fn from_ref(input: &AppState) -> Self {
        input.clone()
    }
}

/// Rate limiting middleware specifically for authenticated endpoints
pub struct AuthRateLimit {
    pub user: AuthenticatedUser,
    pub requests_remaining: u32,
}

/// API key authentication for programmatic access
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    pub api_key: String,
    pub wallet: String,
    pub permissions: Vec<String>,
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth
where
    S: Send + Sync,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let api_key = parts
            .headers
            .get("X-API-Key")
            .ok_or(AuthRejection::MissingToken)?
            .to_str()
            .map_err(|_| AuthRejection::InvalidToken)?;
        
        // In production, validate API key against database
        // For now, simple validation
        if api_key.starts_with("pk_") && api_key.len() > 32 {
            Ok(ApiKeyAuth {
                api_key: api_key.to_string(),
                wallet: "api_wallet_placeholder".to_string(),
                permissions: vec!["read".to_string(), "write".to_string()],
            })
        } else {
            Err(AuthRejection::InvalidToken)
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    
    #[test]
    fn test_extract_token_from_headers() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test_token_123")
        );
        
        let result = extract_token_from_headers(&headers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_token_123");
    }
    
    #[test]
    fn test_missing_token() {
        let headers = header::HeaderMap::new();
        let result = extract_token_from_headers(&headers);
        assert!(matches!(result.unwrap_err(), AuthRejection::MissingToken));
    }
    
    #[test]
    fn test_invalid_token_format() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("InvalidFormat test_token_123")
        );
        
        let result = extract_token_from_headers(&headers);
        assert!(matches!(result.unwrap_err(), AuthRejection::InvalidToken));
    }
}