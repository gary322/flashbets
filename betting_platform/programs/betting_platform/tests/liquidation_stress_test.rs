use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform_native::{
    liquidation::high_performance_engine::*,
    state::accounts::{PositionPDA, UserStatsPDA},
    math::U64F64,
};
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_4k_liquidations_per_second() {
    println!("=== 4k Liquidations/Second Stress Test ===");
    
    let mut processor = LiquidationProcessor::new();
    let start = Instant::now();
    
    // Populate queue with test positions
    let positions_to_add = 20_000; // Add more than we can process
    println!("Adding {} test positions to liquidation queue...", positions_to_add);
    
    for i in 0..positions_to_add {
        let position = create_test_position(i, calculate_test_health_ratio(i));
        let mark_price = 50_000_000 + (i as u64 * 1000); // Vary prices
        
        if let Err(e) = processor.add_to_queue(&position, mark_price) {
            if i < 10_000 {
                panic!("Failed to add position {}: {:?}", i, e);
            }
            // Queue full is expected after MAX_QUEUE_SIZE
        }
    }
    
    println!("Queue populated with {} positions", processor.queue.heap.len());
    
    // Simulate processing for multiple slots
    let test_slots = 10; // Test 10 slots = 4 seconds
    let mut total_processed = 0u32;
    let mut total_failed = 0u32;
    
    for slot in 0..test_slots {
        let slot_start = Instant::now();
        
        // Process liquidations for this slot
        let result = processor.process_slot(slot).unwrap();
        
        total_processed += result.processed;
        total_failed += result.failed;
        
        let slot_duration = slot_start.elapsed();
        
        println!(
            "Slot {}: Processed {} liquidations in {:?} ({} failed, {} capacity remaining)",
            slot,
            result.processed,
            slot_duration,
            result.failed,
            result.remaining_capacity
        );
        
        // Verify we're meeting performance targets
        assert!(
            result.processed + result.failed <= LIQUIDATIONS_PER_SLOT,
            "Exceeded slot capacity"
        );
        
        // Simulate slot time (0.4s)
        if slot_duration < Duration::from_millis(400) {
            tokio::time::sleep(Duration::from_millis(400) - slot_duration).await;
        }
    }
    
    let total_duration = start.elapsed();
    let liquidations_per_second = (total_processed as f64) / total_duration.as_secs_f64();
    
    println!("\n=== Results ===");
    println!("Total processed: {}", total_processed);
    println!("Total failed: {}", total_failed);
    println!("Total duration: {:?}", total_duration);
    println!("Liquidations per second: {:.0}", liquidations_per_second);
    
    // Get final stats
    let stats = processor.get_stats();
    println!("\n=== Performance Stats ===");
    println!("Total liquidations: {}", stats.total_liquidations);
    println!("Queue size: {}", stats.current_queue_size);
    println!("Avg processing time: {}ms", stats.avg_processing_time_ms);
    println!("Thread utilization: {:.1}%", stats.thread_utilization);
    println!("Success rate: {:.1}%", stats.success_rate);
    
    // Verify we meet the 4k/sec target
    assert!(
        liquidations_per_second >= 3800.0, // Allow 5% margin
        "Failed to meet 4k liquidations/sec target: {:.0}",
        liquidations_per_second
    );
}

#[test]
fn test_parallel_batch_processing() {
    println!("=== Parallel Batch Processing Test ===");
    
    let processor = LiquidationProcessor::new();
    
    // Verify thread configuration
    assert_eq!(
        processor.engine.thread_states.len(),
        PARALLEL_LIQUIDATION_THREADS
    );
    
    // Verify batch sizes
    let total_per_slot = PARALLEL_LIQUIDATION_THREADS * BATCH_SIZE_PER_THREAD;
    assert_eq!(
        total_per_slot,
        LIQUIDATIONS_PER_SLOT as usize,
        "Batch configuration mismatch"
    );
    
    println!("Threads: {}", PARALLEL_LIQUIDATION_THREADS);
    println!("Batch size per thread: {}", BATCH_SIZE_PER_THREAD);
    println!("Total per slot: {}", total_per_slot);
}

#[test]
fn test_priority_queue_ordering() {
    println!("=== Priority Queue Ordering Test ===");
    
    let mut queue = LiquidationQueue::new(100);
    
    // Add candidates with different priorities
    let candidates = vec![
        create_test_candidate(1, 0.5, 1000), // Low health, high priority
        create_test_candidate(2, 0.8, 500),  // Medium health
        create_test_candidate(3, 0.3, 2000), // Very low health, highest priority
        create_test_candidate(4, 0.9, 100),  // High health, low priority
    ];
    
    for candidate in candidates {
        queue.add_candidate(candidate).unwrap();
    }
    
    // Get batch and verify ordering
    let batch = queue.get_next_batch(4);
    
    // Should be ordered by priority (lowest health first)
    assert_eq!(batch[0].position_id, Pubkey::new_unique()); // ID would be different
    assert!(batch[0].health_ratio < batch[1].health_ratio);
    assert!(batch[1].health_ratio < batch[2].health_ratio);
    
    println!("Priority ordering verified");
}

#[test]
fn test_health_ratio_calculation() {
    println!("=== Health Ratio Calculation Test ===");
    
    // Test long position
    let mut position = create_test_position(1, 1.0);
    position.is_long = true;
    position.entry_price = 50_000_000; // $50
    position.leverage = 10;
    
    // Price drops to $45 (10% drop)
    let mark_price = 45_000_000;
    let health = calculate_health_ratio(&position, mark_price).unwrap();
    
    // With 10x leverage, 10% drop = 100% loss = 0 health
    let health_value = health.to_num::<f64>() / 1_000_000.0;
    assert!(
        health_value < 0.1,
        "Health ratio should be near 0 for 100% loss: {}",
        health_value
    );
    
    // Test short position
    position.is_long = false;
    let health = calculate_health_ratio(&position, mark_price).unwrap();
    
    // Short profits from price drop
    let health_value = health.to_num::<f64>() / 1_000_000.0;
    assert!(
        health_value > 1.0,
        "Short position should be healthy when price drops: {}",
        health_value
    );
    
    println!("Health ratio calculations verified");
}

#[tokio::test]
async fn test_concurrent_queue_operations() {
    println!("=== Concurrent Queue Operations Test ===");
    
    use tokio::sync::Arc;
    use tokio::sync::Mutex;
    
    let processor = Arc::new(Mutex::new(LiquidationProcessor::new()));
    let mut handles = vec![];
    
    // Spawn multiple tasks adding to queue
    for thread_id in 0..4 {
        let proc = processor.clone();
        let handle = tokio::spawn(async move {
            let mut added = 0;
            for i in 0..1000 {
                let position = create_test_position(thread_id * 1000 + i, 0.5);
                let mark_price = 50_000_000;
                
                let mut proc_lock = proc.lock().await;
                if proc_lock.add_to_queue(&position, mark_price).is_ok() {
                    added += 1;
                }
            }
            added
        });
        handles.push(handle);
    }
    
    // Wait for all tasks
    let mut total_added = 0;
    for handle in handles {
        total_added += handle.await.unwrap();
    }
    
    println!("Total positions added concurrently: {}", total_added);
    
    // Verify queue state
    let proc_lock = processor.lock().await;
    assert!(proc_lock.queue.heap.len() <= MAX_QUEUE_SIZE);
    println!("Queue size: {}", proc_lock.queue.heap.len());
}

// Helper functions

fn create_test_position(id: u32, health_ratio: f64) -> PositionPDA {
    PositionPDA {
        position_id: Pubkey::new_unique(),
        user: Pubkey::new_unique(),
        market: Pubkey::new_unique(),
        size: 1_000_000_000, // $1000
        collateral: 100_000_000, // $100
        entry_price: 50_000_000, // $50
        liquidation_price: 45_000_000, // $45
        leverage: 10,
        is_long: true,
        created_at: 0,
        last_update: 0,
        stop_loss: None,
        take_profit: None,
        accumulated_funding: 0,
        oracle_price_at_entry: 50_000_000,
        bump: 0,
    }
}

fn create_test_candidate(id: u32, health: f64, size: u64) -> LiquidationCandidate {
    LiquidationCandidate {
        position_id: Pubkey::new_unique(),
        user: Pubkey::new_unique(),
        market_id: Pubkey::new_unique(),
        health_ratio: U64F64::from_num((health * 1_000_000.0) as u64),
        size: size * 1_000_000,
        leverage: 10,
        entry_price: 50_000_000,
        liquidation_price: 45_000_000,
        priority_score: calculate_priority_score(
            U64F64::from_num((health * 1_000_000.0) as u64),
            size * 1_000_000,
            10
        ),
        added_slot: 0,
    }
}

fn calculate_test_health_ratio(index: u32) -> f64 {
    // Generate varied health ratios for testing
    0.2 + (index as f64 % 10) * 0.08
}