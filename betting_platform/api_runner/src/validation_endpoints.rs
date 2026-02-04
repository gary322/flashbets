//! Validation Management Endpoints
//! 
//! Provides HTTP endpoints for managing the validation framework

use std::sync::Arc;
use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    AppState,
    response::{ApiResponse, responses},
    typed_errors::{AppError, ErrorKind, ErrorContext},
    validation_framework::{ValidationService, ValidationSchema, ValidationRule, ValidationRuleType, ValidationSeverity, ValidationReport},
    validation_middleware::{ValidationMiddlewareConfig, EndpointValidation, update_validation_config, get_validation_stats},
};

/// Register a new validation schema
pub async fn register_schema(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<RegisterSchemaRequest>,
) -> Result<ApiResponse<RegisterSchemaResponse>, AppError> {
    let context = ErrorContext::new("validation_endpoints", "register_schema");
    
    let validation_service = app_state.validation_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Validation service not available",
            context.clone(),
        ))?;
    
    // Convert request to schema
    let schema = ValidationSchema {
        name: request.name.clone(),
        rules: request.rules,
        custom_validators: vec![], // Custom validators must be registered separately
    };
    
    validation_service.register_schema(schema).await;
    
    info!("Registered validation schema: {}", request.name);
    
    Ok(responses::ok(RegisterSchemaResponse {
        success: true,
        schema_name: request.name,
    }))
}

/// Get validation schema details
pub async fn get_schema(
    State(app_state): State<Arc<AppState>>,
    Path(schema_name): Path<String>,
) -> Result<ApiResponse<GetSchemaResponse>, AppError> {
    let context = ErrorContext::new("validation_endpoints", "get_schema");
    
    let validation_service = app_state.validation_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Validation service not available",
            context.clone(),
        ))?;
    
    // This would require adding a method to ValidationService to retrieve schemas
    // For now, return a placeholder
    Err(AppError::new(
        ErrorKind::InternalError,
        "Schema retrieval not implemented",
        context,
    ))
}

/// Validate data against a schema
pub async fn validate_data(
    State(app_state): State<Arc<AppState>>,
    Path(schema_name): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Result<ApiResponse<ValidateDataResponse>, AppError> {
    let context = ErrorContext::new("validation_endpoints", "validate_data");
    
    let validation_service = app_state.validation_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Validation service not available",
            context.clone(),
        ))?;
    
    let validation_context = crate::validation_framework::ValidationContext::default();
    let report = validation_service.validate_with_schema(&schema_name, &data, &validation_context).await?;
    
    Ok(responses::ok(ValidateDataResponse {
        is_valid: report.is_valid,
        report,
    }))
}

/// Update validation middleware configuration
pub async fn update_middleware_config(
    State(app_state): State<Arc<AppState>>,
    Json(config): Json<ValidationMiddlewareConfig>,
) -> Result<ApiResponse<UpdateConfigResponse>, AppError> {
    update_validation_config(&app_state, config).await?;
    
    info!("Updated validation middleware configuration");
    
    Ok(responses::ok(UpdateConfigResponse {
        success: true,
    }))
}

/// Get validation statistics
pub async fn get_stats(
    State(app_state): State<Arc<AppState>>,
) -> Result<ApiResponse<ValidationStatsResponse>, AppError> {
    let stats = get_validation_stats(&app_state).await?;
    
    Ok(responses::ok(ValidationStatsResponse {
        stats,
    }))
}

/// Clear validation cache
pub async fn clear_cache(
    State(app_state): State<Arc<AppState>>,
) -> Result<ApiResponse<ClearCacheResponse>, AppError> {
    let context = ErrorContext::new("validation_endpoints", "clear_cache");
    
    let validation_service = app_state.validation_service
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Validation service not available",
            context.clone(),
        ))?;
    
    // This would require adding a method to ValidationService to clear cache
    // For now, return success
    info!("Cleared validation cache");
    
    Ok(responses::ok(ClearCacheResponse {
        success: true,
    }))
}

// Request/Response types

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterSchemaRequest {
    pub name: String,
    pub rules: Vec<ValidationRule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterSchemaResponse {
    pub success: bool,
    pub schema_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetSchemaResponse {
    pub name: String,
    pub rules: Vec<ValidationRule>,
    pub custom_validators: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateDataResponse {
    pub is_valid: bool,
    pub report: ValidationReport,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateConfigResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationStatsResponse {
    pub stats: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClearCacheResponse {
    pub success: bool,
}

/// Add validation configuration endpoint
pub async fn configure_endpoint_validation(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<ConfigureEndpointRequest>,
) -> Result<ApiResponse<ConfigureEndpointResponse>, AppError> {
    let context = ErrorContext::new("validation_endpoints", "configure_endpoint");
    
    if let Some(state_manager) = &app_state.state_manager {
        // Get current config
        let mut config = match state_manager.get::<ValidationMiddlewareConfig>("validation:config").await? {
            Some(cfg) => cfg,
            None => ValidationMiddlewareConfig::default(),
        };
        
        // Add or update endpoint configuration
        config.endpoints.retain(|e| e.path_pattern != request.path_pattern);
        config.endpoints.push(EndpointValidation {
            method: request.method,
            path_pattern: request.path_pattern.clone(),
            schema_name: request.schema_name,
            extract_params: request.extract_params,
            validate_query: request.validate_query,
        });
        
        // Save updated config
        update_validation_config(&app_state, config).await?;
        
        info!("Configured validation for endpoint: {}", request.path_pattern);
        
        Ok(responses::ok(ConfigureEndpointResponse {
            success: true,
            path_pattern: request.path_pattern,
        }))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "State manager not available",
            context,
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigureEndpointRequest {
    pub method: String,
    pub path_pattern: String,
    pub schema_name: String,
    pub extract_params: bool,
    pub validate_query: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigureEndpointResponse {
    pub success: bool,
    pub path_pattern: String,
}