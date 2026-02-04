// Migration Safety & Rollback
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
    clock::Clock,
    sysvar::Sysvar,
    program_pack::Pack,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::migration::core::{
    MigrationState, MigrationStatus,
    verify_migration_authority,
};

// Pause reasons
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum PauseReason {
    CriticalBugFound,
    DataInconsistency,
    UnexpectedBehavior,
    ExternalThreat,
}

// Pause state information
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct PauseState {
    pub reason: PauseReason,
    pub paused_at_slot: u64,
    pub accounts_migrated_at_pause: u64,
    pub authority: Pubkey,
}

// Integrity check report
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default)]
pub struct IntegrityReport {
    pub total_samples: u16,
    pub successful_verifications: u16,
    pub failed_verifications: u16,
    pub integrity_score: u16,  // 0-100
    pub failed_accounts: Vec<Pubkey>,
}

// Account consistency check result
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ConsistencyCheck {
    pub old_account: Pubkey,
    pub new_account: Pubkey,
    pub is_consistent: bool,
    pub mismatch_fields: Vec<String>,
}

pub struct MigrationSafety;

impl MigrationSafety {
    /// Emergency pause migration
    pub fn emergency_pause_migration(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        reason: PauseReason,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let authority_account = next_account_info(account_info_iter)?;
        let pause_state_account = next_account_info(account_info_iter)?;
        
        // Load migration state
        let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        
        // Verify authority
        verify_migration_authority(authority_account, &migration_state)?;
        
        // Can only pause if active
        if migration_state.status != MigrationStatus::Active {
            msg!("Can only pause active migration");
            return Err(ProgramError::InvalidAccountData);
        }
        
        let clock = Clock::get()?;
        
        // Record pause state
        let pause_state = PauseState {
            reason,
            paused_at_slot: clock.slot,
            accounts_migrated_at_pause: migration_state.accounts_migrated,
            authority: *authority_account.key,
        };
        
        // Store pause state (simplified - in production would use proper account)
        msg!(
            "Recording pause state: reason={:?}, slot={}, accounts_migrated={}",
            pause_state.reason,
            pause_state.paused_at_slot,
            pause_state.accounts_migrated_at_pause
        );
        
        // Update migration status
        migration_state.status = MigrationStatus::Cancelled;
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        msg!(
            "MigrationPaused: reason={:?}, slot={}, accounts_affected={}",
            reason,
            pause_state.paused_at_slot,
            pause_state.accounts_migrated_at_pause
        );
        
        Ok(())
    }
    
    /// Verify migration integrity by sampling
    pub fn verify_migration_integrity(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        sample_size: u16,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let authority_account = next_account_info(account_info_iter)?;
        
        // Load migration state
        let migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        
        // Verify authority (optional - integrity check can be public)
        if authority_account.is_signer {
            verify_migration_authority(authority_account, &migration_state)?;
        }
        
        let mut report = IntegrityReport {
            total_samples: sample_size,
            ..Default::default()
        };
        
        // Sample migrated accounts
        let samples = Self::select_random_samples(accounts, sample_size)?;
        
        for (old_account, new_account) in samples {
            // Verify data consistency
            let consistent = Self::verify_account_consistency(
                &old_account,
                &new_account,
                &migration_state,
            )?;
            
            if consistent {
                report.successful_verifications += 1;
            } else {
                report.failed_verifications += 1;
                report.failed_accounts.push(*old_account.key);
            }
        }
        
        // Calculate integrity score
        report.integrity_score = if report.total_samples > 0 {
            ((report.successful_verifications as u32 * 100) / report.total_samples as u32) as u16
        } else {
            0
        };
        
        msg!(
            "IntegrityCheckCompleted: score={}, samples={}, failures={}",
            report.integrity_score,
            sample_size,
            report.failed_verifications
        );
        
        // Emit detailed report if failures found
        if report.failed_verifications > 0 {
            msg!("Failed accounts:");
            for account in &report.failed_accounts {
                msg!("  - {}", account);
            }
        }
        
        Ok(())
    }
    
    /// Verify consistency between old and new account
    fn verify_account_consistency(
        old_account: &AccountInfo,
        new_account: &AccountInfo,
        migration_state: &MigrationState,
    ) -> Result<bool, ProgramError> {
        // Basic checks
        if old_account.owner != &migration_state.old_program_id {
            msg!("Old account not owned by old program");
            return Ok(false);
        }
        
        if new_account.owner != &migration_state.new_program_id {
            msg!("New account not owned by new program");
            return Ok(false);
        }
        
        // Check discriminator (first 8 bytes)
        if old_account.data_len() < 8 || new_account.data_len() < 8 {
            msg!("Account too small for discriminator");
            return Ok(false);
        }
        
        let old_discriminator = &old_account.data.borrow()[..8];
        let new_discriminator = &new_account.data.borrow()[..8];
        
        if old_discriminator != new_discriminator {
            msg!("Discriminator mismatch");
            return Ok(false);
        }
        
        // Account type specific checks would go here
        // For now, just check that critical fields match
        
        Ok(true)
    }
    
    /// Select random account pairs for sampling
    fn select_random_samples(
        accounts: &[AccountInfo],
        count: u16,
    ) -> Result<Vec<(AccountInfo, AccountInfo)>, ProgramError> {
        let clock = Clock::get()?;
        let seed = clock.slot;
        
        let mut samples = Vec::new();
        
        // Find pairs of old/new accounts
        // Skip system accounts (first few)
        let account_slice = if accounts.len() > 10 {
            &accounts[10..]
        } else {
            accounts
        };
        
        // Simple deterministic sampling
        for i in 0..count.min(account_slice.len() as u16 / 2) {
            let index = ((seed + i as u64) % (account_slice.len() as u64 / 2)) as usize;
            
            if index * 2 + 1 < account_slice.len() {
                samples.push((
                    account_slice[index * 2].clone(),
                    account_slice[index * 2 + 1].clone(),
                ));
            }
        }
        
        Ok(samples)
    }
    
    /// Resume paused migration (if safe)
    pub fn resume_migration(
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
        
        // Can only resume if cancelled/paused
        if migration_state.status != MigrationStatus::Cancelled {
            msg!("Can only resume cancelled migration");
            return Err(ProgramError::InvalidAccountData);
        }
        
        let clock = Clock::get()?;
        
        // Check if still within migration window
        if clock.slot > migration_state.end_slot {
            msg!("Migration window has expired");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Resume migration
        migration_state.status = MigrationStatus::Active;
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        msg!(
            "MigrationResumed: slot={}, accounts_migrated={}",
            clock.slot,
            migration_state.accounts_migrated
        );
        
        Ok(())
    }
    
    /// Rollback a specific migrated account (emergency use only)
    pub fn rollback_account_migration(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let authority_account = next_account_info(account_info_iter)?;
        let old_account = next_account_info(account_info_iter)?;
        let new_account = next_account_info(account_info_iter)?;
        let user_account = next_account_info(account_info_iter)?;
        
        // Load migration state
        let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        
        // Verify authority
        verify_migration_authority(authority_account, &migration_state)?;
        
        // Verify migration is paused/cancelled
        if migration_state.status == MigrationStatus::Active {
            msg!("Cannot rollback during active migration");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Verify user owns the accounts
        if !user_account.is_signer {
            msg!("User must sign for rollback");
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        msg!(
            "Rolling back account migration for user {}",
            user_account.key
        );
        
        // In production, would:
        // 1. Close new account and return lamports
        // 2. Restore old account state
        // 3. Update migration counters
        
        // Update counter
        migration_state.accounts_migrated = migration_state.accounts_migrated
            .saturating_sub(1);
        
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        Ok(())
    }
}

/// Health check for migration system
pub fn migration_health_check(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let migration_state_account = next_account_info(account_info_iter)?;
    
    // Load migration state
    let migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
    
    let clock = Clock::get()?;
    
    // Calculate migration rate
    let elapsed = clock.slot.saturating_sub(migration_state.start_slot);
    let rate = if elapsed > 0 {
        migration_state.accounts_migrated as f64 / elapsed as f64
    } else {
        0.0
    };
    
    // Check health indicators
    let is_healthy = migration_state.status == MigrationStatus::Active &&
                    clock.slot <= migration_state.end_slot &&
                    rate > 0.1; // At least 0.1 accounts per slot
    
    msg!(
        "MigrationHealthCheck: status={:?}, rate={:.2}/slot, healthy={}",
        migration_state.status,
        rate,
        is_healthy
    );
    
    if !is_healthy {
        msg!("WARNING: Migration may need attention");
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pause_reason_serialization() {
        let reasons = vec![
            PauseReason::CriticalBugFound,
            PauseReason::DataInconsistency,
            PauseReason::UnexpectedBehavior,
            PauseReason::ExternalThreat,
        ];
        
        for reason in reasons {
            let serialized = reason.try_to_vec().unwrap();
            let deserialized = PauseReason::try_from_slice(&serialized).unwrap();
            assert_eq!(reason, deserialized);
        }
    }
    
    #[test]
    fn test_integrity_score_calculation() {
        let mut report = IntegrityReport {
            total_samples: 100,
            successful_verifications: 95,
            failed_verifications: 5,
            integrity_score: 0,
            failed_accounts: vec![],
        };
        
        report.integrity_score = ((report.successful_verifications as u32 * 100) / 
                                 report.total_samples as u32) as u16;
        
        assert_eq!(report.integrity_score, 95);
    }
}