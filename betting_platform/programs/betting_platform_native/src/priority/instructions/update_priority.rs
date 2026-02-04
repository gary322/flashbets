use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{Pack, IsInitialized},
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use crate::error::BettingPlatformError;
use crate::priority::{
    PriorityQueue, QueueEntry, PriorityCalculator, OrderingState,
    FairOrderingProtocol, TimeBasedOrdering, queue_storage
};

/// Update priority scores for aging orders
pub fn process_update_priorities(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let ordering_state_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;

    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify authority is authorized to update priorities
    // Load global config to check update authority
    let global_config_account = next_account_info(account_info_iter)?;
    let global_config = crate::state::GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    if global_config.update_authority != *authority.key {
        msg!("Unauthorized: {} is not update authority", authority.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }

    // Load queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;

    // Verify queue is initialized
    if !queue.is_initialized() {
        return Err(ProgramError::UninitializedAccount);
    }

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Load all queue entries
    let entries_storage_account = next_account_info(account_info_iter)?;
    let mut entries = queue_storage::load_all_queue_entries(entries_storage_account)?;
    
    // Calculate new priorities
    let calculator = PriorityCalculator::default();
    let time_ordering = TimeBasedOrdering::default();
    // Load staking pool to get total stake
    let staking_pool_account = next_account_info(account_info_iter)?;
    let staking_pool_data = staking_pool_account.try_borrow_data()?;
    let staking_pool = crate::mmt::state::StakingPool::unpack(&staking_pool_data)?;
    let total_stake = staking_pool.total_staked;

    let mut updated_count = 0;
    for entry in entries.iter_mut() {
        if entry.status == crate::priority::EntryStatus::Pending {
            // Calculate base priority
            let base_priority = calculator.calculate_priority(
                entry.stake_snapshot,
                entry.depth_boost,
                entry.submission_slot,
                entry.trade_data.amount,
                clock.slot,
                total_stake,
            )?;

            // Apply time-based adjustment
            let adjusted_priority = time_ordering.adjust_priority_by_time(
                base_priority,
                entry.submission_slot,
                clock.slot,
            );

            // Check if order is stale
            if time_ordering.is_stale(entry.submission_slot, clock.slot) {
                entry.status = crate::priority::EntryStatus::Expired;
                msg!("Order {} expired due to staleness", entry.entry_id);
            } else if adjusted_priority != entry.priority_score {
                entry.priority_score = adjusted_priority;
                updated_count += 1;
            }
        }
    }

    // Save queue_id before packing
    let queue_id = queue.queue_id;
    
    // Update queue
    queue.last_process_slot = clock.slot;
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    // Save updated entries
    queue_storage::save_updated_entries(
        entries_storage_account,
        &entries,
        queue_id,
        clock.slot,
    )?;

    msg!("Updated {} order priorities at slot {}", updated_count, clock.slot);

    Ok(())
}

/// Apply fair ordering randomization
pub fn process_apply_fair_ordering(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let ordering_state_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;

    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load queue
    let queue = PriorityQueue::unpack(&queue_account.data.borrow())?;
    
    // Load ordering state
    let ordering_state = queue_storage::load_ordering_state(ordering_state_account)?;

    // Load queue entries
    let entries_storage_account = next_account_info(account_info_iter)?;
    let mut entries = queue_storage::load_all_queue_entries(entries_storage_account)?;

    // Apply fair ordering
    let protocol = FairOrderingProtocol::default();
    protocol.apply_fair_ordering(&mut entries, &ordering_state)?;

    // Get clock for timestamp
    let clock_sysvar = next_account_info(account_info_iter)?;
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Save reordered entries
    queue_storage::save_reordered_entries(
        entries_storage_account,
        &entries,
        queue.queue_id,
        clock.slot,
    )?;

    msg!("Applied fair ordering to {} entries", entries.len());

    Ok(())
}

/// Request randomness for fair ordering
pub fn process_request_randomness(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let ordering_state_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load ordering state from account
    let mut ordering_state = if ordering_state_account.data_len() > 0 {
        OrderingState::unpack(&ordering_state_account.data.borrow())?
    } else {
        OrderingState::new()
    };

    // Request randomness
    let protocol = FairOrderingProtocol::default();
    protocol.request_randomness(&mut ordering_state)?;

    let current_epoch = ordering_state.current_epoch;

    // Save ordering state
    OrderingState::pack(ordering_state, &mut ordering_state_account.data.borrow_mut())?;

    msg!("Randomness requested for epoch {}", current_epoch);

    Ok(())
}

/// Update randomness from VRF
pub fn process_update_randomness(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    vrf_output: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let ordering_state_account = next_account_info(account_info_iter)?;
    let vrf_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;

    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify VRF output
    verify_vrf_output(vrf_account, vrf_output, authority)?;

    // Load ordering state from account
    let mut ordering_state = if ordering_state_account.data_len() > 0 {
        OrderingState::unpack(&ordering_state_account.data.borrow())?
    } else {
        OrderingState::new()
    };

    // Update randomness
    let protocol = FairOrderingProtocol::default();
    protocol.update_randomness(&mut ordering_state, vrf_output)?;

    let current_epoch = ordering_state.current_epoch;

    // Save ordering state
    OrderingState::pack(ordering_state, &mut ordering_state_account.data.borrow_mut())?;

    msg!("Randomness updated for epoch {}", current_epoch);

    Ok(())
}

/// Maintain queue by removing expired entries
pub fn process_maintain_queue(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;

    // Verify keeper is signer
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Load and clean expired entries
    let entries_storage_account = next_account_info(account_info_iter)?;
    let max_age = 150u64; // 150 slots = ~1 minute
    
    let (remaining_entries, removed_count) = queue_storage::load_and_clean_expired_entries(
        entries_storage_account,
        queue.queue_id,
        clock.slot,
        max_age,
    )?;
    
    let updated_count = remaining_entries.len() as u32;
    queue.current_size = updated_count;
    queue.last_process_slot = clock.slot;

    // Save current size before packing
    let remaining_size = queue.current_size;
    
    // Update queue
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    msg!(
        "Queue maintenance: {} removed, {} updated, {} remaining",
        removed_count,
        updated_count,
        remaining_size
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_update() {
        // Test that priorities are updated correctly over time
        let calculator = PriorityCalculator::default();
        let time_ordering = TimeBasedOrdering::default();

        let base_priority = 1000u128;
        let submission_slot = 100u64;
        let current_slot = 150u64;

        let adjusted = time_ordering.adjust_priority_by_time(
            base_priority,
            submission_slot,
            current_slot,
        );

        assert!(adjusted > base_priority);
    }

    #[test]
    fn test_staleness_check() {
        let time_ordering = TimeBasedOrdering::default();

        // Not stale
        assert!(!time_ordering.is_stale(100, 150));

        // Stale
        assert!(time_ordering.is_stale(100, 250));
    }
}

/// Verify VRF output for randomness
fn verify_vrf_output(
    vrf_account: &AccountInfo,
    vrf_output: [u8; 32],
    authority: &AccountInfo,
) -> Result<(), ProgramError> {
    // Verify VRF account is owned by the VRF program
    const VRF_PROGRAM_ID: Pubkey = solana_program::pubkey!("VRFjPDvL2S8FMHfp1LAyJF1YPCj2v59aTZqkYQQbVpe");
    
    if vrf_account.owner != &VRF_PROGRAM_ID {
        msg!("Invalid VRF account owner: {}", vrf_account.owner);
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify VRF output matches account data
    let vrf_data = vrf_account.try_borrow_data()?;
    if vrf_data.len() < 32 {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if output matches stored value
    let stored_output: [u8; 32] = vrf_data[..32].try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    if stored_output != vrf_output {
        msg!("VRF output mismatch");
        return Err(BettingPlatformError::InvalidVRFOutput.into());
    }
    
    // Verify freshness (VRF should be recent)
    if vrf_data.len() >= 40 {
        let timestamp_bytes: [u8; 8] = vrf_data[32..40].try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        let timestamp = i64::from_le_bytes(timestamp_bytes);
        
        let clock = Clock::get()?;
        let age = clock.unix_timestamp - timestamp;
        
        // VRF output should be less than 5 minutes old
        if age > 300 {
            msg!("VRF output too old: {} seconds", age);
            return Err(BettingPlatformError::StaleVRFOutput.into());
        }
    }
    
    Ok(())
}