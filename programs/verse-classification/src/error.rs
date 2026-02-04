use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    program_error::{PrintProgramError, ProgramError},
};
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone, FromPrimitive, PartialEq)]
pub enum ClassificationError {
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
    
    #[error("Invalid regex pattern")]
    InvalidRegex = 8,
    
    #[error("Invalid date format")]
    InvalidDate = 9,
    
    #[error("Verse not found")]
    VerseNotFound = 10,
    
    #[error("Max depth exceeded")]
    MaxDepthExceeded = 11,
    
    #[error("Title too long")]
    TitleTooLong = 12,
    
    #[error("Too many keywords")]
    TooManyKeywords = 13,
    
    #[error("Invalid normalization config")]
    InvalidNormalizationConfig = 14,
    
    #[error("Insufficient data for correlation")]
    InsufficientData = 15,
    
    #[error("Invalid outcome count")]
    InvalidOutcomeCount = 16,
    
    #[error("Unauthorized")]
    Unauthorized = 17,
}

impl PrintProgramError for ClassificationError {
    fn print<E>(&self) {
        use solana_program::msg;
        msg!("ClassificationError: {}", self);
    }
}

impl From<ClassificationError> for ProgramError {
    fn from(e: ClassificationError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for ClassificationError {
    fn type_of() -> &'static str {
        "ClassificationError"
    }
}