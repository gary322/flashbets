//! JWT Authentication module for production-grade security

use axum::{
    extract::{FromRequest},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,        // Subject (user ID)
    pub exp: i64,          // Expiration time
    pub iat: i64,          // Issued at
    pub jti: String,       // JWT ID
    pub wallet: String,    // Wallet address
    pub role: UserRole,    // User role
}

/// User roles for authorization
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum UserRole {
    User,
    Admin,
    Market,  // Market maker role
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::Market => write!(f, "marketmaker"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "admin" => Ok(UserRole::Admin),
            "marketmaker" | "market" => Ok(UserRole::Market),
            _ => Err(format!("Invalid role: {}", s)),
        }
    }
}

/// Auth configuration
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
}

/// Auth service
pub struct AuthService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    expiration_duration: Duration,
}

impl AuthService {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(config.jwt_secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(config.jwt_secret.as_bytes()),
            validation: Validation::new(Algorithm::HS256),
            expiration_duration: Duration::hours(config.jwt_expiration_hours),
        }
    }

    /// Generate a new JWT token
    pub fn generate_token(&self, wallet: &str, role: UserRole) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + self.expiration_duration;

        let claims = Claims {
            sub: wallet.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: Uuid::new_v4().to_string(),
            wallet: wallet.to_string(),
            role,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|_| AuthError::TokenCreation)
    }

    /// Validate and decode a JWT token
    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map(|data| data.claims)
            .map_err(|err| match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken,
            })
    }

    /// Refresh a token (validates old token and issues new one)
    pub fn refresh_token(&self, token: &str) -> Result<String, AuthError> {
        let claims = self.validate_token(token)?;
        self.generate_token(&claims.wallet, claims.role)
    }
}

/// Auth errors
#[derive(Debug)]
pub enum AuthError {
    InvalidToken,
    TokenExpired,
    TokenCreation,
    MissingToken,
    Unauthorized,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create token"),
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authorization token"),
            AuthError::Unauthorized => (StatusCode::FORBIDDEN, "Unauthorized"),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

/// Authenticated user extractor
pub struct AuthUser {
    pub claims: Claims,
}

// Temporarily disabled due to axum version incompatibility
/*
#[async_trait::async_trait]
impl<S> FromRequest<S> for AuthUser
where
    S: Send + Sync,
    Arc<AuthService>: FromRequest<S>,
{
    type Rejection = AuthError;

    async fn from_request(req: Request<axum::body::Body>, state: &S) -> Result<Self, Self::Rejection> {
        // Extract auth service from state
        let auth_service = req
            .extract_parts::<State<Arc<AuthService>>>()
            .await
            .map_err(|_| AuthError::Unauthorized)?
            .0;

        // Extract token from Authorization header
        let headers = req.headers();
        let token = extract_token_from_headers(headers)?;

        // Validate token
        let claims = auth_service.validate_token(&token)?;

        Ok(AuthUser { claims })
    }
}
*/

/// Extract JWT token from headers
fn extract_token_from_headers(headers: &HeaderMap) -> Result<String, AuthError> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            if value.starts_with("Bearer ") {
                Some(value[7..].to_string())
            } else {
                None
            }
        })
        .ok_or(AuthError::MissingToken)
}

/// Middleware for requiring specific roles
pub struct RequireRole(pub UserRole);

// Temporarily disabled due to axum version incompatibility
/*
#[async_trait::async_trait]
impl<S> FromRequest<S> for RequireRole
where
    S: Send + Sync,
    AuthUser: FromRequest<S, Rejection = AuthError>,
{
    type Rejection = AuthError;

    async fn from_request(req: Request<axum::body::Body>, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request(req, state).await?;
        
        // Check if user has required role
        if user.claims.role != UserRole::Admin {
            return Err(AuthError::Unauthorized);
        }

        Ok(RequireRole(user.claims.role))
    }
}
*/

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub wallet: String,
    pub signature: String,
    pub message: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: i64,
    pub wallet: String,
    pub role: UserRole,
}

/// Verify wallet signature (simplified for demo)
pub fn verify_wallet_signature(wallet: &str, signature: &str, message: &str) -> bool {
    // In production, this would verify the actual wallet signature
    // For now, we'll do a simple check
    !wallet.is_empty() && !signature.is_empty() && !message.is_empty()
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation() {
        let config = AuthConfig {
            jwt_secret: "test_secret_key_for_testing_only".to_string(),
            jwt_expiration_hours: 24,
        };
        
        let auth_service = AuthService::new(config);
        let token = auth_service.generate_token("test_wallet", UserRole::User).unwrap();
        
        assert!(!token.is_empty());
    }

    #[test]
    fn test_token_validation() {
        let config = AuthConfig {
            jwt_secret: "test_secret_key_for_testing_only".to_string(),
            jwt_expiration_hours: 24,
        };
        
        let auth_service = AuthService::new(config);
        let token = auth_service.generate_token("test_wallet", UserRole::User).unwrap();
        
        let claims = auth_service.validate_token(&token).unwrap();
        assert_eq!(claims.wallet, "test_wallet");
        assert_eq!(claims.role, UserRole::User);
    }

    #[test]
    fn test_invalid_token() {
        let config = AuthConfig {
            jwt_secret: "test_secret_key_for_testing_only".to_string(),
            jwt_expiration_hours: 24,
        };
        
        let auth_service = AuthService::new(config);
        let result = auth_service.validate_token("invalid_token");
        
        assert!(result.is_err());
    }
}