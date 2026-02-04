use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum CompressionError {
    #[error("Compression disabled")]
    CompressionDisabled,
    
    #[error("Batch too large")]
    BatchTooLarge,
    
    #[error("Proof mismatch")]
    ProofMismatch,
    
    #[error("Proof verification failed")]
    ProofVerificationFailed,
    
    #[error("Unsupported proof type")]
    UnsupportedProofType,
    
    #[error("Market not in compressed state")]
    MarketNotInCompressedState,
    
    #[error("Invalid proof data")]
    InvalidProofData,
    
    #[error("Compression ratio below minimum")]
    CompressionRatioBelowMinimum,
    
    #[error("Decompression cache full")]
    DecompressionCacheFull,
    
    #[error("Stale cache entry")]
    StaleCacheEntry,
    
    #[error("Invalid market data")]
    InvalidMarketData,
    
    #[error("Arithmetic overflow")]
    ArithmeticOverflow,
    
    #[error("Invalid authority")]
    InvalidAuthority,
    
    #[error("Already initialized")]
    AlreadyInitialized,
}

impl From<CompressionError> for ProgramError {
    fn from(e: CompressionError) -> Self {
        ProgramError::Custom(e as u32)
    }
}