//! Typed error system with context and tracing

use std::fmt;
use std::error::Error as StdError;
use std::collections::HashMap;
use axum::{
    response::{IntoResponse, Response},
    Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use tracing::{error, warn};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Error context containing metadata about the error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub error_id: String,
    pub timestamp: DateTime<Utc>,
    pub service: String,
    pub operation: String,
    pub user_id: Option<String>,
    pub request_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ErrorContext {
    pub fn new(service: &str, operation: &str) -> Self {
        Self {
            error_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            service: service.to_string(),
            operation: operation.to_string(),
            user_id: None,
            request_id: None,
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
    
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

/// Main application error type
#[derive(Debug, Clone)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    pub context: ErrorContext,
    pub source: Option<String>,
    pub stack_trace: Vec<String>,
}

/// Error categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    // Authentication & Authorization
    Unauthorized,
    Forbidden,
    InvalidCredentials,
    TokenExpired,
    
    // Validation
    ValidationError,
    InvalidInput,
    MissingField,
    InvalidFormat,
    
    // Resource errors
    NotFound,
    AlreadyExists,
    Conflict,
    Gone,
    
    // Business logic
    InsufficientBalance,
    MarketClosed,
    OrderRejected,
    PositionLiquidated,
    RateLimitExceeded,
    
    // External service errors
    ExternalServiceError,
    SolanaRpcError,
    DatabaseError,
    CacheError,
    QueueError,
    
    // System errors
    InternalError,
    ConfigurationError,
    ServiceUnavailable,
    Timeout,
    CircuitBreakerOpen,
    FeatureDisabled,
}

impl ErrorKind {
    /// Get HTTP status code for error kind
    pub fn status_code(&self) -> StatusCode {
        match self {
            ErrorKind::Unauthorized | ErrorKind::InvalidCredentials => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden | ErrorKind::TokenExpired => StatusCode::FORBIDDEN,
            ErrorKind::ValidationError | ErrorKind::InvalidInput | 
            ErrorKind::MissingField | ErrorKind::InvalidFormat => StatusCode::BAD_REQUEST,
            ErrorKind::NotFound | ErrorKind::Gone => StatusCode::NOT_FOUND,
            ErrorKind::AlreadyExists | ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::InsufficientBalance | ErrorKind::MarketClosed |
            ErrorKind::OrderRejected | ErrorKind::PositionLiquidated => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorKind::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            ErrorKind::ExternalServiceError | ErrorKind::SolanaRpcError |
            ErrorKind::DatabaseError | ErrorKind::CacheError | 
            ErrorKind::QueueError => StatusCode::BAD_GATEWAY,
            ErrorKind::InternalError | ErrorKind::ConfigurationError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::ServiceUnavailable | ErrorKind::Timeout |
            ErrorKind::CircuitBreakerOpen => StatusCode::SERVICE_UNAVAILABLE,
            ErrorKind::FeatureDisabled => StatusCode::NOT_IMPLEMENTED,
        }
    }
    
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            ErrorKind::ServiceUnavailable |
            ErrorKind::Timeout |
            ErrorKind::ExternalServiceError |
            ErrorKind::SolanaRpcError |
            ErrorKind::DatabaseError |
            ErrorKind::CacheError |
            ErrorKind::QueueError
        )
    }
    
    /// Get user-facing error message
    pub fn user_message(&self) -> &'static str {
        match self {
            ErrorKind::Unauthorized => "Authentication required",
            ErrorKind::Forbidden => "Access denied",
            ErrorKind::InvalidCredentials => "Invalid credentials provided",
            ErrorKind::TokenExpired => "Authentication token has expired",
            ErrorKind::ValidationError => "Validation failed",
            ErrorKind::InvalidInput => "Invalid input provided",
            ErrorKind::MissingField => "Required field missing",
            ErrorKind::InvalidFormat => "Invalid format",
            ErrorKind::NotFound => "Resource not found",
            ErrorKind::AlreadyExists => "Resource already exists",
            ErrorKind::Conflict => "Resource conflict",
            ErrorKind::Gone => "Resource no longer available",
            ErrorKind::InsufficientBalance => "Insufficient balance",
            ErrorKind::MarketClosed => "Market is closed",
            ErrorKind::OrderRejected => "Order was rejected",
            ErrorKind::PositionLiquidated => "Position has been liquidated",
            ErrorKind::RateLimitExceeded => "Rate limit exceeded",
            ErrorKind::ExternalServiceError => "External service error",
            ErrorKind::SolanaRpcError => "Blockchain service error",
            ErrorKind::DatabaseError => "Database service error",
            ErrorKind::CacheError => "Cache service error",
            ErrorKind::QueueError => "Queue service error",
            ErrorKind::InternalError => "Internal server error",
            ErrorKind::ConfigurationError => "Configuration error",
            ErrorKind::ServiceUnavailable => "Service temporarily unavailable",
            ErrorKind::Timeout => "Request timeout",
            ErrorKind::CircuitBreakerOpen => "Service temporarily disabled",
            ErrorKind::FeatureDisabled => "Feature not available",
        }
    }
}

impl AppError {
    /// Create new error with context
    pub fn new(kind: ErrorKind, message: impl Into<String>, context: ErrorContext) -> Self {
        let message = message.into();
        
        // Log error based on severity
        match kind {
            ErrorKind::InternalError | ErrorKind::ConfigurationError => {
                error!(
                    error_id = %context.error_id,
                    service = %context.service,
                    operation = %context.operation,
                    "Critical error: {}", message
                );
            }
            ErrorKind::ExternalServiceError | ErrorKind::SolanaRpcError |
            ErrorKind::DatabaseError | ErrorKind::CacheError | ErrorKind::QueueError => {
                warn!(
                    error_id = %context.error_id,
                    service = %context.service,
                    operation = %context.operation,
                    "External service error: {}", message
                );
            }
            _ => {
                // Debug level for business logic errors
                tracing::debug!(
                    error_id = %context.error_id,
                    service = %context.service,
                    operation = %context.operation,
                    "Business error: {}", message
                );
            }
        }
        
        Self {
            kind,
            message,
            context,
            source: None,
            stack_trace: Vec::new(),
        }
    }
    
    /// Add source error
    pub fn with_source(mut self, source: impl StdError) -> Self {
        self.source = Some(source.to_string());
        
        // Capture stack trace
        let mut current: Option<&dyn StdError> = Some(&source);
        while let Some(err) = current {
            self.stack_trace.push(err.to_string());
            current = err.source();
        }
        
        self
    }
    
    /// Add metadata to error context
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.context.metadata.insert(key.to_string(), value);
        self
    }
    
    /// Create validation error with field details
    pub fn validation(field: &str, message: &str, context: ErrorContext) -> Self {
        Self::new(ErrorKind::ValidationError, message, context)
            .with_metadata("field", serde_json::json!(field))
    }
    
    /// Create not found error
    pub fn not_found(resource: &str, id: &str, context: ErrorContext) -> Self {
        Self::new(
            ErrorKind::NotFound,
            format!("{} with id {} not found", resource, id),
            context
        )
        .with_metadata("resource", serde_json::json!(resource))
        .with_metadata("id", serde_json::json!(id))
    }
    
    /// Create rate limit error
    pub fn rate_limit(limit: u32, window_seconds: u64, context: ErrorContext) -> Self {
        Self::new(
            ErrorKind::RateLimitExceeded,
            format!("Rate limit exceeded: {} requests per {} seconds", limit, window_seconds),
            context
        )
        .with_metadata("limit", serde_json::json!(limit))
        .with_metadata("window_seconds", serde_json::json!(window_seconds))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.user_message(), self.message)
    }
}

impl StdError for AppError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

/// Error response sent to clients
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetails,
    pub error_id: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetails {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.kind.status_code();
        
        // Extract field from metadata if validation error
        let field = if self.kind == ErrorKind::ValidationError {
            self.context.metadata.get("field")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };
        
        // Build user-safe metadata
        let mut safe_metadata = HashMap::new();
        
        // Add retry information if applicable
        if self.kind.is_retryable() {
            safe_metadata.insert("retryable".to_string(), serde_json::json!(true));
            safe_metadata.insert("retry_after_seconds".to_string(), serde_json::json!(5));
        }
        
        let response = ErrorResponse {
            error: ErrorDetails {
                code: format!("{:?}", self.kind).to_lowercase(),
                message: self.kind.user_message().to_string(),
                field,
                metadata: safe_metadata,
            },
            error_id: self.context.error_id.clone(),
            timestamp: self.context.timestamp,
        };
        
        (status, Json(response)).into_response()
    }
}

/// Result type alias for application
pub type AppResult<T> = Result<T, AppError>;

/// Implementation to convert typed_errors::AppError to error::AppError
impl From<AppError> for crate::error::AppError {
    fn from(err: AppError) -> Self {
        use crate::error::AppError as ErrorAppError;
        
        match err.kind {
            ErrorKind::Unauthorized => ErrorAppError::Unauthorized(err.message),
            ErrorKind::Forbidden => ErrorAppError::Forbidden(err.message),
            ErrorKind::NotFound => ErrorAppError::NotFound(err.message),
            ErrorKind::Conflict => ErrorAppError::Conflict(err.message),
            ErrorKind::ValidationError => {
                if let Some(field) = err.context.metadata.get("field").and_then(|v| v.as_str()) {
                    ErrorAppError::ValidationError {
                        field: field.to_string(),
                        message: err.message,
                    }
                } else {
                    ErrorAppError::BadRequest(err.message)
                }
            }
            ErrorKind::InvalidInput | ErrorKind::MissingField | ErrorKind::InvalidFormat => {
                ErrorAppError::BadRequest(err.message)
            }
            ErrorKind::RateLimitExceeded => ErrorAppError::RateLimitExceeded { retry_after: 60 },
            ErrorKind::ServiceUnavailable | ErrorKind::CircuitBreakerOpen => {
                ErrorAppError::ServiceUnavailable(err.message)
            }
            ErrorKind::ExternalServiceError | ErrorKind::SolanaRpcError => {
                ErrorAppError::ExternalServiceError {
                    service: err.context.service.clone(),
                    error: err.message,
                }
            }
            ErrorKind::InsufficientBalance => {
                if let (Some(required), Some(available)) = (
                    err.context.metadata.get("required").and_then(|v| v.as_u64()),
                    err.context.metadata.get("available").and_then(|v| v.as_u64())
                ) {
                    ErrorAppError::InsufficientBalance { required, available }
                } else {
                    ErrorAppError::BadRequest(err.message)
                }
            }
            ErrorKind::MarketClosed => {
                if let Some(market_id) = err.context.metadata.get("market_id").and_then(|v| v.as_str()) {
                    ErrorAppError::MarketClosed { market_id: market_id.to_string() }
                } else {
                    ErrorAppError::BadRequest(err.message)
                }
            }
            ErrorKind::NotFound => {
                if let Some(position_id) = err.context.metadata.get("position_id").and_then(|v| v.as_str()) {
                    ErrorAppError::PositionNotFound { position_id: position_id.to_string() }
                } else {
                    ErrorAppError::NotFound(err.message)
                }
            }
            _ => ErrorAppError::Internal(err.message),
        }
    }
}

/// Extension trait for converting external errors
pub trait ErrorExt<T> {
    fn app_err(self, kind: ErrorKind, context: ErrorContext) -> AppResult<T>;
    fn app_err_with(self, f: impl FnOnce() -> AppError) -> AppResult<T>;
}

impl<T, E: StdError + 'static> ErrorExt<T> for Result<T, E> {
    fn app_err(self, kind: ErrorKind, context: ErrorContext) -> AppResult<T> {
        self.map_err(|e| {
            AppError::new(kind, e.to_string(), context)
                .with_source(e)
        })
    }
    
    fn app_err_with(self, f: impl FnOnce() -> AppError) -> AppResult<T> {
        self.map_err(|e| {
            let mut err = f();
            err.source = Some(e.to_string());
            err
        })
    }
}

/// Error conversion implementations
// Note: sqlx conversions commented out as sqlx is not currently enabled
// Uncomment when sqlx is added back to dependencies
/*
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        let context = ErrorContext::new("database", "query");
        
        match &err {
            sqlx::Error::RowNotFound => {
                AppError::new(ErrorKind::NotFound, "Record not found", context)
            }
            sqlx::Error::Database(db_err) => {
                // Check for constraint violations
                if let Some(constraint) = db_err.constraint() {
                    AppError::new(
                        ErrorKind::Conflict,
                        format!("Database constraint violation: {}", constraint),
                        context
                    )
                } else {
                    AppError::new(ErrorKind::DatabaseError, db_err.to_string(), context)
                }
            }
            _ => AppError::new(ErrorKind::DatabaseError, err.to_string(), context)
        }.with_source(err)
    }
}
*/

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        let context = ErrorContext::new("cache", "redis_operation");
        AppError::new(ErrorKind::CacheError, err.to_string(), context)
            .with_source(err)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        let context = ErrorContext::new("http_client", "request");
        
        let kind = if err.is_timeout() {
            ErrorKind::Timeout
        } else if err.is_connect() {
            ErrorKind::ServiceUnavailable
        } else {
            ErrorKind::ExternalServiceError
        };
        
        AppError::new(kind, err.to_string(), context)
            .with_source(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        let context = ErrorContext::new("serialization", "json");
        AppError::new(ErrorKind::InvalidFormat, err.to_string(), context)
            .with_source(err)
    }
}

/// Macro for creating errors with context
#[macro_export]
macro_rules! app_error {
    ($kind:expr, $msg:expr) => {
        $crate::typed_errors::AppError::new(
            $kind,
            $msg,
            $crate::typed_errors::ErrorContext::new(module_path!(), "operation")
        )
    };
    ($kind:expr, $msg:expr, $ctx:expr) => {
        $crate::typed_errors::AppError::new($kind, $msg, $ctx)
    };
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let context = ErrorContext::new("test_service", "test_operation")
            .with_user("user123".to_string())
            .with_request_id("req123".to_string());
            
        let error = AppError::new(
            ErrorKind::ValidationError,
            "Invalid email format",
            context
        );
        
        assert_eq!(error.kind, ErrorKind::ValidationError);
        assert_eq!(error.message, "Invalid email format");
        assert_eq!(error.context.user_id, Some("user123".to_string()));
    }
    
    #[test]
    fn test_error_status_codes() {
        assert_eq!(ErrorKind::Unauthorized.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(ErrorKind::NotFound.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(ErrorKind::ValidationError.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(ErrorKind::InternalError.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    #[test]
    fn test_retryable_errors() {
        assert!(ErrorKind::ServiceUnavailable.is_retryable());
        assert!(ErrorKind::Timeout.is_retryable());
        assert!(!ErrorKind::ValidationError.is_retryable());
        assert!(!ErrorKind::Unauthorized.is_retryable());
    }
}