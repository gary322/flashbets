use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Burn};
use crate::account_structs::*;
use crate::errors::ErrorCode;
use crate::events::*;
use crate::fixed_math::{FixedPoint, PRECISION as FIXED_PRECISION};
use crate::math::calculate_exp_positive;

// Fee Calculation Engine

#[derive(Accounts)]
pub struct DistributeFees<'info> {
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub vault_token_account: Account<'info, token::TokenAccount>,
    
    /// CHECK: This is the vault authority PDA
    pub vault_authority: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub usdc_mint: Account<'info, token::Mint>,
    
    pub token_program: Program<'info, Token>,
}

pub fn calculate_trading_fee(
    global_config: &GlobalConfigPDA,
    notional: u64,
    coverage: u128,
) -> Result<u64> {
    let global = global_config;

    // Formula: taker_fee = FEE_BASE (3bp) + FEE_SLOPE (25bp) * exp(-3*coverage)
    let fee_base = global.fee_base; // 300 = 3bp

    // Calculate exponential component
    let exp_component = calculate_exp_fee_component(coverage);

    let fee_slope_adjusted = global.fee_slope
        .saturating_mul(exp_component)
        .checked_div(PRECISION as u64)
        .unwrap_or(0);

    let total_fee_bps = fee_base.saturating_add(fee_slope_adjusted);

    // Apply to notional
    let fee = notional
        .saturating_mul(total_fee_bps)
        .checked_div(10000)
        .unwrap_or(0);

    // Minimum fee of 0.001 SOL
    Ok(fee.max(1_000_000))
}

pub fn calculate_exp_fee_component(coverage: u128) -> u64 {
    // exp(-3*coverage) approximation using Taylor series
    // For coverage > 2.0, return near 0
    if coverage > 2 * FIXED_PRECISION {
        return 1; // Near 0
    }

    // Scale coverage to fixed point
    let x = (coverage * 3) / FIXED_PRECISION; // -3*coverage

    // Taylor series: e^x ≈ 1 + x + x²/2 + x³/6 + ...
    // Since x is negative, we calculate e^(-|x|) = 1/e^|x|

    if x == 0 {
        return FIXED_PRECISION as u64;
    }

    // For negative exponent, calculate 1/e^|x|
    let exp_positive = calculate_exp_positive(x);

    // Return 1/exp_positive
    (FIXED_PRECISION as u64)
        .saturating_mul(FIXED_PRECISION as u64)
        .checked_div(exp_positive)
        .unwrap_or(1)
}

// Removed - using calculate_exp_positive from math module

pub fn distribute_fees(
    ctx: Context<DistributeFees>,
    fee_amount: u64,
) -> Result<()> {
    let global = &mut ctx.accounts.global_config;

    // Distribution: 70% vault, 20% MMT rewards, 10% burn
    let vault_portion = fee_amount
        .saturating_mul(70)
        .checked_div(100)
        .unwrap_or(0);

    let mmt_portion = fee_amount
        .saturating_mul(20)
        .checked_div(100)
        .unwrap_or(0);

    let burn_portion = fee_amount
        .saturating_sub(vault_portion)
        .saturating_sub(mmt_portion);

    // Add to vault
    global.vault = global.vault
        .checked_add(vault_portion)
        .ok_or(ErrorCode::MathOverflow)?;

    // Add to MMT reward pool
    global.mmt_reward_pool = global.mmt_reward_pool
        .checked_add(mmt_portion)
        .ok_or(ErrorCode::MathOverflow)?;

    // Burn tokens
    let cpi_accounts = Burn {
        mint: ctx.accounts.usdc_mint.to_account_info(),
        from: ctx.accounts.vault_token_account.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::burn(cpi_ctx, burn_portion)?;

    emit!(FeeDistributionEvent {
        total_fee: fee_amount,
        vault_portion,
        mmt_portion,
        burn_portion,
        new_vault_balance: global.vault,
    });

    Ok(())
}

// Maker rebate calculation for spread improvement
pub fn calculate_maker_rebate(
    spread_improvement_bps: u64,
    volume: u64,
) -> u64 {
    // Rebate = volume * min(spread_improvement, 10bp) * 0.5
    let capped_improvement = spread_improvement_bps.min(10);
    
    volume
        .saturating_mul(capped_improvement)
        .saturating_mul(5) // 0.5 = 50%
        .checked_div(100_000) // Convert from basis points and percentage
        .unwrap_or(0)
}

// Calculate fee based on market conditions
pub fn calculate_dynamic_fee(
    base_fee_bps: u64,
    volume_24h: u64,
    volatility: u64,
    coverage: u128,
) -> u64 {
    // Start with base fee
    let mut fee = base_fee_bps;
    
    // Volume discount: reduce fee by up to 50% for high volume
    if volume_24h > 1_000_000_000_000 { // > $1M
        fee = fee.saturating_mul(50).checked_div(100).unwrap_or(fee);
    } else if volume_24h > 100_000_000_000 { // > $100k
        fee = fee.saturating_mul(75).checked_div(100).unwrap_or(fee);
    }
    
    // Volatility surcharge: increase fee by up to 100% for high volatility
    let vol_multiplier = if volatility > VOLATILITY_PRECISION {
        200 // 2x for extreme volatility
    } else {
        100 + (volatility * 100) / VOLATILITY_PRECISION
    };
    
    fee = fee.saturating_mul(vol_multiplier).checked_div(100).unwrap_or(fee);
    
    // Coverage adjustment: lower coverage = higher fees
    if coverage < PRECISION {
        let coverage_multiplier = (PRECISION * 150) / coverage.max(PRECISION / 10);
        fee = fee.saturating_mul(coverage_multiplier as u64)
            .checked_div(100)
            .unwrap_or(fee);
    }
    
    // Cap at 28 basis points
    fee.min(2800)
}

// Calculate protocol revenue share
pub fn calculate_protocol_revenue(
    total_fees: u64,
    mmt_staked: u64,
    total_mmt_supply: u64,
) -> (u64, u64) {
    // Protocol takes 30% base revenue
    let protocol_base = total_fees
        .saturating_mul(30)
        .checked_div(100)
        .unwrap_or(0);
    
    // Additional revenue based on MMT staking ratio
    let stake_ratio = if total_mmt_supply > 0 {
        (mmt_staked * 10000) / total_mmt_supply
    } else {
        0
    };
    
    // If > 50% MMT staked, protocol gets additional 10%
    let bonus_revenue = if stake_ratio > 5000 {
        total_fees.saturating_mul(10).checked_div(100).unwrap_or(0)
    } else {
        0
    };
    
    let protocol_total = protocol_base.saturating_add(bonus_revenue);
    let community_share = total_fees.saturating_sub(protocol_total);
    
    (protocol_total, community_share)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_calculation() {
        // Test with high coverage (2.0)
        let high_coverage = 2 * PRECISION;
        let high_cov_fee = calculate_exp_fee_component(high_coverage);
        assert!(high_cov_fee < PRECISION as u64 / 100); // <1% of base

        // Test with low coverage (0.5)
        let low_coverage = PRECISION / 2;
        let low_cov_fee = calculate_exp_fee_component(low_coverage);
        assert!(low_cov_fee > PRECISION as u64 / 2); // >50% of base
    }

    #[test]
    fn test_dynamic_fee() {
        let base_fee = 300; // 3bp
        
        // High volume should reduce fee
        let high_vol_fee = calculate_dynamic_fee(
            base_fee,
            2_000_000_000_000, // $2M volume
            5000, // Normal volatility
            PRECISION, // Normal coverage
        );
        assert!(high_vol_fee < base_fee);
        
        // High volatility should increase fee
        let high_vol_fee = calculate_dynamic_fee(
            base_fee,
            100_000_000, // Low volume
            15000, // High volatility (150%)
            PRECISION, // Normal coverage
        );
        assert!(high_vol_fee > base_fee);
    }
}