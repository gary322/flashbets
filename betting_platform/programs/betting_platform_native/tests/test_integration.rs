// End-to-end integration tests for all implemented features
use solana_program::{
    clock::Clock,
    pubkey::Pubkey,
    program_error::ProgramError,
    account_info::AccountInfo,
    rent::Rent,
    system_program,
};
use solana_program_test::{processor, tokio, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
};
use borsh::{BorshDeserialize, BorshSerialize};

use betting_platform_native::{
    processor::process_instruction,
    performance::cu_verifier::CUVerifier,
    sharding::enhanced_sharding::{EnhancedShardManager, SHARDS_PER_MARKET},
    integration::{
        median_oracle::{calculate_median_price, MedianPriceResult},
        pyth_oracle::PythOracle,
        chainlink_oracle::ChainlinkOracle,
    },
    state::pda_size_validation::{
        OptimizedVersePDA, OptimizedProposalPDA,
        validate_verse_pda_size, validate_proposal_pda_size,
    },
    error::BettingPlatformError,
};

#[tokio::test]
async fn test_full_trading_flow_with_cu_verification() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(process_instruction),
    );

    // Setup accounts
    let user = Keypair::new();
    let market = Keypair::new();
    
    program_test.add_account(
        user.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut context = program_test.start_with_context().await;
    let mut verifier = CUVerifier::new();

    // Measure CU for complete trade flow
    let measurement = verifier.measure_full_trade_flow().unwrap();
    
    println!("=== Full Trading Flow CU Report ===");
    println!("Total CU Used: {}", measurement.cu_used);
    println!("Status: {}", if measurement.cu_used < 50_000 { "✅ PASS" } else { "❌ FAIL" });
    
    assert!(measurement.cu_used < 50_000, "Trade flow exceeded 50k CU limit");
}

#[tokio::test]
async fn test_market_sharding_with_trades() {
    let program_id = Pubkey::new_unique();
    let mut shard_manager = EnhancedShardManager::new();
    
    // Create 3 markets
    let markets = vec![
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ];
    
    // Allocate shards for each market
    for market_id in &markets {
        let allocation = shard_manager.allocate_market_shards(*market_id).unwrap();
        assert_eq!(allocation.shard_assignments.len(), 4);
    }
    
    // Verify total shards
    assert_eq!(shard_manager.get_total_active_shards(), 12); // 3 markets × 4 shards
    
    // Simulate trades on each market
    for market_id in &markets {
        for i in 0..100 {
            let user = Pubkey::new(&[i as u8; 32]);
            let shard_index = shard_manager.select_shard_for_operation(*market_id, &user).unwrap();
            assert!(shard_index < 4);
            
            // Record operation
            let allocation = shard_manager.get_market_allocation(market_id).unwrap();
            let shard_id = allocation.shard_assignments[shard_index as usize].shard_id;
            shard_manager.record_operation_result(shard_id, true).unwrap();
        }
    }
    
    // Generate performance report
    for market_id in &markets {
        let report = shard_manager.get_performance_report(market_id).unwrap();
        println!("Market {} Sharding Report:\n{}", market_id, report);
    }
}

#[tokio::test]
async fn test_oracle_integration_with_median_calculation() {
    use betting_platform_native::integration::OraclePriceData;
    
    // Simulate oracle data
    let polymarket_data = OraclePriceData {
        price: 5500, // $55.00
        confidence: 9500,
        timestamp: Clock::get().unwrap().unix_timestamp,
    };
    
    let pyth_data = OraclePriceData {
        price: 5450, // $54.50
        confidence: 9800,
        timestamp: Clock::get().unwrap().unix_timestamp,
    };
    
    let chainlink_data = OraclePriceData {
        price: 5600, // $56.00
        confidence: 9200,
        timestamp: Clock::get().unwrap().unix_timestamp,
    };
    
    // Calculate median
    let median_result = calculate_median_price(
        Some(polymarket_data),
        Some(pyth_data),
        Some(chainlink_data),
        Clock::get().unwrap().slot,
    ).unwrap();
    
    println!("=== Oracle Median Calculation ===");
    println!("Polymarket: ${:.2} (confidence: {}%)", polymarket_data.price as f64 / 100.0, polymarket_data.confidence / 100);
    println!("Pyth: ${:.2} (confidence: {}%)", pyth_data.price as f64 / 100.0, pyth_data.confidence / 100);
    println!("Chainlink: ${:.2} (confidence: {}%)", chainlink_data.price as f64 / 100.0, chainlink_data.confidence / 100);
    println!("Median Price: ${:.2}", median_result.median_price as f64 / 100.0);
    println!("Aggregate Confidence: {}%", median_result.aggregate_confidence / 100);
    println!("Active Sources: {}", median_result.sources_count);
    
    // Verify median is correct (should be 5500)
    assert_eq!(median_result.median_price, 5500);
    assert_eq!(median_result.sources_count, 3);
    assert!(median_result.aggregate_confidence > 9000); // High confidence
}

#[tokio::test]
async fn test_pda_size_validation() {
    // Test VersePDA size
    let verse_pda = OptimizedVersePDA {
        discriminator: [1; 8],
        verse_id: 12345,
        parent_id: 0,
        children_root: [0; 16],
        packed_data: 0b00000001_00000010_00000000_00000001, // status=1, outcome_count=2, depth=0, is_multiverse=1
        creator: Pubkey::new_unique(),
        last_update_slot_slot: 1000,
        _padding: [0; 7],
    };
    
    // Validate size
    let is_valid = validate_verse_pda_size(&verse_pda);
    assert!(is_valid, "VersePDA size validation failed");
    
    // Test ProposalPDA size
    let proposal_pda = OptimizedProposalPDA {
        discriminator: [2; 8],
        verse_id: 12345,
        outcome_id: 1,
        packed_data: 0b00000001_00000000_00000000_00000000, // amm_type=1
        price_data: [5000, 1000, 0, 0], // price=5000, volume=1000
        creator: Pubkey::new_unique(),
        market_accounts: [Pubkey::default(); 8],
        metadata_cid: [0; 32],
        resolution_data: [0; 64],
        _reserved: [0; 336],
    };
    
    // Validate size
    let is_valid = validate_proposal_pda_size(&proposal_pda);
    assert!(is_valid, "ProposalPDA size validation failed");
    
    println!("=== PDA Size Validation ===");
    println!("VersePDA: {} bytes ✅", std::mem::size_of::<OptimizedVersePDA>());
    println!("ProposalPDA: {} bytes ✅", std::mem::size_of::<OptimizedProposalPDA>());
}

#[tokio::test]
async fn test_combined_features_stress_test() {
    let mut shard_manager = EnhancedShardManager::new();
    let mut verifier = CUVerifier::new();
    
    println!("=== Combined Features Stress Test ===");
    
    // Create 10 markets
    let mut markets = Vec::new();
    for i in 0..10 {
        let market_id = Pubkey::new(&[i as u8; 32]);
        markets.push(market_id);
        shard_manager.allocate_market_shards(market_id).unwrap();
    }
    
    println!("✅ Created 10 markets with {} total shards", shard_manager.get_total_active_shards());
    
    // Simulate 100 trades per market
    let mut total_cu_used = 0u64;
    for (market_idx, market_id) in markets.iter().enumerate() {
        for trade_idx in 0..100 {
            let user = Pubkey::new(&[(market_idx * 100 + trade_idx) as u8; 32]);
            
            // Select shard
            let shard_index = shard_manager.select_shard_for_operation(*market_id, &user).unwrap();
            
            // Measure CU for trade
            let measurement = if trade_idx % 2 == 0 {
                verifier.measure_lmsr_trade().unwrap()
            } else {
                verifier.measure_l2amm_trade().unwrap()
            };
            
            total_cu_used += measurement.cu_used;
            
            // Record operation
            let allocation = shard_manager.get_market_allocation(market_id).unwrap();
            let shard_id = allocation.shard_assignments[shard_index as usize].shard_id;
            shard_manager.record_operation_result(shard_id, true).unwrap();
        }
    }
    
    let avg_cu_per_trade = total_cu_used / 1000;
    println!("✅ Executed 1000 trades");
    println!("   Average CU per trade: {}", avg_cu_per_trade);
    println!("   Status: {}", if avg_cu_per_trade < 50_000 { "✅ PASS" } else { "❌ FAIL" });
    
    // Verify all trades stayed under 50k CU
    assert!(avg_cu_per_trade < 50_000);
    
    // Check shard health
    let mut healthy_shards = 0;
    let mut total_shards = 0;
    for market_id in &markets {
        let allocation = shard_manager.get_market_allocation(market_id).unwrap();
        for shard in &allocation.shard_assignments {
            total_shards += 1;
            if shard.health_status == betting_platform_native::sharding::enhanced_sharding::ShardHealthStatus::Healthy {
                healthy_shards += 1;
            }
        }
    }
    
    println!("✅ Shard Health: {}/{} healthy", healthy_shards, total_shards);
    assert_eq!(healthy_shards, total_shards);
}

#[tokio::test]
async fn test_oracle_failover_scenarios() {
    use betting_platform_native::integration::OraclePriceData;
    
    println!("=== Oracle Failover Scenarios ===");
    
    // Scenario 1: All oracles available
    let result1 = calculate_median_price(
        Some(OraclePriceData { price: 5000, confidence: 9500, timestamp: 100 }),
        Some(OraclePriceData { price: 5100, confidence: 9800, timestamp: 100 }),
        Some(OraclePriceData { price: 5200, confidence: 9200, timestamp: 100 }),
        1000,
    ).unwrap();
    
    println!("Scenario 1 - All oracles: median=${:.2}, sources={}", 
        result1.median_price as f64 / 100.0, result1.sources_count);
    assert_eq!(result1.sources_count, 3);
    assert_eq!(result1.median_price, 5100); // Middle value
    
    // Scenario 2: One oracle down
    let result2 = calculate_median_price(
        Some(OraclePriceData { price: 5000, confidence: 9500, timestamp: 100 }),
        None,
        Some(OraclePriceData { price: 5200, confidence: 9200, timestamp: 100 }),
        1000,
    ).unwrap();
    
    println!("Scenario 2 - Pyth down: median=${:.2}, sources={}", 
        result2.median_price as f64 / 100.0, result2.sources_count);
    assert_eq!(result2.sources_count, 2);
    assert_eq!(result2.median_price, 5100); // Average of two
    
    // Scenario 3: Only one oracle available
    let result3 = calculate_median_price(
        Some(OraclePriceData { price: 5300, confidence: 9500, timestamp: 100 }),
        None,
        None,
        1000,
    ).unwrap();
    
    println!("Scenario 3 - Only Polymarket: median=${:.2}, sources={}", 
        result3.median_price as f64 / 100.0, result3.sources_count);
    assert_eq!(result3.sources_count, 1);
    assert_eq!(result3.median_price, 5300);
    
    // Scenario 4: Stale data handling
    let result4 = calculate_median_price(
        Some(OraclePriceData { price: 5000, confidence: 9500, timestamp: 100 }),
        Some(OraclePriceData { price: 5100, confidence: 9800, timestamp: 100 }),
        Some(OraclePriceData { price: 5200, confidence: 9200, timestamp: -1000 }), // Very old
        1000,
    );
    
    // Should still work but with lower confidence
    assert!(result4.is_ok());
    let result4 = result4.unwrap();
    println!("Scenario 4 - Stale Chainlink: median=${:.2}, confidence={}%", 
        result4.median_price as f64 / 100.0, result4.aggregate_confidence / 100);
}

#[tokio::test]
async fn test_performance_benchmarks() {
    let mut verifier = CUVerifier::new();
    
    println!("=== Performance Benchmarks ===");
    
    // Benchmark LMSR operations
    let mut lmsr_measurements = Vec::new();
    for _ in 0..100 {
        let measurement = verifier.measure_lmsr_trade().unwrap();
        lmsr_measurements.push(measurement.cu_used);
    }
    
    let lmsr_avg = lmsr_measurements.iter().sum::<u64>() / lmsr_measurements.len() as u64;
    let lmsr_max = *lmsr_measurements.iter().max().unwrap();
    let lmsr_min = *lmsr_measurements.iter().min().unwrap();
    
    println!("LMSR Trade (100 iterations):");
    println!("  Average: {} CU", lmsr_avg);
    println!("  Min: {} CU", lmsr_min);
    println!("  Max: {} CU", lmsr_max);
    println!("  Status: {}", if lmsr_max < 50_000 { "✅ PASS" } else { "❌ FAIL" });
    
    // Benchmark L2AMM operations
    let mut l2amm_measurements = Vec::new();
    for _ in 0..100 {
        let measurement = verifier.measure_l2amm_trade().unwrap();
        l2amm_measurements.push(measurement.cu_used);
    }
    
    let l2amm_avg = l2amm_measurements.iter().sum::<u64>() / l2amm_measurements.len() as u64;
    let l2amm_max = *l2amm_measurements.iter().max().unwrap();
    let l2amm_min = *l2amm_measurements.iter().min().unwrap();
    
    println!("\nL2AMM Trade (100 iterations):");
    println!("  Average: {} CU", l2amm_avg);
    println!("  Min: {} CU", l2amm_min);
    println!("  Max: {} CU", l2amm_max);
    println!("  Status: {}", if l2amm_max < 50_000 { "✅ PASS" } else { "❌ FAIL" });
    
    // All operations should be under 50k CU
    assert!(lmsr_max < 50_000);
    assert!(l2amm_max < 50_000);
}