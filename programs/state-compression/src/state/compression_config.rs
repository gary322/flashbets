use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Configuration for state compression system
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressionConfig {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Is initialized
    pub is_initialized: bool,
    
    /// Authority that can update configuration
    pub authority: Pubkey,
    
    /// Compression enabled flag
    pub enabled: bool,
    
    /// Target compression ratio (e.g., 10 for 10x compression)
    pub compression_ratio: u8,
    
    /// Compute units required for proof verification (~2k CU)
    pub proof_verification_cu: u32,
    
    /// Maximum batch size for compression
    pub batch_size: u16,
    
    /// Last compression timestamp
    pub last_compression: i64,
    
    /// Total markets compressed
    pub total_compressed: u64,
    
    /// Total space saved in bytes
    pub space_saved: u64,
    
    /// Minimum markets required for compression
    pub min_markets_for_compression: u16,
    
    /// Maximum proof size in bytes
    pub max_proof_size: u32,
    
    /// Compression algorithm version
    pub compression_version: u8,
    
    /// Emergency pause flag
    pub emergency_pause: bool,

    /// Reserved bytes for future upgrades (keeps account size stable)
    pub reserved: [u8; 64],
}

impl CompressionConfig {
    pub const DISCRIMINATOR: [u8; 8] = [67, 79, 77, 80, 95, 67, 70, 71]; // "COMP_CFG"
    
    pub const LEN: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // authority
        1 + // enabled
        1 + // compression_ratio
        4 + // proof_verification_cu
        2 + // batch_size
        8 + // last_compression
        8 + // total_compressed
        8 + // space_saved
        2 + // min_markets_for_compression
        4 + // max_proof_size
        1 + // compression_version
        1 + // emergency_pause
        64; // reserved
    
    /// Create default configuration
    pub fn default(authority: Pubkey) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            authority,
            enabled: true,
            compression_ratio: 10, // Target 10x compression as per CLAUDE.md
            proof_verification_cu: 2000, // ~2k CU per verification
            batch_size: 100, // Max 100 markets per batch
            last_compression: 0,
            total_compressed: 0,
            space_saved: 0,
            min_markets_for_compression: 10,
            max_proof_size: 1024 * 10, // 10KB max proof size
            compression_version: 1,
            emergency_pause: false,
            reserved: [0u8; 64],
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check discriminator
        if self.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Check initialized
        if !self.is_initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        
        // Validate compression ratio
        if self.compression_ratio == 0 || self.compression_ratio > 100 {
            return Err(crate::error::CompressionError::CompressionRatioBelowMinimum.into());
        }
        
        // Validate batch size
        if self.batch_size == 0 || self.batch_size > 1000 {
            return Err(crate::error::CompressionError::BatchTooLarge.into());
        }
        
        // Validate proof verification CU
        if self.proof_verification_cu == 0 || self.proof_verification_cu > 10000 {
            return Err(ProgramError::InvalidInstructionData);
        }
        
        Ok(())
    }
    
    /// Check if compression is allowed
    pub fn can_compress(&self, market_count: usize) -> bool {
        self.enabled && 
        !self.emergency_pause &&
        market_count >= self.min_markets_for_compression as usize
    }
    
    /// Update compression statistics
    pub fn update_stats(&mut self, markets_compressed: u64, space_saved: u64, timestamp: i64) {
        self.total_compressed += markets_compressed;
        self.space_saved += space_saved;
        self.last_compression = timestamp;
    }
}
