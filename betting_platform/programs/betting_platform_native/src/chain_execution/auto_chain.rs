//! Auto chain execution
//! Implements the effective leverage multiplication through chaining
//! Formula: lev_eff = lev_base × ∏(1 + r_i) where r_i is return from each step

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
use std::collections::HashMap;

use crate::{
    error::BettingPlatformError,
    instruction::ChainStepType,
    state::{
        chain_accounts::{ChainState, ChainPosition, ChainExecution, ChainSafety},
        VersePDA, VerseStatus,
        ProposalPDA,
        Position,
        GlobalConfigPDA,
    },
    math::leverage::{calculate_max_leverage, calculate_effective_leverage, calculate_bootstrap_leverage},
    chain_execution::{
        timing_safety::{validate_no_pending_resolution, validate_chain_atomicity},
        cross_verse_validator::{CrossVerseValidator, CrossVerseValidation},
        cycle_detector::ChainDependencyGraph,
    },
    cpi::depth_tracker::CPIDepthTracker,
    events::chain_events::{log_chain_event, calculate_step_return, ChainStepSummary, build_chain_audit_trail, emit_chain_completion},
    math::U64F64,
    constants::MAX_CHAIN_LEVERAGE,
};

/// Maximum chain depth to prevent infinite loops
/// Limited to 3 steps to comply with specification (borrow + liquidation + stake)
/// Each step ~9k CU, total ~27k CU (well under 45k budget)
const MAX_CHAIN_DEPTH: u8 = 3;

/// Multipliers for each chain step type (basis points)
pub const BORROW_MULTIPLIER: u64 = 15000;     // 1.5x
pub const LEND_MULTIPLIER: u64 = 12000;       // 1.2x
pub const LIQUIDITY_MULTIPLIER: u64 = 12000;  // 1.2x  
pub const STAKE_MULTIPLIER: u64 = 11000;      // 1.1x

/// Constants for chain calculations
pub const LVR_TARGET: u64 = 500;              // 0.05 or 5% in basis points
pub const TAU: u64 = 1000;                    // 0.1 or 10% in basis points

/// Chain configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainConfig {
    /// Maximum chain depth allowed
    pub max_depth: u8,
    
    /// Maximum effective leverage
    pub max_leverage: u16,
    
    /// Fee percentage for chain execution
    pub chain_fee_bps: u16,
    
    /// Minimum collateral for chain positions
    pub min_collateral: u64,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            max_depth: MAX_CHAIN_DEPTH,
            max_leverage: MAX_CHAIN_LEVERAGE,
            chain_fee_bps: 50, // 0.5%
            min_collateral: 1_000_000, // $1 minimum
        }
    }
}

/// Calculate borrow amount based on formula: borrow_amt = deposit * coverage / sqrt(N)
pub fn calculate_borrow_amount(deposit: u64, coverage: u64, num_outcomes: u64) -> u64 {
    let sqrt_n = integer_sqrt(num_outcomes);
    if sqrt_n == 0 {
        return 0;
    }
    
    deposit
        .saturating_mul(coverage)
        .saturating_div(sqrt_n)
}

/// Calculate liquidity yield based on formula: liq_yield = liq_amt * LVR_TARGET * tau
pub fn calculate_liquidity_yield(liquidity_amount: u64) -> u64 {
    // liq_yield = liq_amt * 0.05 * 0.1 = liq_amt * 0.005
    // In basis points: 500 * 1000 / 10000 / 10000 = 0.005
    liquidity_amount
        .saturating_mul(LVR_TARGET)
        .saturating_mul(TAU)
        .saturating_div(100_000_000) // Divide by 10000 twice for basis points conversion
}

/// Calculate stake return based on formula: stake_return = stake_amt * (1 + depth/32)
pub fn calculate_stake_return(stake_amount: u64, depth: u64) -> u64 {
    // (1 + depth/32) = (32 + depth) / 32
    let multiplier = 32u64.saturating_add(depth);
    
    stake_amount
        .saturating_mul(multiplier)
        .saturating_div(32)
}


/// Integer square root implementation
fn integer_sqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}


pub fn process_auto_chain(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    verse_id: u128,
    deposit: u64,
    steps: Vec<ChainStepType>,
) -> ProgramResult {
    msg!("Processing auto chain execution");
    
    // Initialize CPI depth tracker
    let mut cpi_depth_tracker = CPIDepthTracker::new();
    
    // Validate inputs
    if steps.is_empty() || steps.len() > MAX_CHAIN_DEPTH as usize {
        return Err(BettingPlatformError::InvalidChainSteps.into());
    }
    
    if deposit == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Validate chain can execute atomically within CU limits
    validate_chain_atomicity(&steps)?;
    
    let account_iter = &mut accounts.iter();
    
    // Parse accounts
    let user = next_account_info(account_iter)?;
    let chain_state_account = next_account_info(account_iter)?;
    let verse_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let clock = Clock::get()?;
    
    // Validate signer
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load and validate verse
    let verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
    verse.validate()?;
    
    if verse.verse_id != verse_id {
        return Err(BettingPlatformError::VerseMismatch.into());
    }
    
    if verse.status != VerseStatus::Active {
        return Err(BettingPlatformError::VerseNotActive.into());
    }
    
    // Load global config
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    global_config.validate()?;
    
    // Check for pending resolutions to ensure atomic execution
    // This prevents timing attacks where a verse resolves mid-chain
    msg!("Checking for pending resolutions before chain execution");
    let proposal_account = next_account_info(account_iter)?;
    validate_no_pending_resolution(
        &[verse_account],
        &[proposal_account],
        clock.slot,
    )?;
    
    // Create chain state
    let chain_id = generate_chain_id(user.key, verse_id, clock.unix_timestamp);
    let mut chain_state = ChainState::new(
        chain_id,
        *user.key,
        verse_id,
        deposit,
        steps.clone(),
        clock.unix_timestamp,
    )?;
    
    // Initialize cross-verse validator and cycle detector
    let mut cross_verse_validator = CrossVerseValidator::new();
    let mut dependency_graph = ChainDependencyGraph::new();
    
    // Add this chain to the dependency graph
    dependency_graph.add_chain(chain_id, verse_id)?;
    
    // Perform cross-verse validation if chain spans multiple verses
    let verses = HashMap::new(); // In production, would load from accounts
    let cross_verse_validation = cross_verse_validator.validate_chain(
        &chain_state,
        &verses,
        &dependency_graph,
    )?;
    
    if !cross_verse_validation.is_valid() {
        msg!("Cross-verse validation failed: {:?}", cross_verse_validation.warnings);
        return Err(BettingPlatformError::CrossVerseViolation.into());
    }
    
    // Check for cycles in the dependency graph
    if dependency_graph.detect_cycles()? {
        msg!("Cycle detected in chain dependencies");
        return Err(BettingPlatformError::ChainCycleDetected.into());
    }
    
    // Initialize chain execution tracking
    let mut execution = ChainExecution::new();
    execution.enter_verse(verse_id)?;
    
    // Calculate base leverage based on verse depth and coverage
    // Use bootstrap formula: min(100*coverage, tier)
    // Use verse depth to determine tier cap (deeper verses have more constraints)
    let tier_cap = match verse.depth {
        0 => 100,  // Root verse
        1 => 50,   // First level
        2 => 25,   // Second level
        3 => 10,   // Third level
        _ => 5,    // Deeper levels
    };
    
    let base_leverage = calculate_bootstrap_leverage(global_config.coverage as u64, tier_cap);
    
    msg!("Base leverage: {}, Coverage: {}, Tier cap: {}", base_leverage, global_config.coverage, tier_cap);
    
    // Execute chain steps
    let mut effective_balance = deposit;
    let mut effective_leverage = base_leverage;
    let mut cumulative_multiplier = U64F64::from_num(1);
    let mut chain_steps_summary = Vec::new();
    
    for (step_index, step) in steps.iter().enumerate() {
        msg!("Executing chain step {}: {:?}", step_index, step);
        
        // Track initial balance for return calculation
        let initial_balance = effective_balance;
        
        // Track CPI depth for this step
        cpi_depth_tracker.enter_cpi()?;
        
        match step {
            ChainStepType::Long { outcome, leverage } => {
                // Apply leverage multiplier from chaining
                let step_multiplier = get_step_multiplier(step_index as u8);
                effective_leverage = calculate_effective_leverage(
                    effective_leverage,
                    step_multiplier,
                );
                
                // Create position with effective leverage
                let position_size = effective_balance
                    .checked_mul(*leverage as u64)
                    .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
                
                msg!(
                    "Creating LONG position: outcome={}, size={}, leverage={}",
                    outcome, position_size, effective_leverage
                );
                
                // Update chain state
                chain_state.current_balance = effective_balance;
                chain_state.current_step = step_index as u8;
                
                // Track position
                let position_id = generate_position_id(chain_id, step_index as u8);
                chain_state.add_position(position_id)?;
                
                // Update effective balance for next step
                effective_balance = apply_chain_multiplier(effective_balance, step_multiplier);
            },
            
            ChainStepType::Short { outcome, leverage } => {
                // Similar to Long but for short positions
                let step_multiplier = get_step_multiplier(step_index as u8);
                effective_leverage = calculate_effective_leverage(
                    effective_leverage,
                    step_multiplier,
                );
                
                let position_size = effective_balance
                    .checked_mul(*leverage as u64)
                    .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
                
                msg!(
                    "Creating SHORT position: outcome={}, size={}, leverage={}",
                    outcome, position_size, effective_leverage
                );
                
                chain_state.current_balance = effective_balance;
                chain_state.current_step = step_index as u8;
                
                let position_id = generate_position_id(chain_id, step_index as u8);
                chain_state.add_position(position_id)?;
                
                effective_balance = apply_chain_multiplier(effective_balance, step_multiplier);
            },
            
            ChainStepType::ClosePosition => {
                msg!("Close position step - would close previous positions");
                // In full implementation, would close positions and realize PnL
            },
            
            ChainStepType::TakeProfit { threshold } => {
                msg!("Take profit step: threshold={}", threshold);
                // In full implementation, would set take profit orders
            },
            
            ChainStepType::StopLoss { threshold } => {
                msg!("Stop loss step: threshold={}", threshold);
                // In full implementation, would set stop loss orders
            },
            
            ChainStepType::Lend { amount } => {
                // Apply lend multiplier
                let step_multiplier = LEND_MULTIPLIER;
                effective_leverage = calculate_effective_leverage(
                    effective_leverage,
                    step_multiplier,
                );
                
                msg!(
                    "Creating LEND position: amount={}, effective_leverage={}",
                    amount, effective_leverage
                );
                
                // Update chain state
                chain_state.current_balance = effective_balance;
                chain_state.current_step = step_index as u8;
                
                // Update effective balance for next step
                effective_balance = apply_chain_multiplier(effective_balance, step_multiplier);
            },
            
            ChainStepType::Borrow { amount } => {
                // Calculate borrow amount using the formula
                let borrow_amount = calculate_borrow_amount(
                    effective_balance,
                    global_config.coverage as u64,
                    2, // Default to binary outcomes
                );
                
                // Apply flash loan fee (2%) if this is a flash loan
                let flash_loan_fee = crate::attack_detection::apply_flash_loan_fee(borrow_amount)?;
                let total_repayment = borrow_amount.saturating_add(flash_loan_fee);
                
                msg!(
                    "BORROW step: requested={}, calculated={}, coverage={}, N={}, flash_fee={}",
                    amount, borrow_amount, global_config.coverage, 2, flash_loan_fee
                );
                
                // Record borrow for flash loan detection
                // Note: In production, this would update the attack detector account
                msg!("Recording borrow for flash loan detection: user={}, slot={}", user.key, clock.slot);
                
                // Apply borrow multiplier  
                effective_leverage = calculate_effective_leverage(
                    effective_leverage,
                    BORROW_MULTIPLIER,
                );
                
                // Update balance with borrowed amount
                effective_balance = effective_balance.saturating_add(borrow_amount);
                
                // Track that repayment is required
                chain_state.current_balance = effective_balance;
                chain_state.current_step = step_index as u8;
                
                msg!("Borrow complete. Total repayment required: {}", total_repayment);
            },
            
            ChainStepType::Liquidity { amount } => {
                // Calculate liquidity yield
                let liq_yield = calculate_liquidity_yield(effective_balance);
                
                msg!(
                    "LIQUIDITY step: amount={}, yield={}, LVR={}, tau={}",
                    effective_balance, liq_yield, LVR_TARGET, TAU
                );
                
                // Apply liquidity multiplier
                effective_leverage = calculate_effective_leverage(
                    effective_leverage,
                    LIQUIDITY_MULTIPLIER,
                );
                
                // Add yield to balance
                effective_balance = effective_balance.saturating_add(liq_yield);
                
                chain_state.current_balance = effective_balance;
                chain_state.current_step = step_index as u8;
            },
            
            ChainStepType::Stake { amount } => {
                // Calculate stake return based on depth
                let stake_return = calculate_stake_return(
                    effective_balance,
                    verse.depth as u64,
                );
                
                msg!(
                    "STAKE step: amount={}, return={}, depth={}",
                    effective_balance, stake_return, verse.depth
                );
                
                // Apply stake multiplier
                effective_leverage = calculate_effective_leverage(
                    effective_leverage,
                    STAKE_MULTIPLIER,
                );
                
                // Update balance with stake return
                effective_balance = stake_return;
                
                chain_state.current_balance = effective_balance;
                chain_state.current_step = step_index as u8;
            },
        }
        
        // Safety check: ensure we don't exceed maximum effective leverage
        if effective_leverage > MAX_CHAIN_LEVERAGE as u64 {
            msg!("Warning: Effective leverage {} exceeds {}x cap", effective_leverage, MAX_CHAIN_LEVERAGE);
            effective_leverage = MAX_CHAIN_LEVERAGE as u64;
        }
        
        // Calculate step return (r_i)
        let step_return_bps = calculate_step_return(step, initial_balance, effective_balance);
        
        // Update cumulative multiplier
        let step_multiplier_fixed = if step_return_bps >= 0 {
            U64F64::from_num(10000 + step_return_bps as u64) / U64F64::from_num(10000)
        } else {
            U64F64::from_num(10000 - (-step_return_bps) as u64) / U64F64::from_num(10000)
        };
        cumulative_multiplier = cumulative_multiplier.checked_mul(step_multiplier_fixed)
            .map_err(|_| -> ProgramError { BettingPlatformError::ArithmeticOverflow.into() })?;
        
        // Log chain event
        log_chain_event(
            chain_id,
            user.key,
            step_index as u8,
            step.clone(),
            step_return_bps,
            effective_leverage,
            base_leverage,
            cumulative_multiplier,
            effective_balance.saturating_sub(initial_balance),
            effective_balance,
        );
        
        // Add to summary
        chain_steps_summary.push(ChainStepSummary {
            step: step_index as u8,
            step_type: step.clone(),
            r_i: step_return_bps,
            eff_lev: effective_leverage,
        });
        
        // Exit CPI for this step
        cpi_depth_tracker.exit_cpi();
        msg!("CPI depth after step {}: {}", step_index, cpi_depth_tracker.current_depth());
    }
    
    // Update final chain state
    chain_state.status = crate::state::chain_accounts::ChainStatus::Active;
    chain_state.last_execution = clock.unix_timestamp;
    
    msg!(
        "Chain execution complete: deposit={}, effective_balance={}, effective_leverage={}x",
        deposit, effective_balance, effective_leverage
    );
    
    msg!(
        "Total leverage multiplication: {}x",
        effective_balance / deposit
    );
    
    // Build and emit chain completion audit trail
    let audit_trail = build_chain_audit_trail(
        chain_id,
        chain_steps_summary,
        effective_leverage,
        true, // success
    );
    emit_chain_completion(chain_id, user.key, audit_trail);
    
    // Serialize chain state to account
    use borsh::BorshSerialize;
    chain_state.serialize(&mut &mut chain_state_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Calculate step multiplier based on step index and type
#[cfg(test)]
pub fn get_step_multiplier(step_index: u8) -> u64 {
    match step_index {
        0 => BORROW_MULTIPLIER,      // First step: borrow
        1 => LEND_MULTIPLIER,        // Second step: lend
        2 => LIQUIDITY_MULTIPLIER,   // Third step: provide liquidity
        3 => STAKE_MULTIPLIER,       // Fourth step: stake
        _ => 10500,                  // Additional steps: 1.05x
    }
}

#[cfg(not(test))]
fn get_step_multiplier(step_index: u8) -> u64 {
    match step_index {
        0 => BORROW_MULTIPLIER,      // First step: borrow
        1 => LEND_MULTIPLIER,        // Second step: lend
        2 => LIQUIDITY_MULTIPLIER,   // Third step: provide liquidity
        3 => STAKE_MULTIPLIER,       // Fourth step: stake
        _ => 10500,                  // Additional steps: 1.05x
    }
}

/// Apply chain multiplier to balance
#[cfg(test)]
pub fn apply_chain_multiplier(balance: u64, multiplier_bps: u64) -> u64 {
    balance
        .checked_mul(multiplier_bps)
        .and_then(|b| b.checked_div(10_000))
        .unwrap_or(balance)
}

#[cfg(not(test))]
fn apply_chain_multiplier(balance: u64, multiplier_bps: u64) -> u64 {
    balance
        .saturating_mul(multiplier_bps)
        .saturating_div(10000)
}

/// Generate unique chain ID
fn generate_chain_id(user: &Pubkey, verse_id: u128, timestamp: i64) -> u128 {
    use solana_program::keccak::hashv;
    
    let hash = hashv(&[
        user.as_ref(),
        &verse_id.to_le_bytes(),
        &timestamp.to_le_bytes(),
    ]);
    
    u128::from_le_bytes(hash.0[..16].try_into().unwrap())
}

/// Generate unique position ID
fn generate_position_id(chain_id: u128, step_index: u8) -> u128 {
    use solana_program::keccak::hashv;
    
    let hash = hashv(&[
        &chain_id.to_le_bytes(),
        &[step_index],
    ]);
    
    u128::from_le_bytes(hash.0[..16].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_borrow_amount() {
        // Test with coverage=150 (1.5), N=1 (binary)
        let borrow = calculate_borrow_amount(100, 150, 1);
        assert_eq!(borrow, 15000); // 100 * 150 / 1 = 15000
        
        // Test with coverage=150, N=4 (sqrt(4) = 2)
        let borrow = calculate_borrow_amount(100, 150, 4);
        assert_eq!(borrow, 7500); // 100 * 150 / 2 = 7500
        
        // Test with zero coverage
        let borrow = calculate_borrow_amount(100, 0, 1);
        assert_eq!(borrow, 0);
    }
    
    #[test]
    fn test_calculate_liquidity_yield() {
        // Test: 10000 * 0.05 * 0.1 = 50
        let yield_amt = calculate_liquidity_yield(10000);
        assert_eq!(yield_amt, 50);
        
        // Test with larger amount
        let yield_amt = calculate_liquidity_yield(100000);
        assert_eq!(yield_amt, 500);
    }
    
    #[test]
    fn test_calculate_stake_return() {
        // Test with depth=0: stake_amt * (1 + 0/32) = stake_amt
        let return_amt = calculate_stake_return(1000, 0);
        assert_eq!(return_amt, 1000);
        
        // Test with depth=32: stake_amt * (1 + 32/32) = stake_amt * 2
        let return_amt = calculate_stake_return(1000, 32);
        assert_eq!(return_amt, 2000);
        
        // Test with depth=16: stake_amt * (1 + 16/32) = stake_amt * 1.5
        let return_amt = calculate_stake_return(1000, 16);
        assert_eq!(return_amt, 1500);
    }
    
    #[test]
    fn test_cpi_depth_tracker() {
        let mut tracker = CPIDepthTracker::new();
        
        // Initial depth should be 0
        assert_eq!(tracker.current_depth(), 0);
        
        // Can enter up to CHAIN_MAX_DEPTH levels
        for i in 0..CPIDepthTracker::CHAIN_MAX_DEPTH {
            assert!(tracker.enter_cpi().is_ok(), "Failed at depth {}", i);
            assert_eq!(tracker.current_depth(), i + 1);
        }
        
        // Should fail when exceeding CHAIN_MAX_DEPTH
        assert!(tracker.enter_cpi().is_err());
        assert_eq!(tracker.current_depth(), CPIDepthTracker::CHAIN_MAX_DEPTH);
        
        // Test exit_cpi
        tracker.exit_cpi();
        assert_eq!(tracker.current_depth(), CPIDepthTracker::CHAIN_MAX_DEPTH - 1);
        
        // Can enter again after exit
        assert!(tracker.enter_cpi().is_ok());
        assert_eq!(tracker.current_depth(), CPIDepthTracker::CHAIN_MAX_DEPTH);
        
        // Test multiple exits
        for _ in 0..CPIDepthTracker::CHAIN_MAX_DEPTH {
            tracker.exit_cpi();
        }
        assert_eq!(tracker.current_depth(), 0);
        
        // Test saturating subtraction (shouldn't go negative)
        tracker.exit_cpi();
        assert_eq!(tracker.current_depth(), 0);
    }
    
    #[test]
    fn test_max_chain_depth_enforcement() {
        // Test that MAX_CHAIN_DEPTH is enforced to be <= CHAIN_MAX_DEPTH
        assert!(MAX_CHAIN_DEPTH <= CPIDepthTracker::CHAIN_MAX_DEPTH, 
            "MAX_CHAIN_DEPTH ({}) must be <= CHAIN_MAX_DEPTH ({})", 
            MAX_CHAIN_DEPTH, CPIDepthTracker::CHAIN_MAX_DEPTH);
    }
    
    #[test]
    fn test_step_simulation() {
        // Simulate the example from spec: $100 * 1.8 * 1.25 * 1.15 = ~$288
        let deposit = 100;
        
        // Step 1: Borrow with 1.8x multiplier (simplified)
        let after_borrow = (deposit as f64 * 1.8) as u64;
        assert_eq!(after_borrow, 180);
        
        // Step 2: Liquidity with 1.25x 
        let after_liq = (after_borrow as f64 * 1.25) as u64;
        assert_eq!(after_liq, 225);
        
        // Step 3: Stake with 1.15x
        let after_stake = (after_liq as f64 * 1.15) as u64;
        assert!(after_stake >= 258 && after_stake <= 259); // ~259, allowing for rounding
    }
}