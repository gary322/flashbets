//! Test data management endpoints
//! Provides REST API for test data creation and management

use anyhow::Context as AnyhowContext;
use axum::{
    extract::{Path, Query, State},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::{
    jwt_validation::AuthenticatedUser,
    response::{ApiResponse, responses},
    test_data_manager::{
        TestDataCategory, TestDataConfig, TestDataManager, TestDataBuilder,
        TestUser, TestMarket, TestPosition,
    },
    tracing_logger::CorrelationId,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    AppState,
};

/// Test data creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTestDataRequest {
    pub users: Option<usize>,
    pub markets: Option<usize>,
    pub positions_per_user: Option<usize>,
    pub settled_markets: Option<usize>,
    pub scenario_name: Option<String>,
}

/// Test data query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDataQuery {
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub lifecycle: Option<String>,
    pub limit: Option<usize>,
}

/// Test data cleanup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupRequest {
    pub force: Option<bool>,
    pub categories: Option<Vec<String>>,
}

/// Test data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDataResponse {
    pub id: String,
    pub category: String,
    pub data_type: String,
    pub lifecycle: String,
    pub created_at: String,
    pub tags: Vec<String>,
}

/// Create test data
pub async fn create_test_data(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
    Json(request): Json<CreateTestDataRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "create_test_data");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can create test data",
            context,
        ));
    }

    info!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        "Creating test data"
    );

    // Get test data manager
    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context.clone(),
        ))?;

    // Create test data based on request
    let mut result = serde_json::json!({});

    if let Some(scenario_name) = request.scenario_name {
        // Create complete scenario
        let scenario = manager.create_test_scenario(&scenario_name).await
            .map_err(|e| AppError::new(
                ErrorKind::ExternalServiceError,
                format!("Failed to create scenario: {}", e),
                context,
            ))?;
        
        result = serde_json::json!({
            "scenario": scenario_name,
            "data": scenario,
        });
    } else {
        // Create individual components
        let builder = TestDataBuilder::new(manager.clone());
        
        let mut builder = builder;
        
        if let Some(user_count) = request.users {
            builder = builder.with_users(user_count).await
                .map_err(|e| AppError::new(
                    ErrorKind::ExternalServiceError,
                    format!("Failed to create users: {}", e),
                    context.clone(),
                ))?;
        }
        
        if let Some(market_count) = request.markets {
            builder = builder.with_markets(market_count).await
                .map_err(|e| AppError::new(
                    ErrorKind::ExternalServiceError,
                    format!("Failed to create markets: {}", e),
                    context.clone(),
                ))?;
        }
        
        if let Some(positions_per_user) = request.positions_per_user {
            builder = builder.with_positions(positions_per_user).await
                .map_err(|e| AppError::new(
                    ErrorKind::ExternalServiceError,
                    format!("Failed to create positions: {}", e),
                    context.clone(),
                ))?;
        }
        
        if let Some(settled_count) = request.settled_markets {
            builder = builder.with_settled_markets(settled_count).await
                .map_err(|e| AppError::new(
                    ErrorKind::ExternalServiceError,
                    format!("Failed to settle markets: {}", e),
                    context.clone(),
                ))?;
        }
        
        let dataset = builder.build();
        
        result = serde_json::json!({
            "users": dataset.users.len(),
            "markets": dataset.markets.len(),
            "positions": dataset.positions.len(),
            "data": {
                "users": dataset.users,
                "markets": dataset.markets,
                "positions": dataset.positions,
            }
        });
    }

    Ok(Json(responses::success_with_data(
        "Test data created successfully",
        result,
    )))
}

/// List test data
pub async fn list_test_data(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
    Query(query): Query<TestDataQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "list_test_data");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view test data",
            context,
        ));
    }

    info!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        "Listing test data"
    );

    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context,
        ))?;

    // Get test data based on query
    let records = if let Some(category_str) = query.category {
        match category_str.as_str() {
            "users" => manager.get_by_category(TestDataCategory::Users).await,
            "markets" => manager.get_by_category(TestDataCategory::Markets).await,
            "positions" => manager.get_by_category(TestDataCategory::Positions).await,
            _ => Vec::new(),
        }
    } else if let Some(tags) = query.tags {
        manager.search_by_tags(&tags).await
    } else {
        // Return sample data
        Vec::new()
    };

    let limit = query.limit.unwrap_or(100);
    let response: Vec<TestDataResponse> = records
        .into_iter()
        .take(limit)
        .map(|record| TestDataResponse {
            id: record.id,
            category: format!("{:?}", record.category),
            data_type: record.data_type,
            lifecycle: format!("{:?}", record.lifecycle),
            created_at: record.created_at.to_rfc3339(),
            tags: record.tags,
        })
        .collect();

    Ok(Json(responses::success_with_data(
        "Test data retrieved successfully",
        response,
    )))
}

/// Get test data by ID
pub async fn get_test_data(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "get_test_data");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view test data",
            context,
        ));
    }

    info!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        id = %id,
        "Getting test data"
    );

    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context.clone(),
        ))?;

    let record = manager.get_by_id(&id).await
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            format!("Test data not found: {}", id),
            context,
        ))?;

    Ok(Json(responses::success_with_data(
        "Test data retrieved successfully",
        serde_json::json!({
            "id": record.id,
            "category": format!("{:?}", record.category),
            "data_type": record.data_type,
            "data": record.data,
            "lifecycle": format!("{:?}", record.lifecycle),
            "created_at": record.created_at,
            "updated_at": record.updated_at,
            "expires_at": record.expires_at,
            "tags": record.tags,
            "references": record.references,
        }),
    )))
}

/// Clean up test data
pub async fn cleanup_test_data(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
    Json(request): Json<CleanupRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "cleanup_test_data");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can cleanup test data",
            context,
        ));
    }

    info!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        force = ?request.force,
        "Cleaning up test data"
    );

    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context.clone(),
        ))?;

    let force = request.force.unwrap_or(false);
    let count = manager.cleanup(force).await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to cleanup test data: {}", e),
            context,
        ))?;

    Ok(Json(responses::success_with_data(
        "Test data cleaned up successfully",
        serde_json::json!({
            "cleaned_records": count,
            "force": force,
        }),
    )))
}

/// Get test data report
pub async fn get_test_data_report(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "get_test_data_report");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can view test data reports",
            context,
        ));
    }

    info!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        "Generating test data report"
    );

    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context,
        ))?;

    let report = manager.generate_report().await;

    Ok(Json(responses::success_with_data(
        "Test data report generated successfully",
        serde_json::to_value(report).unwrap(),
    )))
}

/// Create test JWT tokens
pub async fn create_test_tokens(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "create_test_tokens");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can create test tokens",
            context,
        ));
    }

    info!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        "Creating test tokens"
    );

    let count = request.get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize;

    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context.clone(),
        ))?;

    // Create test users and return their tokens
    let users = manager.create_test_users(count).await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to create test users: {}", e),
            context,
        ))?;

    let tokens: Vec<serde_json::Value> = users
        .into_iter()
        .map(|user| serde_json::json!({
            "user_id": user.id,
            "email": user.email,
            "wallet": user.wallet,
            "role": user.role,
            "token": user.jwt_token,
            "balance": user.balance,
        }))
        .collect();

    Ok(Json(responses::success_with_data(
        "Test tokens created successfully",
        tokens,
    )))
}

/// Reset test database
pub async fn reset_test_database(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthenticatedUser>,
    Extension(correlation_id): Extension<CorrelationId>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let context = ErrorContext::new("test_data_endpoints", "reset_test_database");
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Only admins can reset test database",
            context,
        ));
    }

    warn!(
        correlation_id = %correlation_id,
        user = %user.claims.wallet,
        "Resetting test database"
    );

    let manager = state.test_data_manager.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::NotFound,
            "Test data manager not configured",
            context.clone(),
        ))?;

    // Clean up all test data
    let cleanup_count = manager.cleanup(true).await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to cleanup test data: {}", e),
            context.clone(),
        ))?;

    // Create fresh test scenario
    let scenario = manager.create_test_scenario("default").await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to create default scenario: {}", e),
            context,
        ))?;

    Ok(Json(responses::success_with_data(
        "Test database reset successfully",
        serde_json::json!({
            "cleaned_records": cleanup_count,
            "new_scenario": "default",
            "users_created": scenario.get("users").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            "markets_created": scenario.get("markets").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            "positions_created": scenario.get("positions").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
        }),
    )))
}