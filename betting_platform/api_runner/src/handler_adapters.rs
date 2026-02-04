//! Handler adapters to fix extractor ordering issues

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
    http::StatusCode,
};
use crate::{
    AppState,
    jwt_validation::AuthenticatedUser,
    rbac_authorization::{RequireRole, CanCreateMarkets, CanViewAllPositions, CanUpdateSystemConfig},
};

// Auth endpoint adapters
pub async fn logout_adapter(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    crate::auth_endpoints::logout(user, State(state)).await
}

pub async fn get_user_info_adapter(
    State(_state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    crate::auth_endpoints::get_user_info(user).await
}

// RBAC endpoint adapters
pub async fn get_user_permissions_adapter(
    State(_state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    crate::rbac_endpoints::get_user_permissions(user).await
}

pub async fn grant_permission_adapter(
    State(state): State<AppState>,
    role: RequireRole,
    Json(payload): Json<crate::rbac_endpoints::GrantPermissionRequest>,
) -> impl IntoResponse {
    match crate::rbac_endpoints::grant_permission(State(state), role, Json(payload)).await {
        Ok(response) => response.into_response(),
        Err(e) => (e, "Grant permission failed").into_response(),
    }
}

pub async fn update_user_role_adapter(
    State(state): State<AppState>,
    role: RequireRole,
    Json(payload): Json<crate::rbac_endpoints::UpdateUserRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::rbac_endpoints::update_user_role(State(state), role, Json(payload)).await
}

pub async fn view_all_positions_adapter(
    State(state): State<AppState>,
    perm: CanViewAllPositions,
) -> impl IntoResponse {
    crate::rbac_endpoints::view_all_positions(State(state), perm).await
}

pub async fn update_system_config_adapter(
    State(state): State<AppState>,
    perm: CanUpdateSystemConfig,
    Json(payload): Json<crate::rbac_endpoints::SystemConfigUpdate>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::rbac_endpoints::update_system_config(State(state), perm, Json(payload)).await
}

pub async fn create_market_authorized_adapter(
    State(state): State<AppState>,
    perm: CanCreateMarkets,
    Json(payload): Json<crate::types::CreateMarketRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::rbac_endpoints::create_market_authorized(State(state), perm, Json(payload)).await
}

// Trading API adapters
pub async fn place_order_adapter(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<crate::trading_engine::PlaceOrderRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::trading_api::place_order(State(state), user, Json(payload)).await
}

pub async fn get_user_orders_adapter(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    query: axum::extract::Query<crate::trading_api::OrdersQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::trading_api::get_user_orders(State(state), user, query).await
}

pub async fn cancel_order_adapter(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::trading_api::cancel_order(State(state), user, path).await
}

// Security endpoint adapters
pub async fn get_security_events_adapter(
    State(state): State<AppState>,
    role: RequireRole,
    query: axum::extract::Query<crate::security_endpoints::SecurityEventQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::get_security_events(State(state), role, query).await
}

pub async fn get_security_stats_adapter(
    State(state): State<AppState>,
    role: RequireRole,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::get_security_stats(State(state), role).await
}

pub async fn update_alert_config_adapter(
    State(state): State<AppState>,
    perm: CanUpdateSystemConfig,
    Json(payload): Json<crate::security_endpoints::AlertConfig>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::update_alert_config(State(state), perm, Json(payload)).await
}

pub async fn manage_ip_block_adapter(
    State(state): State<AppState>,
    role: RequireRole,
    path: axum::extract::Path<String>,
    Json(payload): Json<crate::security_endpoints::IpBlockRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::manage_ip_block(State(state), role, path, Json(payload)).await
}

pub async fn search_security_logs_adapter(
    State(state): State<AppState>,
    role: RequireRole,
    Json(payload): Json<crate::security_endpoints::SecuritySearchRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::search_security_logs(State(state), role, Json(payload)).await
}

pub async fn export_security_logs_adapter(
    State(state): State<AppState>,
    role: RequireRole,
    Json(payload): Json<crate::security_endpoints::ExportRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::export_security_logs(State(state), role, Json(payload)).await
}

pub async fn get_security_dashboard_adapter(
    State(state): State<AppState>,
    role: RequireRole,
) -> Result<impl IntoResponse, StatusCode> {
    crate::security_endpoints::get_security_dashboard(State(state), role).await
}

// Deployment endpoint adapters
pub async fn register_program_adapter(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::deployment_endpoints::RegisterProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::deployment_endpoints::register_program(State(state), auth, Json(payload)).await
}

pub async fn deploy_program_adapter(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::deployment_endpoints::DeployProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::deployment_endpoints::deploy_program(State(state), auth, Json(payload)).await
}

pub async fn upgrade_program_adapter(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::deployment_endpoints::UpgradeProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::deployment_endpoints::upgrade_program(State(state), auth, Json(payload)).await
}

pub async fn initialize_program_adapter(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::deployment_endpoints::InitializeProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::deployment_endpoints::initialize_program(State(state), auth, Json(payload)).await
}

// External API endpoint adapters
pub async fn fetch_external_markets_adapter(
    State(state): State<AppState>,
    query: axum::extract::Query<crate::external_api_endpoints::MarketQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::external_api_endpoints::fetch_external_markets(State(state), query).await
}

pub async fn get_external_prices_adapter(
    State(state): State<AppState>,
    path: axum::extract::Path<String>,
    Json(payload): Json<Vec<String>>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::external_api_endpoints::get_external_prices(State(state), path, Json(payload)).await
}

pub async fn test_external_api_adapter(
    State(state): State<AppState>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::external_api_endpoints::test_external_api(State(state), path).await
}

// Circuit breaker endpoint adapters
pub async fn reset_circuit_breakers_adapter(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    match crate::circuit_breaker_middleware::reset_circuit_breakers(State(state), auth).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// Market creation endpoint adapters
pub async fn create_market_adapter(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::market_creation_service::CreateMarketRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    match crate::market_creation_endpoints::create_market(State(state), user, Json(payload)).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn update_market_adapter(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    path: axum::extract::Path<u128>,
    Json(payload): Json<crate::market_creation_service::UpdateMarketRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    match crate::market_creation_endpoints::update_market(State(state), user, path, Json(payload)).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn list_markets_adapter(
    query: axum::extract::Query<crate::market_creation_endpoints::ListMarketsQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    match crate::market_creation_endpoints::list_markets(State(state), query).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// Trade execution endpoint adapters
pub async fn execute_trade_adapter(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::trade_execution_service::TradeExecutionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    match crate::trade_execution_endpoints::execute_trade(State(state), user, Json(payload)).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_trade_history_adapter(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    query: axum::extract::Query<crate::trade_execution_endpoints::TradeHistoryQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    match crate::trade_execution_endpoints::get_trade_history(State(state), user, query).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// Solana transaction endpoint adapters
pub async fn simulate_transaction_adapter(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<crate::solana_endpoints::SimulateTransactionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    crate::solana_endpoints::simulate_transaction(State(state), user, Json(payload)).await
}