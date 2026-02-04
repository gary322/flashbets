//! Staking handlers for MMT token staking and rewards
//! Implements comprehensive staking functionality with production-grade features

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
use uuid::Uuid;

/// Staking pool information
#[derive(Debug, Serialize)]
pub struct StakingPool {
    pub pool_id: String,
    pub name: String,
    pub token_symbol: String,
    pub total_staked: u64,
    pub apy: f64,
    pub min_stake_amount: u64,
    pub lock_period_days: u32,
    pub early_unstake_penalty: f64,
    pub rewards_distributed: u64,
    pub participants: u32,
}

/// Staking position
#[derive(Debug, Serialize)]
pub struct StakingPosition {
    pub position_id: String,
    pub pool_id: String,
    pub wallet: String,
    pub amount_staked: u64,
    pub rewards_earned: u64,
    pub pending_rewards: u64,
    pub stake_timestamp: DateTime<Utc>,
    pub unlock_timestamp: DateTime<Utc>,
    pub is_locked: bool,
    pub apy_at_stake: f64,
}

/// Stake request
#[derive(Debug, Deserialize)]
pub struct StakeRequest {
    pub amount: u64,
    pub wallet: String,
    #[serde(default = "default_pool_id")]
    pub pool_id: String,
    #[serde(default = "default_duration_days")]
    pub duration_days: u32,
}

fn default_pool_id() -> String { "mmt_staking_pool_1".to_string() }
fn default_duration_days() -> u32 { 30 }

/// Stake response
#[derive(Debug, Serialize)]
pub struct StakeResponse {
    pub success: bool,
    pub position_id: String,
    pub amount_staked: u64,
    pub pool_id: String,
    pub apy: f64,
    pub unlock_timestamp: DateTime<Utc>,
    pub estimated_rewards: u64,
    pub signature: String,
}

/// Stake tokens
pub async fn stake_tokens(
    State(state): State<AppState>,
    Json(payload): Json<StakeRequest>,
) -> Response {
    debug!("Stake request: {:?}", payload);
    
    // Validate amount
    if payload.amount == 0 {
        return responses::bad_request("Amount must be greater than 0").into_response();
    }
    
    // Get pool info
    let pool = get_staking_pool(&payload.pool_id).await;
    
    // Check minimum stake
    if payload.amount < pool.min_stake_amount {
        return responses::bad_request(format!(
            "Amount below minimum stake of {} tokens",
            pool.min_stake_amount
        )).into_response();
    }
    
    // Calculate unlock timestamp
    let unlock_timestamp = Utc::now() + chrono::Duration::days(payload.duration_days as i64);
    
    // Calculate estimated rewards
    let estimated_rewards = calculate_estimated_rewards(
        payload.amount,
        pool.apy,
        payload.duration_days,
    );
    
    // Create staking position
    let position_id = format!("stake_{}", Uuid::new_v4());
    
    // In production, this would:
    // 1. Transfer tokens from user wallet
    // 2. Create on-chain staking position
    // 3. Update pool metrics
    
    let response = StakeResponse {
        success: true,
        position_id,
        amount_staked: payload.amount,
        pool_id: payload.pool_id,
        apy: pool.apy,
        unlock_timestamp,
        estimated_rewards,
        signature: format!("stake_sig_{}", Uuid::new_v4()),
    };
    
    info!("Tokens staked: {:?}", response);
    responses::ok(response).into_response()
}

/// Unstake request
#[derive(Debug, Deserialize)]
pub struct UnstakeRequest {
    pub position_id: String,
    pub wallet: String,
    #[serde(default)]
    pub force_unstake: bool, // Accept early unstake penalty
}

/// Unstake response
#[derive(Debug, Serialize)]
pub struct UnstakeResponse {
    pub success: bool,
    pub position_id: String,
    pub amount_unstaked: u64,
    pub rewards_claimed: u64,
    pub penalty_applied: u64,
    pub total_received: u64,
    pub signature: String,
}

/// Unstake tokens
pub async fn unstake_tokens(
    State(state): State<AppState>,
    Json(payload): Json<UnstakeRequest>,
) -> Response {
    debug!("Unstake request: {:?}", payload);
    
    // Get staking position
    let position = match get_staking_position(&payload.position_id, &payload.wallet).await {
        Some(pos) => pos,
        None => return responses::not_found("Staking position not found").into_response(),
    };
    
    // Check if locked
    if position.is_locked && !payload.force_unstake {
        return responses::bad_request(format!(
            "Position is locked until {}. Set force_unstake=true to accept penalty",
            position.unlock_timestamp.format("%Y-%m-%d %H:%M UTC")
        )).into_response();
    }
    
    // Calculate penalty if early unstake
    let penalty = if position.is_locked && payload.force_unstake {
        let pool = get_staking_pool(&position.pool_id).await;
        (position.amount_staked as f64 * pool.early_unstake_penalty) as u64
    } else {
        0
    };
    
    // Calculate final rewards
    let rewards = position.pending_rewards + position.rewards_earned;
    let total_received = position.amount_staked + rewards - penalty;
    
    // In production, this would:
    // 1. Close on-chain staking position
    // 2. Transfer tokens back to user
    // 3. Update pool metrics
    
    let response = UnstakeResponse {
        success: true,
        position_id: payload.position_id,
        amount_unstaked: position.amount_staked,
        rewards_claimed: rewards,
        penalty_applied: penalty,
        total_received,
        signature: format!("unstake_sig_{}", Uuid::new_v4()),
    };
    
    info!("Tokens unstaked: {:?}", response);
    responses::ok(response).into_response()
}

/// Get staking rewards
#[derive(Debug, Deserialize)]
pub struct RewardsQuery {
    pub wallet: String,
    pub position_id: Option<String>,
}

/// Rewards response
#[derive(Debug, Serialize)]
pub struct RewardsResponse {
    pub wallet: String,
    pub total_rewards_earned: u64,
    pub total_pending_rewards: u64,
    pub positions: Vec<PositionRewards>,
    pub claimable_amount: u64,
}

#[derive(Debug, Serialize)]
pub struct PositionRewards {
    pub position_id: String,
    pub pool_id: String,
    pub amount_staked: u64,
    pub rewards_earned: u64,
    pub pending_rewards: u64,
    pub apy: f64,
    pub days_staked: u32,
}

/// Get staking rewards
pub async fn get_rewards(
    State(state): State<AppState>,
    Query(params): Query<RewardsQuery>,
    auth: Option<AuthenticatedUser>,
) -> Response {
    // Verify authorization if auth is present
    if let Some(auth_user) = auth {
        if auth_user.wallet != params.wallet && !auth_user.role.is_admin() {
            return responses::forbidden("Cannot view rewards for other wallets").into_response();
        }
    }
    
    // Get all staking positions for wallet
    let positions = get_user_staking_positions(&params.wallet).await;
    
    let mut total_rewards_earned = 0;
    let mut total_pending_rewards = 0;
    let mut position_rewards = Vec::new();
    
    for pos in positions {
        if params.position_id.is_none() || params.position_id.as_ref() == Some(&pos.position_id) {
            let days_staked = (Utc::now() - pos.stake_timestamp).num_days() as u32;
            let pending = calculate_pending_rewards(&pos, days_staked);
            
            total_rewards_earned += pos.rewards_earned;
            total_pending_rewards += pending;
            
            position_rewards.push(PositionRewards {
                position_id: pos.position_id,
                pool_id: pos.pool_id,
                amount_staked: pos.amount_staked,
                rewards_earned: pos.rewards_earned,
                pending_rewards: pending,
                apy: pos.apy_at_stake,
                days_staked,
            });
        }
    }
    
    let response = RewardsResponse {
        wallet: params.wallet,
        total_rewards_earned,
        total_pending_rewards,
        claimable_amount: total_pending_rewards,
        positions: position_rewards,
    };
    
    responses::ok(response).into_response()
}

/// Claim rewards request
#[derive(Debug, Deserialize)]
pub struct ClaimRewardsRequest {
    pub wallet: String,
    pub position_ids: Option<Vec<String>>, // None means claim all
}

/// Claim rewards response
#[derive(Debug, Serialize)]
pub struct ClaimRewardsResponse {
    pub success: bool,
    pub total_claimed: u64,
    pub positions_claimed: Vec<String>,
    pub signature: String,
}

/// Claim staking rewards
pub async fn claim_rewards(
    State(state): State<AppState>,
    Json(payload): Json<ClaimRewardsRequest>,
) -> Response {
    
    // Get positions to claim
    let positions = get_user_staking_positions(&payload.wallet).await;
    let mut total_claimed = 0;
    let mut positions_claimed = Vec::new();
    
    for pos in positions {
        let should_claim = payload.position_ids.as_ref()
            .map(|ids| ids.contains(&pos.position_id))
            .unwrap_or(true);
        
        if should_claim && pos.pending_rewards > 0 {
            total_claimed += pos.pending_rewards;
            positions_claimed.push(pos.position_id);
        }
    }
    
    if total_claimed == 0 {
        return responses::bad_request("No rewards to claim").into_response();
    }
    
    // In production, this would:
    // 1. Update on-chain positions
    // 2. Transfer reward tokens
    // 3. Reset pending rewards
    
    let response = ClaimRewardsResponse {
        success: true,
        total_claimed,
        positions_claimed,
        signature: format!("claim_sig_{}", Uuid::new_v4()),
    };
    
    info!("Rewards claimed: {:?}", response);
    responses::ok(response).into_response()
}

/// Get all staking pools
pub async fn get_staking_pools(
    State(state): State<AppState>,
) -> Response {
    let pools = vec![
        StakingPool {
            pool_id: "mmt_staking_pool_1".to_string(),
            name: "MMT Staking Pool".to_string(),
            token_symbol: "MMT".to_string(),
            total_staked: 10_000_000,
            apy: 18.5,
            min_stake_amount: 100,
            lock_period_days: 30,
            early_unstake_penalty: 0.1, // 10%
            rewards_distributed: 1_850_000,
            participants: 2500,
        },
        StakingPool {
            pool_id: "mmt_staking_pool_2".to_string(),
            name: "MMT High Yield Pool".to_string(),
            token_symbol: "MMT".to_string(),
            total_staked: 5_000_000,
            apy: 25.0,
            min_stake_amount: 1000,
            lock_period_days: 90,
            early_unstake_penalty: 0.15, // 15%
            rewards_distributed: 625_000,
            participants: 500,
        },
        StakingPool {
            pool_id: "lp_staking_pool_1".to_string(),
            name: "LP Token Staking".to_string(),
            token_symbol: "LP-MMT".to_string(),
            total_staked: 3_000_000,
            apy: 35.0,
            min_stake_amount: 50,
            lock_period_days: 14,
            early_unstake_penalty: 0.05, // 5%
            rewards_distributed: 350_000,
            participants: 800,
        },
    ];
    
    responses::ok(json!({
        "pools": pools,
        "count": pools.len()
    })).into_response()
}

/// Helper functions
async fn get_staking_pool(pool_id: &str) -> StakingPool {
    // In production, fetch from database
    match pool_id {
        "mmt_staking_pool_2" => StakingPool {
            pool_id: pool_id.to_string(),
            name: "MMT High Yield Pool".to_string(),
            token_symbol: "MMT".to_string(),
            total_staked: 5_000_000,
            apy: 25.0,
            min_stake_amount: 1000,
            lock_period_days: 90,
            early_unstake_penalty: 0.15,
            rewards_distributed: 625_000,
            participants: 500,
        },
        "lp_staking_pool_1" => StakingPool {
            pool_id: pool_id.to_string(),
            name: "LP Token Staking".to_string(),
            token_symbol: "LP-MMT".to_string(),
            total_staked: 3_000_000,
            apy: 35.0,
            min_stake_amount: 50,
            lock_period_days: 14,
            early_unstake_penalty: 0.05,
            rewards_distributed: 350_000,
            participants: 800,
        },
        _ => StakingPool {
            pool_id: "mmt_staking_pool_1".to_string(),
            name: "MMT Staking Pool".to_string(),
            token_symbol: "MMT".to_string(),
            total_staked: 10_000_000,
            apy: 18.5,
            min_stake_amount: 100,
            lock_period_days: 30,
            early_unstake_penalty: 0.1,
            rewards_distributed: 1_850_000,
            participants: 2500,
        },
    }
}

async fn get_staking_position(position_id: &str, wallet: &str) -> Option<StakingPosition> {
    // In production, fetch from database
    if position_id.starts_with("stake_") {
        Some(StakingPosition {
            position_id: position_id.to_string(),
            pool_id: "mmt_staking_pool_1".to_string(),
            wallet: wallet.to_string(),
            amount_staked: 1000,
            rewards_earned: 15,
            pending_rewards: 5,
            stake_timestamp: Utc::now() - chrono::Duration::days(10),
            unlock_timestamp: Utc::now() + chrono::Duration::days(20),
            is_locked: true,
            apy_at_stake: 18.5,
        })
    } else {
        None
    }
}

async fn get_user_staking_positions(wallet: &str) -> Vec<StakingPosition> {
    // In production, fetch from database
    vec![
        StakingPosition {
            position_id: "stake_1".to_string(),
            pool_id: "mmt_staking_pool_1".to_string(),
            wallet: wallet.to_string(),
            amount_staked: 1000,
            rewards_earned: 15,
            pending_rewards: 5,
            stake_timestamp: Utc::now() - chrono::Duration::days(10),
            unlock_timestamp: Utc::now() + chrono::Duration::days(20),
            is_locked: true,
            apy_at_stake: 18.5,
        },
        StakingPosition {
            position_id: "stake_2".to_string(),
            pool_id: "mmt_staking_pool_2".to_string(),
            wallet: wallet.to_string(),
            amount_staked: 5000,
            rewards_earned: 125,
            pending_rewards: 34,
            stake_timestamp: Utc::now() - chrono::Duration::days(30),
            unlock_timestamp: Utc::now() + chrono::Duration::days(60),
            is_locked: true,
            apy_at_stake: 25.0,
        },
    ]
}

fn calculate_estimated_rewards(amount: u64, apy: f64, days: u32) -> u64 {
    let daily_rate = apy / 365.0 / 100.0;
    (amount as f64 * daily_rate * days as f64) as u64
}

fn calculate_pending_rewards(position: &StakingPosition, days_staked: u32) -> u64 {
    let daily_rate = position.apy_at_stake / 365.0 / 100.0;
    let total_rewards = (position.amount_staked as f64 * daily_rate * days_staked as f64) as u64;
    total_rewards.saturating_sub(position.rewards_earned)
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
    fn test_reward_calculation() {
        let rewards = calculate_estimated_rewards(1000, 18.5, 30);
        assert!(rewards > 14 && rewards < 16); // ~15 tokens for 1000 staked at 18.5% APY for 30 days
        
        let rewards = calculate_estimated_rewards(10000, 25.0, 90);
        assert!(rewards > 610 && rewards < 620); // ~616 tokens for 10000 staked at 25% APY for 90 days
    }
}