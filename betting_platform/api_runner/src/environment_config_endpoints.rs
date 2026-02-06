//! Environment Configuration API Endpoints
//! 
//! REST API endpoints for configuration management

use axum::{
    extract::{Query, State, Path},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;

use crate::{
    AppState,
    environment_config::{EnvironmentConfigService, Config, ConfigFormat, Environment},
    jwt_validation::AuthenticatedUser,
    response::{ApiResponse, responses},
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Configuration query parameters
#[derive(Debug, Deserialize)]
pub struct ConfigQuery {
    /// Include sensitive values (admin only)
    pub include_sensitive: Option<bool>,
    /// Export format
    pub format: Option<String>,
}

/// Configuration response
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub environment: Environment,
    pub config: serde_json::Value,
    pub overrides: HashMap<String, serde_json::Value>,
    pub sources: HashMap<String, String>,
}

/// Get current configuration (admin only)
pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(params): Query<ConfigQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "get_config");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view configuration",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    let config = config_service.get_config().await;
    let mut config_json = serde_json::to_value(&config)
        .map_err(|e| AppError::new(
            ErrorKind::InternalError,
            &format!("Failed to serialize config: {}", e),
            context.clone(),
        ))?;
    
    // Remove sensitive values unless explicitly requested
    if !params.include_sensitive.unwrap_or(false) {
        if let Some(obj) = config_json.as_object_mut() {
            // Redact sensitive fields
            if let Some(security) = obj.get_mut("security").and_then(|v| v.as_object_mut()) {
                security.insert("jwt_secret".to_string(), serde_json::json!("[REDACTED]"));
            }
            if let Some(database) = obj.get_mut("database").and_then(|v| v.as_object_mut()) {
                if let Some(url) = database.get("url").and_then(|v| v.as_str()) {
                    // Redact password in database URL
                    let redacted = redact_database_url(url);
                    database.insert("url".to_string(), serde_json::json!(redacted));
                }
            }
            if let Some(redis) = obj.get_mut("redis").and_then(|v| v.as_object_mut()) {
                if let Some(url) = redis.get("url").and_then(|v| v.as_str()) {
                    let redacted = redact_redis_url(url);
                    redis.insert("url".to_string(), serde_json::json!(redacted));
                }
            }
        }
    }
    
    Ok(Json(responses::success_with_data(
        "Configuration retrieved",
        ConfigResponse {
            environment: config.environment,
            config: config_json,
            overrides: HashMap::new(), // TODO: Get from service
            sources: HashMap::new(), // TODO: Track config sources
        },
    )))
}

/// Get specific configuration value
pub async fn get_config_value(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "get_config_value");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view configuration",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    // Check if key is sensitive
    let sensitive_keys = vec![
        "security.jwt_secret",
        "database.url",
        "redis.url",
        "external_apis.polymarket.api_key",
    ];
    
    if sensitive_keys.contains(&key.as_str()) {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Cannot retrieve sensitive configuration values through this endpoint",
            context,
        ));
    }
    
    let value: serde_json::Value = config_service.get(&key).await?;
    
    Ok(Json(responses::success_with_data(
        &format!("Configuration value for '{}'", key),
        value,
    )))
}

/// Configuration update request
#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub reason: String,
}

/// Set configuration override (admin only)
pub async fn set_config_override(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "set_config_override");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can modify configuration",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    // Log configuration change
    tracing::warn!(
        "Configuration override set by {}: {} = {:?} (reason: {})",
        user.claims.wallet,
        request.key,
        request.value,
        request.reason
    );
    
    config_service.set_override(&request.key, request.value).await?;
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": format!("Configuration override set for '{}'", request.key),
    }))))
}

/// Reload configuration from disk (admin only)
pub async fn reload_config(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "reload_config");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can reload configuration",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    config_service.reload().await?;
    
    tracing::info!("Configuration reloaded by {}", user.claims.wallet);
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": "Configuration reloaded successfully",
    }))))
}

/// Export configuration (admin only)
pub async fn export_config(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(params): Query<ConfigQuery>,
) -> Result<String, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "export_config");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can export configuration",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    let format = match params.format.as_deref() {
        Some("json") => ConfigFormat::Json,
        Some("yaml") => ConfigFormat::Yaml,
        Some("toml") | None => ConfigFormat::Toml,
        Some(f) => return Err(AppError::new(
            ErrorKind::InvalidInput,
            &format!("Unsupported format: {}", f),
            context,
        )),
    };
    
    let exported = config_service.export(format).await?;
    
    Ok(exported)
}

/// Configuration diff response
#[derive(Debug, Serialize)]
pub struct ConfigDiffResponse {
    pub changes: Vec<ConfigChange>,
    pub total_changes: usize,
}

#[derive(Debug, Serialize)]
pub struct ConfigChange {
    pub key: String,
    pub current: serde_json::Value,
    pub default: serde_json::Value,
}

/// Get configuration diff from defaults (admin only)
pub async fn get_config_diff(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "get_config_diff");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view configuration diff",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    let diff = config_service.get_diff().await;
    
    let changes: Vec<ConfigChange> = diff.into_iter()
        .map(|(key, (current, default))| ConfigChange {
            key,
            current,
            default,
        })
        .collect();
    
    let total_changes = changes.len();
    
    Ok(Json(responses::success_with_data(
        "Configuration diff retrieved",
        ConfigDiffResponse {
            changes,
            total_changes,
        },
    )))
}

/// Validate configuration (admin only)
pub async fn validate_config(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("environment_config_endpoints", "validate_config");
    
    // Check admin permission
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can validate configuration",
            context,
        ));
    }
    
    let config_service = state.environment_config
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Configuration service not initialized",
            context.clone(),
        ))?;
    
    let config = config_service.get_config().await;
    
    let mut validation_results = Vec::new();
    
    // Check database connectivity
    validation_results.push(ConfigValidationResult {
        component: "database".to_string(),
        valid: !config.database.url.is_empty(),
        message: if config.database.url.is_empty() {
            Some("Database URL is not configured".to_string())
        } else {
            None
        },
    });
    
    // Check Redis connectivity
    validation_results.push(ConfigValidationResult {
        component: "redis".to_string(),
        valid: !config.redis.url.is_empty(),
        message: if config.redis.url.is_empty() {
            Some("Redis URL is not configured".to_string())
        } else {
            None
        },
    });
    
    // Check Solana configuration
    validation_results.push(ConfigValidationResult {
        component: "solana".to_string(),
        valid: !config.solana.program_id.is_empty() && config.solana.program_id != "11111111111111111111111111111111",
        message: if config.solana.program_id.is_empty() {
            Some("Solana program ID is not configured".to_string())
        } else if config.solana.program_id == "11111111111111111111111111111111" {
            Some("Solana program ID is using default value".to_string())
        } else {
            None
        },
    });
    
    // Check security configuration
    validation_results.push(ConfigValidationResult {
        component: "security".to_string(),
        valid: config.security.jwt_secret != "change-me-in-production" || config.environment != Environment::Production,
        message: if config.security.jwt_secret == "change-me-in-production" && config.environment == Environment::Production {
            Some("JWT secret must be changed in production".to_string())
        } else {
            None
        },
    });
    
    let is_valid = validation_results.iter().all(|r| r.valid);
    
    Ok(Json(responses::success_with_data(
        "Configuration validated",
        ConfigValidationResponse {
            valid: is_valid,
            results: validation_results,
        },
    )))
}

#[derive(Debug, Serialize)]
pub struct ConfigValidationResponse {
    pub valid: bool,
    pub results: Vec<ConfigValidationResult>,
}

#[derive(Debug, Serialize)]
pub struct ConfigValidationResult {
    pub component: String,
    pub valid: bool,
    pub message: Option<String>,
}

/// Redact password from database URL
fn redact_database_url(url: &str) -> String {
    redact_url_password(url)
}

/// Redact password from Redis URL
fn redact_redis_url(url: &str) -> String {
    redact_url_password(url)
}

fn redact_url_password(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return url.to_string(),
    };

    if parsed.password().is_none() {
        return url.to_string();
    }

    let host = match parsed.host_str() {
        Some(host) => host,
        None => return url.to_string(),
    };

    let mut out = String::new();
    out.push_str(parsed.scheme());
    out.push_str("://");

    // URL userinfo can be `user:pass@` or `:pass@`. We always keep the username and redact the password.
    out.push_str(parsed.username());
    out.push(':');
    out.push_str("[REDACTED]");
    out.push('@');

    out.push_str(host);
    if let Some(port) = parsed.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }

    out.push_str(parsed.path());
    if let Some(query) = parsed.query() {
        out.push('?');
        out.push_str(query);
    }
    if let Some(fragment) = parsed.fragment() {
        out.push('#');
        out.push_str(fragment);
    }

    out
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_database_url_redaction() {
        let url = "postgresql://user:password@localhost/db";
        let redacted = redact_database_url(url);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("password"));
    }
    
    #[test]
    fn test_redis_url_redaction() {
        let url = "redis://:password@localhost:6379";
        let redacted = redact_redis_url(url);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("password"));
    }
}
