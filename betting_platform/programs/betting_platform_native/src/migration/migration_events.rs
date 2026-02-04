//! Migration events for parallel deployment
//!
//! Events emitted during the 60-day migration process

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    msg,
};

/// Event emitted when migration starts
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationStarted {
    pub old_program: Pubkey,
    pub new_program: Pubkey,
    pub start_slot: u64,
    pub end_slot: u64,
    pub authority: Pubkey,
}

impl MigrationStarted {
    pub fn emit(&self) {
        msg!("EVENT: MigrationStarted");
        msg!("  old_program: {}", self.old_program);
        msg!("  new_program: {}", self.new_program);
        msg!("  start_slot: {}", self.start_slot);
        msg!("  end_slot: {}", self.end_slot);
        msg!("  authority: {}", self.authority);
    }
}

/// Event emitted when a position is migrated
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PositionMigrated {
    pub position_id: [u8; 32],
    pub user: Pubkey,
    pub old_program: Pubkey,
    pub new_program: Pubkey,
    pub mmt_reward: u64,
    pub migration_slot: u64,
}

impl PositionMigrated {
    pub fn emit(&self) {
        msg!("EVENT: PositionMigrated");
        msg!("  position_id: {:?}", &self.position_id[..8]); // Log first 8 bytes
        msg!("  user: {}", self.user);
        msg!("  old_program: {}", self.old_program);
        msg!("  new_program: {}", self.new_program);
        msg!("  mmt_reward: {}", self.mmt_reward);
        msg!("  migration_slot: {}", self.migration_slot);
    }
}

/// Event emitted when migration completes
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationCompleted {
    pub old_program: Pubkey,
    pub new_program: Pubkey,
    pub positions_migrated: u64,
    pub mmt_rewards: u64,
    pub completion_slot: u64,
}

impl MigrationCompleted {
    pub fn emit(&self) {
        msg!("EVENT: MigrationCompleted");
        msg!("  old_program: {}", self.old_program);
        msg!("  new_program: {}", self.new_program);
        msg!("  positions_migrated: {}", self.positions_migrated);
        msg!("  mmt_rewards: {}", self.mmt_rewards);
        msg!("  completion_slot: {}", self.completion_slot);
    }
}

/// Event emitted when migration is paused
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationPaused {
    pub reason: String,
    pub paused_at: u64,
    pub authority: Pubkey,
}

impl MigrationPaused {
    pub fn emit(&self) {
        msg!("EVENT: MigrationPaused");
        msg!("  reason: {}", self.reason);
        msg!("  paused_at: {}", self.paused_at);
        msg!("  authority: {}", self.authority);
    }
}

/// Event emitted when migration is resumed
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationResumed {
    pub resumed_at: u64,
    pub authority: Pubkey,
}

impl MigrationResumed {
    pub fn emit(&self) {
        msg!("EVENT: MigrationResumed");
        msg!("  resumed_at: {}", self.resumed_at);
        msg!("  authority: {}", self.authority);
    }
}