use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient coverage for leverage")]
    InsufficientCoverage,
    
    #[msg("Maximum leverage exceeded")]
    MaxLeverageExceeded,
    
    #[msg("System halted")]
    SystemHalted,
    
    #[msg("Invalid verse hierarchy")]
    InvalidVerseHierarchy,
    
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    
    #[msg("Arithmetic underflow")]
    ArithmeticUnderflow,
    
    #[msg("Division by zero")]
    DivisionByZero,
    
    #[msg("Invalid input")]
    InvalidInput,
    
    #[msg("Emergency halt expired")]
    EmergencyHaltExpired,
    
    #[msg("Invalid coverage")]
    InvalidCoverage,
    
    #[msg("Invalid vault decrease")]
    InvalidVaultDecrease,
    
    #[msg("Invalid fee rate")]
    InvalidFeeRate,
    
    #[msg("Verse not found")]
    VerseNotFound,
    
    #[msg("Proposal not found")]
    ProposalNotFound,
    
    #[msg("Unauthorized")]
    Unauthorized,
    
    #[msg("Invalid proposal status")]
    InvalidProposalStatus,
    
    #[msg("Proposal expired")]
    ProposalExpired,
    
    #[msg("Invalid outcome")]
    InvalidOutcome,
    
    #[msg("Already resolved")]
    AlreadyResolved,
    
    #[msg("Insufficient balance")]
    InsufficientBalance,
    
    #[msg("Invalid leverage tier")]
    InvalidLeverageTier,
    
    #[msg("Position not found")]
    PositionNotFound,
    
    #[msg("Invalid oracle feed")]
    InvalidOracleFeed,
    
    #[msg("Mint authority not burned")]
    MintAuthorityNotBurned,
    
    #[msg("Excessive price movement detected")]
    ExcessivePriceMovement,
    
    #[msg("Maximum depth exceeded")]
    MaxDepthExceeded,
    
    #[msg("Circular hierarchy detected")]
    CircularHierarchy,
    
    #[msg("Verse not active")]
    VerseNotActive,
    
    #[msg("Verse already settled")]
    VerseSettled,
    
    #[msg("Stale price data")]
    StalePrice,
    
    #[msg("Excessive leverage")]
    ExcessiveLeverage,
    
    #[msg("Math overflow")]
    MathOverflow,
    
    #[msg("Invalid position")]
    InvalidPosition,
    
    #[msg("Position healthy - cannot liquidate")]
    PositionHealthy,
    
    #[msg("Circuit breaker triggered")]
    CircuitBreakerTriggered,
    
    #[msg("Low coverage")]
    LowCoverage,
    
    #[msg("Inconsistent coverage")]
    InconsistentCoverage,
    
    #[msg("Insufficient vault balance")]
    InsufficientVaultBalance,
    
    #[msg("Invalid MMT supply")]
    InvalidMMTSupply,
    
    #[msg("Invalid deposit amount")]
    InvalidDeposit,
    
    #[msg("Too many chain steps")]
    TooManySteps,
    
    #[msg("No chain steps provided")]
    NoSteps,
    
    #[msg("Inactive verse")]
    InactiveVerse,
    
    #[msg("Wrong verse")]
    WrongVerse,
    
    #[msg("Invalid chain status")]
    InvalidChainStatus,
    
    #[msg("Chain cycle detected")]
    ChainCycle,
    
    #[msg("Exceeds verse limit")]
    ExceedsVerseLimit,
    
    #[msg("Insufficient liquidation buffer")]
    InsufficientLiquidationBuffer,
    
    #[msg("Unauthorized emergency shutdown attempt")]
    UnauthorizedEmergency,
    
    #[msg("Attack detected")]
    AttackDetected,
    
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
    
    // AMM-specific errors
    #[msg("Invalid shares amount")]
    InvalidShares,
    
    #[msg("Price sum does not equal 1")]
    PriceSumError,
    
    #[msg("Convergence failed in numerical solver")]
    ConvergenceFailed,
    
    #[msg("Insufficient points for integration")]
    InsufficientPoints,
    
    #[msg("Unsupported distribution type")]
    UnsupportedDistribution,
    
    #[msg("Invalid position index")]
    InvalidPositionIndex,
    
    // Advanced trading errors
    #[msg("Invalid visible size for iceberg order")]
    InvalidVisibleSize,
    
    #[msg("Invalid total size for iceberg order")]
    InvalidTotalSize,
    
    #[msg("Visible size too large for iceberg order")]
    VisibleSizeTooLarge,
    
    #[msg("Exceeds visible size")]
    ExceedsVisibleSize,
    
    #[msg("Not an iceberg order")]
    NotIcebergOrder,
    
    #[msg("Invalid intervals for TWAP order")]
    InvalidIntervals,
    
    #[msg("Duration too short for TWAP order")]
    DurationTooShort,
    
    #[msg("Size too small for order")]
    SizeTooSmall,
    
    #[msg("Invalid TWAP state")]
    InvalidTWAPState,
    
    #[msg("Too early for TWAP execution")]
    TooEarlyForTWAP,
    
    #[msg("Not a TWAP order")]
    NotTWAPOrder,
    
    // Test-specific errors
    #[msg("Price clamp exceeded")]
    PriceClampExceeded,
    
    #[msg("Invalid distribution")]
    InvalidDistribution,
    
    #[msg("Circular borrow detected")]
    CircularBorrow,
    
    #[msg("Cross-verse borrow not allowed")]
    CrossVerseBorrowNotAllowed,
    
    #[msg("Invalid order type")]
    InvalidOrderType,
    
    #[msg("Invalid global config")]
    InvalidGlobalConfig,
    
    #[msg("Invalid user map")]
    InvalidUserMap,
    
    #[msg("Invalid compression proof")]
    InvalidCompressionProof,
    
    #[msg("Decompression failed")]
    DecompressionFailed,
    
    #[msg("No active keepers available")]
    NoActiveKeepers,
    
    #[msg("Position not at risk")]
    PositionNotAtRisk,
    
    #[msg("Stop condition not met")]
    StopConditionNotMet,
    
    #[msg("Insufficient prepaid bounty")]
    InsufficientPrepaidBounty,
    
    #[msg("Stale price update")]
    StalePriceUpdate,
    
    #[msg("No backup keeper available")]
    NoBackupKeeperAvailable,
    
    #[msg("Market not found")]
    MarketNotFound,
    
    #[msg("Exceeds compute unit limit")]
    ExceedsCULimit,
}

pub type BettingPlatformError = ErrorCode;