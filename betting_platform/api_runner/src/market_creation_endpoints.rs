//! REST API endpoints for market creation

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
    market_creation_service::{
        MarketCreationService,
        CreateMarketRequest,
        CreateMarketResponse,
        UpdateMarketRequest,
    },
    jwt_validation::AuthenticatedUser,
    rbac_authorization::Permission,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    tracing_middleware::get_correlation_id,
};

/// Query parameters for listing markets
#[derive(Debug, Deserialize)]
pub struct ListMarketsQuery {
    pub creator: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Market listing response
#[derive(Debug, Serialize)]
pub struct MarketListResponse {
    pub markets: Vec<MarketSummary>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// Market summary for listing
#[derive(Debug, Serialize)]
pub struct MarketSummary {
    pub market_id: u128,
    pub title: String,
    pub category: String,
    pub creator: String,
    pub end_time: i64,
    pub total_volume: u64,
    pub total_liquidity: u64,
    pub status: String,
}

/// Create new market
pub async fn create_market(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<CreateMarketRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::CreateMarkets) {
        return Err(AppError::new(
            ErrorKind::Forbidden,
            "Insufficient permissions to create market",
            ErrorContext::new("market_creation", "create"),
        ));
    }
    
    // Get market creation service
    let service = get_market_creation_service(&state)?;
    
    // Get correlation ID
    let correlation_id = crate::tracing_logger::CorrelationId::new();
    
    // Parse creator pubkey
    let creator = auth.claims.wallet.parse().map_err(|_| {
        AppError::new(
            ErrorKind::ValidationError,
            "Invalid wallet address",
            ErrorContext::new("market_creation", "parse_creator"),
        )
    })?;
    
    // Create market
    let response = service.create_market(request, creator, &correlation_id).await?;
    
    Ok(Json(response))
}

/// Update existing market
pub async fn update_market(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(market_id): Path<u128>,
    Json(mut request): Json<UpdateMarketRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Set market ID from path
    request.market_id = market_id;
    
    // Get market creation service
    let service = get_market_creation_service(&state)?;
    
    // Get correlation ID
    let correlation_id = crate::tracing_logger::CorrelationId::new();
    
    // Parse updater pubkey
    let updater = auth.claims.wallet.parse().map_err(|_| {
        AppError::new(
            ErrorKind::ValidationError,
            "Invalid wallet address",
            ErrorContext::new("market_update", "parse_updater"),
        )
    })?;
    
    // Update market
    service.update_market(request, updater, &correlation_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// Get market details
pub async fn get_market(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
) -> Result<impl IntoResponse, AppError> {
    // Get from database
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                ErrorContext::new("market_get", "database"),
            )
        })?;
        
        let row = client.query_one(
            r#"
            SELECT 
                market_id, title, description, creator, market_address,
                outcomes, end_time, resolution_time, category, tags,
                amm_type, initial_liquidity, creator_fee_bps, platform_fee_bps,
                min_bet_amount, max_bet_amount, oracle_sources,
                created_at, updated_at
            FROM markets 
            WHERE market_id = $1
            "#,
            &[&(market_id as i64)],
        ).await.map_err(|_| {
            AppError::new(
                ErrorKind::NotFound,
                format!("Market {} not found", market_id),
                ErrorContext::new("market_get", "query"),
            )
        })?;
        
        let market = serde_json::json!({
            "market_id": row.get::<_, i64>(0) as u128,
            "title": row.get::<_, String>(1),
            "description": row.get::<_, String>(2),
            "creator": row.get::<_, String>(3),
            "market_address": row.get::<_, String>(4),
            "outcomes": row.get::<_, serde_json::Value>(5),
            "end_time": row.get::<_, chrono::DateTime<chrono::Utc>>(6).timestamp(),
            "resolution_time": row.get::<_, chrono::DateTime<chrono::Utc>>(7).timestamp(),
            "category": row.get::<_, String>(8),
            "tags": row.get::<_, Vec<String>>(9),
            "amm_type": row.get::<_, String>(10),
            "initial_liquidity": row.get::<_, i64>(11) as u64,
            "creator_fee_bps": row.get::<_, i32>(12) as u16,
            "platform_fee_bps": row.get::<_, i32>(13) as u16,
            "min_bet_amount": row.get::<_, i64>(14) as u64,
            "max_bet_amount": row.get::<_, i64>(15) as u64,
            "oracle_sources": row.get::<_, serde_json::Value>(16),
            "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(17).timestamp(),
            "updated_at": row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(18)
                .map(|dt| dt.timestamp()),
        });
        
        Ok(Json(market))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            ErrorContext::new("market_get", "database"),
        ))
    }
}

/// List markets with filters
pub async fn list_markets(
    State(state): State<AppState>,
    Query(query): Query<ListMarketsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                ErrorContext::new("market_list", "database"),
            )
        })?;
        
        // Build query
        let mut sql = String::from(
            r#"
            SELECT 
                market_id, title, category, creator, end_time,
                COALESCE(total_volume, 0) as total_volume,
                COALESCE(total_liquidity, initial_liquidity) as total_liquidity,
                CASE 
                    WHEN end_time < NOW() THEN 'closed'
                    WHEN resolution_time < NOW() THEN 'resolving'
                    ELSE 'active'
                END as status
            FROM markets
            WHERE 1=1
            "#
        );
        
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = vec![];
        let mut param_count = 1;
        
        if let Some(creator) = query.creator {
            sql.push_str(&format!(" AND creator = ${}", param_count));
            params.push(Box::new(creator));
            param_count += 1;
        }
        
        if let Some(category) = query.category {
            sql.push_str(&format!(" AND category = ${}", param_count));
            params.push(Box::new(category));
            param_count += 1;
        }
        
        if let Some(status) = query.status {
            match status.as_str() {
                "active" => sql.push_str(" AND end_time > NOW() AND resolution_time > NOW()"),
                "closed" => sql.push_str(" AND end_time < NOW()"),
                "resolving" => sql.push_str(" AND end_time < NOW() AND resolution_time > NOW()"),
                _ => {}
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
                format!("Failed to query markets: {}", e),
                ErrorContext::new("market_list", "query"),
            )
        })?;
        
        let markets: Vec<MarketSummary> = rows.iter().map(|row| {
            MarketSummary {
                market_id: row.get::<_, i64>(0) as u128,
                title: row.get(1),
                category: row.get(2),
                creator: row.get(3),
                end_time: row.get::<_, chrono::DateTime<chrono::Utc>>(4).timestamp(),
                total_volume: row.get::<_, i64>(5) as u64,
                total_liquidity: row.get::<_, i64>(6) as u64,
                status: row.get(7),
            }
        }).collect();
        
        // Get total count
        let count_sql = "SELECT COUNT(*) FROM markets WHERE 1=1";
        let total: i64 = client.query_one(count_sql, &[]).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to count markets: {}", e),
                ErrorContext::new("market_list", "count"),
            )
        })?.get(0);
        
        Ok(Json(MarketListResponse {
            markets,
            total: total as u64,
            limit,
            offset,
        }))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            ErrorContext::new("market_list", "database"),
        ))
    }
}

/// Get market statistics
pub async fn get_market_stats(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
) -> Result<impl IntoResponse, AppError> {
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                ErrorContext::new("market_stats", "database"),
            )
        })?;
        
        // Get market stats from various tables
        let stats = client.query_one(
            r#"
            SELECT 
                COUNT(DISTINCT t.user_wallet) as unique_traders,
                COUNT(t.id) as total_trades,
                COALESCE(SUM(t.amount), 0) as total_volume,
                COALESCE(AVG(t.amount), 0) as avg_trade_size,
                MAX(t.timestamp) as last_trade_time
            FROM trades t
            WHERE t.market_id = $1
            "#,
            &[&(market_id as i64)],
        ).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get market stats: {}", e),
                ErrorContext::new("market_stats", "query"),
            )
        })?;
        
        let response = serde_json::json!({
            "market_id": market_id,
            "unique_traders": stats.get::<_, i64>(0),
            "total_trades": stats.get::<_, i64>(1),
            "total_volume": stats.get::<_, i64>(2) as u64,
            "avg_trade_size": stats.get::<_, f64>(3),
            "last_trade_time": stats.get::<_, Option<chrono::DateTime<chrono::Utc>>>(4)
                .map(|dt| dt.timestamp()),
        });
        
        Ok(Json(response))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            ErrorContext::new("market_stats", "database"),
        ))
    }
}

/// Helper to get market creation service
fn get_market_creation_service(state: &AppState) -> Result<Arc<MarketCreationService>, AppError> {
    state.market_creation_service.as_ref().cloned().ok_or_else(|| {
        AppError::new(
            ErrorKind::ConfigurationError,
            "Market creation service not configured",
            ErrorContext::new("market_creation", "service"),
        )
    })
}