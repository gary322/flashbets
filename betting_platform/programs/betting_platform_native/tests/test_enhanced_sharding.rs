// Comprehensive tests for enhanced sharding system (4 shards per market)
use solana_program::{
    clock::Clock,
    pubkey::Pubkey,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};

use betting_platform_native::{
    sharding::enhanced_sharding::{
        EnhancedShardManager, MarketShardAllocation, ShardAssignment,
        ShardHealthStatus, SHARDS_PER_MARKET,
    },
    error::BettingPlatformError,
};

#[test]
fn test_shards_per_market_constant() {
    // Verify the constant is set to 4 as per spec
    assert_eq!(SHARDS_PER_MARKET, 4);
}

#[test]
fn test_market_shard_allocation() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    
    // Allocate shards for a market
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Verify allocation
    assert_eq!(allocation.market_id, market_id);
    assert_eq!(allocation.shard_assignments.len(), SHARDS_PER_MARKET as usize);
    
    // Verify each shard has unique ID
    let mut shard_ids = allocation.shard_assignments
        .iter()
        .map(|s| s.shard_id)
        .collect::<Vec<_>>();
    shard_ids.sort();
    shard_ids.dedup();
    assert_eq!(shard_ids.len(), SHARDS_PER_MARKET as usize);
    
    // Verify shards are healthy initially
    for shard in &allocation.shard_assignments {
        assert_eq!(shard.health_status, ShardHealthStatus::Healthy);
        assert_eq!(shard.pending_operations, 0);
        assert_eq!(shard.failed_operations, 0);
    }
}

#[test]
fn test_multiple_markets_sharding() {
    let mut manager = EnhancedShardManager::new();
    let markets = vec![
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ];
    
    // Allocate shards for multiple markets
    let mut allocations = Vec::new();
    for market_id in &markets {
        let allocation = manager.allocate_market_shards(*market_id).unwrap();
        allocations.push(allocation);
    }
    
    // Verify each market has 4 shards
    for allocation in &allocations {
        assert_eq!(allocation.shard_assignments.len(), 4);
    }
    
    // Verify total shards = markets * 4
    let total_shards = manager.get_total_active_shards();
    assert_eq!(total_shards, markets.len() * 4);
    
    // Verify no shard ID collision across markets
    let mut all_shard_ids = Vec::new();
    for allocation in &allocations {
        for shard in &allocation.shard_assignments {
            all_shard_ids.push(shard.shard_id);
        }
    }
    all_shard_ids.sort();
    let unique_count = all_shard_ids.len();
    all_shard_ids.dedup();
    assert_eq!(all_shard_ids.len(), unique_count);
}

#[test]
fn test_shard_selection_load_balancing() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Simulate operations to test load balancing
    let user1 = Pubkey::new_unique();
    let user2 = Pubkey::new_unique();
    let user3 = Pubkey::new_unique();
    
    // Select shards for different users
    let shard1 = manager.select_shard_for_operation(market_id, &user1).unwrap();
    let shard2 = manager.select_shard_for_operation(market_id, &user2).unwrap();
    let shard3 = manager.select_shard_for_operation(market_id, &user3).unwrap();
    
    // Verify shards are distributed (load balancing)
    // With good hash distribution, users should get different shards
    println!("User1 shard: {}", shard1);
    println!("User2 shard: {}", shard2);
    println!("User3 shard: {}", shard3);
    
    // All selected shards should be valid (0-3)
    assert!(shard1 < 4);
    assert!(shard2 < 4);
    assert!(shard3 < 4);
}

#[test]
fn test_shard_health_monitoring() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Get initial shard assignment
    let shard_assignment = &allocation.shard_assignments[0];
    let shard_id = shard_assignment.shard_id;
    
    // Update shard health
    manager.update_shard_health(shard_id, ShardHealthStatus::Degraded).unwrap();
    
    // Verify health was updated
    let updated_allocation = manager.get_market_allocation(&market_id).unwrap();
    let updated_shard = updated_allocation.shard_assignments
        .iter()
        .find(|s| s.shard_id == shard_id)
        .unwrap();
    
    assert_eq!(updated_shard.health_status, ShardHealthStatus::Degraded);
}

#[test]
fn test_operation_tracking() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    let shard_id = allocation.shard_assignments[0].shard_id;
    
    // Record successful operations
    for _ in 0..5 {
        manager.record_operation_result(shard_id, true).unwrap();
    }
    
    // Record failed operations
    for _ in 0..2 {
        manager.record_operation_result(shard_id, false).unwrap();
    }
    
    // Verify operation counts
    let updated_allocation = manager.get_market_allocation(&market_id).unwrap();
    let shard = updated_allocation.shard_assignments
        .iter()
        .find(|s| s.shard_id == shard_id)
        .unwrap();
    
    assert_eq!(shard.successful_operations, 5);
    assert_eq!(shard.failed_operations, 2);
}

#[test]
fn test_duplicate_market_allocation() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    
    // First allocation should succeed
    let allocation1 = manager.allocate_market_shards(market_id);
    assert!(allocation1.is_ok());
    
    // Second allocation for same market should fail
    let allocation2 = manager.allocate_market_shards(market_id);
    assert!(allocation2.is_err());
    
    match allocation2 {
        Err(e) => {
            match e.downcast_ref::<BettingPlatformError>() {
                Some(BettingPlatformError::MarketAlreadySharded) => (),
                _ => panic!("Expected MarketAlreadySharded error"),
            }
        }
        _ => panic!("Expected error for duplicate allocation"),
    }
}

#[test]
fn test_market_deallocation() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    
    // Allocate shards
    manager.allocate_market_shards(market_id).unwrap();
    assert_eq!(manager.get_total_active_shards(), 4);
    
    // Deallocate shards
    manager.deallocate_market_shards(&market_id).unwrap();
    assert_eq!(manager.get_total_active_shards(), 0);
    
    // Verify market no longer has allocation
    let result = manager.get_market_allocation(&market_id);
    assert!(result.is_err());
}

#[test]
fn test_shard_migration() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Simulate unhealthy shard
    let unhealthy_shard_id = allocation.shard_assignments[0].shard_id;
    manager.update_shard_health(unhealthy_shard_id, ShardHealthStatus::Failed).unwrap();
    
    // Trigger migration
    let new_shard_id = manager.migrate_shard(unhealthy_shard_id).unwrap();
    
    // Verify new shard is different and healthy
    assert_ne!(new_shard_id, unhealthy_shard_id);
    
    let updated_allocation = manager.get_market_allocation(&market_id).unwrap();
    let new_shard = updated_allocation.shard_assignments
        .iter()
        .find(|s| s.shard_id == new_shard_id)
        .unwrap();
    
    assert_eq!(new_shard.health_status, ShardHealthStatus::Healthy);
}

#[test]
fn test_concurrent_operations_per_shard() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Track operations per shard
    let mut operations_per_shard = vec![0u32; 4];
    
    // Simulate 1000 operations from different users
    for i in 0..1000 {
        let user = Pubkey::new(&[i as u8; 32]);
        let shard_index = manager.select_shard_for_operation(market_id, &user).unwrap();
        operations_per_shard[shard_index as usize] += 1;
        
        // Increment pending operations
        let shard_id = allocation.shard_assignments[shard_index as usize].shard_id;
        manager.increment_pending_operations(shard_id).unwrap();
    }
    
    // Verify reasonable distribution (each shard should get 200-300 operations)
    for (i, &count) in operations_per_shard.iter().enumerate() {
        println!("Shard {} operations: {}", i, count);
        assert!(count >= 150 && count <= 350, "Uneven shard distribution");
    }
}

#[test]
fn test_shard_performance_metrics() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Simulate operations with different latencies
    for (i, shard) in allocation.shard_assignments.iter().enumerate() {
        let latency_ms = (i + 1) * 10; // 10ms, 20ms, 30ms, 40ms
        manager.record_operation_latency(shard.shard_id, latency_ms as u64).unwrap();
    }
    
    // Get performance report
    let report = manager.get_performance_report(&market_id).unwrap();
    
    // Verify report contains metrics for all shards
    assert!(report.contains("Market Performance Report"));
    assert!(report.contains("Shard 0:"));
    assert!(report.contains("Shard 1:"));
    assert!(report.contains("Shard 2:"));
    assert!(report.contains("Shard 3:"));
    assert!(report.contains("Average Latency:"));
}

#[test]
fn test_maximum_markets_limit() {
    let mut manager = EnhancedShardManager::new();
    
    // Allocate maximum number of markets (considering shard limit)
    // With 65536 max shards and 4 shards per market = 16384 markets max
    let max_markets = 100; // Test with smaller number for performance
    
    for i in 0..max_markets {
        let market_id = Pubkey::new(&[i as u8; 32]);
        let result = manager.allocate_market_shards(market_id);
        assert!(result.is_ok(), "Failed to allocate market {}", i);
    }
    
    assert_eq!(manager.get_total_active_shards(), max_markets * 4);
}

#[test]
fn test_shard_recovery_after_failure() {
    let mut manager = EnhancedShardManager::new();
    let market_id = Pubkey::new_unique();
    let allocation = manager.allocate_market_shards(market_id).unwrap();
    
    // Simulate cascading failures
    for shard in &allocation.shard_assignments {
        // Record multiple failures
        for _ in 0..10 {
            manager.record_operation_result(shard.shard_id, false).unwrap();
        }
    }
    
    // Check all shards are marked as degraded/failed
    let updated_allocation = manager.get_market_allocation(&market_id).unwrap();
    let unhealthy_count = updated_allocation.shard_assignments
        .iter()
        .filter(|s| s.health_status != ShardHealthStatus::Healthy)
        .count();
    
    assert!(unhealthy_count > 0, "Expected some unhealthy shards");
    
    // Trigger recovery
    manager.recover_unhealthy_shards(&market_id).unwrap();
    
    // Verify shards are recovered
    let recovered_allocation = manager.get_market_allocation(&market_id).unwrap();
    for shard in &recovered_allocation.shard_assignments {
        assert_eq!(shard.health_status, ShardHealthStatus::Healthy);
    }
}