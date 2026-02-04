use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Type of ZK proof used for compression
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ProofType {
    Merkle,
    ZKSnark,
    Poseidon,
}

/// Compressed state proof containing multiple markets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressedStateProof {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Is initialized
    pub is_initialized: bool,
    
    /// Authority that created this proof
    pub authority: Pubkey,
    
    /// Proof hash (32 bytes)
    pub proof_hash: [u8; 32],
    
    /// State root hash
    pub state_root: [u8; 32],
    
    /// Timestamp of compression
    pub timestamp: i64,
    
    /// Number of markets in this proof
    pub market_count: u32,
    
    /// Original uncompressed size in bytes
    pub uncompressed_size: u64,
    
    /// Compressed size in bytes
    pub compressed_size: u64,
    
    /// Type of proof used
    pub proof_type: ProofType,
    
    /// Compression version
    pub compression_version: u8,
    
    /// Slot when compressed
    pub slot: u64,
    
    /// Market IDs included (first 10 for reference)
    pub sample_market_ids: [[u8; 32]; 10],
    
    /// Proof data (variable size, but we allocate fixed space)
    pub proof_data: Vec<u8>,
}

impl CompressedStateProof {
    pub const DISCRIMINATOR: [u8; 8] = [67, 79, 77, 80, 95, 80, 82, 70]; // "COMP_PRF"
    
    pub const FIXED_LEN: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // authority
        32 + // proof_hash
        32 + // state_root
        8 + // timestamp
        4 + // market_count
        8 + // uncompressed_size
        8 + // compressed_size
        1 + // proof_type
        1 + // compression_version
        8 + // slot
        320; // sample_market_ids (10 * 32)
    
    // Maximum account size (10KB for proof data)
    pub const MAX_SIZE: usize = Self::FIXED_LEN + 4 + 10240; // 4 bytes for vec length + 10KB data
    
    /// Create new compressed state proof
    pub fn new(
        authority: Pubkey,
        proof_hash: [u8; 32],
        state_root: [u8; 32],
        timestamp: i64,
        market_count: u32,
        uncompressed_size: u64,
        compressed_size: u64,
        proof_type: ProofType,
        slot: u64,
        proof_data: Vec<u8>,
    ) -> Result<Self, ProgramError> {
        // Validate proof data size
        if proof_data.len() > 10240 {
            return Err(crate::error::CompressionError::InvalidProofData.into());
        }
        
        // Validate compression ratio
        if compressed_size >= uncompressed_size {
            return Err(crate::error::CompressionError::CompressionRatioBelowMinimum.into());
        }
        
        Ok(Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            authority,
            proof_hash,
            state_root,
            timestamp,
            market_count,
            uncompressed_size,
            compressed_size,
            proof_type,
            compression_version: 1,
            slot,
            sample_market_ids: [[0u8; 32]; 10], // Will be filled by caller
            proof_data,
        })
    }
    
    /// Validate proof structure
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check discriminator
        if self.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Check initialized
        if !self.is_initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        
        // Check market count
        if self.market_count == 0 {
            return Err(crate::error::CompressionError::InvalidProofData.into());
        }
        
        // Check compression achieved
        if self.compressed_size >= self.uncompressed_size {
            return Err(crate::error::CompressionError::CompressionRatioBelowMinimum.into());
        }
        
        // Validate proof data size
        if self.proof_data.len() > 10240 {
            return Err(crate::error::CompressionError::InvalidProofData.into());
        }
        
        Ok(())
    }
    
    /// Get compression ratio achieved
    pub fn get_compression_ratio(&self) -> f64 {
        if self.compressed_size == 0 {
            return 0.0;
        }
        self.uncompressed_size as f64 / self.compressed_size as f64
    }
    
    /// Check if market is in sample list
    pub fn contains_market_sample(&self, market_id: &[u8; 32]) -> bool {
        self.sample_market_ids.iter().any(|id| id == market_id)
    }
}

/// Proof index entry for finding proofs containing specific markets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ProofIndexEntry {
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Proof account containing this market
    pub proof_pubkey: Pubkey,
    
    /// Proof hash for verification
    pub proof_hash: [u8; 32],
    
    /// Position in proof (for extraction)
    pub position: u32,
}

impl ProofIndexEntry {
    pub const LEN: usize = 32 + // market_id
        32 + // proof_pubkey
        32 + // proof_hash
        4; // position
}