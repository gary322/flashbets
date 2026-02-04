//! REST API endpoints for trade execution

use axum::{
    extract::{State, Path, Query},
    response::IntoResponse,
    Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::str::FromStr;

use crate::{
    AppState,
    trade_execution_service::{
        TradeExecutionService,
        TradeExecutionRequest,
        TradeExecutionResponse,
    },
    jwt_validation::AuthenticatedUser,
    rbac_authorization::Permission,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    tracing_middleware::get_correlation_id,
};
use rust_decimal::prelude::ToPrimitive;

/// Order query parameters
#[derive(Debug, Deserialize)]
pub struct OrderQuery {
    pub market_id: Option<u128>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Order response
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub market_id: u128,
    pub user_wallet: String,
    pub side: String,
    pub outcome: u8,
    pub order_type: String,
    pub amount: u64,
    pub price: Option<f64>,
    pub filled_amount: u64,
    pub remaining_amount: u64,
    pub average_price: f64,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Trade history query
#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    pub market_id: Option<u128>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Trade history response
#[derive(Debug, Serialize)]
pub struct TradeHistoryResponse {
    pub trades: Vec<TradeRecord>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// Trade record
#[derive(Debug, Serialize)]
pub struct TradeRecord {
    pub trade_id: String,
    pub order_id: String,
    pub market_id: u128,
    pub side: String,
    pub outcome: u8,
    pub amount: u64,
    pub price: f64,
    pub total_cost: u64,
    pub fees: TradeFeesSummary,
    pub transaction_signature: Option<String>,
    pub executed_at: i64,
}

/// Trade fees summary
#[derive(Debug, Serialize)]
pub struct TradeFeesSummary {
    pub platform_fee: u64,
    pub creator_fee: u64,
    pub liquidity_fee: u64,
    pub gas_fee: u64,
    pub total: u64,
}

/// Execute trade
pub async fn execute_trade(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(mut request): Json<TradeExecutionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::PlaceTrades) {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Insufficient permissions to place trades",
            ErrorContext::new("trade_execution", "execute"),
        ));
    }
    
    // Set user wallet from auth
    request.user_wallet = auth.claims.wallet.clone();
    
    // Get trade execution service
    let service = get_trade_execution_service(&state)?;
    
    // Get correlation ID
    let correlation_id = crate::tracing_logger::CorrelationId::new();
    
    // Execute trade
    let response = service.execute_trade(request, &correlation_id).await?;
    
    Ok(Json(response))
}

/// Cancel order
pub async fn cancel_order(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(order_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // Check permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::CloseTrades) {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Insufficient permissions to cancel orders",
            ErrorContext::new("trade_execution", "cancel"),
        ));
    }
    
    // Get trade execution service
    let service = get_trade_execution_service(&state)?;
    
    // Get correlation ID
    let correlation_id = crate::tracing_logger::CorrelationId::new();
    
    // Cancel order
    service.cancel_order(
        &order_id,
        &auth.claims.wallet,
        &correlation_id,
    ).await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// Get user orders
pub async fn get_user_orders(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Query(query): Query<OrderQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                ErrorContext::new("trade_execution", "get_orders"),
            )
        })?;
        
        // Build query
        let mut sql = String::from(
            r#"
            SELECT 
                order_id, market_id, user_wallet, side, outcome,
                order_type, amount, price, filled_amount,
                (amount - filled_amount) as remaining_amount,
                CASE 
                    WHEN filled_amount > 0 THEN total_cost / filled_amount 
                    ELSE 0 
                END as average_price,
                status, created_at, updated_at
            FROM orders
            WHERE user_wallet = $1
            "#
        );
        
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = vec![
            Box::new(auth.claims.wallet.clone()),
        ];
        let mut param_count = 2;
        
        if let Some(market_id) = query.market_id {
            sql.push_str(&format!(" AND market_id = ${}", param_count));
            params.push(Box::new(market_id as i64));
            param_count += 1;
        }
        
        if let Some(status) = query.status {
            sql.push_str(&format!(" AND status = ${}", param_count));
            params.push(Box::new(status));
            param_count += 1;
        }
        
        sql.push_str(" ORDER BY created_at DESC");
        sql.push_str(&format!(" LIMIT ${} OFFSET ${}", param_count, param_count + 1));
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));
        
        let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = 
            params.iter().map(|p| p.as_ref()).collect();
        
        let rows = client.query(&sql, &params_refs).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to query orders: {}", e),
                ErrorContext::new("trade_execution", "query"),
            )
        })?;
        
        let orders: Vec<OrderResponse> = rows.iter().map(|row| {
            OrderResponse {
                order_id: row.get(0),
                market_id: row.get::<_, i64>(1) as u128,
                user_wallet: row.get(2),
                side: row.get(3),
                outcome: row.get::<_, i32>(4) as u8,
                order_type: row.get(5),
                amount: row.get::<_, i64>(6) as u64,
                price: row.get::<_, Option<f64>>(7),
                filled_amount: row.get::<_, i64>(8) as u64,
                remaining_amount: row.get::<_, i64>(9) as u64,
                average_price: row.get(10),
                status: row.get(11),
                created_at: row.get::<_, chrono::DateTime<chrono::Utc>>(12).timestamp(),
                updated_at: row.get::<_, chrono::DateTime<chrono::Utc>>(13).timestamp(),
            }
        }).collect();
        
        Ok(Json(orders))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            ErrorContext::new("trade_execution", "database"),
        ))
    }
}

/// Get trade history
pub async fn get_trade_history(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Query(query): Query<TradeHistoryQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                ErrorContext::new("trade_execution", "get_trades"),
            )
        })?;
        
        // Build query
        let mut sql = String::from(
            r#"
            SELECT 
                trade_id, order_id, market_id, side, outcome,
                amount, price, total_cost, platform_fee, creator_fee,
                liquidity_fee, gas_fee, transaction_signature, created_at
            FROM trades
            WHERE user_wallet = $1
            "#
        );
        
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = vec![
            Box::new(auth.claims.wallet.clone()),
        ];
        let mut param_count = 2;
        
        if let Some(market_id) = query.market_id {
            sql.push_str(&format!(" AND market_id = ${}", param_count));
            params.push(Box::new(market_id as i64));
            param_count += 1;
        }
        
        if let Some(from_date) = query.from_date {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&from_date) {
                sql.push_str(&format!(" AND created_at >= ${}", param_count));
                params.push(Box::new(date.with_timezone(&chrono::Utc)));
                param_count += 1;
            }
        }
        
        if let Some(to_date) = query.to_date {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&to_date) {
                sql.push_str(&format!(" AND created_at <= ${}", param_count));
                params.push(Box::new(date.with_timezone(&chrono::Utc)));
                param_count += 1;
            }
        }
        
        sql.push_str(" ORDER BY created_at DESC");
        sql.push_str(&format!(" LIMIT ${} OFFSET ${}", param_count, param_count + 1));
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));
        
        let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = 
            params.iter().map(|p| p.as_ref()).collect();
        
        let rows = client.query(&sql, &params_refs).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to query trades: {}", e),
                ErrorContext::new("trade_execution", "query"),
            )
        })?;
        
        let trades: Vec<TradeRecord> = rows.iter().map(|row| {
            TradeRecord {
                trade_id: row.get(0),
                order_id: row.get(1),
                market_id: row.get::<_, i64>(2) as u128,
                side: row.get(3),
                outcome: row.get::<_, i32>(4) as u8,
                amount: row.get::<_, i64>(5) as u64,
                price: row.get(6),
                total_cost: row.get::<_, i64>(7) as u64,
                fees: TradeFeesSummary {
                    platform_fee: row.get::<_, i64>(8) as u64,
                    creator_fee: row.get::<_, i64>(9) as u64,
                    liquidity_fee: row.get::<_, i64>(10) as u64,
                    gas_fee: row.get::<_, i64>(11) as u64,
                    total: (row.get::<_, i64>(8) + row.get::<_, i64>(9) + 
                           row.get::<_, i64>(10) + row.get::<_, i64>(11)) as u64,
                },
                transaction_signature: row.get(12),
                executed_at: row.get::<_, chrono::DateTime<chrono::Utc>>(13).timestamp(),
            }
        }).collect();
        
        // Get total count
        let count_sql = "SELECT COUNT(*) FROM trades WHERE user_wallet = $1";
        let total: i64 = client.query_one(count_sql, &[&auth.claims.wallet]).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to count user orders: {}", e),
                ErrorContext::new("user_orders", "count"),
            )
        })?.get(0);
        
        Ok(Json(TradeHistoryResponse {
            trades,
            total: total as u64,
            limit,
            offset,
        }))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            ErrorContext::new("trade_execution", "database"),
        ))
    }
}

/// Get market order book
pub async fn get_order_book(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
    Query(params): Query<OrderBookQuery>,
) -> Result<impl IntoResponse, AppError> {
    let depth = params.depth.unwrap_or(10).min(50);
    let outcome = params.outcome.unwrap_or(0);
    
    // Get from trading engine (it's not an Option in lib.rs AppState)
    let order_book = state.trading_engine
        .get_order_book(market_id, outcome, depth as usize)
        .await;
    
    // Calculate spread and mid price before moving the vectors
    let best_back_price = order_book.backs.first().map(|l| l.price.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
    let best_lay_price = order_book.lays.first().map(|l| l.price.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
    let spread = if best_back_price > 0.0 && best_lay_price > 0.0 {
        best_lay_price - best_back_price
    } else {
        0.0
    };
    let mid_price = if best_back_price > 0.0 && best_lay_price > 0.0 {
        (best_back_price + best_lay_price) / 2.0
    } else {
        0.0
    };
    
    let response = serde_json::json!({
        "market_id": market_id,
        "outcome": outcome,
        "bids": order_book.backs.into_iter().map(|level| {
            serde_json::json!({
                "price": level.price,
                "amount": level.amount,
                "orders": level.orders,
            })
        }).collect::<Vec<_>>(),
        "asks": order_book.lays.into_iter().map(|level| {
            serde_json::json!({
                "price": level.price,
                "amount": level.amount,
                "orders": level.orders,
            })
        }).collect::<Vec<_>>(),
        "spread": spread,
        "mid_price": mid_price,
        "timestamp": chrono::Utc::now().timestamp(),
    });
    
    Ok(Json(response))
}

/// Order book query parameters
#[derive(Debug, Deserialize)]
pub struct OrderBookQuery {
    pub outcome: Option<u8>,
    pub depth: Option<u32>,
}

/// Get execution statistics
pub async fn get_execution_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                ErrorContext::new("trade_execution", "stats"),
            )
        })?;
        
        let stats = client.query_one(
            r#"
            SELECT 
                COUNT(*) as total_trades,
                COALESCE(SUM(amount), 0) as total_volume,
                COUNT(DISTINCT user_wallet) as unique_traders,
                COUNT(DISTINCT market_id) as markets_traded,
                AVG(price) as avg_price,
                COALESCE(SUM(platform_fee + creator_fee + liquidity_fee), 0) as total_fees
            FROM trades
            WHERE created_at >= NOW() - INTERVAL '24 hours'
            "#,
            &[],
        ).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get execution stats: {}", e),
                ErrorContext::new("trade_execution", "query"),
            )
        })?;
        
        let response = serde_json::json!({
            "total_trades_24h": stats.get::<_, i64>(0),
            "total_volume_24h": stats.get::<_, i64>(1) as u64,
            "unique_traders_24h": stats.get::<_, i64>(2),
            "markets_traded_24h": stats.get::<_, i64>(3),
            "avg_price_24h": stats.get::<_, Option<f64>>(4).unwrap_or(0.0),
            "total_fees_24h": stats.get::<_, i64>(5) as u64,
            "timestamp": chrono::Utc::now().timestamp(),
        });
        
        Ok(Json(response))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            ErrorContext::new("trade_execution", "database"),
        ))
    }
}

/// Helper to get trade execution service
fn get_trade_execution_service(state: &AppState) -> Result<Arc<TradeExecutionService>, AppError> {
    state.trade_execution_service.as_ref().cloned().ok_or_else(|| {
        AppError::new(
            ErrorKind::ConfigurationError,
            "Trade execution service not configured",
            ErrorContext::new("trade_execution", "service"),
        )
    })
}