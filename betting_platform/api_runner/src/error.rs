//! Structured error handling for production-grade API

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// API Error response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
    pub request_id: String,
    pub timestamp: i64,
}

/// Error detail structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub help: Option<String>,
}

/// Application error types
#[derive(Debug)]
pub enum AppError {
    // Client errors (4xx)
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    ValidationError { field: String, message: String },
    RateLimitExceeded { retry_after: u64 },
    
    // Server errors (5xx)
    Internal(String),
    ServiceUnavailable(String),
    ExternalServiceError { service: String, error: String },
    
    // Business logic errors
    InsufficientBalance { required: u64, available: u64 },
    MarketClosed { market_id: String },
    InvalidLeverage { requested: f64, max: f64 },
    PositionNotFound { position_id: String },
    
    // Database errors
    DatabaseError(String),
    
    // Solana/Blockchain errors
    BlockchainError(String),
    TransactionFailed { signature: String, error: String },
    
    // Cache errors
    CacheError(String),
    
    // Wallet verification errors
    WalletVerificationFailed { reason: String, wallet: String },
}

impl AppError {
    /// Get HTTP status code for error
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::ValidationError { .. } => StatusCode::BAD_REQUEST,
            AppError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::ExternalServiceError { .. } => StatusCode::BAD_GATEWAY,
            
            AppError::InsufficientBalance { .. } => StatusCode::BAD_REQUEST,
            AppError::MarketClosed { .. } => StatusCode::BAD_REQUEST,
            AppError::InvalidLeverage { .. } => StatusCode::BAD_REQUEST,
            AppError::PositionNotFound { .. } => StatusCode::NOT_FOUND,
            
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BlockchainError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::TransactionFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::CacheError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::WalletVerificationFailed { .. } => StatusCode::UNAUTHORIZED,
        }
    }
    
    /// Get error code
    fn error_code(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::Forbidden(_) => "FORBIDDEN",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::ValidationError { .. } => "VALIDATION_ERROR",
            AppError::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
            
            AppError::Internal(_) => "INTERNAL_ERROR",
            AppError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            AppError::ExternalServiceError { .. } => "EXTERNAL_SERVICE_ERROR",
            
            AppError::InsufficientBalance { .. } => "INSUFFICIENT_BALANCE",
            AppError::MarketClosed { .. } => "MARKET_CLOSED",
            AppError::InvalidLeverage { .. } => "INVALID_LEVERAGE",
            AppError::PositionNotFound { .. } => "POSITION_NOT_FOUND",
            
            AppError::DatabaseError(_) => "DATABASE_ERROR",
            AppError::BlockchainError(_) => "BLOCKCHAIN_ERROR",
            AppError::TransactionFailed { .. } => "TRANSACTION_FAILED",
            AppError::CacheError(_) => "CACHE_ERROR",
            AppError::WalletVerificationFailed { .. } => "WALLET_VERIFICATION_FAILED",
        }
    }
    
    /// Get user-friendly message
    fn message(&self) -> String {
        match self {
            AppError::BadRequest(msg) => msg.clone(),
            AppError::Unauthorized(msg) => msg.clone(),
            AppError::Forbidden(msg) => msg.clone(),
            AppError::NotFound(msg) => msg.clone(),
            AppError::Conflict(msg) => msg.clone(),
            AppError::ValidationError { field, message } => {
                format!("Validation error on field '{}': {}", field, message)
            }
            AppError::RateLimitExceeded { retry_after } => {
                format!("Rate limit exceeded. Retry after {} seconds", retry_after)
            }
            
            AppError::Internal(_) => "Internal server error occurred".to_string(),
            AppError::ServiceUnavailable(msg) => msg.clone(),
            AppError::ExternalServiceError { service, error } => {
                format!("External service '{}' error: {}", service, error)
            }
            
            AppError::InsufficientBalance { required, available } => {
                format!("Insufficient balance. Required: {}, Available: {}", required, available)
            }
            AppError::MarketClosed { market_id } => {
                format!("Market {} is closed", market_id)
            }
            AppError::InvalidLeverage { requested, max } => {
                format!("Invalid leverage {}. Maximum allowed: {}", requested, max)
            }
            AppError::PositionNotFound { position_id } => {
                format!("Position {} not found", position_id)
            }
            
            AppError::DatabaseError(_) => "Database operation failed".to_string(),
            AppError::BlockchainError(msg) => format!("Blockchain error: {}", msg),
            AppError::TransactionFailed { signature, error } => {
                format!("Transaction {} failed: {}", signature, error)
            }
            AppError::CacheError(msg) => format!("Cache operation failed: {}", msg),
            AppError::WalletVerificationFailed { reason, wallet } => {
                format!("Wallet verification failed for {}: {}", wallet, reason)
            }
        }
    }
    
    /// Get error details as JSON
    fn details(&self) -> Option<serde_json::Value> {
        match self {
            AppError::ValidationError { field, .. } => Some(serde_json::json!({
                "field": field
            })),
            AppError::RateLimitExceeded { retry_after } => Some(serde_json::json!({
                "retry_after": retry_after
            })),
            AppError::InsufficientBalance { required, available } => Some(serde_json::json!({
                "required": required,
                "available": available
            })),
            AppError::InvalidLeverage { requested, max } => Some(serde_json::json!({
                "requested": requested,
                "max": max
            })),
            AppError::TransactionFailed { signature, .. } => Some(serde_json::json!({
                "signature": signature
            })),
            AppError::WalletVerificationFailed { wallet, .. } => Some(serde_json::json!({
                "wallet": wallet
            })),
            _ => None,
        }
    }
    
    /// Get help text for error
    fn help(&self) -> Option<String> {
        match self {
            AppError::ValidationError { .. } => {
                Some("Check the field requirements and try again".to_string())
            }
            AppError::RateLimitExceeded { .. } => {
                Some("Please wait before making more requests".to_string())
            }
            AppError::InsufficientBalance { .. } => {
                Some("Deposit more funds to continue trading".to_string())
            }
            AppError::InvalidLeverage { .. } => {
                Some("Reduce your leverage to within allowed limits".to_string())
            }
            AppError::WalletVerificationFailed { .. } => {
                Some("Generate a new challenge and sign it with your wallet".to_string())
            }
            _ => None,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_response = ErrorResponse {
            error: ErrorDetail {
                code: self.error_code().to_string(),
                message: self.message(),
                details: self.details(),
                help: self.help(),
            },
            request_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        // Log error for monitoring
        tracing::error!(
            request_id = %error_response.request_id,
            error_code = %error_response.error.code,
            status = %status,
            "API error occurred"
        );
        
        (status, Json(error_response)).into_response()
    }
}

// Convenient type alias
pub type Result<T> = std::result::Result<T, AppError>;

// From implementations for common errors
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::BadRequest(format!("Invalid JSON: {}", err))
    }
}

impl From<solana_client::client_error::ClientError> for AppError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        AppError::BlockchainError(err.to_string())
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_response() {
        let error = AppError::ValidationError {
            field: "amount".to_string(),
            message: "Must be positive".to_string(),
        };
        
        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(error.error_code(), "VALIDATION_ERROR");
        assert!(error.details().is_some());
        assert!(error.help().is_some());
    }
    
    #[test]
    fn test_error_display() {
        let error = AppError::InsufficientBalance {
            required: 1000,
            available: 500,
        };
        
        let message = error.to_string();
        assert!(message.contains("1000"));
        assert!(message.contains("500"));
    }
}