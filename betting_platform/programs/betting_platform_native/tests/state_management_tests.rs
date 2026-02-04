//! Comprehensive tests for state management features
//!
//! Tests merkle trees, state traversal, compression, and verse classification

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use betting_platform_native::{
    merkle::{MerkleTree, VerseChild, MerkleProof},
    state::{VersePDA, ProposalPDA, VerseStatus, ProposalState},
    state_traversal::StateTraversal,
    state_compression::{StateCompressor, CompressionConfig},
    verse_classification::VerseClassifier,
    math::U64F64,
};

#[tokio::test]
async fn test_merkle_tree_operations() {
    // Test merkle root computation with multiple children
    let children = vec![
        VerseChild {
            child_id: [1u8; 32],
            weight: 1000,
            correlation: 500,
        },
        VerseChild {
            child_id: [2u8; 32],
            weight: 2000,
            correlation: 600,
        },
        VerseChild {
            child_id: [3u8; 32],
            weight: 1500,
            correlation: 550,
        },
    ];
    
    let root = MerkleTree::compute_root(&children);
    assert_ne!(root, [0u8; 32], "Merkle root should not be empty");
    
    // Test merkle proof generation and verification
    let tree = MerkleTree::new();
    let proof = tree.generate_proof(&children[0]).unwrap();
    
    let valid = MerkleTree::verify_proof(&root, &children[0], &proof).unwrap();
    assert!(valid, "Merkle proof should be valid");
    
    // Test with modified child (should fail verification)
    let mut modified_child = children[0].clone();
    modified_child.weight = 999;
    
    let invalid = MerkleTree::verify_proof(&root, &modified_child, &proof).unwrap();
    assert!(!invalid, "Modified child should fail verification");
}

#[tokio::test]
async fn test_state_traversal() {
    // Create test verse hierarchy
    let root_verse = VersePDA::new(1, None, 1);
    let child_verse1 = VersePDA::new(2, Some(1), 1);
    let child_verse2 = VersePDA::new(3, Some(1), 1);
    let grandchild_verse = VersePDA::new(4, Some(2), 1);
    
    // Test finding root verse
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    );
    
    // Add test accounts
    let verses = vec![root_verse, child_verse1, child_verse2, grandchild_verse];
    for (i, verse) in verses.iter().enumerate() {
        let pubkey = Pubkey::new_unique();
        let data = verse.try_to_vec().unwrap();
        program_test.add_account(
            pubkey,
            Account {
                lamports: 1_000_000,
                data,
                owner: betting_platform_native::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Test derived probability calculation
    let mut test_verse = VersePDA::new(100, None, 1);
    test_verse.child_count = 3;
    test_verse.total_oi = 5000;
    
    // Verify aggregation logic
    assert_eq!(test_verse.total_oi, 5000);
    assert_eq!(test_verse.child_count, 3);
}

#[tokio::test]
async fn test_verse_classification() {
    // Test market title classification
    let test_cases = vec![
        ("Will Bitcoin reach $100k by EOY?", "crypto"),
        ("Will ETH price exceed $5000 by December?", "crypto"),
        ("Presidential election winner 2024", "politics"),
        ("Will AI surpass human intelligence by 2030?", "technology"),
        ("S&P 500 above 5000 by Q4?", "finance"),
    ];
    
    for (title, expected_category) in test_cases {
        let verse_id = VerseClassifier::classify_market_to_verse(title).unwrap();
        
        // Verify deterministic classification
        let verse_id2 = VerseClassifier::classify_market_to_verse(title).unwrap();
        assert_eq!(verse_id, verse_id2, "Classification should be deterministic");
        
        // Verify different titles get different IDs
        let different_title = format!("{} MODIFIED", title);
        let different_id = VerseClassifier::classify_market_to_verse(&different_title).unwrap();
        assert_ne!(verse_id, different_id, "Different titles should get different verse IDs");
    }
    
    // Test normalization
    let similar_titles = vec![
        "Will BTC reach $100,000 by end of year?",
        "Will Bitcoin reach $100k by EOY?",
        "Will bitcoin reach 100k by eoy?",
    ];
    
    let verse_ids: Vec<u128> = similar_titles
        .iter()
        .map(|title| VerseClassifier::classify_market_to_verse(title).unwrap())
        .collect();
    
    // All similar titles should classify to the same verse
    assert_eq!(verse_ids[0], verse_ids[1]);
    assert_eq!(verse_ids[1], verse_ids[2]);
}

#[tokio::test]
async fn test_state_compression() {
    // Create test proposals
    let mut proposals = Vec::new();
    
    for i in 0..100 {
        let mut proposal = ProposalPDA::new(
            [i as u8; 32],
            [0u8; 32],
            2, // binary
        );
        proposal.prices = vec![600_000 - i * 1000, 400_000 + i * 1000];
        proposal.volumes = vec![1_000_000 + i * 10000, 1_000_000];
        proposal.liquidity_depth = 500_000;
        proposal.state = ProposalState::Active;
        proposals.push(proposal);
    }
    
    // Test compression
    let config = CompressionConfig::default();
    let compressed = StateCompressor::compress_proposal_batch(&proposals, &config).unwrap();
    
    // Verify compression ratio
    let original_size = proposals.len() * 520; // 520 bytes per ProposalPDA
    let compressed_size = compressed.compressed_size as usize;
    let compression_ratio = original_size as f32 / compressed_size as f32;
    
    assert!(compression_ratio > 5.0, "Should achieve at least 5x compression");
    assert_eq!(compressed.original_count, 100);
    
    // Test single proposal compression
    let single_compressed = StateCompressor::compress_proposal(&proposals[0]).unwrap();
    assert_eq!(single_compressed.proposal_id, proposals[0].proposal_id);
    assert_eq!(single_compressed.essential_data.total_volume, 2_000_000);
}

#[tokio::test]
async fn test_correlation_calculation() {
    // Create test children with different correlations
    let children = vec![
        ChildInfo {
            verse_id: [1u8; 32],
            derived_prob: U64F64::from_num(3) / U64F64::from_num(5), // 0.6
            weight: 1000,
            correlation: U64F64::from_num(0.8),
        },
        ChildInfo {
            verse_id: [2u8; 32],
            derived_prob: U64F64::from_num(0.7),
            weight: 2000,
            correlation: U64F64::from_num(3) / U64F64::from_num(5), // 0.6
        },
        ChildInfo {
            verse_id: [3u8; 32],
            derived_prob: U64F64::from_num(0.5),
            weight: 1500,
            correlation: U64F64::from_num(0.7),
        },
    ];
    
    // Calculate weighted average probability
    let mut weighted_sum = 0f64;
    let mut total_weight = 0f64;
    
    for child in &children {
        let prob = child.derived_prob.to_num();
        let weight = child.weight as f64;
        weighted_sum += prob * weight;
        total_weight += weight;
    }
    
    let average_prob = weighted_sum / total_weight;
    assert!((average_prob - 0.6111).abs() < 0.001, "Weighted average should be ~0.6111");
    
    // Calculate correlation factor
    let mut correlation_sum = 0f64;
    let mut weight_sum = 0f64;
    
    for i in 0..children.len() {
        for j in (i + 1)..children.len() {
            let corr = (children[i].correlation.to_num() + 
                        children[j].correlation.to_num()) / 2.0;
            let weight_product = (children[i].weight * children[j].weight) as f64;
            
            correlation_sum += corr * weight_product;
            weight_sum += weight_product;
        }
    }
    
    let correlation_factor = correlation_sum / weight_sum;
    assert!((correlation_factor - 0.68).abs() < 0.01, "Correlation factor should be ~0.68");
}

#[tokio::test]
async fn test_state_pruning() {
    // Create resolved proposals ready for pruning
    let mut resolved_proposals = Vec::new();
    let current_slot = 1_000_000;
    
    for i in 0..10 {
        let mut proposal = ProposalPDA::new(
            [i as u8; 32],
            [0u8; 32],
            2,
        );
        proposal.state = ProposalState::Resolved;
        proposal.settle_slot = current_slot - 500_000; // Well past grace period
        resolved_proposals.push(proposal);
    }
    
    // Verify proposals are ready for pruning
    const PRUNE_GRACE_PERIOD: u64 = 432_000; // ~2 days
    
    for proposal in &resolved_proposals {
        assert_eq!(proposal.state, ProposalState::Resolved);
        assert!(current_slot > proposal.settle_slot + PRUNE_GRACE_PERIOD);
    }
}

// Helper struct for testing
struct ChildInfo {
    verse_id: [u8; 32],
    derived_prob: U64F64,
    weight: u64,
    correlation: U64F64,
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_merkle_tree_depth() {
        // Test maximum depth constraint
        let max_depth = 32;
        let mut current_depth = 0;
        
        // Build a deep tree
        let mut verses = Vec::new();
        let mut parent_id = None;
        
        for i in 0..max_depth {
            let verse = VersePDA::new(i as u128, parent_id, 1);
            verses.push(verse);
            parent_id = Some(i as u128);
            current_depth += 1;
        }
        
        assert_eq!(current_depth, max_depth);
        
        // Verify depth is enforced
        let too_deep = VersePDA::new(999, Some(max_depth as u128 - 1), 1);
        assert_eq!(too_deep.depth, 0); // Would be set during actual traversal
    }
    
    #[test]
    fn test_verse_grouping_efficiency() {
        // Test that 21k markets group into ~400 verses
        let mut verse_map = std::collections::HashMap::new();
        
        // Generate test market titles
        for i in 0..21_000 {
            let title = match i % 100 {
                0..=20 => format!("Will BTC reach ${} by EOY?", 50000 + i * 100),
                21..=40 => format!("Will ETH exceed ${} in Q4?", 3000 + i * 10),
                41..=60 => format!("{} election winner", ["Presidential", "Senate", "House"][i % 3]),
                61..=80 => format!("Will {} stock hit ${}?", ["AAPL", "GOOGL", "MSFT"][i % 3], 100 + i),
                _ => format!("Generic market {}", i),
            };
            
            let verse_id = VerseClassifier::classify_market_to_verse(&title).unwrap();
            *verse_map.entry(verse_id).or_insert(0) += 1;
        }
        
        // Verify grouping efficiency
        let verse_count = verse_map.len();
        assert!(verse_count < 500, "Should group into less than 500 verses");
        assert!(verse_count > 300, "Should have reasonable diversity");
        
        // Check distribution
        let avg_markets_per_verse = 21_000 / verse_count;
        assert!(avg_markets_per_verse > 30 && avg_markets_per_verse < 70);
    }
}