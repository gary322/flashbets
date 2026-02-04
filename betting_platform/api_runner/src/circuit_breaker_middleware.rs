//! Circuit breaker middleware for HTTP endpoints and service calls

use axum::{
    middleware::Next,
    response::{IntoResponse, Response},
    http::Request,
    body::Body,
    extract::State,
};
use std::sync::Arc;
use std::time::Duration;
use std::str::FromStr;
use tracing::{warn, debug};

use crate::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerError, CircuitBreakerConfig, CircuitBreakerManager},
    typed_errors::{AppError, ErrorKind, ErrorContext},
    AppState,
};

/// Circuit breaker middleware for HTTP endpoints
pub async fn circuit_breaker_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next<Body>,
) -> Result<Response, AppError> {
    let path = req.uri().path().to_string();
    let method = req.method().to_string();
    let breaker_name = format!("http:{}:{}", method, path);
    
    // Get circuit breaker manager
    let manager = state.circuit_breaker_manager.as_ref()
        .ok_or_else(|| {
            AppError::new(
                ErrorKind::ConfigurationError,
                "Circuit breaker manager not configured",
                ErrorContext::new("middleware", "circuit_breaker"),
            )
        })?;
    
    // Get or create circuit breaker for this endpoint
    let breaker = manager.get_or_create(&breaker_name).await;
    
    // Execute request with circuit breaker
    match breaker.call(|| async { Ok::<_, std::convert::Infallible>(next.run(req).await) }).await {
        Ok(response) => Ok(response),
        Err(CircuitBreakerError::CircuitOpen) => {
            let context = ErrorContext::new("circuit_breaker", "http_endpoint")
                .with_metadata("endpoint", serde_json::json!(breaker_name));
            
            Err(AppError::new(
                ErrorKind::CircuitBreakerOpen,
                format!("Service temporarily unavailable: {}", path),
                context,
            ))
        }
        Err(CircuitBreakerError::OperationFailed(_)) => {
            // This should never happen with Infallible
            unreachable!("Infallible error should never occur")
        }
    }
}

/// Service-level circuit breakers
pub struct ServiceCircuitBreakers {
    database: Arc<CircuitBreaker>,
    redis: Arc<CircuitBreaker>,
    solana_rpc: Arc<CircuitBreaker>,
    external_api: Arc<CircuitBreaker>,
}

impl ServiceCircuitBreakers {
    pub fn new() -> Self {
        // Database circuit breaker - more lenient
        let db_config = CircuitBreakerConfig {
            failure_threshold: 10,
            success_threshold: 5,
            reset_timeout: Duration::from_secs(60),
            failure_rate_threshold: 0.7,
            slow_call_duration: Duration::from_secs(10),
            ..Default::default()
        };
        
        // Redis circuit breaker - fast recovery
        let redis_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            reset_timeout: Duration::from_secs(10),
            failure_rate_threshold: 0.6,
            slow_call_duration: Duration::from_secs(2),
            ..Default::default()
        };
        
        // Solana RPC circuit breaker
        let solana_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            reset_timeout: Duration::from_secs(30),
            failure_rate_threshold: 0.5,
            slow_call_duration: Duration::from_secs(5),
            ..Default::default()
        };
        
        // External API circuit breaker
        let external_config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            reset_timeout: Duration::from_secs(60),
            failure_rate_threshold: 0.4,
            slow_call_duration: Duration::from_secs(10),
            half_open_max_calls: 5,
            ..Default::default()
        };
        
        Self {
            database: Arc::new(CircuitBreaker::new("database", db_config)),
            redis: Arc::new(CircuitBreaker::new("redis", redis_config)),
            solana_rpc: Arc::new(CircuitBreaker::new("solana_rpc", solana_config)),
            external_api: Arc::new(CircuitBreaker::new("external_api", external_config)),
        }
    }
    
    /// Get database circuit breaker
    pub fn database(&self) -> &Arc<CircuitBreaker> {
        &self.database
    }
    
    /// Get Redis circuit breaker
    pub fn redis(&self) -> &Arc<CircuitBreaker> {
        &self.redis
    }
    
    /// Get Solana RPC circuit breaker
    pub fn solana_rpc(&self) -> &Arc<CircuitBreaker> {
        &self.solana_rpc
    }
    
    /// Get external API circuit breaker
    pub fn external_api(&self) -> &Arc<CircuitBreaker> {
        &self.external_api
    }
}

/// Execute database operation with circuit breaker
pub async fn with_database_circuit_breaker<F, Fut, T>(
    breaker: &Arc<CircuitBreaker>,
    operation: F,
) -> Result<T, AppError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    match breaker.call(operation).await {
        Ok(result) => Ok(result),
        Err(CircuitBreakerError::CircuitOpen) => {
            let context = ErrorContext::new("database", "circuit_breaker");
            Err(AppError::new(
                ErrorKind::CircuitBreakerOpen,
                "Database service temporarily unavailable",
                context,
            ))
        }
        Err(CircuitBreakerError::OperationFailed(err)) => Err(err),
    }
}

/// Execute Redis operation with circuit breaker
pub async fn with_redis_circuit_breaker<F, Fut, T>(
    breaker: &Arc<CircuitBreaker>,
    operation: F,
) -> Result<T, AppError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    match breaker.call(operation).await {
        Ok(result) => Ok(result),
        Err(CircuitBreakerError::CircuitOpen) => {
            let context = ErrorContext::new("redis", "circuit_breaker");
            Err(AppError::new(
                ErrorKind::CircuitBreakerOpen,
                "Cache service temporarily unavailable",
                context,
            ))
        }
        Err(CircuitBreakerError::OperationFailed(err)) => Err(err),
    }
}

/// Execute Solana RPC operation with circuit breaker
pub async fn with_solana_circuit_breaker<F, Fut, T>(
    breaker: &Arc<CircuitBreaker>,
    operation: F,
) -> Result<T, AppError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    match breaker.call(operation).await {
        Ok(result) => Ok(result),
        Err(CircuitBreakerError::CircuitOpen) => {
            let context = ErrorContext::new("solana_rpc", "circuit_breaker");
            Err(AppError::new(
                ErrorKind::CircuitBreakerOpen,
                "Blockchain service temporarily unavailable",
                context,
            ))
        }
        Err(CircuitBreakerError::OperationFailed(err)) => Err(err),
    }
}

/// Execute external API operation with circuit breaker
pub async fn with_external_api_circuit_breaker<F, Fut, T>(
    breaker: &Arc<CircuitBreaker>,
    operation: F,
) -> Result<T, AppError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    match breaker.call(operation).await {
        Ok(result) => Ok(result),
        Err(CircuitBreakerError::CircuitOpen) => {
            let context = ErrorContext::new("external_api", "circuit_breaker");
            Err(AppError::new(
                ErrorKind::CircuitBreakerOpen,
                "External API service temporarily unavailable",
                context,
            ))
        }
        Err(CircuitBreakerError::OperationFailed(err)) => Err(err),
    }
}

/// Circuit breaker health check endpoint
pub async fn circuit_breaker_health(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let manager = state.circuit_breaker_manager.as_ref()
        .ok_or_else(|| {
            AppError::new(
                ErrorKind::ConfigurationError,
                "Circuit breaker manager not configured",
                ErrorContext::new("health", "circuit_breaker"),
            )
        })?;
    
    let all_metrics = manager.all_metrics().await;
    
    // Get service breakers if available
    let service_metrics = if let Some(service_breakers) = &state.service_circuit_breakers {
        serde_json::json!({
            "database": {
                "state": format!("{:?}", service_breakers.database().state().await),
                "metrics": service_breakers.database().metrics(),
            },
            "redis": {
                "state": format!("{:?}", service_breakers.redis().state().await),
                "metrics": service_breakers.redis().metrics(),
            },
            "solana_rpc": {
                "state": format!("{:?}", service_breakers.solana_rpc().state().await),
                "metrics": service_breakers.solana_rpc().metrics(),
            },
            "external_api": {
                "state": format!("{:?}", service_breakers.external_api().state().await),
                "metrics": service_breakers.external_api().metrics(),
            },
        })
    } else {
        serde_json::json!(null)
    };
    
    Ok(axum::Json(serde_json::json!({
        "endpoint_breakers": all_metrics,
        "service_breakers": service_metrics,
        "timestamp": chrono::Utc::now(),
    })))
}

/// Reset circuit breakers endpoint (admin only)
pub async fn reset_circuit_breakers(
    State(state): State<AppState>,
    auth: crate::jwt_validation::AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    // Check admin permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &crate::rbac_authorization::Permission::UpdateSystemConfig) {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Admin access required",
            ErrorContext::new("circuit_breaker", "reset"),
        ));
    }
    
    // Reset endpoint breakers
    if let Some(manager) = &state.circuit_breaker_manager {
        manager.reset_all().await;
    }
    
    // Reset service breakers
    if let Some(service_breakers) = &state.service_circuit_breakers {
        service_breakers.database().reset().await;
        service_breakers.redis().reset().await;
        service_breakers.solana_rpc().reset().await;
        service_breakers.external_api().reset().await;
    }
    
    Ok(axum::Json(serde_json::json!({
        "success": true,
        "message": "All circuit breakers reset",
        "timestamp": chrono::Utc::now(),
    })))
}

/// Circuit breaker configuration for different services
pub fn create_default_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        reset_timeout: Duration::from_secs(30),
        half_open_max_calls: 3,
        failure_window: Duration::from_secs(60),
        min_calls: 10,
        failure_rate_threshold: 0.5,
        slow_call_duration: Duration::from_secs(5),
        slow_call_rate_threshold: 0.5,
    }
}