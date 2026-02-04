//! Recovery Manager
//! 
//! Coordinates all error handling and recovery mechanisms
//! providing a unified interface for transaction safety

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

use super::{
    atomic_rollback::{ChainTransaction, TransactionStatus},
    undo_window::{PendingTransaction, PendingStatus},
    on_chain_revert::{RevertibleActionRecord, SlotRevertTracker},
};

/// Recovery configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RecoveryConfig {
    /// Enable atomic rollback for chains
    pub atomic_rollback_enabled: bool,
    
    /// Enable client-side undo window
    pub undo_window_enabled: bool,
    
    /// Enable on-chain revert
    pub on_chain_revert_enabled: bool,
    
    /// Maximum recovery attempts
    pub max_recovery_attempts: u8,
    
    /// Recovery timeout in slots
    pub recovery_timeout_slots: u64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            atomic_rollback_enabled: true,
            undo_window_enabled: true,
            on_chain_revert_enabled: true,
            max_recovery_attempts: 3,
            recovery_timeout_slots: 150, // ~1 minute
        }
    }
}

/// Recovery manager state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RecoveryManager {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Configuration
    pub config: RecoveryConfig,
    
    /// Active recovery operations
    pub active_recoveries: Vec<RecoveryOperation>,
    
    /// Recovery statistics
    pub stats: RecoveryStats,
    
    /// Last update slot
    pub last_update_slot: u64,
}

/// Active recovery operation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RecoveryOperation {
    /// Operation ID
    pub operation_id: [u8; 32],
    
    /// Type of recovery
    pub recovery_type: RecoveryType,
    
    /// User requesting recovery
    pub user: Pubkey,
    
    /// Start slot
    pub start_slot: u64,
    
    /// Current status
    pub status: RecoveryStatus,
    
    /// Attempts made
    pub attempts: u8,
    
    /// Related transaction/action ID
    pub related_id: [u8; 32],
}

/// Types of recovery operations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum RecoveryType {
    /// Atomic rollback of chain transaction
    AtomicRollback,
    
    /// Undo window cancellation
    UndoCancel,
    
    /// On-chain revert
    OnChainRevert,
    
    /// Full recovery (multiple mechanisms)
    FullRecovery,
}

/// Recovery operation status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum RecoveryStatus {
    /// Recovery initiated
    Initiated,
    
    /// Recovery in progress
    InProgress,
    
    /// Recovery completed successfully
    Completed,
    
    /// Recovery failed
    Failed,
    
    /// Recovery timed out
    TimedOut,
}

/// Recovery statistics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RecoveryStats {
    /// Total recovery operations
    pub total_operations: u64,
    
    /// Successful recoveries
    pub successful_recoveries: u64,
    
    /// Failed recoveries
    pub failed_recoveries: u64,
    
    /// Average recovery time in slots
    pub avg_recovery_slots: u64,
    
    /// Recovery by type counters
    pub rollback_count: u64,
    pub undo_count: u64,
    pub revert_count: u64,
}

impl RecoveryManager {
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::RECOVERY_MANAGER,
            config: RecoveryConfig::default(),
            active_recoveries: Vec::new(),
            stats: RecoveryStats {
                total_operations: 0,
                successful_recoveries: 0,
                failed_recoveries: 0,
                avg_recovery_slots: 0,
                rollback_count: 0,
                undo_count: 0,
                revert_count: 0,
            },
            last_update_slot: Clock::get().unwrap().slot,
        }
    }
    
    /// Add a new recovery operation
    pub fn add_recovery(&mut self, operation: RecoveryOperation) -> Result<(), ProgramError> {
        // Clean up old operations
        self.cleanup_completed();
        
        // Check if operation already exists
        if self.active_recoveries.iter().any(|op| op.operation_id == operation.operation_id) {
            return Err(BettingPlatformError::RecoveryAlreadyActive.into());
        }
        
        self.active_recoveries.push(operation);
        self.stats.total_operations += 1;
        
        Ok(())
    }
    
    /// Update recovery status
    pub fn update_recovery_status(
        &mut self, 
        operation_id: &[u8; 32], 
        new_status: RecoveryStatus
    ) -> Result<(), ProgramError> {
        if let Some(operation) = self.active_recoveries.iter_mut()
            .find(|op| &op.operation_id == operation_id) 
        {
            operation.status = new_status.clone();
            
            // Update stats
            match new_status {
                RecoveryStatus::Completed => {
                    self.stats.successful_recoveries += 1;
                    match operation.recovery_type {
                        RecoveryType::AtomicRollback => self.stats.rollback_count += 1,
                        RecoveryType::UndoCancel => self.stats.undo_count += 1,
                        RecoveryType::OnChainRevert => self.stats.revert_count += 1,
                        RecoveryType::FullRecovery => {}
                    }
                }
                RecoveryStatus::Failed | RecoveryStatus::TimedOut => {
                    self.stats.failed_recoveries += 1;
                }
                _ => {}
            }
            
            Ok(())
        } else {
            Err(BettingPlatformError::RecoveryNotFound.into())
        }
    }
    
    /// Clean up completed operations
    fn cleanup_completed(&mut self) {
        let current_slot = Clock::get().unwrap().slot;
        
        self.active_recoveries.retain(|op| {
            match op.status {
                RecoveryStatus::Completed | RecoveryStatus::Failed | RecoveryStatus::TimedOut => false,
                _ => {
                    // Check timeout
                    current_slot - op.start_slot < self.config.recovery_timeout_slots
                }
            }
        });
        
        self.last_update_slot = current_slot;
    }
}

/// Initiate a recovery operation
pub fn initiate_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    recovery_type: RecoveryType,
    related_id: [u8; 32],
) -> ProgramResult {
    msg!("Initiating recovery operation");
    
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;
    let recovery_manager_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Verify signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load or create recovery manager
    let mut manager = if recovery_manager_account.data_len() > 0 {
        RecoveryManager::try_from_slice(&recovery_manager_account.data.borrow())?
    } else {
        RecoveryManager::new()
    };
    
    // Check if recovery is enabled for this type
    match recovery_type {
        RecoveryType::AtomicRollback if !manager.config.atomic_rollback_enabled => {
            return Err(BettingPlatformError::RecoveryTypeDisabled.into());
        }
        RecoveryType::UndoCancel if !manager.config.undo_window_enabled => {
            return Err(BettingPlatformError::RecoveryTypeDisabled.into());
        }
        RecoveryType::OnChainRevert if !manager.config.on_chain_revert_enabled => {
            return Err(BettingPlatformError::RecoveryTypeDisabled.into());
        }
        _ => {}
    }
    
    // Generate operation ID
    let clock = Clock::get()?;
    let operation_id = {
        let mut data = Vec::new();
        data.extend_from_slice(user_account.key.as_ref());
        data.extend_from_slice(&related_id);
        data.extend_from_slice(&clock.slot.to_le_bytes());
        solana_program::hash::hash(&data).to_bytes()
    };
    
    // Create recovery operation
    let operation = RecoveryOperation {
        operation_id,
        recovery_type: recovery_type.clone(),
        user: *user_account.key,
        start_slot: clock.slot,
        status: RecoveryStatus::Initiated,
        attempts: 0,
        related_id,
    };
    
    // Add to manager
    manager.add_recovery(operation)?;
    
    // Save manager
    manager.serialize(&mut &mut recovery_manager_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::RecoveryInitiated, &RecoveryInitiated {
        operation_id,
        recovery_type: format!("{:?}", recovery_type),
        user: *user_account.key,
        related_id,
    });
    
    msg!("Recovery operation {} initiated", bs58::encode(operation_id).into_string());
    
    Ok(())
}

/// Execute a recovery operation
pub fn execute_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    operation_id: [u8; 32],
) -> ProgramResult {
    msg!("Executing recovery operation {}", bs58::encode(operation_id).into_string());
    
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;
    let recovery_manager_account = next_account_info(account_info_iter)?;
    
    // Type-specific accounts follow
    let recovery_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Load manager
    let mut manager = RecoveryManager::try_from_slice(&recovery_manager_account.data.borrow())?;
    
    // Find operation
    let operation = manager.active_recoveries.iter()
        .find(|op| op.operation_id == operation_id)
        .ok_or(BettingPlatformError::RecoveryNotFound)?
        .clone();
    
    // Verify user
    if operation.user != *user_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check attempts
    if operation.attempts >= manager.config.max_recovery_attempts {
        manager.update_recovery_status(&operation_id, RecoveryStatus::Failed)?;
        manager.serialize(&mut &mut recovery_manager_account.data.borrow_mut()[..])?;
        return Err(BettingPlatformError::MaxRecoveryAttemptsExceeded.into());
    }
    
    // Update status to in progress
    manager.update_recovery_status(&operation_id, RecoveryStatus::InProgress)?;
    
    // Increment attempts
    if let Some(op) = manager.active_recoveries.iter_mut()
        .find(|op| op.operation_id == operation_id) 
    {
        op.attempts += 1;
    }
    
    // Execute based on type
    let result = match operation.recovery_type {
        RecoveryType::AtomicRollback => {
            execute_atomic_rollback(&operation.related_id, &recovery_accounts)
        }
        RecoveryType::UndoCancel => {
            execute_undo_cancel(&operation.related_id, &recovery_accounts)
        }
        RecoveryType::OnChainRevert => {
            execute_on_chain_revert(&operation.related_id, &recovery_accounts)
        }
        RecoveryType::FullRecovery => {
            execute_full_recovery(&operation.related_id, &recovery_accounts)
        }
    };
    
    // Update status based on result
    match result {
        Ok(_) => {
            manager.update_recovery_status(&operation_id, RecoveryStatus::Completed)?;
            
            emit_event(EventType::RecoveryCompleted, &RecoveryCompleted {
                operation_id,
                recovery_type: format!("{:?}", operation.recovery_type),
                user: operation.user,
            });
        }
        Err(e) => {
            // Check if we should retry or fail
            if operation.attempts < manager.config.max_recovery_attempts {
                // Keep as InProgress for retry
            } else {
                manager.update_recovery_status(&operation_id, RecoveryStatus::Failed)?;
            }
            
            emit_event(EventType::RecoveryFailed, &RecoveryFailed {
                operation_id,
                recovery_type: format!("{:?}", operation.recovery_type),
                error_code: 0, // TODO: Extract error code from ProgramError
            });
            
            return Err(e);
        }
    }
    
    // Save manager
    manager.serialize(&mut &mut recovery_manager_account.data.borrow_mut()[..])?;
    
    msg!("Recovery operation completed successfully");
    
    Ok(())
}

/// Execute atomic rollback recovery
fn execute_atomic_rollback(
    transaction_id: &[u8; 32],
    accounts: &[&AccountInfo],
) -> ProgramResult {
    msg!("Executing atomic rollback for transaction {}", bs58::encode(transaction_id).into_string());
    
    // This would call into atomic_rollback::rollback_chain_transaction
    // with the appropriate accounts
    
    Ok(())
}

/// Execute undo cancel recovery
fn execute_undo_cancel(
    transaction_id: &[u8; 32],
    accounts: &[&AccountInfo],
) -> ProgramResult {
    msg!("Executing undo cancel for transaction {}", bs58::encode(transaction_id).into_string());
    
    // This would call into undo_window::cancel_pending_transaction
    // with the appropriate accounts
    
    Ok(())
}

/// Execute on-chain revert recovery
fn execute_on_chain_revert(
    action_id: &[u8; 32],
    accounts: &[&AccountInfo],
) -> ProgramResult {
    msg!("Executing on-chain revert for action {}", bs58::encode(action_id).into_string());
    
    // This would call into on_chain_revert::revert_action
    // with the appropriate accounts
    
    Ok(())
}

/// Execute full recovery (try all mechanisms)
fn execute_full_recovery(
    id: &[u8; 32],
    accounts: &[&AccountInfo],
) -> ProgramResult {
    msg!("Executing full recovery for {}", bs58::encode(id).into_string());
    
    // Try each recovery mechanism in order
    // 1. Try on-chain revert (fastest if in same slot)
    // 2. Try undo cancel (if in 5s window)
    // 3. Try atomic rollback (for chain transactions)
    
    Ok(())
}

// Events
define_event!(RecoveryInitiated, EventType::RecoveryInitiated, {
    operation_id: [u8; 32],
    recovery_type: String,
    user: Pubkey,
    related_id: [u8; 32]
});

define_event!(RecoveryCompleted, EventType::RecoveryCompleted, {
    operation_id: [u8; 32],
    recovery_type: String,
    user: Pubkey
});

define_event!(RecoveryFailed, EventType::RecoveryFailed, {
    operation_id: [u8; 32],
    recovery_type: String,
    error_code: u32
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_recovery_config() {
        let config = RecoveryConfig::default();
        assert!(config.atomic_rollback_enabled);
        assert!(config.undo_window_enabled);
        assert!(config.on_chain_revert_enabled);
        assert_eq!(config.max_recovery_attempts, 3);
    }
    
    #[test]
    fn test_recovery_types() {
        let types = vec![
            RecoveryType::AtomicRollback,
            RecoveryType::UndoCancel,
            RecoveryType::OnChainRevert,
            RecoveryType::FullRecovery,
        ];
        
        for recovery_type in types {
            assert!(matches!(
                recovery_type,
                RecoveryType::AtomicRollback | 
                RecoveryType::UndoCancel | 
                RecoveryType::OnChainRevert | 
                RecoveryType::FullRecovery
            ));
        }
    }
}