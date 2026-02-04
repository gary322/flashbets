use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum LeverageSafetyError {
    #[error("Invalid instruction")]
    InvalidInstruction,
    
    #[error("Account not initialized")]
    AccountNotInitialized,
    
    #[error("Account already initialized")]
    AccountAlreadyInitialized,
    
    #[error("Invalid authority")]
    InvalidAuthority,
    
    #[error("Coverage below minimum")]
    CoverageBelowMinimum,
    
    #[error("Max leverage exceeded")]
    MaxLeverageExceeded,
    
    #[error("Invalid outcome count")]
    InvalidOutcomeCount,
    
    #[error("Depth limit exceeded")]
    DepthLimitExceeded,
    
    #[error("Invalid tier configuration")]
    InvalidTierConfiguration,
    
    #[error("Position health critical")]
    PositionHealthCritical,
    
    #[error("Liquidation amount exceeds limit")]
    LiquidationAmountExceedsLimit,
    
    #[error("Invalid correlation factor")]
    InvalidCorrelationFactor,
    
    #[error("Invalid volatility value")]
    InvalidVolatility,
    
    #[error("Arithmetic overflow")]
    ArithmeticOverflow,
    
    #[error("Division by zero")]
    DivisionByZero,
    
    #[error("Emergency halt active")]
    EmergencyHaltActive,
    
    #[error("Price data stale")]
    PriceDataStale,
}

impl From<LeverageSafetyError> for ProgramError {
    fn from(e: LeverageSafetyError) -> Self {
        ProgramError::Custom(e as u32)
    }
}