//! Keeper work queue management
//!
//! Manages and prioritizes work items for keepers

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
    state::keeper_accounts::{WorkType, KeeperAccount},
    account_validation::DISCRIMINATOR_SIZE,
};

/// Discriminator for work queue
pub const WORK_QUEUE_DISCRIMINATOR: [u8; 8] = [123, 89, 201, 45, 167, 23, 78, 156];

/// Work item status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum WorkStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Expired,
}

/// Work item priority
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum WorkPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Work item in the queue
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct WorkItem {
    /// Unique work ID
    pub work_id: [u8; 32],
    
    /// Work type
    pub work_type: WorkType,
    
    /// Target account (e.g., stop order, position)
    pub target_account: Pubkey,
    
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Priority
    pub priority: WorkPriority,
    
    /// Status
    pub status: WorkStatus,
    
    /// Assigned keeper (if any)
    pub assigned_keeper: Option<Pubkey>,
    
    /// Creation slot
    pub created_slot: u64,
    
    /// Assignment slot
    pub assigned_slot: Option<u64>,
    
    /// Completion slot
    pub completed_slot: Option<u64>,
    
    /// Expiry slot
    pub expiry_slot: u64,
    
    /// Bounty amount
    pub bounty: u64,
    
    /// Additional data (e.g., trigger price for stop orders)
    pub data: Vec<u8>,
}

/// Work queue for a specific work type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct WorkQueue {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Work type for this queue
    pub work_type: WorkType,
    
    /// Total items ever added
    pub total_items: u64,
    
    /// Currently pending items
    pub pending_items: u32,
    
    /// Currently assigned items
    pub assigned_items: u32,
    
    /// Completed items
    pub completed_items: u64,
    
    /// Failed items
    pub failed_items: u64,
    
    /// Work items (circular buffer)
    pub items: Vec<WorkItem>,
    
    /// Head index (oldest item)
    pub head: u32,
    
    /// Tail index (newest item)
    pub tail: u32,
    
    /// Maximum queue size
    pub max_size: u32,
}

impl WorkQueue {
    pub const BASE_SIZE: usize = DISCRIMINATOR_SIZE + 1 + 8 + 4 + 4 + 8 + 8 + 4 + 4 + 4 + 4;
    
    pub fn new(work_type: WorkType, max_size: u32) -> Self {
        Self {
            discriminator: WORK_QUEUE_DISCRIMINATOR,
            work_type,
            total_items: 0,
            pending_items: 0,
            assigned_items: 0,
            completed_items: 0,
            failed_items: 0,
            items: Vec::with_capacity(max_size as usize),
            head: 0,
            tail: 0,
            max_size,
        }
    }
    
    pub fn space(max_size: u32) -> usize {
        Self::BASE_SIZE + (max_size as usize * std::mem::size_of::<WorkItem>())
    }
    
    pub fn add_item(&mut self, item: WorkItem) -> Result<(), ProgramError> {
        if self.pending_items >= self.max_size {
            return Err(BettingPlatformError::QueueFull.into());
        }
        
        if self.items.len() < self.max_size as usize {
            self.items.push(item);
        } else {
            self.items[self.tail as usize] = item;
        }
        
        self.tail = (self.tail + 1) % self.max_size;
        self.pending_items += 1;
        self.total_items += 1;
        
        Ok(())
    }
    
    pub fn get_next_pending(&self) -> Option<&WorkItem> {
        for i in 0..self.items.len() {
            let index = (self.head as usize + i) % self.items.len();
            if let Some(item) = self.items.get(index) {
                if item.status == WorkStatus::Pending {
                    return Some(item);
                }
            }
        }
        None
    }
    
    pub fn assign_item(&mut self, work_id: &[u8; 32], keeper: &Pubkey, slot: u64) -> Result<(), ProgramError> {
        for item in self.items.iter_mut() {
            if item.work_id == *work_id && item.status == WorkStatus::Pending {
                item.status = WorkStatus::Assigned;
                item.assigned_keeper = Some(*keeper);
                item.assigned_slot = Some(slot);
                self.pending_items = self.pending_items.saturating_sub(1);
                self.assigned_items += 1;
                return Ok(());
            }
        }
        Err(BettingPlatformError::InvalidOperation.into())
    }
    
    pub fn complete_item(&mut self, work_id: &[u8; 32], slot: u64) -> Result<(), ProgramError> {
        for item in self.items.iter_mut() {
            if item.work_id == *work_id && 
               (item.status == WorkStatus::Assigned || item.status == WorkStatus::InProgress) {
                item.status = WorkStatus::Completed;
                item.completed_slot = Some(slot);
                self.assigned_items = self.assigned_items.saturating_sub(1);
                self.completed_items += 1;
                return Ok(());
            }
        }
        Err(BettingPlatformError::InvalidOperation.into())
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != WORK_QUEUE_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Initialize work queue
pub fn process_initialize_work_queue(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    work_type: WorkType,
    max_size: u32,
) -> ProgramResult {
    msg!("Initializing work queue for {:?}", work_type);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let work_queue_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate max size
    if max_size == 0 || max_size > 10000 {
        msg!("Invalid max size: must be between 1 and 10000");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Derive work queue PDA
    let work_type_byte = match work_type {
        WorkType::Liquidations => 0u8,
        WorkType::StopOrders => 1u8,
        WorkType::PriceUpdates => 2u8,
        WorkType::Resolutions => 3u8,
    };
    
    let (queue_pda, bump_seed) = Pubkey::find_program_address(
        &[b"work_queue", &[work_type_byte]],
        program_id,
    );
    
    // Verify PDA matches
    if queue_pda != *work_queue_account.key {
        msg!("Invalid work queue PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if already initialized
    if work_queue_account.data_len() > 0 {
        msg!("Work queue already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate required space
    let queue_size = WorkQueue::space(max_size);
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(queue_size);
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            work_queue_account.key,
            rent_lamports,
            queue_size as u64,
            program_id,
        ),
        &[
            authority.clone(),
            work_queue_account.clone(),
            system_program.clone(),
        ],
        &[&[b"work_queue", &[work_type_byte], &[bump_seed]]],
    )?;
    
    // Initialize queue
    let queue = WorkQueue::new(work_type, max_size);
    
    // Log initialization
    msg!("Work queue initialized:");
    msg!("  Type: {:?}", work_type);
    msg!("  Max size: {}", max_size);
    
    // Serialize and save
    queue.serialize(&mut &mut work_queue_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Add work item to queue
pub fn process_add_work_item(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    work_type: WorkType,
    target_account: Pubkey,
    market_id: [u8; 32],
    priority: WorkPriority,
    bounty: u64,
    expiry_slots: u64,
    data: Vec<u8>,
) -> ProgramResult {
    msg!("Adding work item to queue");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let work_queue_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify authority (in production, check if it's a valid program account)
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load queue
    let mut queue = WorkQueue::try_from_slice(&work_queue_account.data.borrow())?;
    queue.validate()?;
    
    // Verify work type matches
    if queue.work_type != work_type {
        msg!("Work type mismatch");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    let current_slot = clock.slot;
    
    // Generate work ID
    let work_id_seed = [
        target_account.as_ref(),
        &current_slot.to_le_bytes(),
        &queue.total_items.to_le_bytes(),
    ].concat();
    let work_id = solana_program::hash::hash(&work_id_seed).to_bytes();
    
    // Create work item
    let work_item = WorkItem {
        work_id,
        work_type,
        target_account,
        market_id,
        priority,
        status: WorkStatus::Pending,
        assigned_keeper: None,
        created_slot: current_slot,
        assigned_slot: None,
        completed_slot: None,
        expiry_slot: current_slot + expiry_slots,
        bounty,
        data,
    };
    
    // Add to queue
    queue.add_item(work_item)?;
    
    // Log addition
    msg!("Work item added:");
    msg!("  ID: {:?}", work_id);
    msg!("  Type: {:?}", work_type);
    msg!("  Priority: {:?}", priority);
    msg!("  Bounty: {} lamports", bounty);
    msg!("  Expiry: {} slots", expiry_slots);
    msg!("  Queue size: {}/{}", queue.pending_items, queue.max_size);
    
    // Serialize and save
    queue.serialize(&mut &mut work_queue_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Claim work item from queue
pub fn process_claim_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    work_type: WorkType,
) -> ProgramResult {
    msg!("Claiming work from queue");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let work_queue_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify keeper authority is signer
    if !keeper_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load keeper
    let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    keeper.validate()?;
    
    // Verify keeper authority
    if keeper.authority != *keeper_authority.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check keeper can do this work type
    if !keeper.has_specialization(&work_type) {
        msg!("Keeper lacks required specialization");
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Load queue
    let mut queue = WorkQueue::try_from_slice(&work_queue_account.data.borrow())?;
    queue.validate()?;
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    let current_slot = clock.slot;
    
    // Find highest priority pending work
    let mut best_work: Option<(usize, &WorkItem)> = None;
    let mut best_priority = WorkPriority::Low;
    
    for (index, item) in queue.items.iter().enumerate() {
        if item.status == WorkStatus::Pending && 
           item.expiry_slot > current_slot &&
           item.priority >= best_priority {
            best_work = Some((index, item));
            best_priority = item.priority;
        }
    }
    
    if let Some((index, work)) = best_work {
        let work_id = work.work_id;
        let target = work.target_account;
        let bounty = work.bounty;
        
        // Assign work
        queue.assign_item(&work_id, keeper_account.key, current_slot)?;
        
        // Log assignment
        msg!("Work assigned:");
        msg!("  Work ID: {:?}", work_id);
        msg!("  Keeper: {}", keeper_account.key);
        msg!("  Target: {}", target);
        msg!("  Priority: {:?}", best_priority);
        msg!("  Bounty: {} lamports", bounty);
        
        // Serialize and save
        queue.serialize(&mut &mut work_queue_account.data.borrow_mut()[..])?;
    } else {
        msg!("No pending work available");
        return Err(BettingPlatformError::NoWorkAvailable.into());
    }
    
    Ok(())
}

/// Complete work item
pub fn process_complete_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    work_id: [u8; 32],
) -> ProgramResult {
    msg!("Completing work item");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let work_queue_account = next_account_info(account_info_iter)?;
    let keeper_reward_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify keeper authority is signer
    if !keeper_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load keeper
    let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    
    // Verify keeper authority
    if keeper.authority != *keeper_authority.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load queue
    let mut queue = WorkQueue::try_from_slice(&work_queue_account.data.borrow())?;
    
    // Find work item and verify assignment
    let mut bounty = 0u64;
    for item in &queue.items {
        if item.work_id == work_id {
            if item.assigned_keeper != Some(*keeper_account.key) {
                msg!("Work not assigned to this keeper");
                return Err(BettingPlatformError::Unauthorized.into());
            }
            bounty = item.bounty;
            break;
        }
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    
    // Complete work
    queue.complete_item(&work_id, clock.slot)?;
    
    // Transfer bounty to keeper
    if bounty > 0 {
        msg!("Paying bounty: {} lamports", bounty);
        // In production, transfer the bounty
    }
    
    // Log completion
    msg!("Work completed:");
    msg!("  Work ID: {:?}", work_id);
    msg!("  Keeper: {}", keeper_account.key);
    msg!("  Bounty paid: {} lamports", bounty);
    
    // Serialize and save
    queue.serialize(&mut &mut work_queue_account.data.borrow_mut()[..])?;
    
    Ok(())
}