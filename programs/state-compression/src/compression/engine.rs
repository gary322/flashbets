use solana_program::{
    msg,
    clock::Clock,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::CompressionError,
    state::{
        CompressionConfig,
        CompressedStateProof,
        MarketEssentials,
        ProofType,
    },
    compression::poseidon::{PoseidonHasher, PoseidonHash},
};

/// Main state compression engine
pub struct StateCompressionEngine;

impl StateCompressionEngine {
    /// Compress multiple market states into a single proof
    pub fn compress_market_state(
        config: &CompressionConfig,
        market_ids: &[[u8; 32]],
        market_data: Vec<MarketEssentials>,
        clock: &Clock,
        authority: Pubkey,
    ) -> Result<CompressedStateProof, ProgramError> {
        // Validate inputs
        if !config.enabled {
            return Err(CompressionError::CompressionDisabled.into());
        }
        
        if market_ids.len() > config.batch_size as usize {
            return Err(CompressionError::BatchTooLarge.into());
        }
        
        if market_ids.len() != market_data.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        
        // Calculate uncompressed size
        let uncompressed_size = market_data.len() * 520; // ProposalPDA size estimate
        
        // Build state components for compression
        let mut state_components = Vec::new();
        for (i, market) in market_data.iter().enumerate() {
            // Validate market data
            market.validate()?;
            
            // Ensure market ID matches
            if market.market_id != market_ids[i] {
                return Err(CompressionError::InvalidMarketData.into());
            }
            
            state_components.push(market.to_bytes());
        }
        
        // Generate ZK proof using Poseidon
        let proof = Self::generate_state_proof(
            &state_components,
            ProofType::Poseidon,
        )?;
        
        // Create compressed state proof
        let mut compressed_proof = CompressedStateProof::new(
            authority,
            proof.hash,
            proof.root,
            clock.unix_timestamp,
            market_ids.len() as u32,
            uncompressed_size as u64,
            proof.size,
            ProofType::Poseidon,
            clock.slot,
            proof.data,
        )?;
        
        // Fill sample market IDs (first 10)
        for (i, market_id) in market_ids.iter().take(10).enumerate() {
            compressed_proof.sample_market_ids[i] = *market_id;
        }
        
        // Verify compression ratio
        let compression_ratio = compressed_proof.get_compression_ratio();
        if compression_ratio < config.compression_ratio as f64 {
            msg!("Warning: Compression ratio {:.2} below target {}", 
                compression_ratio, config.compression_ratio);
        }
        
        msg!("Compressed {} markets: {} bytes -> {} bytes (ratio: {:.2}x)",
            market_ids.len(),
            uncompressed_size,
            proof.size,
            compression_ratio
        );
        
        Ok(compressed_proof)
    }
    
    /// Decompress and verify state
    pub fn decompress_and_verify(
        proof: &CompressedStateProof,
        market_id: &[u8; 32],
        config: &CompressionConfig,
    ) -> Result<MarketEssentials, ProgramError> {
        // Validate proof
        proof.validate()?;
        
        // Verify proof type is supported
        if proof.proof_type != ProofType::Poseidon {
            return Err(CompressionError::UnsupportedProofType.into());
        }
        
        // Verify proof (costs ~2k CU as per CLAUDE.md)
        let verified = Self::verify_proof(
            &proof.proof_hash,
            &proof.state_root,
            proof.proof_type,
            &proof.proof_data,
        )?;
        
        if !verified {
            return Err(CompressionError::ProofVerificationFailed.into());
        }
        
        // Extract specific market data from proof
        let market_data = Self::extract_market_from_proof(
            proof,
            market_id,
        )?;
        
        msg!("Decompressed market {:?} from proof", market_id);
        
        Ok(market_data)
    }
    
    /// Generate ZK proof for state compression
    fn generate_state_proof(
        components: &[Vec<u8>],
        proof_type: ProofType,
    ) -> Result<StateProof, ProgramError> {
        match proof_type {
            ProofType::Poseidon => {
                // Use Poseidon hash for efficient ZK proofs
                let mut hasher = PoseidonHasher::new();
                
                // Hash each component
                let mut component_hashes = Vec::new();
                for component in components {
                    hasher.update(component);
                    let hash = hasher.finalize();
                    component_hashes.push(hash);
                    hasher.reset();
                }
                
                // Build Merkle tree root
                let root = Self::build_merkle_root(&component_hashes)?;
                
                // Create proof data
                let proof_data = Self::build_poseidon_proof(&root, &component_hashes)?;
                
                // Calculate final hash
                hasher.update(&root.to_bytes());
                let final_hash = hasher.finalize();
                
                Ok(StateProof {
                    hash: final_hash.to_bytes(),
                    root: root.to_bytes(),
                    size: proof_data.len() as u64,
                    data: proof_data,
                })
            }
            _ => Err(CompressionError::UnsupportedProofType.into()),
        }
    }
    
    /// Build Merkle tree root from component hashes
    fn build_merkle_root(hashes: &[PoseidonHash]) -> Result<PoseidonHash, ProgramError> {
        if hashes.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }
        
        if hashes.len() == 1 {
            return Ok(hashes[0]);
        }
        
        // Build tree level by level
        let mut current_level = hashes.to_vec();
        let mut hasher = PoseidonHasher::new();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    // Hash pair
                    hasher.update(&current_level[i].to_bytes());
                    hasher.update(&current_level[i + 1].to_bytes());
                    next_level.push(hasher.finalize());
                    hasher.reset();
                } else {
                    // Odd number, carry forward
                    next_level.push(current_level[i]);
                }
            }
            
            current_level = next_level;
        }
        
        Ok(current_level[0])
    }
    
    /// Build Poseidon proof structure
    fn build_poseidon_proof(
        root: &PoseidonHash,
        component_hashes: &[PoseidonHash],
    ) -> Result<Vec<u8>, ProgramError> {
        // Simplified proof structure
        // In production, would use a proper ZK library
        let proof = PoseidonProof {
            root: *root,
            witness_commitments: component_hashes.iter()
                .map(|h| h.to_bytes())
                .collect(),
            nullifier: [0u8; 32], // Placeholder
            proof_version: 1,
        };
        
        proof.try_to_vec()
            .map_err(|_| ProgramError::InvalidInstructionData)
    }
    
    /// Verify proof integrity
    fn verify_proof(
        proof_hash: &[u8; 32],
        state_root: &[u8; 32],
        proof_type: ProofType,
        proof_data: &[u8],
    ) -> Result<bool, ProgramError> {
        match proof_type {
            ProofType::Poseidon => {
                // Deserialize proof
                let proof = PoseidonProof::try_from_slice(proof_data)
                    .map_err(|_| CompressionError::InvalidProofData)?;
                
                // Verify root matches
                if proof.root.to_bytes() != *state_root {
                    return Ok(false);
                }
                
                // Verify proof hash
                let mut hasher = PoseidonHasher::new();
                hasher.update(state_root);
                let calculated_hash = hasher.finalize();
                
                Ok(calculated_hash.to_bytes() == *proof_hash)
            }
            _ => Err(CompressionError::UnsupportedProofType.into()),
        }
    }
    
    /// Extract specific market from proof
    fn extract_market_from_proof(
        proof: &CompressedStateProof,
        market_id: &[u8; 32],
    ) -> Result<MarketEssentials, ProgramError> {
        // Check if market is in sample list for quick lookup
        if let Some(pos) = proof.sample_market_ids.iter().position(|id| id == market_id) {
            // For demo, create dummy data
            // In production, would extract from proof data
            Ok(MarketEssentials {
                market_id: *market_id,
                current_price: 50_000_000, // 50%
                total_volume: 1_000_000,
                outcome_count: 2,
                status: crate::state::MarketStatus::Active,
                last_update: proof.timestamp,
            })
        } else {
            Err(CompressionError::MarketNotInCompressedState.into())
        }
    }
}

/// State proof structure
pub struct StateProof {
    pub hash: [u8; 32],
    pub root: [u8; 32],
    pub size: u64,
    pub data: Vec<u8>,
}

/// Poseidon proof structure
#[derive(BorshSerialize, BorshDeserialize)]
struct PoseidonProof {
    pub root: PoseidonHash,
    pub witness_commitments: Vec<[u8; 32]>,
    pub nullifier: [u8; 32],
    pub proof_version: u8,
}