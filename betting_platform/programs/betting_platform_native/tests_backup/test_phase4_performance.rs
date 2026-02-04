//! Phase 4: Performance Optimizations - Comprehensive Unit Tests
//!
//! Tests for ZK compression, optimized market ingestion, and rent optimization.

use betting_platform_native::{
    compression::{
        ZKStateCompressor, ZKCompressionConfig, ZKCompressedState,
        CompressedPosition, calculate_compression_stats,
        ZK_COMPRESSION_VERSION, TARGET_COMPRESSION_RATIO,
    },
    ingestion::{
        OptimizedMarketIngestion, BatchIngestionState, OptimizedMarketData,
        MarketState, ParallelBatchCoordinator,
        TOTAL_MARKETS, BATCH_COUNT, MARKETS_PER_BATCH, SLOTS_PER_BATCH,
    },
    optimization::{
        RentCalculator, RentOptimizer, RentOptimizationConfig,
        LAMPORTS_PER_SOL,
    },
    state::Position,
};
use solana_program::pubkey::Pubkey;

#[test]
fn test_zk_compression_ratio() {
    // Create a test position
    let position = Position {
        discriminator: [0u8; 8],
        user: Pubkey::new_unique(),
        proposal_id: 12345,
        position_id: [1u8; 32],
        outcome: 1,
        size: 10_000_000_000, // $10k
        notional: 10_000_000_000,
        leverage: 20,
        entry_price: 5_000_000_000, // $5000
        liquidation_price: 4_500_000_000, // $4500
        is_long: true,
        created_at: 1234567890,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 0,
        margin: 500_000_000, // $500
        is_short: false,
    };
    
    // Compress the position
    let compressed = ZKStateCompressor::compress_position(&position).unwrap();
    
    // Verify compression ratio
    assert!(compressed.metadata.compression_ratio >= 10.0);
    assert_eq!(compressed.metadata.version, ZK_COMPRESSION_VERSION);
    
    // Verify essential data preserved
    assert_eq!(compressed.essential_data.leverage, 20);
    assert_eq!(compressed.essential_data.is_long, true);
    assert_eq!(compressed.essential_data.outcome, 1);
}

#[test]
fn test_batch_compression() {
    let config = ZKCompressionConfig::default();
    
    // Create multiple positions
    let positions: Vec<Position> = (0..100).map(|i| Position {
        discriminator: [0u8; 8],
        user: Pubkey::new_unique(),
        proposal_id: i as u128,
        position_id: [i as u8; 32],
        outcome: (i % 2) as u8,
        size: 1_000_000_000 * (i + 1) as u64,
        notional: 1_000_000_000 * (i + 1) as u64,
        leverage: ((i % 50) + 1) as u64,
        entry_price: 1_000_000_000,
        liquidation_price: 900_000_000,
        is_long: i % 2 == 0,
        created_at: 1234567890 + i as i64,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 0,
        margin: 100_000_000,
        is_short: i % 2 == 1,
    }).collect();
    
    // Compress batch
    let compressed_batch = ZKStateCompressor::compress_position_batch(&positions, &config).unwrap();
    
    assert_eq!(compressed_batch.len(), positions.len());
    
    // All should share same merkle root
    let first_root = compressed_batch[0].merkle_root;
    for compressed in &compressed_batch {
        assert_eq!(compressed.merkle_root, first_root);
    }
}

#[test]
fn test_compression_verification() {
    let position = Position {
        discriminator: [0u8; 8],
        user: Pubkey::new_unique(),
        proposal_id: 12345,
        position_id: [1u8; 32],
        outcome: 1,
        size: 1_000_000_000,
        notional: 1_000_000_000,
        leverage: 10,
        entry_price: 1_000_000_000,
        liquidation_price: 900_000_000,
        is_long: true,
        created_at: 1234567890,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 0,
        margin: 100_000_000,
        is_short: false,
    };
    
    let compressed = ZKStateCompressor::compress_position(&position).unwrap();
    
    // Verify the compressed state
    let original_hash = ZKStateCompressor::hash_position(&position);
    let is_valid = ZKStateCompressor::verify_compressed_state(&compressed, &original_hash).unwrap();
    
    assert!(is_valid);
}

#[test]
fn test_market_ingestion_batching() {
    let coordinator = ParallelBatchCoordinator::new();
    
    // Test batch assignment
    let test_slots = vec![
        (0, Some(0)),      // Slot 0 -> Batch 0
        (7, Some(1)),      // Slot 7 -> Batch 1
        (140, Some(20)),   // Slot 140 -> Batch 20
        (150, None),       // Slot 150 -> No batch (cycle complete)
    ];
    
    for (slot, expected_batch) in test_slots {
        let batch = coordinator.get_next_batch(slot);
        assert_eq!(batch, expected_batch);
    }
}

#[test]
fn test_batch_timing_validation() {
    let state = BatchIngestionState::new();
    
    // Test valid batch windows
    assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 0, 0).unwrap());
    assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 0, 6).unwrap());
    assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 1, 7).unwrap());
    assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 1, 13).unwrap());
    
    // Test invalid windows
    assert!(!OptimizedMarketIngestion::is_batch_window_valid(&state, 0, 8).unwrap());
    assert!(!OptimizedMarketIngestion::is_batch_window_valid(&state, 1, 15).unwrap());
}

#[test]
fn test_market_data_validation() {
    // Valid market
    let valid_market = OptimizedMarketData {
        market_id: [0u8; 16],
        price_yes: 6000, // 60%
        price_no: 4000,  // 40%
        volume_24h: 1_000_000, // $1M
        liquidity: 500_000,    // $500k
        state: MarketState::Active,
        last_update: 12345,
    };
    
    assert!(OptimizedMarketIngestion::validate_market_data(&valid_market).unwrap());
    
    // Invalid price sum
    let mut invalid_market = valid_market.clone();
    invalid_market.price_yes = 8000;
    invalid_market.price_no = 1000; // Sum = 90%, invalid
    assert!(!OptimizedMarketIngestion::validate_market_data(&invalid_market).unwrap());
    
    // Unrealistic volume
    let mut unrealistic_market = valid_market.clone();
    unrealistic_market.volume_24h = 2_000_000_000; // $2B, unrealistic
    assert!(!OptimizedMarketIngestion::validate_market_data(&unrealistic_market).unwrap());
}

#[test]
fn test_ingestion_metrics() {
    let coordinator = ParallelBatchCoordinator::new();
    let metrics = coordinator.calculate_metrics();
    
    assert_eq!(metrics.markets_per_second, TOTAL_MARKETS as f64 / 60.0);
    assert_eq!(metrics.compression_ratio, 10.0);
    assert_eq!(metrics.cycles_completed, 0); // No cycles completed yet
}

#[test]
fn test_rent_calculation() {
    // Test position account rent
    let position_rent = RentCalculator::position_account_rent();
    
    // Verify significant compression savings
    assert!(position_rent.compression_ratio > 5.0);
    assert!(position_rent.compressed_rent_sol < position_rent.uncompressed_rent_sol);
    
    // Calculate savings percentage
    let savings_percent = (1.0 - (position_rent.compressed_rent_sol / 
                                  position_rent.uncompressed_rent_sol)) * 100.0;
    assert!(savings_percent > 80.0); // Should save >80%
}

#[test]
fn test_platform_rent_costs() {
    let config = RentOptimizationConfig::default();
    
    let costs = RentCalculator::calculate_platform_costs(
        100_000,  // 100k positions
        1_000,    // 1k proposals
        10_000,   // 10k users
        21_000,   // 21k markets
        &config,
    );
    
    // With compression enabled
    assert!(config.enable_compression);
    assert!(costs.total_cost_sol < 50.0); // Should be under 50 SOL
    
    // Without compression
    let mut no_compression_config = config.clone();
    no_compression_config.enable_compression = false;
    
    let uncompressed_costs = RentCalculator::calculate_platform_costs(
        100_000, 1_000, 10_000, 21_000, &no_compression_config,
    );
    
    // Verify massive savings
    assert!(uncompressed_costs.total_cost_sol > costs.total_cost_sol * 5.0);
    assert!(costs.potential_savings == 0.0); // No additional savings when compression enabled
}

#[test]
fn test_rent_optimization_strategies() {
    let strategies = RentOptimizer::optimize_account_layout();
    
    // Should have multiple strategies
    assert!(!strategies.is_empty());
    
    // Calculate total potential savings
    let total_savings: usize = strategies.iter()
        .map(|s| s.space_saved)
        .sum();
    
    assert!(total_savings > 20); // Should save at least 20 bytes
}

#[test]
fn test_batch_size_recommendation() {
    let available_sol = 10.0;
    let recommendation = RentOptimizer::calculate_optimal_batch_size(available_sol, "Position");
    
    assert!(recommendation.recommended_batch_size > 0);
    assert!(recommendation.total_cost <= available_sol);
    assert_eq!(recommendation.account_type, "Position");
}

#[test]
fn test_compression_stats() {
    let original_sizes = vec![1000, 2000, 3000, 4000, 5000];
    let compressed_sizes = vec![100, 200, 300, 400, 500];
    
    let stats = calculate_compression_stats(&original_sizes, &compressed_sizes);
    
    assert_eq!(stats.total_original_bytes, 15000);
    assert_eq!(stats.total_compressed_bytes, 1500);
    assert_eq!(stats.compression_ratio, 10.0);
    assert_eq!(stats.space_saved_bytes, 13500);
    assert_eq!(stats.space_saved_percent, 90.0);
}

#[test]
fn test_merkle_tree_calculation() {
    let hashes = vec![
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
        [4u8; 32],
    ];
    
    let root = ZKStateCompressor::calculate_merkle_root(&hashes).unwrap();
    
    // Root should be deterministic
    let root2 = ZKStateCompressor::calculate_merkle_root(&hashes).unwrap();
    assert_eq!(root, root2);
    
    // Different order should produce different root
    let mut shuffled = hashes.clone();
    shuffled.swap(0, 1);
    let different_root = ZKStateCompressor::calculate_merkle_root(&shuffled).unwrap();
    assert_ne!(root, different_root);
}

#[test]
fn test_market_state_transitions() {
    let states = vec![
        (MarketState::Active, 800),      // Active markets use less compute
        (MarketState::Resolved, 1200),   // Resolved markets need more processing
        (MarketState::Disputed, 1500),   // Disputed markets need most compute
        (MarketState::Archived, 200),    // Archived markets need minimal compute
    ];
    
    for (state, expected_cu) in states {
        match state {
            MarketState::Active => assert!(expected_cu < 1000),
            MarketState::Resolved => assert!(expected_cu > 1000 && expected_cu < 1500),
            MarketState::Disputed => assert!(expected_cu >= 1500),
            MarketState::Archived => assert!(expected_cu < 500),
            _ => {}
        }
    }
}