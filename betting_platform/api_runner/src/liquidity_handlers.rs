//! Liquidity management handlers for DeFi features
//! Implements comprehensive liquidity pool operations

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
    risk_engine_ext::LiquidityPosition,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Liquidity pool information
#[derive(Debug, Serialize)]
pub struct LiquidityPool {
    pub market_id: u64,
    pub market_title: String,
    pub total_liquidity: u64,
    pub volume_24h: u64,
    pub fees_24h: u64,
    pub apy: f64,
    pub your_liquidity: Option<u64>,
    pub your_share: Option<f64>,
    pub token_a_reserve: u64,
    pub token_b_reserve: u64,
    pub lp_token_supply: u64,
    pub fee_rate: f64,
}

/// Add liquidity request
#[derive(Debug, Deserialize)]
pub struct AddLiquidityRequest {
    pub market_id: u64,
    pub amount: u64,
    pub wallet: String,
    #[serde(default)]
    pub slippage_tolerance: f64, // Default 0.5%
}

/// Add liquidity response
#[derive(Debug, Serialize)]
pub struct AddLiquidityResponse {
    pub success: bool,
    pub market_id: u64,
    pub amount_deposited: u64,
    pub lp_tokens_received: u64,
    pub pool_share: f64,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}

/// Add liquidity to a pool
pub async fn add_liquidity(
    State(state): State<AppState>,
    Json(payload): Json<AddLiquidityRequest>,
) -> Response {
    debug!("Add liquidity request: {:?}", payload);
    
    // Validate amount
    if payload.amount == 0 {
        return responses::bad_request("Amount must be greater than 0").into_response();
    }
    
    // Get market
    let market = match state.seeded_markets.get_market(payload.market_id).await {
        Some(m) => m,
        None => return responses::not_found("Market not found").into_response(),
    };
    
    // Calculate LP tokens based on current pool state
    let pool_info = get_pool_info(&state, payload.market_id).await;
    let lp_tokens = calculate_lp_tokens(payload.amount, &pool_info);
    
    // Update pool state
    state.risk_engine.add_liquidity(
        payload.market_id,
        &payload.wallet,
        payload.amount,
        lp_tokens,
    ).await;
    
    // Calculate pool share
    let pool_share = (lp_tokens as f64 / (pool_info.lp_token_supply + lp_tokens) as f64) * 100.0;
    
    let response = AddLiquidityResponse {
        success: true,
        market_id: payload.market_id,
        amount_deposited: payload.amount,
        lp_tokens_received: lp_tokens,
        pool_share,
        signature: format!("add_liq_{}", Uuid::new_v4()),
        timestamp: Utc::now(),
    };
    
    info!("Liquidity added: {:?}", response);
    responses::ok(response).into_response()
}

/// Remove liquidity request
#[derive(Debug, Deserialize)]
pub struct RemoveLiquidityRequest {
    pub market_id: u64,
    pub lp_tokens: u64,
    pub wallet: String,
    #[serde(default)]
    pub min_amount: Option<u64>, // Minimum amount to receive
}

/// Remove liquidity response
#[derive(Debug, Serialize)]
pub struct RemoveLiquidityResponse {
    pub success: bool,
    pub market_id: u64,
    pub lp_tokens_burned: u64,
    pub amount_received: u64,
    pub fees_earned: u64,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}

/// Remove liquidity from a pool
pub async fn remove_liquidity(
    State(state): State<AppState>,
    Json(payload): Json<RemoveLiquidityRequest>,
) -> Response {
    debug!("Remove liquidity request: {:?}", payload);
    
    // Validate LP tokens
    if payload.lp_tokens == 0 {
        return responses::bad_request("LP tokens must be greater than 0").into_response();
    }
    
    // Get user's LP balance
    let user_lp_balance = state.risk_engine.get_lp_balance(&payload.wallet, payload.market_id).await;
    if user_lp_balance < payload.lp_tokens {
        return responses::bad_request("Insufficient LP tokens").into_response();
    }
    
    // Calculate amount to receive
    let pool_info = get_pool_info(&state, payload.market_id).await;
    let amount_out = calculate_liquidity_removal(payload.lp_tokens, &pool_info);
    
    // Check minimum amount if specified
    if let Some(min_amount) = payload.min_amount {
        if amount_out < min_amount {
            return responses::bad_request("Output amount below minimum").into_response();
        }
    }
    
    // Calculate fees earned
    let fees_earned = calculate_fees_earned(&payload.wallet, payload.market_id, &state).await;
    
    // Update pool state
    state.risk_engine.remove_liquidity(
        payload.market_id,
        &payload.wallet,
        payload.lp_tokens,
        amount_out,
    ).await;
    
    let response = RemoveLiquidityResponse {
        success: true,
        market_id: payload.market_id,
        lp_tokens_burned: payload.lp_tokens,
        amount_received: amount_out,
        fees_earned,
        signature: format!("remove_liq_{}", Uuid::new_v4()),
        timestamp: Utc::now(),
    };
    
    info!("Liquidity removed: {:?}", response);
    responses::ok(response).into_response()
}

/// Get liquidity statistics
#[derive(Debug, Deserialize)]
pub struct LiquidityStatsQuery {
    pub wallet: String,
    pub market_id: Option<u64>,
}

/// Liquidity statistics response
#[derive(Debug, Serialize)]
pub struct LiquidityStatsResponse {
    pub wallet: String,
    pub total_liquidity_provided: u64,
    pub total_fees_earned: u64,
    pub active_positions: Vec<LiquidityPositionInfo>,
    pub historical_apy: f64,
    pub current_apy: f64,
    pub impermanent_loss: f64,
}

#[derive(Debug, Serialize)]
pub struct LiquidityPositionInfo {
    pub market_id: u64,
    pub market_title: String,
    pub lp_tokens: u64,
    pub liquidity_value: u64,
    pub fees_earned: u64,
    pub pool_share: f64,
    pub position_apy: f64,
    pub entry_date: DateTime<Utc>,
}

/// Get liquidity statistics for a wallet
pub async fn get_liquidity_stats(
    State(state): State<AppState>,
    Query(params): Query<LiquidityStatsQuery>,
    auth: Option<AuthenticatedUser>,
) -> Response {
    // Verify authorization if auth is present
    if let Some(auth_user) = auth {
        if auth_user.wallet != params.wallet && !auth_user.role.is_admin() {
            return responses::forbidden("Cannot view liquidity stats for other wallets").into_response();
        }
    }
    
    // Get all liquidity positions for wallet
    let positions = state.risk_engine.get_liquidity_positions(&params.wallet).await;
    
    let mut total_liquidity = 0;
    let mut total_fees = 0;
    let mut active_positions = Vec::new();
    
    for pos in positions {
        if params.market_id.is_none() || params.market_id == Some(pos._market_id) {
            let market = state.seeded_markets.get_market(pos._market_id).await;
            let market_title = market.as_ref()
                .and_then(|m| m["title"].as_str())
                .unwrap_or("Unknown")
                .to_string();
            
            let pool_info = get_pool_info(&state, pos._market_id).await;
            let liquidity_value = (pos.lp_tokens as f64 / pool_info.lp_token_supply as f64 
                * pool_info.total_liquidity as f64) as u64;
            
            let fees_earned = calculate_fees_earned(&params.wallet, pos._market_id, &state).await;
            let pool_share = (pos.lp_tokens as f64 / pool_info.lp_token_supply as f64) * 100.0;
            let position_apy = calculate_position_apy(&pos, &pool_info);
            
            total_liquidity += liquidity_value;
            total_fees += fees_earned;
            
            active_positions.push(LiquidityPositionInfo {
                market_id: pos._market_id,
                market_title,
                lp_tokens: pos.lp_tokens,
                liquidity_value,
                fees_earned,
                pool_share,
                position_apy,
                entry_date: pos.created_at,
            });
        }
    }
    
    // Calculate overall APY
    let historical_apy = calculate_historical_apy(&params.wallet, &state).await;
    let current_apy = if total_liquidity > 0 {
        (total_fees as f64 / total_liquidity as f64) * 365.0 * 100.0
    } else {
        0.0
    };
    
    // Calculate impermanent loss
    let impermanent_loss = calculate_impermanent_loss(&active_positions, &state).await;
    
    let response = LiquidityStatsResponse {
        wallet: params.wallet,
        total_liquidity_provided: total_liquidity,
        total_fees_earned: total_fees,
        active_positions,
        historical_apy,
        current_apy,
        impermanent_loss,
    };
    
    responses::ok(response).into_response()
}

/// Get all liquidity pools
pub async fn get_all_pools(
    State(state): State<AppState>,
    auth: Option<AuthenticatedUser>,
) -> Response {
    let markets = state.seeded_markets.get_all_markets().await;
    let mut pools = Vec::new();
    
    for market in markets {
        let market_id = market["id"].as_u64().unwrap_or(0);
        let pool_info = get_pool_info(&state, market_id).await;
        
        let your_liquidity = if let Some(ref auth_user) = auth {
            let lp_balance = state.risk_engine.get_lp_balance(&auth_user.wallet, market_id).await;
            if lp_balance > 0 {
                Some((lp_balance as f64 / pool_info.lp_token_supply as f64 
                    * pool_info.total_liquidity as f64) as u64)
            } else {
                None
            }
        } else {
            None
        };
        
        let your_share = if let Some(liq) = your_liquidity {
            Some((liq as f64 / pool_info.total_liquidity as f64) * 100.0)
        } else {
            None
        };
        
        pools.push(LiquidityPool {
            market_id,
            market_title: market["title"].as_str().unwrap_or("Unknown").to_string(),
            total_liquidity: pool_info.total_liquidity,
            volume_24h: pool_info.volume_24h,
            fees_24h: pool_info.fees_24h,
            apy: pool_info.apy,
            your_liquidity,
            your_share,
            token_a_reserve: pool_info.token_a_reserve,
            token_b_reserve: pool_info.token_b_reserve,
            lp_token_supply: pool_info.lp_token_supply,
            fee_rate: pool_info.fee_rate,
        });
    }
    
    responses::ok(json!({
        "pools": pools,
        "count": pools.len()
    })).into_response()
}

/// Helper functions
#[derive(Debug)]
struct PoolInfo {
    total_liquidity: u64,
    volume_24h: u64,
    fees_24h: u64,
    apy: f64,
    token_a_reserve: u64,
    token_b_reserve: u64,
    lp_token_supply: u64,
    fee_rate: f64,
}

async fn get_pool_info(state: &AppState, market_id: u64) -> PoolInfo {
    // In production, fetch from blockchain
    // For now, return mock data
    PoolInfo {
        total_liquidity: 1_000_000 + (market_id * 100_000),
        volume_24h: 500_000 + (market_id * 50_000),
        fees_24h: 1_000 + (market_id * 100),
        apy: 15.0 + (market_id as f64 * 0.5),
        token_a_reserve: 500_000 + (market_id * 50_000),
        token_b_reserve: 500_000 + (market_id * 50_000),
        lp_token_supply: 1_000_000,
        fee_rate: 0.003, // 0.3%
    }
}

fn calculate_lp_tokens(amount: u64, pool_info: &PoolInfo) -> u64 {
    if pool_info.lp_token_supply == 0 {
        // First liquidity provider
        amount
    } else {
        // Proportional to existing liquidity
        (amount as f64 / pool_info.total_liquidity as f64 * pool_info.lp_token_supply as f64) as u64
    }
}

fn calculate_liquidity_removal(lp_tokens: u64, pool_info: &PoolInfo) -> u64 {
    (lp_tokens as f64 / pool_info.lp_token_supply as f64 * pool_info.total_liquidity as f64) as u64
}

async fn calculate_fees_earned(wallet: &str, market_id: u64, state: &AppState) -> u64 {
    // In production, calculate from fee distribution records
    // For now, return mock calculation
    let lp_balance = state.risk_engine.get_lp_balance(wallet, market_id).await;
    let pool_info = get_pool_info(state, market_id).await;
    
    if pool_info.lp_token_supply > 0 {
        (lp_balance as f64 / pool_info.lp_token_supply as f64 * pool_info.fees_24h as f64 * 30.0) as u64
    } else {
        0
    }
}

fn calculate_position_apy(position: &LiquidityPosition, pool_info: &PoolInfo) -> f64 {
    if position.initial_value > 0 {
        let current_value = (position.lp_tokens as f64 / pool_info.lp_token_supply as f64 
            * pool_info.total_liquidity as f64) as u64;
        let days_held = (Utc::now() - position.created_at).num_days().max(1) as f64;
        let return_rate = (current_value as f64 - position.initial_value as f64) / position.initial_value as f64;
        return_rate * (365.0 / days_held) * 100.0
    } else {
        pool_info.apy
    }
}

async fn calculate_historical_apy(wallet: &str, state: &AppState) -> f64 {
    // In production, calculate from historical data
    // For now, return average APY
    18.5
}

async fn calculate_impermanent_loss(positions: &[LiquidityPositionInfo], state: &AppState) -> f64 {
    // Simplified IL calculation
    // In production, would calculate based on price movements
    if positions.is_empty() {
        0.0
    } else {
        // Mock calculation: 2-5% IL
        2.0 + (rand::random::<f64>() * 3.0)
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

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lp_token_calculation() {
        let pool_info = PoolInfo {
            total_liquidity: 1_000_000,
            volume_24h: 100_000,
            fees_24h: 300,
            apy: 15.0,
            token_a_reserve: 500_000,
            token_b_reserve: 500_000,
            lp_token_supply: 1_000_000,
            fee_rate: 0.003,
        };
        
        let lp_tokens = calculate_lp_tokens(100_000, &pool_info);
        assert_eq!(lp_tokens, 100_000); // 10% of pool = 10% of LP tokens
        
        let amount_out = calculate_liquidity_removal(100_000, &pool_info);
        assert_eq!(amount_out, 100_000); // 10% of LP tokens = 10% of liquidity
    }
}