//! Error handling middleware for consistent error responses and logging

use axum::{
    middleware::Next,
    response::{IntoResponse, Response},
    http::{Request, StatusCode},
    body::Body,
};
use std::time::Instant;
use tracing::{error, warn, info_span, Instrument};
use uuid::Uuid;

use crate::typed_errors::{AppError, ErrorKind, ErrorContext};

/// Error handling middleware
pub async fn error_handling_middleware(
    req: Request<Body>,
    next: Next<Body>,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    
    // Create span for request tracking
    let span = info_span!(
        "http_request",
        request_id = %request_id,
        method = %method,
        path = %path,
    );
    
    // Add request ID to extensions for downstream use
    let req = {
        let mut req = req;
        req.extensions_mut().insert(RequestId(request_id.clone()));
        req
    };
    
    // Execute request
    let response = next.run(req).instrument(span.clone()).await;
    
    // Log request completion
    let duration = start.elapsed();
    let status = response.status();
    
    if status.is_server_error() {
        error!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request failed with server error"
        );
    } else if status.is_client_error() && status != StatusCode::NOT_FOUND {
        warn!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request failed with client error"
        );
    } else {
        tracing::debug!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "Request completed"
        );
    }
    
    Ok(response)
}

/// Request ID extension
#[derive(Clone)]
pub struct RequestId(pub String);

/// Error recovery middleware that catches panics
pub async fn panic_recovery_middleware(
    req: Request<Body>,
    next: Next<Body>,
) -> Response {
    let request_id = req.extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    
    // Catch panics and convert to errors
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tokio::runtime::Handle::current().block_on(async {
            next.run(req).await
        })
    }));
    
    match result {
        Ok(response) => response,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            
            error!(
                request_id = %request_id,
                panic_message = %message,
                "Request handler panicked"
            );
            
            let context = ErrorContext::new("http_handler", "request_processing")
                .with_request_id(request_id);
            
            let error = AppError::new(
                ErrorKind::InternalError,
                "Internal server error occurred",
                context
            );
            
            error.into_response()
        }
    }
}

/// Global error handler for unhandled errors
pub async fn global_error_handler(err: Box<dyn std::error::Error + Send + Sync>) -> Response {
    error!(
        error = %err,
        "Unhandled error in request processing"
    );
    
    let context = ErrorContext::new("global", "unhandled_error");
    let app_error = AppError::new(
        ErrorKind::InternalError,
        "An unexpected error occurred",
        context
    );
    
    app_error.into_response()
}

/// Timeout middleware wrapper
pub async fn timeout_middleware(
    req: Request<Body>,
    next: Next<Body>,
    timeout_duration: std::time::Duration,
) -> Result<Response, AppError> {
    let request_id = req.extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    
    match tokio::time::timeout(timeout_duration, next.run(req)).await {
        Ok(response) => Ok(response),
        Err(_) => {
            let context = ErrorContext::new("http_handler", "request_timeout")
                .with_request_id(request_id);
            
            Err(AppError::new(
                ErrorKind::Timeout,
                format!("Request timed out after {:?}", timeout_duration),
                context
            ))
        }
    }
}

/// Error metrics tracking
pub struct ErrorMetrics {
    pub total_errors: std::sync::atomic::AtomicU64,
    pub errors_by_kind: dashmap::DashMap<String, std::sync::atomic::AtomicU64>,
    pub errors_by_path: dashmap::DashMap<String, std::sync::atomic::AtomicU64>,
}

impl ErrorMetrics {
    pub fn new() -> Self {
        Self {
            total_errors: std::sync::atomic::AtomicU64::new(0),
            errors_by_kind: dashmap::DashMap::new(),
            errors_by_path: dashmap::DashMap::new(),
        }
    }
    
    pub fn record_error(&self, kind: ErrorKind, path: &str) {
        use std::sync::atomic::Ordering;
        
        self.total_errors.fetch_add(1, Ordering::Relaxed);
        
        let kind_str = format!("{:?}", kind).to_lowercase();
        self.errors_by_kind
            .entry(kind_str)
            .or_insert_with(|| std::sync::atomic::AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
            
        self.errors_by_path
            .entry(path.to_string())
            .or_insert_with(|| std::sync::atomic::AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_metrics(&self) -> serde_json::Value {
        use std::sync::atomic::Ordering;
        
        let errors_by_kind: std::collections::HashMap<String, u64> = self.errors_by_kind
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect();
            
        let errors_by_path: std::collections::HashMap<String, u64> = self.errors_by_path
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect();
        
        serde_json::json!({
            "total_errors": self.total_errors.load(Ordering::Relaxed),
            "errors_by_kind": errors_by_kind,
            "errors_by_path": errors_by_path,
        })
    }
}

/// Error logging middleware that tracks metrics
pub async fn error_metrics_middleware(
    req: Request<Body>,
    next: Next<Body>,
    metrics: std::sync::Arc<ErrorMetrics>,
) -> Response {
    let path = req.uri().path().to_string();
    let response = next.run(req).await;
    
    // Track error metrics
    if response.status().is_client_error() || response.status().is_server_error() {
        let kind = match response.status() {
            StatusCode::UNAUTHORIZED => ErrorKind::Unauthorized,
            StatusCode::FORBIDDEN => ErrorKind::Forbidden,
            StatusCode::NOT_FOUND => ErrorKind::NotFound,
            StatusCode::BAD_REQUEST => ErrorKind::ValidationError,
            StatusCode::CONFLICT => ErrorKind::Conflict,
            StatusCode::TOO_MANY_REQUESTS => ErrorKind::RateLimitExceeded,
            StatusCode::INTERNAL_SERVER_ERROR => ErrorKind::InternalError,
            StatusCode::SERVICE_UNAVAILABLE => ErrorKind::ServiceUnavailable,
            StatusCode::GATEWAY_TIMEOUT => ErrorKind::Timeout,
            _ => ErrorKind::InternalError,
        };
        
        metrics.record_error(kind, &path);
    }
    
    response
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use tower::ServiceExt;
    
    #[tokio::test]
    async fn test_error_metrics() {
        let metrics = ErrorMetrics::new();
        
        metrics.record_error(ErrorKind::NotFound, "/api/users/123");
        metrics.record_error(ErrorKind::ValidationError, "/api/trades");
        metrics.record_error(ErrorKind::NotFound, "/api/users/456");
        
        let stats = metrics.get_metrics();
        assert_eq!(stats["total_errors"], 3);
    }
}