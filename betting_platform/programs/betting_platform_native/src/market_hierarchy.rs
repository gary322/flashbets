use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    keccak,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::{
    error::BettingPlatformError,
    merkle::{MerkleTree, MerkleProof, VerseChild},
    synthetics::MarketData,
};
use std::collections::{HashMap, HashSet};

/// Market hierarchy constants for Part 7
pub const MAX_MARKETS: usize = 21_300;
pub const TARGET_VERSES: usize = 400;
pub const AVG_CHILDREN_PER_VERSE: usize = 50;
pub const MAX_TREE_DEPTH: usize = 6; // log2(50) ~= 6

/// Verse structure containing child markets
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct Verse {
    pub verse_id: u128,
    pub title: String,
    pub category: String,
    pub children: Vec<ChildMarket>,
    pub merkle_root: [u8; 32],
    pub total_volume: u64,
    pub average_probability: u64,
    pub created_at: i64,
    pub last_updated: i64,
}

/// Child market within a verse
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ChildMarket {
    pub market_id: Pubkey,
    pub title: String,
    pub probability: u64,
    pub volume: u64,
    pub liquidity: u64,
    pub correlation_factor: u64,
}

/// Market hierarchy manager
pub struct MarketHierarchy {
    pub verses: HashMap<u128, Verse>,
    pub market_to_verse: HashMap<Pubkey, u128>,
    pub total_markets: usize,
}

impl MarketHierarchy {
    /// Create new market hierarchy
    pub fn new() -> Self {
        Self {
            verses: HashMap::new(),
            market_to_verse: HashMap::new(),
            total_markets: 0,
        }
    }

    /// Add market to appropriate verse
    pub fn add_market(
        &mut self,
        market: &MarketData,
        verse_id: u128,
    ) -> Result<(), ProgramError> {
        // Check capacity
        if self.total_markets >= MAX_MARKETS {
            return Err(BettingPlatformError::MarketCapacityExceeded.into());
        }

        // Create or update verse
        let verse = self.verses.entry(verse_id).or_insert_with(|| {
            Verse {
                verse_id,
                title: Self::generate_verse_title(&market.category),
                category: market.category.clone(),
                children: Vec::new(),
                merkle_root: [0u8; 32],
                total_volume: 0,
                average_probability: 0,
                created_at: market.created_at,
                last_updated: market.created_at,
            }
        });

        // Check verse capacity
        if verse.children.len() >= AVG_CHILDREN_PER_VERSE * 2 {
            return Err(BettingPlatformError::VerseCapacityExceeded.into());
        }

        // Create child market
        let child = ChildMarket {
            market_id: market.market_id,
            title: market.title.clone(),
            probability: market.yes_price,
            volume: market.volume_24h,
            liquidity: market.liquidity,
            correlation_factor: Self::calculate_correlation(&market.title, &verse.title),
        };

        // Add to verse
        verse.children.push(child);
        verse.total_volume += market.volume_24h;
        verse.last_updated = market.created_at;

        // Update average probability
        let total_prob: u64 = verse.children.iter().map(|c| c.probability).sum();
        verse.average_probability = total_prob / verse.children.len() as u64;

        // Update mappings
        self.market_to_verse.insert(market.market_id, verse_id);
        self.total_markets += 1;

        // Rebuild merkle tree for verse
        self.rebuild_merkle_tree(verse_id)?;

        Ok(())
    }

    /// Rebuild merkle tree for a verse
    fn rebuild_merkle_tree(&mut self, verse_id: u128) -> Result<(), ProgramError> {
        let verse = self.verses.get(&verse_id)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Create leaf nodes from children
        let mut leaves = Vec::new();
        for child in &verse.children {
            let leaf_data = format!("{}{}{}", 
                child.market_id, 
                child.volume, 
                child.probability
            );
            let hash = keccak::hash(leaf_data.as_bytes());
            leaves.push(hash.to_bytes());
        }

        // Build verse children for merkle root computation
        let verse_children: Vec<VerseChild> = self.verses[&verse_id].children
            .iter()
            .map(|child| {
                let hash = keccak::hash(child.market_id.as_ref()).to_bytes();
                VerseChild {
                    child_id: hash,
                    weight: child.volume, // Use volume as weight
                    correlation: child.correlation_factor,
                }
            })
            .collect();

        // Compute merkle root
        let merkle_root = MerkleTree::compute_root(&verse_children)?;

        // Update verse merkle root
        if let Some(verse) = self.verses.get_mut(&verse_id) {
            verse.merkle_root = merkle_root;
        }

        Ok(())
    }

    /// Get merkle proof for a child market
    pub fn get_market_proof(
        &self,
        market_id: &Pubkey,
    ) -> Result<Vec<MerkleProof>, ProgramError> {
        // Find verse containing market
        let verse_id = self.market_to_verse.get(market_id)
            .ok_or(BettingPlatformError::MarketNotFound)?;

        let verse = self.verses.get(verse_id)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Find market index
        let market_index = verse.children.iter()
            .position(|c| c.market_id == *market_id)
            .ok_or(BettingPlatformError::MarketNotFound)?;

        // Generate proof using static method
        let verse_children: Vec<VerseChild> = verse.children
            .iter()
            .map(|child| {
                let hash = keccak::hash(child.market_id.as_ref()).to_bytes();
                VerseChild {
                    child_id: hash,
                    weight: child.volume,
                    correlation: child.correlation_factor,
                }
            })
            .collect();

        // Get target child ID
        let target_child = &verse.children[market_index];
        let target_child_id = keccak::hash(target_child.market_id.as_ref()).to_bytes();

        MerkleTree::generate_proof(&verse_children, &target_child_id)
    }

    /// Verify market belongs to verse using merkle proof
    pub fn verify_market_inclusion(
        &self,
        market_id: &Pubkey,
        verse_id: u128,
        proof: &[MerkleProof],
    ) -> Result<bool, ProgramError> {
        let verse = self.verses.get(&verse_id)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Find market in verse
        let market = verse.children.iter()
            .find(|c| c.market_id == *market_id)
            .ok_or(BettingPlatformError::MarketNotFound)?;

        // Create leaf hash
        let leaf_data = format!("{}{}{}", 
            market.market_id, 
            market.volume, 
            market.probability
        );
        let leaf_hash = keccak::hash(leaf_data.as_bytes()).to_bytes();

        // Create VerseChild for verification
        let leaf_child = VerseChild {
            child_id: leaf_hash,
            weight: market.volume,
            correlation: market.correlation_factor,
        };
        
        MerkleTree::verify_proof(
            &verse.merkle_root,
            &leaf_child,
            proof
        )
    }

    /// Calculate correlation factor between market and verse
    fn calculate_correlation(market_title: &str, verse_title: &str) -> u64 {
        // Simple correlation based on common words
        let market_words: HashSet<_> = market_title.split_whitespace().collect();
        let verse_words: HashSet<_> = verse_title.split_whitespace().collect();
        
        let common = market_words.intersection(&verse_words).count();
        let total = market_words.union(&verse_words).count();
        
        if total > 0 {
            (common as u64 * 10000) / total as u64 // Basis points
        } else {
            0
        }
    }

    /// Generate verse title from category
    fn generate_verse_title(category: &str) -> String {
        format!("{} Verse", category)
    }

    /// Get verse statistics
    pub fn get_verse_stats(&self, verse_id: u128) -> Result<VerseStats, ProgramError> {
        let verse = self.verses.get(&verse_id)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        Ok(VerseStats {
            verse_id,
            child_count: verse.children.len() as u32,
            total_volume: verse.total_volume,
            average_probability: verse.average_probability,
            average_liquidity: verse.children.iter()
                .map(|c| c.liquidity)
                .sum::<u64>() / verse.children.len().max(1) as u64,
            merkle_root: verse.merkle_root,
        })
    }

    /// Find markets by O(log n) merkle path
    pub fn find_markets_by_path(
        &self,
        verse_id: u128,
        path_indices: &[usize],
    ) -> Result<Vec<Pubkey>, ProgramError> {
        let verse = self.verses.get(&verse_id)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Navigate tree using path indices
        let mut markets = Vec::new();
        let mut current_index = 0;

        for &direction in path_indices {
            current_index = current_index * 2 + direction;
            if current_index < verse.children.len() {
                markets.push(verse.children[current_index].market_id);
            }
        }

        Ok(markets)
    }

    /// Batch lookup optimization
    pub fn batch_lookup_markets(
        &self,
        market_ids: &[Pubkey],
    ) -> HashMap<Pubkey, (u128, Vec<MerkleProof>)> {
        let mut results = HashMap::new();

        for market_id in market_ids {
            if let Ok(proofs) = self.get_market_proof(market_id) {
                if let Some(&verse_id) = self.market_to_verse.get(market_id) {
                    results.insert(*market_id, (verse_id, proofs));
                }
            }
        }

        results
    }
}

/// Verse statistics
#[derive(Debug)]
pub struct VerseStats {
    pub verse_id: u128,
    pub child_count: u32,
    pub total_volume: u64,
    pub average_probability: u64,
    pub average_liquidity: u64,
    pub merkle_root: [u8; 32],
}

/// Initialize market hierarchy
pub fn initialize_market_hierarchy(
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing market hierarchy for {} markets", MAX_MARKETS);
    msg!("Target verses: {}, Avg children per verse: {}", TARGET_VERSES, AVG_CHILDREN_PER_VERSE);
    msg!("Merkle tree depth: {} (supports {} children)", MAX_TREE_DEPTH, 2_usize.pow(MAX_TREE_DEPTH as u32));
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_hierarchy() {
        let mut hierarchy = MarketHierarchy::new();
        
        // Create test market
        let market = MarketData {
            market_id: Pubkey::new_unique(),
            probability: crate::math::U64F64::from_num(600_000), // 60%
            volume_7d: 7_000_000, // Weekly volume
            liquidity_depth: 500_000,
            last_trade_time: 0,
            category: "Crypto".to_string(),
            title: "BTC above $100k by 2025".to_string(),
            yes_price: 6000,
            volume_24h: 1_000_000,
            liquidity: 500_000,
            created_at: 0,
        };

        // Add to verse
        let verse_id = 12345u128;
        hierarchy.add_market(&market, verse_id).unwrap();

        // Verify
        assert_eq!(hierarchy.total_markets, 1);
        assert!(hierarchy.verses.contains_key(&verse_id));
        assert_eq!(hierarchy.market_to_verse[&market.market_id], verse_id);
    }

    #[test]
    fn test_merkle_proof_generation() {
        let mut hierarchy = MarketHierarchy::new();
        let verse_id = 12345u128;

        // Add multiple markets
        for i in 0..10 {
            let market = MarketData {
                market_id: Pubkey::new_unique(),
                probability: crate::math::U64F64::from_num(500_000), // 50%
                volume_7d: 3_500_000,
                liquidity_depth: 250_000,
                last_trade_time: 0,
                category: "Test".to_string(),
                title: format!("Market {}", i),
                yes_price: 5000,
                volume_24h: 100_000 * (i + 1) as u64,
                liquidity: 50_000,
                created_at: 0,
            };
            
            hierarchy.add_market(&market, verse_id).unwrap();
        }

        // Get proof for first market
        let first_market = &hierarchy.verses[&verse_id].children[0].market_id;
        let proof = hierarchy.get_market_proof(first_market).unwrap();
        
        // Verify proof
        assert!(!proof.is_empty());
        let verified = hierarchy.verify_market_inclusion(first_market, verse_id, &proof).unwrap();
        assert!(verified);
    }
}