use anchor_lang::prelude::*;

#[error_code]
pub enum ShardingError {
    #[msg("Shard assignment failed")]
    ShardAssignmentFailed,
    
    #[msg("Contention threshold exceeded")]
    ContentionThresholdExceeded,
    
    #[msg("Invalid shard ID")]
    InvalidShardId,
    
    #[msg("Rebalance already in progress")]
    RebalanceInProgress,
    
    #[msg("No rebalance needed")]
    NoRebalanceNeeded,
}

#[error_code]
pub enum RebalanceError {
    #[msg("No markets to move")]
    NoMarketsToMove,
    
    #[msg("Insufficient improvement")]
    InsufficientImprovement,
    
    #[msg("Unauthorized keeper")]
    UnauthorizedKeeper,
    
    #[msg("Proposal not found")]
    ProposalNotFound,
    
    #[msg("Voting period ended")]
    VotingPeriodEnded,
    
    #[msg("Proposal already executed")]
    ProposalAlreadyExecuted,
    
    #[msg("Insufficient vote count")]
    InsufficientVoteCount,
}

#[error_code]
pub enum MigrationError {
    #[msg("Migration not found")]
    MigrationNotFound,
    
    #[msg("Migration already in progress")]
    MigrationAlreadyInProgress,
    
    #[msg("Failed to take market snapshot")]
    SnapshotFailed,
    
    #[msg("Failed to pause market writes")]
    PauseWritesFailed,
    
    #[msg("Failed to transfer state")]
    StateTransferFailed,
    
    #[msg("Migration timeout")]
    MigrationTimeout,
    
    #[msg("Invalid migration state")]
    InvalidMigrationState,
}