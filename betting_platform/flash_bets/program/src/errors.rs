use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum FlashError {
    #[error("Market time exceeds 4 hours - not a flash market")]
    NotFlashMarket,
    
    #[error("Insufficient outcomes - minimum 2 required")]
    InsufficientOutcomes,
    
    #[error("Too many outcomes - maximum 10 allowed")]
    TooManyOutcomes,
    
    #[error("Market already resolved")]
    MarketResolved,
    
    #[error("Market expired")]
    MarketExpired,

    #[error("Market not ready to settle")]
    MarketNotSettled,
    
    #[error("Invalid outcome index")]
    InvalidOutcome,
    
    #[error("Invalid amount")]
    InvalidAmount,
    
    #[error("Excessive slippage")]
    ExcessiveSlippage,
    
    #[error("Too many chain steps - maximum 5")]
    TooManyChainSteps,
    
    #[error("Excessive leverage - maximum 500x")]
    ExcessiveLeverage,
    
    #[error("Invalid ZK proof")]
    InvalidProof,
    
    #[error("Already resolved")]
    AlreadyResolved,
    
    #[error("Already collapsed")]
    AlreadyCollapsed,
    
    #[error("Market not resolved")]
    MarketNotResolved,
    
    #[error("No winning outcome")]
    NoWinningOutcome,
    
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,
    
    #[error("Circuit breaker triggered")]
    CircuitBreakerOpen,
    
    #[error("Provider unavailable")]
    ProviderUnavailable,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Geographic restriction")]
    GeographicRestriction,
}

impl From<FlashError> for ProgramError {
    fn from(e: FlashError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
