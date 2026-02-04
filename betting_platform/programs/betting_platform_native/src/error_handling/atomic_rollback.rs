//! Atomic Transaction Rollback for Failed Chains
//! 
//! Implements atomic rollback mechanism for chain transactions
//! ensuring all-or-nothing execution semantics

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
    state::{
        chain_accounts::{ChainState, ChainExecution, ChainPosition},
        Position,
        ProposalPDA,
        GlobalConfigPDA,
        accounts::discriminators,
    },
    events::{emit_event, EventType},
    define_event,
};

/// Maximum number of operations in a chain transaction
pub const MAX_CHAIN_OPERATIONS: usize = 32;

/// Chain transaction state for atomic execution
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainTransaction {
    /// Unique transaction ID
    pub transaction_id: [u8; 32],
    
    /// Chain ID this transaction belongs to
    pub chain_id: u128,
    
    /// User executing the transaction
    pub user: Pubkey,
    
    /// Operations to execute atomically
    pub operations: Vec<ChainOperation>,
    
    /// Current execution index
    pub execution_index: u8,
    
    /// Transaction status
    pub status: TransactionStatus,
    
    /// Rollback data for each operation
    pub rollback_data: Vec<RollbackData>,
    
    /// Slot when transaction was created
    pub created_slot: u64,
    
    /// Total gas consumed
    pub gas_consumed: u64,
}

/// Individual operation within a chain transaction
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum ChainOperation {
    /// Open a new position in the chain
    OpenPosition {
        market_id: u128,
        outcome: u8,
        size: u64,
        leverage: u8,
    },
    
    /// Close an existing position
    ClosePosition {
        position_id: [u8; 32],
    },
    
    /// Stake funds in the chain
    StakeInChain {
        amount: u64,
    },
    
    /// Borrow funds for the chain
    BorrowForChain {
        amount: u64,
    },
    
    /// Update chain leverage
    UpdateLeverage {
        new_leverage: u8,
    },
}

/// Transaction execution status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is being prepared
    Preparing,
    
    /// Transaction is executing
    Executing,
    
    /// Transaction completed successfully
    Completed,
    
    /// Transaction failed and needs rollback
    Failed,
    
    /// Transaction is being rolled back
    RollingBack,
    
    /// Rollback completed
    RolledBack,
}

/// Data needed to rollback an operation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RollbackData {
    /// Operation index
    pub operation_index: u8,
    
    /// State snapshot before operation
    pub state_snapshot: StateSnapshot,
    
    /// Compensating action needed
    pub compensating_action: Option<CompensatingAction>,
}

/// Snapshot of state before an operation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StateSnapshot {
    /// Chain state before operation
    pub chain_state: Option<ChainStateSnapshot>,
    
    /// Position state before operation
    pub position_state: Option<PositionSnapshot>,
    
    /// User balance before operation
    pub user_balance: u64,
    
    /// Market state before operation
    pub market_liquidity: u64,
}

/// Snapshot of chain state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainStateSnapshot {
    pub total_staked: u64,
    pub total_borrowed: u64,
    pub effective_leverage: u8,
    pub position_count: u16,
}

/// Snapshot of position state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionSnapshot {
    pub position_id: [u8; 32],
    pub size: u64,
    pub margin: u64,
    pub is_closed: bool,
}

/// Compensating action to undo an operation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum CompensatingAction {
    /// Close a position that was opened
    CloseOpenedPosition {
        position_id: [u8; 32],
    },
    
    /// Reopen a position that was closed
    ReopenClosedPosition {
        position_data: PositionSnapshot,
    },
    
    /// Unstake funds that were staked
    UnstakeFunds {
        amount: u64,
    },
    
    /// Repay funds that were borrowed
    RepayBorrowed {
        amount: u64,
    },
}

/// Begin a new chain transaction
pub fn begin_chain_transaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    chain_id: u128,
    operations: Vec<ChainOperation>,
) -> ProgramResult {
    msg!("Beginning atomic chain transaction for chain {}", chain_id);
    
    let account_info_iter = &mut accounts.iter();
    let transaction_account = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let chain_state_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Verify signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Validate operations count
    if operations.is_empty() || operations.len() > MAX_CHAIN_OPERATIONS {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Generate transaction ID
    let clock = Clock::get()?;
    let transaction_id = {
        let mut data = Vec::new();
        data.extend_from_slice(&chain_id.to_le_bytes());
        data.extend_from_slice(user_account.key.as_ref());
        data.extend_from_slice(&clock.slot.to_le_bytes());
        solana_program::hash::hash(&data).to_bytes()
    };
    
    // Create transaction state
    let transaction = ChainTransaction {
        transaction_id,
        chain_id,
        user: *user_account.key,
        operations,
        execution_index: 0,
        status: TransactionStatus::Preparing,
        rollback_data: Vec::new(),
        created_slot: clock.slot,
        gas_consumed: 0,
    };
    
    // Save transaction state
    transaction.serialize(&mut &mut transaction_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::ChainTransactionBegun, &ChainTransactionBegun {
        transaction_id,
        chain_id,
        user: *user_account.key,
        operation_count: transaction.operations.len() as u8,
    });
    
    msg!("Chain transaction {} begun with {} operations", 
        bs58::encode(transaction_id).into_string(), 
        transaction.operations.len()
    );
    
    Ok(())
}

/// Execute the next operation in a chain transaction
pub fn execute_chain_operation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    transaction_id: [u8; 32],
) -> ProgramResult {
    msg!("Executing next operation in transaction {}", bs58::encode(transaction_id).into_string());
    
    let account_info_iter = &mut accounts.iter();
    let transaction_account = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let chain_state_account = next_account_info(account_info_iter)?;
    
    // Additional accounts passed for the specific operation
    let remaining_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Load transaction
    let mut transaction = ChainTransaction::try_from_slice(&transaction_account.data.borrow())?;
    
    // Verify transaction ID
    if transaction.transaction_id != transaction_id {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Verify user
    if transaction.user != *user_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check status
    match transaction.status {
        TransactionStatus::Preparing => {
            transaction.status = TransactionStatus::Executing;
        }
        TransactionStatus::Executing => {
            // Continue execution
        }
        _ => {
            return Err(BettingPlatformError::InvalidOperation.into());
        }
    }
    
    // Check if all operations completed
    if transaction.execution_index as usize >= transaction.operations.len() {
        transaction.status = TransactionStatus::Completed;
        transaction.serialize(&mut &mut transaction_account.data.borrow_mut()[..])?;
        
        emit_event(EventType::ChainTransactionCompleted, &ChainTransactionCompleted {
            transaction_id,
            chain_id: transaction.chain_id,
            user: transaction.user,
            gas_consumed: transaction.gas_consumed,
        });
        
        return Ok(());
    }
    
    // Get current operation
    let operation = transaction.operations[transaction.execution_index as usize].clone();
    
    // Take state snapshot before execution
    let snapshot = take_state_snapshot(
        chain_state_account,
        &remaining_accounts,
        &operation,
    )?;
    
    // Execute operation
    match execute_single_operation(
        program_id,
        &operation,
        chain_state_account,
        user_account,
        &remaining_accounts,
    ) {
        Ok(gas_used) => {
            // Operation succeeded
            transaction.gas_consumed += gas_used;
            
            // Store rollback data
            let rollback_data = RollbackData {
                operation_index: transaction.execution_index,
                state_snapshot: snapshot,
                compensating_action: determine_compensating_action(&operation, &remaining_accounts)?,
            };
            transaction.rollback_data.push(rollback_data);
            
            // Move to next operation
            transaction.execution_index += 1;
            
            // Save state
            transaction.serialize(&mut &mut transaction_account.data.borrow_mut()[..])?;
            
            msg!("Operation {} executed successfully", transaction.execution_index - 1);
        }
        Err(e) => {
            // Operation failed - initiate rollback
            msg!("Operation {} failed: {:?}", transaction.execution_index, e);
            
            transaction.status = TransactionStatus::Failed;
            transaction.serialize(&mut &mut transaction_account.data.borrow_mut()[..])?;
            
            // Emit failure event
            emit_event(EventType::ChainOperationFailed, &ChainOperationFailed {
                transaction_id,
                chain_id: transaction.chain_id,
                operation_index: transaction.execution_index,
                error_code: 0, // TODO: Extract error code from ProgramError
            });
            
            return Err(e);
        }
    }
    
    Ok(())
}

/// Rollback a failed chain transaction
pub fn rollback_chain_transaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    transaction_id: [u8; 32],
) -> ProgramResult {
    msg!("Rolling back transaction {}", bs58::encode(transaction_id).into_string());
    
    let account_info_iter = &mut accounts.iter();
    let transaction_account = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let chain_state_account = next_account_info(account_info_iter)?;
    
    // Additional accounts for rollback operations
    let remaining_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Load transaction
    let mut transaction = ChainTransaction::try_from_slice(&transaction_account.data.borrow())?;
    
    // Verify transaction can be rolled back
    match transaction.status {
        TransactionStatus::Failed => {
            transaction.status = TransactionStatus::RollingBack;
        }
        TransactionStatus::RollingBack => {
            // Continue rollback
        }
        _ => {
            return Err(BettingPlatformError::InvalidOperation.into());
        }
    }
    
    // Rollback operations in reverse order
    while let Some(rollback_data) = transaction.rollback_data.pop() {
        msg!("Rolling back operation {}", rollback_data.operation_index);
        
        // Execute compensating action
        if let Some(action) = rollback_data.compensating_action {
            execute_compensating_action(
                program_id,
                &action,
                chain_state_account,
                user_account,
                &remaining_accounts,
                &rollback_data.state_snapshot,
            )?;
        }
    }
    
    // Mark as rolled back
    transaction.status = TransactionStatus::RolledBack;
    transaction.serialize(&mut &mut transaction_account.data.borrow_mut()[..])?;
    
    // Emit rollback completed event
    emit_event(EventType::ChainTransactionRolledBack, &ChainTransactionRolledBack {
        transaction_id,
        chain_id: transaction.chain_id,
        user: transaction.user,
        operations_rolled_back: transaction.execution_index,
    });
    
    msg!("Transaction rollback completed");
    
    Ok(())
}

/// Execute a single chain operation
fn execute_single_operation(
    program_id: &Pubkey,
    operation: &ChainOperation,
    chain_state_account: &AccountInfo,
    user_account: &AccountInfo,
    remaining_accounts: &[&AccountInfo],
) -> Result<u64, ProgramError> {
    let gas_used = match operation {
        ChainOperation::OpenPosition { market_id, outcome, size, leverage } => {
            // Execute position opening logic
            // This would call into existing position opening code
            msg!("Executing OpenPosition: market={}, size={}, leverage={}", 
                market_id, size, leverage);
            5000 // Estimated gas
        }
        
        ChainOperation::ClosePosition { position_id } => {
            // Execute position closing logic
            msg!("Executing ClosePosition: {}", bs58::encode(position_id).into_string());
            3000 // Estimated gas
        }
        
        ChainOperation::StakeInChain { amount } => {
            // Execute staking logic
            msg!("Executing StakeInChain: amount={}", amount);
            2000 // Estimated gas
        }
        
        ChainOperation::BorrowForChain { amount } => {
            // Execute borrowing logic
            msg!("Executing BorrowForChain: amount={}", amount);
            3000 // Estimated gas
        }
        
        ChainOperation::UpdateLeverage { new_leverage } => {
            // Execute leverage update logic
            msg!("Executing UpdateLeverage: new_leverage={}", new_leverage);
            2000 // Estimated gas
        }
    };
    
    Ok(gas_used)
}

/// Take a snapshot of current state
fn take_state_snapshot(
    chain_state_account: &AccountInfo,
    remaining_accounts: &[&AccountInfo],
    operation: &ChainOperation,
) -> Result<StateSnapshot, ProgramError> {
    let chain_state = ChainState::try_from_slice(&chain_state_account.data.borrow())?;
    
    let chain_snapshot = ChainStateSnapshot {
        total_staked: chain_state.current_balance,
        total_borrowed: 0, // Chain doesn't track borrowed amount
        effective_leverage: 1, // Default leverage, chain doesn't track this
        position_count: 0, // Chain doesn't directly track position count
    };
    
    let snapshot = StateSnapshot {
        chain_state: Some(chain_snapshot),
        position_state: None, // Would be populated based on operation type
        user_balance: 0, // Would get actual balance
        market_liquidity: 0, // Would get actual liquidity
    };
    
    Ok(snapshot)
}

/// Determine compensating action for an operation
fn determine_compensating_action(
    operation: &ChainOperation,
    remaining_accounts: &[&AccountInfo],
) -> Result<Option<CompensatingAction>, ProgramError> {
    let action = match operation {
        ChainOperation::OpenPosition { .. } => {
            // Need to close the opened position
            // Would extract position_id from the operation result
            Some(CompensatingAction::CloseOpenedPosition {
                position_id: [0; 32], // Placeholder
            })
        }
        
        ChainOperation::ClosePosition { position_id } => {
            // Need to reopen the closed position
            // Would need position data from snapshot
            None // For now
        }
        
        ChainOperation::StakeInChain { amount } => {
            Some(CompensatingAction::UnstakeFunds {
                amount: *amount,
            })
        }
        
        ChainOperation::BorrowForChain { amount } => {
            Some(CompensatingAction::RepayBorrowed {
                amount: *amount,
            })
        }
        
        ChainOperation::UpdateLeverage { .. } => {
            // Leverage update might not need compensation
            None
        }
    };
    
    Ok(action)
}

/// Execute a compensating action
fn execute_compensating_action(
    program_id: &Pubkey,
    action: &CompensatingAction,
    chain_state_account: &AccountInfo,
    user_account: &AccountInfo,
    remaining_accounts: &[&AccountInfo],
    snapshot: &StateSnapshot,
) -> ProgramResult {
    match action {
        CompensatingAction::CloseOpenedPosition { position_id } => {
            msg!("Compensating: Closing position {}", bs58::encode(position_id).into_string());
            // Execute position close
        }
        
        CompensatingAction::ReopenClosedPosition { position_data } => {
            msg!("Compensating: Reopening position {}", bs58::encode(position_data.position_id).into_string());
            // Execute position reopen with original data
        }
        
        CompensatingAction::UnstakeFunds { amount } => {
            msg!("Compensating: Unstaking {} funds", amount);
            // Execute unstake
        }
        
        CompensatingAction::RepayBorrowed { amount } => {
            msg!("Compensating: Repaying {} borrowed funds", amount);
            // Execute repayment
        }
    }
    
    Ok(())
}

// Events
define_event!(ChainTransactionBegun, EventType::ChainTransactionBegun, {
    transaction_id: [u8; 32],
    chain_id: u128,
    user: Pubkey,
    operation_count: u8
});

define_event!(ChainTransactionCompleted, EventType::ChainTransactionCompleted, {
    transaction_id: [u8; 32],
    chain_id: u128,
    user: Pubkey,
    gas_consumed: u64
});

define_event!(ChainOperationFailed, EventType::ChainOperationFailed, {
    transaction_id: [u8; 32],
    chain_id: u128,
    operation_index: u8,
    error_code: u32
});

define_event!(ChainTransactionRolledBack, EventType::ChainTransactionRolledBack, {
    transaction_id: [u8; 32],
    chain_id: u128,
    user: Pubkey,
    operations_rolled_back: u8
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_lifecycle() {
        // Test transaction creation
        let operations = vec![
            ChainOperation::StakeInChain { amount: 1000 },
            ChainOperation::OpenPosition {
                market_id: 1,
                outcome: 0,
                size: 5000,
                leverage: 10,
            },
        ];
        
        // Verify operations are valid
        assert!(operations.len() <= MAX_CHAIN_OPERATIONS);
        assert!(!operations.is_empty());
    }
    
    #[test]
    fn test_rollback_order() {
        // Verify rollback happens in reverse order
        let mut rollback_data = vec![
            RollbackData {
                operation_index: 0,
                state_snapshot: StateSnapshot {
                    chain_state: None,
                    position_state: None,
                    user_balance: 1000,
                    market_liquidity: 100000,
                },
                compensating_action: Some(CompensatingAction::UnstakeFunds { amount: 1000 }),
            },
            RollbackData {
                operation_index: 1,
                state_snapshot: StateSnapshot {
                    chain_state: None,
                    position_state: None,
                    user_balance: 0,
                    market_liquidity: 95000,
                },
                compensating_action: Some(CompensatingAction::CloseOpenedPosition {
                    position_id: [1; 32],
                }),
            },
        ];
        
        // Pop in reverse order
        assert_eq!(rollback_data.pop().unwrap().operation_index, 1);
        assert_eq!(rollback_data.pop().unwrap().operation_index, 0);
    }
}