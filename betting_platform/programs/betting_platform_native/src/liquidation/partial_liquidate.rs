//! Partial liquidation implementation
//!
//! Implements partial liquidation as per specification:
//! - partial_close(pos, allowed=cap - acc) where cap = 2-8% OI/slot
//! - Uses coverage-based liquidation formula

use solana_program::{
    account_info::{AccountInfo, next_account_info},
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
    events::{Event, EventType},
    keeper_liquidation::{LiquidationKeeper, KEEPER_REWARD_BPS, LIQ_CAP_MIN, LIQ_CAP_MAX},
    math::U64F64,
    state::{Position, GlobalConfigPDA, GlobalConfigPDA as GlobalConfig},
    trading::helpers::{should_liquidate_coverage_based, calculate_liquidation_price_coverage_based},
    liquidation::halt_mechanism::{LiquidationHaltState, check_halt_status},
    define_event,
};

/// Partial liquidation result
#[derive(Debug)]
pub struct PartialLiquidationResult {
    pub liquidated_amount: u64,
    pub remaining_position: u64,
    pub keeper_reward: u64,
    pub is_fully_liquidated: bool,
}

/// Process partial liquidation instruction
pub fn process_partial_liquidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_index: u8,
) -> ProgramResult {
    msg!("Processing partial liquidation for position index: {}", position_index);
    
    let account_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let halt_state_account = next_account_info(account_iter)?;
    
    // Validate keeper is signer
    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    let global_config = GlobalConfig::try_from_slice(&global_config_account.data.borrow())?;
    let halt_state = LiquidationHaltState::try_from_slice(&halt_state_account.data.borrow())?;
    
    // Check if liquidations are halted
    if check_halt_status(&halt_state)? {
        msg!("Liquidations are currently halted");
        return Err(BettingPlatformError::LiquidationHalted.into());
    }
    
    // Get current price from oracle (simplified for now)
    let current_price = get_current_price(&position)?; // In production, would use oracle
    
    // Update position with current price to recalculate PnL and effective leverage
    position.update_with_price(current_price)?;
    
    // Calculate coverage
    let coverage = calculate_coverage(&global_config)?;
    
    // Check if position should be liquidated with updated effective leverage
    let should_liquidate = crate::liquidation::helpers::should_liquidate_coverage_based(
        &position,
        current_price,
        coverage,
    )?;
    
    if !should_liquidate {
        return Err(BettingPlatformError::PositionHealthy.into());
    }
    
    // Calculate liquidation amount based on dynamic cap
    let open_interest = global_config.total_oi;
    let volatility = estimate_volatility(&position); // Simplified volatility estimation
    
    let liquidation_cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(volatility, open_interest as u64)?;
    
    // Calculate actual liquidation amount
    let max_liquidation = (position.size as u128 * liquidation_cap as u128 / 10000) as u64;
    let partial_liquidation_amount = max_liquidation
        .min(position.size)
        .saturating_sub(position.partial_liq_accumulator);
    
    if partial_liquidation_amount == 0 {
        msg!("Position already at liquidation cap for this slot");
        return Ok(());
    }
    
    // Update position
    let actual_liquidation = partial_liquidation_amount.min(position.size);
    
    // Calculate liquidation value in USD for halt mechanism
    let liquidation_value = (actual_liquidation as u128 * current_price as u128 / 1_000_000) as u64;
    
    position.size = position.size.saturating_sub(actual_liquidation);
    position.partial_liq_accumulator = position.partial_liq_accumulator
        .saturating_add(actual_liquidation);
    
    // Calculate keeper reward
    let keeper_reward = (actual_liquidation as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
    
    // Mark position as closed if fully liquidated
    if position.size == 0 {
        position.is_closed = true;
    }
    
    // Save updated position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Emit event
    PartialLiquidationExecuted {
        position_id: position.position_id,
        keeper: *keeper_account.key,
        liquidated_amount: actual_liquidation,
        remaining_size: position.size,
        keeper_reward,
        slot: Clock::get()?.slot,
    }.emit();
    
    msg!(
        "Partial liquidation completed: liquidated={}, remaining={}, reward={}",
        actual_liquidation,
        position.size,
        keeper_reward
    );
    
    Ok(())
}

/// Calculate coverage from global config
fn calculate_coverage(global_config: &GlobalConfig) -> Result<U64F64, ProgramError> {
    // coverage = clamp(LIQ_CAP_MIN, coverage_raw, LIQ_CAP_MAX)
    let coverage_raw = if global_config.vault > 0 && global_config.total_oi > 0 {
        U64F64::from_num(global_config.vault as u64) / U64F64::from_num(global_config.total_oi as u64)
    } else {
        U64F64::from_num(1) // Default coverage
    };
    
    // Apply clamps
    let min_coverage = U64F64::from_num(LIQ_CAP_MIN) / U64F64::from_num(10000);
    let max_coverage = U64F64::from_num(LIQ_CAP_MAX) / U64F64::from_num(10000);
    
    let clamped_coverage = coverage_raw.max(min_coverage).min(max_coverage);
    
    Ok(clamped_coverage)
}

/// Get current price for position (simplified - would use oracle in production)
fn get_current_price(position: &Position) -> Result<u64, ProgramError> {
    // In production, this would fetch from oracle
    // For now, use last mark price if available, otherwise simulate movement
    if position.last_mark_price > 0 && position.last_mark_price != position.entry_price {
        Ok(position.last_mark_price)
    } else {
        // Simulate a small price movement for testing
        let price_movement = position.entry_price / 100; // 1% movement
        let current_price = if position.is_long {
            position.entry_price.saturating_sub(price_movement * 2) // Simulate adverse movement
        } else {
            position.entry_price.saturating_add(price_movement * 2)
        };
        Ok(current_price)
    }
}

/// Estimate volatility for position (simplified)
fn estimate_volatility(position: &Position) -> U64F64 {
    // In production, would calculate from historical price data
    // For now, use a simple leverage-based estimation
    let base_volatility = U64F64::from_num(20); // 20% base volatility
    let leverage_factor = U64F64::from_num(position.leverage) / U64F64::from_num(10);
    
    base_volatility * leverage_factor
}

// Define the partial liquidation event
define_event!(PartialLiquidationExecuted, EventType::PartialLiquidationExecuted, {
    position_id: [u8; 32],
    keeper: Pubkey,
    liquidated_amount: u64,
    remaining_size: u64,
    keeper_reward: u64,
    slot: u64,
});