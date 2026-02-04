//! Trading API endpoints for the trading engine

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error};

use crate::{
    AppState,
    jwt_validation::AuthenticatedUser,
    trading_engine::{
        TradingEngine, PlaceOrderRequest, CancelOrderRequest,
        OrderBookSnapshot, Order, Trade,
    },
    throughput_optimization::FastJson,
};

/// Initialize trading engine and add to app state
pub fn init_trading_engine(
    ws_manager: Option<Arc<crate::websocket::enhanced::EnhancedWebSocketManager>>,
) -> Arc<TradingEngine> {
    let config = crate::trading_engine::TradingEngineConfig::default();
    Arc::new(TradingEngine::new(config, ws_manager))
}

/// Place a new order
pub async fn place_order(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get trading engine from state
    let engine = &state.trading_engine;
    
    // Convert request to order
    let order = request.to_order(user.claims.sub.clone(), user.claims.wallet.clone())
        .map_err(|e| {
            warn!("Invalid order request: {}", e);
            StatusCode::BAD_REQUEST
        })?;
    
    // Place order
    match engine.place_order(order).await {
        Ok(placed_order) => {
            info!(
                "Order placed: {} for user {} on market {} outcome {}",
                placed_order.id,
                user.claims.sub,
                placed_order.market_id,
                placed_order.outcome
            );
            
            Ok(Json(OrderResponse::from(placed_order)).into_response())
        }
        Err(e) => {
            error!("Failed to place order: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Cancel an order
pub async fn cancel_order(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(order_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let engine = &state.trading_engine;
    
    match engine.cancel_order(&order_id, &user.claims.sub).await {
        Ok(cancelled_order) => {
            info!("Order {} cancelled by user {}", order_id, user.claims.sub);
            Ok(Json(OrderResponse::from(cancelled_order)).into_response())
        }
        Err(e) => {
            warn!("Failed to cancel order {}: {}", order_id, e);
            match e.as_str() {
                "Order not found" => Err(StatusCode::NOT_FOUND),
                "Unauthorized" => Err(StatusCode::FORBIDDEN),
                _ => Err(StatusCode::BAD_REQUEST),
            }
        }
    }
}

/// Get user's orders
pub async fn get_user_orders(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<OrdersQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let engine = &state.trading_engine;
    
    let mut orders = engine.get_user_orders(&user.claims.sub).await;
    
    // Filter by status if specified
    if let Some(status) = params.status {
        orders.retain(|o| match status.as_str() {
            "active" => o.is_active(),
            "filled" => o.is_filled(),
            "cancelled" => matches!(o.status, crate::trading_engine::OrderStatus::Cancelled),
            _ => true,
        });
    }
    
    // Filter by market if specified
    if let Some(market_id) = params.market_id {
        orders.retain(|o| o.market_id == market_id);
    }
    
    // Sort by creation time (newest first)
    orders.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    
    // Apply pagination
    let total = orders.len();
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);
    
    let paginated: Vec<_> = orders
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(OrderResponse::from)
        .collect();
    
    Ok(Json(OrdersResponse {
        orders: paginated,
        total,
        limit,
        offset,
    }).into_response())
}

/// Get order book for a market
pub async fn get_order_book(
    State(state): State<AppState>,
    Path((market_id, outcome)): Path<(u128, u8)>,
    Query(params): Query<OrderBookQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let engine = &state.trading_engine;
    
    let depth = params.depth.unwrap_or(20).min(50);
    let snapshot = engine.get_order_book(market_id, outcome, depth).await;
    
    Ok(FastJson(OrderBookResponse::from(snapshot)).into_response())
}

/// Get recent trades
pub async fn get_recent_trades(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
    Query(params): Query<TradesQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let engine = &state.trading_engine;
    
    let limit = params.limit.unwrap_or(50).min(100);
    let trades = engine.get_recent_trades(market_id, limit).await;
    
    let trade_responses: Vec<_> = trades
        .into_iter()
        .map(TradeResponse::from)
        .collect();
    
    Ok(FastJson(TradesResponse {
        trades: trade_responses,
        market_id,
    }).into_response())
}

/// Get market ticker (summary statistics)
pub async fn get_market_ticker(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
) -> Result<impl IntoResponse, StatusCode> {
    let engine = &state.trading_engine;
    
    // Get order books for both outcomes
    let book0 = engine.get_order_book(market_id, 0, 1).await;
    let book1 = engine.get_order_book(market_id, 1, 1).await;
    
    // Get recent trades
    let trades = engine.get_recent_trades(market_id, 100).await;
    
    // Calculate 24h stats
    let now = chrono::Utc::now();
    let yesterday = now - chrono::Duration::days(1);
    let trades_24h: Vec<_> = trades.into_iter()
        .filter(|t| t.timestamp > yesterday)
        .collect();
    
    let volume_24h = trades_24h.iter()
        .map(|t| t.amount * t.price)
        .sum::<rust_decimal::Decimal>();
    
    let ticker = MarketTicker {
        market_id,
        outcome_0: OutcomeTicker {
            best_back: book0.backs.first().map(|l| l.price),
            best_lay: book0.lays.first().map(|l| l.price),
            last_price: trades_24h.iter()
                .filter(|t| t.outcome == 0)
                .map(|t| t.price)
                .next(),
            volume_24h: trades_24h.iter()
                .filter(|t| t.outcome == 0)
                .map(|t| t.amount)
                .sum(),
        },
        outcome_1: OutcomeTicker {
            best_back: book1.backs.first().map(|l| l.price),
            best_lay: book1.lays.first().map(|l| l.price),
            last_price: trades_24h.iter()
                .filter(|t| t.outcome == 1)
                .map(|t| t.price)
                .next(),
            volume_24h: trades_24h.iter()
                .filter(|t| t.outcome == 1)
                .map(|t| t.amount)
                .sum(),
        },
        total_volume_24h: volume_24h,
        trade_count_24h: trades_24h.len(),
        timestamp: now,
    };
    
    Ok(Json(ticker).into_response())
}

// Query parameters
#[derive(Debug, Deserialize)]
pub struct OrdersQuery {
    pub status: Option<String>,
    pub market_id: Option<u128>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct OrderBookQuery {
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    pub limit: Option<usize>,
}

// Response types
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub market_id: u128,
    pub outcome: u8,
    pub side: String,
    pub order_type: String,
    pub amount: String,
    pub price: Option<String>,
    pub time_in_force: String,
    pub status: String,
    pub filled_amount: String,
    pub average_price: Option<String>,
    pub fees_paid: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub client_order_id: Option<String>,
}

impl From<Order> for OrderResponse {
    fn from(order: Order) -> Self {
        Self {
            id: order.id,
            market_id: order.market_id,
            outcome: order.outcome,
            side: if order.side == crate::trading_engine::Side::Back { 
                "back".to_string() 
            } else { 
                "lay".to_string() 
            },
            order_type: match order.order_type {
                crate::trading_engine::OrderType::Market => "market".to_string(),
                crate::trading_engine::OrderType::Limit { .. } => "limit".to_string(),
                crate::trading_engine::OrderType::PostOnly { .. } => "post_only".to_string(),
            },
            amount: order.amount.to_string(),
            price: order.price.map(|p| p.to_string()),
            time_in_force: match order.time_in_force {
                crate::trading_engine::TimeInForce::GTC => "GTC".to_string(),
                crate::trading_engine::TimeInForce::IOC => "IOC".to_string(),
                crate::trading_engine::TimeInForce::FOK => "FOK".to_string(),
                crate::trading_engine::TimeInForce::GTD(_) => "GTD".to_string(),
            },
            status: match order.status {
                crate::trading_engine::OrderStatus::New => "new".to_string(),
                crate::trading_engine::OrderStatus::PartiallyFilled { .. } => "partially_filled".to_string(),
                crate::trading_engine::OrderStatus::Filled => "filled".to_string(),
                crate::trading_engine::OrderStatus::Cancelled => "cancelled".to_string(),
                crate::trading_engine::OrderStatus::Rejected { .. } => "rejected".to_string(),
                crate::trading_engine::OrderStatus::Expired => "expired".to_string(),
            },
            filled_amount: order.filled_amount.to_string(),
            average_price: order.average_price.map(|p| p.to_string()),
            fees_paid: order.fees_paid.to_string(),
            created_at: order.created_at.timestamp(),
            updated_at: order.updated_at.timestamp(),
            client_order_id: order.client_order_id,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrdersResponse {
    pub orders: Vec<OrderResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Serialize)]
pub struct OrderBookResponse {
    pub market_id: u128,
    pub outcome: u8,
    pub backs: Vec<PriceLevelResponse>,
    pub lays: Vec<PriceLevelResponse>,
    pub sequence: u64,
    pub timestamp: i64,
}

impl From<OrderBookSnapshot> for OrderBookResponse {
    fn from(snapshot: OrderBookSnapshot) -> Self {
        Self {
            market_id: snapshot.market_id,
            outcome: snapshot.outcome,
            backs: snapshot.backs.into_iter().map(Into::into).collect(),
            lays: snapshot.lays.into_iter().map(Into::into).collect(),
            sequence: snapshot.sequence,
            timestamp: snapshot.timestamp.timestamp(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PriceLevelResponse {
    pub price: String,
    pub amount: String,
    pub orders: usize,
}

impl From<crate::trading_engine::PriceLevel> for PriceLevelResponse {
    fn from(level: crate::trading_engine::PriceLevel) -> Self {
        Self {
            price: level.price.to_string(),
            amount: level.amount.to_string(),
            orders: level.orders,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TradeResponse {
    pub id: String,
    pub market_id: u128,
    pub outcome: u8,
    pub price: String,
    pub amount: String,
    pub maker_order_id: String,
    pub taker_order_id: String,
    pub maker_side: String,
    pub taker_side: String,
    pub timestamp: i64,
    pub sequence: u64,
}

impl From<Trade> for TradeResponse {
    fn from(trade: Trade) -> Self {
        Self {
            id: trade.id,
            market_id: trade.market_id,
            outcome: trade.outcome,
            price: trade.price.to_string(),
            amount: trade.amount.to_string(),
            maker_order_id: trade.maker_order_id,
            taker_order_id: trade.taker_order_id,
            maker_side: if trade.maker_side == crate::trading_engine::Side::Back {
                "back".to_string()
            } else {
                "lay".to_string()
            },
            taker_side: if trade.taker_side == crate::trading_engine::Side::Back {
                "back".to_string()
            } else {
                "lay".to_string()
            },
            timestamp: trade.timestamp.timestamp(),
            sequence: trade.sequence,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TradesResponse {
    pub trades: Vec<TradeResponse>,
    pub market_id: u128,
}

#[derive(Debug, Serialize)]
pub struct MarketTicker {
    pub market_id: u128,
    pub outcome_0: OutcomeTicker,
    pub outcome_1: OutcomeTicker,
    pub total_volume_24h: rust_decimal::Decimal,
    pub trade_count_24h: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct OutcomeTicker {
    pub best_back: Option<rust_decimal::Decimal>,
    pub best_lay: Option<rust_decimal::Decimal>,
    pub last_price: Option<rust_decimal::Decimal>,
    pub volume_24h: rust_decimal::Decimal,
}