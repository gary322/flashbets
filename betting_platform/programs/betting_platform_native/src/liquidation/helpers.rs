//! Liquidation helper functions
//!
//! Utility functions for liquidation calculations

use solana_program::program_error::ProgramError;

use crate::{
    error::BettingPlatformError,
    math::U64F64,
    state::{Position, ProposalPDA},
    constants::*,
};

/// Calculate liquidation amount based on position size and coverage
pub fn calculate_liquidation_amount(
    position_size: u64,
    coverage: U64F64,
) -> Result<u64, ProgramError> {
    // Base liquidation: 8% of position size per spec
    let base_amount = position_size / 12; // ~8.33%
    
    // Adjust based on coverage (lower coverage = more aggressive liquidation)
    let coverage_factor = if coverage < U64F64::from_num(1) {
        U64F64::from_num(2) - coverage // 2x at 0 coverage, 1x at 1 coverage
    } else {
        U64F64::from_num(1)
    };
    
    let adjusted_amount = U64F64::from_num(base_amount) * coverage_factor;
    
    // Cap at 20% of position
    let max_amount = position_size / 5;
    let final_amount = adjusted_amount.to_num().min(max_amount);
    
    Ok(final_amount)
}

/// Calculate keeper reward for liquidation
pub fn calculate_keeper_reward(
    liquidation_amount: u64,
    base_reward_bps: u16,
) -> Result<u64, ProgramError> {
    // Base reward is percentage of liquidation amount
    let base_reward = (liquidation_amount as u128 * base_reward_bps as u128 / 10_000) as u64;
    
    // Minimum reward to ensure keeper incentive
    let min_reward = 1_000_000; // $1 with 6 decimals
    
    // Maximum reward to prevent excessive payouts
    let max_reward = 100_000_000; // $100 with 6 decimals
    
    let reward = base_reward.max(min_reward).min(max_reward);
    
    Ok(reward)
}

/// Check if position should be liquidated based on coverage
pub fn should_liquidate_coverage_based(
    position: &Position,
    current_price: u64,
    coverage: U64F64,
) -> Result<bool, ProgramError> {
    // First check simple price-based liquidation
    if position.should_liquidate(current_price) {
        return Ok(true);
    }
    
    // Then check coverage-based liquidation with margin ratio
    let notional = (position.size as u128 * current_price as u128 / PRICE_PRECISION as u128) as u64;
    let margin_ratio = U64F64::from_num(position.margin) / U64F64::from_num(notional);
    
    // Get effective leverage considering PnL
    let effective_leverage = position.get_effective_leverage()?;
    let effective_leverage_fp = U64F64::from_num(effective_leverage);
    
    // Adjusted liquidation threshold based on effective leverage
    let threshold = U64F64::from_num(1) / (coverage * effective_leverage_fp);
    
    // Add small buffer to prevent edge case liquidations
    let buffer = U64F64::from_num(1u64) / U64F64::from_num(1000u64); // 0.1% buffer
    let adjusted_threshold = threshold + buffer;
    
    Ok(margin_ratio < adjusted_threshold)
}

/// Calculate liquidation penalty
pub fn calculate_liquidation_penalty(
    liquidation_amount: u64,
    penalty_bps: u16,
) -> Result<u64, ProgramError> {
    if penalty_bps > MAX_LIQUIDATION_PENALTY_BPS {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    let penalty = (liquidation_amount as u128 * penalty_bps as u128 / 10_000) as u64;
    
    Ok(penalty)
}

/// Calculate maximum liquidatable amount per slot
pub fn calculate_max_liquidation_per_slot(
    total_oi: u64,
    max_liquidation_percentage: u16,
) -> Result<u64, ProgramError> {
    if max_liquidation_percentage > 10_000 {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    let max_amount = (total_oi as u128 * max_liquidation_percentage as u128 / 10_000) as u64;
    
    Ok(max_amount)
}

/// Calculate priority score for liquidation queue
pub fn calculate_liquidation_priority(
    margin_ratio: U64F64,
    position_size: u64,
    time_since_warning: i64,
) -> u64 {
    // Lower margin ratio = higher priority
    let margin_score = if margin_ratio < U64F64::from_num(1) {
        let inverse = U64F64::from_num(1) / margin_ratio;
        inverse.to_num().min(1000)
    } else {
        0
    };
    
    // Larger positions get slightly higher priority
    let size_score = (position_size / 1_000_000).min(100); // Cap at 100
    
    // Longer time since warning = higher priority
    let time_score = (time_since_warning as u64 / 60).min(100); // Minutes, cap at 100
    
    // Weighted score: margin is most important
    margin_score * 100 + size_score * 10 + time_score
}

/// Validate liquidation parameters
pub fn validate_liquidation_params(
    position: &Position,
    liquidation_amount: u64,
    current_price: u64,
) -> Result<(), ProgramError> {
    // Amount must be positive and not exceed position size
    if liquidation_amount == 0 || liquidation_amount > position.size {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Position must be open
    if position.is_closed {
        return Err(BettingPlatformError::PositionClosed.into());
    }
    
    // Price must be reasonable
    if current_price == 0 || current_price > MAX_PRICE {
        return Err(BettingPlatformError::InvalidPrice.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_liquidation_amount() {
        let position_size = 1_000_000_000; // $1000
        let coverage = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
        
        let amount = calculate_liquidation_amount(position_size, coverage).unwrap();
        
        // Should be ~16.67% (8.33% * 2)
        assert!(amount > 150_000_000 && amount < 200_000_000);
    }

    #[test]
    fn test_calculate_keeper_reward() {
        let liquidation_amount = 100_000_000; // $100
        let base_reward_bps = 50; // 0.5%
        
        let reward = calculate_keeper_reward(liquidation_amount, base_reward_bps).unwrap();
        
        // Should be at least minimum reward
        assert!(reward >= 1_000_000);
    }

    #[test]
    fn test_liquidation_priority() {
        let margin_ratio = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
        let position_size = 10_000_000_000; // $10k
        let time_since_warning = 300; // 5 minutes
        
        let priority = calculate_liquidation_priority(margin_ratio, position_size, time_since_warning);
        
        // Should have high priority due to low margin ratio
        assert!(priority > 100);
    }
}