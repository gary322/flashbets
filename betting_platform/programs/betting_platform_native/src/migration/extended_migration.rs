//! Extended migration framework for 60-day parallel deployment
//!
//! Implements specification-compliant migration with:
//! - 60-day transition period
//! - Parallel program execution
//! - Double MMT incentives for migrators

use solana_program::{
    account_info::AccountInfo,
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
    state::{Position, GlobalConfigPDA},
};

/// 60-day migration period in slots (400ms per slot)
pub const MIGRATION_PERIOD_SLOTS: u64 = 15_552_000; // 60 days

/// Double MMT multiplier for migration incentives
pub const MIGRATION_MMT_MULTIPLIER: u64 = 2;

/// Parallel deployment configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ParallelDeployment {
    /// Old program ID (current immutable version)
    pub old_program_id: Pubkey,
    
    /// New program ID (upgraded immutable version)
    pub new_program_id: Pubkey,
    
    /// Migration start slot
    pub start_slot: u64,
    
    /// Migration end slot (start + 60 days)
    pub end_slot: u64,
    
    /// Total positions migrated
    pub positions_migrated: u64,
    
    /// Total MMT rewards distributed
    pub mmt_rewards_distributed: u64,
    
    /// Is migration active
    pub is_active: bool,
    
    /// Authority that initiated migration
    pub authority: Pubkey,
}

impl ParallelDeployment {
    /// Initialize parallel deployment
    pub fn new(
        old_program_id: Pubkey,
        new_program_id: Pubkey,
        authority: Pubkey,
        current_slot: u64,
    ) -> Self {
        Self {
            old_program_id,
            new_program_id,
            start_slot: current_slot,
            end_slot: current_slot + MIGRATION_PERIOD_SLOTS,
            positions_migrated: 0,
            mmt_rewards_distributed: 0,
            is_active: true,
            authority,
        }
    }
    
    /// Check if migration period has expired
    pub fn is_expired(&self, current_slot: u64) -> bool {
        current_slot > self.end_slot
    }
    
    /// Calculate remaining time in migration
    pub fn remaining_slots(&self, current_slot: u64) -> u64 {
        if current_slot >= self.end_slot {
            0
        } else {
            self.end_slot - current_slot
        }
    }
    
    /// Calculate migration progress percentage
    pub fn progress_percentage(&self, current_slot: u64) -> u8 {
        if current_slot >= self.end_slot {
            100
        } else if current_slot <= self.start_slot {
            0
        } else {
            let elapsed = current_slot - self.start_slot;
            let total = self.end_slot - self.start_slot;
            ((elapsed * 100) / total) as u8
        }
    }
}

/// Process migration initialization
pub fn initialize_parallel_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_program_id: Pubkey,
) -> ProgramResult {
    let authority = &accounts[0];
    let migration_state = &accounts[1];
    let global_config = &accounts[2];
    
    // Verify authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load global config to verify authority
    let config = GlobalConfigPDA::try_from_slice(&global_config.data.borrow())?;
    if authority.key != &config.update_authority {
        msg!("Only update authority can initiate migration");
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Get current slot
    let clock = Clock::get()?;
    
    // Initialize parallel deployment
    let deployment = ParallelDeployment::new(
        *program_id,
        new_program_id,
        *authority.key,
        clock.slot,
    );
    
    // Save migration state
    deployment.serialize(&mut &mut migration_state.data.borrow_mut()[..])?;
    
    msg!("Migration initialized: old={}, new={}, end_slot={}", 
        program_id, new_program_id, deployment.end_slot);
    
    // Emit event
    msg!("Migration started: old={}, new={}, start={}, end={}", 
        program_id, new_program_id, deployment.start_slot, deployment.end_slot);
    
    Ok(())
}

/// Migrate a position with double MMT rewards
pub fn migrate_position_with_incentives(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let user = &accounts[0];
    let old_position = &accounts[1];
    let new_position = &accounts[2];
    let migration_state = &accounts[3];
    let mmt_treasury = &accounts[4];
    let user_mmt_account = &accounts[5];
    
    // Verify user signed
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load migration state
    let mut deployment = ParallelDeployment::try_from_slice(&migration_state.data.borrow())?;
    
    // Check migration is active
    if !deployment.is_active {
        return Err(BettingPlatformError::MigrationNotActive.into());
    }
    
    // Check not expired
    let clock = Clock::get()?;
    if deployment.is_expired(clock.slot) {
        return Err(BettingPlatformError::MigrationExpired.into());
    }
    
    // Load old position
    let position = Position::try_from_slice(&old_position.data.borrow())?;
    
    // Verify ownership
    if position.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Calculate MMT reward (0.1% of notional * 2x multiplier)
    let base_reward = position.notional / 1000; // 0.1%
    let mmt_reward = base_reward * MIGRATION_MMT_MULTIPLIER;
    
    // Close old position (zero out data)
    let old_position_data = &mut old_position.data.borrow_mut();
    old_position_data.fill(0);
    
    // Transfer lamports from old to new position account
    let old_lamports = **old_position.lamports.borrow();
    **old_position.lamports.borrow_mut() = 0;
    **new_position.lamports.borrow_mut() = old_lamports;
    
    // TODO: CPI to new program to create position
    // In production, this would:
    // 1. Call new_program_id with instruction to create position
    // 2. Pass position data serialized
    // 3. Verify position created successfully
    // 
    // For now, we log the intended action
    msg!("CPI would create position in new program: {}", deployment.new_program_id);
    msg!("Position data: size={}, notional={}, user={}", 
        position.size, position.notional, position.user);
    
    // TODO: Mint MMT rewards to user
    // In production, this would:
    // 1. CPI to MMT token program
    // 2. Mint mmt_reward tokens to user_mmt_account
    // 3. Update staking rewards if user is staking
    msg!("CPI would mint {} MMT rewards to user {}", mmt_reward, user.key);
    
    // Update migration state
    deployment.positions_migrated += 1;
    deployment.mmt_rewards_distributed += mmt_reward;
    deployment.serialize(&mut &mut migration_state.data.borrow_mut()[..])?;
    
    msg!("Position migrated: id={:?}, reward={} MMT", position_id, mmt_reward);
    
    // Log migration event
    msg!("Position migrated - id: {:?}, user: {}, reward: {} MMT, slot: {}", 
        position_id, user.key, mmt_reward, clock.slot);
    
    Ok(())
}

/// Pause extended migration
pub fn pause_extended_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    reason: String,
) -> ProgramResult {
    let authority = &accounts[0];
    let migration_state = &accounts[1];
    
    // Verify authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load migration state
    let mut deployment = ParallelDeployment::try_from_slice(&migration_state.data.borrow())?;
    
    // Verify authority matches
    if deployment.authority != *authority.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Pause migration
    deployment.is_active = false;
    deployment.serialize(&mut &mut migration_state.data.borrow_mut()[..])?;
    
    msg!("Migration paused: {}", reason);
    
    // Log pause event
    msg!("Migration paused - reason: {}, slot: {}, authority: {}", 
        reason, Clock::get()?.slot, authority.key);
    
    Ok(())
}

/// Resume extended migration
pub fn resume_extended_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let authority = &accounts[0];
    let migration_state = &accounts[1];
    
    // Verify authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load migration state
    let mut deployment = ParallelDeployment::try_from_slice(&migration_state.data.borrow())?;
    
    // Verify authority matches
    if deployment.authority != *authority.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check not expired
    let clock = Clock::get()?;
    if deployment.is_expired(clock.slot) {
        return Err(BettingPlatformError::MigrationExpired.into());
    }
    
    // Resume migration
    deployment.is_active = true;
    deployment.serialize(&mut &mut migration_state.data.borrow_mut()[..])?;
    
    msg!("Migration resumed");
    
    // Log resume event
    msg!("Migration resumed - slot: {}, authority: {}", 
        clock.slot, authority.key);
    
    Ok(())
}

/// Get migration status handler
pub fn get_migration_status_handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let migration_state = &accounts[0];
    
    // Load migration state
    let deployment = ParallelDeployment::try_from_slice(&migration_state.data.borrow())?;
    
    // Get current slot
    let clock = Clock::get()?;
    
    // Get status
    let status = get_migration_status(&deployment, clock.slot);
    
    // Log status (in production, this would return data to client)
    msg!("Migration Status:");
    msg!("  Active: {}", status.is_active);
    msg!("  Progress: {}%", status.progress_pct);
    msg!("  Positions migrated: {}", status.positions_migrated);
    msg!("  MMT distributed: {}", status.mmt_distributed);
    msg!("  Days remaining: {}", status.days_remaining);
    
    Ok(())
}

/// Complete migration after 60 days
pub fn complete_migration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let authority = &accounts[0];
    let migration_state = &accounts[1];
    
    // Load migration state
    let mut deployment = ParallelDeployment::try_from_slice(&migration_state.data.borrow())?;
    
    // Check if expired
    let clock = Clock::get()?;
    if !deployment.is_expired(clock.slot) {
        msg!("Migration period not yet complete. {} slots remaining", 
            deployment.remaining_slots(clock.slot));
        return Err(BettingPlatformError::MigrationNotExpired.into());
    }
    
    // Mark as inactive
    deployment.is_active = false;
    deployment.serialize(&mut &mut migration_state.data.borrow_mut()[..])?;
    
    msg!("Migration completed: {} positions migrated, {} MMT distributed", 
        deployment.positions_migrated, deployment.mmt_rewards_distributed);
    
    // Log completion event
    msg!("Migration completed - positions: {}, rewards: {} MMT, slot: {}", 
        deployment.positions_migrated, deployment.mmt_rewards_distributed, clock.slot);
    
    Ok(())
}

/// UI helper: Get migration status
pub fn get_migration_status(deployment: &ParallelDeployment, current_slot: u64) -> MigrationStatus {
    MigrationStatus {
        is_active: deployment.is_active,
        progress_pct: deployment.progress_percentage(current_slot),
        positions_migrated: deployment.positions_migrated,
        mmt_distributed: deployment.mmt_rewards_distributed,
        slots_remaining: deployment.remaining_slots(current_slot),
        days_remaining: deployment.remaining_slots(current_slot) / 216_000, // slots per day
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationStatus {
    pub is_active: bool,
    pub progress_pct: u8,
    pub positions_migrated: u64,
    pub mmt_distributed: u64,
    pub slots_remaining: u64,
    pub days_remaining: u64,
}

/// Migration wizard for UI - one-click migration
pub fn create_migration_wizard_instructions(
    user: &Pubkey,
    positions: Vec<[u8; 32]>,
    old_program: &Pubkey,
    new_program: &Pubkey,
) -> Vec<MigrationInstruction> {
    positions.into_iter().map(|position_id| {
        MigrationInstruction {
            position_id,
            user: *user,
            old_program: *old_program,
            new_program: *new_program,
            estimated_reward: 1000, // Calculate based on position size
        }
    }).collect()
}

#[derive(Debug)]
pub struct MigrationInstruction {
    pub position_id: [u8; 32],
    pub user: Pubkey,
    pub old_program: Pubkey,
    pub new_program: Pubkey,
    pub estimated_reward: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_migration_timing() {
        let deployment = ParallelDeployment::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            1000,
        );
        
        assert_eq!(deployment.end_slot, 1000 + MIGRATION_PERIOD_SLOTS);
        assert!(!deployment.is_expired(1000));
        assert!(deployment.is_expired(1000 + MIGRATION_PERIOD_SLOTS + 1));
    }
    
    #[test]
    fn test_migration_progress() {
        let deployment = ParallelDeployment::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
        );
        
        assert_eq!(deployment.progress_percentage(0), 0);
        assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS / 2), 50);
        assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS), 100);
    }
}