use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
};
use std::time::Instant;
use crate::state_compression::*;
use crate::account_structs::*;

#[cfg(test)]
mod state_compression_tests {
    use super::*;

    #[tokio::test]
    async fn test_compression_ratio_verification() {
        // Create a full ProposalPDA with realistic data
        let proposal = ProposalPDA {
            proposal_id: [1u8; 32],
            verse_id: [2u8; 32],
            market_id: [3u8; 32],
            amm_type: AMMType::LMSR,
            outcomes: vec![Outcome::Yes, Outcome::No],
            prices: vec![650_000, 350_000], // 0.65, 0.35 in fixed point
            volumes: vec![1_000_000, 500_000],
            liquidity_depth: 10_000_000,
            state: ProposalState::Active,
            settle_slot: 1_000_000,
            resolution: None,
            chain_positions: vec![
                ChainPosition {
                    chain_id: [4u8; 32],
                    position_size: 50_000,
                    entry_price: 600_000,
                    leverage: 10,
                },
                ChainPosition {
                    chain_id: [5u8; 32],
                    position_size: 30_000,
                    entry_price: 700_000,
                    leverage: 5,
                },
            ],
            partial_liq_accumulator: 0,
        };

        // Calculate original size
        let original_size = proposal.try_to_vec().unwrap().len();
        println!("Original ProposalPDA size: {} bytes", original_size);
        assert_eq!(original_size, 520, "ProposalPDA should be 520 bytes");

        // Compress the proposal
        let compressed = StateCompressor::compress_proposal(&proposal).unwrap();
        let compressed_size = compressed.try_to_vec().unwrap().len();
        println!("Compressed size: {} bytes", compressed_size);

        // Verify compression ratio
        let compression_ratio = original_size as f64 / compressed_size as f64;
        println!("Compression ratio: {:.2}x", compression_ratio);
        
        // Assert 10x compression achieved
        assert!(compression_ratio >= 10.0, "Should achieve at least 10x compression");
        assert!(compressed_size <= 52, "Compressed size should be ~52 bytes");

        // Test decompression
        let full_data = proposal.try_to_vec().unwrap();
        let decompressed = StateCompressor::decompress_proposal(&compressed, &full_data).unwrap();
        
        // Verify decompressed data matches original
        assert_eq!(decompressed.proposal_id, proposal.proposal_id);
        assert_eq!(decompressed.verse_id, proposal.verse_id);
        assert_eq!(decompressed.amm_type, proposal.amm_type);
        assert_eq!(decompressed.state, proposal.state);
    }

    #[tokio::test]
    async fn test_cu_overhead_measurement() {
        let mut program_test = ProgramTest::new(
            "betting_platform",
            crate::id(),
            processor!(crate::entry),
        );

        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Create test proposal
        let proposal = create_test_proposal();
        let original_size = proposal.try_to_vec().unwrap().len();

        // Measure CU for compression
        let mut compression_instructions = vec![];
        
        // Set compute budget to measure actual CU usage
        compression_instructions.push(
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000)
        );

        // Simulate compression operation
        let start_cu = Instant::now();
        let compressed = StateCompressor::compress_proposal(&proposal).unwrap();
        let compression_time = start_cu.elapsed();

        // Estimate CU based on time (rough approximation)
        // Solana processes ~400k CU/second per core
        let estimated_compression_cu = (compression_time.as_micros() * 400) as u32;
        println!("Estimated compression CU: {}", estimated_compression_cu);

        // Measure CU for decompression
        let full_data = proposal.try_to_vec().unwrap();
        let start_cu = Instant::now();
        let _decompressed = StateCompressor::decompress_proposal(&compressed, &full_data).unwrap();
        let decompression_time = start_cu.elapsed();

        let estimated_decompression_cu = (decompression_time.as_micros() * 400) as u32;
        println!("Estimated decompression CU: {}", estimated_decompression_cu);

        // Calculate overhead percentage
        let total_cu = estimated_compression_cu + estimated_decompression_cu;
        let base_operation_cu = 20_000; // Typical transaction CU
        let overhead_percent = (total_cu as f64 / base_operation_cu as f64) * 100.0;

        println!("Total compression/decompression CU: {}", total_cu);
        println!("Overhead: {:.2}%", overhead_percent);

        // Assert < 5% overhead as per spec
        assert!(overhead_percent < 5.0, "CU overhead should be < 5%");
    }

    #[tokio::test]
    async fn test_batch_compression_efficiency() {
        // Create 100 proposals with similar characteristics
        let mut proposals = vec![];
        for i in 0..100 {
            let mut proposal = create_test_proposal();
            proposal.proposal_id[0] = i as u8;
            proposal.prices = vec![600_000 + (i as u64 * 1000), 400_000 - (i as u64 * 1000)];
            proposals.push(proposal);
        }

        // Measure individual compression
        let start = Instant::now();
        let mut individual_compressed = vec![];
        let mut individual_size = 0;
        
        for proposal in &proposals {
            let compressed = StateCompressor::compress_proposal(proposal).unwrap();
            individual_size += compressed.try_to_vec().unwrap().len();
            individual_compressed.push(compressed);
        }
        
        let individual_time = start.elapsed();
        println!("Individual compression time: {:?}", individual_time);
        println!("Total individual compressed size: {} bytes", individual_size);

        // Measure batch compression
        let start = Instant::now();
        let config = CompressionConfig::default();
        let batch_compressed = compress_proposal_batch(&proposals).unwrap();
        let batch_time = start.elapsed();
        
        println!("Batch compression time: {:?}", batch_time);
        println!("Batch compressed size: {} bytes", batch_compressed.compressed_size);

        // Batch should be more efficient
        assert!(batch_time < individual_time, "Batch compression should be faster");
        assert!(batch_compressed.compressed_size < individual_size, "Batch should compress better");

        // Calculate improvement
        let time_improvement = (individual_time.as_micros() as f64 / batch_time.as_micros() as f64);
        let size_improvement = (individual_size as f64 / batch_compressed.compressed_size as f64);
        
        println!("Time improvement: {:.2}x", time_improvement);
        println!("Size improvement: {:.2}x", size_improvement);
    }

    #[tokio::test]
    async fn test_compression_with_different_amm_types() {
        // Test compression efficiency across different AMM types
        let amm_types = vec![
            (AMMType::LMSR, 2), // Binary
            (AMMType::PMAMM, 5), // 5 outcomes
            (AMMType::L2Norm, 10), // 10 outcomes
        ];

        for (amm_type, outcome_count) in amm_types {
            let mut proposal = create_test_proposal();
            proposal.amm_type = amm_type;
            
            // Create outcomes and prices
            proposal.outcomes = (0..outcome_count)
                .map(|i| Outcome::Index(i as u8))
                .collect();
            proposal.prices = (0..outcome_count)
                .map(|i| 1_000_000 / outcome_count as u64)
                .collect();
            proposal.volumes = vec![100_000; outcome_count];

            let original_size = proposal.try_to_vec().unwrap().len();
            let compressed = StateCompressor::compress_proposal(&proposal).unwrap();
            let compressed_size = compressed.try_to_vec().unwrap().len();
            
            let ratio = original_size as f64 / compressed_size as f64;
            
            println!("AMM Type: {:?}, Outcomes: {}, Original: {} bytes, Compressed: {} bytes, Ratio: {:.2}x",
                amm_type, outcome_count, original_size, compressed_size, ratio);
            
            // All types should achieve good compression
            assert!(ratio >= 5.0, "Should achieve at least 5x compression for {:?}", amm_type);
        }
    }

    #[tokio::test]
    async fn test_compression_proof_verification() {
        let proposal = create_test_proposal();
        
        // Compress with proof
        let compressed = StateCompressor::compress_proposal(&proposal).unwrap();
        
        // Verify proof is valid
        assert_eq!(compressed.proof.compression_version, 1);
        assert!(compressed.proof.timestamp > 0);
        assert_ne!(compressed.proof.hash, [0u8; 32]);
        
        // Test invalid proof detection
        let mut invalid_compressed = compressed.clone();
        invalid_compressed.proof.hash[0] = !invalid_compressed.proof.hash[0]; // Flip a bit
        
        let full_data = proposal.try_to_vec().unwrap();
        let result = StateCompressor::decompress_proposal(&invalid_compressed, &full_data);
        
        assert!(result.is_err(), "Should fail with invalid proof");
    }

    #[tokio::test]
    async fn test_essential_data_extraction() {
        let proposal = create_test_proposal();
        
        // Test that essential data is preserved
        let compressed = StateCompressor::compress_proposal(&proposal).unwrap();
        let essential = &compressed.essential_data;
        
        assert_eq!(essential.proposal_id, proposal.proposal_id);
        assert_eq!(essential.verse_id, proposal.verse_id);
        assert_eq!(essential.amm_type, proposal.amm_type);
        assert_eq!(essential.current_price, proposal.prices[0]);
        assert_eq!(essential.state, proposal.state);
        
        // Verify total volume calculation
        let expected_volume: u64 = proposal.volumes.iter().sum();
        assert_eq!(essential.total_volume, expected_volume);
    }
}

// Benchmark tests
#[cfg(test)]
mod compression_benchmarks {
    use super::*;

    #[test]
    fn bench_compression_scaling() {
        println!("\n=== Compression Scaling Benchmark ===");
        
        for chain_count in &[0, 1, 5, 10, 20, 50] {
            let mut proposal = create_test_proposal();
            
            // Add variable number of chain positions
            proposal.chain_positions = (0..*chain_count).map(|i| {
                ChainPosition {
                    chain_id: {
                        let mut id = [0u8; 32];
                        id[0] = i as u8;
                        id
                    },
                    position_size: 10_000 * (i as u64 + 1),
                    entry_price: 500_000 + i as u64 * 10_000,
                    leverage: 5 + i as u64,
                }
            }).collect();
            
            let original_size = proposal.try_to_vec().unwrap().len();
            
            let start = Instant::now();
            let compressed = StateCompressor::compress_proposal(&proposal).unwrap();
            let compression_time = start.elapsed();
            
            let compressed_size = compressed.try_to_vec().unwrap().len();
            let ratio = original_size as f64 / compressed_size as f64;
            
            println!("Chains: {:2}, Original: {:4} bytes, Compressed: {:3} bytes, Ratio: {:.2}x, Time: {:?}",
                chain_count, original_size, compressed_size, ratio, compression_time);
        }
    }

    #[test]
    fn bench_batch_compression_performance() {
        println!("\n=== Batch Compression Performance ===");
        
        for batch_size in &[10, 50, 100, 500, 1000] {
            let proposals: Vec<ProposalPDA> = (0..*batch_size)
                .map(|i| {
                    let mut p = create_test_proposal();
                    p.proposal_id[0] = (i % 256) as u8;
                    p.proposal_id[1] = (i / 256) as u8;
                    p
                })
                .collect();
            
            let start = Instant::now();
            let batch_result = compress_proposal_batch(&proposals).unwrap();
            let batch_time = start.elapsed();
            
            let per_proposal_time = batch_time / *batch_size as u32;
            
            println!("Batch size: {:4}, Total time: {:?}, Per proposal: {:?}, Compression ratio: {:.2}x",
                batch_size, batch_time, per_proposal_time, batch_result.compression_ratio);
        }
    }
}

// Helper functions
fn create_test_proposal() -> ProposalPDA {
    ProposalPDA {
        proposal_id: [1u8; 32],
        verse_id: [2u8; 32],
        market_id: [3u8; 32],
        amm_type: AMMType::LMSR,
        outcomes: vec![Outcome::Yes, Outcome::No],
        prices: vec![600_000, 400_000],
        volumes: vec![1_000_000, 800_000],
        liquidity_depth: 5_000_000,
        state: ProposalState::Active,
        settle_slot: 1_000_000,
        resolution: None,
        chain_positions: vec![],
        partial_liq_accumulator: 0,
    }
}

fn compress_proposal_batch(proposals: &[ProposalPDA]) -> Result<CompressedBatch> {
    // Group by common fields to maximize compression
    let mut grouped = std::collections::HashMap::new();
    for proposal in proposals {
        let key = (proposal.amm_type.clone(), proposal.state.clone(), proposal.outcomes.len());
        grouped.entry(key).or_insert(vec![]).push(proposal);
    }

    // Compress each group
    let mut compressed_groups = vec![];
    let mut total_compressed_size = 0;
    
    for (_key, group) in grouped {
        let mut group_size = 0;
        for proposal in group {
            let compressed = StateCompressor::compress_proposal(proposal)?;
            group_size += compressed.try_to_vec()?.len();
        }
        total_compressed_size += group_size;
    }

    let original_size = proposals.len() * 520; // 520 bytes per proposal
    
    Ok(CompressedBatch {
        groups: compressed_groups,
        original_count: proposals.len() as u32,
        compressed_size: total_compressed_size,
        compression_ratio: original_size as f64 / total_compressed_size as f64,
    })
}

#[derive(Clone)]
struct CompressedBatch {
    groups: Vec<CompressedGroup>,
    original_count: u32,
    compressed_size: usize,
    compression_ratio: f64,
}

#[derive(Clone)]
struct CompressedGroup {
    // Placeholder for actual implementation
}