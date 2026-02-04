//! Credit Locking Mechanism
//!
//! Implements per-position credit locking to ensure proper collateralization

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    state::{Position, ProposalPDA},
    credits::credits_manager::{UserCredits, derive_user_credits_pda},
};

/// Credit lock information for a position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CreditLock {
    /// Position ID this lock belongs to
    pub position_id: [u8; 32],
    
    /// User who owns the position
    pub user: Pubkey,
    
    /// Proposal ID
    pub proposal_id: u128,
    
    /// Amount of credits locked
    pub locked_amount: u64,
    
    /// Timestamp when locked
    pub locked_at: i64,
    
    /// Is this lock active
    pub is_active: bool,
}

impl CreditLock {
    /// Create new credit lock
    pub fn new(
        position_id: [u8; 32],
        user: Pubkey,
        proposal_id: u128,
        amount: u64,
    ) -> Self {
        let clock = Clock::get().unwrap_or_default();
        
        Self {
            position_id,
            user,
            proposal_id,
            locked_amount: amount,
            locked_at: clock.unix_timestamp,
            is_active: true,
        }
    }
}

/// Credit locking manager
pub struct CreditLockingManager;

impl CreditLockingManager {
    /// Lock credits for a new position
    pub fn lock_credits_for_position<'a>(
        user_credits_account: &AccountInfo<'a>,
        position: &Position,
        required_margin: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        msg!("Locking {} credits for position", required_margin);
        
        // Deserialize user credits
        let mut user_credits = UserCredits::try_from_slice(&user_credits_account.data.borrow())?;
        user_credits.validate()?;
        
        // Verify account ownership
        let (expected_pda, _) = derive_user_credits_pda(
            program_id,
            &position.user,
            position.verse_id,
        );
        if user_credits_account.key != &expected_pda {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Lock the credits
        user_credits.lock_credits(required_margin)?;
        
        // Save updated credits
        user_credits.serialize(&mut &mut user_credits_account.data.borrow_mut()[..])?;
        
        msg!("Successfully locked {} credits, {} available", 
            required_margin, 
            user_credits.available_credits
        );
        
        Ok(())
    }
    
    /// Release credits when position closes
    pub fn release_credits_from_position<'a>(
        user_credits_account: &AccountInfo<'a>,
        position: &Position,
        amount_to_release: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {
        msg!("Releasing {} credits from position", amount_to_release);
        
        // Deserialize user credits
        let mut user_credits = UserCredits::try_from_slice(&user_credits_account.data.borrow())?;
        user_credits.validate()?;
        
        // Verify account ownership
        let (expected_pda, _) = derive_user_credits_pda(
            program_id,
            &position.user,
            position.verse_id,
        );
        if user_credits_account.key != &expected_pda {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Release the credits
        user_credits.release_credits(amount_to_release)?;
        
        // Save updated credits
        user_credits.serialize(&mut &mut user_credits_account.data.borrow_mut()[..])?;
        
        msg!("Successfully released {} credits, {} available", 
            amount_to_release, 
            user_credits.available_credits
        );
        
        Ok(())
    }
    
    /// Check if user has sufficient credits for position
    pub fn check_credit_availability(
        user_credits: &UserCredits,
        required_margin: u64,
        max_positions: u8,
    ) -> Result<bool, ProgramError> {
        // Check available credits
        if user_credits.available_credits < required_margin {
            msg!("Insufficient credits: {} available, {} required", 
                user_credits.available_credits, 
                required_margin
            );
            return Ok(false);
        }
        
        // Check position limit
        if user_credits.active_positions >= max_positions as u32 {
            msg!("Position limit reached: {} active, {} max", 
                user_credits.active_positions, 
                max_positions
            );
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Calculate required margin for position
    pub fn calculate_required_margin(
        position_size: u64,
        leverage: u8,
        proposal: &ProposalPDA,
    ) -> Result<u64, ProgramError> {
        if leverage == 0 || leverage > 100 {
            return Err(BettingPlatformError::InvalidLeverageTier.into());
        }
        
        // Base margin = size / leverage
        let base_margin = position_size / leverage as u64;
        
        // Add safety buffer for volatile markets
        let volatility_buffer = if proposal.outcomes > 2 {
            // Multi-outcome markets need more margin
            base_margin / 10 // 10% extra
        } else {
            0
        };
        
        Ok(base_margin.saturating_add(volatility_buffer))
    }
    
    /// Handle conflicting positions (same proposal, different outcomes)
    pub fn handle_conflicting_positions(
        user_credits: &UserCredits,
        new_position: &Position,
        existing_positions: &[Position],
    ) -> Result<ConflictResolution, ProgramError> {
        let mut conflicts = Vec::new();
        let mut total_locked_same_proposal = 0u64;
        
        for existing in existing_positions {
            if existing.proposal_id == new_position.proposal_id && !existing.is_closed {
                // Found conflicting position
                conflicts.push(ConflictInfo {
                    position_id: existing.position_id,
                    outcome: existing.outcome,
                    locked_amount: existing.margin,
                    is_opposite: existing.outcome != new_position.outcome,
                });
                
                total_locked_same_proposal = total_locked_same_proposal
                    .saturating_add(existing.margin);
            }
        }
        
        // Credits are shared across all positions in the same proposal
        // This is the quantum superposition aspect
        let can_proceed = if conflicts.is_empty() {
            true
        } else {
            // Check if new position's margin fits within total deposit
            let new_total = total_locked_same_proposal.saturating_add(new_position.margin);
            new_total <= user_credits.total_deposit
        };
        
        Ok(ConflictResolution {
            has_conflicts: !conflicts.is_empty(),
            conflicts,
            can_proceed,
            total_locked_in_proposal: total_locked_same_proposal,
            available_for_proposal: user_credits.total_deposit.saturating_sub(total_locked_same_proposal),
        })
    }
}

/// Information about conflicting positions
#[derive(Debug)]
pub struct ConflictInfo {
    pub position_id: [u8; 32],
    pub outcome: u8,
    pub locked_amount: u64,
    pub is_opposite: bool,
}

/// Result of conflict resolution
#[derive(Debug)]
pub struct ConflictResolution {
    pub has_conflicts: bool,
    pub conflicts: Vec<ConflictInfo>,
    pub can_proceed: bool,
    pub total_locked_in_proposal: u64,
    pub available_for_proposal: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::accounts::discriminators;
    
    #[test]
    fn test_calculate_required_margin() {
        let mut proposal = ProposalPDA::new([0; 32], [0; 32], 2);
        
        // Test basic margin calculation
        let margin = CreditLockingManager::calculate_required_margin(1000, 10, &proposal).unwrap();
        assert_eq!(margin, 100); // 1000/10 = 100
        
        // Test with multi-outcome (adds 10% buffer)
        proposal.outcomes = 5;
        let margin = CreditLockingManager::calculate_required_margin(1000, 10, &proposal).unwrap();
        assert_eq!(margin, 110); // 100 + 10% = 110
    }
    
    #[test]
    fn test_conflicting_positions() {
        let user = Pubkey::new_unique();
        let user_credits = UserCredits::new(user, 1, 1000, 255);
        
        // Create existing position
        let existing = Position::new(
            user,
            1,
            1,
            0, // outcome 0
            500,
            5,
            50000,
            true,
            0,
        );
        
        // Create new position on same proposal, different outcome
        let new_position = Position::new(
            user,
            1, // same proposal
            1,
            1, // different outcome
            300,
            5,
            50000,
            true,
            0,
        );
        
        let resolution = CreditLockingManager::handle_conflicting_positions(
            &user_credits,
            &new_position,
            &[existing],
        ).unwrap();
        
        assert!(resolution.has_conflicts);
        assert_eq!(resolution.conflicts.len(), 1);
        assert!(resolution.conflicts[0].is_opposite);
        assert_eq!(resolution.total_locked_in_proposal, 100); // 500/5 = 100 margin
        assert!(resolution.can_proceed); // 100 + 60 = 160 < 1000 total
    }
}