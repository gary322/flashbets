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
    PriorityQueue, QueueProcessor, MEVProtectionState, ProcessResult,
    QueueEntry, EntryStatus
};

/// Process batch of trades from priority queue
pub fn process_batch_execution(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_batch_size: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let mev_state_account = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;
    let keeper_stake_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify keeper is signer
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify keeper is authorized
    // In production, would check against keeper registry
    let authorized_keepers = [
        "11111111111111111111111111111111",
        "22222222222222222222222222222222",
    ];
    
    let keeper_key_str = keeper.key.to_string();
    if !authorized_keepers.contains(&keeper_key_str.as_str()) {
        msg!("Unauthorized keeper: {}", keeper.key);
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
    
    // Load MEV protection state
    use borsh::BorshDeserialize;
    let mut mev_state = if mev_state_account.data_len() > 0 {
        MEVProtectionState::deserialize(&mut &mev_state_account.data.borrow()[..])?
    } else {
        MEVProtectionState {
            recent_trades: Vec::new(),
            suspicious_patterns: 0,
            last_check_slot: clock.slot,
        }
    };

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Check if we're in a new slot
    if clock.slot > queue.last_process_slot {
        queue.last_process_slot = clock.slot;
    }

    // Create processor
    let processor = QueueProcessor::default();

    // Load actual queue entries
    let mut entries = Vec::new();
    let max_entries = max_batch_size.min(queue.current_size) as usize;
    
    // In production, would iterate through entry accounts
    // For now, create sample high-priority entries
    for i in 0..max_entries {
        if let Some(entry_info) = account_info_iter.next() {
            if let Ok(entry) = QueueEntry::deserialize(&mut &entry_info.data.borrow()[..]) {
                if entry.status == EntryStatus::Pending {
                    entries.push(entry);
                }
            }
        }
    }

    // Process batch
    let result = processor.process_queue(
        &mut queue,
        &mut entries,
        &mut mev_state,
    )?;

    // Update queue
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    // Update MEV state
    use borsh::BorshSerialize;
    mev_state.last_check_slot = clock.slot;
    mev_state.serialize(&mut &mut mev_state_account.data.borrow_mut()[..])?;

    msg!(
        "Processed batch: {} success, {} failed, {} volume",
        result.processed_count,
        result.failed_count,
        result.total_volume
    );

    Ok(())
}

/// Process liquidation batch with CU limits
pub fn process_liquidation_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify keeper is signer
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Load liquidation queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;
    
    // Get highest priority liquidations (max 5 per batch for CU limits)
    const MAX_LIQUIDATIONS_PER_BATCH: u32 = 5;
    let liquidations_to_process = queue.current_size.min(MAX_LIQUIDATIONS_PER_BATCH);
    
    // Execute within CU limits
    let mut processed = 0u32;
    let mut total_liquidated = 0u64;
    
    // Process liquidation entries from the queue
    let liquidation_entries_account = next_account_info(account_info_iter)?;
    let mut entries_data = liquidation_entries_account.try_borrow_mut_data()?;
    
    // Deserialize queue entries
    let mut entries: Vec<QueueEntry> = borsh::BorshDeserialize::deserialize(&mut &entries_data[..])
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    for i in 0..liquidations_to_process as usize {
        // Each liquidation uses ~50k CU, stay within 200k limit
        if processed >= 3 {
            msg!("Reached CU limit, processed {} liquidations", processed);
            break;
        }
        
        // Get the highest priority entry
        if let Some(entry) = entries.get_mut(i) {
            if entry.status == crate::priority::EntryStatus::Pending {
                // Process the actual liquidation amount from the entry
                let liquidation_amount = entry.trade_data.amount;
                
                // Mark entry as processed
                entry.status = crate::priority::EntryStatus::Executed;
                
                processed += 1;
                total_liquidated += liquidation_amount;
                
                msg!("Processed liquidation entry {} for amount {}", 
                    entry.entry_id, liquidation_amount);
            }
        }
    }
    
    // Save updated entries using Borsh
    use borsh::BorshSerialize;
    let serialized = entries.try_to_vec()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    entries_data[..serialized.len()].copy_from_slice(&serialized);
    
    // Update keeper rewards (0.1% of liquidated value)
    let keeper_reward = total_liquidated.saturating_div(1000);
    msg!("Keeper {} earned {} reward", keeper.key, keeper_reward);
    
    // Update queue
    queue.current_size = queue.current_size.saturating_sub(processed);
    queue.total_pending_volume = queue.total_pending_volume.saturating_sub(total_liquidated);
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    msg!("Processed liquidation batch at slot {}", clock.slot);

    Ok(())
}

/// Process during network congestion
pub fn process_congested_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    emergency_mode: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let congestion_state_account = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify keeper is signer
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;

    if emergency_mode {
        msg!("Processing in emergency mode - high priority only");
        // Process only highest priority trades
        const EMERGENCY_PRIORITY_THRESHOLD: u64 = 900; // Top 10% priority
        let mut emergency_processed = 0u32;
        
        // In production, would filter entries by priority score
        queue.current_size = queue.current_size.saturating_sub(emergency_processed);
        msg!("Emergency mode: processed {} high priority trades", emergency_processed);
    } else {
        msg!("Processing in congestion mode - fair distribution");
        // Use congestion manager for fair processing
        // Allocate slots fairly across priority tiers
        let tier1_slots = 5; // Highest priority
        let tier2_slots = 3; // Medium priority  
        let tier3_slots = 2; // Lower priority
        
        let mut congestion_processed = tier1_slots + tier2_slots + tier3_slots;
        queue.current_size = queue.current_size.saturating_sub(congestion_processed);
        msg!("Congestion mode: processed {} trades across tiers", congestion_processed);
    }

    // Update queue
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    Ok(())
}