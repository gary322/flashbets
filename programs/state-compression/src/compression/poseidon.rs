use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

/// Poseidon hash output (simplified for native Solana)
#[derive(Debug, Clone, Copy, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct PoseidonHash {
    pub bytes: [u8; 32],
}

impl PoseidonHash {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }
    
    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }
    
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }
}

/// Simplified Poseidon hasher for ZK-friendly hashing
/// In production, would use a proper Poseidon implementation
pub struct PoseidonHasher {
    state: Vec<u8>,
}

impl PoseidonHasher {
    /// Create new hasher
    pub fn new() -> Self {
        Self {
            state: Vec::new(),
        }
    }
    
    /// Update hasher with data
    pub fn update(&mut self, data: &[u8]) {
        self.state.extend_from_slice(data);
    }
    
    /// Finalize and get hash
    pub fn finalize(&self) -> PoseidonHash {
        // Simplified hash using Solana's keccak for demo
        // In production, would use actual Poseidon algorithm
        let hash = solana_program::keccak::hash(&self.state);
        PoseidonHash::new(hash.to_bytes())
    }
    
    /// Reset hasher state
    pub fn reset(&mut self) {
        self.state.clear();
    }
    
    /// Hash two values together (for Merkle tree)
    pub fn hash_pair(left: &PoseidonHash, right: &PoseidonHash) -> PoseidonHash {
        let mut hasher = Self::new();
        hasher.update(&left.to_bytes());
        hasher.update(&right.to_bytes());
        hasher.finalize()
    }
}

/// Poseidon parameters for BN254 curve
pub struct PoseidonParams {
    pub t: usize,           // Width (number of inputs + 1)
    pub rounds_f: usize,    // Full rounds
    pub rounds_p: usize,    // Partial rounds
}

impl Default for PoseidonParams {
    fn default() -> Self {
        Self {
            t: 3,           // Binary tree (2 inputs + 1 state)
            rounds_f: 8,    // 8 full rounds
            rounds_p: 57,   // 57 partial rounds
        }
    }
}

/// Poseidon-based Merkle tree for efficient state compression
pub struct PoseidonMerkleTree {
    pub leaves: Vec<PoseidonHash>,
    pub root: Option<PoseidonHash>,
}

impl PoseidonMerkleTree {
    /// Create new Merkle tree
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            root: None,
        }
    }
    
    /// Add leaf to tree
    pub fn add_leaf(&mut self, leaf: PoseidonHash) {
        self.leaves.push(leaf);
        self.root = None; // Invalidate root
    }
    
    /// Build tree and calculate root
    pub fn build(&mut self) -> Result<PoseidonHash, ProgramError> {
        if self.leaves.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }
        
        // If already built, return cached root
        if let Some(root) = self.root {
            return Ok(root);
        }
        
        // Build tree level by level
        let mut current_level = self.leaves.clone();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    // Hash pair
                    let hash = PoseidonHasher::hash_pair(
                        &current_level[i],
                        &current_level[i + 1],
                    );
                    next_level.push(hash);
                } else {
                    // Odd number, carry forward
                    next_level.push(current_level[i]);
                }
            }
            
            current_level = next_level;
        }
        
        self.root = Some(current_level[0]);
        Ok(current_level[0])
    }
    
    /// Get Merkle proof for a leaf
    pub fn get_proof(&self, leaf_index: usize) -> Result<Vec<PoseidonHash>, ProgramError> {
        if leaf_index >= self.leaves.len() {
            return Err(ProgramError::InvalidArgument);
        }
        
        let mut proof = Vec::new();
        let mut current_index = leaf_index;
        let mut current_level = self.leaves.clone();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    // Add sibling to proof if needed
                    if i == current_index || i + 1 == current_index {
                        let sibling_index = if i == current_index { i + 1 } else { i };
                        proof.push(current_level[sibling_index]);
                    }
                    
                    // Hash pair for next level
                    let hash = PoseidonHasher::hash_pair(
                        &current_level[i],
                        &current_level[i + 1],
                    );
                    next_level.push(hash);
                }
            }
            
            // Update index for next level
            current_index /= 2;
            current_level = next_level;
        }
        
        Ok(proof)
    }
    
    /// Verify Merkle proof
    pub fn verify_proof(
        leaf: &PoseidonHash,
        proof: &[PoseidonHash],
        root: &PoseidonHash,
        leaf_index: usize,
    ) -> bool {
        let mut current_hash = *leaf;
        let mut current_index = leaf_index;
        
        for sibling in proof {
            if current_index % 2 == 0 {
                // Current node is left child
                current_hash = PoseidonHasher::hash_pair(&current_hash, sibling);
            } else {
                // Current node is right child
                current_hash = PoseidonHasher::hash_pair(sibling, &current_hash);
            }
            current_index /= 2;
        }
        
        current_hash == *root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_poseidon_hasher() {
        let mut hasher = PoseidonHasher::new();
        hasher.update(b"test data");
        let hash1 = hasher.finalize();
        
        hasher.reset();
        hasher.update(b"test data");
        let hash2 = hasher.finalize();
        
        assert_eq!(hash1, hash2);
    }
    
    #[test]
    fn test_merkle_tree() {
        let mut tree = PoseidonMerkleTree::new();
        
        // Add leaves
        for i in 0..8 {
            let mut hasher = PoseidonHasher::new();
            hasher.update(&i.to_le_bytes());
            tree.add_leaf(hasher.finalize());
        }
        
        // Build tree
        let root = tree.build().unwrap();
        
        // Get and verify proof
        let proof = tree.get_proof(3).unwrap();
        assert!(PoseidonMerkleTree::verify_proof(
            &tree.leaves[3],
            &proof,
            &root,
            3
        ));
    }
}