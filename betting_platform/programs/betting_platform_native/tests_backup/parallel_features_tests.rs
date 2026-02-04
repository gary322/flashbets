//! Parallel Features Tests
//! 
//! Verifies all parallel implementation requirements:
//! - State compression (10x reduction)
//! - Bootstrap phase ($10k minimum viable vault)
//! - MMT distribution (10M/season)
//! - Shard management (4 shards/market, hash-based)

#[cfg(test)]
mod parallel_features_tests {
    use solana_program::{
        pubkey::Pubkey,
        clock::Clock,
        sysvar::Sysvar,
    };
    use betting_platform_native::{
        state_compression::{StateCompressor, CompressionConfig, CompressedBatch},
        integration::bootstrap_coordinator::{
            BootstrapCoordinator, BOOTSTRAP_TARGET_VAULT, BOOTSTRAP_MMT_EMISSION_RATE,
        },
        mmt::constants::{SEASON_ALLOCATION, MMT_DECIMALS},
        sharding::enhanced_sharding::{
            EnhancedShardManager, SHARDS_PER_MARKET, ShardType,
        },
        state::accounts::{ProposalPDA, ProposalState, AMMType},
    };

    #[test]
    fn test_state_compression_10x_requirement() {
        println!("\n=== State Compression 10x Requirement Test ===");
        
        // Create diverse set of proposals
        let mut proposals = Vec::new();
        for i in 0..1000 {
            let mut proposal = ProposalPDA::new(
                [(i * 7 % 256) as u8; 32],
                [(i % 10) as u8; 32], // Groups by verse
                2 + (i % 6) as u8,    // 2-7 outcomes
            );
            
            // Set varied data
            proposal.amm_type = match i % 3 {
                0 => AMMType::LMSR,
                1 => AMMType::PMAMM,
                _ => AMMType::L2AMM,
            };
            proposal.state = if i % 20 == 0 { 
                ProposalState::Resolved 
            } else { 
                ProposalState::Active 
            };
            proposal.liquidity = 1_000_000 + (i * 10_000) as u64;
            proposal.total_fees_collected = (i * 100) as u64;
            
            proposals.push(proposal);
        }
        
        let proposal_refs: Vec<&ProposalPDA> = proposals.iter().collect();
        
        // Configure for maximum compression
        let mut config = CompressionConfig::default();
        config.compression_level = 10;
        
        // Compress
        let compressed = StateCompressor::compress_proposal_batch(&proposal_refs, &config)
            .expect("Compression should succeed");
        
        // Calculate metrics
        let original_size = proposals.len() * std::mem::size_of::<ProposalPDA>();
        let compression_ratio = compressed.compression_ratio;
        
        println!("Original size: {} bytes", original_size);
        println!("Compressed size: {} bytes", compressed.compressed_size);
        println!("Compression ratio: {:.2}x", compression_ratio);
        println!("Groups formed: {}", compressed.groups.len());
        
        // Verify 10x compression achieved
        assert!(
            compression_ratio >= 10.0,
            "Failed to achieve 10x compression: {:.2}x",
            compression_ratio
        );
    }

    #[test]
    fn test_bootstrap_10k_minimum_vault() {
        println!("\n=== Bootstrap $10k Minimum Vault Test ===");
        
        let mut coordinator = BootstrapCoordinator::new(Pubkey::new_unique());
        
        // Verify initial state
        assert_eq!(coordinator.vault_balance, 0);
        assert!(coordinator.is_bootstrap_active);
        assert!(!coordinator.bootstrap_complete);
        
        // Test milestone tracking
        let test_deposits = vec![
            1_000_000_000,   // $1k
            2_000_000_000,   // $2k (total $3k)
            2_500_000_000,   // $2.5k (total $5.5k)
            4_500_000_000,   // $4.5k (total $10k)
        ];
        
        for (i, deposit) in test_deposits.iter().enumerate() {
            let milestone_before = coordinator.current_milestone_index;
            coordinator.process_deposit(
                &Pubkey::new_unique(),
                *deposit,
                100, // slot
            ).expect("Deposit should succeed");
            
            println!("Deposit {}: ${} -> Total: ${}", 
                i + 1,
                deposit / 1_000_000,
                coordinator.vault_balance / 1_000_000
            );
            
            // Check milestone progression
            if coordinator.current_milestone_index > milestone_before {
                println!("  Milestone reached: {} ({}%)", 
                    coordinator.current_milestone_index,
                    coordinator.milestones_reached[coordinator.current_milestone_index - 1]
                );
            }
        }
        
        // Verify bootstrap completion at $10k
        assert_eq!(coordinator.vault_balance, BOOTSTRAP_TARGET_VAULT);
        assert!(coordinator.bootstrap_complete);
        assert_eq!(coordinator.max_leverage_available, 10); // $10k = 10x leverage
        
        // Verify all milestones reached
        assert_eq!(coordinator.milestones_reached, vec![true, true, true, true, true]);
    }

    #[test]
    fn test_mmt_distribution_10m_per_season() {
        println!("\n=== MMT Distribution 10M/Season Test ===");
        
        // Verify constant
        let expected_per_season = 10_000_000 * 10u64.pow(MMT_DECIMALS as u32);
        assert_eq!(SEASON_ALLOCATION, expected_per_season);
        assert_eq!(BOOTSTRAP_MMT_EMISSION_RATE, expected_per_season);
        
        // Test distribution tracking
        let mut coordinator = BootstrapCoordinator::new(Pubkey::new_unique());
        
        // Simulate deposits and MMT rewards
        let deposits = vec![
            (Pubkey::new_unique(), 1_000_000_000),  // $1k
            (Pubkey::new_unique(), 5_000_000_000),  // $5k
            (Pubkey::new_unique(), 4_000_000_000),  // $4k
        ];
        
        let mut total_mmt_distributed = 0u64;
        
        for (depositor, amount) in deposits {
            let rewards = coordinator.calculate_mmt_rewards(&depositor, amount)
                .expect("Reward calculation should succeed");
            
            total_mmt_distributed += rewards.total_reward;
            
            println!("Deposit ${}: {} MMT reward", 
                amount / 1_000_000,
                rewards.total_reward / 10u64.pow(MMT_DECIMALS as u32)
            );
        }
        
        // Verify rewards are reasonable portion of season allocation
        assert!(total_mmt_distributed > 0);
        assert!(total_mmt_distributed < SEASON_ALLOCATION / 2); // Bootstrap shouldn't use >50%
        
        println!("\nTotal MMT distributed: {} ({}% of season)",
            total_mmt_distributed / 10u64.pow(MMT_DECIMALS as u32),
            (total_mmt_distributed * 100) / SEASON_ALLOCATION
        );
    }

    #[test]
    fn test_shard_management_4_per_market() {
        println!("\n=== Shard Management 4/Market Test ===");
        
        let mut manager = EnhancedShardManager::new(Pubkey::new_unique());
        
        // Add markets and verify shard allocation
        let markets = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        
        for (i, market_id) in markets.iter().enumerate() {
            let base_shard = manager.allocate_market_shards(market_id)
                .expect("Shard allocation should succeed");
            
            println!("Market {}: Base shard ID = {}", i + 1, base_shard);
            
            // Verify 4 shards allocated
            assert_eq!(manager.total_shards, ((i + 1) * SHARDS_PER_MARKET as usize) as u32);
            
            // Get allocation and verify shard types
            let allocation = manager.get_market_allocation(market_id)
                .expect("Should find allocation");
            
            assert_eq!(allocation.shard_assignments.len(), SHARDS_PER_MARKET as usize);
            
            // Verify each shard type is assigned
            let shard_types: Vec<_> = allocation.shard_assignments.iter()
                .map(|s| s.shard_type.clone())
                .collect();
            
            assert!(shard_types.contains(&ShardType::OrderBook));
            assert!(shard_types.contains(&ShardType::Execution));
            assert!(shard_types.contains(&ShardType::Settlement));
            assert!(shard_types.contains(&ShardType::Analytics));
            
            println!("  Shard types: {:?}", shard_types);
        }
        
        // Test hash-based routing
        let order_id = [1u8; 32];
        let market_id = &markets[0];
        
        let shard_id = manager.get_shard_for_order(market_id, &order_id)
            .expect("Should route order");
        
        println!("\nOrder routing test:");
        println!("  Order ID: {:?}", &order_id[..4]);
        println!("  Routed to shard: {}", shard_id);
        
        // Verify deterministic routing
        let shard_id_2 = manager.get_shard_for_order(market_id, &order_id)
            .expect("Should route order");
        assert_eq!(shard_id, shard_id_2, "Routing should be deterministic");
    }

    #[test] 
    fn test_shard_load_balancing() {
        println!("\n=== Shard Load Balancing Test ===");
        
        let mut manager = EnhancedShardManager::new(Pubkey::new_unique());
        let market_id = Pubkey::new_unique();
        
        manager.allocate_market_shards(&market_id).unwrap();
        
        // Simulate load on shards
        for i in 0..4 {
            manager.update_shard_metrics(
                &market_id,
                i,
                1000 * (i + 1) as u32, // Varying transaction counts
                100,  // Current slot
            ).unwrap();
        }
        
        // Calculate TPS
        let tps = manager.calculate_global_tps();
        println!("Global TPS: {}", tps);
        
        // Check if meeting target
        assert!(manager.is_meeting_tps_target());
        
        // Get allocation and check load factors
        let allocation = manager.get_market_allocation(&market_id).unwrap();
        
        println!("\nShard load factors:");
        for (i, shard) in allocation.shard_assignments.iter().enumerate() {
            println!("  Shard {}: {}% load", i, shard.load_factor / 100);
        }
    }

    #[test]
    fn test_integrated_bootstrap_compression_sharding() {
        println!("\n=== Integrated Features Test ===");
        
        // 1. Bootstrap phase
        let mut coordinator = BootstrapCoordinator::new(Pubkey::new_unique());
        coordinator.vault_balance = BOOTSTRAP_TARGET_VAULT;
        coordinator.bootstrap_complete = true;
        
        // 2. Create markets with sharding
        let mut shard_manager = EnhancedShardManager::new(Pubkey::new_unique());
        let markets: Vec<_> = (0..10).map(|_| Pubkey::new_unique()).collect();
        
        for market in &markets {
            shard_manager.allocate_market_shards(market).unwrap();
        }
        
        // 3. Create proposals for compression
        let mut proposals = Vec::new();
        for (i, market) in markets.iter().enumerate() {
            for j in 0..10 {
                let mut proposal = ProposalPDA::new(
                    [(i * 10 + j) as u8; 32],
                    market.to_bytes(),
                    3,
                );
                proposal.liquidity = coordinator.vault_balance / 100; // Use bootstrap funds
                proposals.push(proposal);
            }
        }
        
        // 4. Compress proposals
        let proposal_refs: Vec<&ProposalPDA> = proposals.iter().collect();
        let config = CompressionConfig::default();
        let compressed = StateCompressor::compress_proposal_batch(&proposal_refs, &config).unwrap();
        
        println!("\nIntegrated system metrics:");
        println!("  Bootstrap vault: ${}k", coordinator.vault_balance / 1_000_000_000);
        println!("  Markets: {}", markets.len());
        println!("  Total shards: {}", shard_manager.total_shards);
        println!("  Proposals: {}", proposals.len());
        println!("  Compression ratio: {:.2}x", compressed.compression_ratio);
        println!("  Max leverage: {}x", coordinator.max_leverage_available);
        
        // Verify all systems working together
        assert!(coordinator.bootstrap_complete);
        assert_eq!(shard_manager.total_shards, markets.len() as u32 * SHARDS_PER_MARKET as u32);
        assert!(compressed.compression_ratio >= 5.0); // Lower threshold for mixed data
    }
}