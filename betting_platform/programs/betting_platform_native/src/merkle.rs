//! Merkle tree operations for state management
//!
//! Implements hierarchical state management using merkle trees for efficient verse organization

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    keccak::hash,
    program_error::ProgramError,
};

use crate::error::BettingPlatformError;

/// Maximum children per verse
pub const MAX_CHILDREN: usize = 64;

/// Maximum merkle tree depth (log2(64))
pub const MERKLE_DEPTH: usize = 6;

/// Merkle tree structure for verse hierarchy
pub struct MerkleTree {
    pub nodes: Vec<[u8; 32]>,
    pub leaf_count: usize,
}

/// Node in the merkle tree
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MerkleNode {
    pub hash: [u8; 32],
    pub left_child: Option<[u8; 32]>,
    pub right_child: Option<[u8; 32]>,
    pub verse_id: Option<u128>,
    pub depth: u8,
}

/// Child verse information for merkle tree
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VerseChild {
    pub child_id: [u8; 32],
    pub weight: u64,        // Volume-based weight
    pub correlation: u64,   // Correlation factor for risk
}

/// Merkle proof element
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MerkleProof {
    pub hash: [u8; 32],
    pub is_left: bool,
}

/// Calculate merkle root from children (alias for compute_root)
pub fn calculate_merkle_root(children: &[VerseChild]) -> Result<[u8; 32], ProgramError> {
    MerkleTree::compute_root(children)
}

impl MerkleTree {
    /// Create a new empty merkle tree
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            leaf_count: 0,
        }
    }

    /// Compute merkle root from children
    pub fn compute_root(children: &[VerseChild]) -> Result<[u8; 32], ProgramError> {
        if children.is_empty() {
            return Ok([0u8; 32]);
        }

        if children.len() > MAX_CHILDREN {
            return Err(BettingPlatformError::ExceedsVerseLimit.into());
        }

        // Sort children by ID for deterministic ordering
        let mut sorted_children = children.to_vec();
        sorted_children.sort_by_key(|c| c.child_id);

        // Compute leaf hashes
        let mut current_level: Vec<[u8; 32]> = sorted_children
            .iter()
            .map(|child| Self::hash_leaf(child))
            .collect();

        // Build tree bottom-up
        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let hash_value = if chunk.len() == 2 {
                    // Hash two nodes together
                    let mut data = Vec::with_capacity(64);
                    // Sort hashes to ensure consistent ordering
                    if chunk[0] < chunk[1] {
                        data.extend_from_slice(&chunk[0]);
                        data.extend_from_slice(&chunk[1]);
                    } else {
                        data.extend_from_slice(&chunk[1]);
                        data.extend_from_slice(&chunk[0]);
                    }
                    hash(&data).to_bytes()
                } else {
                    // Odd number, promote directly
                    chunk[0]
                };
                next_level.push(hash_value);
            }

            current_level = next_level;
        }

        Ok(current_level[0])
    }

    /// Hash a leaf node
    fn hash_leaf(child: &VerseChild) -> [u8; 32] {
        let mut data = Vec::new();
        data.extend_from_slice(&child.child_id);
        data.extend_from_slice(&child.weight.to_le_bytes());
        data.extend_from_slice(&child.correlation.to_le_bytes());
        hash(&data).to_bytes()
    }

    /// Verify merkle proof for child inclusion
    pub fn verify_proof(
        root: &[u8; 32],
        leaf: &VerseChild,
        proof: &[MerkleProof],
    ) -> Result<bool, ProgramError> {
        if proof.len() > MERKLE_DEPTH {
            return Err(BettingPlatformError::InvalidProof.into());
        }

        let mut current_hash = Self::hash_leaf(leaf);

        for proof_element in proof {
            let mut data = Vec::with_capacity(64);
            
            if proof_element.is_left {
                // Proof element is on the left
                if proof_element.hash < current_hash {
                    data.extend_from_slice(&proof_element.hash);
                    data.extend_from_slice(&current_hash);
                } else {
                    data.extend_from_slice(&current_hash);
                    data.extend_from_slice(&proof_element.hash);
                }
            } else {
                // Proof element is on the right
                if current_hash < proof_element.hash {
                    data.extend_from_slice(&current_hash);
                    data.extend_from_slice(&proof_element.hash);
                } else {
                    data.extend_from_slice(&proof_element.hash);
                    data.extend_from_slice(&current_hash);
                }
            }
            
            current_hash = hash(&data).to_bytes();
        }

        Ok(&current_hash == root)
    }

    /// Generate merkle proof for a specific child
    pub fn generate_proof(
        children: &[VerseChild],
        target_child_id: &[u8; 32],
    ) -> Result<Vec<MerkleProof>, ProgramError> {
        if children.is_empty() {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        // Sort children by ID for deterministic ordering
        let mut sorted_children = children.to_vec();
        sorted_children.sort_by_key(|c| c.child_id);

        // Find target index
        let target_index = sorted_children
            .iter()
            .position(|c| &c.child_id == target_child_id)
            .ok_or(BettingPlatformError::InvalidInput)?;

        let mut proof = Vec::new();
        let mut level_hashes: Vec<[u8; 32]> = sorted_children
            .iter()
            .map(|child| Self::hash_leaf(child))
            .collect();

        let mut current_index = target_index;

        while level_hashes.len() > 1 {
            let mut next_level = Vec::new();
            let mut next_index = None;

            for (i, chunk) in level_hashes.chunks(2).enumerate() {
                if chunk.len() == 2 {
                    // Two nodes to hash
                    if i * 2 == current_index || i * 2 + 1 == current_index {
                        // This chunk contains our target
                        let is_left = i * 2 == current_index;
                        proof.push(MerkleProof {
                            hash: if is_left { chunk[1] } else { chunk[0] },
                            is_left: !is_left,
                        });
                        next_index = Some(i);
                    }

                    let mut data = Vec::with_capacity(64);
                    if chunk[0] < chunk[1] {
                        data.extend_from_slice(&chunk[0]);
                        data.extend_from_slice(&chunk[1]);
                    } else {
                        data.extend_from_slice(&chunk[1]);
                        data.extend_from_slice(&chunk[0]);
                    }
                    next_level.push(hash(&data).to_bytes());
                } else {
                    // Single node, promote directly
                    if i * 2 == current_index {
                        next_index = Some(i);
                    }
                    next_level.push(chunk[0]);
                }
            }

            level_hashes = next_level;
            current_index = next_index.ok_or(BettingPlatformError::InvalidInput)?;
        }

        Ok(proof)
    }
}