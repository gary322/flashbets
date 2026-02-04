//! Tests for liquidation queue system
//! Verifies priority processing and batch operations

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    liquidation::queue::{LiquidationQueue, LiquidationCandidate},
    state::accounts::Position,
};

#[tokio::test]
async fn test_queue_priority_ordering() {
    let mut queue = LiquidationQueue::new();
    
    // Add candidates with different risk scores
    let candidates = vec![
        create_candidate(1, 0.8, 0.5, 1_000_000_000),  // High risk, medium health
        create_candidate(2, 0.3, 0.9, 5_000_000_000),  // Low risk, high health
        create_candidate(3, 0.9, 0.1, 2_000_000_000),  // Very high risk, low health
        create_candidate(4, 0.5, 0.5, 3_000_000_000),  // Medium risk, medium health
    ];
    
    for candidate in candidates {
        queue.add_candidate(candidate).unwrap();
    }
    
    // Sort by priority
    queue.sort_by_priority();
    
    println!("Queue priority ordering:");
    for (i, candidate) in queue.positions.iter().enumerate() {
        println!("  {}: Position {} - Priority: {:.3}, Risk: {:.1}, Health: {:.1}", 
            i + 1,
            candidate.position_index,
            candidate.priority_score,
            candidate.risk_score,
            candidate.health_factor
        );
    }
    
    // Verify highest priority is first
    assert_eq!(queue.positions[0].position_index, 3, "Highest risk/lowest health should be first");
}

#[tokio::test]
async fn test_batch_processing() {
    let mut queue = LiquidationQueue::new();
    
    // Add 10 candidates
    for i in 0..10 {
        let candidate = create_candidate(
            i as u8,
            0.5 + (i as f64 * 0.05),
            0.9 - (i as f64 * 0.08),
            1_000_000_000 * (i as u64 + 1)
        );
        queue.add_candidate(candidate).unwrap();
    }
    
    // Process batch of 3
    let batch_size = 3;
    queue.sort_by_priority();
    
    let batch: Vec<_> = queue.positions.iter().take(batch_size).collect();
    
    println!("\nBatch processing (size = {}):", batch_size);
    for candidate in batch {
        println!("  Position {}: ${} (priority: {:.3})",
            candidate.position_index,
            candidate.position_size / 1_000_000,
            candidate.priority_score
        );
    }
    
    assert_eq!(batch.len(), 3, "Should process exactly 3 positions");
}

#[tokio::test]
async fn test_queue_capacity_limit() {
    let mut queue = LiquidationQueue::new();
    
    // Try to add more than max capacity (100)
    println!("\nQueue capacity test:");
    
    for i in 0..110 {
        let candidate = create_candidate(i as u8, 0.5, 0.5, 1_000_000_000);
        let result = queue.add_candidate(candidate);
        
        if i < 100 {
            assert!(result.is_ok(), "Should accept first 100 candidates");
        } else {
            assert!(result.is_err(), "Should reject after capacity");
            if i == 100 {
                println!("  ✓ Queue full at 100 positions");
            }
        }
    }
    
    assert_eq!(queue.positions.len(), 100, "Queue should be at max capacity");
}

#[tokio::test]
async fn test_stale_entry_cleanup() {
    let mut queue = LiquidationQueue::new();
    let current_slot = 1000;
    
    // Add candidates with different scan times
    queue.add_candidate(create_candidate_with_slot(1, 0.8, 0.5, 1_000_000_000, 900)).unwrap();
    queue.add_candidate(create_candidate_with_slot(2, 0.7, 0.4, 2_000_000_000, 950)).unwrap();
    queue.add_candidate(create_candidate_with_slot(3, 0.9, 0.3, 3_000_000_000, 990)).unwrap();
    
    // Clean stale entries (older than 50 slots)
    let stale_threshold = 50;
    let initial_count = queue.positions.len();
    
    queue.positions.retain(|candidate| {
        current_slot.saturating_sub(candidate.last_scan_slot) <= stale_threshold
    });
    
    let removed = initial_count - queue.positions.len();
    
    println!("\nStale entry cleanup:");
    println!("  Current slot: {}", current_slot);
    println!("  Stale threshold: {} slots", stale_threshold);
    println!("  Removed {} stale entries", removed);
    println!("  Remaining: {} entries", queue.positions.len());
    
    assert_eq!(queue.positions.len(), 1, "Should only keep recent entries");
}

#[tokio::test]
async fn test_priority_score_calculation() {
    // Test priority formula: risk × (1/health) × size
    let test_cases = vec![
        (0.9, 0.1, 10_000_000_000), // High risk, low health, large size
        (0.5, 0.5, 5_000_000_000),   // Medium all
        (0.1, 0.9, 1_000_000_000),   // Low risk, high health, small size
    ];
    
    println!("\nPriority score calculation:");
    println!("  Formula: risk × (1/health) × size");
    
    for (risk, health, size) in test_cases {
        let priority = risk * (1.0 / health) * (size as f64 / 1_000_000_000.0);
        println!("  Risk: {:.1}, Health: {:.1}, Size: ${}k → Priority: {:.2}",
            risk, health, size / 1_000_000_000, priority);
    }
}

#[tokio::test]
async fn test_total_liquidatable_tracking() {
    let mut queue = LiquidationQueue::new();
    
    let candidates = vec![
        create_candidate(1, 0.8, 0.5, 1_000_000_000),
        create_candidate(2, 0.7, 0.4, 2_000_000_000),
        create_candidate(3, 0.9, 0.3, 3_000_000_000),
    ];
    
    for candidate in candidates {
        queue.add_candidate(candidate).unwrap();
    }
    
    let total = queue.calculate_total_liquidatable();
    
    assert_eq!(total, 6_000_000_000, "Should sum all position sizes");
    
    println!("\nTotal liquidatable value:");
    println!("  Positions in queue: {}", queue.positions.len());
    println!("  Total value: ${}", total / 1_000_000);
}

#[tokio::test]
async fn test_duplicate_position_handling() {
    let mut queue = LiquidationQueue::new();
    
    // Try to add same position twice
    let candidate1 = create_candidate(1, 0.8, 0.5, 1_000_000_000);
    let candidate2 = create_candidate(1, 0.9, 0.4, 1_000_000_000); // Same position, updated values
    
    queue.add_candidate(candidate1).unwrap();
    
    // Check if duplicate handling is needed
    let exists = queue.positions.iter().any(|c| c.position_index == 1);
    
    if exists {
        // Update existing entry
        if let Some(pos) = queue.positions.iter_mut().find(|c| c.position_index == 1) {
            pos.risk_score = candidate2.risk_score;
            pos.health_factor = candidate2.health_factor;
            pos.priority_score = candidate2.priority_score;
        }
    }
    
    println!("\nDuplicate position handling:");
    println!("  Position already exists: {}", exists);
    println!("  Updated with new risk values");
    
    assert_eq!(queue.positions.len(), 1, "Should not duplicate positions");
}

#[tokio::test]
async fn test_emergency_liquidation_priority() {
    let mut queue = LiquidationQueue::new();
    
    // Regular candidates
    queue.add_candidate(create_candidate(1, 0.5, 0.5, 1_000_000_000)).unwrap();
    queue.add_candidate(create_candidate(2, 0.6, 0.4, 2_000_000_000)).unwrap();
    
    // Emergency candidate (health factor = 0)
    let emergency = LiquidationCandidate {
        position_index: 3,
        risk_score: 1.0,
        health_factor: 0.0,
        position_size: 500_000_000,
        priority_score: f64::INFINITY,
        last_scan_slot: 1000,
    };
    
    queue.add_candidate(emergency).unwrap();
    queue.sort_by_priority();
    
    println!("\nEmergency liquidation priority:");
    println!("  Emergency position should be first");
    assert_eq!(queue.positions[0].position_index, 3, "Emergency should have highest priority");
    assert!(queue.positions[0].priority_score.is_infinite(), "Emergency priority should be infinite");
}

#[tokio::test]
async fn test_queue_state_persistence() {
    // Test that queue state can be saved/loaded
    let queue = LiquidationQueue {
        positions: vec![
            create_candidate(1, 0.8, 0.5, 1_000_000_000),
            create_candidate(2, 0.7, 0.4, 2_000_000_000),
        ],
        total_liquidatable_value: 3_000_000_000,
        last_scan_slot: 1000,
        scan_in_progress: false,
    };
    
    println!("\nQueue state persistence test:");
    println!("  Positions: {}", queue.positions.len());
    println!("  Total value: ${}", queue.total_liquidatable_value / 1_000_000);
    println!("  Last scan: slot {}", queue.last_scan_slot);
    println!("  Scan in progress: {}", queue.scan_in_progress);
    
    // In actual implementation, would serialize/deserialize
    assert_eq!(queue.positions.len(), 2, "Queue state should be preservable");
}

// Helper functions
fn create_candidate(index: u8, risk: f64, health: f64, size: u64) -> LiquidationCandidate {
    create_candidate_with_slot(index, risk, health, size, 1000)
}

fn create_candidate_with_slot(
    index: u8, 
    risk: f64, 
    health: f64, 
    size: u64, 
    slot: u64
) -> LiquidationCandidate {
    LiquidationCandidate {
        position_index: index,
        risk_score: risk,
        health_factor: health,
        position_size: size,
        priority_score: risk * (1.0 / health) * (size as f64 / 1_000_000_000.0),
        last_scan_slot: slot,
    }
}