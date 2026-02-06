//! Tracing middleware for HTTP requests

use axum::{
    middleware::Next,
    response::{IntoResponse, Response},
    extract::{State, OriginalUri, ConnectInfo},
    http::{Request, HeaderMap, StatusCode},
    body::Body,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info_span, Span, info};

use crate::{
    tracing_logger::{TracingLogger, CorrelationId, RequestContext},
    typed_errors::AppError,
    AppState,
};

/// Extract correlation ID from headers or generate new one
fn extract_correlation_id(headers: &HeaderMap) -> CorrelationId {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| CorrelationId::from_string(s.to_string()))
        .unwrap_or_else(CorrelationId::new)
}

/// Tracing middleware for all HTTP requests
pub async fn tracing_middleware(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    mut req: Request<Body>,
    next: Next<Body>,
) -> Result<Response, AppError> {
    // Extract or generate correlation ID
    let correlation_id = extract_correlation_id(&headers);
    
    // Add correlation ID to request extensions
    req.extensions_mut().insert(correlation_id.clone());
    
    // Create request context
    let path = uri.path().to_string();
    let method = req.method().to_string();
    let logger = get_tracing_logger(&state)?;
    
    let mut context = logger.create_request_context(path.clone(), method.clone()).await;
    
    // Add request metadata
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| connect_info.map(|ConnectInfo(addr)| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    
    info!(
        correlation_id = %correlation_id,
        path = %path,
        method = %method,
        client_ip = %client_ip,
        user_agent = %user_agent,
        "Incoming request"
    );
    
    // Execute request with span
    let _enter = context.span.enter();
    let response = next.run(req).await;
    
    // Log response
    let status = response.status();
    let error = if status.is_server_error() {
        Some(format!("Server error: {}", status))
    } else if status.is_client_error() && status != StatusCode::UNAUTHORIZED {
        Some(format!("Client error: {}", status))
    } else {
        None
    };
    
    logger.complete_request_context(
        &correlation_id.0,
        status.as_u16(),
        error,
    ).await;
    
    // Add correlation ID to response headers
    let mut response = response;
    response.headers_mut().insert(
        "x-correlation-id",
        correlation_id.0.parse().unwrap(),
    );
    
    Ok(response)
}

/// Get tracing logger from app state
fn get_tracing_logger(state: &AppState) -> Result<&Arc<TracingLogger>, AppError> {
    state.tracing_logger.as_ref().ok_or_else(|| {
        AppError::new(
            crate::typed_errors::ErrorKind::ConfigurationError,
            "Tracing logger not configured",
            crate::typed_errors::ErrorContext::new("middleware", "tracing"),
        )
    })
}


/// Extension trait for adding correlation ID to requests
pub trait CorrelationIdExt {
    fn correlation_id(&self) -> Option<&CorrelationId>;
}

impl<B> CorrelationIdExt for Request<B> {
    fn correlation_id(&self) -> Option<&CorrelationId> {
        self.extensions().get::<CorrelationId>()
    }
}

/// Log database operations with correlation ID
#[macro_export]
macro_rules! log_db_operation {
    ($logger:expr, $correlation_id:expr, $query_type:expr, $table:expr, $operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $operation;
        let duration = start.elapsed();
        
        $logger.log_query(
            $query_type,
            $table,
            &$correlation_id.0,
            duration,
            result.is_ok(),
            result.as_ref().err().map(|e| e.to_string()),
        ).await;
        
        result
    }};
}

/// Log external API calls with correlation ID
#[macro_export]
macro_rules! log_api_call {
    ($logger:expr, $correlation_id:expr, $service:expr, $endpoint:expr, $method:expr, $operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $operation;
        let duration = start.elapsed();
        
        let (status_code, error) = match &result {
            Ok(response) => (Some(response.status().as_u16()), None),
            Err(e) => (None, Some(e.to_string())),
        };
        
        $logger.log_external_api_call(
            $service,
            $endpoint,
            $method,
            &$correlation_id.0,
            duration,
            status_code,
            error,
        ).await;
        
        result
    }};
}

/// Log Solana operations with correlation ID
#[macro_export]
macro_rules! log_solana_operation {
    ($logger:expr, $correlation_id:expr, $tx_type:expr, $operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $operation;
        let duration = start.elapsed();
        
        let (signature, success, error) = match &result {
            Ok(sig) => (Some(sig.to_string()), true, None),
            Err(e) => (None, false, Some(e.to_string())),
        };
        
        $logger.log_solana_transaction(
            $tx_type,
            signature.as_deref(),
            &$correlation_id.0,
            duration,
            success,
            error,
        ).await;
        
        result
    }};
}

/// Helper to extract correlation ID from request
pub async fn get_correlation_id<B>(req: &Request<B>) -> CorrelationId {
    req.extensions()
        .get::<CorrelationId>()
        .cloned()
        .unwrap_or_else(CorrelationId::new)
}

/// Helper to log with correlation context
pub async fn log_with_correlation<F, T>(
    logger: &TracingLogger,
    correlation_id: &CorrelationId,
    operation: &str,
    metadata: std::collections::HashMap<String, serde_json::Value>,
    f: F,
) -> Result<T, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    logger.log_operation(
        operation,
        &correlation_id.0,
        metadata,
        f,
    ).await
}
