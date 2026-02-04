//! Drawdown Handler for Extreme Market Scenarios
//!
//! Handles -297% drawdown scenarios with partial liquidations

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    constants::{PARTIAL_LIQUIDATION_BPS, MAX_DRAWDOWN_BPS},
    state::{Position, GlobalConfigPDA},
    math::fixed_point::U64F64,
};

/// Drawdown tracking state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DrawdownState {
    /// Current drawdown in basis points (negative value)
    pub current_drawdown_bps: i32,
    
    /// Maximum observed drawdown
    pub max_drawdown_bps: i32,
    
    /// Number of partial liquidations executed
    pub partial_liquidation_count: u32,
    
    /// Total value liquidated
    pub total_liquidated: u64,
    
    /// Last update timestamp
    pub last_update: i64,
}

/// Calculate partial liquidation amount for extreme drawdown
pub fn calculate_extreme_drawdown_liquidation(
    position_size: u64,
    current_drawdown_bps: i32,
    slots_since_last: u64,
) -> Result<u64, ProgramError> {
    // Validate drawdown is negative
    if current_drawdown_bps > 0 {
        return Ok(0); // No liquidation needed for positive PnL
    }
    
    // Calculate severity factor based on drawdown depth
    let severity = if current_drawdown_bps <= MAX_DRAWDOWN_BPS {
        // Extreme drawdown (-297% or worse)
        msg!("EXTREME DRAWDOWN: {}bps, initiating emergency liquidation", current_drawdown_bps);
        3 // Triple liquidation rate
    } else if current_drawdown_bps <= -10000 {
        // Severe drawdown (-100% or worse)
        2 // Double liquidation rate
    } else if current_drawdown_bps <= -5000 {
        // Moderate drawdown (-50% or worse)
        1 // Normal liquidation rate
    } else {
        return Ok(0); // No liquidation for small drawdowns
    };
    
    // Calculate base liquidation amount (8% per slot)
    let base_amount = (position_size as u128 * PARTIAL_LIQUIDATION_BPS as u128 / 10000) as u64;
    
    // Apply severity multiplier and slot count
    let liquidation_amount = base_amount
        .saturating_mul(severity)
        .saturating_mul(slots_since_last.min(10)); // Cap at 10 slots
    
    // Cap at position size
    Ok(liquidation_amount.min(position_size))
}

/// Handle extreme drawdown scenario
pub fn handle_extreme_drawdown(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let position_account = next_account_info(account_info_iter)?;
    let drawdown_account = next_account_info(account_info_iter)?;
    let global_config = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify keeper is authorized
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    let mut drawdown_state = DrawdownState::try_from_slice(&drawdown_account.data.borrow())?;
    let global = GlobalConfigPDA::try_from_slice(&global_config.data.borrow())?;
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Calculate current PnL and drawdown
    let current_value = calculate_position_value(&position, global.vault as u64)?;
    let initial_value = position.collateral;
    let pnl_bps = if current_value < initial_value {
        -((initial_value - current_value) as i128 * 10000 / initial_value as i128) as i32
    } else {
        ((current_value - initial_value) as i128 * 10000 / initial_value as i128) as i32
    };
    
    // Update drawdown tracking
    drawdown_state.current_drawdown_bps = pnl_bps;
    if pnl_bps < drawdown_state.max_drawdown_bps {
        drawdown_state.max_drawdown_bps = pnl_bps;
    }
    
    // Calculate slots elapsed
    let slots_elapsed = if drawdown_state.last_update > 0 {
        ((clock.unix_timestamp - drawdown_state.last_update) as u64 / 400).max(1) // 400ms per slot
    } else {
        1
    };
    
    // Calculate liquidation amount
    let liquidation_amount = calculate_extreme_drawdown_liquidation(
        position.size,
        pnl_bps,
        slots_elapsed,
    )?;
    
    if liquidation_amount == 0 {
        msg!("No liquidation needed for drawdown {}bps", pnl_bps);
        return Ok(());
    }
    
    // Execute partial liquidation
    msg!(
        "Executing extreme drawdown liquidation: {} tokens for {}bps drawdown",
        liquidation_amount,
        pnl_bps
    );
    
    // Update position
    position.size = position.size.saturating_sub(liquidation_amount);
    position.created_at = clock.unix_timestamp;
    
    // Update drawdown state
    drawdown_state.partial_liquidation_count += 1;
    drawdown_state.total_liquidated += liquidation_amount;
    drawdown_state.last_update = clock.unix_timestamp;
    
    // Check if position should be closed
    if position.size == 0 || pnl_bps <= MAX_DRAWDOWN_BPS {
        position.is_closed = true;
        msg!("Position closed due to extreme drawdown");
    }
    
    // Serialize back
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    drawdown_state.serialize(&mut &mut drawdown_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Calculate position value based on current market conditions
fn calculate_position_value(
    position: &Position,
    vault_balance: u64,
) -> Result<u64, ProgramError> {
    // Simplified calculation - in production would use oracle prices
    let leverage_factor = U64F64::from_num(position.leverage as u64) / U64F64::from_num(100);
    let coverage_ratio = U64F64::from_num(vault_balance) / U64F64::from_num(position.collateral);
    
    let value = U64F64::from_num(position.collateral) * leverage_factor * coverage_ratio;
    Ok(value.to_num())
}

/// Liquidation cascade prevention
pub fn prevent_liquidation_cascade(
    total_oi: u64,
    pending_liquidations: u64,
    market_depth: u64,
) -> Result<bool, ProgramError> {
    // Prevent cascade if pending liquidations exceed 20% of market depth
    let cascade_threshold = market_depth / 5;
    
    if pending_liquidations > cascade_threshold {
        msg!(
            "Liquidation cascade risk detected: {} pending vs {} threshold",
            pending_liquidations,
            cascade_threshold
        );
        return Ok(false); // Halt liquidations
    }
    
    // Check if total OI is at risk
    let oi_risk_threshold = total_oi / 3; // 33% of OI
    if pending_liquidations > oi_risk_threshold {
        msg!("Excessive OI at risk: {} of {} total", pending_liquidations, total_oi);
        return Ok(false);
    }
    
    Ok(true) // Safe to proceed
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extreme_drawdown_calculation() {
        // Test -297% drawdown
        let amount = calculate_extreme_drawdown_liquidation(
            1_000_000, // 1M position
            -29700,    // -297% drawdown
            1,         // 1 slot
        ).unwrap();
        
        // Should be 8% * 3 (severity) * 1 (slot) = 24%
        assert_eq!(amount, 240_000);
    }
    
    #[test]
    fn test_cascade_prevention() {
        let safe = prevent_liquidation_cascade(
            10_000_000, // 10M total OI
            1_000_000,  // 1M pending liquidations (10%)
            5_000_000,  // 5M market depth
        ).unwrap();
        
        assert!(safe); // Should be safe (under 20% of depth)
        
        let unsafe_cascade = prevent_liquidation_cascade(
            10_000_000, // 10M total OI
            2_000_000,  // 2M pending liquidations (40% of depth)
            5_000_000,  // 5M market depth
        ).unwrap();
        
        assert!(!unsafe_cascade); // Should halt (over 20% of depth)
    }
}