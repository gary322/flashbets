//! Max Probability Collapse Implementation
//!
//! Implements collapse rules where markets collapse to the outcome with highest probability
//! at settle_slot, with lexical ID tiebreaker

use borsh::BorshDeserialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    error::BettingPlatformError,
    events::{emit_event, EventType, MarketCollapsed},
    pda::ProposalPDA as ProposalPDAType,
    state::accounts::{ProposalPDA, ProposalState, Resolution},
};

use borsh::BorshSerialize;

/// Process market collapse at settle_slot
pub fn process_settle_slot_collapse(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Processing settle slot collapse");
    
    let account_info_iter = &mut accounts.iter();
    
    let keeper = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(keeper)?;
    validate_writable(proposal_account)?;
    
    // Load proposal
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Verify account ownership
    if proposal_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Check if proposal is active
    if proposal.state != ProposalState::Active {
        msg!("Proposal not active, state: {:?}", proposal.state);
        return Err(BettingPlatformError::ProposalNotActive.into());
    }
    
    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    
    // Check if we've reached settle_slot
    if current_slot < proposal.settle_slot {
        msg!(
            "Not yet at settle slot. Current: {}, Settle: {}",
            current_slot,
            proposal.settle_slot
        );
        return Err(BettingPlatformError::TooEarly.into());
    }
    
    // Find outcome with max probability
    let winning_outcome = find_max_probability_outcome(&proposal)?;
    
    msg!(
        "Collapsing market to outcome {} with probability {}",
        winning_outcome,
        proposal.prices[winning_outcome as usize]
    );
    
    // Create resolution
    proposal.resolution = Some(Resolution {
        outcome: winning_outcome,
        timestamp: clock.unix_timestamp,
        oracle_signature: [0u8; 64], // Will be filled by oracle
    });
    
    // Update state
    proposal.state = ProposalState::Resolved;
    
    // Save proposal
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::MarketCollapsed, &MarketCollapsed {
        proposal_id: proposal.proposal_id,
        winning_outcome,
        probability: proposal.prices[winning_outcome as usize],
        collapse_type: 0, // 0 = SettleSlot
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}

/// Find outcome with maximum probability, using lexical ID as tiebreaker
fn find_max_probability_outcome(proposal: &ProposalPDA) -> Result<u8, ProgramError> {
    if proposal.prices.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    let mut max_outcome = 0u8;
    let mut max_price = proposal.prices[0];
    
    for (outcome, &price) in proposal.prices.iter().enumerate().skip(1) {
        if outcome >= proposal.outcomes as usize {
            break;
        }
        
        // Use lexical tiebreaker: if prices are equal, prefer lower outcome ID
        if price > max_price {
            max_price = price;
            max_outcome = outcome as u8;
        }
        // If price equals max_price, keep the lower outcome ID (lexical order)
    }
    
    Ok(max_outcome)
}

/// Check if flash loan protection should halt trading
pub fn check_flash_loan_halt(
    price_history: &[(u64, u64)], // (slot, price)
    current_slot: u64,
    current_price: u64,
) -> Result<bool, ProgramError> {
    const FLASH_LOAN_WINDOW: u64 = 4; // 4 slots
    const FLASH_LOAN_THRESHOLD_BPS: u64 = 500; // 5%
    
    // Filter history to last 4 slots
    let window_start = current_slot.saturating_sub(FLASH_LOAN_WINDOW);
    let window_history: Vec<_> = price_history
        .iter()
        .filter(|(slot, _)| *slot >= window_start)
        .collect();
    
    if window_history.is_empty() {
        return Ok(false);
    }
    
    // Get oldest price in window
    let oldest_price = window_history.first().unwrap().1;
    
    // Calculate price change
    let price_change_bps = if current_price > oldest_price {
        ((current_price - oldest_price) * 10000) / oldest_price
    } else {
        ((oldest_price - current_price) * 10000) / oldest_price
    };
    
    // Halt if change exceeds 5% in 4 slots
    Ok(price_change_bps > FLASH_LOAN_THRESHOLD_BPS)
}

/// Collapse type for events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollapseType {
    SettleSlot,
    MaxProbability,
    Emergency,
}

/// Process emergency collapse (admin only)
pub fn process_emergency_collapse(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    winning_outcome: u8,
) -> ProgramResult {
    msg!("Processing emergency collapse");
    
    let account_info_iter = &mut accounts.iter();
    
    let admin = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(admin)?;
    validate_writable(proposal_account)?;
    
    // Load global config to verify admin authority
    let global_config_account = next_account_info(account_info_iter)?;
    let global_config = crate::state::GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Verify admin is the update authority
    if global_config.update_authority != *admin.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Load proposal
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Verify outcome is valid
    if winning_outcome >= proposal.outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Create resolution
    let clock = Clock::get()?;
    proposal.resolution = Some(Resolution {
        outcome: winning_outcome,
        timestamp: clock.unix_timestamp,
        oracle_signature: [0u8; 64], // Will be filled by oracle
    });
    
    // Update state
    proposal.state = ProposalState::Resolved;
    
    // Save proposal
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::MarketCollapsed, &MarketCollapsed {
        proposal_id: proposal.proposal_id,
        winning_outcome,
        probability: proposal.prices[winning_outcome as usize],
        collapse_type: 2, // 2 = Emergency
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_max_probability_selection() {
        let mut proposal = ProposalPDA::new([1u8; 32], [0u8; 32], 3);
        
        // Test clear winner
        proposal.prices = vec![2000, 7000, 1000]; // Outcome 1 wins
        assert_eq!(find_max_probability_outcome(&proposal).unwrap(), 1);
        
        // Test tie - lexical order wins
        proposal.prices = vec![4000, 4000, 2000]; // Outcome 0 wins (lexical)
        assert_eq!(find_max_probability_outcome(&proposal).unwrap(), 0);
        
        // Test all equal - first wins
        proposal.prices = vec![3333, 3333, 3334]; // Outcome 2 wins by 1 bp
        assert_eq!(find_max_probability_outcome(&proposal).unwrap(), 2);
    }
    
    #[test]
    fn test_flash_loan_detection() {
        let mut history = vec![
            (100, 1000),
            (101, 1020),
            (102, 1030),
            (103, 1040),
        ];
        
        // 4% change - should not halt
        assert!(!check_flash_loan_halt(&history, 104, 1040).unwrap());
        
        // 6% change - should halt
        assert!(check_flash_loan_halt(&history, 104, 1060).unwrap());
        
        // Test with price drop
        history = vec![
            (100, 1000),
            (101, 980),
            (102, 970),
            (103, 960),
        ];
        
        // 5.2% drop - should halt
        assert!(check_flash_loan_halt(&history, 104, 948).unwrap());
    }
}