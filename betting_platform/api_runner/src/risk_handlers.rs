//! Risk management handlers for comprehensive risk control
//! Implements production-grade risk limits, margin management, and liquidation

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
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/// Risk limit configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct RiskLimits {
    pub wallet: String,
    pub max_position_size: u64,
    pub max_leverage: u8,
    pub max_daily_loss: u64,
    pub max_weekly_loss: u64,
    pub max_open_positions: u32,
    pub max_exposure: u64,
    pub margin_call_level: f64,  // 0.2 = 20%
    pub liquidation_level: f64,   // 0.1 = 10%
    pub auto_deleverage_enabled: bool,
    pub risk_rating: RiskRating,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RiskRating {
    Conservative,
    Moderate,
    Aggressive,
    Professional,
}

/// Set risk limits request
#[derive(Debug, Deserialize)]
pub struct SetRiskLimitsRequest {
    pub wallet: String,
    pub limits: RiskLimitUpdate,
}

#[derive(Debug, Deserialize)]
pub struct RiskLimitUpdate {
    pub max_position_size: Option<u64>,
    pub max_leverage: Option<u8>,
    pub max_daily_loss: Option<u64>,
    pub max_weekly_loss: Option<u64>,
    pub max_open_positions: Option<u32>,
    pub max_exposure: Option<u64>,
    pub margin_call_level: Option<f64>,
    pub liquidation_level: Option<f64>,
    pub auto_deleverage_enabled: Option<bool>,
    pub risk_rating: Option<RiskRating>,
}

/// Set risk limits response
#[derive(Debug, Serialize)]
pub struct SetRiskLimitsResponse {
    pub success: bool,
    pub limits: RiskLimits,
    pub warnings: Vec<String>,
}

/// Set or update risk limits
pub async fn set_risk_limits(
    State(state): State<AppState>,
    Json(payload): Json<SetRiskLimitsRequest>,
) -> Response {
    debug!("Set risk limits request: {:?}", payload);
    
    // Get current limits or create defaults
    let mut limits = get_or_create_risk_limits(&payload.wallet).await;
    let mut warnings = Vec::new();
    
    // Apply updates
    if let Some(max_position_size) = payload.limits.max_position_size {
        if max_position_size > 1_000_000 {
            warnings.push("Maximum position size exceeds recommended limit".to_string());
        }
        limits.max_position_size = max_position_size;
    }
    
    if let Some(max_leverage) = payload.limits.max_leverage {
        if max_leverage > 20 {
            warnings.push("Leverage exceeds 20x - extreme risk warning".to_string());
        }
        limits.max_leverage = max_leverage;
    }
    
    if let Some(max_daily_loss) = payload.limits.max_daily_loss {
        limits.max_daily_loss = max_daily_loss;
    }
    
    if let Some(max_weekly_loss) = payload.limits.max_weekly_loss {
        limits.max_weekly_loss = max_weekly_loss;
    }
    
    if let Some(max_open_positions) = payload.limits.max_open_positions {
        if max_open_positions > 50 {
            warnings.push("High number of open positions may impact performance".to_string());
        }
        limits.max_open_positions = max_open_positions;
    }
    
    if let Some(max_exposure) = payload.limits.max_exposure {
        limits.max_exposure = max_exposure;
    }
    
    if let Some(margin_call_level) = payload.limits.margin_call_level {
        if margin_call_level < 0.15 {
            warnings.push("Margin call level below 15% - high liquidation risk".to_string());
        }
        limits.margin_call_level = margin_call_level;
    }
    
    if let Some(liquidation_level) = payload.limits.liquidation_level {
        if liquidation_level < 0.05 {
            warnings.push("Liquidation level below 5% - extreme risk".to_string());
        }
        limits.liquidation_level = liquidation_level;
    }
    
    if let Some(auto_deleverage) = payload.limits.auto_deleverage_enabled {
        limits.auto_deleverage_enabled = auto_deleverage;
    }
    
    if let Some(risk_rating) = payload.limits.risk_rating {
        limits.risk_rating = risk_rating;
    }
    
    // Store updated limits
    state.risk_engine.update_risk_limits(&limits).await;
    
    let response = SetRiskLimitsResponse {
        success: true,
        limits,
        warnings,
    };
    
    info!("Risk limits updated: {:?}", response);
    responses::ok(response).into_response()
}

/// Get risk limits
pub async fn get_risk_limits(
    State(state): State<AppState>,
    auth: Option<AuthenticatedUser>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let wallet = match params.get("wallet") {
        Some(w) => w,
        None => return responses::bad_request("Wallet parameter required").into_response(),
    };
    
    // Verify authorization
    if let Some(auth_user) = auth {
        if auth_user.wallet != wallet.to_string() && !auth_user.role.is_admin() {
            return responses::forbidden("Cannot view risk limits for other wallets").into_response();
        }
    }
    
    let limits = get_or_create_risk_limits(wallet).await;
    responses::ok(limits).into_response()
}

/// Margin status
#[derive(Debug, Serialize)]
pub struct MarginStatus {
    pub wallet: String,
    pub total_margin_used: u64,
    pub total_margin_available: u64,
    pub margin_usage_percent: f64,
    pub margin_level: f64,
    pub status: MarginHealthStatus,
    pub positions: Vec<MarginPosition>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarginHealthStatus {
    Healthy,
    Warning,
    MarginCall,
    Liquidation,
}

#[derive(Debug, Serialize)]
pub struct MarginPosition {
    pub position_id: String,
    pub market_id: u64,
    pub margin_used: u64,
    pub current_value: u64,
    pub unrealized_pnl: f64,
    pub margin_ratio: f64,
}

/// Get margin requirements and status
pub async fn get_margin_status(
    State(state): State<AppState>,
    auth: Option<AuthenticatedUser>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let wallet = match params.get("wallet") {
        Some(w) => w,
        None => return responses::bad_request("Wallet parameter required").into_response(),
    };
    
    // Verify authorization
    if let Some(auth_user) = auth {
        if auth_user.wallet != wallet.to_string() && !auth_user.role.is_admin() {
            return responses::forbidden("Cannot view margin status for other wallets").into_response();
        }
    }
    
    // Get positions and calculate margin
    let positions = state.risk_engine.get_positions(wallet).await;
    let mut total_margin_used = 0;
    let mut margin_positions = Vec::new();
    let mut warnings = Vec::new();
    
    for pos in positions {
        if !pos.is_closed {
            let market = state.seeded_markets.get_market(pos._market_id).await;
            let current_price = market.as_ref()
                .and_then(|m| m["outcomes"][0]["total_stake"].as_f64())
                .unwrap_or(0.5);
            
            let current_value = (pos.size as f64 * current_price) as u64;
            let unrealized_pnl = (current_price - pos.entry_price) * pos.size as f64;
            let margin_ratio = pos.margin_used as f64 / current_value as f64;
            
            total_margin_used += pos.margin_used;
            
            if margin_ratio < 0.2 {
                warnings.push(format!("Position {} approaching margin call level", pos.id));
            }
            
            margin_positions.push(MarginPosition {
                position_id: pos.id,
                market_id: pos._market_id,
                margin_used: pos.margin_used,
                current_value,
                unrealized_pnl,
                margin_ratio,
            });
        }
    }
    
    // Get account balance (mock for now)
    let total_balance: u64 = 100_000;
    let total_margin_available = total_balance.saturating_sub(total_margin_used);
    let margin_usage_percent = (total_margin_used as f64 / total_balance as f64) * 100.0;
    let margin_level = if total_margin_used > 0 {
        total_balance as f64 / total_margin_used as f64
    } else {
        999.0
    };
    
    let status = if margin_level < 1.1 {
        MarginHealthStatus::Liquidation
    } else if margin_level < 1.2 {
        MarginHealthStatus::MarginCall
    } else if margin_level < 1.5 {
        MarginHealthStatus::Warning
    } else {
        MarginHealthStatus::Healthy
    };
    
    if matches!(status, MarginHealthStatus::MarginCall | MarginHealthStatus::Liquidation) {
        warnings.push("Account at risk of liquidation - reduce positions or add margin".to_string());
    }
    
    let response = MarginStatus {
        wallet: wallet.to_string(),
        total_margin_used,
        total_margin_available,
        margin_usage_percent,
        margin_level,
        status,
        positions: margin_positions,
        warnings,
    };
    
    responses::ok(response).into_response()
}

/// Shock simulation request
#[derive(Debug, Deserialize)]
pub struct ShockSimulationRequest {
    pub wallet: String,
    pub scenarios: Vec<ShockScenario>,
}

#[derive(Debug, Deserialize)]
pub struct ShockScenario {
    pub name: String,
    pub market_shocks: HashMap<u64, f64>, // market_id -> price change %
    pub global_shock: Option<f64>,         // Apply to all markets
}

/// Shock simulation response
#[derive(Debug, Serialize)]
pub struct ShockSimulationResponse {
    pub wallet: String,
    pub current_portfolio_value: u64,
    pub current_margin_level: f64,
    pub scenarios: Vec<ScenarioResult>,
}

#[derive(Debug, Serialize)]
pub struct ScenarioResult {
    pub name: String,
    pub portfolio_value: u64,
    pub portfolio_change: f64,
    pub margin_level: f64,
    pub liquidated_positions: Vec<String>,
    pub total_loss: u64,
    pub survives: bool,
}

/// Simulate market shocks
pub async fn simulate_shock(
    State(state): State<AppState>,
    Json(payload): Json<ShockSimulationRequest>,
) -> Response {
    
    // Get current positions
    let positions = state.risk_engine.get_positions(&payload.wallet).await;
    let current_value = calculate_portfolio_value(&positions, &state).await;
    let current_margin_level = calculate_margin_level(&positions);
    
    let mut scenario_results = Vec::new();
    
    for scenario in payload.scenarios {
        let result = simulate_scenario(&positions, &scenario, &state).await;
        scenario_results.push(result);
    }
    
    let response = ShockSimulationResponse {
        wallet: payload.wallet,
        current_portfolio_value: current_value,
        current_margin_level,
        scenarios: scenario_results,
    };
    
    info!("Shock simulation completed: {:?}", response);
    responses::ok(response).into_response()
}

/// Auto-deleverage request
#[derive(Debug, Deserialize)]
pub struct AutoDeleverageRequest {
    pub wallet: String,
    pub target_leverage: Option<u8>,
    pub preserve_positions: Option<Vec<String>>, // Position IDs to keep
}

/// Auto-deleverage response
#[derive(Debug, Serialize)]
pub struct AutoDeleverageResponse {
    pub success: bool,
    pub positions_closed: Vec<PositionClosed>,
    pub positions_reduced: Vec<PositionReduced>,
    pub new_leverage: f64,
    pub margin_freed: u64,
}

#[derive(Debug, Serialize)]
pub struct PositionClosed {
    pub position_id: String,
    pub market_id: u64,
    pub size: u64,
    pub realized_pnl: f64,
}

#[derive(Debug, Serialize)]
pub struct PositionReduced {
    pub position_id: String,
    pub market_id: u64,
    pub old_size: u64,
    pub new_size: u64,
    pub margin_freed: u64,
}

/// Auto-deleverage positions
pub async fn auto_deleverage(
    State(state): State<AppState>,
    Json(payload): Json<AutoDeleverageRequest>,
) -> Response {
    
    let target_leverage = payload.target_leverage.unwrap_or(5);
    let preserve_ids = payload.preserve_positions.unwrap_or_default();
    
    // Get current positions
    let positions = state.risk_engine.get_positions(&payload.wallet).await;
    let mut positions_closed = Vec::new();
    let mut positions_reduced = Vec::new();
    let mut margin_freed = 0;
    
    // Sort positions by P&L (close losers first)
    let mut sorted_positions = positions.clone();
    sorted_positions.sort_by(|a, b| {
        let a_pnl = calculate_position_pnl(a, &state);
        let b_pnl = calculate_position_pnl(b, &state);
        a_pnl.partial_cmp(&b_pnl).unwrap()
    });
    
    // Close or reduce positions until target leverage reached
    for pos in sorted_positions {
        if preserve_ids.contains(&pos.id) || pos.is_closed {
            continue;
        }
        
        let current_leverage = calculate_overall_leverage(&positions);
        if current_leverage <= target_leverage as f64 {
            break;
        }
        
        // Decide whether to close or reduce
        if pos.leverage > target_leverage {
            // Reduce position
            let new_size = pos.size * target_leverage as u64 / pos.leverage as u64;
            let freed = pos.margin_used - (pos.margin_used * target_leverage as u64 / pos.leverage as u64);
            
            positions_reduced.push(PositionReduced {
                position_id: pos.id.clone(),
                market_id: pos._market_id,
                old_size: pos.size,
                new_size,
                margin_freed: freed,
            });
            
            margin_freed += freed;
        } else {
            // Close position
            let pnl = calculate_position_pnl(&pos, &state);
            
            positions_closed.push(PositionClosed {
                position_id: pos.id.clone(),
                market_id: pos._market_id,
                size: pos.size,
                realized_pnl: pnl,
            });
            
            margin_freed += pos.margin_used;
        }
    }
    
    let new_leverage = calculate_overall_leverage(&positions) - 
        (margin_freed as f64 / 100_000.0); // Approximate
    
    let response = AutoDeleverageResponse {
        success: true,
        positions_closed,
        positions_reduced,
        new_leverage,
        margin_freed,
    };
    
    info!("Auto-deleverage completed: {:?}", response);
    responses::ok(response).into_response()
}

/// Test liquidation request
#[derive(Debug, Deserialize)]
pub struct TestLiquidationRequest {
    pub wallet: String,
    pub position_id: Option<String>,
    pub price_scenario: HashMap<u64, f64>, // market_id -> test price
}

/// Test liquidation response
#[derive(Debug, Serialize)]
pub struct TestLiquidationResponse {
    pub wallet: String,
    pub liquidation_triggered: bool,
    pub positions_at_risk: Vec<PositionRisk>,
    pub total_collateral: u64,
    pub margin_shortfall: u64,
    pub liquidation_price: HashMap<String, f64>, // position_id -> price
}

#[derive(Debug, Serialize)]
pub struct PositionRisk {
    pub position_id: String,
    pub market_id: u64,
    pub current_margin_ratio: f64,
    pub margin_call: bool,
    pub will_liquidate: bool,
    pub liquidation_price: f64,
}

/// Test liquidation scenarios
pub async fn test_liquidation(
    State(state): State<AppState>,
    Json(payload): Json<TestLiquidationRequest>,
) -> Response {
    
    // Get positions
    let positions = state.risk_engine.get_positions(&payload.wallet).await;
    let limits = get_or_create_risk_limits(&payload.wallet).await;
    
    let mut positions_at_risk = Vec::new();
    let mut liquidation_prices = HashMap::new();
    let mut liquidation_triggered = false;
    let mut total_margin_required = 0;
    
    for pos in positions {
        if pos.is_closed {
            continue;
        }
        
        // Use test price if provided, otherwise current price
        let test_price = payload.price_scenario.get(&pos._market_id).copied()
            .or_else(|| {
                let market = state.seeded_markets.get_market(pos._market_id);
                None // Simplified for compilation
            })
            .unwrap_or(0.5);
        
        // Calculate margin ratio at test price
        let position_value = (pos.size as f64 * test_price) as u64;
        let margin_ratio = pos.margin_used as f64 / position_value as f64;
        
        // Calculate liquidation price
        let liq_price = pos.entry_price * limits.liquidation_level / margin_ratio;
        liquidation_prices.insert(pos.id.clone(), liq_price);
        
        let margin_call = margin_ratio < limits.margin_call_level;
        let will_liquidate = margin_ratio < limits.liquidation_level;
        
        if margin_call || will_liquidate {
            liquidation_triggered = liquidation_triggered || will_liquidate;
            total_margin_required += pos.margin_used;
            
            positions_at_risk.push(PositionRisk {
                position_id: pos.id,
                market_id: pos._market_id,
                current_margin_ratio: margin_ratio,
                margin_call,
                will_liquidate,
                liquidation_price: liq_price,
            });
        }
    }
    
    let total_collateral = 100_000; // Mock balance
    let margin_shortfall = total_margin_required.saturating_sub(total_collateral);
    
    let response = TestLiquidationResponse {
        wallet: payload.wallet,
        liquidation_triggered,
        positions_at_risk,
        total_collateral,
        margin_shortfall,
        liquidation_price: liquidation_prices,
    };
    
    info!("Liquidation test completed: {:?}", response);
    responses::ok(response).into_response()
}

/// Helper functions
async fn get_or_create_risk_limits(wallet: &str) -> RiskLimits {
    // In production, fetch from database or create defaults
    RiskLimits {
        wallet: wallet.to_string(),
        max_position_size: 100_000,
        max_leverage: 10,
        max_daily_loss: 10_000,
        max_weekly_loss: 50_000,
        max_open_positions: 20,
        max_exposure: 500_000,
        margin_call_level: 0.2,
        liquidation_level: 0.1,
        auto_deleverage_enabled: true,
        risk_rating: RiskRating::Moderate,
    }
}

async fn calculate_portfolio_value(
    positions: &[crate::risk_engine_ext::RiskPosition],
    state: &AppState
) -> u64 {
    let mut total_value = 0;
    
    for pos in positions {
        if !pos.is_closed {
            let market = state.seeded_markets.get_market(pos._market_id).await;
            let current_price = market.as_ref()
                .and_then(|m| m["outcomes"][0]["total_stake"].as_f64())
                .unwrap_or(0.5);
            
            total_value += (pos.size as f64 * current_price) as u64;
        }
    }
    
    total_value
}

fn calculate_margin_level(positions: &[crate::risk_engine_ext::RiskPosition]) -> f64 {
    let total_margin: u64 = positions.iter()
        .filter(|p| !p.is_closed)
        .map(|p| p.margin_used)
        .sum();
    
    if total_margin > 0 {
        100_000.0 / total_margin as f64 // Mock balance
    } else {
        999.0
    }
}

async fn simulate_scenario(
    positions: &[crate::risk_engine_ext::RiskPosition],
    scenario: &ShockScenario,
    state: &AppState
) -> ScenarioResult {
    let mut portfolio_value = 0;
    let mut liquidated_positions = Vec::new();
    let mut total_loss = 0;
    
    for pos in positions {
        if pos.is_closed {
            continue;
        }
        
        // Get base price
        let market = state.seeded_markets.get_market(pos._market_id).await;
        let base_price = market.as_ref()
            .and_then(|m| m["outcomes"][0]["total_stake"].as_f64())
            .unwrap_or(0.5);
        
        // Apply shock
        let shock_pct = scenario.market_shocks.get(&pos._market_id)
            .or(scenario.global_shock.as_ref())
            .copied()
            .unwrap_or(0.0);
        
        let shocked_price = base_price * (1.0 + shock_pct / 100.0);
        let position_value = (pos.size as f64 * shocked_price) as u64;
        let margin_ratio = pos.margin_used as f64 / position_value as f64;
        
        if margin_ratio < 0.1 {
            liquidated_positions.push(pos.id.clone());
            total_loss += pos.margin_used;
        } else {
            portfolio_value += position_value;
        }
    }
    
    let portfolio_change = if portfolio_value > 0 {
        (portfolio_value as f64 - 100_000.0) / 100_000.0 * 100.0
    } else {
        -100.0
    };
    
    ScenarioResult {
        name: scenario.name.clone(),
        portfolio_value,
        portfolio_change,
        margin_level: calculate_margin_level(positions),
        survives: liquidated_positions.is_empty(),
        liquidated_positions,
        total_loss,
    }
}

fn calculate_position_pnl(
    position: &crate::risk_engine_ext::RiskPosition,
    state: &AppState
) -> f64 {
    // Simplified - in production would fetch current price
    let current_price = 0.5;
    (current_price - position.entry_price) * position.size as f64
}

fn calculate_overall_leverage(positions: &[crate::risk_engine_ext::RiskPosition]) -> f64 {
    let total_size: u64 = positions.iter()
        .filter(|p| !p.is_closed)
        .map(|p| p.size)
        .sum();
    
    let total_margin: u64 = positions.iter()
        .filter(|p| !p.is_closed)
        .map(|p| p.margin_used)
        .sum();
    
    if total_margin > 0 {
        total_size as f64 / total_margin as f64
    } else {
        0.0
    }
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

// Extension for RiskEngine
impl crate::risk_engine::RiskEngine {
    pub async fn update_risk_limits(&self, limits: &RiskLimits) {
        // In production, store in database
        tracing::info!("Risk limits updated for wallet {}: {:?}", limits.wallet, limits);
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_margin_calculation() {
        let positions = vec![
            crate::risk_engine_ext::RiskPosition {
                id: "pos1".to_string(),
                _market_id: 1000,
                wallet: "test".to_string(),
                outcome: 0,
                size: 1000,
                entry_price: 0.5,
                leverage: 5,
                margin_used: 200,
                opened_at: Utc::now(),
                stop_loss: None,
                take_profit: None,
                is_closed: false,
                realized_pnl: None,
            }
        ];
        
        let margin_level = calculate_margin_level(&positions);
        assert!((margin_level - 500.0).abs() < 0.01); // 100000 / 200
    }
}