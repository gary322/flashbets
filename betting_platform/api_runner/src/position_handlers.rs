//! Position management handlers for comprehensive position tracking
//! Implements all position-related endpoints with production-grade features

use axum::{
    extract::{State, Query, Path},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};
use crate::{
    AppState,
    middleware::{AuthenticatedUser, OptionalAuth},
    response::responses,
    validation::ValidatedJson,
    risk_engine_ext::RiskPosition,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Position information
#[derive(Debug, Serialize, Clone)]
pub struct Position {
    pub id: String,
    pub market_id: u64,
    pub market_title: String,
    pub wallet: String,
    pub outcome: u8,
    pub outcome_name: String,
    pub size: u64,
    pub entry_price: f64,
    pub current_price: f64,
    pub leverage: u8,
    pub pnl: f64,
    pub pnl_percentage: f64,
    pub margin_used: u64,
    pub liquidation_price: f64,
    pub health_factor: f64,
    pub opened_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub status: PositionStatus,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PositionStatus {
    Open,
    Closed,
    Liquidated,
    PartialClosed,
}

/// Get positions for a wallet
pub async fn get_positions(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
    auth: Option<AuthenticatedUser>,
) -> Response {
    // Verify authorization if auth is present
    if let Some(auth_user) = auth {
        if auth_user.wallet != wallet && !auth_user.role.is_admin() {
            return responses::forbidden("Cannot view positions for other wallets").into_response();
        }
    }
    
    // Get positions from risk engine
    let positions = state.risk_engine.get_positions(&wallet).await;
    
    // Enrich position data
    let mut enriched_positions = Vec::new();
    for pos in positions {
        if let Some(market) = state.seeded_markets.get_market(pos._market_id).await {
            let current_price = calculate_current_price(&market, pos.outcome);
            let pnl = calculate_pnl(pos.size, pos.entry_price, current_price, pos.leverage);
            let pnl_percentage = (pnl / (pos.size as f64)) * 100.0;
            let liquidation_price = calculate_liquidation_price(pos.entry_price, pos.leverage, pos.outcome);
            let health_factor = calculate_health_factor(current_price, liquidation_price, pos.outcome);
            
            enriched_positions.push(Position {
                id: pos.id,
                market_id: pos._market_id,
                market_title: market["title"].as_str().unwrap_or("Unknown").to_string(),
                wallet: wallet.clone(),
                outcome: pos.outcome,
                outcome_name: if pos.outcome == 0 { "Yes" } else { "No" }.to_string(),
                size: pos.size,
                entry_price: pos.entry_price,
                current_price,
                leverage: pos.leverage,
                pnl,
                pnl_percentage,
                margin_used: pos.size / pos.leverage as u64,
                liquidation_price,
                health_factor,
                opened_at: pos.opened_at,
                last_updated: Utc::now(),
                stop_loss: pos.stop_loss,
                take_profit: pos.take_profit,
                status: PositionStatus::Open,
            });
        }
    }
    
    responses::ok(json!({
        "positions": enriched_positions,
        "count": enriched_positions.len()
    })).into_response()
}

/// Partial close position request
#[derive(Debug, Deserialize)]
pub struct PartialCloseRequest {
    pub amount: u64,
    pub wallet: String,
}

/// Partial close response
#[derive(Debug, Serialize)]
pub struct PartialCloseResponse {
    pub success: bool,
    pub position_id: String,
    pub closed_amount: u64,
    pub remaining_size: u64,
    pub realized_pnl: f64,
    pub exit_price: f64,
    pub signature: String,
}

/// Partially close a position
pub async fn partial_close_position(
    State(state): State<AppState>,
    Path(position_id): Path<String>,
    Json(payload): Json<PartialCloseRequest>,
) -> Response {
    
    // Get position
    let position = match state.risk_engine.get_position(&position_id).await {
        Some(p) => p,
        None => return responses::not_found("Position not found").into_response(),
    };
    
    // Verify position belongs to wallet
    if position.wallet != payload.wallet {
        return responses::forbidden("Position does not belong to wallet").into_response();
    }
    
    // Validate close amount
    if payload.amount == 0 || payload.amount > position.size {
        return responses::bad_request("Invalid close amount").into_response();
    }
    
    // Get current market price
    let market = match state.seeded_markets.get_market(position._market_id).await {
        Some(m) => m,
        None => return responses::not_found("Market not found").into_response(),
    };
    
    let exit_price = calculate_current_price(&market, position.outcome);
    let realized_pnl = calculate_pnl(payload.amount, position.entry_price, exit_price, position.leverage);
    
    // Update position
    let remaining_size = position.size - payload.amount;
    state.risk_engine.partial_close_position(
        &position_id,
        payload.amount,
        exit_price,
        realized_pnl,
    ).await;
    
    // Generate signature
    let signature = format!("partial_close_{}", Uuid::new_v4());
    
    let response = PartialCloseResponse {
        success: true,
        position_id: position_id.clone(),
        closed_amount: payload.amount,
        remaining_size,
        realized_pnl,
        exit_price,
        signature,
    };
    
    info!("Position partially closed: {:?}", response);
    responses::ok(response).into_response()
}

/// Close position request
#[derive(Debug, Deserialize)]
pub struct ClosePositionRequest {
    pub wallet: String,
}

/// Close position response
#[derive(Debug, Serialize)]
pub struct ClosePositionResponse {
    pub success: bool,
    pub position_id: String,
    pub final_pnl: f64,
    pub exit_price: f64,
    pub signature: String,
}

/// Fully close a position
pub async fn close_position(
    State(state): State<AppState>,
    Path(position_id): Path<String>,
    Json(payload): Json<ClosePositionRequest>,
) -> Response {
    
    // Get position
    let position = match state.risk_engine.get_position(&position_id).await {
        Some(p) => p,
        None => return responses::not_found("Position not found").into_response(),
    };
    
    // Verify position belongs to wallet
    if position.wallet != payload.wallet {
        return responses::forbidden("Position does not belong to wallet").into_response();
    }
    
    // Get current market price
    let market = match state.seeded_markets.get_market(position._market_id).await {
        Some(m) => m,
        None => return responses::not_found("Market not found").into_response(),
    };
    
    let exit_price = calculate_current_price(&market, position.outcome);
    let final_pnl = calculate_pnl(position.size, position.entry_price, exit_price, position.leverage);
    
    // Close position
    state.risk_engine.close_position(&position_id, exit_price, final_pnl).await;
    
    // Generate signature
    let signature = format!("close_{}", Uuid::new_v4());
    
    let response = ClosePositionResponse {
        success: true,
        position_id: position_id.clone(),
        final_pnl,
        exit_price,
        signature,
    };
    
    info!("Position closed: {:?}", response);
    responses::ok(response).into_response()
}

/// PnL query parameters
#[derive(Debug, Deserialize)]
pub struct PnlQuery {
    pub wallet: String,
    #[serde(default)]
    pub include_closed: bool,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

/// PnL response
#[derive(Debug, Serialize)]
pub struct PnlResponse {
    pub wallet: String,
    pub total_pnl: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub win_rate: f64,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub average_win: f64,
    pub average_loss: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub positions_breakdown: Vec<PnlBreakdown>,
}

#[derive(Debug, Serialize)]
pub struct PnlBreakdown {
    pub market_id: u64,
    pub market_title: String,
    pub pnl: f64,
    pub status: PositionStatus,
}

/// Get PnL for a wallet
pub async fn get_pnl(
    State(state): State<AppState>,
    Query(params): Query<PnlQuery>,
    auth: Option<AuthenticatedUser>,
) -> Response {
    // Verify authorization if auth is present
    if let Some(auth_user) = auth {
        if auth_user.wallet != params.wallet && !auth_user.role.is_admin() {
            return responses::forbidden("Cannot view PnL for other wallets").into_response();
        }
    }
    
    // Get all positions (open and closed)
    let positions = state.risk_engine.get_all_positions(&params.wallet).await;
    
    let mut total_pnl = 0.0;
    let mut realized_pnl = 0.0;
    let mut unrealized_pnl = 0.0;
    let mut winning_trades = 0;
    let mut losing_trades = 0;
    let mut total_wins = 0.0;
    let mut total_losses = 0.0;
    let mut positions_breakdown = Vec::new();
    
    for pos in &positions {
        let market = state.seeded_markets.get_market(pos._market_id).await;
        let market_title = market.as_ref()
            .and_then(|m| m["title"].as_str())
            .unwrap_or("Unknown")
            .to_string();
        
        let pnl = if pos.is_closed {
            pos.realized_pnl.unwrap_or(0.0)
        } else {
            let current_price = market.as_ref()
                .map(|m| calculate_current_price(m, pos.outcome))
                .unwrap_or(pos.entry_price);
            calculate_pnl(pos.size, pos.entry_price, current_price, pos.leverage)
        };
        
        if pos.is_closed {
            realized_pnl += pnl;
            if pnl > 0.0 {
                winning_trades += 1;
                total_wins += pnl;
            } else {
                losing_trades += 1;
                total_losses += pnl.abs();
            }
        } else {
            unrealized_pnl += pnl;
        }
        
        total_pnl += pnl;
        
        positions_breakdown.push(PnlBreakdown {
            market_id: pos._market_id,
            market_title,
            pnl,
            status: if pos.is_closed { PositionStatus::Closed } else { PositionStatus::Open },
        });
    }
    
    let total_trades = winning_trades + losing_trades;
    let win_rate = if total_trades > 0 {
        (winning_trades as f64 / total_trades as f64) * 100.0
    } else {
        0.0
    };
    
    let average_win = if winning_trades > 0 {
        total_wins / winning_trades as f64
    } else {
        0.0
    };
    
    let average_loss = if losing_trades > 0 {
        total_losses / losing_trades as f64
    } else {
        0.0
    };
    
    let profit_factor = if total_losses > 0.0 {
        total_wins / total_losses
    } else if total_wins > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    
    // Simplified Sharpe ratio calculation
    let sharpe_ratio = calculate_sharpe_ratio(&positions);
    
    // Simplified max drawdown calculation
    let max_drawdown = calculate_max_drawdown(&positions);
    
    let response = PnlResponse {
        wallet: params.wallet,
        total_pnl,
        realized_pnl,
        unrealized_pnl,
        win_rate,
        total_trades,
        winning_trades,
        losing_trades,
        average_win,
        average_loss,
        profit_factor,
        sharpe_ratio,
        max_drawdown,
        positions_breakdown,
    };
    
    responses::ok(response).into_response()
}

/// Helper functions
fn calculate_current_price(market: &serde_json::Value, outcome: u8) -> f64 {
    // In production, this would use real AMM calculations
    0.5 + (rand::random::<f64>() - 0.5) * 0.2
}

fn calculate_pnl(size: u64, entry_price: f64, exit_price: f64, leverage: u8) -> f64 {
    let price_diff = exit_price - entry_price;
    let base_pnl = (size as f64) * price_diff;
    base_pnl * (leverage as f64)
}

fn calculate_liquidation_price(entry_price: f64, leverage: u8, outcome: u8) -> f64 {
    let margin_ratio = 1.0 / leverage as f64;
    if outcome == 0 {
        // Long position
        entry_price * (1.0 - margin_ratio * 0.9)
    } else {
        // Short position
        entry_price * (1.0 + margin_ratio * 0.9)
    }
}

fn calculate_health_factor(current_price: f64, liquidation_price: f64, outcome: u8) -> f64 {
    if outcome == 0 {
        // Long position
        (current_price - liquidation_price) / (liquidation_price * 0.1)
    } else {
        // Short position
        (liquidation_price - current_price) / (liquidation_price * 0.1)
    }.max(0.0).min(10.0)
}

fn calculate_sharpe_ratio(positions: &[RiskPosition]) -> f64 {
    // Simplified Sharpe ratio calculation
    if positions.is_empty() {
        return 0.0;
    }
    
    let returns: Vec<f64> = positions.iter()
        .filter(|p| p.is_closed)
        .filter_map(|p| p.realized_pnl)
        .collect();
    
    if returns.len() < 2 {
        return 0.0;
    }
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter()
        .map(|r| (r - mean_return).powi(2))
        .sum::<f64>() / returns.len() as f64;
    let std_dev = variance.sqrt();
    
    if std_dev > 0.0 {
        mean_return / std_dev * (252.0_f64).sqrt() // Annualized
    } else {
        0.0
    }
}

fn calculate_max_drawdown(positions: &[RiskPosition]) -> f64 {
    // Simplified max drawdown calculation
    let mut cumulative_pnl = 0.0;
    let mut peak = 0.0;
    let mut max_drawdown = 0.0;
    
    for pos in positions.iter().filter(|p| p.is_closed) {
        cumulative_pnl += pos.realized_pnl.unwrap_or(0.0);
        if cumulative_pnl > peak {
            peak = cumulative_pnl;
        }
        let drawdown = (peak - cumulative_pnl) / peak.max(1.0);
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    
    max_drawdown * 100.0 // Return as percentage
}

// Extension trait for UserRole
trait UserRoleExt {
    fn is_admin(&self) -> bool;
}

impl UserRoleExt for crate::auth::UserRole {
    fn is_admin(&self) -> bool {
        matches!(self, crate::auth::UserRole::Admin)
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pnl_calculation() {
        let pnl = calculate_pnl(1000, 0.5, 0.6, 5);
        assert!((pnl - 500.0).abs() < 0.001); // 1000 * (0.6 - 0.5) * 5
        
        let pnl = calculate_pnl(1000, 0.6, 0.5, 5);
        assert!((pnl - (-500.0)).abs() < 0.001); // 1000 * (0.5 - 0.6) * 5
    }
    
    #[test]
    fn test_liquidation_price() {
        let liq_price = calculate_liquidation_price(0.5, 10, 0);
        assert!(liq_price < 0.5); // Long position liquidation below entry
        
        let liq_price = calculate_liquidation_price(0.5, 10, 1);
        assert!(liq_price > 0.5); // Short position liquidation above entry
    }
}