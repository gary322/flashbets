//! Polymarket API Endpoints
//! Comprehensive REST API for all Polymarket operations

use axum::{
    extract::{Path, Query, State},
    response::Json,
    http::StatusCode,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    AppState,
    response::{ApiResponse, responses},
    auth::Claims,
    services::polymarket_order_service::{
        PolymarketOrderService, CreateOrderParams, OrderSide, OrderTracking
    },
    db::polymarket_repository::PolymarketRepository,
    integration::{
        polymarket_auth::PolymarketOrderData,
        polymarket_clob::PolymarketClobClient,
        polymarket_ctf::PolymarketCtfClient,
    },
};

// ==================== Order Management ====================

/// Create order request
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateOrderRequest {
    pub market_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: u8,
    pub side: String, // "buy" or "sell"
    pub size: String,
    pub price: String,
    pub order_type: Option<String>, // gtc, fok, ioc
    pub expiration: Option<u64>,
}

/// Order response
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub size: String,
    pub price: String,
    pub filled_amount: String,
    pub remaining_amount: String,
    pub average_fill_price: Option<String>,
    pub estimated_fees: String,
}

/// Submit order request
#[derive(Debug, Deserialize)]
pub struct SubmitOrderRequest {
    pub order_data: PolymarketOrderData,
    pub signature: String,
}

/// Create a new order (returns unsigned order for client signing)
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<CreateOrderRequest>,
) -> Result<Json<ApiResponse<PolymarketOrderData>>, StatusCode> {
    info!("Creating order for user: {}", claims.wallet_address);
    
    let order_service = state.polymarket_order_service
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let side = match request.side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => return Ok(Json(responses::error("Invalid order side"))),
    };
    
    let size = Decimal::from_str_exact(&request.size)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let price = Decimal::from_str_exact(&request.price)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let params = CreateOrderParams {
        wallet_address: claims.wallet_address.clone(),
        condition_id: request.condition_id,
        token_id: request.token_id,
        outcome: request.outcome,
        side,
        size,
        price,
        order_type: request.order_type.unwrap_or_else(|| "gtc".to_string()),
        expiration: request.expiration,
        fee_rate_bps: 10,
    };
    
    let order_data = order_service.create_order(params)
        .await
        .map_err(|e| {
            error!("Failed to create order: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(order_data)))
}

/// Submit a signed order to Polymarket
pub async fn submit_order(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<SubmitOrderRequest>,
) -> Result<Json<ApiResponse<OrderResponse>>, StatusCode> {
    info!("Submitting order for user: {}", claims.wallet_address);
    
    let order_service = state.polymarket_order_service
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let submission = order_service
        .submit_order(request.order_data.clone(), request.signature)
        .await
        .map_err(|e| {
            error!("Failed to submit order: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(OrderResponse {
        order_id: submission.order_id,
        status: format!("{:?}", submission.status),
        created_at: submission.submitted_at,
        size: request.order_data.maker_amount.clone(),
        price: "0".to_string(), // Calculate from order_data
        filled_amount: "0".to_string(),
        remaining_amount: request.order_data.maker_amount,
        average_fill_price: None,
        estimated_fees: submission.estimated_fees.to_string(),
    })))
}

/// Cancel an order
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    info!("Cancelling order {} for user: {}", order_id, claims.wallet_address);
    
    let order_service = state.polymarket_order_service
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    order_service.cancel_order(&order_id)
        .await
        .map_err(|e| {
            error!("Failed to cancel order: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(())))
}

/// Get order status
pub async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<String>,
) -> Result<Json<ApiResponse<OrderTracking>>, StatusCode> {
    let order_service = state.polymarket_order_service
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let tracking = order_service.get_order_status(&order_id)
        .await
        .map_err(|e| {
            error!("Failed to get order: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(tracking)))
}

// ==================== Market Data ====================

/// Market data response
#[derive(Debug, Serialize)]
pub struct MarketDataResponse {
    pub condition_id: String,
    pub token_id: String,
    pub liquidity: String,
    pub volume_24h: String,
    pub last_price: Option<String>,
    pub bid: Option<String>,
    pub ask: Option<String>,
    pub spread: Option<String>,
    pub open_interest: String,
}

/// Order book response
#[derive(Debug, Serialize)]
pub struct OrderBookResponse {
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub spread: Option<String>,
    pub mid_price: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrderBookLevel {
    pub price: String,
    pub size: String,
    pub num_orders: u32,
}

/// Get market data
pub async fn get_market_data(
    State(state): State<Arc<AppState>>,
    Path(condition_id): Path<String>,
) -> Result<Json<ApiResponse<MarketDataResponse>>, StatusCode> {
    info!("Getting market data for: {}", condition_id);
    
    let repository = state.polymarket_repository
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let market = repository.get_market_by_condition(&condition_id)
        .await
        .map_err(|e| {
            error!("Failed to get market: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let spread = match (market.bid, market.ask) {
        (Some(bid), Some(ask)) => Some((ask - bid).to_string()),
        _ => None,
    };
    
    Ok(Json(responses::success(MarketDataResponse {
        condition_id: market.condition_id,
        token_id: market.token_id,
        liquidity: market.liquidity.to_string(),
        volume_24h: market.volume_24h.to_string(),
        last_price: market.last_price.map(|p| p.to_string()),
        bid: market.bid.map(|p| p.to_string()),
        ask: market.ask.map(|p| p.to_string()),
        spread,
        open_interest: "0".to_string(), // Would calculate from positions
    })))
}

/// Get order book
pub async fn get_order_book(
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<String>,
) -> Result<Json<ApiResponse<OrderBookResponse>>, StatusCode> {
    info!("Getting order book for token: {}", token_id);
    
    let clob_client = state.polymarket_clob_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let book = clob_client.get_order_book(&token_id)
        .await
        .map_err(|e| {
            error!("Failed to get order book: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let bids: Vec<OrderBookLevel> = book.bids.into_iter()
        .map(|entry| OrderBookLevel {
            price: entry.price.to_string(),
            size: entry.size.to_string(),
            num_orders: entry.num_orders,
        })
        .collect();
    
    let asks: Vec<OrderBookLevel> = book.asks.into_iter()
        .map(|entry| OrderBookLevel {
            price: entry.price.to_string(),
            size: entry.size.to_string(),
            num_orders: entry.num_orders,
        })
        .collect();
    
    let (spread, mid_price) = if !bids.is_empty() && !asks.is_empty() {
        let best_bid = Decimal::from_str_exact(&bids[0].price).ok();
        let best_ask = Decimal::from_str_exact(&asks[0].price).ok();
        
        match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => (
                Some((ask - bid).to_string()),
                Some(((bid + ask) / Decimal::from(2)).to_string()),
            ),
            _ => (None, None),
        }
    } else {
        (None, None)
    };
    
    Ok(Json(responses::success(OrderBookResponse {
        bids,
        asks,
        spread,
        mid_price,
    })))
}

// ==================== User Positions & Balances ====================

/// Position response
#[derive(Debug, Serialize)]
pub struct PositionResponse {
    pub condition_id: String,
    pub outcome_index: u8,
    pub balance: String,
    pub locked_balance: String,
    pub average_price: Option<String>,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    pub market_value: String,
}

/// Balance response
#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub usdc_balance: String,
    pub matic_balance: String,
    pub total_position_value: String,
    pub available_balance: String,
    pub locked_in_orders: String,
}

/// Get user's positions
pub async fn get_positions(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ApiResponse<Vec<PositionResponse>>>, StatusCode> {
    info!("Getting positions for user: {}", claims.wallet_address);
    
    let repository = state.polymarket_repository
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let positions = repository.get_user_ctf_positions(&claims.wallet_address)
        .await
        .map_err(|e| {
            error!("Failed to get positions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let response: Vec<PositionResponse> = positions.into_iter()
        .map(|pos| PositionResponse {
            condition_id: pos.condition_id,
            outcome_index: pos.outcome_index as u8,
            balance: pos.balance.to_string(),
            locked_balance: pos.locked_balance.to_string(),
            average_price: pos.average_price.map(|p| p.to_string()),
            realized_pnl: pos.realized_pnl.to_string(),
            unrealized_pnl: pos.unrealized_pnl.to_string(),
            market_value: "0".to_string(), // Would calculate based on current price
        })
        .collect();
    
    Ok(Json(responses::success(response)))
}

/// Get user's balances
pub async fn get_balances(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ApiResponse<BalanceResponse>>, StatusCode> {
    info!("Getting balances for user: {}", claims.wallet_address);
    
    let ctf_client = state.polymarket_ctf_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let usdc_balance = ctf_client.get_usdc_balance(&claims.wallet_address)
        .await
        .map_err(|e| {
            error!("Failed to get USDC balance: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let matic_balance = ctf_client.get_matic_balance(&claims.wallet_address)
        .await
        .map_err(|e| {
            error!("Failed to get MATIC balance: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(BalanceResponse {
        usdc_balance: usdc_balance.to_string(),
        matic_balance: matic_balance.to_string(),
        total_position_value: "0".to_string(),
        available_balance: usdc_balance.to_string(),
        locked_in_orders: "0".to_string(),
    })))
}

// ==================== CTF Operations ====================

#[derive(Debug, Deserialize)]
pub struct SplitPositionRequest {
    pub condition_id: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct SplitPositionResponse {
    pub tx_hash: String,
    pub yes_tokens: String,
    pub no_tokens: String,
    pub gas_used: u64,
}

/// Split position (mint outcome tokens)
pub async fn split_position(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<SplitPositionRequest>,
) -> Result<Json<ApiResponse<SplitPositionResponse>>, StatusCode> {
    info!("Splitting position for user: {}", claims.wallet_address);
    
    let ctf_client = state.polymarket_ctf_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let amount = ethereum_types::U256::from_dec_str(&request.amount)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let result = ctf_client.split_position(&request.condition_id, amount)
        .await
        .map_err(|e| {
            error!("Failed to split position: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(SplitPositionResponse {
        tx_hash: result.tx_hash,
        yes_tokens: result.yes_tokens.to_string(),
        no_tokens: result.no_tokens.to_string(),
        gas_used: result.gas_used,
    })))
}

#[derive(Debug, Deserialize)]
pub struct MergePositionsRequest {
    pub condition_id: String,
    pub amount: String,
}

#[derive(Debug, Serialize)]
pub struct MergePositionsResponse {
    pub tx_hash: String,
    pub collateral_returned: String,
    pub gas_used: u64,
}

/// Merge positions (burn outcome tokens)
pub async fn merge_positions(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<MergePositionsRequest>,
) -> Result<Json<ApiResponse<MergePositionsResponse>>, StatusCode> {
    info!("Merging positions for user: {}", claims.wallet_address);
    
    let ctf_client = state.polymarket_ctf_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let amount = ethereum_types::U256::from_dec_str(&request.amount)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let result = ctf_client.merge_positions(&request.condition_id, amount)
        .await
        .map_err(|e| {
            error!("Failed to merge positions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(MergePositionsResponse {
        tx_hash: result.tx_hash,
        collateral_returned: result.collateral_returned.to_string(),
        gas_used: result.gas_used,
    })))
}

// ==================== Analytics & History ====================

/// Price history query
#[derive(Debug, Deserialize)]
pub struct PriceHistoryQuery {
    pub hours: Option<i32>,
    pub resolution: Option<String>, // "1m", "5m", "1h", "1d"
}

/// Price point
#[derive(Debug, Serialize)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub price: String,
    pub volume: Option<String>,
}

/// Get price history
pub async fn get_price_history(
    State(state): State<Arc<AppState>>,
    Path(condition_id): Path<String>,
    Query(query): Query<PriceHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<PricePoint>>>, StatusCode> {
    info!("Getting price history for: {}", condition_id);
    
    let repository = state.polymarket_repository
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let hours = query.hours.unwrap_or(24);
    
    let history = repository.get_price_history(&condition_id, hours)
        .await
        .map_err(|e| {
            error!("Failed to get price history: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let points: Vec<PricePoint> = history.into_iter()
        .map(|h| PricePoint {
            timestamp: h["timestamp"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            price: h["price"].as_str().unwrap_or("0").to_string(),
            volume: h["volume"].as_str().map(|s| s.to_string()),
        })
        .collect();
    
    Ok(Json(responses::success(points)))
}

/// User statistics
#[derive(Debug, Serialize)]
pub struct UserStats {
    pub total_volume_traded: String,
    pub total_markets_traded: u32,
    pub win_rate: f64,
    pub total_pnl: String,
    pub best_trade: Option<String>,
    pub worst_trade: Option<String>,
    pub active_positions: u32,
    pub pending_orders: u32,
}

/// Get user statistics
pub async fn get_user_stats(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ApiResponse<UserStats>>, StatusCode> {
    info!("Getting stats for user: {}", claims.wallet_address);
    
    let repository = state.polymarket_repository
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let stats = repository.get_user_stats(&claims.wallet_address)
        .await
        .map_err(|e| {
            error!("Failed to get user stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(responses::success(UserStats {
        total_volume_traded: stats["total_volume"].as_str().unwrap_or("0").to_string(),
        total_markets_traded: stats["markets_traded"].as_u64().unwrap_or(0) as u32,
        win_rate: 0.0, // Would calculate from trade history
        total_pnl: "0".to_string(),
        best_trade: None,
        worst_trade: None,
        active_positions: stats["active_positions"].as_u64().unwrap_or(0) as u32,
        pending_orders: stats["pending_orders"].as_u64().unwrap_or(0) as u32,
    })))
}

// ==================== Admin Operations ====================

/// Sync market data from Polymarket
pub async fn sync_market(
    State(state): State<Arc<AppState>>,
    Path(condition_id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    info!("Syncing market: {}", condition_id);
    
    let clob_client = state.polymarket_clob_client
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let repository = state.polymarket_repository
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Get market data from Polymarket
    let market = clob_client.get_market(&condition_id)
        .await
        .map_err(|e| {
            error!("Failed to get market from Polymarket: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Update database
    repository.update_market_data(
        &condition_id,
        Decimal::from_f64_retain(market.liquidity).unwrap_or_default(),
        Decimal::from_f64_retain(market.volume).unwrap_or_default(),
        None,
        None,
        None,
    ).await
    .map_err(|e| {
        error!("Failed to update market data: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    Ok(Json(responses::success(())))
}

/// Health check for Polymarket integration
#[derive(Debug, Serialize)]
pub struct PolymarketHealth {
    pub clob_connected: bool,
    pub websocket_connected: bool,
    pub database_connected: bool,
    pub last_sync: Option<DateTime<Utc>>,
    pub pending_orders: u32,
    pub active_positions: u32,
}

/// Get Polymarket integration health
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<PolymarketHealth>>, StatusCode> {
    let clob_connected = state.polymarket_clob_client.is_some();
    let websocket_connected = state.polymarket_ws_client.is_some();
    let database_connected = state.polymarket_repository.is_some();
    
    Ok(Json(responses::success(PolymarketHealth {
        clob_connected,
        websocket_connected,
        database_connected,
        last_sync: Some(Utc::now()),
        pending_orders: 0,
        active_positions: 0,
    })))
}