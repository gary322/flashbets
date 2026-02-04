//! Market resolution processing
//!
//! Handles market resolution, settlement, and payout distribution

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
    state::{
        resolution_accounts::{
            ResolutionState, ResolutionStatus, OracleSignature,
            discriminators as res_discriminators,
        },
        amm_accounts::MarketState,
    },
};

/// Process market resolution
pub fn process_resolution(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    verse_id: u128,
    market_id: u128,
    resolution_outcome: u8,
) -> ProgramResult {
    msg!("Processing market resolution for market {} with outcome {}", market_id, resolution_outcome);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let oracle = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let resolution_account = next_account_info(account_info_iter)?;
    let oracle_config = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify oracle is signer
    if !oracle.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load market and verify it exists
    let market_data = market_account.data.borrow();
    if market_data.len() < 40 {
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    
    // Extract market state (assume it's at a fixed offset)
    let market_state_offset = 100; // Adjust based on actual market structure
    if market_data.len() <= market_state_offset {
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    let market_state = MarketState::try_from_slice(&market_data[market_state_offset..market_state_offset + 1])?;
    
    // Verify market is active
    if market_state != MarketState::Active {
        msg!("Market is not active, current state: {:?}", market_state);
        return Err(BettingPlatformError::MarketNotActive.into());
    }
    
    // Verify oracle is authorized (simplified - in production, check oracle config)
    // For now, we'll assume any signer can be an oracle
    
    // Get current time
    let clock = Clock::from_account_info(clock)?;
    let current_time = clock.unix_timestamp;
    
    // Derive resolution state PDA
    let (resolution_pda, bump_seed) = Pubkey::find_program_address(
        &[
            b"resolution",
            &market_id.to_le_bytes(),
        ],
        program_id,
    );
    
    // Check if resolution account exists
    let mut resolution_state = if resolution_account.data_len() > 0 {
        // Load existing resolution state
        let mut state = ResolutionState::try_from_slice(&resolution_account.data.borrow())?;
        state.validate()?;
        
        // Verify it's for the correct market
        if state.market_id != market_id {
            return Err(BettingPlatformError::InvalidAccountData.into());
        }
        
        state
    } else {
        // Create new resolution state
        if resolution_pda != *resolution_account.key {
            msg!("Invalid resolution PDA");
            return Err(ProgramError::InvalidSeeds);
        }
        
        // Calculate required space
        let resolution_size = ResolutionState::space(10); // Space for 10 oracle signatures
        
        // Create account
        let rent_lamports = Rent::from_account_info(rent)?
            .minimum_balance(resolution_size);
        
        invoke_signed(
            &system_instruction::create_account(
                oracle.key,
                resolution_account.key,
                rent_lamports,
                resolution_size as u64,
                program_id,
            ),
            &[
                oracle.clone(),
                resolution_account.clone(),
                system_program.clone(),
            ],
            &[&[b"resolution", &market_id.to_le_bytes(), &[bump_seed]]],
        )?;
        
        ResolutionState::new(market_id, verse_id)
    };
    
    // Update resolution state based on current status
    match resolution_state.status {
        ResolutionStatus::Pending => {
            // First oracle proposing resolution
            resolution_state.status = ResolutionStatus::Proposed;
            resolution_state.proposed_outcome = Some(resolution_outcome);
            resolution_state.proposing_oracle = Some(*oracle.key);
            resolution_state.proposed_at = Some(current_time);
            
            // Set dispute window (24 hours)
            resolution_state.dispute_window_end = Some(current_time + 86400);
            
            msg!("Resolution proposed:");
            msg!("  Outcome: {}", resolution_outcome);
            msg!("  Oracle: {}", oracle.key);
            msg!("  Dispute window ends: {}", current_time + 86400);
        }
        ResolutionStatus::Proposed => {
            // Additional oracle confirming or conflicting
            if resolution_state.proposed_outcome != Some(resolution_outcome) {
                msg!("Conflicting resolution proposed");
                return Err(BettingPlatformError::ConflictingResolution.into());
            }
            
            // Add oracle signature
            let signature = OracleSignature {
                oracle: *oracle.key,
                outcome: resolution_outcome,
                timestamp: current_time,
                signature: [0u8; 64], // In production, would include actual signature
            };
            
            // Check if oracle already signed
            let already_signed = resolution_state.oracle_signatures
                .iter()
                .any(|s| s.oracle == *oracle.key);
            
            if already_signed {
                msg!("Oracle already signed this resolution");
                return Err(BettingPlatformError::AlreadySigned.into());
            }
            
            resolution_state.oracle_signatures.push(signature);
            
            // Check if we have enough confirmations (for now, just 1 is enough)
            // In production, check oracle config for required confirmations
            if resolution_state.oracle_signatures.len() >= 1 {
                resolution_state.status = ResolutionStatus::Confirmed;
                resolution_state.confirmed_at = Some(current_time);
                
                msg!("Resolution confirmed with {} signatures", resolution_state.oracle_signatures.len());
            }
        }
        ResolutionStatus::Confirmed => {
            // Check if dispute window has passed
            if let Some(dispute_end) = resolution_state.dispute_window_end {
                if current_time < dispute_end {
                    msg!("Still within dispute window");
                    return Err(BettingPlatformError::DisputeWindowActive.into());
                }
            }
            
            // Finalize resolution
            resolution_state.status = ResolutionStatus::Resolved;
            resolution_state.final_outcome = resolution_state.proposed_outcome;
            resolution_state.resolved_at = Some(current_time);
            
            msg!("Market resolved with outcome {}", resolution_outcome);
        }
        ResolutionStatus::Disputed => {
            msg!("Market is under dispute");
            return Err(BettingPlatformError::MarketDisputed.into());
        }
        ResolutionStatus::Resolved => {
            msg!("Market already resolved");
            return Err(BettingPlatformError::AlreadyResolved.into());
        }
        ResolutionStatus::Cancelled => {
            msg!("Resolution was cancelled");
            return Err(BettingPlatformError::ResolutionCancelled.into());
        }
    }
    
    // Serialize and save resolution state
    resolution_state.serialize(&mut &mut resolution_account.data.borrow_mut()[..])?;
    
    // If market is now resolved, update market state
    if resolution_state.status == ResolutionStatus::Resolved {
        // In production, update the market account to MarketState::Resolved
        msg!("Market {} is now resolved with outcome {}", market_id, resolution_outcome);
    }
    
    Ok(())
}

/// Process settlement for resolved market
pub fn process_settlement(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    batch_size: u8,
) -> ProgramResult {
    msg!("Processing settlement for market {} with batch size {}", market_id, batch_size);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper = next_account_info(account_info_iter)?;
    let resolution_account = next_account_info(account_info_iter)?;
    let settlement_queue = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let vault = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify keeper is signer (in production, verify it's a registered keeper)
    if !keeper.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load resolution state
    let mut resolution_state = ResolutionState::try_from_slice(&resolution_account.data.borrow())?;
    resolution_state.validate()?;
    
    // Verify market is resolved
    if resolution_state.status != ResolutionStatus::Resolved {
        msg!("Market not yet resolved");
        return Err(BettingPlatformError::MarketNotResolved.into());
    }
    
    let final_outcome = resolution_state.final_outcome
        .ok_or(BettingPlatformError::InvalidResolution)?;
    
    // Load settlement queue
    let mut queue = crate::state::resolution_accounts::SettlementQueue::try_from_slice(
        &settlement_queue.data.borrow()
    )?;
    queue.validate()?;
    
    // Process batch of settlements
    let mut processed = 0u8;
    let clock = Clock::from_account_info(clock)?;
    
    while processed < batch_size {
        if let Some(item) = queue.get_next() {
            // Calculate payout based on outcome
            let payout = if item.outcome == final_outcome {
                // Winner gets their shares as payout
                item.shares
            } else {
                // Loser gets nothing
                0
            };
            
            if payout > 0 {
                msg!("Settling position:");
                msg!("  Holder: {}", item.position_holder);
                msg!("  Outcome: {}", item.outcome);
                msg!("  Shares: {}", item.shares);
                msg!("  Payout: {}", payout);
                
                // In production, transfer payout from vault to position holder
                
                resolution_state.total_payout += payout;
            }
            
            resolution_state.positions_settled += 1;
            processed += 1;
        } else {
            // No more items in queue
            queue.settlement_completed = true;
            resolution_state.settlement_completed = true;
            break;
        }
    }
    
    msg!("Processed {} settlements", processed);
    msg!("Total positions settled: {}", resolution_state.positions_settled);
    msg!("Total payout: {}", resolution_state.total_payout);
    
    // Save updated states
    resolution_state.serialize(&mut &mut resolution_account.data.borrow_mut()[..])?;
    queue.serialize(&mut &mut settlement_queue.data.borrow_mut()[..])?;
    
    // Reward keeper for processing settlements
    if processed > 0 {
        let keeper_reward = processed as u64 * 100_000; // 0.0001 SOL per settlement
        msg!("Keeper reward: {} lamports", keeper_reward);
        // In production, transfer reward to keeper
    }
    
    Ok(())
}