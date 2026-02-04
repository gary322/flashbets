//! Trading helper functions

use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    error::BettingPlatformError,
    math::{U64F64, U128F128},
    state::{Position, Proposal},
};

/// Calculate entry price for a position
pub fn calculate_entry_price(
    proposal: &Proposal,
    outcome: u8,
    is_long: bool,
) -> Result<u64, ProgramError> {
    if outcome >= proposal.num_outcomes() {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }

    let current_price = proposal.current_prices()[outcome as usize];
    
    // Add small spread for market impact
    let spread = current_price / 1000; // 0.1% spread
    
    if is_long {
        Ok(current_price.saturating_add(spread))
    } else {
        Ok(current_price.saturating_sub(spread))
    }
}

/// Calculate margin requirement
pub fn calculate_margin_requirement(
    notional: u64,
    leverage: u64,
) -> Result<u64, ProgramError> {
    if leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }

    Ok(notional / leverage)
}

/// Calculate liquidation price using proper margin ratio formula
/// MR = 1/lev + sigma * sqrt(lev) * f(n)
pub fn calculate_liquidation_price(
    entry_price: u64,
    leverage: u64,
    is_long: bool,
) -> Result<u64, ProgramError> {
    if leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }

    // Calculate margin ratio using the proper formula
    let margin_ratio = calculate_margin_ratio(leverage, 1)?; // n=1 for single position
    
    let price_fp = U64F64::from_num(entry_price);
    let margin_ratio_fp = U64F64::from_num(margin_ratio) / U64F64::from_num(10000); // Convert from bps
    
    let liquidation_distance = price_fp * margin_ratio_fp;
    
    if is_long {
        let liq_price = price_fp - liquidation_distance;
        Ok(liq_price.to_num())
    } else {
        let liq_price = price_fp + liquidation_distance;
        Ok(liq_price.to_num())
    }
}

/// Calculate margin ratio using the specification formula
/// MR = 1/lev + sigma * sqrt(lev) * f(n)
/// Returns margin ratio in basis points
pub fn calculate_margin_ratio(leverage: u64, num_positions: u64) -> Result<u64, ProgramError> {
    use crate::keeper_liquidation::{SIGMA_FACTOR};
    
    if leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    // Calculate 1/leverage in basis points
    let base_margin_bps = 10000u64 / leverage;
    
    // Calculate sigma * sqrt(leverage) * f(n)
    // f(n) = 1 + log(n)/10 for n > 1, normalized to basis points
    // This accounts for correlation risk between multiple positions
    let f_n = if num_positions <= 1 {
        10000u64 // f(1) = 1.0
    } else {
        // Approximate log(n) using bit position (leading zeros)
        let log_n = 64u64.saturating_sub(num_positions.leading_zeros() as u64);
        // f(n) = 1 + log(n)/10, scaled to basis points
        10000u64 + (log_n * 1000u64) / 10u64
    };
    
    // Calculate sqrt(leverage) using integer approximation
    let sqrt_lev = integer_sqrt(leverage);
    
    // sigma * sqrt(lev) * f(n) / 10000 (to normalize f(n))
    let volatility_component = (SIGMA_FACTOR * sqrt_lev * f_n) / (10000 * 10000);
    
    Ok(base_margin_bps + volatility_component)
}

/// Integer square root approximation
fn integer_sqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}

/// Calculate PnL for a position
pub fn calculate_pnl(
    position: &Position,
    exit_price: u64,
) -> Result<i64, ProgramError> {
    let entry_fp = U128F128::from_num(position.entry_price as u128);
    let exit_fp = U128F128::from_num(exit_price as u128);
    let size_fp = U128F128::from_num(position.size as u128);
    
    let price_diff = if position.is_long {
        exit_fp - entry_fp
    } else {
        entry_fp - exit_fp
    };
    
    let pnl = price_diff.checked_mul(size_fp)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(entry_fp)
        .ok_or(BettingPlatformError::DivisionByZero)?;
    
    // Apply leverage
    let leveraged_pnl = pnl.checked_mul(U128F128::from_num(position.leverage as u128))
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    Ok(leveraged_pnl.to_num() as i64)
}

/// Calculate fee for a trade
pub fn calculate_trading_fee(
    notional: u64,
    fee_bps: u16,
) -> Result<u64, ProgramError> {
    let fee = (notional as u128)
        .saturating_mul(fee_bps as u128)
        .saturating_div(10_000);
    
    Ok(fee as u64)
}

/// Check if position should be liquidated
pub fn should_liquidate(
    position: &Position,
    current_price: u64,
) -> bool {
    if position.is_closed {
        return false;
    }

    if position.is_long {
        current_price <= position.liquidation_price
    } else {
        current_price >= position.liquidation_price
    }
}

/// Validate leverage is within allowed bounds
pub fn validate_leverage(
    leverage: u64,
    max_leverage: u64,
) -> Result<(), ProgramError> {
    if leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    if leverage > max_leverage {
        return Err(BettingPlatformError::LeverageTooHigh.into());
    }
    
    Ok(())
}

/// Generate unique position ID
pub fn generate_position_id(
    user: &Pubkey,
    proposal_id: u128,
    nonce: u64,
) -> u128 {
    use solana_program::keccak;
    
    let mut data = Vec::new();
    data.extend_from_slice(user.as_ref());
    data.extend_from_slice(&proposal_id.to_le_bytes());
    data.extend_from_slice(&nonce.to_le_bytes());
    
    let hash = keccak::hash(&data);
    u128::from_le_bytes(hash.0[..16].try_into().unwrap())
}

/// Calculate liquidation price using margin_ratio < 1/coverage formula
/// This is the specification-compliant version
/// Liquidation occurs when: margin_ratio < 1/coverage
pub fn calculate_liquidation_price_coverage_based(
    entry_price: u64,
    position_size: u64,
    margin: u64,
    coverage: U64F64,
    is_long: bool,
) -> Result<u64, ProgramError> {
    if coverage == U64F64::from_num(0) {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // margin_ratio = margin / position_value
    // liquidation when: margin_ratio < 1/coverage
    // => margin / (position_size * price) < 1/coverage
    // => margin * coverage < position_size * price
    // => price > (margin * coverage) / position_size  (for long)
    // => price < (margin * coverage) / position_size  (for short)
    
    let margin_fp = U64F64::from_num(margin);
    let size_fp = U64F64::from_num(position_size);
    let entry_fp = U64F64::from_num(entry_price);
    
    // For leverage-based positions: margin = position_value / leverage
    // So: margin = (size * entry_price) / leverage
    // We need to find the price where margin_ratio = 1/coverage
    
    // Calculate the liquidation threshold based on coverage
    let coverage_inv = U64F64::from_num(1)
        .checked_div(coverage)?;
    
    if is_long {
        // Long positions liquidate when price drops
        // Price drops by (1 - coverage_inv) from entry
        let liq_factor = U64F64::from_num(1)
            .checked_sub(coverage_inv)
            .unwrap_or(U64F64::from_num(0));
        let liq_price = entry_fp
            .checked_mul(liq_factor)?;
        Ok(liq_price.to_num())
    } else {
        // Short positions liquidate when price rises
        // Price rises by (1 + coverage_inv) from entry
        let liq_factor = U64F64::from_num(1)
            .checked_add(coverage_inv)?;
        let liq_price = entry_fp
            .checked_mul(liq_factor)?;
        Ok(liq_price.to_num())
    }
}

/// Check if position should be liquidated based on margin_ratio < 1/coverage
pub fn should_liquidate_coverage_based(
    current_price: u64,
    position_size: u64,
    margin: u64,
    coverage: U64F64,
) -> Result<bool, ProgramError> {
    if coverage == U64F64::from_num(0) {
        return Ok(true); // Zero coverage means immediate liquidation
    }
    
    let price_fp = U64F64::from_num(current_price);
    let size_fp = U64F64::from_num(position_size);
    let margin_fp = U64F64::from_num(margin);
    
    // Calculate position value
    let position_value = price_fp
        .checked_mul(size_fp)?;
    
    // Calculate margin ratio
    let margin_ratio = margin_fp
        .checked_div(position_value)?;
    
    // Check if margin_ratio < 1/coverage
    let coverage_threshold = U64F64::from_num(1)
        .checked_div(coverage)?;
    
    Ok(margin_ratio < coverage_threshold)
}