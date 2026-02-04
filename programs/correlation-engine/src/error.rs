use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    program_error::{PrintProgramError, ProgramError},
};
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone, FromPrimitive, PartialEq)]
pub enum CorrelationError {
    #[error("Invalid instruction")]
    InvalidInstruction = 0,
    
    #[error("Invalid account data")]
    InvalidAccountData = 1,
    
    #[error("Account not found")]
    AccountNotFound = 2,
    
    #[error("Invalid PDA")]
    InvalidPDA = 3,
    
    #[error("Already initialized")]
    AlreadyInitialized = 4,
    
    #[error("Not initialized")]
    NotInitialized = 5,
    
    #[error("Invalid authority")]
    InvalidAuthority = 6,
    
    #[error("Arithmetic overflow")]
    ArithmeticOverflow = 7,
    
    #[error("Insufficient data")]
    InsufficientData = 8,
    
    #[error("Invalid outcome count")]
    InvalidOutcomeCount = 9,
    
    #[error("Mismatched data length")]
    MismatchedDataLength = 10,
    
    #[error("Divide by zero")]
    DivideByZero = 11,
    
    #[error("Weight mismatch")]
    WeightMismatch = 12,
    
    #[error("Too many markets")]
    TooManyMarkets = 13,
    
    #[error("Invalid market index")]
    InvalidMarket = 14,
    
    #[error("Unauthorized")]
    Unauthorized = 15,
    
    #[error("Invalid market count")]
    InvalidMarketCount = 16,
}

impl PrintProgramError for CorrelationError {
    fn print<E>(&self) {
        use solana_program::msg;
        msg!("CorrelationError: {}", self);
    }
}

impl From<CorrelationError> for ProgramError {
    fn from(e: CorrelationError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for CorrelationError {
    fn type_of() -> &'static str {
        "CorrelationError"
    }
}