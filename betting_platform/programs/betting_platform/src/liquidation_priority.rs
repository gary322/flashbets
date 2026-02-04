use anchor_lang::prelude::*;
use crate::fixed_types::U64F64;
use std::cmp::Ordering;
use crate::errors::*;

#[account]
pub struct LiquidationQueue {
    /// Queue identifier
    pub queue_id: [u8; 32],
    /// Priority-ordered positions at risk
    pub at_risk_positions: Vec<AtRiskPosition>,
    /// Active liquidations in progress
    pub active_liquidations: Vec<ActiveLiquidation>,
    /// Liquidation configuration
    pub config: LiquidationConfig,
    /// Performance metrics
    pub metrics: LiquidationMetrics,
    /// Keeper rewards pool
    pub keeper_rewards_pool: u64,
    /// Last update slot
    pub last_update_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AtRiskPosition {
    /// Position identifier
    pub position_id: [u8; 32],
    /// Owner of position
    pub owner: Pubkey,
    /// Market/verse ID
    pub market_id: [u8; 32],
    /// Position size
    pub size: u64,
    /// Entry price
    pub entry_price: U64F64,
    /// Current mark price
    pub mark_price: U64F64,
    /// Effective leverage (including chains)
    pub effective_leverage: U64F64,
    /// Distance to liquidation price
    pub distance_to_liquidation: U64F64,
    /// Risk score (0-100, higher = more urgent)
    pub risk_score: u8,
    /// MMT staking tier for priority
    pub staking_tier: StakingTier,
    /// Bootstrap trader priority
    pub bootstrap_priority: u8,
    /// Time at risk (slots)
    pub time_at_risk: u64,
    /// Is this a chained position
    pub is_chained: bool,
    /// Chain depth if chained
    pub chain_depth: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ActiveLiquidation {
    /// Liquidation ID
    pub liquidation_id: [u8; 32],
    /// Position being liquidated
    pub position_id: [u8; 32],
    /// Keeper performing liquidation
    pub keeper: Pubkey,
    /// Start slot
    pub start_slot: u64,
    /// Amount liquidated so far
    pub amount_liquidated: u64,
    /// Target liquidation amount
    pub target_amount: u64,
    /// Liquidation price
    pub liquidation_price: U64F64,
    /// Status
    pub status: LiquidationStatus,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum LiquidationStatus {
    /// In progress
    InProgress,
    /// Completed successfully
    Completed,
    /// Partially completed
    Partial,
    /// Failed/reverted
    Failed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum StakingTier {
    /// No MMT staked
    None,
    /// Bronze tier (100-1k MMT)
    Bronze,
    /// Silver tier (1k-10k MMT)
    Silver,
    /// Gold tier (10k-100k MMT)
    Gold,
    /// Platinum tier (100k+ MMT)
    Platinum,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct LiquidationConfig {
    /// Minimum liquidation size
    pub min_liquidation_size: u64,
    /// Maximum liquidation per slot (8% from CLAUDE.md)
    pub max_liquidation_per_slot: U64F64,
    /// Liquidation penalty (to keeper)
    pub liquidation_penalty_bps: u16, // 5bp from CLAUDE.md
    /// Grace period before liquidation (slots)
    pub grace_period: u64,
    /// Priority boost per staking tier
    pub staking_tier_boost: [u8; 5],
    /// Bootstrap trader protection multiplier
    pub bootstrap_protection_multiplier: U64F64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct LiquidationMetrics {
    /// Total liquidations processed
    pub total_liquidations: u64,
    /// Total volume liquidated
    pub total_volume_liquidated: u64,
    /// Average liquidation size
    pub avg_liquidation_size: u64,
    /// Keeper rewards distributed
    pub keeper_rewards_distributed: u64,
    /// Failed liquidation attempts
    pub failed_attempts: u64,
}

impl AtRiskPosition {
    /// Calculate priority score for liquidation ordering
    pub fn calculate_priority_score(&self) -> u64 {
        // Base score from risk (0-100)
        let mut score = self.risk_score as u64 * 1_000_000;

        // Adjust for distance to liquidation (closer = higher priority)
        let distance_factor = if self.distance_to_liquidation < U64F64::from_num(0.01) {
            1_000_000 // <1% from liquidation
        } else if self.distance_to_liquidation < U64F64::from_num(0.05) {
            500_000 // <5% from liquidation
        } else {
            100_000 // >5% from liquidation
        };
        score += distance_factor;

        // Subtract staking tier protection (higher tier = lower priority)
        let staking_protection = match self.staking_tier {
            StakingTier::None => 0,
            StakingTier::Bronze => 100_000,
            StakingTier::Silver => 200_000,
            StakingTier::Gold => 300_000,
            StakingTier::Platinum => 500_000,
        };
        score = score.saturating_sub(staking_protection);

        // Subtract bootstrap protection
        if self.bootstrap_priority > 0 {
            score = score.saturating_sub(self.bootstrap_priority as u64 * 50_000);
        }

        // Add time factor (longer at risk = higher priority)
        score += self.time_at_risk.min(1000);

        // Add chain risk factor
        if self.is_chained {
            score += self.chain_depth as u64 * 100_000;
        }

        score
    }
}

impl Ord for AtRiskPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher score = higher priority
        self.calculate_priority_score().cmp(&other.calculate_priority_score())
    }
}

impl PartialOrd for AtRiskPosition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for AtRiskPosition {}

impl PartialEq for AtRiskPosition {
    fn eq(&self, other: &Self) -> bool {
        self.position_id == other.position_id
    }
}

pub struct LiquidationEngine;

impl LiquidationEngine {
    /// Process liquidation queue and return positions to liquidate
    pub fn process_queue(
        queue: &mut LiquidationQueue,
        max_liquidations: u64,
        current_slot: u64,
    ) -> Result<Vec<LiquidationOrder>> {
        let mut orders = Vec::new();
        let mut total_liquidated = 0u64;

        // Sort positions by priority
        queue.at_risk_positions.sort_by(|a, b| b.cmp(a));

        // Process highest priority positions
        for position in &queue.at_risk_positions {
            if orders.len() >= max_liquidations as usize {
                break;
            }

            // Check if position is actually liquidatable
            if position.distance_to_liquidation > U64F64::zero() {
                continue;
            }

            // Calculate liquidation amount (partial liquidation)
            let max_per_position = (U64F64::from_num(position.size) * 
                                   queue.config.max_liquidation_per_slot).to_num::<u64>();
            let liquidation_amount = position.size.min(max_per_position);

            // Check minimum size
            if liquidation_amount < queue.config.min_liquidation_size {
                continue;
            }

            total_liquidated += liquidation_amount;

            orders.push(LiquidationOrder {
                position_id: position.position_id,
                owner: position.owner,
                market_id: position.market_id,
                liquidation_amount,
                liquidation_price: position.mark_price,
                keeper_reward: (liquidation_amount as u128 * 
                               queue.config.liquidation_penalty_bps as u128 / 10_000) as u64,
                priority_score: position.calculate_priority_score(),
            });
        }

        // Update metrics
        queue.metrics.total_liquidations += orders.len() as u64;
        queue.metrics.total_volume_liquidated += total_liquidated;
        if orders.len() > 0 {
            queue.metrics.avg_liquidation_size = 
                queue.metrics.total_volume_liquidated / queue.metrics.total_liquidations;
        }

        queue.last_update_slot = current_slot;

        Ok(orders)
    }

    /// Add position to at-risk queue
    pub fn add_at_risk_position(
        queue: &mut LiquidationQueue,
        position: AtRiskPosition,
    ) -> Result<()> {
        // Check if position already in queue
        if queue.at_risk_positions.iter().any(|p| p.position_id == position.position_id) {
            // Update existing position
            if let Some(existing) = queue.at_risk_positions.iter_mut()
                .find(|p| p.position_id == position.position_id) {
                *existing = position;
            }
        } else {
            // Add new position
            queue.at_risk_positions.push(position);
        }

        // Keep queue size manageable (top 1000 positions)
        if queue.at_risk_positions.len() > 1000 {
            queue.at_risk_positions.sort_by(|a, b| b.cmp(a));
            queue.at_risk_positions.truncate(1000);
        }

        Ok(())
    }

    /// Remove position from queue (e.g., after closing or improving health)
    pub fn remove_position(
        queue: &mut LiquidationQueue,
        position_id: [u8; 32],
    ) -> Result<()> {
        queue.at_risk_positions.retain(|p| p.position_id != position_id);
        Ok(())
    }

    /// Calculate risk score for a position
    pub fn calculate_risk_score(
        mark_price: U64F64,
        entry_price: U64F64,
        effective_leverage: U64F64,
        is_long: bool,
    ) -> u8 {
        // Calculate unrealized PnL percentage
        let pnl_percent = if is_long {
            (mark_price - entry_price) / entry_price
        } else {
            (entry_price - mark_price) / entry_price
        };

        // Calculate margin used
        let margin_used = U64F64::one() / effective_leverage;

        // Calculate how close to liquidation
        let margin_remaining = margin_used + pnl_percent;

        if margin_remaining <= U64F64::zero() {
            100 // Already liquidatable
        } else if margin_remaining < U64F64::from_num(0.05) {
            90 // <5% margin remaining
        } else if margin_remaining < U64F64::from_num(0.1) {
            75 // <10% margin remaining
        } else if margin_remaining < U64F64::from_num(0.2) {
            50 // <20% margin remaining
        } else if margin_remaining < U64F64::from_num(0.3) {
            25 // <30% margin remaining
        } else {
            10 // >30% margin remaining
        }
    }

    /// Process keeper liquidation
    pub fn process_keeper_liquidation(
        queue: &mut LiquidationQueue,
        liquidation_order: &LiquidationOrder,
        keeper: Pubkey,
        current_slot: u64,
    ) -> Result<()> {
        // Create active liquidation record
        let active_liq = ActiveLiquidation {
            liquidation_id: Pubkey::new_unique().to_bytes(),
            position_id: liquidation_order.position_id,
            keeper,
            start_slot: current_slot,
            amount_liquidated: 0,
            target_amount: liquidation_order.liquidation_amount,
            liquidation_price: liquidation_order.liquidation_price,
            status: LiquidationStatus::InProgress,
        };

        queue.active_liquidations.push(active_liq);

        // Update keeper rewards
        queue.keeper_rewards_pool = queue.keeper_rewards_pool
            .saturating_sub(liquidation_order.keeper_reward);
        queue.metrics.keeper_rewards_distributed += liquidation_order.keeper_reward;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LiquidationOrder {
    pub position_id: [u8; 32],
    pub owner: Pubkey,
    pub market_id: [u8; 32],
    pub liquidation_amount: u64,
    pub liquidation_price: U64F64,
    pub keeper_reward: u64,
    pub priority_score: u64,
}

/// Determine staking tier from MMT balance
pub fn get_staking_tier(mmt_staked: u64) -> StakingTier {
    match mmt_staked {
        0..=99_999_999 => StakingTier::None,
        100_000_000..=999_999_999 => StakingTier::Bronze,
        1_000_000_000..=9_999_999_999 => StakingTier::Silver,
        10_000_000_000..=99_999_999_999 => StakingTier::Gold,
        _ => StakingTier::Platinum,
    }
}