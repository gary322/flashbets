//! State Compression Tests
//! 
//! Verifies the 10x compression requirement

#[cfg(test)]
mod state_compression_tests {
    use solana_program::pubkey::Pubkey;
    use betting_platform_native::{
        state_compression::{StateCompressor, CompressionConfig},
        state::{
            accounts::{ProposalPDA, ProposalState, AMMType},
            verse_accounts::VerseMembershipLevel,
        },
    };

    #[test]
    fn test_10x_compression_requirement() {
        println!("\n=== 10x State Compression Test ===");
        
        // Test different batch sizes
        let batch_sizes = vec![10, 50, 100, 500, 1000];
        
        for batch_size in batch_sizes {
            let mut proposals = Vec::new();
            
            // Create realistic proposals with varying data
            for i in 0..batch_size {
                let mut proposal = ProposalPDA {
                    discriminator: b"PROPOSAL".clone(),
                    proposal_id: [((i * 7) % 256) as u8; 32],
                    verse_account: Pubkey::new_unique(),
                    oracle_account: Pubkey::new_unique(),
                    creator: Pubkey::new_unique(),
                    outcomes: 2 + (i % 6) as u8, // Vary outcomes 2-7
                    outcome_names: vec![
                        format!("Outcome {}", i),
                        format!("Counter {}", i),
                    ],
                    prices: vec![600_000 - (i * 100) as u64, 400_000 + (i * 100) as u64],
                    volumes: vec![1_000_000 + (i * 1000) as u64; 2],
                    liquidity: 10_000_000 + (i * 10_000) as u64,
                    resolution_threshold: 75,
                    resolution_delay: 86400,
                    created_at: 1700000000 + i as i64,
                    resolved_at: None,
                    final_outcome: None,
                    state: if i % 10 == 0 { ProposalState::Resolved } else { ProposalState::Active },
                    amm_type: match i % 3 {
                        0 => AMMType::LMSR,
                        1 => AMMType::PMAMM,
                        _ => AMMType::L2AMM,
                    },
                    fee_bps: 30,
                    operator_fee_bps: 5,
                    total_fees_collected: (i * 100) as u64,
                    total_operator_fees: (i * 20) as u64,
                    min_liquidity: 1_000_000,
                    max_liquidity: 100_000_000,
                    liquidity_locked_until: 1700000000 + (i * 3600) as i64,
                    markets_count: 1 + (i % 5) as u32,
                    unique_traders: 100 + i as u32,
                    last_trade_timestamp: 1700000000 + (i * 60) as i64,
                    oracle_last_update: 1700000000 + (i * 30) as i64,
                    oracle_confidence: 95,
                    dispute_window: 3600,
                    dispute_bond: 1_000_000,
                    disputes_count: 0,
                    membership_required: VerseMembershipLevel::Basic,
                    metadata_uri: format!("https://metadata.example.com/{}", i),
                    reserved: [0u8; 32],
                };
                
                proposals.push(proposal);
            }
            
            // Get references for compression
            let proposal_refs: Vec<&ProposalPDA> = proposals.iter().collect();
            
            // Compress with max compression level
            let mut config = CompressionConfig::default();
            config.compression_level = 10; // Maximum compression
            
            let compressed = StateCompressor::compress_proposal_batch(&proposal_refs, &config)
                .expect("Compression should succeed");
            
            // Calculate actual sizes
            let original_size = proposals.len() * std::mem::size_of::<ProposalPDA>();
            let compressed_size = compressed.compressed_size as usize;
            let actual_ratio = original_size as f32 / compressed_size as f32;
            
            println!("Batch size: {}", batch_size);
            println!("  Original size: {} bytes", original_size);
            println!("  Compressed size: {} bytes", compressed_size);
            println!("  Compression ratio: {:.2}x", actual_ratio);
            println!("  Groups formed: {}", compressed.groups.len());
            
            // Verify compression ratio meets requirement
            assert!(
                actual_ratio >= 10.0,
                "Compression ratio {:.2}x is below 10x requirement for batch size {}",
                actual_ratio,
                batch_size
            );
        }
    }

    #[test]
    fn test_compression_by_grouping() {
        println!("\n=== Compression Grouping Efficiency Test ===");
        
        let mut proposals = Vec::new();
        
        // Create proposals that will group well
        // 100 proposals with only 3 unique (amm_type, state, outcome_count) combinations
        for i in 0..100 {
            let group_id = i % 3;
            let mut proposal = ProposalPDA::new(
                [i as u8; 32],
                [group_id as u8; 32],
                2 + group_id as u8,
            );
            
            proposal.amm_type = match group_id {
                0 => AMMType::LMSR,
                1 => AMMType::PMAMM,
                _ => AMMType::L2AMM,
            };
            
            proposal.state = if group_id == 0 {
                ProposalState::Active
            } else {
                ProposalState::Resolved
            };
            
            proposal.prices = vec![500_000 + (i * 100) as u64; (2 + group_id) as usize];
            proposal.volumes = vec![1_000_000; (2 + group_id) as usize];
            
            proposals.push(proposal);
        }
        
        let proposal_refs: Vec<&ProposalPDA> = proposals.iter().collect();
        let config = CompressionConfig::default();
        
        let compressed = StateCompressor::compress_proposal_batch(&proposal_refs, &config)
            .expect("Compression should succeed");
        
        println!("Groups formed: {} (expected: 3)", compressed.groups.len());
        assert_eq!(compressed.groups.len(), 3, "Should form exactly 3 groups");
        
        // With good grouping, compression should be even better
        println!("Compression ratio with grouping: {:.2}x", compressed.compression_ratio);
        assert!(
            compressed.compression_ratio >= 15.0,
            "Well-grouped data should achieve >15x compression"
        );
    }

    #[test]
    fn test_compression_proof_verification() {
        println!("\n=== Compression Proof Verification Test ===");
        
        let proposal = ProposalPDA::new([1u8; 32], [2u8; 32], 3);
        
        // Compress single proposal
        let compressed = StateCompressor::compress_proposal(&proposal)
            .expect("Compression should succeed");
        
        // Verify proof exists and is valid
        assert_eq!(compressed.proposal_id, proposal.proposal_id);
        assert_eq!(compressed.proof.compression_version, 1);
        assert!(!compressed.proof.merkle_path.is_empty());
        
        // Verify essential data preservation
        assert_eq!(compressed.essential_data.proposal_id, proposal.proposal_id);
        assert_eq!(compressed.essential_data.verse_id, proposal.verse_account.to_bytes());
        assert_eq!(compressed.essential_data.amm_type, proposal.amm_type);
        assert_eq!(compressed.essential_data.state, proposal.state);
        
        println!("Proof hash: {:?}", compressed.proof_hash);
        println!("Merkle path length: {}", compressed.proof.merkle_path.len());
    }

    #[test]
    fn test_compression_cu_usage() {
        println!("\n=== Compression CU Usage Test ===");
        
        let config = CompressionConfig::default();
        
        // Verify CU constants match specification
        assert_eq!(config.proof_verification_cu, 2000, "Proof verification should use ~2000 CU");
        assert_eq!(config.compression_cu, 5000, "Compression should use ~5000 CU");
        
        // Total CU for compression + verification should be reasonable
        let total_cu = config.compression_cu + config.proof_verification_cu;
        assert!(total_cu <= 10_000, "Total CU usage should be under 10k");
        
        println!("Compression CU: {}", config.compression_cu);
        println!("Verification CU: {}", config.proof_verification_cu);
        println!("Total CU: {}", total_cu);
    }
}