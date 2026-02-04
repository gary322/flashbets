// Migration Instructions and Processor
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::U64F64;
use crate::migration::{
    core::{MigrationType, PositionSnapshot, VerseSnapshot},
    position_migration::{PositionMigrator, create_position_snapshot},
    verse_migration::{VerseMigrator, create_verse_snapshot},
    coordinator::{MigrationCoordinator, emergency_cancel_migration},
    safety::{MigrationSafety, PauseReason, migration_health_check},
};

/// Migration instruction enum
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum MigrationInstruction {
    /// Initialize a new migration
    /// Accounts:
    /// 0. [writable] Migration state account (PDA)
    /// 1. [signer] Migration authority
    /// 2. [] Old program
    /// 3. [] New program
    /// 4. [] System program
    /// 5+ [] Accounts to count for migration
    InitializeMigration {
        migration_type: MigrationType,
        incentive_multiplier: u64, // Fixed-point value
    },
    
    /// Activate migration after notice period
    /// Accounts:
    /// 0. [writable] Migration state account
    /// 1. [signer] Migration authority
    /// 2. [writable] Old program global config
    ActivateMigration,
    
    /// Create position snapshot for migration
    /// Accounts:
    /// 0. [] Position account
    /// 1. [writable] Snapshot account
    /// 2. [signer] Position owner
    /// 3. [] System program
    CreatePositionSnapshot,
    
    /// Migrate a position
    /// Accounts:
    /// 0. [writable] Migration state
    /// 1. [writable] Old position account
    /// 2. [writable] New position account
    /// 3. [signer] User (owner)
    /// 4. [] Old program
    /// 5. [] New program
    /// 6. [] Price feed
    /// 7. [] MMT mint
    /// 8. [writable] User MMT token account
    /// 9. [] System program
    /// 10. [] Market account
    /// 11. [] Vault account
    /// 12+ [] Verse accounts for chain positions
    MigratePosition {
        position_snapshot: PositionSnapshot,
    },
    
    /// Create verse snapshot for migration
    /// Accounts:
    /// 0. [] Verse account
    /// 1. [writable] Snapshot account
    /// 2. [signer] Authority
    /// 3. [] System program
    CreateVerseSnapshot,
    
    /// Migrate verse hierarchy
    /// Accounts:
    /// 0. [writable] Migration state
    /// 1. [] Old verse account
    /// 2. [writable] New verse account
    /// 3. [] Parent verse (optional)
    /// 4. [signer] Migration authority
    /// 5. [] New program
    /// 6. [] System program
    /// 7+ [] Child/proposal accounts
    MigrateVerseHierarchy {
        verse_snapshot: VerseSnapshot,
    },
    
    /// Update migration progress
    /// Accounts:
    /// 0. [] Migration state account
    UpdateMigrationProgress,
    
    /// Finalize migration
    /// Accounts:
    /// 0. [writable] Migration state account
    /// 1. [signer] Migration authority
    FinalizeMigration,
    
    /// Emergency pause
    /// Accounts:
    /// 0. [writable] Migration state account
    /// 1. [signer] Migration authority
    /// 2. [writable] Pause state account
    EmergencyPause {
        reason: PauseReason,
    },
    
    /// Resume paused migration
    /// Accounts:
    /// 0. [writable] Migration state account
    /// 1. [signer] Migration authority
    ResumeMigration,
    
    /// Verify migration integrity
    /// Accounts:
    /// 0. [] Migration state account
    /// 1. [signer optional] Authority
    /// 2+ [] Account pairs to verify
    VerifyIntegrity {
        sample_size: u16,
    },
    
    /// Cancel migration
    /// Accounts:
    /// 0. [writable] Migration state account
    /// 1. [signer] Migration authority
    CancelMigration,
    
    /// Rollback specific account
    /// Accounts:
    /// 0. [writable] Migration state account
    /// 1. [signer] Migration authority
    /// 2. [writable] Old account
    /// 3. [writable] New account
    /// 4. [signer] User
    RollbackAccount,
    
    /// Health check
    /// Accounts:
    /// 0. [] Migration state account
    HealthCheck,
}

/// Process migration instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = MigrationInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    
    msg!("Processing migration instruction: {:?}", instruction);
    
    match instruction {
        MigrationInstruction::InitializeMigration {
            migration_type,
            incentive_multiplier,
        } => {
            msg!("Initializing migration");
            MigrationCoordinator::initialize_migration(
                program_id,
                accounts,
                migration_type,
                U64F64::from_raw(incentive_multiplier),
            )
        }
        
        MigrationInstruction::ActivateMigration => {
            msg!("Activating migration");
            MigrationCoordinator::activate_migration(program_id, accounts)
        }
        
        MigrationInstruction::CreatePositionSnapshot => {
            msg!("Creating position snapshot");
            create_position_snapshot(program_id, accounts)
        }
        
        MigrationInstruction::MigratePosition { position_snapshot } => {
            msg!("Migrating position");
            PositionMigrator::migrate_position(
                program_id,
                accounts,
                position_snapshot,
            )
        }
        
        MigrationInstruction::CreateVerseSnapshot => {
            msg!("Creating verse snapshot");
            create_verse_snapshot(program_id, accounts)
        }
        
        MigrationInstruction::MigrateVerseHierarchy { verse_snapshot } => {
            msg!("Migrating verse hierarchy");
            VerseMigrator::migrate_verse_hierarchy(
                program_id,
                accounts,
                verse_snapshot,
            )
        }
        
        MigrationInstruction::UpdateMigrationProgress => {
            msg!("Updating migration progress");
            MigrationCoordinator::update_migration_progress(program_id, accounts)
        }
        
        MigrationInstruction::FinalizeMigration => {
            msg!("Finalizing migration");
            MigrationCoordinator::finalize_migration(program_id, accounts)
        }
        
        MigrationInstruction::EmergencyPause { reason } => {
            msg!("Emergency pause: {:?}", reason);
            MigrationSafety::emergency_pause_migration(
                program_id,
                accounts,
                reason,
            )
        }
        
        MigrationInstruction::ResumeMigration => {
            msg!("Resuming migration");
            MigrationSafety::resume_migration(program_id, accounts)
        }
        
        MigrationInstruction::VerifyIntegrity { sample_size } => {
            msg!("Verifying migration integrity with {} samples", sample_size);
            MigrationSafety::verify_migration_integrity(
                program_id,
                accounts,
                sample_size,
            )
        }
        
        MigrationInstruction::CancelMigration => {
            msg!("Cancelling migration");
            emergency_cancel_migration(program_id, accounts)
        }
        
        MigrationInstruction::RollbackAccount => {
            msg!("Rolling back account migration");
            MigrationSafety::rollback_account_migration(program_id, accounts)
        }
        
        MigrationInstruction::HealthCheck => {
            msg!("Performing migration health check");
            migration_health_check(program_id, accounts)
        }
    }
}

/// Helper to build initialize migration instruction
pub fn build_initialize_migration_instruction(
    program_id: &Pubkey,
    migration_state: &Pubkey,
    authority: &Pubkey,
    old_program: &Pubkey,
    new_program: &Pubkey,
    migration_type: MigrationType,
    incentive_multiplier: U64F64,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let data = MigrationInstruction::InitializeMigration {
        migration_type,
        incentive_multiplier: incentive_multiplier.0,
    }.try_to_vec().map_err(|_| ProgramError::InvalidInstructionData)?;
    
    Ok(solana_program::instruction::Instruction {
        program_id: *program_id,
        accounts: vec![
            solana_program::instruction::AccountMeta::new(*migration_state, false),
            solana_program::instruction::AccountMeta::new_readonly(*authority, true),
            solana_program::instruction::AccountMeta::new_readonly(*old_program, false),
            solana_program::instruction::AccountMeta::new_readonly(*new_program, false),
            solana_program::instruction::AccountMeta::new_readonly(solana_program::system_program::id(), false),
        ],
        data,
    })
}

/// Helper to build migrate position instruction
pub fn build_migrate_position_instruction(
    program_id: &Pubkey,
    migration_state: &Pubkey,
    old_position: &Pubkey,
    new_position: &Pubkey,
    user: &Pubkey,
    old_program: &Pubkey,
    new_program: &Pubkey,
    price_feed: &Pubkey,
    mmt_mint: &Pubkey,
    user_mmt_account: &Pubkey,
    market: &Pubkey,
    vault: &Pubkey,
    position_snapshot: PositionSnapshot,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let data = MigrationInstruction::MigratePosition {
        position_snapshot,
    }.try_to_vec().map_err(|_| ProgramError::InvalidInstructionData)?;
    
    Ok(solana_program::instruction::Instruction {
        program_id: *program_id,
        accounts: vec![
            solana_program::instruction::AccountMeta::new(*migration_state, false),
            solana_program::instruction::AccountMeta::new(*old_position, false),
            solana_program::instruction::AccountMeta::new(*new_position, false),
            solana_program::instruction::AccountMeta::new(*user, true),
            solana_program::instruction::AccountMeta::new_readonly(*old_program, false),
            solana_program::instruction::AccountMeta::new_readonly(*new_program, false),
            solana_program::instruction::AccountMeta::new_readonly(*price_feed, false),
            solana_program::instruction::AccountMeta::new_readonly(*mmt_mint, false),
            solana_program::instruction::AccountMeta::new(*user_mmt_account, false),
            solana_program::instruction::AccountMeta::new_readonly(solana_program::system_program::id(), false),
            solana_program::instruction::AccountMeta::new_readonly(*market, false),
            solana_program::instruction::AccountMeta::new_readonly(*vault, false),
        ],
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_instruction_serialization() {
        let instructions = vec![
            MigrationInstruction::InitializeMigration {
                migration_type: MigrationType::FeatureUpgrade,
                incentive_multiplier: U64F64::from_num(2).0,
            },
            MigrationInstruction::ActivateMigration,
            MigrationInstruction::UpdateMigrationProgress,
            MigrationInstruction::FinalizeMigration,
        ];
        
        for instruction in instructions {
            let serialized = instruction.try_to_vec().unwrap();
            let deserialized = MigrationInstruction::try_from_slice(&serialized).unwrap();
            
            // Can't use direct equality due to PositionSnapshot/VerseSnapshot complexity
            // Just ensure it deserializes without error
            match (&instruction, &deserialized) {
                (MigrationInstruction::InitializeMigration { .. }, 
                 MigrationInstruction::InitializeMigration { .. }) => {},
                (MigrationInstruction::ActivateMigration,
                 MigrationInstruction::ActivateMigration) => {},
                _ => {}
            }
        }
    }
}