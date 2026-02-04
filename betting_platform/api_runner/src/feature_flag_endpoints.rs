//! REST API endpoints for feature flag management

use axum::{
    extract::{State, Path, Query, Extension},
    response::IntoResponse,
    Json,
};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::{
    AppState,
    feature_flags::{FeatureFlag, FlagStatus, TargetRule, EvaluationContext},
    jwt_validation::AuthenticatedUser,
    rbac_authorization::{Permission, RequireRole},
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Feature flag query parameters
#[derive(Debug, Deserialize)]
pub struct FlagQuery {
    pub active_only: Option<bool>,
    pub search: Option<String>,
    pub status: Option<String>,
}

/// Feature flag evaluation request
#[derive(Debug, Deserialize)]
pub struct EvaluationRequest {
    pub flags: Vec<String>,
    pub context: Option<EvaluationContextRequest>,
}

/// Evaluation context in request
#[derive(Debug, Deserialize)]
pub struct EvaluationContextRequest {
    pub user_id: Option<String>,
    pub group_ids: Option<Vec<String>>,
    pub ip_address: Option<String>,
    pub market_id: Option<u128>,
    pub custom_attributes: Option<std::collections::HashMap<String, String>>,
}

/// Feature flag evaluation response
#[derive(Debug, Serialize)]
pub struct EvaluationResponse {
    pub flags: std::collections::HashMap<String, bool>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Feature flag update request
#[derive(Debug, Deserialize)]
pub struct FlagUpdateRequest {
    pub description: Option<String>,
    pub status: Option<FlagStatus>,
    pub target_rules: Option<Vec<TargetRule>>,
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Create new feature flag (admin only)
pub async fn create_flag(
    State(state): State<Arc<AppState>>,
    _role: RequireRole,
    Json(flag): Json<FeatureFlag>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "create");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    // Check if flag already exists
    if service.get_all_flags().await?
        .iter()
        .any(|f| f.name == flag.name) {
        return Err(AppError::new(
            ErrorKind::AlreadyExists,
            format!("Feature flag '{}' already exists", flag.name),
            context,
        ));
    }
    
    service.update_flag(&flag).await?;
    
    info!("Created feature flag: {}", flag.name);
    
    Ok(Json(serde_json::json!({
        "message": "Feature flag created successfully",
        "flag": flag,
    })))
}

/// Get all feature flags
pub async fn get_flags(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FlagQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "list");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    let mut flags = service.get_all_flags().await?;
    
    // Apply filters
    if let Some(true) = query.active_only {
        flags.retain(|f| matches!(f.status, FlagStatus::Enabled | FlagStatus::Percentage(_)));
    }
    
    if let Some(search) = &query.search {
        let search_lower = search.to_lowercase();
        flags.retain(|f| 
            f.name.to_lowercase().contains(&search_lower) ||
            f.description.to_lowercase().contains(&search_lower)
        );
    }
    
    if let Some(status) = &query.status {
        flags.retain(|f| {
            match (status.as_str(), &f.status) {
                ("enabled", FlagStatus::Enabled) => true,
                ("disabled", FlagStatus::Disabled) => true,
                ("percentage", FlagStatus::Percentage(_)) => true,
                _ => false,
            }
        });
    }
    
    Ok(Json(serde_json::json!({
        "flags": flags,
        "count": flags.len(),
    })))
}

/// Get specific feature flag
pub async fn get_flag(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "get");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    let flags = service.get_all_flags().await?;
    let flag = flags.into_iter()
        .find(|f| f.name == name)
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            format!("Feature flag '{}' not found", name),
            context,
        ))?;
    
    Ok(Json(flag))
}

/// Update feature flag (admin only)
pub async fn update_flag(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    _role: RequireRole,
    Json(update): Json<FlagUpdateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "update");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    // Get existing flag
    let flags = service.get_all_flags().await?;
    let mut flag = flags.into_iter()
        .find(|f| f.name == name)
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            format!("Feature flag '{}' not found", name),
            context.clone(),
        ))?;
    
    // Apply updates
    if let Some(description) = update.description {
        flag.description = description;
    }
    if let Some(status) = update.status {
        flag.status = status;
    }
    if let Some(target_rules) = update.target_rules {
        flag.target_rules = target_rules;
    }
    if let Some(metadata) = update.metadata {
        flag.metadata = metadata;
    }
    if let Some(expires_at) = update.expires_at {
        flag.expires_at = Some(expires_at);
    }
    
    flag.updated_at = chrono::Utc::now();
    
    service.update_flag(&flag).await?;
    
    info!("Updated feature flag: {}", flag.name);
    
    Ok(Json(serde_json::json!({
        "message": "Feature flag updated successfully",
        "flag": flag,
    })))
}

/// Delete feature flag (admin only)
pub async fn delete_flag(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    _role: RequireRole,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "delete");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    service.delete_flag(&name).await?;
    
    info!("Deleted feature flag: {}", name);
    
    Ok(Json(serde_json::json!({
        "message": "Feature flag deleted successfully",
    })))
}

/// Evaluate feature flags for current user
pub async fn evaluate_flags(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
    Json(request): Json<EvaluationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "evaluate");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context.clone(),
        ))?;
    
    // Build evaluation context
    let mut eval_context = EvaluationContext {
        user_id: Some(auth.claims.wallet.clone()),
        ..Default::default()
    };
    
    if let Some(ctx) = request.context {
        if let Some(group_ids) = ctx.group_ids {
            eval_context.group_ids = group_ids;
        }
        if let Some(ip) = ctx.ip_address {
            eval_context.ip_address = Some(ip);
        }
        if let Some(market_id) = ctx.market_id {
            eval_context.market_id = Some(market_id);
        }
        if let Some(attrs) = ctx.custom_attributes {
            eval_context.custom_attributes = attrs;
        }
    }
    
    // Evaluate requested flags
    let mut results = std::collections::HashMap::new();
    for flag_name in request.flags {
        let enabled = service.is_enabled(&flag_name, &eval_context).await?;
        results.insert(flag_name, enabled);
    }
    
    Ok(Json(EvaluationResponse {
        flags: results,
        timestamp: chrono::Utc::now(),
    }))
}

/// Clear feature flag cache (admin only)
pub async fn clear_cache(
    State(state): State<Arc<AppState>>,
    _role: RequireRole,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "clear_cache");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context,
        ))?;
    
    service.clear_cache().await;
    
    info!("Cleared feature flag cache");
    
    Ok(Json(serde_json::json!({
        "message": "Feature flag cache cleared successfully",
    })))
}

/// Get feature flag statistics (admin only)
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    _role: RequireRole,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("feature_flags", "stats");
    
    let service = state.feature_flags.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Feature flag service not available",
            context,
        ))?;
    
    let flags = service.get_all_flags().await?;
    
    let enabled_count = flags.iter()
        .filter(|f| matches!(f.status, FlagStatus::Enabled))
        .count();
    
    let disabled_count = flags.iter()
        .filter(|f| matches!(f.status, FlagStatus::Disabled))
        .count();
    
    let percentage_count = flags.iter()
        .filter(|f| matches!(f.status, FlagStatus::Percentage(_)))
        .count();
    
    let with_targets = flags.iter()
        .filter(|f| !f.target_rules.is_empty())
        .count();
    
    let expiring_soon = flags.iter()
        .filter(|f| {
            if let Some(expires) = f.expires_at {
                expires - chrono::Utc::now() < chrono::Duration::days(7)
            } else {
                false
            }
        })
        .count();
    
    Ok(Json(serde_json::json!({
        "total_flags": flags.len(),
        "enabled": enabled_count,
        "disabled": disabled_count,
        "percentage_rollout": percentage_count,
        "with_targeting": with_targets,
        "expiring_soon": expiring_soon,
        "timestamp": chrono::Utc::now(),
    })))
}