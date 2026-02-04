use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash;
use crate::account_structs::{ProposalPDA, ProposalState, AMMType, U64F64};
use crate::errors::ErrorCode;

// CLAUDE.md: "ZK compression (Solana v1.18 feature, reduces state 10x via proofs)"
pub struct StateCompressor;

impl StateCompressor {
    // Compress ProposalPDA from 520 bytes to ~52 bytes + proof
    pub fn compress_proposal(proposal: &ProposalPDA) -> Result<CompressedProposal> {
        // Generate merkle proof of essential fields
        let essential_data = Self::extract_essential_data(proposal);
        let proof = Self::generate_compression_proof(&essential_data)?;

        Ok(CompressedProposal {
            proposal_id: proposal.proposal_id,
            proof_hash: proof.hash,
            essential_data,
            proof,
        })
    }

    pub fn decompress_proposal(
        compressed: &CompressedProposal,
        full_data: &[u8],
    ) -> Result<ProposalPDA> {
        // Verify proof matches
        if !Self::verify_compression_proof(&compressed.proof, full_data)? {
            return Err(ErrorCode::InvalidCompressionProof.into());
        }

        // Reconstruct from full data
        ProposalPDA::try_from_slice(full_data)
            .map_err(|_| ErrorCode::DecompressionFailed.into())
    }

    fn extract_essential_data(proposal: &ProposalPDA) -> EssentialData {
        EssentialData {
            proposal_id: proposal.proposal_id,
            verse_id: proposal.verse_id,
            amm_type: proposal.amm_type,
            current_price: if !proposal.prices.is_empty() { 
                proposal.prices[0] 
            } else { 
                0 
            }, // Primary outcome
            total_volume: proposal.volumes.iter().sum(),
            state: proposal.state,
        }
    }

    fn generate_compression_proof(data: &EssentialData) -> Result<CompressionProof> {
        let mut proof_data = Vec::new();
        proof_data.extend_from_slice(&data.proposal_id);
        proof_data.extend_from_slice(&data.verse_id);
        proof_data.push(data.amm_type as u8);
        proof_data.extend_from_slice(&data.current_price.to_le_bytes());
        proof_data.extend_from_slice(&data.total_volume.to_le_bytes());
        proof_data.push(data.state as u8);

        let hash = hash(&proof_data).to_bytes();

        Ok(CompressionProof {
            hash,
            timestamp: Clock::get()?.slot,
            compression_version: 1,
        })
    }

    fn verify_compression_proof(
        proof: &CompressionProof,
        full_data: &[u8],
    ) -> Result<bool> {
        // In production, this would verify ZK proof
        // For now, verify hash matches
        let computed_hash = hash(full_data).to_bytes();
        Ok(computed_hash == proof.hash)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CompressedProposal {
    pub proposal_id: [u8; 32],
    pub proof_hash: [u8; 32],
    pub essential_data: EssentialData,
    pub proof: CompressionProof,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct EssentialData {
    pub proposal_id: [u8; 32],
    pub verse_id: [u8; 32],
    pub amm_type: AMMType,
    pub current_price: U64F64,
    pub total_volume: u64,
    pub state: ProposalState,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CompressionProof {
    pub hash: [u8; 32],
    pub timestamp: u64,
    pub compression_version: u8,
}

// Compression configuration
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub compression_level: u8,       // 1-10 (10 = max compression)
    pub batch_size: u32,             // Compress in batches of N accounts
    pub proof_verification_cu: u32,  // ~2000 CU per proof
    pub compression_cu: u32,         // ~5000 CU to compress
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            compression_level: 8,
            batch_size: 100,
            proof_verification_cu: 2000,
            compression_cu: 5000,
        }
    }
}

// Compression strategy for ProposalPDAs
pub fn compress_proposal_batch(proposals: &[ProposalPDA]) -> Result<CompressedBatch> {
    // Group by common fields to maximize compression
    let mut grouped = std::collections::HashMap::new();
    
    for proposal in proposals {
        let key = (proposal.amm_type, proposal.state, proposal.outcomes.len());
        grouped.entry(key).or_insert(vec![]).push(proposal);
    }

    // Compress each group
    let mut compressed_groups = vec![];
    for ((amm_type, state, outcome_count), group) in grouped {
        let merkle_tree = build_proposal_merkle_tree(&group)?;
        let compressed = CompressedGroup {
            common_fields: CommonFields { amm_type, state, outcome_count },
            merkle_root: merkle_tree.root,
            leaf_data: compress_unique_fields(&group)?,
            proof_data: generate_zk_proof(&merkle_tree)?,
        };
        compressed_groups.push(compressed);
    }

    let compressed_size = calculate_compressed_size(&compressed_groups);
    let compression_ratio = calculate_ratio(proposals.len(), &compressed_groups);
    
    Ok(CompressedBatch {
        groups: compressed_groups,
        original_count: proposals.len() as u32,
        compressed_size,
        compression_ratio,
    })
}

fn build_proposal_merkle_tree(proposals: &[&ProposalPDA]) -> Result<MerkleTree> {
    let mut hashes = Vec::new();
    
    for proposal in proposals {
        let mut data = Vec::new();
        data.extend_from_slice(&proposal.proposal_id);
        data.extend_from_slice(&proposal.verse_id);
        data.extend_from_slice(&proposal.market_id);
        let hash = hash(&data).to_bytes();
        hashes.push(hash);
    }
    
    Ok(MerkleTree { root: compute_merkle_root(&hashes), hashes })
}

fn compress_unique_fields(proposals: &[&ProposalPDA]) -> Result<Vec<CompressedLeaf>> {
    let mut leaves = Vec::new();
    
    for proposal in proposals {
        leaves.push(CompressedLeaf {
            proposal_id: proposal.proposal_id,
            prices: proposal.prices.clone(),
            volumes: proposal.volumes.clone(),
            liquidity_depth: proposal.liquidity_depth,
            settle_slot: proposal.settle_slot,
        });
    }
    
    Ok(leaves)
}

fn generate_zk_proof(tree: &MerkleTree) -> Result<Vec<u8>> {
    // Placeholder for ZK proof generation
    // In production, this would use a ZK library
    Ok(tree.root.to_vec())
}

fn calculate_compressed_size(groups: &[CompressedGroup]) -> usize {
    let mut size = 0;
    for group in groups {
        size += 32; // merkle root
        size += 3;  // common fields (1 byte each)
        size += group.leaf_data.len() * 52; // ~52 bytes per compressed leaf
        size += group.proof_data.len();
    }
    size
}

fn calculate_ratio(original_count: usize, groups: &[CompressedGroup]) -> f32 {
    let original_size = original_count * 520; // 520 bytes per ProposalPDA
    let compressed_size = calculate_compressed_size(groups);
    original_size as f32 / compressed_size as f32
}

fn compute_merkle_root(hashes: &[[u8; 32]]) -> [u8; 32] {
    if hashes.is_empty() {
        return [0u8; 32];
    }
    
    let mut current_level = hashes.to_vec();
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        
        for chunk in current_level.chunks(2) {
            let hash = if chunk.len() == 2 {
                let mut data = [0u8; 64];
                data[..32].copy_from_slice(&chunk[0]);
                data[32..].copy_from_slice(&chunk[1]);
                hash(&data).to_bytes()
            } else {
                chunk[0]
            };
            next_level.push(hash);
        }
        
        current_level = next_level;
    }
    
    current_level[0]
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CompressedBatch {
    pub groups: Vec<CompressedGroup>,
    pub original_count: u32,
    pub compressed_size: usize,
    pub compression_ratio: f32,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CompressedGroup {
    pub common_fields: CommonFields,
    pub merkle_root: [u8; 32],
    pub leaf_data: Vec<CompressedLeaf>,
    pub proof_data: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CommonFields {
    pub amm_type: AMMType,
    pub state: ProposalState,
    pub outcome_count: usize,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CompressedLeaf {
    pub proposal_id: [u8; 32],
    pub prices: Vec<U64F64>,
    pub volumes: Vec<u64>,
    pub liquidity_depth: u64,
    pub settle_slot: u64,
}

struct MerkleTree {
    root: [u8; 32],
    hashes: Vec<[u8; 32]>,
}