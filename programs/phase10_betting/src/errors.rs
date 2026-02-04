use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Bootstrap phase is not active")]
    BootstrapNotActive,
    
    #[msg("Trade volume below minimum")]
    TradeTooSmall,
    
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
    
    #[msg("Math overflow")]
    MathOverflow,
    
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    
    #[msg("No liquidity available")]
    NoLiquidityAvailable,
    
    #[msg("Unsupported routing strategy")]
    UnsupportedRoutingStrategy,
    
    #[msg("Invalid market type")]
    InvalidMarketType,
    
    #[msg("AMM transition in progress")]
    AMMTransitionInProgress,
    
    #[msg("Router not initialized")]
    RouterNotInitialized,
    
    #[msg("Child market limit reached")]
    ChildMarketLimitReached,
    
    #[msg("Invalid probability value")]
    InvalidProbability,
    
    #[msg("Unauthorized")]
    Unauthorized,
    
    #[msg("Emergency pause active")]
    EmergencyPauseActive,
    
    #[msg("Coverage target not met")]
    CoverageTargetNotMet,
    
    #[msg("Bootstrap already completed")]
    BootstrapAlreadyCompleted,
    
    #[msg("Invalid leverage")]
    InvalidLeverage,
    
    #[msg("Position not found")]
    PositionNotFound,
    
    #[msg("Insufficient collateral")]
    InsufficientCollateral,
    
    #[msg("Market expired")]
    MarketExpired,
    
    #[msg("Invalid fee configuration")]
    InvalidFeeConfiguration,
}