//! Validation Middleware
//! 
//! Provides automatic request validation based on endpoints and schemas

use std::{
    sync::Arc,
    collections::HashMap,
};
use axum::{
    extract::{State, Path},
    middleware::Next,
    response::Response,
    body::Body,
    http::{StatusCode, Request},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use tracing::{debug, warn, error};

use crate::{
    AppState,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    validation_framework::{ValidationService, ValidationContext, ValidationSeverity},
    response::{ApiResponse, responses},
    correlation_context::CorrelationContext,
};

/// Endpoint validation configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EndpointValidation {
    pub method: String,
    pub path_pattern: String,
    pub schema_name: String,
    pub extract_params: bool,
    pub validate_query: bool,
}

/// Validation middleware configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationMiddlewareConfig {
    pub enabled: bool,
    pub log_violations: bool,
    pub fail_on_warning: bool,
    pub endpoints: Vec<EndpointValidation>,
}

impl Default for ValidationMiddlewareConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_violations: true,
            fail_on_warning: false,
            endpoints: vec![
                EndpointValidation {
                    method: "POST".to_string(),
                    path_pattern: "/api/markets/create".to_string(),
                    schema_name: "market_creation".to_string(),
                    extract_params: false,
                    validate_query: false,
                },
                EndpointValidation {
                    method: "POST".to_string(),
                    path_pattern: "/api/trade/place".to_string(),
                    schema_name: "trade_execution".to_string(),
                    extract_params: false,
                    validate_query: false,
                },
                EndpointValidation {
                    method: "POST".to_string(),
                    path_pattern: "/api/auth/register".to_string(),
                    schema_name: "user_registration".to_string(),
                    extract_params: false,
                    validate_query: false,
                },
                EndpointValidation {
                    method: "POST".to_string(),
                    path_pattern: "/api/v2/orders".to_string(),
                    schema_name: "order_placement".to_string(),
                    extract_params: false,
                    validate_query: false,
                },
                EndpointValidation {
                    method: "POST".to_string(),
                    path_pattern: "/api/liquidity/provide".to_string(),
                    schema_name: "liquidity_provision".to_string(),
                    extract_params: false,
                    validate_query: false,
                },
                EndpointValidation {
                    method: "POST".to_string(),
                    path_pattern: "/api/settlement/create".to_string(),
                    schema_name: "settlement_creation".to_string(),
                    extract_params: false,
                    validate_query: false,
                },
            ],
        }
    }
}

/// Validation middleware
pub async fn validation_middleware(
    State(app_state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next<Body>,
) -> Result<Response, AppError> {
    let context = ErrorContext::new("validation_middleware", "process_request");
    
    // Check if validation service is available
    let validation_service = match &app_state.validation_service {
        Some(service) => service,
        None => {
            // Validation not configured, proceed without it
            return Ok(next.run(request).await);
        }
    };
    
    // Get configuration
    let config = get_validation_config(&app_state).await;
    if !config.enabled {
        return Ok(next.run(request).await);
    }
    
    // Check if this endpoint needs validation
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    
    let endpoint_config = config.endpoints.iter()
        .find(|e| e.method == method && path_matches(&path, &e.path_pattern));
    
    let endpoint_config = match endpoint_config {
        Some(config) => config,
        None => {
            // No validation configured for this endpoint
            return Ok(next.run(request).await);
        }
    };
    
    // Extract request body for validation
    let body_bytes = if endpoint_config.schema_name != "" {
        let (parts, body) = request.into_parts();
        
        // Use hyper's to_bytes for Body
        let bytes = hyper::body::to_bytes(body).await
            .map_err(|e| AppError::new(
                ErrorKind::InvalidInput,
                format!("Failed to read request body: {}", e),
                context.clone(),
            ))?;
        
        let body_clone = bytes.clone();
        request = Request::from_parts(parts, Body::from(bytes));
        Some(body_clone)
    } else {
        None
    };
    
    // Perform validation if body was extracted
    if let Some(body_bytes) = &body_bytes {
        // Parse JSON
        let json_value: serde_json::Value = serde_json::from_slice(&body_bytes)
            .map_err(|e| AppError::new(
                ErrorKind::InvalidInput,
                format!("Invalid JSON: {}", e),
                context.clone(),
            ))?;
        
        // Create validation context
        let correlation_id = request
            .extensions()
            .get::<CorrelationContext>()
            .map(|ctx| ctx.correlation_id.clone());
        
        let user_id = request
            .headers()
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        
        let validation_context = ValidationContext {
            user_id,
            request_id: correlation_id.map(|c| c.0),
            source: format!("{} {}", method, path),
            metadata: HashMap::new(),
        };
        
        // Validate against schema
        let report = validation_service
            .validate_with_schema(&endpoint_config.schema_name, &json_value, &validation_context)
            .await?;
        
        // Log violations if configured
        if config.log_violations && (!report.errors.is_empty() || !report.warnings.is_empty()) {
            warn!(
                "Validation violations for {} {}: {} errors, {} warnings",
                method, path, report.errors.len(), report.warnings.len()
            );
            
            for error in &report.errors {
                warn!("Validation error - {}: {}", error.field, error.message);
            }
            
            for warning in &report.warnings {
                debug!("Validation warning - {}: {}", warning.field, warning.message);
            }
        }
        
        // Check if request should be rejected
        if !report.is_valid || (config.fail_on_warning && !report.warnings.is_empty()) {
            let mut error_messages = Vec::new();
            
            for violation in report.errors {
                error_messages.push(format!("{}: {}", violation.field, violation.message));
            }
            
            if config.fail_on_warning {
                for violation in report.warnings {
                    error_messages.push(format!("{}: {} (warning)", violation.field, violation.message));
                }
            }
            
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Validation failed: {}", error_messages.join("; ")),
                context,
            ));
        }
        
        // Add validation report to request extensions
        request.extensions_mut().insert(report);
    }
    
    // Process request
    Ok(next.run(request).await)
}

/// Check if a path matches a pattern (simple pattern matching)
fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern.contains(':') {
        // Simple parameter matching
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();
        
        if pattern_parts.len() != path_parts.len() {
            return false;
        }
        
        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if !pattern_part.starts_with(':') && pattern_part != path_part {
                return false;
            }
        }
        
        true
    } else {
        path == pattern
    }
}

/// Get validation configuration
async fn get_validation_config(app_state: &AppState) -> ValidationMiddlewareConfig {
    // Check if config is in state manager
    if let Some(state_manager) = &app_state.state_manager {
        if let Ok(Some(config)) = state_manager.get::<ValidationMiddlewareConfig>("validation:config").await {
            return config;
        }
    }
    
    // Return default config
    ValidationMiddlewareConfig::default()
}

/// Initialize validation service with default schemas and validators
pub async fn initialize_validation_service(
    cache_enabled: bool,
    cache_ttl: std::time::Duration,
) -> Arc<ValidationService> {
    let service = Arc::new(ValidationService::new(cache_enabled, cache_ttl));
    
    // Initialize default schemas
    crate::validation_framework::initialize_default_schemas(&service).await;
    
    // Register domain validators
    let validators = crate::domain_validators::create_default_validators();
    for (i, validator) in validators.into_iter().enumerate() {
        service.register_validator(
            format!("domain_validator_{}", i),
            validator
        ).await;
    }
    
    // Register additional schemas for other endpoints
    register_additional_schemas(&service).await;
    
    service
}

/// Register additional validation schemas
async fn register_additional_schemas(service: &ValidationService) {
    use crate::validation_framework::{ValidationSchema, ValidationRule, ValidationRuleType, ValidationSeverity};
    
    // Order placement schema
    let order_schema = ValidationSchema {
        name: "order_placement".to_string(),
        rules: vec![
            ValidationRule {
                field: "market_id".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "side".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "size".to_string(),
                rule_type: ValidationRuleType::Range { min: Some(0.001), max: Some(1000000.0) },
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "order_type".to_string(),
                rule_type: ValidationRuleType::OneOf(vec![
                    "market".to_string(),
                    "limit".to_string(),
                    "stop".to_string(),
                    "stop_limit".to_string(),
                ]),
                message: None,
                severity: ValidationSeverity::Error,
            },
        ],
        custom_validators: vec![
            service.validators.read().await.get("domain_validator_1").cloned().unwrap_or_else(|| {
                Arc::new(crate::domain_validators::OrderValidator {
                    min_order_size: 1_000_000,
                    max_order_size: 100_000_000_000,
                    price_precision: 6,
                    max_price_deviation: 0.5,
                })
            }),
        ],
    };
    
    service.register_schema(order_schema).await;
    
    // Liquidity provision schema
    let liquidity_schema = ValidationSchema {
        name: "liquidity_provision".to_string(),
        rules: vec![
            ValidationRule {
                field: "market_id".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "amount".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "lock_period".to_string(),
                rule_type: ValidationRuleType::Range { min: Some(86400.0), max: None }, // Min 1 day
                message: Some("Lock period must be at least 1 day".into()),
                severity: ValidationSeverity::Error,
            },
        ],
        custom_validators: vec![],
    };
    
    service.register_schema(liquidity_schema).await;
    
    // Settlement creation schema
    let settlement_schema = ValidationSchema {
        name: "settlement_creation".to_string(),
        rules: vec![
            ValidationRule {
                field: "market_id".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "winning_outcome".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "oracle".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "proof".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
        ],
        custom_validators: vec![],
    };
    
    service.register_schema(settlement_schema).await;
}

/// Update validation configuration
pub async fn update_validation_config(
    app_state: &AppState,
    config: ValidationMiddlewareConfig,
) -> Result<(), AppError> {
    let context = ErrorContext::new("validation_middleware", "update_config");
    
    if let Some(state_manager) = &app_state.state_manager {
        state_manager.set("validation:config", config, "validation_middleware").await?;
        debug!("Validation configuration updated");
        Ok(())
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "State manager not available",
            context,
        ))
    }
}

/// Get validation statistics
pub async fn get_validation_stats(app_state: &AppState) -> Result<serde_json::Value, AppError> {
    let context = ErrorContext::new("validation_middleware", "get_stats");
    
    if let Some(state_manager) = &app_state.state_manager {
        let keys = state_manager.get_keys_by_prefix("validation:stats:").await;
        let mut stats = serde_json::Map::new();
        
        for key in keys {
            if let Ok(Some(value)) = state_manager.get::<serde_json::Value>(&key).await {
                let endpoint = key.strip_prefix("validation:stats:").unwrap_or(&key);
                stats.insert(endpoint.to_string(), value);
            }
        }
        
        Ok(serde_json::Value::Object(stats))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "State manager not available",
            context,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path_matching() {
        assert!(path_matches("/api/markets/create", "/api/markets/create"));
        assert!(path_matches("/api/markets/123", "/api/markets/:id"));
        assert!(path_matches("/api/markets/123/orders", "/api/markets/:id/orders"));
        assert!(!path_matches("/api/markets", "/api/markets/:id"));
        assert!(!path_matches("/api/users/123", "/api/markets/:id"));
    }
}