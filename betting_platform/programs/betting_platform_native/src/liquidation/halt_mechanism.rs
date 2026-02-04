//! Liquidation Halt Mechanism
//!
//! Implements 1-hour halt after significant liquidation events
//! to prevent cascading liquidations and allow market recovery

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
    state::GlobalConfigPDA,
    events::{emit_event, EventType, LiquidationHaltEvent},
    account_validation::DISCRIMINATOR_SIZE,
};

/// Halt duration in slots (1 hour = 9000 slots at 400ms/slot)
pub const LIQUIDATION_HALT_DURATION: u64 = 9000;

/// Thresholds for triggering halt
pub const LIQUIDATION_COUNT_THRESHOLD: u32 = 10; // More than 10 liquidations
pub const LIQUIDATION_VALUE_THRESHOLD: u64 = 100_000_000_000; // $100k total
pub const COVERAGE_RATIO_THRESHOLD: u64 = 5000; // Below 50% coverage

/// Liquidation halt state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LiquidationHaltState {
    /// Discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Is halt active
    pub is_halted: bool,
    
    /// Halt started at slot
    pub halt_start_slot: u64,
    
    /// Halt end slot
    pub halt_end_slot: u64,
    
    /// Reason for halt
    pub halt_reason: HaltReason,
    
    /// Liquidations in current window
    pub window_liquidation_count: u32,
    
    /// Total value liquidated in window
    pub window_liquidation_value: u64,
    
    /// Window start slot
    pub window_start_slot: u64,
    
    /// Last liquidation slot
    pub last_liquidation_slot: u64,
    
    /// Coverage ratio at halt
    pub coverage_at_halt: u64,
    
    /// Authority that can override halt
    pub override_authority: Pubkey,
    
    /// Halt history
    pub halt_count: u32,
    
    /// Last halt timestamp
    pub last_halt_timestamp: i64,
}

impl LiquidationHaltState {
    pub const DISCRIMINATOR: [u8; 8] = [76, 73, 81, 72, 65, 76, 84, 83]; // "LIQHALTS"
    
    pub fn new(override_authority: Pubkey) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_halted: false,
            halt_start_slot: 0,
            halt_end_slot: 0,
            halt_reason: HaltReason::None,
            window_liquidation_count: 0,
            window_liquidation_value: 0,
            window_start_slot: 0,
            last_liquidation_slot: 0,
            coverage_at_halt: 10000, // 100%
            override_authority,
            halt_count: 0,
            last_halt_timestamp: 0,
        }
    }
    
    /// Check if halt should be triggered
    pub fn should_halt(
        &self,
        coverage_ratio: u64,
        current_slot: u64,
    ) -> bool {
        // Already halted
        if self.is_halted && current_slot < self.halt_end_slot {
            return true;
        }
        
        // Check liquidation count threshold
        if self.window_liquidation_count > LIQUIDATION_COUNT_THRESHOLD {
            return true;
        }
        
        // Check liquidation value threshold
        if self.window_liquidation_value > LIQUIDATION_VALUE_THRESHOLD {
            return true;
        }
        
        // Check coverage ratio threshold
        if coverage_ratio < COVERAGE_RATIO_THRESHOLD {
            return true;
        }
        
        false
    }
    
    /// Trigger halt
    pub fn trigger_halt(
        &mut self,
        reason: HaltReason,
        current_slot: u64,
        current_timestamp: i64,
        coverage_ratio: u64,
    ) {
        self.is_halted = true;
        self.halt_start_slot = current_slot;
        self.halt_end_slot = current_slot + LIQUIDATION_HALT_DURATION;
        self.halt_reason = reason;
        self.coverage_at_halt = coverage_ratio;
        self.halt_count += 1;
        self.last_halt_timestamp = current_timestamp;
        
        msg!(
            "Liquidation halt triggered: {:?} until slot {}",
            reason,
            self.halt_end_slot
        );
    }
    
    /// Check if halt expired
    pub fn check_halt_expired(&mut self, current_slot: u64) -> bool {
        if self.is_halted && current_slot >= self.halt_end_slot {
            self.is_halted = false;
            self.halt_reason = HaltReason::None;
            
            // Reset window counters
            self.window_liquidation_count = 0;
            self.window_liquidation_value = 0;
            self.window_start_slot = current_slot;
            
            msg!("Liquidation halt expired at slot {}", current_slot);
            return true;
        }
        false
    }
    
    /// Record liquidation for halt tracking
    pub fn record_liquidation(
        &mut self,
        liquidation_value: u64,
        current_slot: u64,
    ) {
        // Reset window if too old (1 hour)
        if current_slot > self.window_start_slot + LIQUIDATION_HALT_DURATION {
            self.window_liquidation_count = 0;
            self.window_liquidation_value = 0;
            self.window_start_slot = current_slot;
        }
        
        self.window_liquidation_count += 1;
        self.window_liquidation_value += liquidation_value;
        self.last_liquidation_slot = current_slot;
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum HaltReason {
    None,
    TooManyLiquidations,
    ExcessiveLiquidationValue,
    LowCoverageRatio,
    ManualHalt,
    CircuitBreaker,
}

/// Process liquidation with halt check
pub fn process_liquidation_with_halt_check(
    halt_state: &mut LiquidationHaltState,
    liquidation_value: u64,
    current_slot: u64,
) -> ProgramResult {
    // Check if currently halted
    if halt_state.is_halted && current_slot < halt_state.halt_end_slot {
        return Err(BettingPlatformError::LiquidationHalted.into());
    }
    
    // Check if halt expired
    halt_state.check_halt_expired(current_slot);
    
    // Record this liquidation
    halt_state.record_liquidation(liquidation_value, current_slot);
    
    // For now, use a default coverage ratio
    // In production, this would be passed from the caller
    let coverage_ratio = 10000; // 100% default
    
    // Check if halt should be triggered
    if halt_state.should_halt(coverage_ratio, current_slot) {
        // Determine halt reason
        let reason = if halt_state.window_liquidation_count > LIQUIDATION_COUNT_THRESHOLD {
            HaltReason::TooManyLiquidations
        } else if halt_state.window_liquidation_value > LIQUIDATION_VALUE_THRESHOLD {
            HaltReason::ExcessiveLiquidationValue
        } else if coverage_ratio < COVERAGE_RATIO_THRESHOLD {
            HaltReason::LowCoverageRatio
        } else {
            HaltReason::CircuitBreaker
        };
        
        // Get current timestamp
        let current_timestamp = Clock::get()?.unix_timestamp;
        
        // Trigger halt
        halt_state.trigger_halt(
            reason,
            current_slot,
            current_timestamp,
            coverage_ratio,
        );
        
        // Emit halt event
        emit_event(
            EventType::LiquidationHalt,
            &LiquidationHaltEvent {
                reason: format!("{:?}", reason),
                halt_start_slot: halt_state.halt_start_slot,
                halt_end_slot: halt_state.halt_end_slot,
                liquidation_count: halt_state.window_liquidation_count,
                liquidation_value: halt_state.window_liquidation_value,
                coverage_ratio,
                timestamp: current_timestamp,
            },
        );
        
        // Prevent the current liquidation
        return Err(BettingPlatformError::LiquidationHalted.into());
    }
    
    Ok(())
}

/// Manual halt override by authority
pub fn process_override_halt(
    accounts: &[AccountInfo],
    force_resume: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let halt_state_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load halt state
    let mut halt_data = halt_state_account.try_borrow_mut_data()?;
    let mut halt_state = LiquidationHaltState::try_from_slice(&halt_data)?;
    
    if halt_state.override_authority != *authority_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    if force_resume {
        // Force resume
        halt_state.is_halted = false;
        halt_state.halt_reason = HaltReason::None;
        halt_state.window_liquidation_count = 0;
        halt_state.window_liquidation_value = 0;
        halt_state.window_start_slot = clock.slot;
        
        msg!("Liquidation halt manually resumed by authority");
    } else {
        // Force halt
        halt_state.trigger_halt(
            HaltReason::ManualHalt,
            clock.slot,
            clock.unix_timestamp,
            0, // Coverage not relevant for manual halt
        );
        
        msg!("Liquidation halt manually triggered by authority");
    }
    
    // Save state
    halt_state.serialize(&mut *halt_data)?;
    
    Ok(())
}

/// Initialize halt state account
pub fn process_initialize_halt_state(
    accounts: &[AccountInfo],
    override_authority: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let halt_state_account = next_account_info(account_info_iter)?;
    let payer_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Create account if needed
    if halt_state_account.data_is_empty() {
        let rent = solana_program::rent::Rent::get()?;
        let space = std::mem::size_of::<LiquidationHaltState>();
        let lamports = rent.minimum_balance(space);
        
        solana_program::program::invoke(
            &solana_program::system_instruction::create_account(
                payer_account.key,
                halt_state_account.key,
                lamports,
                space as u64,
                &crate::id(),
            ),
            &[
                payer_account.clone(),
                halt_state_account.clone(),
                system_program.clone(),
            ],
        )?;
    }
    
    // Initialize state
    let halt_state = LiquidationHaltState::new(override_authority);
    let mut halt_data = halt_state_account.try_borrow_mut_data()?;
    halt_state.serialize(&mut *halt_data)?;
    
    msg!("Liquidation halt state initialized");
    
    Ok(())
}

/// Calculate coverage ratio
fn calculate_coverage_ratio(vault: u128, total_oi: u128) -> u64 {
    if total_oi == 0 {
        return 10000; // 100%
    }
    
    // Coverage = vault / (0.5 * total_oi)
    let half_oi = total_oi / 2;
    if half_oi == 0 {
        return 10000;
    }
    
    ((vault * 10000) / half_oi) as u64
}

/// Check if liquidations are currently halted
pub fn check_halt_status(halt_state: &LiquidationHaltState) -> Result<bool, ProgramError> {
    if !halt_state.is_halted {
        return Ok(false);
    }
    
    let current_slot = Clock::get()?.slot;
    
    // Check if halt period has expired
    if current_slot >= halt_state.halt_end_slot {
        // Halt period expired, liquidations can resume
        Ok(false)
    } else {
        // Still within halt period
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_halt_triggers() {
        let mut halt_state = LiquidationHaltState::new(Pubkey::new_unique());
        
        // Test liquidation count trigger
        halt_state.window_liquidation_count = 11;
        assert!(halt_state.should_halt(10000, 1000));
        
        // Test liquidation value trigger
        halt_state.window_liquidation_count = 5;
        halt_state.window_liquidation_value = 150_000_000_000;
        assert!(halt_state.should_halt(10000, 1000));
        
        // Test coverage ratio trigger
        halt_state.window_liquidation_count = 5;
        halt_state.window_liquidation_value = 50_000_000_000;
        assert!(halt_state.should_halt(4000, 1000)); // 40% coverage
    }
    
    #[test]
    fn test_halt_expiration() {
        let mut halt_state = LiquidationHaltState::new(Pubkey::new_unique());
        
        // Trigger halt
        halt_state.trigger_halt(
            HaltReason::TooManyLiquidations,
            1000,
            1234567890,
            5000,
        );
        
        assert!(halt_state.is_halted);
        assert_eq!(halt_state.halt_end_slot, 1000 + LIQUIDATION_HALT_DURATION);
        
        // Check before expiration
        assert!(!halt_state.check_halt_expired(5000));
        assert!(halt_state.is_halted);
        
        // Check after expiration
        assert!(halt_state.check_halt_expired(11000));
        assert!(!halt_state.is_halted);
        assert_eq!(halt_state.window_liquidation_count, 0);
    }
    
    #[test]
    fn test_window_reset() {
        let mut halt_state = LiquidationHaltState::new(Pubkey::new_unique());
        
        // Record liquidations
        halt_state.record_liquidation(10_000_000_000, 1000);
        halt_state.record_liquidation(20_000_000_000, 1500);
        
        assert_eq!(halt_state.window_liquidation_count, 2);
        assert_eq!(halt_state.window_liquidation_value, 30_000_000_000);
        
        // Record after window expires
        halt_state.record_liquidation(5_000_000_000, 12000);
        
        assert_eq!(halt_state.window_liquidation_count, 1);
        assert_eq!(halt_state.window_liquidation_value, 5_000_000_000);
    }
}