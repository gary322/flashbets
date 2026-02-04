//! Production-ready JWT validation with proper expiration handling

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT Claims with proper expiration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub sub: String,        // Subject (wallet address)
    pub exp: i64,          // Expiration time (Unix timestamp)
    pub iat: i64,          // Issued at (Unix timestamp)
    pub nbf: i64,          // Not before (Unix timestamp)
    pub jti: String,       // JWT ID (unique identifier)
    pub role: String,      // User role
    pub wallet: String,    // Wallet address
}

/// JWT configuration
#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_minutes: i64,
    pub refresh_expiration_days: i64,
    pub issuer: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key".to_string()),
            expiration_minutes: 60, // 1 hour
            refresh_expiration_days: 30,
            issuer: "betting-platform".to_string(),
        }
    }
}

/// JWT manager for token operations
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    config: JwtConfig,
}

impl JwtManager {
    pub fn new(config: JwtConfig) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[config.issuer.clone()]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 5; // 5 seconds leeway for clock skew
        
        Self {
            encoding_key: EncodingKey::from_secret(config.secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(config.secret.as_bytes()),
            validation,
            config,
        }
    }
    
    /// Generate access token
    pub fn generate_access_token(&self, wallet: &str, role: &str) -> Result<String, JwtError> {
        let now = Utc::now();
        let exp = now + Duration::minutes(self.config.expiration_minutes);
        
        let claims = JwtClaims {
            sub: wallet.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            role: role.to_string(),
            wallet: wallet.to_string(),
        };
        
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("JWT".to_string());
        
        encode(&header, &claims, &self.encoding_key)
            .map_err(|_| JwtError::TokenCreation)
    }
    
    /// Generate refresh token
    pub fn generate_refresh_token(&self, wallet: &str) -> Result<String, JwtError> {
        let now = Utc::now();
        let exp = now + Duration::days(self.config.refresh_expiration_days);
        
        let claims = JwtClaims {
            sub: wallet.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            role: "refresh".to_string(),
            wallet: wallet.to_string(),
        };
        
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
            .map_err(|_| JwtError::TokenCreation)
    }
    
    /// Validate token and return claims
    pub fn validate_token(&self, token: &str) -> Result<JwtClaims, JwtError> {
        // Decode and validate token
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|err| {
                use jsonwebtoken::errors::ErrorKind;
                match err.kind() {
                    ErrorKind::ExpiredSignature => JwtError::TokenExpired,
                    ErrorKind::ImmatureSignature => JwtError::TokenNotYetValid,
                    ErrorKind::InvalidIssuer => JwtError::InvalidIssuer,
                    _ => JwtError::InvalidToken,
                }
            })?;
        
        // Additional validation
        let now = Utc::now().timestamp();
        if token_data.claims.exp < now {
            return Err(JwtError::TokenExpired);
        }
        
        Ok(token_data.claims)
    }
    
    /// Refresh access token using refresh token
    pub fn refresh_access_token(&self, refresh_token: &str) -> Result<(String, String), JwtError> {
        let claims = self.validate_token(refresh_token)?;
        
        // Verify this is a refresh token
        if claims.role != "refresh" {
            return Err(JwtError::InvalidTokenType);
        }
        
        // Generate new tokens
        let access_token = self.generate_access_token(&claims.wallet, "user")?;
        let new_refresh_token = self.generate_refresh_token(&claims.wallet)?;
        
        Ok((access_token, new_refresh_token))
    }
}

/// JWT validation errors
#[derive(Debug)]
pub enum JwtError {
    InvalidToken,
    TokenExpired,
    TokenNotYetValid,
    TokenCreation,
    MissingToken,
    InvalidTokenType,
    InvalidIssuer,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::InvalidToken => write!(f, "Invalid token"),
            JwtError::TokenExpired => write!(f, "Token expired"),
            JwtError::TokenNotYetValid => write!(f, "Token not yet valid"),
            JwtError::TokenCreation => write!(f, "Token creation failed"),
            JwtError::MissingToken => write!(f, "Missing token"),
            JwtError::InvalidTokenType => write!(f, "Invalid token type"),
            JwtError::InvalidIssuer => write!(f, "Invalid issuer"),
        }
    }
}

impl IntoResponse for JwtError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            JwtError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            JwtError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token has expired"),
            JwtError::TokenNotYetValid => (StatusCode::UNAUTHORIZED, "Token not yet valid"),
            JwtError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create token"),
            JwtError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authorization header"),
            JwtError::InvalidTokenType => (StatusCode::BAD_REQUEST, "Invalid token type"),
            JwtError::InvalidIssuer => (StatusCode::UNAUTHORIZED, "Invalid token issuer"),
        };
        
        (status, Json(serde_json::json!({
            "error": message,
            "code": format!("{:?}", self)
        }))).into_response()
    }
}

/// Authenticated user extractor
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub claims: JwtClaims,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    Arc<JwtManager>: FromRequestParts<S, Rejection = Response>,
{
    type Rejection = Response;
    
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract JWT manager from app state
        let jwt_manager = Arc::<JwtManager>::from_request_parts(parts, state)
            .await
            .map_err(|_| JwtError::InvalidToken.into_response())?;
        
        // Extract token from Authorization header
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                if value.starts_with("Bearer ") {
                    Some(&value[7..])
                } else {
                    None
                }
            })
            .ok_or_else(|| JwtError::MissingToken.into_response())?;
        
        // Validate token
        let claims = jwt_manager
            .validate_token(token)
            .map_err(|e| e.into_response())?;
        
        Ok(AuthenticatedUser { claims })
    }
}

/// Optional authentication extractor
pub struct OptionalAuth {
    pub claims: Option<JwtClaims>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
    AuthenticatedUser: FromRequestParts<S, Rejection = Response>,
{
    type Rejection = Response;
    
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match AuthenticatedUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalAuth { claims: Some(user.claims) }),
            Err(_) => Ok(OptionalAuth { claims: None }),
        }
    }
}