use solana_program::{
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::CompressionError,
    state::{MarketEssentials, ProofType},
    compression::poseidon::{PoseidonHasher, PoseidonHash, PoseidonMerkleTree},
};

/// Builder for creating compressed state proofs
pub struct ProofBuilder {
    markets: Vec<MarketEssentials>,
    proof_type: ProofType,
    merkle_tree: PoseidonMerkleTree,
}

impl ProofBuilder {
    /// Create new proof builder
    pub fn new(proof_type: ProofType) -> Self {
        Self {
            markets: Vec::new(),
            proof_type,
            merkle_tree: PoseidonMerkleTree::new(),
        }
    }
    
    /// Add market to proof
    pub fn add_market(&mut self, market: MarketEssentials) -> Result<(), ProgramError> {
        // Validate market
        market.validate()?;
        
        // Hash market data
        let mut hasher = PoseidonHasher::new();
        hasher.update(&market.to_bytes());
        let market_hash = hasher.finalize();
        
        // Add to merkle tree
        self.merkle_tree.add_leaf(market_hash);
        
        // Store market
        self.markets.push(market);
        
        Ok(())
    }
    
    /// Build final proof
    pub fn build(mut self) -> Result<BuiltProof, ProgramError> {
        if self.markets.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }
        
        msg!("Building proof for {} markets", self.markets.len());
        
        // Build merkle tree
        let root = self.merkle_tree.build()?;
        
        // Calculate sizes
        let uncompressed_size = self.markets.len() * MarketEssentials::SIZE;
        
        // Create proof data based on type
        let proof_data = match self.proof_type {
            ProofType::Poseidon => {
                self.build_poseidon_proof(&root)?
            }
            _ => return Err(CompressionError::UnsupportedProofType.into()),
        };
        
        let compressed_size = proof_data.len();
        
        // Calculate compression ratio
        let ratio = uncompressed_size as f64 / compressed_size as f64;
        msg!("Compression ratio: {:.2}x", ratio);
        
        Ok(BuiltProof {
            root,
            proof_data,
            markets: self.markets,
            uncompressed_size,
            compressed_size,
            proof_type: self.proof_type,
        })
    }
    
    /// Build Poseidon-based proof
    fn build_poseidon_proof(&self, root: &PoseidonHash) -> Result<Vec<u8>, ProgramError> {
        let proof = CompactPoseidonProof {
            version: 1,
            root: *root,
            market_count: self.markets.len() as u32,
            market_hashes: self.merkle_tree.leaves.clone(),
            // Additional proof data would go here
        };
        
        proof.try_to_vec()
            .map_err(|_| ProgramError::InvalidInstructionData)
    }
    
    /// Get proof for specific market
    pub fn get_market_proof(&self, market_index: usize) -> Result<MarketProof, ProgramError> {
        if market_index >= self.markets.len() {
            return Err(ProgramError::InvalidArgument);
        }
        
        let merkle_proof = self.merkle_tree.get_proof(market_index)?;
        
        Ok(MarketProof {
            market: self.markets[market_index].clone(),
            merkle_path: merkle_proof,
            position: market_index as u32,
        })
    }
}

/// Built proof ready for storage
pub struct BuiltProof {
    pub root: PoseidonHash,
    pub proof_data: Vec<u8>,
    pub markets: Vec<MarketEssentials>,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub proof_type: ProofType,
}

/// Compact Poseidon proof format
#[derive(BorshSerialize, BorshDeserialize)]
struct CompactPoseidonProof {
    pub version: u8,
    pub root: PoseidonHash,
    pub market_count: u32,
    pub market_hashes: Vec<PoseidonHash>,
}

/// Proof for individual market
pub struct MarketProof {
    pub market: MarketEssentials,
    pub merkle_path: Vec<PoseidonHash>,
    pub position: u32,
}

impl MarketProof {
    /// Verify this market proof against a root
    pub fn verify(&self, root: &PoseidonHash) -> bool {
        let mut hasher = PoseidonHasher::new();
        hasher.update(&self.market.to_bytes());
        let market_hash = hasher.finalize();
        
        PoseidonMerkleTree::verify_proof(
            &market_hash,
            &self.merkle_path,
            root,
            self.position as usize,
        )
    }
}

/// Batch proof builder for efficient compression
pub struct BatchProofBuilder {
    batches: Vec<ProofBuilder>,
    current_batch: ProofBuilder,
    batch_size: usize,
}

impl BatchProofBuilder {
    /// Create new batch builder
    pub fn new(batch_size: usize, proof_type: ProofType) -> Self {
        Self {
            batches: Vec::new(),
            current_batch: ProofBuilder::new(proof_type),
            batch_size,
        }
    }
    
    /// Add market to batch
    pub fn add_market(&mut self, market: MarketEssentials) -> Result<(), ProgramError> {
        self.current_batch.add_market(market)?;
        
        // Check if batch is full
        if self.current_batch.markets.len() >= self.batch_size {
            let proof_type = self.current_batch.proof_type;
            let full_batch = std::mem::replace(
                &mut self.current_batch,
                ProofBuilder::new(proof_type),
            );
            self.batches.push(full_batch);
        }
        
        Ok(())
    }
    
    /// Build all batches
    pub fn build_all(mut self) -> Result<Vec<BuiltProof>, ProgramError> {
        // Add final batch if not empty
        if !self.current_batch.markets.is_empty() {
            self.batches.push(self.current_batch);
        }
        
        let mut built_proofs = Vec::new();
        
        for (i, batch) in self.batches.into_iter().enumerate() {
            msg!("Building batch {} with {} markets", i, batch.markets.len());
            built_proofs.push(batch.build()?);
        }
        
        Ok(built_proofs)
    }
}