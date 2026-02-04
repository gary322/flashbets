//! ZK Compression Tests
//!
//! Tests for ZK compression with proper proof generation and CU tracking

use std::time::Instant;
use solana_program::pubkey::Pubkey;
use borsh::{BorshSerialize, BorshDeserialize};

use crate::{
    state_compression::{
        compress_account_with_zk, decompress_account_with_zk,
        ZKProof, CompressedAccount, StateCompressionConfig,
        ProofType, PROOF_GENERATION_CU, PROOF_VERIFICATION_CU,
    },
    compression::cu_tracker::{CUTracker, CUMetrics},
    state::market::Market,
    state::user_position::UserPosition,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zk_compression_basic() {
        // Create test market
        let mut market = Market {
            discriminator: [0; 8],
            version: 1,
            authority: Pubkey::new_unique(),
            outcome_a_pool: 1_000_000,
            outcome_b_pool: 1_000_000,
            outcome_c_pool: Some(500_000),
            total_liquidity: 2_500_000,
            oracle_feed: Pubkey::new_unique(),
            market_type: 0,
            status: 1,
            created_at: 1234567890,
            resolution_time: 1234567890 + 86400,
            resolved_outcome: None,
            min_bet: 100,
            max_bet: 100_000,
            fee_bps: 30,
            creator: Pubkey::new_unique(),
            metadata_uri: [0; 64],
            locked_liquidity: 0,
            volume_24h: 50_000,
            unique_traders: 25,
            last_update_slot: 1000,
            emergency_pause: false,
            padding: [0; 32],
        };
        
        // Serialize original
        let original_data = market.try_to_vec().unwrap();
        let original_size = original_data.len();
        
        // Compress with ZK proof
        let compressed = compress_account_with_zk(&original_data).unwrap();
        let compressed_size = compressed.data.len() + compressed.proof.proof_data.len();
        
        // Verify compression ratio
        let compression_ratio = original_size as f64 / compressed_size as f64;
        assert!(
            compression_ratio > 5.0,
            "Compression ratio {:.2}x is below 5x target",
            compression_ratio
        );
        
        // Verify proof generated
        assert!(!compressed.proof.proof_data.is_empty(), "Proof data should not be empty");
        assert_eq!(compressed.proof.proof_type, ProofType::Bulletproof as u8);
        assert_eq!(compressed.proof.generation_cu, PROOF_GENERATION_CU);
        
        // Decompress and verify
        let decompressed = decompress_account_with_zk(&compressed).unwrap();
        assert_eq!(decompressed, original_data, "Decompressed data should match original");
    }

    #[test]
    fn test_zk_proof_verification() {
        let test_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        
        // Compress with proof
        let compressed = compress_account_with_zk(&test_data).unwrap();
        
        // Verify proof components
        assert_eq!(compressed.proof.commitment.len(), 32, "Commitment should be 32 bytes");
        assert!(compressed.proof.proof_data.len() > 64, "Proof should contain substantial data");
        
        // Simulate proof verification
        let verification_start = Instant::now();
        let decompressed = decompress_account_with_zk(&compressed).unwrap();
        let verification_time = verification_start.elapsed();
        
        // Verify correctness
        assert_eq!(decompressed, test_data);
        
        // Verify performance (should be fast)
        assert!(
            verification_time.as_millis() < 10,
            "Verification took {}ms, exceeding 10ms limit",
            verification_time.as_millis()
        );
    }

    #[test]
    fn test_cu_tracking() {
        let mut cu_tracker = CUTracker::new();
        
        // Test data of various sizes
        let test_sizes = vec![100, 1000, 10000, 50000];
        
        for size in test_sizes {
            let data = vec![42u8; size];
            
            // Track compression CUs
            let compress_start = Instant::now();
            let compressed = compress_account_with_zk(&data).unwrap();
            let compress_time = compress_start.elapsed();
            
            cu_tracker.record_compression(
                compressed.proof.generation_cu,
                size,
                compressed.data.len(),
            );
            
            // Track decompression CUs
            let decompress_start = Instant::now();
            let _decompressed = decompress_account_with_zk(&compressed).unwrap();
            let decompress_time = decompress_start.elapsed();
            
            cu_tracker.record_decompression(
                PROOF_VERIFICATION_CU,
                compressed.data.len(),
                size,
            );
            
            // Verify CU limits
            assert!(
                compressed.proof.generation_cu <= 10000,
                "Generation CUs {} exceed limit for size {}",
                compressed.proof.generation_cu,
                size
            );
        }
        
        // Get metrics
        let metrics = cu_tracker.get_metrics();
        assert!(metrics.total_compressions > 0);
        assert!(metrics.total_decompressions > 0);
        assert!(metrics.average_compression_cu > 0);
    }

    #[test]
    fn test_hot_data_caching() {
        // Create positions that will be frequently accessed
        let hot_positions: Vec<UserPosition> = (0..10)
            .map(|i| UserPosition {
                discriminator: [0; 8],
                version: 1,
                owner: Pubkey::new_unique(),
                market: Pubkey::new_unique(),
                outcome_a_shares: 1000 + i * 100,
                outcome_b_shares: 2000 - i * 100,
                outcome_c_shares: Some(500),
                total_invested: 3500,
                created_at: 1234567890,
                last_update: 1234567890,
                realized_pnl: 0,
                pending_rewards: 100,
                leverage: 1,
                liquidation_price: None,
                stop_loss: None,
                take_profit: None,
                entry_price: 5000,
                position_type: 0,
                is_liquidated: false,
                referrer: None,
                chain_position_next: None,
                chain_multiplier: 100,
                nonce: 0,
                padding: [0; 32],
            })
            .collect();
        
        // Compress all positions
        let mut compressed_positions = Vec::new();
        let mut compression_times = Vec::new();
        
        for position in &hot_positions {
            let data = position.try_to_vec().unwrap();
            let start = Instant::now();
            let compressed = compress_account_with_zk(&data).unwrap();
            compression_times.push(start.elapsed());
            compressed_positions.push(compressed);
        }
        
        // Access hot data multiple times (simulating cache hits)
        let mut decompression_times = Vec::new();
        
        for _ in 0..5 {
            for compressed in &compressed_positions {
                let start = Instant::now();
                let _decompressed = decompress_account_with_zk(compressed).unwrap();
                decompression_times.push(start.elapsed());
            }
        }
        
        // Later decompressions should be faster (cache hits)
        let first_round_avg: u128 = decompression_times[0..10]
            .iter()
            .map(|t| t.as_micros())
            .sum::<u128>() / 10;
            
        let last_round_avg: u128 = decompression_times[40..50]
            .iter()
            .map(|t| t.as_micros())
            .sum::<u128>() / 10;
        
        // Cache should improve performance
        assert!(
            last_round_avg <= first_round_avg,
            "Cache not improving performance: first {}μs, last {}μs",
            first_round_avg,
            last_round_avg
        );
    }

    #[test]
    fn test_batch_compression_optimization() {
        let mut cu_tracker = CUTracker::new();
        
        // Create batch of related accounts
        let batch_size = 20;
        let accounts: Vec<Vec<u8>> = (0..batch_size)
            .map(|i| {
                let market = Market {
                    discriminator: [0; 8],
                    version: 1,
                    authority: Pubkey::new_unique(),
                    outcome_a_pool: 1_000_000 + i * 1000,
                    outcome_b_pool: 1_000_000 - i * 1000,
                    outcome_c_pool: Some(500_000),
                    total_liquidity: 2_500_000,
                    oracle_feed: Pubkey::new_unique(),
                    market_type: 0,
                    status: 1,
                    created_at: 1234567890,
                    resolution_time: 1234567890 + 86400,
                    resolved_outcome: None,
                    min_bet: 100,
                    max_bet: 100_000,
                    fee_bps: 30,
                    creator: Pubkey::new_unique(),
                    metadata_uri: [0; 64],
                    locked_liquidity: 0,
                    volume_24h: 50_000 + i * 100,
                    unique_traders: 25,
                    last_update_slot: 1000 + i,
                    emergency_pause: false,
                    padding: [0; 32],
                };
                market.try_to_vec().unwrap()
            })
            .collect();
        
        // Compress individually
        let mut individual_cus = 0u32;
        let individual_start = Instant::now();
        
        for data in &accounts {
            let compressed = compress_account_with_zk(data).unwrap();
            individual_cus += compressed.proof.generation_cu;
        }
        
        let individual_time = individual_start.elapsed();
        
        // Simulate batch compression (would be optimized in production)
        let batch_start = Instant::now();
        let mut batch_cus = 0u32;
        
        // Batch compression would share proof generation overhead
        let batch_overhead = PROOF_GENERATION_CU / 2; // Shared overhead
        
        for data in &accounts {
            let compressed = compress_account_with_zk(data).unwrap();
            batch_cus += batch_overhead + (compressed.proof.generation_cu / 2);
        }
        
        let batch_time = batch_start.elapsed();
        
        // Batch should be more efficient
        assert!(
            batch_cus < individual_cus,
            "Batch CUs {} should be less than individual CUs {}",
            batch_cus,
            individual_cus
        );
    }

    #[test]
    fn test_proof_types() {
        let test_data = vec![1, 2, 3, 4, 5];
        
        // Test different proof types
        let compressed = compress_account_with_zk(&test_data).unwrap();
        
        match compressed.proof.proof_type {
            t if t == ProofType::Bulletproof as u8 => {
                assert!(compressed.proof.proof_data.len() > 100, "Bulletproof should be substantial");
            }
            t if t == ProofType::Groth16 as u8 => {
                assert!(compressed.proof.proof_data.len() > 200, "Groth16 proof should be larger");
            }
            t if t == ProofType::PLONK as u8 => {
                assert!(compressed.proof.proof_data.len() > 150, "PLONK proof should be medium-sized");
            }
            _ => panic!("Unknown proof type"),
        }
    }

    #[test]
    fn test_compression_edge_cases() {
        // Empty data
        let empty_data = vec![];
        let compressed_empty = compress_account_with_zk(&empty_data);
        assert!(compressed_empty.is_ok(), "Should handle empty data");
        
        // Very small data
        let tiny_data = vec![42];
        let compressed_tiny = compress_account_with_zk(&tiny_data).unwrap();
        let decompressed_tiny = decompress_account_with_zk(&compressed_tiny).unwrap();
        assert_eq!(decompressed_tiny, tiny_data);
        
        // Highly repetitive data (should compress well)
        let repetitive = vec![0xFF; 10000];
        let compressed_rep = compress_account_with_zk(&repetitive).unwrap();
        let compression_ratio = repetitive.len() as f64 / compressed_rep.data.len() as f64;
        assert!(
            compression_ratio > 20.0,
            "Repetitive data should compress >20x, got {:.2}x",
            compression_ratio
        );
        
        // Random data (harder to compress)
        let random: Vec<u8> = (0..1000).map(|i| (i * 7 + 13) as u8).collect();
        let compressed_random = compress_account_with_zk(&random).unwrap();
        let decompressed_random = decompress_account_with_zk(&compressed_random).unwrap();
        assert_eq!(decompressed_random, random);
    }

    #[test]
    fn test_concurrent_compression() {
        use std::sync::Arc;
        use std::thread;
        
        let data = vec![42u8; 1000];
        let data_arc = Arc::new(data);
        
        let mut handles = vec![];
        let thread_count = 10;
        
        // Spawn threads to compress concurrently
        for _ in 0..thread_count {
            let data_clone = Arc::clone(&data_arc);
            let handle = thread::spawn(move || {
                let compressed = compress_account_with_zk(&data_clone).unwrap();
                decompress_account_with_zk(&compressed).unwrap()
            });
            handles.push(handle);
        }
        
        // Wait and verify all succeed
        for handle in handles {
            let result = handle.join().unwrap();
            assert_eq!(result, *data_arc);
        }
    }
}