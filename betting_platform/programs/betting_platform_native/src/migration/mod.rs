//! Migration module for 60-day parallel deployment
//!
//! This module provides the complete migration framework for upgrading
//! from one immutable program to another with a 60-day transition period
//! and double MMT incentives for users who migrate their positions.

pub mod extended_migration;
pub mod migration_events;
pub mod migration_rewards;
pub mod auto_wizard;
pub mod migration_ui;
pub mod halt_mechanism;

pub use extended_migration::*;
pub use migration_events::*;
pub use migration_rewards::*;
pub use auto_wizard::*;
pub use migration_ui::*;
pub use halt_mechanism::*;

// Re-export key types
pub use extended_migration::{
    ParallelDeployment,
    MigrationStatus,
    MigrationInstruction,
    MIGRATION_PERIOD_SLOTS,
    MIGRATION_MMT_MULTIPLIER,
};

pub use migration_events::{
    MigrationStarted,
    PositionMigrated,
    MigrationCompleted,
    MigrationPaused,
    MigrationResumed,
};

#[cfg(test)]
mod tests;