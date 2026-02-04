//! REST API endpoints for settlement operations

use axum::{
    extract::{State, Path, Query},
    response::IntoResponse,
    Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use std::sync::Arc;

use crate::{
    AppState,
    jwt_validation::AuthenticatedUser,
    rbac_authorization::{Permission, RequireRole},
    settlement_service::{
        SettlementService,
        SettlementRequest,
        SettlementResult,
        SettlementStatus,
        OracleResult,
        PositionSettlement,
    },
    tracing_logger::CorrelationId,
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Settlement query parameters
#[derive(Debug, Deserialize)]
pub struct SettlementQuery {
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Settlement history response
#[derive(Debug, Serialize)]
pub struct SettlementHistory {
    pub settlements: Vec<SettlementRecord>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// Settlement record
#[derive(Debug, Serialize)]
pub struct SettlementRecord {
    pub settlement_id: String,
    pub market_id: u128,
    pub market_title: String,
    pub winning_outcome: u8,
    pub total_positions: u64,
    pub total_payout: u64,
    pub oracle_consensus: f64,
    pub settled_at: chrono::DateTime<chrono::Utc>,
    pub transaction_signature: String,
}

/// Oracle status response
#[derive(Debug, Serialize)]
pub struct OracleStatusResponse {
    pub market_id: u128,
    pub oracle_results: Vec<OracleResult>,
    pub consensus_outcome: Option<u8>,
    pub consensus_confidence: Option<f64>,
    pub can_settle: bool,
    pub reason: Option<String>,
}

/// Initiate market settlement (admin only)
pub async fn initiate_settlement(
    State(state): State<AppState>,
    _role: RequireRole,
    Json(request): Json<SettlementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "initiate");
    
    // Get settlement service
    let service = get_settlement_service(&state)?;
    
    // Get correlation ID
    let correlation_id = CorrelationId::new();
    
    // Initiate settlement
    let result = service.initiate_settlement(request, &correlation_id).await?;
    
    Ok(Json(result))
}

/// Query oracles for market resolution
pub async fn query_oracles(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
    _auth: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "query_oracles");
    
    // Get settlement service
    let service = get_settlement_service(&state)?;
    
    // Get market from settlement service
    let market = get_settlement_market(&state, market_id).await?;
    
    // Get correlation ID
    let correlation_id = CorrelationId::new();
    
    // Query oracles
    let oracle_results = service.query_oracles(&market, &correlation_id).await?;
    
    // Calculate consensus
    let (consensus_outcome, consensus_confidence, can_settle, reason) = 
        calculate_consensus(&oracle_results);
    
    Ok(Json(OracleStatusResponse {
        market_id,
        oracle_results,
        consensus_outcome,
        consensus_confidence,
        can_settle,
        reason,
    }))
}

/// Get settlement status for a market
pub async fn get_settlement_status(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "get_status");
    
    // Get settlement service
    let service = get_settlement_service(&state)?;
    
    // Get status
    let status = service.get_settlement_status(market_id).await?;
    
    Ok(Json(serde_json::json!({
        "market_id": market_id,
        "status": status,
        "timestamp": chrono::Utc::now(),
    })))
}

/// Get user's settlement history
pub async fn get_user_settlements(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Query(query): Query<SettlementQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "user_history");
    
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                context.clone(),
            )
        })?;
        
        // Build query
        let mut sql = String::from(
            r#"
            SELECT 
                s.settlement_id, s.market_id, m.title, s.winning_outcome,
                s.position_id, s.shares, s.payout, s.pnl, s.fees,
                s.settlement_price, s.settled_at, s.transaction_signature
            FROM position_settlements s
            JOIN markets m ON m.id = s.market_id
            WHERE s.wallet = $1
            "#
        );
        
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = vec![
            Box::new(auth.claims.wallet.clone()),
        ];
        let mut param_count = 2;
        
        if let Some(status) = query.status {
            sql.push_str(&format!(" AND s.status = ${}", param_count));
            params.push(Box::new(status));
            param_count += 1;
        }
        
        if let Some(from_date) = query.from_date {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&from_date) {
                sql.push_str(&format!(" AND s.settled_at >= ${}", param_count));
                params.push(Box::new(date.with_timezone(&chrono::Utc)));
                param_count += 1;
            }
        }
        
        if let Some(to_date) = query.to_date {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&to_date) {
                sql.push_str(&format!(" AND s.settled_at <= ${}", param_count));
                params.push(Box::new(date.with_timezone(&chrono::Utc)));
                param_count += 1;
            }
        }
        
        sql.push_str(" ORDER BY s.settled_at DESC");
        sql.push_str(&format!(" LIMIT ${} OFFSET ${}", param_count, param_count + 1));
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));
        
        let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = 
            params.iter().map(|p| p.as_ref()).collect();
        
        let rows = client.query(&sql, &params_refs).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to query settlements: {}", e),
                context.clone(),
            )
        })?;
        
        let settlements: Vec<serde_json::Value> = rows.iter().map(|row| {
            serde_json::json!({
                "settlement_id": row.get::<_, String>(0),
                "market_id": row.get::<_, i64>(1) as u128,
                "market_title": row.get::<_, String>(2),
                "winning_outcome": row.get::<_, i32>(3) as u8,
                "position_id": row.get::<_, String>(4),
                "shares": row.get::<_, i64>(5) as u64,
                "payout": row.get::<_, i64>(6) as u64,
                "pnl": row.get::<_, i64>(7),
                "fees": row.get::<_, i64>(8) as u64,
                "settlement_price": row.get::<_, f64>(9),
                "settled_at": row.get::<_, chrono::DateTime<chrono::Utc>>(10),
                "transaction_signature": row.get::<_, String>(11),
            })
        }).collect();
        
        // Get total count
        let count_sql = "SELECT COUNT(*) FROM position_settlements WHERE wallet = $1";
        let total: i64 = client.query_one(count_sql, &[&auth.claims.wallet])
            .await
            .map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to count settlements: {}", e),
                    context,
                )
            })?
            .get(0);
        
        Ok(Json(serde_json::json!({
            "settlements": settlements,
            "total": total,
            "limit": limit,
            "offset": offset,
        })))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            context,
        ))
    }
}

/// Get settlement history for all markets (admin only)
pub async fn get_settlement_history(
    State(state): State<AppState>,
    _role: RequireRole,
    Query(query): Query<SettlementQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "history");
    
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                context.clone(),
            )
        })?;
        
        let rows = client.query(
            r#"
            SELECT 
                s.settlement_id, s.market_id, m.title, s.winning_outcome,
                COUNT(DISTINCT s.position_id) as position_count,
                SUM(s.payout) as total_payout,
                s.oracle_consensus, s.settled_at, s.transaction_signature
            FROM settlement_batches s
            JOIN markets m ON m.id = s.market_id
            GROUP BY s.settlement_id, s.market_id, m.title, s.winning_outcome,
                     s.oracle_consensus, s.settled_at, s.transaction_signature
            ORDER BY s.settled_at DESC
            LIMIT $1 OFFSET $2
            "#,
            &[&(limit as i64), &(offset as i64)],
        ).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to query settlement history: {}", e),
                context.clone(),
            )
        })?;
        
        let settlements: Vec<SettlementRecord> = rows.iter().map(|row| {
            SettlementRecord {
                settlement_id: row.get(0),
                market_id: row.get::<_, i64>(1) as u128,
                market_title: row.get(2),
                winning_outcome: row.get::<_, i32>(3) as u8,
                total_positions: row.get::<_, i64>(4) as u64,
                total_payout: row.get::<_, i64>(5) as u64,
                oracle_consensus: row.get(6),
                settled_at: row.get(7),
                transaction_signature: row.get(8),
            }
        }).collect();
        
        // Get total count
        let total: i64 = client.query_one(
            "SELECT COUNT(*) FROM settlement_batches",
            &[],
        ).await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to count settlements: {}", e),
                context,
            )
        })?.get(0);
        
        Ok(Json(SettlementHistory {
            settlements,
            total: total as u64,
            limit,
            offset,
        }))
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            context,
        ))
    }
}

/// Helper to get settlement service
fn get_settlement_service(state: &AppState) -> Result<Arc<SettlementService>, AppError> {
    state.settlement_service.as_ref().cloned().ok_or_else(|| {
        AppError::new(
            ErrorKind::ConfigurationError,
            "Settlement service not configured",
            ErrorContext::new("settlement_endpoints", "service"),
        )
    })
}

/// Helper to get market
async fn get_market(state: &AppState, market_id: u128) -> Result<crate::types::Market, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "get_market");
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                context.clone(),
            )
        })?;
        
        let row = client.query_one(
            r#"
            SELECT 
                id, creator, title, description, category,
                outcome_count, total_volume, total_liquidity,
                end_time, resolution_time, status, created_at,
                current_price
            FROM markets
            WHERE id = $1
            "#,
            &[&(market_id as i64)],
        ).await.map_err(|e| {
            AppError::new(
                ErrorKind::NotFound,
                format!("Market not found: {}", e),
                context,
            )
        })?;
        
        Ok(crate::types::Market {
            id: row.get::<_, i64>(0) as u128,
            title: row.get(2),
            description: row.get(3),
            creator: solana_sdk::pubkey::Pubkey::new_unique(), // TODO: Get actual creator pubkey
            outcomes: vec![], // TODO: Load outcomes
            amm_type: crate::types::AmmType::Cpmm, // TODO: Get actual AMM type
            total_liquidity: row.get::<_, i64>(7) as u64,
            total_volume: row.get::<_, i64>(6) as u64,
            resolution_time: row.get(9),
            resolved: false, // TODO: Check resolution status
            winning_outcome: None,
            created_at: row.get(11),
            verse_id: None,
            current_price: row.get(12),
        })
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            context,
        ))
    }
}

/// Helper to get market for settlement service
async fn get_settlement_market(state: &AppState, market_id: u128) -> Result<crate::settlement_service::Market, AppError> {
    let context = ErrorContext::new("settlement_endpoints", "get_settlement_market");
    
    if let Ok(pool) = state.database.get_pool() {
        let client = pool.get().await.map_err(|e| {
            AppError::new(
                ErrorKind::DatabaseError,
                format!("Failed to get database connection: {}", e),
                context.clone(),
            )
        })?;
        
        let row = client.query_one(
            r#"
            SELECT 
                id, creator, title, description, category, tags,
                outcome_count, total_volume, total_liquidity,
                end_time, resolution_time, status, created_at, current_price
            FROM markets
            WHERE id = $1
            "#,
            &[&(market_id as i64)],
        ).await.map_err(|e| {
            AppError::new(
                ErrorKind::NotFound,
                format!("Market not found: {}", e),
                context.clone(),
            )
        })?;
        
        Ok(crate::settlement_service::Market {
            id: row.get::<_, i64>(0) as u128,
            pubkey: solana_sdk::pubkey::Pubkey::new_unique(), // TODO: Get actual pubkey
            creator: row.get(1),
            title: row.get(2),
            description: row.get(3),
            category: row.get(4),
            outcomes: vec![], // TODO: Load outcomes
            total_liquidity: row.get::<_, i64>(8) as u64,
            total_volume: row.get::<_, i64>(7) as u64,
            status: row.get(11),
            end_time: row.get(9),
            resolution_time: row.get(10),
            created_at: row.get(12),
            current_price: row.get(13),
        })
    } else {
        Err(AppError::new(
            ErrorKind::ServiceUnavailable,
            "Database not available",
            context,
        ))
    }
}

/// Calculate consensus from oracle results
fn calculate_consensus(oracle_results: &[OracleResult]) -> (Option<u8>, Option<f64>, bool, Option<String>) {
    if oracle_results.is_empty() {
        return (None, None, false, Some("No oracle results available".to_string()));
    }
    
    // Count votes for each outcome weighted by confidence
    let mut outcome_weights: std::collections::HashMap<u8, f64> = std::collections::HashMap::new();
    let mut total_weight = 0.0;
    
    for result in oracle_results {
        let weight = result.confidence;
        *outcome_weights.entry(result.outcome).or_insert(0.0) += weight;
        total_weight += weight;
    }
    
    // Find outcome with highest weight
    if let Some((outcome, weight)) = outcome_weights.iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
    {
        let confidence = weight / total_weight;
        let can_settle = confidence >= 0.66; // Require 66% consensus
        let reason = if !can_settle {
            Some(format!("Insufficient consensus: {:.1}%", confidence * 100.0))
        } else {
            None
        };
        
        (Some(*outcome), Some(confidence), can_settle, reason)
    } else {
        (None, None, false, Some("Failed to calculate consensus".to_string()))
    }
}