//! Funding rate mechanism for perpetual markets
//! 
//! Implements funding rate accumulation and payments between longs and shorts
//! During market halts, funding rate accumulates at +1.25% per hour

use solana_program::{
    account_info::AccountInfo,
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
    state::{GlobalConfigPDA, ProposalPDA, Position},
    coverage::recovery::RecoveryState,
    math::U64F64,
};

/// Funding rate constants
pub const FUNDING_RATE_PRECISION: u64 = 10_000; // 0.01% = 1 basis point
pub const SLOTS_PER_HOUR: u64 = 216_000; // ~3600 seconds at 400ms/slot
pub const MAX_FUNDING_RATE_BPS: u64 = 125; // 1.25% max funding rate per hour
pub const HALT_FUNDING_RATE_BPS: u64 = 125; // 1.25% funding rate during halts

/// Funding rate state per market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct FundingRateState {
    /// Current funding rate (positive = longs pay shorts)
    pub current_funding_rate_bps: i64,
    
    /// Accumulated funding index for longs
    pub long_funding_index: U64F64,
    
    /// Accumulated funding index for shorts
    pub short_funding_index: U64F64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Total funding paid by longs
    pub total_funding_longs: i64,
    
    /// Total funding paid by shorts
    pub total_funding_shorts: i64,
    
    /// Market halted flag
    pub is_halted: bool,
    
    /// Halt start slot
    pub halt_start_slot: u64,
}

impl FundingRateState {
    pub fn new(current_slot: u64) -> Self {
        Self {
            current_funding_rate_bps: 0,
            long_funding_index: U64F64::from_num(0),
            short_funding_index: U64F64::from_num(0),
            last_update_slot: current_slot,
            total_funding_longs: 0,
            total_funding_shorts: 0,
            is_halted: false,
            halt_start_slot: 0,
        }
    }
    
    /// Calculate time-weighted funding rate including halt periods
    pub fn calculate_weighted_funding_rate(
        &self,
        recovery_state: &RecoveryState,
        current_slot: u64,
    ) -> i64 {
        // During halts, use halt funding rate
        if self.is_halted {
            return HALT_FUNDING_RATE_BPS as i64;
        }
        
        // During recovery mode, use the funding rate boost from recovery state
        let recovery_boost = if recovery_state.is_active {
            recovery_state.funding_rate_boost as i64
        } else {
            0
        };
        
        // Base funding rate calculation would go here
        // For now, return recovery boost as the funding rate
        recovery_boost
    }
    
    /// Update funding indices based on elapsed time
    pub fn update_funding_indices(
        &mut self,
        recovery_state: &RecoveryState,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        if current_slot <= self.last_update_slot {
            return Ok(());
        }
        
        let slots_elapsed = current_slot - self.last_update_slot;
        let hours_elapsed = U64F64::from_num(slots_elapsed) / U64F64::from_num(SLOTS_PER_HOUR);
        
        // Get weighted funding rate
        let funding_rate_bps = self.calculate_weighted_funding_rate(recovery_state, current_slot);
        let funding_rate = U64F64::from_num(funding_rate_bps.abs() as u64) / U64F64::from_num(FUNDING_RATE_PRECISION);
        let funding_rate = if funding_rate_bps < 0 { U64F64::from_num(0) - funding_rate } else { funding_rate };
        
        // Calculate funding increment
        let funding_increment = funding_rate * hours_elapsed;
        
        // Update indices based on who pays whom
        if funding_rate_bps > 0 {
            // Longs pay shorts
            self.long_funding_index = self.long_funding_index
                .checked_add(funding_increment)
                .map_err(|_| BettingPlatformError::NumericalOverflow)?;
        } else if funding_rate_bps < 0 {
            // Shorts pay longs
            self.short_funding_index = self.short_funding_index
                .checked_add(if funding_increment > U64F64::from_num(0) { funding_increment } else { U64F64::from_num(0) - funding_increment })
                .map_err(|_| BettingPlatformError::NumericalOverflow)?;
        }
        
        self.current_funding_rate_bps = funding_rate_bps;
        self.last_update_slot = current_slot;
        
        msg!("Funding indices updated: long_index={}, short_index={}, rate={}bps", 
            self.long_funding_index, self.short_funding_index, funding_rate_bps);
        
        Ok(())
    }
    
    /// Mark market as halted
    pub fn halt_market(&mut self, current_slot: u64) {
        self.is_halted = true;
        self.halt_start_slot = current_slot;
        msg!("Market halted at slot {}, funding rate set to {}bps/hour", 
            current_slot, HALT_FUNDING_RATE_BPS);
    }
    
    /// Resume market from halt
    pub fn resume_market(&mut self) {
        self.is_halted = false;
        msg!("Market resumed, normal funding rates apply");
    }
}

/// Calculate funding payment for a position
pub fn calculate_position_funding(
    position: &Position,
    funding_state: &FundingRateState,
    entry_funding_index: U64F64,
) -> Result<i64, ProgramError> {
    let current_index = if position.is_long {
        funding_state.long_funding_index
    } else {
        funding_state.short_funding_index
    };
    
    // Calculate funding difference since position entry
    let funding_diff = current_index
        .checked_sub(entry_funding_index)
        .map_err(|_| BettingPlatformError::NumericalOverflow)?;
    
    // Apply to position size
    let position_value = U64F64::from_num(position.size);
    let funding_payment = position_value
        .checked_mul(funding_diff)
        .map_err(|_| BettingPlatformError::NumericalOverflow)?;
    
    // Convert to i64 (negative means position pays, positive means position receives)
    let payment = funding_payment.to_num() as i64;
    
    // For longs, payment is negative (they pay)
    // For shorts, payment is positive (they receive) when longs pay
    Ok(if position.is_long { -payment } else { payment })
}

/// Update funding for all positions in a market
pub fn update_market_funding(
    market: &mut ProposalPDA,
    recovery_state: &RecoveryState,
    current_slot: u64,
) -> ProgramResult {
    // Get or initialize funding state
    let funding_state = &mut market.funding_state;
    
    // Update funding indices
    funding_state.update_funding_indices(recovery_state, current_slot)?;
    
    // Emit event
    msg!("Market funding updated: rate={}bps/hour, halted={}", 
        funding_state.current_funding_rate_bps, 
        funding_state.is_halted);
    
    Ok(())
}

/// Apply funding payment to a position
pub fn apply_funding_to_position(
    position: &mut Position,
    funding_payment: i64,
) -> ProgramResult {
    // Update position collateral based on funding payment
    if funding_payment < 0 {
        // Position pays funding
        let payment_amount = (-funding_payment) as u64;
        position.collateral = position.collateral
            .checked_sub(payment_amount)
            .ok_or(BettingPlatformError::InsufficientCollateral)?;
    } else {
        // Position receives funding
        position.collateral = position.collateral
            .checked_add(funding_payment as u64)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
    }
    
    msg!("Funding applied to position: payment={}, new_collateral={}", 
        funding_payment, position.collateral);
    
    Ok(())
}

/// Process funding rate update instruction
pub fn process_update_funding_rate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let market_account = next_account_info(account_iter)?;
    let global_state_account = next_account_info(account_iter)?;
    let clock = Clock::get()?;
    
    // Deserialize accounts
    let mut market = ProposalPDA::try_from_slice(&market_account.data.borrow())?;
    let global_state = GlobalConfigPDA::try_from_slice(&global_state_account.data.borrow())?;
    
    // Create a default recovery state based on coverage
    // In production, this would be fetched from a separate recovery state account
    let recovery_state = RecoveryState::new();
    
    // Update market funding
    update_market_funding(&mut market, &recovery_state, clock.slot)?;
    
    // Serialize back
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Process position funding settlement
pub fn process_settle_position_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let position_account = next_account_info(account_iter)?;
    let market_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let clock = Clock::get()?;
    
    // Deserialize accounts
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    let market = ProposalPDA::try_from_slice(&market_account.data.borrow())?;
    
    // Verify user owns position
    if position.user != *user_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Calculate funding payment
    let entry_funding_index = position.entry_funding_index.unwrap_or(U64F64::from_num(0));
    let funding_payment = calculate_position_funding(
        &position,
        &market.funding_state,
        entry_funding_index,
    )?;
    
    // Apply funding to position
    apply_funding_to_position(&mut position, funding_payment)?;
    
    // Update position's funding index
    position.entry_funding_index = Some(if position.is_long {
        market.funding_state.long_funding_index
    } else {
        market.funding_state.short_funding_index
    });
    
    // Serialize back
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    msg!("Position funding settled: payment={}, new_collateral={}", 
        funding_payment, position.collateral);
    
    Ok(())
}

use solana_program::account_info::next_account_info;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_funding_rate_during_halt() {
        let mut funding_state = FundingRateState::new(0);
        let recovery_state = RecoveryState::new();
        
        // Halt market
        funding_state.halt_market(100);
        
        // Check funding rate during halt
        let rate = funding_state.calculate_weighted_funding_rate(&recovery_state, 200);
        assert_eq!(rate, 125); // 1.25% per hour
    }
    
    #[test]
    fn test_funding_accumulation() {
        let mut funding_state = FundingRateState::new(0);
        let mut recovery_state = RecoveryState::new();
        recovery_state.is_active = true;
        recovery_state.funding_rate_boost = 125; // 1.25% per hour
        
        // Update after 1 hour
        funding_state.update_funding_indices(&recovery_state, SLOTS_PER_HOUR).unwrap();
        
        // Long funding index should increase by 1.25%
        let expected = U64F64::from_num(125) / U64F64::from_num(10000);
        assert_eq!(funding_state.long_funding_index, expected);
    }
}