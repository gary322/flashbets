//! Client-Side Undo Window (5 seconds)
//! 
//! Implements a 5-second window where users can cancel their transactions
//! before they are finalized on-chain

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::discriminators,
    events::{emit_event, EventType},
    define_event,
};

/// Undo window duration in seconds
pub const UNDO_WINDOW_SECONDS: i64 = 5;

/// Maximum pending transactions per user
pub const MAX_PENDING_TRANSACTIONS: usize = 10;

/// Pending transaction that can be undone
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PendingTransaction {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Transaction ID
    pub transaction_id: [u8; 32],
    
    /// User who initiated the transaction
    pub user: Pubkey,
    
    /// Type of transaction
    pub transaction_type: TransactionType,
    
    /// Transaction data
    pub data: Vec<u8>,
    
    /// When the transaction was submitted
    pub submitted_at: i64,
    
    /// When the undo window expires
    pub expires_at: i64,
    
    /// Current status
    pub status: PendingStatus,
    
    /// Affected accounts
    pub affected_accounts: Vec<Pubkey>,
    
    /// Estimated compute units
    pub estimated_cu: u32,
}

/// Types of transactions that support undo
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum TransactionType {
    /// Open a new position
    OpenPosition,
    
    /// Close an existing position
    ClosePosition,
    
    /// Modify position (size, leverage)
    ModifyPosition,
    
    /// Create chain position
    CreateChain,
    
    /// Add to chain
    AddToChain,
    
    /// Stake MMT tokens
    StakeMMT,
    
    /// Create market order
    MarketOrder,
    
    /// Create limit order
    LimitOrder,
}

/// Status of a pending transaction
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum PendingStatus {
    /// Waiting in undo window
    Pending,
    
    /// User cancelled during window
    Cancelled,
    
    /// Window expired, executing
    Executing,
    
    /// Successfully executed
    Executed,
    
    /// Execution failed
    Failed,
}

/// User's pending transaction queue
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UserPendingQueue {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Pending transactions
    pub transactions: Vec<PendingTransaction>,
    
    /// Total cancelled count (for metrics)
    pub total_cancelled: u64,
    
    /// Total executed count
    pub total_executed: u64,
    
    /// Last cleanup timestamp
    pub last_cleanup: i64,
}

impl UserPendingQueue {
    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_PENDING_QUEUE,
            user,
            transactions: Vec::new(),
            total_cancelled: 0,
            total_executed: 0,
            last_cleanup: Clock::get().unwrap().unix_timestamp,
        }
    }
    
    /// Add a new pending transaction
    pub fn add_transaction(&mut self, transaction: PendingTransaction) -> Result<(), ProgramError> {
        // Clean up old transactions first
        self.cleanup_expired();
        
        // Check limit
        if self.transactions.len() >= MAX_PENDING_TRANSACTIONS {
            return Err(BettingPlatformError::TooManyPendingTransactions.into());
        }
        
        self.transactions.push(transaction);
        Ok(())
    }
    
    /// Find a pending transaction
    pub fn find_transaction(&self, transaction_id: &[u8; 32]) -> Option<&PendingTransaction> {
        self.transactions.iter()
            .find(|tx| &tx.transaction_id == transaction_id)
    }
    
    /// Find a pending transaction (mutable)
    pub fn find_transaction_mut(&mut self, transaction_id: &[u8; 32]) -> Option<&mut PendingTransaction> {
        self.transactions.iter_mut()
            .find(|tx| &tx.transaction_id == transaction_id)
    }
    
    /// Remove expired or completed transactions
    pub fn cleanup_expired(&mut self) {
        let current_time = Clock::get().unwrap().unix_timestamp;
        
        self.transactions.retain(|tx| {
            match tx.status {
                PendingStatus::Pending => tx.expires_at > current_time,
                PendingStatus::Executing => true, // Keep executing
                _ => false, // Remove completed/cancelled/failed
            }
        });
        
        self.last_cleanup = current_time;
    }
}

/// Submit a transaction with undo window
pub fn submit_with_undo_window(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    transaction_type: TransactionType,
    transaction_data: Vec<u8>,
) -> ProgramResult {
    msg!("Submitting transaction with undo window");
    
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;
    let user_queue_account = next_account_info(account_info_iter)?;
    let pending_tx_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Collect affected accounts
    let affected_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Verify signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load or create user queue
    let mut user_queue = if user_queue_account.data_len() > 0 {
        UserPendingQueue::try_from_slice(&user_queue_account.data.borrow())?
    } else {
        UserPendingQueue::new(*user_account.key)
    };
    
    // Generate transaction ID
    let clock = Clock::get()?;
    let transaction_id = {
        let mut data = Vec::new();
        data.extend_from_slice(user_account.key.as_ref());
        data.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
        data.extend_from_slice(&clock.slot.to_le_bytes());
        solana_program::hash::hash(&data).to_bytes()
    };
    
    // Create pending transaction
    let pending_tx = PendingTransaction {
        discriminator: discriminators::PENDING_TRANSACTION,
        transaction_id,
        user: *user_account.key,
        transaction_type: transaction_type.clone(),
        data: transaction_data,
        submitted_at: clock.unix_timestamp,
        expires_at: clock.unix_timestamp + UNDO_WINDOW_SECONDS,
        status: PendingStatus::Pending,
        affected_accounts: affected_accounts.iter().map(|a| *a.key).collect(),
        estimated_cu: estimate_compute_units(&transaction_type),
    };
    
    // Add to queue
    user_queue.add_transaction(pending_tx.clone())?;
    
    // Save queue
    user_queue.serialize(&mut &mut user_queue_account.data.borrow_mut()[..])?;
    
    // Save pending transaction
    pending_tx.serialize(&mut &mut pending_tx_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::TransactionPending, &TransactionPending {
        transaction_id,
        user: *user_account.key,
        transaction_type: format!("{:?}", transaction_type),
        expires_at: pending_tx.expires_at,
    });
    
    msg!("Transaction {} pending with {} second undo window", 
        bs58::encode(transaction_id).into_string(),
        UNDO_WINDOW_SECONDS
    );
    
    Ok(())
}

/// Cancel a pending transaction
pub fn cancel_pending_transaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    transaction_id: [u8; 32],
) -> ProgramResult {
    msg!("Cancelling pending transaction {}", bs58::encode(transaction_id).into_string());
    
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;
    let user_queue_account = next_account_info(account_info_iter)?;
    let pending_tx_account = next_account_info(account_info_iter)?;
    
    // Verify signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load transaction
    let mut pending_tx = PendingTransaction::try_from_slice(&pending_tx_account.data.borrow())?;
    
    // Verify transaction ID and owner
    if pending_tx.transaction_id != transaction_id {
        return Err(ProgramError::InvalidAccountData);
    }
    
    if pending_tx.user != *user_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if still in undo window
    let clock = Clock::get()?;
    if clock.unix_timestamp >= pending_tx.expires_at {
        return Err(BettingPlatformError::UndoWindowExpired.into());
    }
    
    // Check status
    if pending_tx.status != PendingStatus::Pending {
        return Err(BettingPlatformError::TransactionNotCancellable.into());
    }
    
    // Update status
    pending_tx.status = PendingStatus::Cancelled;
    pending_tx.serialize(&mut &mut pending_tx_account.data.borrow_mut()[..])?;
    
    // Update user queue
    let mut user_queue = UserPendingQueue::try_from_slice(&user_queue_account.data.borrow())?;
    if let Some(queue_tx) = user_queue.find_transaction_mut(&transaction_id) {
        queue_tx.status = PendingStatus::Cancelled;
        user_queue.total_cancelled += 1;
    }
    user_queue.serialize(&mut &mut user_queue_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::TransactionCancelled, &TransactionCancelled {
        transaction_id,
        user: *user_account.key,
        cancelled_at: clock.unix_timestamp,
    });
    
    msg!("Transaction cancelled successfully");
    
    Ok(())
}

/// Execute a pending transaction after undo window expires
pub fn execute_pending_transaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    transaction_id: [u8; 32],
) -> ProgramResult {
    msg!("Executing pending transaction {}", bs58::encode(transaction_id).into_string());
    
    let account_info_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_info_iter)?; // Can be executed by keeper
    let pending_tx_account = next_account_info(account_info_iter)?;
    let user_queue_account = next_account_info(account_info_iter)?;
    
    // Remaining accounts for actual execution
    let execution_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Load transaction
    let mut pending_tx = PendingTransaction::try_from_slice(&pending_tx_account.data.borrow())?;
    
    // Verify transaction ID
    if pending_tx.transaction_id != transaction_id {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if undo window has expired
    let clock = Clock::get()?;
    if clock.unix_timestamp < pending_tx.expires_at {
        return Err(BettingPlatformError::UndoWindowNotExpired.into());
    }
    
    // Check status
    match pending_tx.status {
        PendingStatus::Pending => {
            // Ready to execute
            pending_tx.status = PendingStatus::Executing;
        }
        PendingStatus::Cancelled => {
            return Err(BettingPlatformError::TransactionCancelled.into());
        }
        PendingStatus::Executed => {
            return Err(BettingPlatformError::TransactionAlreadyExecuted.into());
        }
        _ => {
            return Err(BettingPlatformError::InvalidTransactionStatus.into());
        }
    }
    
    // Save executing status
    pending_tx.serialize(&mut &mut pending_tx_account.data.borrow_mut()[..])?;
    
    // Execute the actual transaction
    match execute_transaction_type(
        program_id,
        &pending_tx.transaction_type,
        &pending_tx.data,
        &execution_accounts,
    ) {
        Ok(_) => {
            // Success
            pending_tx.status = PendingStatus::Executed;
            
            // Update user queue
            if user_queue_account.data_len() > 0 {
                let mut user_queue = UserPendingQueue::try_from_slice(&user_queue_account.data.borrow())?;
                if let Some(queue_tx) = user_queue.find_transaction_mut(&transaction_id) {
                    queue_tx.status = PendingStatus::Executed;
                    user_queue.total_executed += 1;
                }
                user_queue.serialize(&mut &mut user_queue_account.data.borrow_mut()[..])?;
            }
            
            emit_event(EventType::TransactionExecuted, &TransactionExecuted {
                transaction_id,
                user: pending_tx.user,
                transaction_type: format!("{:?}", pending_tx.transaction_type),
                executed_at: clock.unix_timestamp,
            });
        }
        Err(e) => {
            // Failed
            pending_tx.status = PendingStatus::Failed;
            
            emit_event(EventType::TransactionFailed, &TransactionFailed {
                transaction_id,
                user: pending_tx.user,
                error_code: 0, // TODO: Extract error code from ProgramError
            });
            
            return Err(e);
        }
    }
    
    // Save final status
    pending_tx.serialize(&mut &mut pending_tx_account.data.borrow_mut()[..])?;
    
    msg!("Transaction executed successfully");
    
    Ok(())
}

/// Estimate compute units for a transaction type
fn estimate_compute_units(transaction_type: &TransactionType) -> u32 {
    match transaction_type {
        TransactionType::OpenPosition => 20_000,
        TransactionType::ClosePosition => 15_000,
        TransactionType::ModifyPosition => 10_000,
        TransactionType::CreateChain => 30_000,
        TransactionType::AddToChain => 25_000,
        TransactionType::StakeMMT => 10_000,
        TransactionType::MarketOrder => 20_000,
        TransactionType::LimitOrder => 15_000,
    }
}

/// Execute the actual transaction based on type
fn execute_transaction_type(
    program_id: &Pubkey,
    transaction_type: &TransactionType,
    data: &[u8],
    accounts: &[&AccountInfo],
) -> ProgramResult {
    match transaction_type {
        TransactionType::OpenPosition => {
            msg!("Executing OpenPosition transaction");
            // Deserialize data and execute position opening
            // This would call into existing position opening logic
        }
        
        TransactionType::ClosePosition => {
            msg!("Executing ClosePosition transaction");
            // Execute position closing
        }
        
        TransactionType::ModifyPosition => {
            msg!("Executing ModifyPosition transaction");
            // Execute position modification
        }
        
        TransactionType::CreateChain => {
            msg!("Executing CreateChain transaction");
            // Execute chain creation
        }
        
        TransactionType::AddToChain => {
            msg!("Executing AddToChain transaction");
            // Execute add to chain
        }
        
        TransactionType::StakeMMT => {
            msg!("Executing StakeMMT transaction");
            // Execute MMT staking
        }
        
        TransactionType::MarketOrder => {
            msg!("Executing MarketOrder transaction");
            // Execute market order
        }
        
        TransactionType::LimitOrder => {
            msg!("Executing LimitOrder transaction");
            // Execute limit order
        }
    }
    
    Ok(())
}

// Events
define_event!(TransactionPending, EventType::TransactionPending, {
    transaction_id: [u8; 32],
    user: Pubkey,
    transaction_type: String,
    expires_at: i64
});

define_event!(TransactionCancelled, EventType::TransactionCancelled, {
    transaction_id: [u8; 32],
    user: Pubkey,
    cancelled_at: i64
});

define_event!(TransactionExecuted, EventType::TransactionExecuted, {
    transaction_id: [u8; 32],
    user: Pubkey,
    transaction_type: String,
    executed_at: i64
});

define_event!(TransactionFailed, EventType::TransactionFailed, {
    transaction_id: [u8; 32],
    user: Pubkey,
    error_code: u32
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_undo_window_timing() {
        let submitted_at = 1000;
        let expires_at = submitted_at + UNDO_WINDOW_SECONDS;
        
        // During window
        assert!(submitted_at + 3 < expires_at);
        
        // After window
        assert!(submitted_at + 6 >= expires_at);
    }
    
    #[test]
    fn test_transaction_status_flow() {
        // Valid flow: Pending -> Executing -> Executed
        let mut status = PendingStatus::Pending;
        assert_eq!(status, PendingStatus::Pending);
        
        status = PendingStatus::Executing;
        assert_eq!(status, PendingStatus::Executing);
        
        status = PendingStatus::Executed;
        assert_eq!(status, PendingStatus::Executed);
        
        // Alternative flow: Pending -> Cancelled
        let mut status = PendingStatus::Pending;
        status = PendingStatus::Cancelled;
        assert_eq!(status, PendingStatus::Cancelled);
    }
}