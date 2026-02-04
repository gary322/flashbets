//! Database-integrated handlers for user and trade tracking

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{AppState, response::ApiResponse};
use crate::db::queries::*;
use chrono::{DateTime, Utc};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Record a new user login
pub async fn record_user_login(
    State(state): State<AppState>,
    Json(payload): Json<UserLoginRequest>,
) -> Response {
    let conn = match state.database.get_connection().await {
        Ok(conn) => conn,
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Database error: {}", e)).into_response(),
    };
    
    match UserQueries::create(&conn, &payload.wallet_address).await {
        Ok(user) => ApiResponse::success(UserLoginResponse {
            user_id: user.id,
            wallet_address: user.wallet_address,
            is_new: user.created_at == user.updated_at,
        }).into_response(),
        Err(e) => ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to record login: {}", e)).into_response(),
    }
}

/// Get user statistics
pub async fn get_user_stats(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let conn = match state.database.get_connection().await {
        Ok(conn) => conn,
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Database error: {}", e)).into_response(),
    };
    
    match UserQueries::get_by_wallet(&conn, &wallet).await {
        Ok(Some(user)) => ApiResponse::success(UserStatsResponse {
            wallet_address: user.wallet_address,
            total_volume: user.total_volume,
            total_trades: user.total_trades,
            created_at: user.created_at,
            last_login: user.last_login,
        }).into_response(),
        Ok(None) => ApiResponse::<()>::error("NOT_FOUND", "User not found").into_response(),
        Err(e) => ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to get user stats: {}", e)).into_response(),
    }
}

/// Record a trade in the database
pub async fn record_trade(
    State(state): State<AppState>,
    Json(payload): Json<RecordTradeRequest>,
) -> Response {
    let conn = match state.database.get_connection().await {
        Ok(conn) => conn,
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Database error: {}", e)).into_response(),
    };
    
    // Get user
    let user = match UserQueries::get_by_wallet(&conn, &payload.wallet_address).await {
        Ok(Some(user)) => user,
        Ok(None) => return ApiResponse::<()>::error("NOT_FOUND", "User not found").into_response(),
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to get user: {}", e)).into_response(),
    };
    
    // Get market
    let market = match MarketQueries::get_by_market_id(&conn, &payload.market_id).await {
        Ok(Some(market)) => market,
        Ok(None) => {
            // Create market if it doesn't exist
            match MarketQueries::upsert(
                &conn,
                &payload.market_id,
                &payload.chain,
                &payload.market_title,
                &payload.market_description,
                &payload.wallet_address,
                payload.market_end_time,
                serde_json::json!({}),
            ).await {
                Ok(market) => market,
                Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to create market: {}", e)).into_response(),
            }
        }
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to get market: {}", e)).into_response(),
    };
    
    // Record trade
    match TradeQueries::create(
        &conn,
        &payload.trade_id,
        user.id,
        market.id,
        payload.position_id,
        &payload.trade_type,
        payload.outcome,
        payload.amount,
        payload.price,
        payload.fee,
        &payload.signature,
    ).await {
        Ok(trade) => {
            // Update user stats
            let _ = UserQueries::update_stats(&conn, user.id, payload.amount, 1).await;
            
            ApiResponse::success(RecordTradeResponse {
                trade_id: trade.trade_id,
                status: trade.status,
                created_at: trade.created_at,
            }).into_response()
        }
        Err(e) => ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to record trade: {}", e)).into_response(),
    }
}

/// Get user trade history
pub async fn get_user_trades(
    Path(wallet): Path<String>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Response {
    let conn = match state.database.get_connection().await {
        Ok(conn) => conn,
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Database error: {}", e)).into_response(),
    };
    
    // Get user
    let user = match UserQueries::get_by_wallet(&conn, &wallet).await {
        Ok(Some(user)) => user,
        Ok(None) => return ApiResponse::<()>::error("NOT_FOUND", "User not found").into_response(),
        Err(e) => return ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to get user: {}", e)).into_response(),
    };
    
    let limit = pagination.limit.unwrap_or(50).min(100);
    let offset = pagination.offset.unwrap_or(0);
    
    match TradeQueries::get_user_history(&conn, user.id, limit, offset).await {
        Ok(trades) => ApiResponse::success(trades).into_response(),
        Err(e) => ApiResponse::<()>::error("DATABASE_ERROR", &format!("Failed to get trades: {}", e)).into_response(),
    }
}

/// Get database pool status
pub async fn get_db_status(
    State(state): State<AppState>,
) -> Response {
    let status = state.database.pool_status();
    ApiResponse::success(status).into_response()
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct UserLoginRequest {
    pub wallet_address: String,
}

#[derive(Debug, Serialize)]
pub struct UserLoginResponse {
    pub user_id: i64,
    pub wallet_address: String,
    pub is_new: bool,
}

#[derive(Debug, Serialize)]
pub struct UserStatsResponse {
    pub wallet_address: String,
    pub total_volume: i64,
    pub total_trades: i32,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RecordTradeRequest {
    pub trade_id: String,
    pub wallet_address: String,
    pub market_id: String,
    pub position_id: Option<i64>,
    pub chain: String,
    pub market_title: String,
    pub market_description: String,
    pub market_end_time: DateTime<Utc>,
    pub trade_type: String,
    pub outcome: i16,
    pub amount: i64,
    pub price: f64,
    pub fee: i64,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct RecordTradeResponse {
    pub trade_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}