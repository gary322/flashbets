//! Input validation middleware

use axum::{
    body::HttpBody,
    extract::{rejection::JsonRejection, FromRequest},
    http::{StatusCode, Request},
    response::{IntoResponse, Response},
    Json,
    BoxError,
};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::str::FromStr;
use validator::{Validate, ValidationError, ValidationErrors};

/// A validated JSON extractor that ensures input validation
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

/// Rejection type for validation errors
#[derive(Debug)]
pub enum ValidationRejection {
    JsonRejection(JsonRejection),
    ValidationError(ValidationErrors),
}

impl IntoResponse for ValidationRejection {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ValidationRejection::JsonRejection(rejection) => {
                let message = match rejection {
                    JsonRejection::JsonDataError(_) => "Invalid JSON format",
                    JsonRejection::JsonSyntaxError(_) => "JSON syntax error",
                    JsonRejection::MissingJsonContentType(_) => "Missing Content-Type: application/json header",
                    _ => "Bad request",
                };
                (StatusCode::BAD_REQUEST, message.to_string())
            }
            ValidationRejection::ValidationError(errors) => {
                let mut messages = Vec::new();
                for (field, errors) in errors.field_errors() {
                    for error in errors {
                        messages.push(format!("{}: {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
                (StatusCode::BAD_REQUEST, messages.join(", "))
            }
        };

        let body = Json(serde_json::json!({
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

#[axum::async_trait]
impl<T, S, B> FromRequest<S, B> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    Json<T>: FromRequest<S, B, Rejection = JsonRejection>,
{
    type Rejection = ValidationRejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(ValidationRejection::JsonRejection)?;

        value
            .validate()
            .map_err(ValidationRejection::ValidationError)?;

        Ok(ValidatedJson(value))
    }
}

/// Common validation functions
pub mod validators {
    use super::*;
    use regex::Regex;
    use solana_sdk::pubkey::Pubkey;

    /// Validate Solana public key
    pub fn validate_pubkey(pubkey: &str) -> Result<(), ValidationError> {
        Pubkey::from_str(pubkey)
            .map(|_| ())
            .map_err(|_| ValidationError::new("invalid_pubkey"))
    }

    /// Validate positive number
    pub fn validate_positive(value: &f64) -> Result<(), ValidationError> {
        if *value > 0.0 {
            Ok(())
        } else {
            Err(ValidationError::new("must_be_positive"))
        }
    }

    /// Validate percentage (0-100)
    pub fn validate_percentage(value: &f64) -> Result<(), ValidationError> {
        if *value >= 0.0 && *value <= 100.0 {
            Ok(())
        } else {
            Err(ValidationError::new("invalid_percentage"))
        }
    }

    /// Validate leverage (1-500)
    pub fn validate_leverage(value: &u32) -> Result<(), ValidationError> {
        if *value >= 1 && *value <= 500 {
            Ok(())
        } else {
            Err(ValidationError::new("invalid_leverage"))
        }
    }

    /// Validate market ID
    pub fn validate_market_id(value: &u128) -> Result<(), ValidationError> {
        if *value > 0 {
            Ok(())
        } else {
            Err(ValidationError::new("invalid_market_id"))
        }
    }

    /// Validate email
    pub fn validate_email(email: &str) -> Result<(), ValidationError> {
        let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
        if email_regex.is_match(email) {
            Ok(())
        } else {
            Err(ValidationError::new("invalid_email"))
        }
    }

    /// Validate non-empty string
    pub fn validate_non_empty(value: &str) -> Result<(), ValidationError> {
        if !value.trim().is_empty() {
            Ok(())
        } else {
            Err(ValidationError::new("cannot_be_empty"))
        }
    }

    /// Validate timestamp
    pub fn validate_future_timestamp(timestamp: &i64) -> Result<(), ValidationError> {
        let now = chrono::Utc::now().timestamp();
        if *timestamp > now {
            Ok(())
        } else {
            Err(ValidationError::new("must_be_future_timestamp"))
        }
    }
}

/// Validation rules for common request types
#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[derive(Debug, Serialize, serde::Deserialize)]
    struct TestRequest {
        wallet: String,
        amount: f64,
        leverage: u32,
    }

    #[test]
    fn test_valid_request() {
        let request = TestRequest {
            wallet: "11111111111111111111111111111111".to_string(),
            amount: 100.0,
            leverage: 10,
        };
        
        // Validate manually
        assert!(validators::validate_pubkey(&request.wallet).is_ok());
        assert!(validators::validate_positive(&request.amount).is_ok());
        assert!(validators::validate_leverage(&request.leverage).is_ok());
    }

    #[test]
    fn test_invalid_wallet() {
        let request = TestRequest {
            wallet: "invalid".to_string(),
            amount: 100.0,
            leverage: 10,
        };
        
        // Validate manually - should fail
        assert!(validators::validate_pubkey(&request.wallet).is_err() ||
                validators::validate_positive(&request.amount).is_err() ||
                validators::validate_leverage(&request.leverage).is_err());
    }

    #[test]
    fn test_invalid_amount() {
        let request = TestRequest {
            wallet: "11111111111111111111111111111111".to_string(),
            amount: -100.0,
            leverage: 10,
        };
        
        // Validate manually - should fail
        assert!(validators::validate_pubkey(&request.wallet).is_err() ||
                validators::validate_positive(&request.amount).is_err() ||
                validators::validate_leverage(&request.leverage).is_err());
    }

    #[test]
    fn test_invalid_leverage() {
        let request = TestRequest {
            wallet: "11111111111111111111111111111111".to_string(),
            amount: 100.0,
            leverage: 1000,
        };
        
        // Validate manually - should fail
        assert!(validators::validate_pubkey(&request.wallet).is_err() ||
                validators::validate_positive(&request.amount).is_err() ||
                validators::validate_leverage(&request.leverage).is_err());
    }
}