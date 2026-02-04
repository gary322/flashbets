//! Migration-specific event definitions
//!
//! Events for tracking migration progress and state changes

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

/// Migration started event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationStarted {
    pub from_version: u32,
    pub to_version: u32,
    pub total_accounts: u64,
    pub timestamp: i64,
}

/// Position migrated event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionMigrated {
    pub position_id: u128,
    pub user: Pubkey,
    pub from_version: u32,
    pub to_version: u32,
    pub incentive_amount: u64,
    pub timestamp: i64,
}

/// Migration completed event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationCompleted {
    pub from_version: u32,
    pub to_version: u32,
    pub total_migrated: u64,
    pub duration_slots: u64,
    pub timestamp: i64,
}

/// Migration paused event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationPaused {
    pub reason: String,
    pub current_progress: u64,
    pub total_accounts: u64,
    pub timestamp: i64,
}

/// Migration resumed event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationResumed {
    pub current_progress: u64,
    pub total_accounts: u64,
    pub timestamp: i64,
}

/// Emit migration event
impl MigrationStarted {
    pub fn emit(&self) {
        use crate::events::emit_event;
        emit_event(crate::events::EventType::MigrationStarted, self);
    }
}

impl PositionMigrated {
    pub fn emit(&self) {
        use crate::events::emit_event;
        emit_event(crate::events::EventType::PositionMigrated, self);
    }
}

impl MigrationCompleted {
    pub fn emit(&self) {
        use crate::events::emit_event;
        emit_event(crate::events::EventType::MigrationCompleted, self);
    }
}

impl MigrationPaused {
    pub fn emit(&self) {
        use crate::events::emit_event;
        emit_event(crate::events::EventType::MigrationPaused, self);
    }
}

impl MigrationResumed {
    pub fn emit(&self) {
        use crate::events::emit_event;
        emit_event(crate::events::EventType::MigrationResumed, self);
    }
}
