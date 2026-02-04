//! Domain-specific error handlers and conversions

use crate::typed_errors::{AppError, ErrorKind, ErrorContext};
use solana_sdk::program_error::ProgramError;
use solana_client::client_error::ClientError;
use std::num::ParseIntError;
use rust_decimal::Error as DecimalError;

/// Trading-specific errors
pub fn handle_trading_error(err: &str, market_id: u128, user: Option<&str>) -> AppError {
    let mut context = ErrorContext::new("trading_engine", "order_processing");
    
    if let Some(user_id) = user {
        context = context.with_user(user_id.to_string());
    }
    
    context = context.with_metadata("market_id", serde_json::json!(market_id));
    
    match err {
        "insufficient_balance" => AppError::new(
            ErrorKind::InsufficientBalance,
            "Insufficient balance to place order",
            context
        ),
        "market_closed" => AppError::new(
            ErrorKind::MarketClosed,
            format!("Market {} is closed for trading", market_id),
            context
        ),
        "order_size_too_small" => AppError::validation(
            "size",
            "Order size below minimum",
            context
        ),
        "order_size_too_large" => AppError::validation(
            "size",
            "Order size exceeds maximum",
            context
        ),
        "invalid_price" => AppError::validation(
            "price",
            "Invalid price for order",
            context
        ),
        _ => AppError::new(
            ErrorKind::OrderRejected,
            format!("Order rejected: {}", err),
            context
        ),
    }
}

/// Solana-specific error conversions
impl From<ClientError> for AppError {
    fn from(err: ClientError) -> Self {
        let context = ErrorContext::new("solana_rpc", "blockchain_operation");
        
        // ClientError doesn't expose its variants directly in newer versions
        // We'll use the error message to determine the type
        let err_str = err.to_string();
        
        let app_error = if err_str.contains("timeout") {
            AppError::new(
                ErrorKind::Timeout,
                "Solana RPC request timeout",
                context
            )
        } else if err_str.contains("connection") || err_str.contains("io error") {
            AppError::new(
                ErrorKind::ServiceUnavailable,
                "Solana RPC connection failed",
                context
            )
        } else if err_str.contains("json") || err_str.contains("parse") {
            AppError::new(
                ErrorKind::InvalidFormat,
                format!("JSON parsing error: {}", err),
                context
            )
        } else {
            AppError::new(
                ErrorKind::SolanaRpcError,
                format!("Solana client error: {}", err),
                context
            )
        };
        
        app_error.with_source(err)
    }
}

impl From<ProgramError> for AppError {
    fn from(err: ProgramError) -> Self {
        let context = ErrorContext::new("solana_program", "instruction_execution");
        
        let (kind, message) = match &err {
            ProgramError::InsufficientFunds => (
                ErrorKind::InsufficientBalance,
                "Insufficient SOL for transaction".to_string()
            ),
            ProgramError::InvalidAccountData => (
                ErrorKind::InvalidInput,
                "Invalid account data".to_string()
            ),
            ProgramError::Custom(code) => (
                ErrorKind::SolanaRpcError,
                format!("Program error: {}", code)
            ),
            _ => (
                ErrorKind::SolanaRpcError,
                format!("Unknown program error: {:?}", err)
            ),
        };
        
        AppError::new(kind, &message, context)
            .with_metadata("program_error", serde_json::json!(format!("{:?}", err)))
    }
}

/// Market data errors
pub fn handle_market_data_error(err: anyhow::Error, operation: &str) -> AppError {
    let context = ErrorContext::new("market_data", operation);
    
    let err_string = err.to_string();
    
    if err_string.contains("not found") {
        AppError::new(
            ErrorKind::NotFound,
            "Market not found",
            context
        )
    } else if err_string.contains("connection") || err_string.contains("timeout") {
        AppError::new(
            ErrorKind::ExternalServiceError,
            "External market data service unavailable",
            context
        )
    } else {
        AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Market data error: {}", err_string),
            context
        )
    }.with_metadata("original_error", serde_json::json!(err.to_string()))
}

/// Validation error builders
pub struct ValidationErrorBuilder {
    errors: Vec<(String, String)>,
    context: ErrorContext,
}

impl ValidationErrorBuilder {
    pub fn new(operation: &str) -> Self {
        Self {
            errors: Vec::new(),
            context: ErrorContext::new("validation", operation),
        }
    }
    
    pub fn add_field_error(mut self, field: &str, message: &str) -> Self {
        self.errors.push((field.to_string(), message.to_string()));
        self
    }
    
    pub fn with_user(mut self, user_id: String) -> Self {
        self.context = self.context.with_user(user_id);
        self
    }
    
    pub fn build(self) -> AppError {
        if self.errors.is_empty() {
            return AppError::new(
                ErrorKind::ValidationError,
                "Validation failed",
                self.context
            );
        }
        
        let mut error = AppError::new(
            ErrorKind::ValidationError,
            "Validation failed for multiple fields",
            self.context
        );
        
        error = error.with_metadata(
            "validation_errors",
            serde_json::json!(self.errors.iter().map(|(field, msg)| {
                serde_json::json!({
                    "field": field,
                    "message": msg
                })
            }).collect::<Vec<_>>())
        );
        
        error
    }
}

/// Parse errors
impl From<ParseIntError> for AppError {
    fn from(err: ParseIntError) -> Self {
        let context = ErrorContext::new("parsing", "integer_conversion");
        AppError::new(
            ErrorKind::InvalidFormat,
            format!("Invalid integer format: {}", err),
            context
        ).with_source(err)
    }
}

impl From<DecimalError> for AppError {
    fn from(err: DecimalError) -> Self {
        let context = ErrorContext::new("parsing", "decimal_conversion");
        AppError::new(
            ErrorKind::InvalidFormat,
            format!("Invalid decimal format: {}", err),
            context
        ).with_source(err)
    }
}

/// WebSocket errors
pub fn handle_websocket_error(err: axum::Error) -> AppError {
    let context = ErrorContext::new("websocket", "connection");
    
    AppError::new(
        ErrorKind::ServiceUnavailable,
        format!("WebSocket error: {}", err),
        context
    )
}

/// Database constraint errors
pub fn handle_db_constraint_error(constraint: &str, operation: &str) -> AppError {
    let context = ErrorContext::new("database", operation);
    
    match constraint {
        "users_email_key" => AppError::new(
            ErrorKind::AlreadyExists,
            "Email address already registered",
            context
        ).with_metadata("field", serde_json::json!("email")),
        
        "users_wallet_key" => AppError::new(
            ErrorKind::AlreadyExists,
            "Wallet address already registered",
            context
        ).with_metadata("field", serde_json::json!("wallet")),
        
        "positions_user_market_unique" => AppError::new(
            ErrorKind::AlreadyExists,
            "Position already exists for this market",
            context
        ),
        
        _ => AppError::new(
            ErrorKind::Conflict,
            format!("Database constraint violation: {}", constraint),
            context
        ),
    }
}

/// Rate limit error helper
pub fn rate_limit_error(
    limit_type: &str,
    limit: u32,
    window_seconds: u64,
    user: Option<&str>,
) -> AppError {
    let mut context = ErrorContext::new("rate_limiter", limit_type);
    
    if let Some(user_id) = user {
        context = context.with_user(user_id.to_string());
    }
    
    AppError::rate_limit(limit, window_seconds, context)
        .with_metadata("limit_type", serde_json::json!(limit_type))
}

/// Circuit breaker error
pub fn circuit_breaker_error(service: &str, recovery_time: Option<u64>) -> AppError {
    let context = ErrorContext::new("circuit_breaker", service);
    
    let mut error = AppError::new(
        ErrorKind::CircuitBreakerOpen,
        format!("Service {} is temporarily unavailable", service),
        context
    );
    
    if let Some(seconds) = recovery_time {
        error = error.with_metadata("retry_after_seconds", serde_json::json!(seconds));
    }
    
    error
}

/// Error result extension for adding context
pub trait ErrorContextExt<T> {
    fn with_context_fn<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> ErrorContext;
}

impl<T, E> ErrorContextExt<T> for Result<T, E>
where
    E: Into<AppError>,
{
    fn with_context_fn<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> ErrorContext,
    {
        self.map_err(|e| {
            let mut err: AppError = e.into();
            err.context = f();
            err
        })
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validation_error_builder() {
        let error = ValidationErrorBuilder::new("create_order")
            .add_field_error("price", "Price must be positive")
            .add_field_error("quantity", "Quantity too large")
            .with_user("user123".to_string())
            .build();
            
        assert_eq!(error.kind, ErrorKind::ValidationError);
        assert!(error.context.user_id.is_some());
    }
    
    #[test]
    fn test_trading_error_handling() {
        let error = handle_trading_error("insufficient_balance", 12345, Some("user123"));
        assert_eq!(error.kind, ErrorKind::InsufficientBalance);
        
        let error = handle_trading_error("market_closed", 12345, None);
        assert_eq!(error.kind, ErrorKind::MarketClosed);
    }
}