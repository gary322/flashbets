use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::{Pack, IsInitialized},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    system_program,
    sysvar::Sysvar,
};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::priority::{
    PriorityQueue, QueueEntry, TradeData, EntryStatus, 
    PriorityCalculator, QueueManager
};

/// Submit trade to priority queue
pub fn process_submit_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    entry_id: u128,
    trade_data: TradeData,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let entry_account = next_account_info(account_info_iter)?;
    let stake_account = next_account_info(account_info_iter)?;
    let trader = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify trader is signer
    if !trader.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify system program
    if *system_program.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Load queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;

    // Verify queue is initialized
    if !queue.is_initialized() {
        return Err(ProgramError::UninitializedAccount);
    }

    // Check queue capacity
    if queue.current_size >= queue.max_size {
        return Err(BettingPlatformError::QueueFull.into());
    }

    // Load stake account to get actual stake amount
    let stake_account_data = stake_account.try_borrow_data()?;
    let stake_info = crate::mmt::state::StakeAccount::unpack(&stake_account_data)?;
    
    // Verify stake account belongs to trader
    if stake_info.owner != *trader.key {
        return Err(BettingPlatformError::InvalidOwner.into());
    }
    
    let stake_amount = stake_info.amount_staked;
    
    // Load market/verse data to get depth
    use borsh::BorshDeserialize;
    let market_data = crate::state::VersePDA::deserialize(&mut &market_account.data.borrow()[..])?;
    let verse_depth = market_data.depth;

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Load staking pool to get total stake
    let staking_pool_account = next_account_info(account_info_iter)?;
    let staking_pool_data = staking_pool_account.try_borrow_data()?;
    let staking_pool = crate::mmt::state::StakingPool::unpack(&staking_pool_data)?;
    let total_stake = staking_pool.total_staked;

    // Calculate priority
    let calculator = PriorityCalculator::default();

    let priority_score = calculator.calculate_priority(
        stake_amount,
        verse_depth as u32,
        clock.slot,
        trade_data.amount,
        clock.slot,
        total_stake,
    )?;

    // Derive PDA for entry
    let (entry_pda, bump_seed) = Pubkey::find_program_address(
        &[
            b"queue_entry",
            trader.key.as_ref(),
            entry_id.to_le_bytes().as_ref(),
        ],
        program_id,
    );

    if entry_pda != *entry_account.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create entry account
    let entry_size = 512; // Simplified size
    let rent = Rent::from_account_info(rent_sysvar)?;
    let rent_lamports = rent.minimum_balance(entry_size);

    invoke_signed(
        &system_instruction::create_account(
            trader.key,
            entry_account.key,
            rent_lamports,
            entry_size as u64,
            program_id,
        ),
        &[
            trader.clone(),
            entry_account.clone(),
            system_program.clone(),
        ],
        &[&[
            b"queue_entry",
            trader.key.as_ref(),
            entry_id.to_le_bytes().as_ref(),
            &[bump_seed],
        ]],
    )?;

    // Save amount before moving trade_data
    let trade_amount = trade_data.amount;
    
    // Create queue entry
    let entry = QueueEntry {
        entry_id,
        user: *trader.key,
        priority_score,
        submission_slot: clock.slot,
        submission_timestamp: clock.unix_timestamp,
        trade_data,
        status: EntryStatus::Pending,
        stake_snapshot: stake_amount,
        depth_boost: verse_depth as u32,
        bump: bump_seed,
    };

    // Serialize entry to account
    entry.serialize(&mut &mut entry_account.data.borrow_mut()[..])?;

    // Update queue
    queue.current_size += 1;
    queue.total_pending_volume = queue.total_pending_volume
        .saturating_add(trade_amount);

    // Pack queue back
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    msg!(
        "Submitted trade {} with priority score {}",
        entry_id,
        priority_score
    );

    Ok(())
}

/// Cancel pending trade
pub fn process_cancel_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    entry_id: u128,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let entry_account = next_account_info(account_info_iter)?;
    let trader = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify trader is signer
    if !trader.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify PDA
    let (entry_pda, _) = Pubkey::find_program_address(
        &[
            b"queue_entry",
            trader.key.as_ref(),
            entry_id.to_le_bytes().as_ref(),
        ],
        program_id,
    );

    if entry_pda != *entry_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Load entry and verify ownership
    use borsh::BorshDeserialize;
    let mut entry = QueueEntry::deserialize(&mut &entry_account.data.borrow()[..])?;
    
    // Verify ownership
    if entry.user != *trader.key {
        msg!("Entry user {} does not match trader {}", entry.user, trader.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Verify status is pending
    if entry.status != EntryStatus::Pending {
        msg!("Entry status is {:?}, not Pending", entry.status);
        return Err(BettingPlatformError::InvalidStatus.into());
    }
    
    // Update status to cancelled
    entry.status = EntryStatus::Cancelled;
    entry.serialize(&mut &mut entry_account.data.borrow_mut()[..])?;
    
    // Update queue volume
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;
    queue.total_pending_volume = queue.total_pending_volume
        .saturating_sub(entry.trade_data.amount);

    // Update queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;
    queue.current_size = queue.current_size.saturating_sub(1);
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;

    msg!("Cancelled trade {}", entry_id);

    Ok(())
}

/// Submit liquidation order with priority
pub fn process_submit_liquidation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: u128,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let queue_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;
    let keeper_stake_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify keeper is signer
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load position and calculate risk
    use crate::state::Position;
    use borsh::BorshDeserialize;
    let position = Position::deserialize(&mut &position_account.data.borrow()[..])?;
    
    // Calculate risk factor based on proximity to liquidation
    let current_price = position.last_mark_price;
    let liquidation_distance = if position.is_long {
        current_price.saturating_sub(position.liquidation_price)
    } else {
        position.liquidation_price.saturating_sub(current_price)
    };
    
    // Risk priority: closer to liquidation = higher priority
    let risk_factor = if liquidation_distance == 0 {
        1000u64 // Maximum risk
    } else {
        (1_000_000u64).saturating_div(liquidation_distance.max(1))
    };
    
    // Get keeper stake for priority boost
    let keeper_stake = 50_000u64; // In production, load from keeper_stake_account
    
    // Create liquidation order with risk-based priority
    let clock = Clock::from_account_info(clock_sysvar)?;
    let liquidation_priority = risk_factor.saturating_mul(2) // Double weight for risk
        .saturating_add(keeper_stake / 1000); // Stake bonus
    
    // Add to liquidation queue
    let mut queue = PriorityQueue::unpack(&queue_account.data.borrow())?;
    // Update queue size and volume tracking
    queue.current_size = queue.current_size.saturating_add(1);
    queue.total_pending_volume = queue.total_pending_volume
        .saturating_add(position.size);
    PriorityQueue::pack(queue, &mut queue_account.data.borrow_mut())?;
    
    msg!("Added liquidation for position {} with priority {}", 
         position_id, liquidation_priority);

    msg!("Submitted liquidation for position {}", position_id);

    Ok(())
}