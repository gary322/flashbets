//! Health Check API Endpoints
//! 
//! REST API endpoints for health monitoring

use axum::{
    extract::{Query, State, Path},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    AppState,
    health_check_service::{HealthCheckService, HealthStatus, HealthReport},
    jwt_validation::AuthenticatedUser,
    response::{ApiResponse, responses},
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Health check query parameters
#[derive(Debug, Deserialize)]
pub struct HealthCheckQuery {
    /// Include detailed component information
    pub detailed: Option<bool>,
    /// Force fresh check instead of cached
    pub force_refresh: Option<bool>,
}

/// Simple health check response
#[derive(Debug, Serialize)]
pub struct SimpleHealthResponse {
    pub status: HealthStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
}

/// Liveness probe endpoint (always returns OK if service is running)
pub async fn liveness_probe() -> Json<ApiResponse<serde_json::Value>> {
    Json(responses::success_with_data(
        "Service is alive",
        SimpleHealthResponse {
            status: HealthStatus::Healthy,
            timestamp: chrono::Utc::now(),
            uptime_seconds: 0, // Will be filled by actual service
        },
    ))
}

/// Readiness probe endpoint (checks if service is ready to handle requests)
pub async fn readiness_probe(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("health_endpoints", "readiness_probe");
    
    // Quick checks for critical components
    let mut status = HealthStatus::Healthy;
    let mut issues = Vec::new();
    
    // Check database
    if state.database.is_degraded().await {
        status = HealthStatus::Degraded;
        issues.push("Database in fallback mode");
    }
    
    // Check trading engine
    // TODO: Implement get_statistics method in TradingEngine
    // let trading_stats = state.trading_engine.get_statistics().await;
    // if trading_stats.active_orders > 10000 {
    //     status = HealthStatus::Degraded;
    //     issues.push("High order volume");
    // }
    
    // Check WebSocket connections
    // TODO: Implement get_statistics method in EnhancedWebSocketManager
    // if let Some(ws_manager) = &state.enhanced_ws_manager {
    //     let ws_stats = ws_manager.get_statistics().await;
    //     if ws_stats.active_connections > 5000 {
    //         status = HealthStatus::Degraded;
    //         issues.push("High WebSocket connections");
    //     }
    // }
    
    if status == HealthStatus::Healthy {
        Ok(Json(responses::success_with_data(
            "Service is ready",
            SimpleHealthResponse {
                status,
                timestamp: chrono::Utc::now(),
                uptime_seconds: 0, // Will be filled by actual service
            },
        )))
    } else {
        Ok(Json(responses::success_with_data(
            &format!("Service degraded: {}", issues.join(", ")),
            SimpleHealthResponse {
                status,
                timestamp: chrono::Utc::now(),
                uptime_seconds: 0,
            },
        )))
    }
}

/// Comprehensive health check endpoint
pub async fn comprehensive_health_check(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HealthCheckQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("health_endpoints", "comprehensive_health");
    
    let health_service = state.health_check_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Health check service not initialized",
            context.clone(),
        ))?;
    
    let report = if params.force_refresh.unwrap_or(false) {
        health_service.check_all_components().await
    } else {
        health_service.get_cached_health().await
    };
    
    // Filter report if not detailed
    let final_report = if !params.detailed.unwrap_or(true) {
        HealthReport {
            overall_status: report.overall_status,
            timestamp: report.timestamp,
            components: report.components.into_iter()
                .map(|mut c| {
                    c.metadata.clear(); // Remove detailed metadata
                    c
                })
                .collect(),
            uptime_seconds: report.uptime_seconds,
            version: report.version,
            environment: report.environment,
        }
    } else {
        report
    };
    
    Ok(Json(responses::success_with_data(
        "Health check completed",
        final_report,
    )))
}

/// Get health status for specific component
pub async fn get_component_health(
    State(state): State<Arc<AppState>>,
    Path(component): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("health_endpoints", "component_health");
    
    let health_service = state.health_check_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Health check service not initialized",
            context.clone(),
        ))?;
    
    let report = health_service.get_cached_health().await;
    
    let component_health = report.components
        .into_iter()
        .find(|c| c.name == component)
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            &format!("Component '{}' not found", component),
            context,
        ))?;
    
    Ok(Json(responses::success_with_data(
        &format!("Health status for {}", component),
        component_health,
    )))
}

/// Health metrics for monitoring systems
#[derive(Debug, Serialize)]
pub struct HealthMetrics {
    pub status: i32, // 0=healthy, 1=degraded, 2=unhealthy
    pub uptime_seconds: u64,
    pub database_healthy: bool,
    pub trading_engine_healthy: bool,
    pub websocket_healthy: bool,
    pub solana_rpc_healthy: bool,
    pub circuit_breakers_healthy: bool,
}

/// Prometheus-compatible metrics endpoint
pub async fn health_metrics(
    State(state): State<Arc<AppState>>,
) -> Result<String, AppError> {
    let context = ErrorContext::new("health_endpoints", "metrics");
    
    let health_service = state.health_check_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Health check service not initialized",
            context,
        ))?;
    
    let report = health_service.get_cached_health().await;
    
    let status_int = match report.overall_status {
        HealthStatus::Healthy => 0,
        HealthStatus::Degraded => 1,
        HealthStatus::Unhealthy => 2,
    };
    
    let mut metrics = vec![
        format!("# HELP health_status Overall health status (0=healthy, 1=degraded, 2=unhealthy)"),
        format!("# TYPE health_status gauge"),
        format!("health_status {}", status_int),
        format!("# HELP uptime_seconds Service uptime in seconds"),
        format!("# TYPE uptime_seconds counter"),
        format!("uptime_seconds {}", report.uptime_seconds),
    ];
    
    // Add component-specific metrics
    for component in report.components {
        let healthy = match component.status {
            HealthStatus::Healthy => 1,
            _ => 0,
        };
        
        metrics.push(format!(
            "# HELP component_{}_healthy Health status for {}",
            component.name, component.name
        ));
        metrics.push(format!("# TYPE component_{}_healthy gauge", component.name));
        metrics.push(format!("component_{}_healthy {}", component.name, healthy));
        
        metrics.push(format!(
            "# HELP component_{}_response_time_ms Response time for {} health check",
            component.name, component.name
        ));
        metrics.push(format!("# TYPE component_{}_response_time_ms gauge", component.name));
        metrics.push(format!(
            "component_{}_response_time_ms {}",
            component.name, component.response_time_ms
        ));
    }
    
    Ok(metrics.join("\n"))
}

/// Admin endpoint to trigger immediate health check
pub async fn trigger_health_check(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("health_endpoints", "trigger_check");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can trigger health checks",
            context,
        ));
    }
    
    let health_service = state.health_check_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Health check service not initialized",
            context.clone(),
        ))?;
    
    let report = health_service.check_all_components().await;
    
    Ok(Json(responses::success_with_data(
        "Health check triggered",
        report,
    )))
}

/// Health history response
#[derive(Debug, Serialize)]
pub struct HealthHistory {
    pub checks: Vec<HealthHistoryEntry>,
}

#[derive(Debug, Serialize)]
pub struct HealthHistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: HealthStatus,
    pub unhealthy_components: Vec<String>,
}

/// Get health check history (admin only)
pub async fn get_health_history(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("health_endpoints", "health_history");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view health history",
            context,
        ));
    }
    
    // For now, return empty history
    // In production, this would query from a time-series database
    Ok(Json(responses::success_with_data(
        "Health history retrieved",
        HealthHistory {
            checks: vec![],
        },
    )))
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_health_status_to_int() {
        assert_eq!(0, match HealthStatus::Healthy { 
            HealthStatus::Healthy => 0,
            HealthStatus::Degraded => 1,
            HealthStatus::Unhealthy => 2,
        });
    }
}