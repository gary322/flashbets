use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::{hash, hashv, Hash};
use crate::account_structs::{U64F64, U128F128};
use crate::errors::ErrorCode;

pub const MAX_CHILDREN: usize = 64;  // Per verse
pub const MERKLE_DEPTH: usize = 6;   // log2(64) for efficient proofs

pub struct MerkleTree {
    pub nodes: Vec<[u8; 32]>,
    pub leaf_count: usize,
}

impl MerkleTree {
    // CLAUDE.md: "Merkle root for children = log(n) lookups"
    pub fn compute_root(children: &[VerseChild]) -> [u8; 32] {
        if children.is_empty() {
            return [0u8; 32];
        }

        // Sort children by ID for deterministic ordering
        let mut sorted_children = children.to_vec();
        sorted_children.sort_by_key(|c| c.child_id);

        // Compute leaf hashes
        let mut current_level: Vec<[u8; 32]> = sorted_children
            .iter()
            .map(|child| {
                let mut data = Vec::new();
                data.extend_from_slice(&child.child_id);
                data.extend_from_slice(&child.weight.to_le_bytes());
                data.extend_from_slice(&child.correlation.to_le_bytes());
                hash(&data).to_bytes()
            })
            .collect();

        // Build tree bottom-up
        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    let mut data = [0u8; 64];
                    data[..32].copy_from_slice(&chunk[0]);
                    data[32..].copy_from_slice(&chunk[1]);
                    hash(&data).to_bytes()
                } else {
                    chunk[0] // Odd number, promote directly
                };
                next_level.push(hash);
            }

            current_level = next_level;
        }

        current_level[0]
    }

    // Verify merkle proof for child inclusion
    pub fn verify_proof(
        root: &[u8; 32],
        leaf: &VerseChild,
        proof: &[MerkleProof],
    ) -> Result<bool> {
        let mut current_hash = Self::hash_leaf(leaf);

        for proof_element in proof {
            current_hash = if proof_element.is_left {
                let mut data = [0u8; 64];
                data[..32].copy_from_slice(&proof_element.hash);
                data[32..].copy_from_slice(&current_hash);
                hash(&data).to_bytes()
            } else {
                let mut data = [0u8; 64];
                data[..32].copy_from_slice(&current_hash);
                data[32..].copy_from_slice(&proof_element.hash);
                hash(&data).to_bytes()
            };
        }

        Ok(current_hash == *root)
    }

    fn hash_leaf(child: &VerseChild) -> [u8; 32] {
        let mut data = Vec::new();
        data.extend_from_slice(&child.child_id);
        data.extend_from_slice(&child.weight.to_le_bytes());
        data.extend_from_slice(&child.correlation.to_le_bytes());
        hash(&data).to_bytes()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct VerseChild {
    pub child_id: [u8; 32],
    pub weight: u64,        // Volume-based weight
    pub correlation: u64,   // Correlation factor for risk
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MerkleProof {
    pub hash: [u8; 32],
    pub is_left: bool,
}

// Detailed Merkle Root Update Algorithm Implementation
pub struct VerseHierarchyTree {
    pub root: [u8; 32],              // Merkle root hash
    pub nodes: Vec<MerkleNode>,      // All intermediate nodes
    pub leaf_count: u32,             // Number of child verses
    pub max_depth: u8,               // Maximum tree depth (32)
    pub update_slot: u64,            // Last update slot
}

pub struct MerkleNode {
    pub hash: [u8; 32],              // keccak256 hash
    pub left_child: Option<Pubkey>,  // Left child reference
    pub right_child: Option<Pubkey>, // Right child reference
    pub verse_id: Option<u128>,      // Leaf verse ID (if leaf node)
    pub depth: u8,                   // Node depth in tree
}

impl VerseHierarchyTree {
    pub fn update_merkle_root(&mut self, changed_verse_id: u128, new_data: VerseData) -> Result<()> {
        // Step 1: Find leaf position (O(log n))
        let leaf_index = self.find_leaf_index(changed_verse_id)?;

        // Step 2: Update leaf hash
        let mut data = Vec::new();
        data.extend_from_slice(&changed_verse_id.to_le_bytes());
        data.extend_from_slice(&new_data.probability.to_le_bytes());
        data.extend_from_slice(&new_data.volume.to_le_bytes());
        let new_leaf_hash = keccak::hash(&data).to_bytes();

        // Step 3: Propagate updates up the tree
        let mut current_index = leaf_index;
        let mut current_hash = new_leaf_hash;

        while let Some(parent_index) = self.get_parent_index(current_index) {
            let sibling_hash = self.get_sibling_hash(current_index)?;

            // Calculate new parent hash
            let (left, right) = if current_index % 2 == 0 {
                (current_hash, sibling_hash)
            } else {
                (sibling_hash, current_hash)
            };

            current_hash = keccak::hashv(&[&left, &right]).to_bytes();
            self.nodes[parent_index].hash = current_hash;
            current_index = parent_index;
        }

        self.root = current_hash;
        self.update_slot = Clock::get()?.slot;
        Ok(())
    }

    fn find_leaf_index(&self, verse_id: u128) -> Result<usize> {
        self.nodes
            .iter()
            .position(|node| node.verse_id == Some(verse_id))
            .ok_or(ErrorCode::VerseNotFound.into())
    }

    fn get_parent_index(&self, child_index: usize) -> Option<usize> {
        if child_index == 0 {
            None
        } else {
            Some((child_index - 1) / 2)
        }
    }

    fn get_sibling_hash(&self, index: usize) -> Result<[u8; 32]> {
        let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
        
        if sibling_index < self.nodes.len() {
            Ok(self.nodes[sibling_index].hash)
        } else {
            // No sibling, return zero hash
            Ok([0u8; 32])
        }
    }
}

pub struct VerseData {
    pub probability: u64,
    pub volume: u64,
}

// Use ErrorCode from crate::errors