//! Refund Processor Implementation
//!
//! Handles instant refunds at settle_slot for unused credits

use crate::state::ProposalPDA;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    program::invoke,
    system_instruction,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    state::{ VersePDA, ProposalState},
    credits::credits_manager::{UserCredits, derive_user_credits_pda},
    events::{Event, RefundProcessed},
    account_validation::{validate_signer, validate_writable},
};

/// Refund processor for handling credit refunds
pub struct RefundProcessor;

impl RefundProcessor {
    /// Process refund at settle_slot
    pub fn process_refund_at_settle_slot(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        proposal_id: u128,
    ) -> ProgramResult {
        msg!("Processing refund at settle_slot for proposal {}", proposal_id);
        
        // Parse accounts
        let account_info_iter = &mut accounts.iter();
        let user = next_account_info(account_info_iter)?;
        let user_credits_account = next_account_info(account_info_iter)?;
        let proposal_account = next_account_info(account_info_iter)?;
        let verse_account = next_account_info(account_info_iter)?;
        let vault_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        
        // Validate accounts
        validate_signer(user)?;
        validate_writable(user_credits_account)?;
        validate_writable(vault_account)?;
        
        // Load and validate proposal
        let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
        proposal.validate()?;
        
        // Load verse
        let verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
        verse.validate()?;
        
        // Check if we're at or past settle_slot
        let clock = Clock::get()?;
        if clock.slot < proposal.settle_slot {
            msg!("Not yet at settle_slot: {} < {}", clock.slot, proposal.settle_slot);
            return Err(BettingPlatformError::TooEarlyForRefund.into());
        }
        
        // Load user credits
        let mut user_credits = UserCredits::try_from_slice(&user_credits_account.data.borrow())?;
        user_credits.validate()?;
        
        // Verify credits account ownership
        let (expected_pda, _) = derive_user_credits_pda(
            program_id,
            user.key,
            verse.verse_id,
        );
        if user_credits_account.key != &expected_pda {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Process refund based on proposal state
        let refund_amount = match proposal.state {
            ProposalState::Resolved => {
                // Proposal resolved - refund available credits
                Self::calculate_resolved_refund(&user_credits)?
            },
            ProposalState::Active => {
                // Still active at settle_slot - mark for refund
                user_credits.mark_refund_eligible();
                user_credits.available_credits
            },
            ProposalState::Paused => {
                // Paused proposals can be refunded
                user_credits.available_credits
            },
        };
        
        if refund_amount == 0 {
            msg!("No credits available for refund");
            return Err(BettingPlatformError::NoCreditsToRefund.into());
        }
        
        // Process the refund
        let actual_refund = user_credits.process_refund()?;
        if actual_refund != refund_amount {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Transfer funds from vault to user
        **vault_account.lamports.borrow_mut() = vault_account
            .lamports()
            .checked_sub(refund_amount)
            .ok_or(ProgramError::InsufficientFunds)?;
        
        **user.lamports.borrow_mut() = user
            .lamports()
            .checked_add(refund_amount)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        // Save updated credits
        user_credits.serialize(&mut &mut user_credits_account.data.borrow_mut()[..])?;
        
        // Emit refund event
        RefundProcessed {
            user: *user.key,
            proposal_id,
            verse_id: verse.verse_id,
            refund_amount,
            timestamp: clock.unix_timestamp,
            refund_type: RefundType::SettleSlot as u8,
        }.emit();
        
        msg!("Successfully refunded {} credits to user", refund_amount);
        Ok(())
    }
    
    /// Process emergency refund (circuit breaker triggered)
    pub fn process_emergency_refund(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        verse_id: u128,
    ) -> ProgramResult {
        msg!("Processing emergency refund for verse {}", verse_id);
        
        // Parse accounts
        let account_info_iter = &mut accounts.iter();
        let user = next_account_info(account_info_iter)?;
        let user_credits_account = next_account_info(account_info_iter)?;
        let verse_account = next_account_info(account_info_iter)?;
        let vault_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        
        // Validate accounts
        validate_signer(user)?;
        validate_writable(user_credits_account)?;
        validate_writable(vault_account)?;
        
        // Load verse
        let verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
        verse.validate()?;
        
        // Verify verse is halted (circuit breaker)
        if verse.status != crate::state::VerseStatus::Halted {
            return Err(BettingPlatformError::VerseNotHalted.into());
        }
        
        // Load user credits
        let mut user_credits = UserCredits::try_from_slice(&user_credits_account.data.borrow())?;
        user_credits.validate()?;
        
        // Verify credits account ownership
        let (expected_pda, _) = derive_user_credits_pda(
            program_id,
            user.key,
            verse_id,
        );
        if user_credits_account.key != &expected_pda {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // For emergency refunds, return all available credits immediately
        let refund_amount = user_credits.available_credits;
        
        if refund_amount == 0 {
            return Err(BettingPlatformError::NoCreditsToRefund.into());
        }
        
        // Mark as eligible and process
        user_credits.mark_refund_eligible();
        let actual_refund = user_credits.process_refund()?;
        
        // Transfer funds
        **vault_account.lamports.borrow_mut() = vault_account
            .lamports()
            .checked_sub(actual_refund)
            .ok_or(ProgramError::InsufficientFunds)?;
        
        **user.lamports.borrow_mut() = user
            .lamports()
            .checked_add(actual_refund)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        // Save updated credits
        user_credits.serialize(&mut &mut user_credits_account.data.borrow_mut()[..])?;
        
        // Emit event
        let clock = Clock::get()?;
        RefundProcessed {
            user: *user.key,
            proposal_id: 0, // No specific proposal for verse-level refund
            verse_id,
            refund_amount: actual_refund,
            timestamp: clock.unix_timestamp,
            refund_type: RefundType::Emergency as u8,
        }.emit();
        
        msg!("Emergency refund processed: {} credits", actual_refund);
        Ok(())
    }
    
    /// Calculate refund amount for resolved proposal
    fn calculate_resolved_refund(user_credits: &UserCredits) -> Result<u64, ProgramError> {
        // For resolved proposals, only refund if no active positions
        if user_credits.active_positions > 0 {
            return Ok(0);
        }
        
        Ok(user_credits.available_credits)
    }
    
    /// Batch process refunds for multiple users
    pub fn batch_process_refunds(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        proposal_id: u128,
        user_count: u8,
    ) -> ProgramResult {
        msg!("Batch processing {} refunds for proposal {}", user_count, proposal_id);
        
        let account_info_iter = &mut accounts.iter();
        
        // Common accounts
        let proposal_account = next_account_info(account_info_iter)?;
        let verse_account = next_account_info(account_info_iter)?;
        let vault_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        
        // Load proposal once
        let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
        proposal.validate()?;
        
        // Check settle_slot
        let clock = Clock::get()?;
        if clock.slot < proposal.settle_slot {
            return Err(BettingPlatformError::TooEarlyForRefund.into());
        }
        
        let mut total_refunded = 0u64;
        let mut users_refunded = 0u8;
        
        // Process each user
        for i in 0..user_count {
            let user = next_account_info(account_info_iter)?;
            let user_credits_account = next_account_info(account_info_iter)?;
            
            // Try to process refund, continue on error
            match Self::process_single_refund(
                program_id,
                user,
                user_credits_account,
                vault_account,
                proposal_id,
                &proposal,
            ) {
                Ok(amount) => {
                    total_refunded = total_refunded.saturating_add(amount);
                    users_refunded += 1;
                },
                Err(e) => {
                    msg!("Failed to refund user {}: {:?}", i, e);
                    // Continue processing other users
                }
            }
        }
        
        msg!("Batch refund complete: {} users, {} total", users_refunded, total_refunded);
        Ok(())
    }
    
    /// Process single user refund (helper for batch)
    fn process_single_refund<'a>(
        program_id: &Pubkey,
        user: &AccountInfo<'a>,
        user_credits_account: &AccountInfo<'a>,
        vault_account: &AccountInfo<'a>,
        proposal_id: u128,
        proposal: &ProposalPDA,
    ) -> Result<u64, ProgramError> {
        // Load user credits
        let mut user_credits = UserCredits::try_from_slice(&user_credits_account.data.borrow())?;
        user_credits.validate()?;
        
        // Calculate refund
        let refund_amount = user_credits.available_credits;
        if refund_amount == 0 {
            return Ok(0);
        }
        
        // Process refund
        user_credits.mark_refund_eligible();
        let actual_refund = user_credits.process_refund()?;
        
        // Transfer funds
        **vault_account.lamports.borrow_mut() = vault_account
            .lamports()
            .checked_sub(actual_refund)
            .ok_or(ProgramError::InsufficientFunds)?;
        
        **user.lamports.borrow_mut() = user
            .lamports()
            .checked_add(actual_refund)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        // Save credits
        user_credits.serialize(&mut &mut user_credits_account.data.borrow_mut()[..])?;
        
        Ok(actual_refund)
    }
}

/// Refund type for events
#[repr(u8)]
pub enum RefundType {
    SettleSlot = 0,
    Emergency = 1,
    ProposalCancelled = 2,
    PartialLiquidation = 3,
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_refund_types() {
        assert_eq!(RefundType::SettleSlot as u8, 0);
        assert_eq!(RefundType::Emergency as u8, 1);
        assert_eq!(RefundType::ProposalCancelled as u8, 2);
        assert_eq!(RefundType::PartialLiquidation as u8, 3);
    }
}