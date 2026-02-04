//! Funding Rate User Journey
//! 
//! Complete flow for funding rate accumulation and payments during market conditions

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{GlobalConfigPDA, ProposalPDA, Position, UserMap},
    trading::funding_rate::{
        FundingRateState,
        calculate_position_funding,
        update_market_funding,
        apply_funding_to_position,
        FUNDING_RATE_PRECISION,
        SLOTS_PER_HOUR,
        HALT_FUNDING_RATE_BPS,
    },
    coverage::recovery::RecoveryState,
    events::{emit_event, EventType, PositionOpened},
    math::U64F64,
};

/// Funding rate journey state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct FundingRateJourney {
    /// Market being monitored
    pub market_id: u128,
    
    /// Current step
    pub current_step: FundingRateStep,
    
    /// Market state
    pub is_market_halted: bool,
    pub halt_start_slot: Option<u64>,
    pub halt_duration_slots: Option<u64>,
    
    /// Funding metrics
    pub normal_funding_rate_bps: i64,
    pub halt_funding_rate_bps: i64,
    pub accumulated_funding_longs: i64,
    pub accumulated_funding_shorts: i64,
    
    /// Position tracking
    pub long_position_id: Option<[u8; 32]>,
    pub short_position_id: Option<[u8; 32]>,
    pub long_funding_payments: Vec<i64>,
    pub short_funding_payments: Vec<i64>,
    
    /// Timestamps
    pub started_at: i64,
    pub halted_at: Option<i64>,
    pub resumed_at: Option<i64>,
}

/// Funding rate journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum FundingRateStep {
    /// Initial state
    MarketActive,
    
    /// Positions created
    PositionsCreated,
    
    /// Normal funding accumulation
    NormalFundingActive,
    
    /// Market halted
    MarketHalted,
    
    /// Halt funding active
    HaltFundingActive,
    
    /// Market resumed
    MarketResumed,
    
    /// Funding settled
    FundingSettled,
}

/// Create positions during normal market conditions
pub fn create_funding_positions(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    position_size: u64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let long_user_account = next_account_info(account_iter)?;
    let short_user_account = next_account_info(account_iter)?;
    let long_position_account = next_account_info(account_iter)?;
    let short_position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    let long_user_map_account = next_account_info(account_iter)?;
    let short_user_map_account = next_account_info(account_iter)?;
    
    msg!("Creating funding rate test positions");
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Verify market ID
    if u128::from_le_bytes(proposal.proposal_id[0..16].try_into().unwrap()) != market_id {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Step 1: Create long position
    msg!("Step 1: Creating long position");
    let long_position_id = create_test_position(
        long_user_account.key,
        &mut proposal,
        position_size,
        true, // is_long
        0, // outcome
        5, // leverage
    )?;
    
    let long_position = Position {
        discriminator: crate::state::accounts::discriminators::POSITION,
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        user: *long_user_account.key,
        proposal_id: market_id,
        position_id: long_position_id,
        outcome: 0,
        size: position_size,
        notional: position_size,
        margin: position_size / 5, // 5x leverage
        collateral: position_size / 5,
        entry_price: proposal.prices[0],
        liquidation_price: calculate_liquidation_price(proposal.prices[0], 5, true),
        is_long: true,
        leverage: 5,
        created_at: Clock::get()?.unix_timestamp,
        entry_funding_index: Some(proposal.funding_state.long_funding_index),
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: u128::from_le_bytes(proposal.verse_id[0..16].try_into().unwrap()),
        is_short: false,
        last_mark_price: proposal.prices[0],
        unrealized_pnl: 0,
        cross_margin_enabled: false,
        unrealized_pnl_pct: 0,
    };
    
    long_position.serialize(&mut &mut long_position_account.data.borrow_mut()[..])?;
    
    // Update long user map
    let mut long_user_map = UserMap::try_from_slice(&long_user_map_account.data.borrow())?;
    long_user_map.add_position(market_id)?;
    long_user_map.serialize(&mut &mut long_user_map_account.data.borrow_mut()[..])?;
    
    // Step 2: Create short position
    msg!("Step 2: Creating short position");
    let short_position_id = create_test_position(
        short_user_account.key,
        &mut proposal,
        position_size,
        false, // is_short
        0, // outcome
        5, // leverage
    )?;
    
    let short_position = Position {
        discriminator: crate::state::accounts::discriminators::POSITION,
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        user: *short_user_account.key,
        proposal_id: market_id,
        position_id: short_position_id,
        outcome: 0,
        size: position_size,
        notional: position_size,
        margin: position_size / 5,
        collateral: position_size / 5,
        entry_price: proposal.prices[0],
        liquidation_price: calculate_liquidation_price(proposal.prices[0], 5, false),
        is_long: false,
        leverage: 5,
        created_at: Clock::get()?.unix_timestamp,
        entry_funding_index: Some(proposal.funding_state.short_funding_index),
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: u128::from_le_bytes(proposal.verse_id[0..16].try_into().unwrap()),
        is_short: true,
        last_mark_price: proposal.prices[0],
        unrealized_pnl: 0,
        cross_margin_enabled: false,
        unrealized_pnl_pct: 0,
    };
    
    short_position.serialize(&mut &mut short_position_account.data.borrow_mut()[..])?;
    
    // Update short user map
    let mut short_user_map = UserMap::try_from_slice(&short_user_map_account.data.borrow())?;
    short_user_map.add_position(market_id)?;
    short_user_map.serialize(&mut &mut short_user_map_account.data.borrow_mut()[..])?;
    
    // Update proposal
    // Update proposal volume
    proposal.total_volume = proposal.total_volume.saturating_add(position_size * 2);
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Update global state
    global_config.total_oi = global_config.total_oi.saturating_add((position_size * 2) as u128);
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    
    // Step 3: Initialize journey state
    msg!("Step 3: Initializing funding rate journey");
    let journey_state = FundingRateJourney {
        market_id,
        current_step: FundingRateStep::PositionsCreated,
        is_market_halted: false,
        halt_start_slot: None,
        halt_duration_slots: None,
        normal_funding_rate_bps: 0,
        halt_funding_rate_bps: HALT_FUNDING_RATE_BPS as i64,
        accumulated_funding_longs: 0,
        accumulated_funding_shorts: 0,
        long_position_id: Some(long_position_id),
        short_position_id: Some(short_position_id),
        long_funding_payments: Vec::new(),
        short_funding_payments: Vec::new(),
        started_at: Clock::get()?.unix_timestamp,
        halted_at: None,
        resumed_at: None,
    };
    
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Emit events
    emit_event(EventType::PositionOpened, &PositionOpened {
        user: *long_user_account.key,
        proposal_id: market_id,
        outcome: 0,
        size: position_size,
        leverage: 5,
        entry_price: proposal.prices[0],
        is_long: true,
        position_id: long_position_id,
        chain_id: None,
    });
    
    emit_event(EventType::PositionOpened, &PositionOpened {
        user: *short_user_account.key,
        proposal_id: market_id,
        outcome: 0,
        size: position_size,
        leverage: 5,
        entry_price: proposal.prices[0],
        is_long: false,
        position_id: short_position_id,
        chain_id: None,
    });
    
    msg!("Funding rate positions created successfully!");
    msg!("Long position: {:?}", long_position_id);
    msg!("Short position: {:?}", short_position_id);
    
    Ok(())
}

/// Accumulate funding during normal conditions
pub fn accumulate_normal_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    hours_elapsed: u8,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let proposal_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    let long_position_account = next_account_info(account_iter)?;
    let short_position_account = next_account_info(account_iter)?;
    
    msg!("Accumulating normal funding for {} hours", hours_elapsed);
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut journey_state = FundingRateJourney::try_from_slice(&journey_state_account.data.borrow())?;
    let long_position = Position::try_from_slice(&long_position_account.data.borrow())?;
    let short_position = Position::try_from_slice(&short_position_account.data.borrow())?;
    
    // Step 1: Set normal funding rate based on market skew
    msg!("Step 1: Calculating market skew and funding rate");
    // In a real implementation, we'd track longs/shorts separately
    // For demo, assume equal distribution
    let total_volume = proposal.total_volume;
    let total_longs = total_volume / 2;
    let total_shorts = total_volume / 2;
    
    let skew_ratio = if total_shorts > 0 {
        (total_longs as i64 - total_shorts as i64) * 10000 / total_shorts as i64
    } else {
        0
    };
    
    // Funding rate: +0.01% per hour per 10% skew (capped at ±0.5%)
    let funding_rate_bps = (skew_ratio / 1000).max(-50).min(50);
    journey_state.normal_funding_rate_bps = funding_rate_bps;
    
    msg!("Market skew: longs={}, shorts={}", total_longs, total_shorts);
    msg!("Skew ratio: {}bps", skew_ratio);
    msg!("Funding rate: {}bps/hour", funding_rate_bps);
    
    // Step 2: Update funding indices
    msg!("Step 2: Updating funding indices");
    let recovery_state = RecoveryState::new(); // Normal conditions
    let current_slot = Clock::get()?.slot;
    let target_slot = current_slot + (hours_elapsed as u64 * SLOTS_PER_HOUR);
    
    proposal.funding_state.current_funding_rate_bps = funding_rate_bps;
    proposal.funding_state.last_update_slot = current_slot;
    
    update_market_funding(&mut proposal, &recovery_state, target_slot)?;
    
    // Step 3: Calculate funding payments for positions
    msg!("Step 3: Calculating funding payments");
    
    // Long position funding
    let long_funding = calculate_position_funding(
        &long_position,
        &proposal.funding_state,
        long_position.entry_funding_index.unwrap(),
    )?;
    journey_state.long_funding_payments.push(long_funding);
    journey_state.accumulated_funding_longs += long_funding;
    
    // Short position funding
    let short_funding = calculate_position_funding(
        &short_position,
        &proposal.funding_state,
        short_position.entry_funding_index.unwrap(),
    )?;
    journey_state.short_funding_payments.push(short_funding);
    journey_state.accumulated_funding_shorts += short_funding;
    
    msg!("Long position funding payment: {}", long_funding);
    msg!("Short position funding payment: {}", short_funding);
    
    // Update journey state
    journey_state.current_step = FundingRateStep::NormalFundingActive;
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Save proposal
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    msg!("Normal funding accumulated successfully!");
    
    Ok(())
}

/// Halt the market
pub fn halt_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let admin_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    
    msg!("Halting market");
    
    // Verify admin authority
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    if *admin_account.key != global_config.update_authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut journey_state = FundingRateJourney::try_from_slice(&journey_state_account.data.borrow())?;
    
    let current_slot = Clock::get()?.slot;
    
    // Step 1: Halt the market
    msg!("Step 1: Setting market to halted state");
    proposal.funding_state.halt_market(current_slot);
    
    // Update journey state
    journey_state.is_market_halted = true;
    journey_state.halt_start_slot = Some(current_slot);
    journey_state.halted_at = Some(Clock::get()?.unix_timestamp);
    journey_state.current_step = FundingRateStep::MarketHalted;
    
    msg!("Market halted at slot {}", current_slot);
    msg!("Halt funding rate: +{}bps/hour", HALT_FUNDING_RATE_BPS);
    
    // Save states
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Emit halt event
    emit_event(EventType::MarketHalted, &MarketHalted {
        market_id: Pubkey::new_from_array(proposal.proposal_id),
        halt_reason: "Admin halt for testing".to_string(),
        halt_slot: current_slot,
    });
    
    Ok(())
}

/// Accumulate funding during halt
pub fn accumulate_halt_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    hours_halted: u8,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let proposal_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    let long_position_account = next_account_info(account_iter)?;
    let short_position_account = next_account_info(account_iter)?;
    
    msg!("Accumulating halt funding for {} hours", hours_halted);
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut journey_state = FundingRateJourney::try_from_slice(&journey_state_account.data.borrow())?;
    let long_position = Position::try_from_slice(&long_position_account.data.borrow())?;
    let short_position = Position::try_from_slice(&short_position_account.data.borrow())?;
    
    // Verify market is halted
    if !proposal.funding_state.is_halted {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Step 1: Update funding indices with halt rate
    msg!("Step 1: Updating funding indices with halt rate");
    let current_slot = Clock::get()?.slot;
    let target_slot = current_slot + (hours_halted as u64 * SLOTS_PER_HOUR);
    
    // During halt, recovery state has high funding rate
    let mut recovery_state = RecoveryState::new();
    recovery_state.is_active = true;
    recovery_state.funding_rate_boost = HALT_FUNDING_RATE_BPS as u16;
    
    update_market_funding(&mut proposal, &recovery_state, target_slot)?;
    
    // Step 2: Calculate halt funding payments
    msg!("Step 2: Calculating halt funding payments");
    
    // Long position pays high funding during halt
    let long_funding = calculate_position_funding(
        &long_position,
        &proposal.funding_state,
        long_position.entry_funding_index.unwrap(),
    )?;
    journey_state.long_funding_payments.push(long_funding);
    journey_state.accumulated_funding_longs += long_funding;
    
    // Short position receives funding during halt
    let short_funding = calculate_position_funding(
        &short_position,
        &proposal.funding_state,
        short_position.entry_funding_index.unwrap(),
    )?;
    journey_state.short_funding_payments.push(short_funding);
    journey_state.accumulated_funding_shorts += short_funding;
    
    msg!("Halt funding - Long pays: {}", -long_funding);
    msg!("Halt funding - Short receives: {}", short_funding);
    
    // Update journey state
    journey_state.halt_duration_slots = Some(hours_halted as u64 * SLOTS_PER_HOUR);
    journey_state.current_step = FundingRateStep::HaltFundingActive;
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Save proposal
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    msg!("Halt funding accumulated successfully!");
    msg!("Total funding accumulated during halt:");
    msg!("  Longs paid: {}", -journey_state.accumulated_funding_longs);
    msg!("  Shorts received: {}", journey_state.accumulated_funding_shorts);
    
    Ok(())
}

/// Resume market from halt
pub fn resume_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let admin_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    
    msg!("Resuming market from halt");
    
    // Verify admin authority
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    if *admin_account.key != global_config.update_authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut journey_state = FundingRateJourney::try_from_slice(&journey_state_account.data.borrow())?;
    
    // Verify market is halted
    if !proposal.funding_state.is_halted {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Resume market
    proposal.funding_state.resume_market();
    
    // Update journey state
    journey_state.is_market_halted = false;
    journey_state.resumed_at = Some(Clock::get()?.unix_timestamp);
    journey_state.current_step = FundingRateStep::MarketResumed;
    
    msg!("Market resumed - normal funding rates apply");
    
    // Save states
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Emit resume event
    emit_event(EventType::MarketResumed, &MarketResumed {
        market_id: Pubkey::new_from_array(proposal.proposal_id),
        resume_slot: Clock::get()?.slot,
        halt_duration_slots: 0, // Would be calculated from halt_start_slot
    });
    
    Ok(())
}

/// Settle funding payments
pub fn settle_funding_payments(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let long_position_account = next_account_info(account_iter)?;
    let short_position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    
    msg!("Settling funding payments");
    
    // Load accounts
    let mut long_position = Position::try_from_slice(&long_position_account.data.borrow())?;
    let mut short_position = Position::try_from_slice(&short_position_account.data.borrow())?;
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut journey_state = FundingRateJourney::try_from_slice(&journey_state_account.data.borrow())?;
    
    // Step 1: Apply funding to long position
    msg!("Step 1: Applying funding to long position");
    let long_total_funding = journey_state.accumulated_funding_longs;
    apply_funding_to_position(&mut long_position, long_total_funding)?;
    
    // Update position funding index
    long_position.entry_funding_index = Some(proposal.funding_state.long_funding_index);
    long_position.serialize(&mut &mut long_position_account.data.borrow_mut()[..])?;
    
    // Step 2: Apply funding to short position
    msg!("Step 2: Applying funding to short position");
    let short_total_funding = journey_state.accumulated_funding_shorts;
    apply_funding_to_position(&mut short_position, short_total_funding)?;
    
    // Update position funding index
    short_position.entry_funding_index = Some(proposal.funding_state.short_funding_index);
    short_position.serialize(&mut &mut short_position_account.data.borrow_mut()[..])?;
    
    // Update journey state
    journey_state.current_step = FundingRateStep::FundingSettled;
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    msg!("Funding payments settled!");
    msg!("Long position:");
    msg!("  Total funding paid: {}", -long_total_funding);
    msg!("  Updated collateral: {}", long_position.collateral);
    msg!("Short position:");
    msg!("  Total funding received: {}", short_total_funding);
    msg!("  Updated collateral: {}", short_position.collateral);
    
    Ok(())
}

/// Verify funding rate journey completion
pub fn verify_funding_journey(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let journey_state_account = next_account_info(account_iter)?;
    
    // Load journey state
    let journey_state = FundingRateJourney::try_from_slice(&journey_state_account.data.borrow())?;
    
    msg!("Verifying funding rate journey completion");
    
    // Verify journey completed
    if journey_state.current_step != FundingRateStep::FundingSettled {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Calculate total journey duration
    let total_duration = if let Some(resumed_at) = journey_state.resumed_at {
        resumed_at - journey_state.started_at
    } else {
        Clock::get()?.unix_timestamp - journey_state.started_at
    };
    
    // Calculate halt duration
    let halt_duration = if let (Some(halted), Some(resumed)) = (journey_state.halted_at, journey_state.resumed_at) {
        resumed - halted
    } else {
        0
    };
    
    msg!("Journey completed successfully!");
    msg!("Market ID: {}", journey_state.market_id);
    msg!("Total duration: {} seconds", total_duration);
    msg!("Halt duration: {} seconds", halt_duration);
    msg!("Normal funding rate: {}bps/hour", journey_state.normal_funding_rate_bps);
    msg!("Halt funding rate: {}bps/hour", journey_state.halt_funding_rate_bps);
    msg!("Total funding payments:");
    msg!("  Longs paid: {}", -journey_state.accumulated_funding_longs);
    msg!("  Shorts received: {}", journey_state.accumulated_funding_shorts);
    msg!("Net funding flow: {}", journey_state.accumulated_funding_longs + journey_state.accumulated_funding_shorts);
    
    Ok(())
}

/// Helper: Create test position
fn create_test_position(
    user: &Pubkey,
    proposal: &mut ProposalPDA,
    size: u64,
    is_long: bool,
    outcome: u8,
    leverage: u64,
) -> Result<[u8; 32], ProgramError> {
    let position_id = {
        let mut hasher_input = Vec::new();
        hasher_input.extend_from_slice(user.as_ref());
        hasher_input.extend_from_slice(&proposal.proposal_id);
        hasher_input.extend_from_slice(&Clock::get()?.slot.to_le_bytes());
        hasher_input.extend_from_slice(&[if is_long { 1 } else { 0 }]);
        solana_program::hash::hash(&hasher_input).to_bytes()
    };
    
    Ok(position_id)
}

/// Helper: Calculate liquidation price
fn calculate_liquidation_price(entry_price: u64, leverage: u64, is_long: bool) -> u64 {
    let liquidation_threshold = 10000 / leverage;
    
    if is_long {
        entry_price.saturating_sub(entry_price * liquidation_threshold / 10000)
    } else {
        entry_price.saturating_add(entry_price * liquidation_threshold / 10000)
    }
}

/// Market halted event
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct MarketHalted {
    pub market_id: Pubkey,
    pub halt_reason: String,
    pub halt_slot: u64,
}

/// Market resumed event
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct MarketResumed {
    pub market_id: Pubkey,
    pub resume_slot: u64,
    pub halt_duration_slots: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_funding_rate_calculation() {
        // Test normal market skew
        let total_longs = 1_000_000;
        let total_shorts = 900_000;
        
        let skew_ratio = ((total_longs - total_shorts) * 10000) / total_shorts;
        assert_eq!(skew_ratio, 1111); // ~11.11% skew
        
        let funding_rate_bps = (skew_ratio / 1000).max(-50).min(50);
        assert_eq!(funding_rate_bps, 1); // 0.01% per hour
    }
    
    #[test]
    fn test_halt_funding_rate() {
        assert_eq!(HALT_FUNDING_RATE_BPS, 125); // 1.25% per hour during halt
    }
}