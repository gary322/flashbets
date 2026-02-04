use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::ErrorCode;
use crate::fixed_math::PRECISION;

pub fn verify_global_invariants(global: &GlobalConfigPDA) -> Result<()> {
    // Vault never negative (u64 ensures this)
    
    // Coverage calculation
    if global.total_oi > 0 {
        let expected_coverage = (global.vault as u128)
            .checked_mul(PRECISION)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(global.total_oi as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        require!(
            global.coverage == expected_coverage,
            ErrorCode::InvalidCoverage
        );
    }
    
    Ok(())
}

pub fn verify_pda_sizes() -> Result<()> {
    assert_eq!(std::mem::size_of::<GlobalConfigPDA>(), GlobalConfigPDA::LEN - 8); // Excluding discriminator
    assert_eq!(std::mem::size_of::<VersePDA>(), VersePDA::LEN - 8);
    // ProposalPDA and MapEntryPDA have dynamic sizes based on content
    // Their size is calculated using the space() method
    Ok(())
}

pub fn calculate_max_leverage(coverage: u128, depth: u32, n: u32) -> Result<u64> {
    // Base leverage calculation based on coverage ratio
    let base_leverage = if coverage == u128::MAX {
        // Infinite coverage = no leverage initially
        0u64
    } else if coverage > 10 * PRECISION {
        // High coverage (>10x) = max leverage
        100u64
    } else if coverage > 5 * PRECISION {
        // Medium coverage (5-10x) = 50x leverage
        50u64
    } else if coverage > 2 * PRECISION {
        // Low coverage (2-5x) = 20x leverage
        20u64
    } else if coverage > PRECISION {
        // Very low coverage (1-2x) = 10x leverage
        10u64
    } else {
        // Critical coverage (<1x) = 5x leverage
        5u64
    };
    
    // Apply depth penalty
    let depth_multiplier = match depth {
        0..=5 => 100,
        6..=10 => 80,
        11..=20 => 60,
        21..=30 => 40,
        _ => 20,
    };
    
    // Apply n-value tier adjustment
    let n_multiplier = match n {
        1 => 100,
        2 => 70,
        3..=4 => 50,
        5..=8 => 40,
        9..=16 => 30,
        17..=32 => 20,
        _ => 10,
    };
    
    // Calculate final leverage
    let leverage = base_leverage
        .checked_mul(depth_multiplier)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_mul(n_multiplier)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    Ok(leverage.min(100)) // Cap at 100x
}

pub fn validate_state_transition(
    before: &GlobalConfigPDA,
    after: &GlobalConfigPDA,
) -> Result<()> {
    // Vault can only increase (no withdrawals)
    require!(
        after.vault >= before.vault,
        ErrorCode::InvalidVaultDecrease
    );
    
    // OI changes must match vault changes
    let fee_collected = after.vault - before.vault;
    let oi_change = after.total_oi as i64 - before.total_oi as i64;
    
    // Rough validation (fees should be 0.03% - 0.28% of OI)
    if fee_collected > 0 && oi_change > 0 {
        let fee_rate = (fee_collected as u128 * 10000) / oi_change as u128;
        require!(
            fee_rate >= 3 && fee_rate <= 28,
            ErrorCode::InvalidFeeRate
        );
    }
    
    Ok(())
}

pub fn get_current_slot() -> u64 {
    Clock::get().unwrap().slot
}

pub fn calculate_fee(amount: u64, oi_ratio: u128, fee_base: u64, fee_slope: u64) -> Result<u64> {
    // Dynamic fee = base_fee + slope * oi_ratio
    // fee_base and fee_slope are in basis points
    let base_fee_amount = amount
        .checked_mul(fee_base)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Calculate additional fee based on OI ratio
    let slope_fee_amount = if oi_ratio > PRECISION {
        // OI ratio > 1 means we need more fees
        let ratio_excess = oi_ratio.saturating_sub(PRECISION);
        let slope_multiplier = ratio_excess
            .checked_mul(fee_slope as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(PRECISION)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        amount
            .checked_mul(slope_multiplier as u64)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };
    
    base_fee_amount
        .checked_add(slope_fee_amount)
        .ok_or_else(|| error!(ErrorCode::ArithmeticOverflow))
}

pub fn is_valid_leverage_tier(n: u32, max_leverage: u64, tiers: &[LeverageTier]) -> bool {
    for tier in tiers {
        if n <= tier.n {
            return max_leverage <= tier.max;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_max_leverage() {
        // Test infinite coverage
        let leverage = calculate_max_leverage(u128::MAX, 0, 1).unwrap();
        assert_eq!(leverage, 0);
        
        // Test high coverage
        let leverage = calculate_max_leverage(15 * PRECISION, 0, 1).unwrap();
        assert_eq!(leverage, 100);
        
        // Test with depth penalty
        let leverage = calculate_max_leverage(15 * PRECISION, 10, 1).unwrap();
        assert_eq!(leverage, 80);
        
        // Test with n-value penalty
        let leverage = calculate_max_leverage(15 * PRECISION, 0, 8).unwrap();
        assert_eq!(leverage, 40);
    }
    
    #[test]
    fn test_calculate_fee() {
        // Base fee only (3bp)
        let fee = calculate_fee(1_000_000, PRECISION, 30, 250).unwrap();
        assert_eq!(fee, 3000); // 0.3% of 1M = 3000
        
        // With OI ratio > 1
        let fee = calculate_fee(1_000_000, 2 * PRECISION, 30, 250).unwrap();
        assert_eq!(fee, 28000); // 0.3% base + 2.5% slope = 2.8%
    }
}