//! Settlement handlers for tracking Polymarket resolutions

use axum::{
    extract::{State, Path, Query},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, error, debug};
use chrono::{DateTime, Utc};

use crate::{
    AppState,
    response::{responses, ApiResponse},
};

/// Settlement status for a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSettlementStatus {
    pub market_id: String,
    pub condition_id: String,
    pub platform: String,
    pub is_resolved: bool,
    pub winning_outcome: Option<u8>,
    pub resolution_time: Option<DateTime<Utc>>,
    pub resolution_source: Option<String>,
    pub total_volume: f64,
    pub outcome_prices: Vec<f64>,
}

/// Get settlement status for a market
pub async fn get_settlement_status(
    Path(market_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // For Polymarket markets, we need to check their resolution status
    match state.polymarket_public_client.get_markets(100).await {
        Ok(markets) => {
            // Find the specific market
            if let Some(market) = markets.into_iter().find(|m| {
                m.condition_id == market_id || 
                m.question.contains(&market_id)
            }) {
                // Parse outcome prices from JSON string
                let outcome_prices: Vec<f64> = match serde_json::from_str(&market.outcome_prices) {
                    Ok(prices) => prices,
                    Err(_) => vec![0.5, 0.5], // Default for binary markets
                };
                
                // For now, markets are not resolved in our system
                // This would need to be tracked separately
                let status = MarketSettlementStatus {
                    market_id: market_id.clone(),
                    condition_id: market.condition_id,
                    platform: "polymarket".to_string(),
                    is_resolved: false, // We don't track resolution status yet
                    winning_outcome: None,
                    resolution_time: None,
                    resolution_source: None,
                    total_volume: market.volume_24hr,
                    outcome_prices,
                };
                
                responses::ok(json!(status)).into_response()
            } else {
                responses::ok(json!({
                    "error": "Market not found"
                })).into_response()
            }
        }
        Err(e) => {
            error!("Failed to fetch market data: {}", e);
            responses::service_unavailable(&format!("Failed to fetch market data: {}", e)).into_response()
        }
    }
}

/// Track user positions and their settlement status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettlementInfo {
    pub wallet_address: String,
    pub positions: Vec<PositionSettlement>,
    pub total_settled: f64,
    pub total_pending: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSettlement {
    pub market_id: String,
    pub outcome: u8,
    pub shares: f64,
    pub avg_price: f64,
    pub current_value: f64,
    pub is_settled: bool,
    pub payout: Option<f64>,
    pub profit_loss: Option<f64>,
}

/// Get user's settlement information
pub async fn get_user_settlements(
    Path(wallet_address): Path<String>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // In a real implementation, this would:
    // 1. Query user's positions from our database
    // 2. Check Polymarket for resolution status
    // 3. Calculate payouts based on winning outcomes
    
    // For now, return a mock response
    let info = UserSettlementInfo {
        wallet_address: wallet_address.clone(),
        positions: vec![],
        total_settled: 0.0,
        total_pending: 0.0,
    };
    
    responses::ok(json!(info))
}

/// Monitor markets for resolution
#[derive(Deserialize)]
pub struct MonitorQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub only_pending: bool,
}

fn default_limit() -> usize {
    50
}

/// Get markets pending settlement
pub async fn get_pending_settlements(
    Query(params): Query<MonitorQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.polymarket_public_client.get_markets(params.limit).await {
        Ok(markets) => {
            let pending_markets: Vec<_> = markets.into_iter()
                .filter(|m| {
                    // Market is closed
                    // Note: We don't track resolution status in our system yet
                    m.closed
                })
                .map(|m| {
                    json!({
                        "market_id": m.condition_id,
                        "question": m.question,
                        "end_date": m.end_date,
                        "volume": m.volume_24hr,
                        "outcome_prices": m.outcome_prices,
                        "status": "pending_resolution"
                    })
                })
                .collect();
            
            responses::ok(json!({
                "pending_count": pending_markets.len(),
                "markets": pending_markets
            })).into_response()
        }
        Err(e) => {
            error!("Failed to fetch markets: {}", e);
            responses::service_unavailable(&format!("Failed to fetch markets: {}", e)).into_response()
        }
    }
}

/// Settlement webhook for Polymarket events
#[derive(Deserialize)]
pub struct SettlementWebhook {
    pub event_type: String,
    pub condition_id: String,
    pub resolution: Option<u8>,
    pub timestamp: i64,
    pub signature: String,
}

/// Handle settlement webhook from Polymarket
pub async fn handle_settlement_webhook(
    State(_state): State<AppState>,
    Json(payload): Json<SettlementWebhook>,
) -> impl IntoResponse {
    // Verify webhook signature
    // In production, you would verify the signature using Polymarket's webhook secret
    
    info!(
        "Received settlement webhook: {} for condition {}",
        payload.event_type, payload.condition_id
    );
    
    match payload.event_type.as_str() {
        "market.resolved" => {
            // Market has been resolved
            if let Some(outcome) = payload.resolution {
                info!(
                    "Market {} resolved with outcome {}",
                    payload.condition_id, outcome
                );
                
                // Here you would:
                // 1. Update your database with resolution
                // 2. Notify users who had positions
                // 3. Update any internal tracking
                
                responses::ok(json!({
                    "status": "processed",
                    "condition_id": payload.condition_id,
                    "outcome": outcome
                }))
            } else {
                responses::ok(json!({
                    "error": "Missing resolution outcome"
                }))
            }
        }
        "market.disputed" => {
            // Market resolution is being disputed
            info!("Market {} resolution disputed", payload.condition_id);
            
            responses::ok(json!({
                "status": "acknowledged",
                "condition_id": payload.condition_id,
                "action": "dispute_noted"
            }))
        }
        _ => {
            responses::ok(json!({
                "error": "Unknown event type"
            }))
        }
    }
}

/// Get historical settlements
#[derive(Deserialize)]
pub struct HistoricalQuery {
    #[serde(default = "default_days")]
    pub days: u32,
    #[serde(default)]
    pub wallet: Option<String>,
}

fn default_days() -> u32 {
    7
}

/// Get historical settlement data
pub async fn get_historical_settlements(
    Query(params): Query<HistoricalQuery>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // This would query historical settlement data
    // For now, return empty response
    
    responses::ok(json!({
        "period_days": params.days,
        "wallet": params.wallet,
        "settlements": [],
        "total_markets_settled": 0,
        "total_volume_settled": 0.0
    }))
}

/// Oracle price at settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementOracle {
    pub oracle_type: String,
    pub reported_at: DateTime<Utc>,
    pub outcome: u8,
    pub confidence: f64,
    pub dispute_period_ends: Option<DateTime<Utc>>,
}

/// Get oracle information for a settled market
pub async fn get_settlement_oracle(
    Path(_market_id): Path<String>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // In Polymarket, UMA (Universal Market Access) is the primary oracle
    // This endpoint would provide details about the oracle resolution
    
    let oracle_info = SettlementOracle {
        oracle_type: "UMA Optimistic Oracle".to_string(),
        reported_at: Utc::now(),
        outcome: 0, // Would come from actual data
        confidence: 1.0, // UMA resolutions are binary
        dispute_period_ends: Some(Utc::now() + chrono::Duration::hours(2)),
    };
    
    responses::ok(json!(oracle_info))
}