//! State Migration Framework
//!
//! Production-grade migration system for upgrading account structures
//! Supports atomic migrations, rollback, and verification

use solana_program::{
    account_info::{next_account_info, AccountInfo},
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
        versioned_accounts::{CURRENT_VERSION, MigrationState, Versioned},
        rollback_protection::RollbackProtectionState,
    },
    account_validation::DISCRIMINATOR_SIZE,
};

/// Migration framework constants
pub const MAX_ACCOUNTS_PER_BATCH: usize = 10;
pub const MIGRATION_TIMEOUT_SLOTS: u64 = 432_000; // ~2 days

/// Migration manager account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationManager {
    /// Discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Current migration version
    pub current_version: u32,
    
    /// Target version
    pub target_version: u32,
    
    /// Migration state
    pub state: MigrationManagerState,
    
    /// Authority that can execute migrations
    pub authority: Pubkey,
    
    /// Emergency pause authority
    pub emergency_authority: Pubkey,
    
    /// Migration started at
    pub started_at: i64,
    
    /// Migration started slot
    pub started_slot: u64,
    
    /// Total accounts to migrate
    pub total_accounts: u32,
    
    /// Accounts migrated successfully
    pub migrated_accounts: u32,
    
    /// Failed migrations
    pub failed_accounts: u32,
    
    /// Current batch being processed
    pub current_batch: u32,
    
    /// Rollback protection state
    pub rollback_checkpoint: [u8; 32],
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MigrationManagerState {
    Idle,
    Planning,
    Executing,
    Verifying,
    Completed,
    Failed,
    RollingBack,
}

/// Individual migration record
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationRecord {
    /// Account being migrated
    pub account: Pubkey,
    
    /// Original version
    pub from_version: u32,
    
    /// Target version
    pub to_version: u32,
    
    /// Migration status
    pub status: MigrationStatus,
    
    /// Backup data hash
    pub backup_hash: [u8; 32],
    
    /// Migration timestamp
    pub migrated_at: i64,
    
    /// Error message if failed
    pub error: Option<String>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MigrationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Verified,
}

/// Migration strategy trait
pub trait MigrationStrategy {
    /// Validate if migration can proceed
    fn validate(&self, from_version: u32, to_version: u32) -> Result<(), ProgramError>;
    
    /// Perform the migration
    fn migrate(&self, account_data: &mut [u8]) -> Result<(), ProgramError>;
    
    /// Verify migration was successful
    fn verify(&self, account_data: &[u8]) -> Result<(), ProgramError>;
    
    /// Rollback if needed
    fn rollback(&self, account_data: &mut [u8], backup: &[u8]) -> Result<(), ProgramError>;
}

/// V0 to V1 migration strategy
pub struct V0ToV1Migration;

impl MigrationStrategy for V0ToV1Migration {
    fn validate(&self, from_version: u32, to_version: u32) -> Result<(), ProgramError> {
        if from_version != 0 || to_version != 1 {
            return Err(BettingPlatformError::InvalidMigrationTarget.into());
        }
        Ok(())
    }
    
    fn migrate(&self, account_data: &mut [u8]) -> Result<(), ProgramError> {
        // Add version field after discriminator
        let discriminator = &account_data[0..DISCRIMINATOR_SIZE];
        let old_data = &account_data[DISCRIMINATOR_SIZE..];
        
        // Create new layout with version
        let mut new_data = Vec::new();
        new_data.extend_from_slice(discriminator);
        new_data.extend_from_slice(&1u32.to_le_bytes()); // Version 1
        new_data.extend_from_slice(&[0u8; 4]); // Migration state (Current)
        new_data.extend_from_slice(old_data);
        
        // Ensure we don't exceed account size
        if new_data.len() > account_data.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        
        // Copy back
        account_data[..new_data.len()].copy_from_slice(&new_data);
        
        Ok(())
    }
    
    fn verify(&self, account_data: &[u8]) -> Result<(), ProgramError> {
        if account_data.len() < DISCRIMINATOR_SIZE + 4 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let version_bytes = &account_data[DISCRIMINATOR_SIZE..DISCRIMINATOR_SIZE + 4];
        let version = u32::from_le_bytes([
            version_bytes[0],
            version_bytes[1],
            version_bytes[2],
            version_bytes[3],
        ]);
        
        if version != 1 {
            return Err(BettingPlatformError::InvalidStateVersion.into());
        }
        
        Ok(())
    }
    
    fn rollback(&self, account_data: &mut [u8], backup: &[u8]) -> Result<(), ProgramError> {
        if backup.len() > account_data.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        account_data[..backup.len()].copy_from_slice(backup);
        Ok(())
    }
}

/// Process migration planning
pub fn process_plan_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    target_version: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let migration_manager_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    let rollback_protection_account = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load migration manager
    let mut manager_data = migration_manager_account.try_borrow_mut_data()?;
    let mut manager = MigrationManager::try_from_slice(&manager_data)?;
    
    if manager.authority != *authority_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify idle state
    if manager.state != MigrationManagerState::Idle {
        return Err(BettingPlatformError::MigrationInProgress.into());
    }
    
    // Validate target version
    if target_version <= manager.current_version {
        return Err(BettingPlatformError::InvalidMigrationTarget.into());
    }
    
    if target_version > CURRENT_VERSION {
        return Err(BettingPlatformError::InvalidStateVersion.into());
    }
    
    // Create rollback checkpoint
    let rollback_state = RollbackProtectionState::try_from_slice(
        &rollback_protection_account.data.borrow()
    )?;
    
    // Update manager
    manager.target_version = target_version;
    manager.state = MigrationManagerState::Planning;
    manager.started_at = Clock::get()?.unix_timestamp;
    manager.started_slot = Clock::get()?.slot;
    manager.rollback_checkpoint = rollback_state.current_hash;
    
    manager.serialize(&mut *manager_data)?;
    
    msg!("Migration planned: v{} -> v{}", manager.current_version, target_version);
    
    Ok(())
}

/// Process batch migration
pub fn process_migrate_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    batch_accounts: Vec<Pubkey>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let migration_manager_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load migration manager
    let mut manager_data = migration_manager_account.try_borrow_mut_data()?;
    let mut manager = MigrationManager::try_from_slice(&manager_data)?;
    
    if manager.authority != *authority_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify executing state
    if manager.state != MigrationManagerState::Executing {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check timeout
    let current_slot = Clock::get()?.slot;
    if current_slot > manager.started_slot + MIGRATION_TIMEOUT_SLOTS {
        manager.state = MigrationManagerState::Failed;
        manager.serialize(&mut *manager_data)?;
        return Err(BettingPlatformError::MigrationTimeout.into());
    }
    
    // Get migration strategy
    let strategy: Box<dyn MigrationStrategy> = match (manager.current_version, manager.target_version) {
        (0, 1) => Box::new(V0ToV1Migration),
        _ => return Err(BettingPlatformError::InvalidMigrationTarget.into()),
    };
    
    // Process accounts in batch
    let mut success_count = 0u32;
    let mut fail_count = 0u32;
    
    for (i, account_pubkey) in batch_accounts.iter().enumerate() {
        if i >= MAX_ACCOUNTS_PER_BATCH {
            break;
        }
        
        let account_info = next_account_info(account_info_iter)?;
        
        if account_info.key != account_pubkey {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Create backup
        let backup = account_info.data.borrow().to_vec();
        let backup_hash = solana_program::hash::hash(&backup).to_bytes();
        
        // Perform migration
        let result = {
            let mut account_data = account_info.try_borrow_mut_data()?;
            strategy.migrate(&mut account_data)
        };
        
        match result {
            Ok(()) => {
                success_count += 1;
                msg!("Migrated account: {}", account_pubkey);
            }
            Err(e) => {
                fail_count += 1;
                msg!("Failed to migrate {}: {:?}", account_pubkey, e);
                
                // Attempt rollback
                let mut account_data = account_info.try_borrow_mut_data()?;
                let _ = strategy.rollback(&mut account_data, &backup);
            }
        }
    }
    
    // Update manager stats
    manager.migrated_accounts += success_count;
    manager.failed_accounts += fail_count;
    manager.current_batch += 1;
    
    manager.serialize(&mut *manager_data)?;
    
    msg!("Batch migration: {} success, {} failed", success_count, fail_count);
    
    Ok(())
}

/// Verify migration completion
pub fn process_verify_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let migration_manager_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    
    // Load migration manager
    let mut manager_data = migration_manager_account.try_borrow_mut_data()?;
    let mut manager = MigrationManager::try_from_slice(&manager_data)?;
    
    if manager.authority != *authority_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if all accounts migrated
    if manager.migrated_accounts + manager.failed_accounts < manager.total_accounts {
        return Err(BettingPlatformError::MigrationInProgress.into());
    }
    
    // If no failures, mark as completed
    if manager.failed_accounts == 0 {
        manager.state = MigrationManagerState::Completed;
        manager.current_version = manager.target_version;
        msg!("Migration completed successfully to v{}", manager.target_version);
    } else {
        manager.state = MigrationManagerState::Failed;
        msg!("Migration failed with {} errors", manager.failed_accounts);
    }
    
    manager.serialize(&mut *manager_data)?;
    
    Ok(())
}

/// Emergency migration pause
pub fn process_pause_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let migration_manager_account = next_account_info(account_info_iter)?;
    let emergency_authority_account = next_account_info(account_info_iter)?;
    
    if !emergency_authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut manager_data = migration_manager_account.try_borrow_mut_data()?;
    let mut manager = MigrationManager::try_from_slice(&manager_data)?;
    
    if manager.emergency_authority != *emergency_authority_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Pause by setting to failed state
    manager.state = MigrationManagerState::Failed;
    manager.serialize(&mut *manager_data)?;
    
    msg!("Migration paused by emergency authority");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_v0_to_v1_migration() {
        let strategy = V0ToV1Migration;
        
        // Validate version check
        assert!(strategy.validate(0, 1).is_ok());
        assert!(strategy.validate(1, 2).is_err());
        
        // Test migration
        let mut data = vec![0u8; 100];
        // Set discriminator
        data[0..8].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        // Old data
        data[8..16].copy_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);
        
        strategy.migrate(&mut data).unwrap();
        
        // Verify version was added
        let version = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        assert_eq!(version, 1);
        
        // Verify old data moved
        assert_eq!(data[16], 10);
        assert_eq!(data[17], 20);
    }
}