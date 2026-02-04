//! Polymarket API Router
//! Registers all Polymarket endpoints

use axum::{
    routing::{get, post, delete},
    Router,
};
use std::sync::Arc;

use crate::AppState;
use super::polymarket_api::*;

/// Create Polymarket API router
pub fn create_polymarket_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Order Management
        .route("/orders", post(create_order).get(get_user_orders))
        .route("/orders/submit", post(submit_order))
        .route("/orders/:order_id", get(get_order).delete(cancel_order))
        
        // Market Data
        .route("/markets/:condition_id", get(get_market_data))
        .route("/markets/:condition_id/sync", post(sync_market))
        .route("/orderbook/:token_id", get(get_order_book))
        .route("/markets/:condition_id/history", get(get_price_history))
        
        // User Positions & Balances
        .route("/positions", get(get_positions))
        .route("/balances", get(get_balances))
        .route("/stats", get(get_user_stats))
        
        // CTF Operations
        .route("/ctf/split", post(split_position))
        .route("/ctf/merge", post(merge_positions))
        .route("/ctf/redeem", post(redeem_positions))
        
        // Health & Admin
        .route("/health", get(health_check))
        
        // Attach state
        .with_state(state)
}

/// Get user's orders
async fn get_user_orders(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<GetOrdersQuery>,
) -> Result<Json<ApiResponse<Vec<OrderResponse>>>, StatusCode> {
    info!("Getting orders for user: {}", claims.wallet_address);
    
    let repository = state.polymarket_repository
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let orders = repository.get_user_open_orders(&claims.wallet_address)
        .await
        .map_err(|e| {
            error!("Failed to get orders: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let response: Vec<OrderResponse> = orders.into_iter()
        .map(|order| OrderResponse {
            order_id: order.order_id,
            status: format!("{:?}", order.status),
            created_at: order.created_at,
            size: order.size.to_string(),
            price: order.price.to_string(),
            filled_amount: order.filled_amount.to_string(),
            remaining_amount: order.remaining_amount
                .unwrap_or_else(|| order.size - order.filled_amount)
                .to_string(),
            average_fill_price: order.average_fill_price.map(|p| p.to_string()),
            estimated_fees: "0".to_string(),
        })
        .collect();
    
    Ok(Json(responses::success(response)))
}

/// Redeem positions request
#[derive(Debug, Deserialize)]
pub struct RedeemPositionsRequest {
    pub condition_id: String,
    pub index_sets: Vec<String>,
}

/// Redeem positions response
#[derive(Debug, Serialize)]
pub struct RedeemPositionsResponse {
    pub tx_hash: String,
    pub payout: String,
    pub gas_used: u64,
}

/// Redeem winning positions
async fn redeem_positions(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<RedeemPositionsRequest>,
) -> Result<Json<ApiResponse<RedeemPositionsResponse>>, StatusCode> {
    info!("Redeeming positions for user: {}", claims.wallet_address);
    
    let ctf_client = state.polymarket_ctf_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let index_sets: Vec<ethereum_types::U256> = request.index_sets
        .iter()
        .map(|s| ethereum_types::U256::from_dec_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let result = ctf_client.redeem_positions(&request.condition_id, index_sets)
        .await
        .map_err(|e| {
            error!("Failed to redeem positions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(RedeemPositionsResponse {
        tx_hash: result.tx_hash,
        payout: result.payout.to_string(),
        gas_used: result.gas_used,
    })))
}

/// Orders query parameters
#[derive(Debug, Deserialize)]
pub struct GetOrdersQuery {
    pub status: Option<String>,
    pub market_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// Import required types
use axum::{
    extract::{Query, State, Path},
    response::Json,
    http::StatusCode,
    Extension,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

use crate::{
    auth::Claims,
    response::{ApiResponse, responses},
};