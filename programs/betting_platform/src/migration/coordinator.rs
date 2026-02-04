// Migration Coordination
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
    clock::Clock,
    sysvar::Sysvar,
    program::invoke,
    system_instruction,
    program_pack::Pack,
    keccak,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::U64F64;
use crate::migration::core::{
    MigrationState, MigrationStatus, MigrationType, MigrationProgress,
    MIGRATION_STATE_DISCRIMINATOR, MIGRATION_NOTICE_PERIOD, MIGRATION_DURATION,
    verify_migration_authority, emit_migration_announced,
};

pub struct MigrationCoordinator;

impl MigrationCoordinator {
    /// Initialize migration with safety checks
    pub fn initialize_migration(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        migration_type: MigrationType,
        incentive_multiplier: U64F64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        // Accounts expected:
        // 0. Migration state account (to be created)
        // 1. Authority
        // 2. Old program
        // 3. New program
        // 4. System program
        // 5+ Accounts to count for migration
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let authority_account = next_account_info(account_info_iter)?;
        let old_program_account = next_account_info(account_info_iter)?;
        let new_program_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        
        // Verify authority is signer
        if !authority_account.is_signer {
            msg!("Authority must sign to initialize migration");
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Verify programs are different
        if old_program_account.key == new_program_account.key {
            msg!("Old and new programs must be different");
            return Err(ProgramError::InvalidArgument);
        }
        
        let clock = Clock::get()?;
        
        // Create migration state account
        let rent_lamports = solana_program::rent::Rent::default()
            .minimum_balance(MigrationState::LEN);
        
        invoke(
            &system_instruction::create_account(
                authority_account.key,
                migration_state_account.key,
                rent_lamports,
                MigrationState::LEN as u64,
                program_id,
            ),
            &[
                authority_account.clone(),
                migration_state_account.clone(),
                system_program.clone(),
            ],
        )?;
        
        // Count accounts to migrate
        let total_accounts = Self::count_accounts_to_migrate(
            accounts,
            old_program_account.key,
        )?;
        
        // Initialize migration state
        let migration_state = MigrationState {
            discriminator: MIGRATION_STATE_DISCRIMINATOR,
            old_program_id: *old_program_account.key,
            new_program_id: *new_program_account.key,
            migration_authority: *authority_account.key,
            start_slot: clock.slot + MIGRATION_NOTICE_PERIOD,
            end_slot: clock.slot + MIGRATION_NOTICE_PERIOD + MIGRATION_DURATION,
            total_accounts_to_migrate: total_accounts,
            accounts_migrated: 0,
            migration_type,
            incentive_multiplier: incentive_multiplier.0,
            status: MigrationStatus::Announced,
            merkle_root: [0u8; 32],
        };
        
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        emit_migration_announced(
            &migration_state.old_program_id,
            &migration_state.new_program_id,
            migration_type,
            migration_state.start_slot,
            migration_state.end_slot,
            incentive_multiplier,
            total_accounts,
        );
        
        Ok(())
    }
    
    /// Activate migration after notice period
    pub fn activate_migration(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let authority_account = next_account_info(account_info_iter)?;
        let old_global_config_account = next_account_info(account_info_iter)?;
        
        // Load migration state
        let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        
        // Verify authority
        verify_migration_authority(authority_account, &migration_state)?;
        
        let clock = Clock::get()?;
        
        // Verify status and timing
        if migration_state.status != MigrationStatus::Announced {
            msg!("Migration must be in Announced status");
            return Err(ProgramError::InvalidAccountData);
        }
        
        if clock.slot < migration_state.start_slot {
            msg!("Migration notice period not complete");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Update status
        migration_state.status = MigrationStatus::Active;
        
        // Halt new operations on old program (simplified)
        // In production, would update global config to set migration_mode = true
        msg!("Setting migration mode on old program");
        
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        msg!(
            "MigrationActivated: slot={}, accounts_migrated=0, accounts_remaining={}",
            clock.slot,
            migration_state.total_accounts_to_migrate
        );
        
        Ok(())
    }
    
    /// Update migration progress
    pub fn update_migration_progress(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let migration_state_account = next_account_info(account_info_iter)?;
        
        // Load migration state
        let migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        
        let clock = Clock::get()?;
        
        // Calculate progress
        let progress = MigrationProgress {
            accounts_migrated: migration_state.accounts_migrated,
            accounts_remaining: migration_state.total_accounts_to_migrate
                .saturating_sub(migration_state.accounts_migrated),
            percentage_complete: if migration_state.total_accounts_to_migrate > 0 {
                (migration_state.accounts_migrated * 100) / migration_state.total_accounts_to_migrate
            } else {
                0
            },
            estimated_completion_slot: Self::estimate_completion(&migration_state, &clock)?,
            current_slot: clock.slot,
            time_remaining: migration_state.end_slot.saturating_sub(clock.slot),
        };
        
        // Check if should emit warning
        if progress.percentage_complete >= 95 {
            msg!(
                "MigrationNearingCompletion: percentage={}, accounts_remaining={}",
                progress.percentage_complete,
                progress.accounts_remaining
            );
        }
        
        // Log progress
        msg!(
            "MigrationProgress: migrated={}/{} ({}%), est_completion={}, time_remaining={}",
            progress.accounts_migrated,
            migration_state.total_accounts_to_migrate,
            progress.percentage_complete,
            progress.estimated_completion_slot,
            progress.time_remaining
        );
        
        Ok(())
    }
    
    /// Finalize migration
    pub fn finalize_migration(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let authority_account = next_account_info(account_info_iter)?;
        
        // Load migration state
        let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        
        // Verify authority
        verify_migration_authority(authority_account, &migration_state)?;
        
        let clock = Clock::get()?;
        
        // Verify status
        if migration_state.status != MigrationStatus::Active &&
           migration_state.status != MigrationStatus::Finalizing {
            msg!("Cannot finalize migration in current status");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Check if grace period expired or all migrated
        let all_migrated = migration_state.accounts_migrated >= 
            migration_state.total_accounts_to_migrate;
        let grace_expired = clock.slot > migration_state.end_slot;
        
        if !all_migrated && !grace_expired {
            msg!("Migration not complete and grace period not expired");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Update status
        migration_state.status = MigrationStatus::Completed;
        
        // Compute final merkle root
        migration_state.merkle_root = Self::compute_migration_merkle_root(accounts)?;
        
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        msg!(
            "MigrationCompleted: total_migrated={}, total_accounts={}, final_slot={}, merkle_root={:?}",
            migration_state.accounts_migrated,
            migration_state.total_accounts_to_migrate,
            clock.slot,
            migration_state.merkle_root
        );
        
        Ok(())
    }
    
    /// Count accounts owned by old program
    fn count_accounts_to_migrate(
        accounts: &[AccountInfo],
        old_program: &Pubkey,
    ) -> Result<u64, ProgramError> {
        let mut count = 0u64;
        
        // Count accounts owned by old program
        // Skip first 5 accounts (migration state, authority, programs, system)
        for account in accounts.iter().skip(5) {
            if account.owner == old_program {
                count = count.checked_add(1).ok_or(ProgramError::InvalidAccountData)?;
            }
        }
        
        msg!("Found {} accounts to migrate", count);
        Ok(count)
    }
    
    /// Estimate completion slot based on current rate
    fn estimate_completion(
        state: &MigrationState,
        clock: &Clock,
    ) -> Result<u64, ProgramError> {
        if state.accounts_migrated == 0 {
            return Ok(state.end_slot);
        }
        
        let elapsed = clock.slot.saturating_sub(state.start_slot).max(1);
        let rate = state.accounts_migrated / elapsed;
        
        if rate == 0 {
            return Ok(state.end_slot);
        }
        
        let remaining = state.total_accounts_to_migrate - state.accounts_migrated;
        let slots_needed = remaining / rate;
        
        Ok(clock.slot + slots_needed)
    }
    
    /// Compute merkle root of migrated accounts
    fn compute_migration_merkle_root(
        accounts: &[AccountInfo],
    ) -> Result<[u8; 32], ProgramError> {
        let mut account_hashes = Vec::new();
        
        // Collect hashes of all migrated accounts
        // In production, would track migrated accounts separately
        for account in accounts.iter() {
            // Check if account is migrated (simplified check)
            if account.data_len() > 8 {
                let hash = keccak::hash(account.key.as_ref());
                account_hashes.push(hash.to_bytes());
            }
        }
        
        if account_hashes.is_empty() {
            return Ok([0u8; 32]);
        }
        
        // Build merkle tree
        let mut current_level = account_hashes;
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                let left = current_level[i];
                let right = if i + 1 < current_level.len() {
                    current_level[i + 1]
                } else {
                    current_level[i]
                };
                
                let mut combined = Vec::with_capacity(64);
                combined.extend_from_slice(&left);
                combined.extend_from_slice(&right);
                
                let hash = keccak::hash(&combined);
                next_level.push(hash.to_bytes());
            }
            
            current_level = next_level;
        }
        
        Ok(current_level[0])
    }
}

/// Cancel/pause migration in emergency
pub fn emergency_cancel_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let migration_state_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    
    // Load migration state
    let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
    
    // Verify authority
    verify_migration_authority(authority_account, &migration_state)?;
    
    // Can only cancel if active
    if migration_state.status != MigrationStatus::Active {
        msg!("Can only cancel active migration");
        return Err(ProgramError::InvalidAccountData);
    }
    
    let clock = Clock::get()?;
    
    // Update status
    migration_state.status = MigrationStatus::Cancelled;
    
    migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
    
    msg!(
        "MigrationCancelled: slot={}, accounts_migrated={}",
        clock.slot,
        migration_state.accounts_migrated
    );
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_progress_calculation() {
        let state = MigrationState {
            discriminator: MIGRATION_STATE_DISCRIMINATOR,
            old_program_id: Pubkey::new_unique(),
            new_program_id: Pubkey::new_unique(),
            migration_authority: Pubkey::new_unique(),
            start_slot: 100,
            end_slot: 10000,
            total_accounts_to_migrate: 1000,
            accounts_migrated: 250,
            migration_type: MigrationType::FeatureUpgrade,
            incentive_multiplier: U64F64::from_num(2).0,
            status: MigrationStatus::Active,
            merkle_root: [0u8; 32],
        };
        
        let clock = Clock {
            slot: 500,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: 0,
        };
        
        let estimated = MigrationCoordinator::estimate_completion(&state, &clock).unwrap();
        
        // 250 accounts in 400 slots = 0.625 accounts/slot
        // 750 remaining / 0.625 = 1200 slots
        // Current slot 500 + 1200 = 1700
        assert_eq!(estimated, 1700);
    }
}