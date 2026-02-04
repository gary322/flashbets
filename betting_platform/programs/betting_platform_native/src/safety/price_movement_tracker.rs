//! Price Movement Tracker for Circuit Breaker
//!
//! Production-grade implementation that tracks price movements over a 4-slot window
//! and triggers halt when movement exceeds 5% threshold

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
    state::ProposalPDA,
    state::security_accounts::CircuitBreakerType,
};

/// Price movement window (4 slots as per specification)
pub const PRICE_MOVEMENT_WINDOW: u64 = 4;

/// Price movement threshold (5% = 500 basis points)
pub const PRICE_MOVEMENT_THRESHOLD_BPS: u64 = 500;

/// Price history entry
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct PriceEntry {
    pub slot: u64,
    pub price: u64,
    pub outcome: u8,
}

/// Price movement tracker state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceMovementTracker {
    /// Discriminator
    pub discriminator: [u8; 8],
    
    /// Proposal ID being tracked
    pub proposal_id: u128,
    
    /// Rolling window of price entries (circular buffer)
    pub price_history: [PriceEntry; 5], // One extra for circular buffer
    pub history_index: u8,
    pub history_count: u8,
    
    /// Last halt trigger slot
    pub last_halt_slot: u64,
    
    /// Statistics
    pub total_halts_triggered: u32,
    pub last_movement_percentage: u64,
}

impl PriceMovementTracker {
    pub const DISCRIMINATOR: [u8; 8] = [99, 88, 77, 66, 55, 44, 33, 22];
    
    pub fn new(proposal_id: u128) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            proposal_id,
            price_history: [PriceEntry { slot: 0, price: 0, outcome: 0 }; 5],
            history_index: 0,
            history_count: 0,
            last_halt_slot: 0,
            total_halts_triggered: 0,
            last_movement_percentage: 0,
        }
    }
    
    /// Add new price and check for excessive movement
    pub fn add_price_and_check(
        &mut self,
        price: u64,
        outcome: u8,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Add new price entry
        let entry = PriceEntry {
            slot: current_slot,
            price,
            outcome,
        };
        
        self.price_history[self.history_index as usize] = entry;
        self.history_index = (self.history_index + 1) % 5;
        if self.history_count < 5 {
            self.history_count += 1;
        }
        
        // Check if we have enough history
        if self.history_count < 2 {
            return Ok(false);
        }
        
        // Find the oldest price within the 4-slot window
        let cutoff_slot = current_slot.saturating_sub(PRICE_MOVEMENT_WINDOW);
        let mut oldest_price: Option<u64> = None;
        let mut oldest_slot = current_slot;
        
        for i in 0..self.history_count {
            let idx = i as usize;
            let entry = &self.price_history[idx];
            
            if entry.outcome == outcome && entry.slot >= cutoff_slot && entry.slot < oldest_slot {
                oldest_slot = entry.slot;
                oldest_price = Some(entry.price);
            }
        }
        
        // If we don't have a price from within the window, no halt
        let Some(base_price) = oldest_price else {
            return Ok(false);
        };
        
        // Calculate price movement percentage
        let price_change = if price > base_price {
            price - base_price
        } else {
            base_price - price
        };
        
        let movement_bps = price_change
            .checked_mul(10_000)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(base_price)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        self.last_movement_percentage = movement_bps;
        
        // Check if movement exceeds threshold
        if movement_bps > PRICE_MOVEMENT_THRESHOLD_BPS {
            msg!(
                "Price movement halt triggered: {}bps movement over {} slots (threshold: {}bps)",
                movement_bps,
                current_slot - oldest_slot,
                PRICE_MOVEMENT_THRESHOLD_BPS
            );
            
            self.last_halt_slot = current_slot;
            self.total_halts_triggered += 1;
            
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Clear old entries outside the window
    pub fn cleanup_old_entries(&mut self, current_slot: u64) {
        let cutoff_slot = current_slot.saturating_sub(PRICE_MOVEMENT_WINDOW * 2);
        
        for i in 0..5 {
            if self.price_history[i].slot < cutoff_slot {
                self.price_history[i] = PriceEntry { slot: 0, price: 0, outcome: 0 };
            }
        }
    }
}

/// Process price movement check instruction
pub fn process_check_price_movement(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    proposal_id: u128,
    outcome: u8,
    new_price: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let tracker_account = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let circuit_breaker_account = next_account_info(account_info_iter)?;
    let clock_account = next_account_info(account_info_iter)?;
    
    // Verify PDA
    let (tracker_pda, _) = Pubkey::find_program_address(
        &[b"price_tracker", &proposal_id.to_le_bytes()],
        program_id,
    );
    
    if tracker_pda != *tracker_account.key {
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock_account)?;
    let current_slot = clock.slot;
    
    // Load or create tracker
    let mut tracker = if tracker_account.data_len() > 0 {
        PriceMovementTracker::try_from_slice(&tracker_account.data.borrow())?
    } else {
        PriceMovementTracker::new(proposal_id)
    };
    
    // Verify proposal ID matches
    if tracker.proposal_id != proposal_id {
        return Err(BettingPlatformError::InvalidProposal.into());
    }
    
    // Add price and check for excessive movement
    let should_halt = tracker.add_price_and_check(new_price, outcome, current_slot)?;
    
    // Clean up old entries
    tracker.cleanup_old_entries(current_slot);
    
    // Save tracker state
    tracker.serialize(&mut &mut tracker_account.data.borrow_mut()[..])?;
    
    // If halt triggered, update circuit breaker
    if should_halt {
        let mut circuit_breaker = crate::state::security_accounts::CircuitBreaker::try_from_slice(
            &circuit_breaker_account.data.borrow()
        )?;
        
        circuit_breaker.price_breaker_active = true;
        circuit_breaker.price_activated_at = Some(clock.unix_timestamp);
        circuit_breaker.total_triggers += 1;
        circuit_breaker.is_active = true;
        circuit_breaker.breaker_type = Some(CircuitBreakerType::Price);
        circuit_breaker.triggered_at = Some(current_slot);
        circuit_breaker.triggered_by = Some(*program_id);
        
        circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        
        msg!("Circuit breaker activated due to excessive price movement");
        return Err(BettingPlatformError::CircuitBreakerTriggered.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_price_movement_detection() {
        let mut tracker = PriceMovementTracker::new(12345);
        
        // Add initial price
        assert!(!tracker.add_price_and_check(10000, 0, 100).unwrap());
        
        // Small movement within threshold (3%)
        assert!(!tracker.add_price_and_check(10300, 0, 101).unwrap());
        
        // Movement exceeding 5% threshold within 4 slots
        assert!(tracker.add_price_and_check(10600, 0, 103).unwrap());
        assert_eq!(tracker.last_movement_percentage, 600); // 6%
        
        // Reset and test downward movement
        let mut tracker = PriceMovementTracker::new(12345);
        assert!(!tracker.add_price_and_check(10000, 0, 200).unwrap());
        assert!(tracker.add_price_and_check(9400, 0, 203).unwrap());
        assert_eq!(tracker.last_movement_percentage, 600); // 6%
    }
    
    #[test]
    fn test_window_constraint() {
        let mut tracker = PriceMovementTracker::new(12345);
        
        // Add initial price
        assert!(!tracker.add_price_and_check(10000, 0, 100).unwrap());
        
        // Add price after window expires (no halt should trigger)
        assert!(!tracker.add_price_and_check(20000, 0, 105).unwrap());
        
        // But within new window, should trigger
        assert!(!tracker.add_price_and_check(20000, 0, 106).unwrap());
        assert!(tracker.add_price_and_check(21100, 0, 108).unwrap());
    }
}