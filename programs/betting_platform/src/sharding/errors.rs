use anchor_lang::prelude::*;

#[error_code]
pub enum ShardingError {
    #[msg("No markets to move in rebalance proposal")]
    NoMarketsToMove,
    
    #[msg("Insufficient improvement in rebalance proposal")]
    InsufficientImprovement,
    
    #[msg("Unauthorized keeper attempting to vote")]
    UnauthorizedKeeper,
    
    #[msg("Rebalance proposal not found")]
    ProposalNotFound,
    
    #[msg("Migration not found")]
    MigrationNotFound,
    
    #[msg("Failed to pause market writes")]
    FailedToPauseWrites,
    
    #[msg("Failed to resume market writes")]
    FailedToResumeWrites,
    
    #[msg("Failed to transfer state to new shard")]
    FailedStateTransfer,
    
    #[msg("Failed to update shard assignment")]
    FailedShardAssignment,
    
    #[msg("Market state not found")]
    MarketStateNotFound,
    
    #[msg("Invalid shard ID")]
    InvalidShardId,
    
    #[msg("Migration timeout exceeded")]
    MigrationTimeout,
    
    #[msg("Shard is overloaded")]
    ShardOverloaded,
    
    #[msg("Invalid contention metrics")]
    InvalidContentionMetrics,
    
    #[msg("Rebalance already in progress")]
    RebalanceInProgress,
}