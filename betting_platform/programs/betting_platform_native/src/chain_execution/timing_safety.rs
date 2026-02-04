//! Chain Timing Safety Module
//!
//! Ensures atomic execution by validating no pending resolutions
//! during chain execution. This prevents timing attacks where
//! a verse resolves mid-chain.
//!
//! Per specification: Chains execute atomically in single transaction

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::BorshDeserialize;

use crate::{
    error::BettingPlatformError,
    state::{VersePDA, ProposalPDA, ProposalState},
};

/// Safety buffer in slots before resolution
pub const RESOLUTION_SAFETY_BUFFER: u64 = 10;

/// Check if any verse in the chain has pending resolution
pub fn validate_no_pending_resolution(
    verse_accounts: &[&AccountInfo],
    proposal_accounts: &[&AccountInfo],
    current_slot: u64,
) -> Result<(), ProgramError> {
    msg!("Validating no pending resolutions for chain execution");
    
    // Check each verse's proposals
    for (verse_account, proposal_account) in verse_accounts.iter().zip(proposal_accounts.iter()) {
        let verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
        let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
        
        // Check if proposal is near resolution
        if proposal.status == ProposalState::Active {
            if proposal.settle_slot > 0 {
                let slots_until_settle = proposal.settle_slot.saturating_sub(current_slot);
                
                if slots_until_settle <= RESOLUTION_SAFETY_BUFFER {
                    msg!(
                        "Proposal {} resolves in {} slots, within safety buffer",
                        bs58::encode(&proposal.proposal_id).into_string(),
                        slots_until_settle
                    );
                    return Err(BettingPlatformError::ProposalExpired.into());
                }
            }
        }
        
        // Check if proposal was recently resolved (could affect chain)
        if proposal.status == ProposalState::Resolved {
            if let Some(settled_at) = proposal.settled_at {
                let slots_since_settled = (current_slot as i64).saturating_sub(settled_at);
                
                if slots_since_settled < RESOLUTION_SAFETY_BUFFER as i64 {
                    msg!(
                        "Proposal {} was settled {} slots ago, too recent",
                        bs58::encode(&proposal.proposal_id).into_string(),
                        slots_since_settled
                    );
                    return Err(BettingPlatformError::AlreadyResolved.into());
                }
            }
        }
    }
    
    Ok(())
}

/// Validate chain can execute atomically within CU limits
pub fn validate_chain_atomicity(
    steps: &[crate::instruction::ChainStepType],
) -> Result<(), ProgramError> {
    // Each step consumes approximately:
    // - Borrow: 5k CU
    // - Liquidity: 3k CU  
    // - Stake: 1k CU
    // - Position operations: 4k CU
    const CU_PER_STEP: u64 = 9_000;
    const MAX_CU_FOR_CHAIN: u64 = 45_000; // Conservative limit
    
    let estimated_cu = steps.len() as u64 * CU_PER_STEP;
    
    if estimated_cu > MAX_CU_FOR_CHAIN {
        msg!(
            "Chain requires {} CU, exceeds limit of {}",
            estimated_cu,
            MAX_CU_FOR_CHAIN
        );
        return Err(BettingPlatformError::TooManySteps.into());
    }
    
    Ok(())
}

/// Ensure all accounts are writable for atomic updates
pub fn validate_account_permissions(
    accounts: &[&AccountInfo],
) -> Result<(), ProgramError> {
    for (idx, account) in accounts.iter().enumerate() {
        if account.data_len() > 0 && !account.is_writable {
            msg!("Account {} is not writable but needs to be updated", idx);
            return Err(ProgramError::InvalidAccountData);
        }
    }
    
    Ok(())
}

/// Bundle operations for atomic execution
pub struct AtomicChainBundle<'a> {
    pub user: &'a AccountInfo<'a>,
    pub verse_accounts: Vec<&'a AccountInfo<'a>>,
    pub proposal_accounts: Vec<&'a AccountInfo<'a>>,
    pub position_accounts: Vec<&'a AccountInfo<'a>>,
    pub clock: Clock,
}

impl<'a> AtomicChainBundle<'a> {
    /// Validate bundle is ready for atomic execution
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check timing
        validate_no_pending_resolution(
            &self.verse_accounts,
            &self.proposal_accounts,
            self.clock.slot,
        )?;
        
        // Check permissions
        let mut all_accounts: Vec<&AccountInfo> = vec![self.user];
        all_accounts.extend(self.verse_accounts.iter().copied());
        all_accounts.extend(self.proposal_accounts.iter().copied());
        all_accounts.extend(self.position_accounts.iter().copied());
        
        validate_account_permissions(&all_accounts[..])?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resolution_timing_check() {
        // Test that chains are blocked when resolution is near
        let current_slot = 1000;
        let settle_slot = 1005; // Only 5 slots away
        
        // This should fail as it's within the safety buffer
        // In a real test we'd create proper account structures
    }
    
    #[test]
    fn test_atomicity_validation() {
        use crate::instruction::ChainStepType;
        
        // Test that 5 steps fit within CU limit
        let steps = vec![
            ChainStepType::Long { outcome: 0, leverage: 10 },
            ChainStepType::Borrow { amount: 1000 },
            ChainStepType::Liquidity { amount: 500 },
            ChainStepType::Stake { amount: 200 },
            ChainStepType::Long { outcome: 1, leverage: 5 },
        ];
        
        assert!(validate_chain_atomicity(&steps).is_ok());
        
        // Test that 6+ steps exceed limit
        let mut too_many_steps = steps.clone();
        too_many_steps.push(ChainStepType::Borrow { amount: 100 });
        
        assert!(validate_chain_atomicity(&too_many_steps).is_err());
    }
}