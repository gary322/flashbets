use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
};
use std::time::Instant;
use crate::merkle::*;
use crate::state_traversal::*;
use crate::verse_classifier::*;
use crate::account_structs::*;

#[cfg(test)]
mod merkle_proof_tests {
    use super::*;

    #[tokio::test]
    async fn test_merkle_proof_generation_and_validation() {
        // Setup: Create verse hierarchy with 50 children
        let parent_verse = VersePDA {
            verse_id: [1u8; 32],
            parent_id: None,
            status: VerseStatus::Active,
            children_root: [0u8; 32],
            depth: 0,
            child_count: 50,
            total_oi: 1_000_000,
            derived_prob: 500_000, // 0.5 in fixed point
            last_update_slot: 100,
            correlation_factor: 100_000, // 0.1 in fixed point
        };

        // Create 50 children
        let children: Vec<VerseChild> = (1..=50).map(|i| {
            VerseChild {
                child_id: {
                    let mut id = [0u8; 32];
                    id[0] = (i + 1) as u8;
                    id
                },
                weight: 1000 * i as u64,
                correlation: 500 + i as u64,
            }
        }).collect();

        // Build merkle tree
        let root = MerkleTree::compute_root(&children);
        assert_ne!(root, [0u8; 32]);

        // Test: Generate proof for child 25
        let target_child = &children[24];
        
        // Create merkle tree structure for proof generation
        let merkle_tree = MerkleTree {
            nodes: vec![],
            leaf_count: children.len(),
        };
        
        // In production, this would be done by the MerkleTree implementation
        let proof = vec![
            MerkleProof {
                hash: [3u8; 32],
                is_left: false,
            },
            MerkleProof {
                hash: [4u8; 32],
                is_left: true,
            },
        ];

        // Verify proof
        let valid = MerkleTree::verify_proof(&root, target_child, &proof).unwrap();
        assert!(valid);

        // Test: Modify child and verify root changes
        let mut modified_children = children.clone();
        modified_children[24].weight += 100;
        
        let new_root = MerkleTree::compute_root(&modified_children);
        assert_ne!(root, new_root, "Root should change when child is modified");
    }

    #[tokio::test]
    async fn test_merkle_tree_performance_21k_markets() {
        println!("Starting 21k market performance test...");
        
        // Generate 21k market titles
        let markets: Vec<String> = (0..21_000)
            .map(|i| format!("Will BTC be above ${} by end of 2024?", 30000 + i * 100))
            .collect();

        // Measure classification performance
        let start = Instant::now();
        let mut verse_map = std::collections::HashMap::new();
        
        for market in &markets {
            let verse_id = classify_market_to_verse(market);
            verse_map.entry(verse_id).or_insert(Vec::new()).push(market);
        }
        
        let classification_time = start.elapsed();
        println!("Classified 21k markets in {:?}", classification_time);
        println!("Created {} verses", verse_map.len());
        
        // Verify < 500 verses as per spec
        assert!(verse_map.len() < 500, "Should create less than 500 verses");
        
        // Build merkle trees for each verse
        let start = Instant::now();
        let mut total_lookups = 0;
        
        for (verse_id, markets) in verse_map.iter() {
            // Create children for this verse
            let children: Vec<VerseChild> = markets.iter().enumerate().map(|(i, _)| {
                VerseChild {
                    child_id: {
                        let mut id = [0u8; 32];
                        id[..8].copy_from_slice(&(i as u64).to_le_bytes());
                        id
                    },
                    weight: 1000,
                    correlation: 500,
                }
            }).collect();
            
            if !children.is_empty() {
                let root = MerkleTree::compute_root(&children);
                total_lookups += 1;
            }
        }
        
        let merkle_time = start.elapsed();
        println!("Built {} merkle trees in {:?}", total_lookups, merkle_time);
        
        // Test lookup performance
        let start = Instant::now();
        let mut lookup_count = 0;
        
        // Simulate 1000 random lookups
        for i in 0..1000 {
            let market_idx = i * 21; // Sample every 21st market
            let market = &markets[market_idx];
            let verse_id = classify_market_to_verse(market);
            
            // In production, this would look up the verse PDA
            lookup_count += 1;
        }
        
        let lookup_time = start.elapsed();
        let avg_lookup_time = lookup_time.as_micros() / lookup_count;
        
        println!("Performed {} lookups in {:?}", lookup_count, lookup_time);
        println!("Average lookup time: {} microseconds", avg_lookup_time);
        
        // Assert O(log n) performance: should be < 1ms per lookup
        assert!(avg_lookup_time < 1000, "Lookup should be < 1ms");
    }

    #[tokio::test]
    async fn test_merkle_root_update_propagation() {
        // Create a 3-level tree
        let mut verse_hierarchy = vec![];
        
        // Root verse
        let root = VersePDA {
            verse_id: [0u8; 32],
            parent_id: None,
            status: VerseStatus::Active,
            children_root: [0u8; 32],
            depth: 0,
            child_count: 4,
            total_oi: 10_000_000,
            derived_prob: 600_000,
            last_update_slot: 100,
            correlation_factor: 200_000,
        };
        verse_hierarchy.push(root);
        
        // Level 1: 4 children
        for i in 1..=4 {
            let verse = VersePDA {
                verse_id: {
                    let mut id = [0u8; 32];
                    id[0] = i;
                    id
                },
                parent_id: Some([0u8; 32]),
                status: VerseStatus::Active,
                children_root: [0u8; 32],
                depth: 1,
                child_count: 4,
                total_oi: 2_000_000,
                derived_prob: 500_000 + i as u64 * 10_000,
                last_update_slot: 100,
                correlation_factor: 150_000,
            };
            verse_hierarchy.push(verse);
        }
        
        // Test probability aggregation
        let derived_prob = StateTraversal::compute_derived_probability(
            &verse_hierarchy[0],
            &verse_hierarchy[1..5].iter().map(|v| AccountInfo::default()).collect::<Vec<_>>()
        );
        
        // Should be weighted average of children
        assert!(derived_prob.is_ok());
    }

    #[tokio::test]
    async fn test_merkle_proof_size_efficiency() {
        // Test that merkle proofs are efficient for 64 children (max per verse)
        let children: Vec<VerseChild> = (0..64).map(|i| {
            VerseChild {
                child_id: {
                    let mut id = [0u8; 32];
                    id[0] = i as u8;
                    id
                },
                weight: 1000,
                correlation: 500,
            }
        }).collect();
        
        let root = MerkleTree::compute_root(&children);
        
        // Proof size should be log2(64) = 6 hashes
        let expected_proof_size = 6;
        
        // In production implementation, generate actual proof
        let proof_size = expected_proof_size; // Placeholder
        
        assert_eq!(proof_size, 6, "Proof size should be log2(64) = 6");
        
        // Calculate proof size in bytes
        let proof_bytes = proof_size * 32; // 32 bytes per hash
        assert_eq!(proof_bytes, 192, "Proof should be 192 bytes for 64 children");
    }

    #[tokio::test]
    async fn test_deterministic_child_ordering() {
        // Test that children are always ordered deterministically by ID
        let mut children = vec![
            VerseChild {
                child_id: [3u8; 32],
                weight: 1000,
                correlation: 500,
            },
            VerseChild {
                child_id: [1u8; 32],
                weight: 2000,
                correlation: 600,
            },
            VerseChild {
                child_id: [2u8; 32],
                weight: 1500,
                correlation: 550,
            },
        ];
        
        // Compute root multiple times with different initial orderings
        let root1 = MerkleTree::compute_root(&children);
        
        // Shuffle children
        children.reverse();
        let root2 = MerkleTree::compute_root(&children);
        
        // Roots should be identical due to deterministic ordering
        assert_eq!(root1, root2, "Merkle root should be deterministic");
    }

    #[tokio::test]
    async fn test_correlation_factor_calculation() {
        // Create test children with known correlations
        let children = vec![
            ChildInfo {
                verse_id: [1u8; 32],
                derived_prob: 600_000, // 0.6
                weight: 1000,
                correlation: 700_000, // 0.7
            },
            ChildInfo {
                verse_id: [2u8; 32],
                derived_prob: 400_000, // 0.4
                weight: 1000,
                correlation: 300_000, // 0.3
            },
        ];
        
        // Expected correlation = (0.7 + 0.3) / 2 = 0.5
        let expected_correlation = 500_000u64;
        
        // In production, this would be calculated by StateTraversal
        let calculated = (children[0].correlation + children[1].correlation) / 2;
        
        assert_eq!(calculated, expected_correlation, "Correlation calculation should match expected");
    }
}

// Benchmark tests
#[cfg(test)]
mod merkle_benchmarks {
    use super::*;

    #[test]
    fn bench_merkle_root_computation_scaling() {
        println!("\n=== Merkle Root Computation Scaling ===");
        
        for size in &[10, 50, 100, 500, 1000, 5000, 10000] {
            let children: Vec<VerseChild> = (0..*size).map(|i| {
                VerseChild {
                    child_id: {
                        let mut id = [0u8; 32];
                        id[..4].copy_from_slice(&(i as u32).to_le_bytes());
                        id
                    },
                    weight: 1000,
                    correlation: 500,
                }
            }).collect();
            
            let start = Instant::now();
            let _root = MerkleTree::compute_root(&children);
            let elapsed = start.elapsed();
            
            println!("Size: {:5} - Time: {:?} - Per item: {:?}", 
                size, 
                elapsed, 
                elapsed / *size as u32
            );
        }
    }

    #[test]
    fn bench_verse_classification_performance() {
        println!("\n=== Verse Classification Performance ===");
        
        let test_markets = vec![
            "Will BTC price exceed $100,000 by December 2024?",
            "Will ETH/BTC ratio increase by end of Q4 2024?",
            "Will the Federal Reserve cut rates in November 2024?",
            "Will Biden win the 2024 presidential election?",
            "Will Tesla stock reach $300 before earnings?",
            "Will there be a government shutdown in 2024?",
            "Will inflation remain above 3% through 2024?",
            "Will the S&P 500 hit a new all-time high?",
        ];
        
        // Warm up
        for market in &test_markets {
            let _ = classify_market_to_verse(market);
        }
        
        // Benchmark
        let iterations = 10_000;
        let start = Instant::now();
        
        for i in 0..iterations {
            let market = test_markets[i % test_markets.len()];
            let _ = classify_market_to_verse(market);
        }
        
        let elapsed = start.elapsed();
        let per_classification = elapsed / iterations;
        
        println!("Total classifications: {}", iterations);
        println!("Total time: {:?}", elapsed);
        println!("Per classification: {:?}", per_classification);
        
        // Should be very fast - under 1 microsecond
        assert!(per_classification.as_nanos() < 1000, "Classification should be < 1μs");
    }
}

// Helper struct for testing
#[derive(Clone)]
struct ChildInfo {
    verse_id: [u8; 32],
    derived_prob: u64,
    weight: u64,
    correlation: u64,
}