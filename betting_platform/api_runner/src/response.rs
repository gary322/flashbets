//! Standardized API response format

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Serialize};
use serde_json::{json, Value};
use std::fmt::Display;

/// Standard API response structure
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
    pub meta: Option<Value>,
}

/// API error structure
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
        }
    }

    /// Create a successful response with metadata
    pub fn success_with_meta(data: T, meta: Value) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
            meta: Some(meta),
        }
    }

    /// Create an error response
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
            meta: None,
        }
    }

    /// Create an error response with details
    pub fn error_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                details: Some(details),
            }),
            meta: None,
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = if self.success {
            StatusCode::OK
        } else {
            match self.error.as_ref().map(|e| e.code.as_str()) {
                Some("BAD_REQUEST") | Some("INVALID_REQUEST") | Some("VALIDATION_ERROR") => StatusCode::BAD_REQUEST,
                Some("UNAUTHORIZED") => StatusCode::UNAUTHORIZED,
                Some("FORBIDDEN") => StatusCode::FORBIDDEN,
                Some("NOT_FOUND") => StatusCode::NOT_FOUND,
                Some("CONFLICT") => StatusCode::CONFLICT,
                Some("RATE_LIMITED") => StatusCode::TOO_MANY_REQUESTS,
                Some("SERVICE_UNAVAILABLE") => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        };

        (status, Json(self)).into_response()
    }
}

/// Helper functions for common responses
pub mod responses {
    use super::*;

    /// Success response with data
    pub fn ok<T: Serialize>(data: T) -> ApiResponse<T> {
        ApiResponse::success(data)
    }

    /// Success response with data and metadata
    pub fn ok_with_meta<T: Serialize>(data: T, meta: Value) -> ApiResponse<T> {
        ApiResponse::success_with_meta(data, meta)
    }
    
    /// Success response with message and data
    pub fn success_with_data<T: Serialize>(message: impl Into<String>, data: T) -> ApiResponse<Value> {
        ApiResponse {
            success: true,
            data: Some(json!({
                "message": message.into(),
                "data": data
            })),
            error: None,
            meta: None,
        }
    }

    /// Created response (201)
    pub fn created<T: Serialize>(data: T) -> Response {
        let response = ApiResponse::success(data);
        (StatusCode::CREATED, Json(response)).into_response()
    }

    /// No content response (204)
    pub fn no_content() -> Response {
        StatusCode::NO_CONTENT.into_response()
    }

    /// Bad request error
    pub fn bad_request(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse::<()>::error("BAD_REQUEST", message)
    }

    /// Validation error
    pub fn validation_error(errors: Value) -> ApiResponse<()> {
        ApiResponse::<()>::error_with_details("VALIDATION_ERROR", "Validation failed", errors)
    }

    /// Not found error
    pub fn not_found(resource: impl Display) -> ApiResponse<()> {
        ApiResponse::<()>::error("NOT_FOUND", format!("{} not found", resource))
    }

    /// Unauthorized error
    pub fn unauthorized(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse::<()>::error("UNAUTHORIZED", message)
    }

    /// Forbidden error
    pub fn forbidden(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse::<()>::error("FORBIDDEN", message)
    }

    /// Conflict error
    pub fn conflict(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse::<()>::error("CONFLICT", message)
    }

    /// Internal server error
    pub fn internal_error(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse::<()>::error("INTERNAL_ERROR", message)
    }

    /// Rate limited error
    pub fn rate_limited() -> ApiResponse<()> {
        ApiResponse::<()>::error("RATE_LIMITED", "Too many requests")
    }
    
    /// Service unavailable error
    pub fn service_unavailable(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", message)
    }
}

/// Convert common errors to API responses
impl From<anyhow::Error> for ApiResponse<()> {
    fn from(err: anyhow::Error) -> Self {
        ApiResponse::<()>::error("INTERNAL_ERROR", err.to_string())
    }
}

impl From<solana_client::client_error::ClientError> for ApiResponse<()> {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        ApiResponse::<()>::error("BLOCKCHAIN_ERROR", err.to_string())
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response() {
        let response = ApiResponse::success(json!({ "id": 1, "name": "Test" }));
        assert!(response.success);
        assert!(response.data.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_error_response() {
        let response = ApiResponse::<()>::error("TEST_ERROR", "Test error message");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
        
        let error = response.error.unwrap();
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "Test error message");
    }

    #[test]
    fn test_status_code_mapping() {
        let test_cases = vec![
            ("BAD_REQUEST", StatusCode::BAD_REQUEST),
            ("UNAUTHORIZED", StatusCode::UNAUTHORIZED),
            ("NOT_FOUND", StatusCode::NOT_FOUND),
            ("RATE_LIMITED", StatusCode::TOO_MANY_REQUESTS),
            ("UNKNOWN_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
        ];

        for (code, expected_status) in test_cases {
            let response = ApiResponse::<()>::error(code, "test");
            let http_response = response.into_response();
            assert_eq!(http_response.status(), expected_status);
        }
    }
}