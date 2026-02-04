// Migration module
// Native Solana implementation - NO ANCHOR

pub mod core;
pub mod position_migration;
pub mod verse_migration;
pub mod coordinator;
pub mod safety;
pub mod instruction;
pub mod entrypoint;

// Re-export commonly used types
pub use core::{
    MigrationState, MigrationStatus, MigrationType,
    PositionSnapshot, VerseSnapshot, ChainSnapshot,
    PositionSide, ChainStepType,
    MigrationProgress,
    MIGRATION_STATE_DISCRIMINATOR,
    POSITION_SNAPSHOT_DISCRIMINATOR,
    VERSE_SNAPSHOT_DISCRIMINATOR,
    MIGRATION_NOTICE_PERIOD,
    MIGRATION_DURATION,
};

pub use position_migration::PositionMigrator;
pub use verse_migration::VerseMigrator;
pub use coordinator::MigrationCoordinator;
pub use safety::{MigrationSafety, PauseReason, IntegrityReport};
pub use instruction::{
    MigrationInstruction,
    process_instruction,
    build_initialize_migration_instruction,
    build_migrate_position_instruction,
};