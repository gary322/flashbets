//! Dispute handling for market resolution
//!
//! Manages disputes, arbitration, and resolution overrides

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::resolution_accounts::{
        ResolutionState, ResolutionStatus, DisputeState, DisputeReason,
        DisputeStatus, DisputeResolution, ArbitrationVote,
        discriminators as res_discriminators,
    },
};

pub mod initiate {
    use super::*;
    
    /// Initiate a dispute for a proposed resolution
    pub fn process_initiate_dispute(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        verse_id: u128,
        market_id: u128,
    ) -> ProgramResult {
        msg!("Initiating dispute for market {}", market_id);
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let disputer = next_account_info(account_info_iter)?;
        let resolution_account = next_account_info(account_info_iter)?;
        let dispute_account = next_account_info(account_info_iter)?;
        let dispute_bond_source = next_account_info(account_info_iter)?;
        let dispute_bond_vault = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = next_account_info(account_info_iter)?;
        let clock = next_account_info(account_info_iter)?;
        
        // Verify disputer is signer
        if !disputer.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Load resolution state
        let mut resolution_state = ResolutionState::try_from_slice(&resolution_account.data.borrow())?;
        resolution_state.validate()?;
        
        // Verify market matches
        if resolution_state.market_id != market_id {
            return Err(BettingPlatformError::InvalidAccountData.into());
        }
        
        // Verify resolution is in disputeable state
        match resolution_state.status {
            ResolutionStatus::Proposed | ResolutionStatus::Confirmed => {
                // Can dispute
            }
            _ => {
                msg!("Cannot dispute resolution in status: {:?}", resolution_state.status);
                return Err(BettingPlatformError::InvalidOperation.into());
            }
        }
        
        // Verify within dispute window
        let clock = Clock::from_account_info(clock)?;
        let current_time = clock.unix_timestamp;
        
        if let Some(dispute_end) = resolution_state.dispute_window_end {
            if current_time > dispute_end {
                msg!("Dispute window has closed");
                return Err(BettingPlatformError::DisputeWindowClosed.into());
            }
        }
        
        // Get proposed outcome
        let disputed_outcome = resolution_state.proposed_outcome
            .ok_or(BettingPlatformError::InvalidResolution)?;
        
        // Derive dispute PDA
        let (dispute_pda, bump_seed) = Pubkey::find_program_address(
            &[
                b"dispute",
                &market_id.to_le_bytes(),
                disputer.key.as_ref(),
            ],
            program_id,
        );
        
        // Verify PDA matches
        if dispute_pda != *dispute_account.key {
            msg!("Invalid dispute PDA");
            return Err(ProgramError::InvalidSeeds);
        }
        
        // Check if dispute already exists
        if dispute_account.data_len() > 0 {
            msg!("Dispute already exists for this user and market");
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        
        // Transfer dispute bond (simplified - in production use SPL token)
        let dispute_bond = 1_000_000_000; // 1 SOL dispute bond
        msg!("Transferring dispute bond: {} lamports", dispute_bond);
        
        // Calculate required space
        let dispute_size = DisputeState::space(5); // Space for 5 arbitrators
        
        // Create dispute account
        let rent_lamports = Rent::from_account_info(rent)?
            .minimum_balance(dispute_size);
        
        invoke_signed(
            &system_instruction::create_account(
                disputer.key,
                dispute_account.key,
                rent_lamports,
                dispute_size as u64,
                program_id,
            ),
            &[
                disputer.clone(),
                dispute_account.clone(),
                system_program.clone(),
            ],
            &[&[b"dispute", &market_id.to_le_bytes(), disputer.key.as_ref(), &[bump_seed]]],
        )?;
        
        // Create dispute state
        let dispute_state = DisputeState::new(
            market_id,
            *disputer.key,
            dispute_bond,
            disputed_outcome,
            0, // Proposed outcome to be provided in separate instruction
            DisputeReason::IncorrectOutcome, // Default reason
            current_time,
        );
        
        // Update resolution status
        resolution_state.status = ResolutionStatus::Disputed;
        
        // Log dispute
        msg!("Dispute initiated:");
        msg!("  Market: {}", market_id);
        msg!("  Disputer: {}", disputer.key);
        msg!("  Disputed outcome: {}", disputed_outcome);
        msg!("  Bond: {} lamports", dispute_bond);
        
        // Serialize and save
        dispute_state.serialize(&mut &mut dispute_account.data.borrow_mut()[..])?;
        resolution_state.serialize(&mut &mut resolution_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

pub mod resolve {
    use super::*;
    
    /// Resolve a dispute through arbitration
    pub fn process_resolve_dispute(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        verse_id: u128,
        market_id: u128,
        final_resolution: u8,
    ) -> ProgramResult {
        msg!("Resolving dispute for market {} with outcome {}", market_id, final_resolution);
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let arbitrator = next_account_info(account_info_iter)?;
        let resolution_account = next_account_info(account_info_iter)?;
        let dispute_account = next_account_info(account_info_iter)?;
        let disputer = next_account_info(account_info_iter)?;
        let dispute_bond_vault = next_account_info(account_info_iter)?;
        let disputer_refund = next_account_info(account_info_iter)?;
        let clock = next_account_info(account_info_iter)?;
        
        // Verify arbitrator is signer (in production, verify it's an authorized arbitrator)
        if !arbitrator.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Load accounts
        let mut resolution_state = ResolutionState::try_from_slice(&resolution_account.data.borrow())?;
        let mut dispute_state = DisputeState::try_from_slice(&dispute_account.data.borrow())?;
        
        // Validate accounts
        resolution_state.validate()?;
        dispute_state.validate()?;
        
        // Verify market matches
        if resolution_state.market_id != market_id || dispute_state.market_id != market_id {
            return Err(BettingPlatformError::InvalidAccountData.into());
        }
        
        // Verify dispute is active
        if dispute_state.status != DisputeStatus::Active && 
           dispute_state.status != DisputeStatus::UnderReview {
            msg!("Dispute is not active");
            return Err(BettingPlatformError::InvalidOperation.into());
        }
        
        // Add arbitrator vote
        let vote = ArbitrationVote {
            arbitrator: *arbitrator.key,
            vote: if final_resolution == dispute_state.disputed_outcome {
                DisputeResolution::Upheld
            } else if final_resolution == dispute_state.proposed_outcome {
                DisputeResolution::Overturned
            } else {
                DisputeResolution::Invalid
            },
            reasoning_cid: None,
            timestamp: Clock::from_account_info(clock)?.unix_timestamp,
        };
        
        // Check if arbitrator already voted
        let already_voted = dispute_state.votes
            .iter()
            .any(|v| v.arbitrator == *arbitrator.key);
        
        if already_voted {
            msg!("Arbitrator already voted");
            return Err(BettingPlatformError::AlreadyVoted.into());
        }
        
        dispute_state.votes.push(vote.clone());
        
        // Check if we have enough votes (for now, 1 is enough)
        // In production, check required number of arbitrators
        if dispute_state.votes.len() >= 1 {
            // Determine resolution
            let resolution = vote.vote;
            
            dispute_state.status = DisputeStatus::Resolved;
            dispute_state.resolution = Some(resolution);
            dispute_state.resolved_at = Some(Clock::from_account_info(clock)?.unix_timestamp);
            
            // Update resolution state based on dispute outcome
            match resolution {
                DisputeResolution::Upheld => {
                    // Original resolution stands
                    msg!("Dispute rejected, original resolution upheld");
                    resolution_state.status = ResolutionStatus::Resolved;
                    resolution_state.final_outcome = resolution_state.proposed_outcome;
                }
                DisputeResolution::Overturned => {
                    // Change to disputer's proposed outcome
                    msg!("Dispute successful, resolution overturned");
                    resolution_state.status = ResolutionStatus::Resolved;
                    resolution_state.final_outcome = Some(dispute_state.proposed_outcome);
                    
                    // Refund dispute bond
                    msg!("Refunding dispute bond: {} lamports", dispute_state.bond_amount);
                    // In production, transfer bond back to disputer
                }
                DisputeResolution::Invalid => {
                    // Market declared invalid
                    msg!("Market declared invalid");
                    resolution_state.status = ResolutionStatus::Cancelled;
                    resolution_state.final_outcome = None;
                    
                    // Refund dispute bond
                    msg!("Refunding dispute bond: {} lamports", dispute_state.bond_amount);
                }
            }
            
            resolution_state.resolved_at = dispute_state.resolved_at;
        }
        
        // Log resolution
        msg!("Dispute vote recorded:");
        msg!("  Arbitrator: {}", arbitrator.key);
        msg!("  Vote: {:?}", vote.vote);
        msg!("  Total votes: {}", dispute_state.votes.len());
        
        // Serialize and save
        dispute_state.serialize(&mut &mut dispute_account.data.borrow_mut()[..])?;
        resolution_state.serialize(&mut &mut resolution_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

pub mod mirror {
    use super::*;
    
    /// Mirror dispute status from another source (e.g., UMA)
    pub fn process_mirror_dispute(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        market_id: u128,
        disputed: bool,
    ) -> ProgramResult {
        msg!("Mirroring dispute status for market {}: {}", market_id, disputed);
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let oracle = next_account_info(account_info_iter)?;
        let resolution_account = next_account_info(account_info_iter)?;
        
        // Verify oracle is signer
        if !oracle.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Load resolution state
        let mut resolution_state = ResolutionState::try_from_slice(&resolution_account.data.borrow())?;
        resolution_state.validate()?;
        
        // Verify market matches
        if resolution_state.market_id != market_id {
            return Err(BettingPlatformError::InvalidAccountData.into());
        }
        
        // Update status based on external dispute state
        if disputed {
            if resolution_state.status == ResolutionStatus::Proposed || 
               resolution_state.status == ResolutionStatus::Confirmed {
                resolution_state.status = ResolutionStatus::Disputed;
                msg!("Market marked as disputed from external source");
            }
        } else {
            if resolution_state.status == ResolutionStatus::Disputed {
                // External dispute resolved, continue with resolution
                resolution_state.status = ResolutionStatus::Confirmed;
                msg!("External dispute resolved, continuing with resolution");
            }
        }
        
        // Serialize and save
        resolution_state.serialize(&mut &mut resolution_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

/// Submit evidence for a dispute
pub fn process_submit_evidence(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    evidence_cid: [u8; 32],
    proposed_outcome: u8,
    reason: DisputeReason,
) -> ProgramResult {
    msg!("Submitting evidence for dispute");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let disputer = next_account_info(account_info_iter)?;
    let dispute_account = next_account_info(account_info_iter)?;
    
    // Verify disputer is signer
    if !disputer.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load dispute
    let mut dispute_state = DisputeState::try_from_slice(&dispute_account.data.borrow())?;
    dispute_state.validate()?;
    
    // Verify disputer owns the dispute
    if dispute_state.disputer != *disputer.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify dispute is still active
    if dispute_state.status != DisputeStatus::Active {
        msg!("Cannot submit evidence for inactive dispute");
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Update dispute with evidence
    dispute_state.evidence_cid = Some(evidence_cid);
    dispute_state.proposed_outcome = proposed_outcome;
    dispute_state.reason = reason;
    dispute_state.status = DisputeStatus::UnderReview;
    
    // Log evidence submission
    msg!("Evidence submitted:");
    msg!("  CID: {:?}", evidence_cid);
    msg!("  Proposed outcome: {}", proposed_outcome);
    msg!("  Reason: {:?}", reason);
    
    // Serialize and save
    dispute_state.serialize(&mut &mut dispute_account.data.borrow_mut()[..])?;
    
    Ok(())
}