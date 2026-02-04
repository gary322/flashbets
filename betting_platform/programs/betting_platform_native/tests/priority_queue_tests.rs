//! Comprehensive tests for Phase 19.5: Priority Queue & Anti-Front-Running

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_program,
};
use solana_program_test::{*};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::collections::{HashMap, HashSet, VecDeque};

use betting_platform_native::{
    priority::{
        queue::{PriorityQueue, QueueEntry, TradeData, EntryStatus, PriorityCalculator},
        anti_mev::{AntiMEVProtection, MEVDetector, MEVProtectionState, RecentTrade},
        processor::{QueueProcessor, CongestionManager, BatchOptimizer},
        fair_ordering::{FairOrderingProtocol, OrderingState, TimeBasedOrdering, FairnessMetrics},
    },
    math::U64F64,
    error::BettingPlatformError,
};

/// Test context for priority queue tests
struct TestContext {
    program_test: ProgramTest,
    program_id: Pubkey,
}

impl TestContext {
    fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(betting_platform_native::process_instruction),
        );
        
        Self {
            program_test,
            program_id,
        }
    }
    
    async fn start(mut self) -> (BanksClient, Keypair, Pubkey) {
        let (banks_client, payer, recent_blockhash) = self.program_test.start().await;
        (banks_client, payer, self.program_id)
    }
}

#[tokio::test]
async fn test_priority_calculation() {
    let calculator = PriorityCalculator::default();
    
    // Test different user profiles
    struct TestCase {
        name: &'static str,
        user_stake: u64,
        verse_depth: u32,
        submission_slot: u64,
        trade_volume: u64,
        current_slot: u64,
        total_stake: u64,
    }
    
    let test_cases = vec![
        TestCase {
            name: "High stake whale",
            user_stake: 1_000_000,
            verse_depth: 5,
            submission_slot: 100,
            trade_volume: 50_000,
            current_slot: 100,
            total_stake: 10_000_000,
        },
        TestCase {
            name: "Medium stake regular",
            user_stake: 10_000,
            verse_depth: 10,
            submission_slot: 95,
            trade_volume: 5_000,
            current_slot: 100,
            total_stake: 10_000_000,
        },
        TestCase {
            name: "Low stake new user",
            user_stake: 100,
            verse_depth: 2,
            submission_slot: 50,
            trade_volume: 1_000,
            current_slot: 100,
            total_stake: 10_000_000,
        },
    ];
    
    let mut priorities = Vec::new();
    
    for case in test_cases {
        let priority = calculator.calculate_priority(
            case.user_stake,
            case.verse_depth,
            case.submission_slot,
            case.trade_volume,
            case.current_slot,
            case.total_stake,
        ).unwrap();
        
        println!("{}: priority score = {}", case.name, priority);
        priorities.push((case.name, priority));
    }
    
    // Verify whale has highest priority due to stake
    assert!(priorities[0].1 > priorities[1].1);
    
    // But not overwhelmingly so (log scale prevents domination)
    assert!(priorities[0].1 < priorities[2].1 * 10);
}

#[tokio::test]
async fn test_anti_mev_sandwich_detection() {
    let mut anti_mev = AntiMEVProtection::default();
    let detector = MEVDetector::default();
    
    // Create MEV state
    let mut mev_state = MEVProtectionState {
        recent_trades: Vec::new(),
        suspicious_patterns: 0,
        last_check_slot: 0,
    };
    
    // Simulate sandwich attack pattern
    let attacker = Pubkey::new_unique();
    let victim = Pubkey::new_unique();
    let market = Pubkey::new_unique();
    
    // Step 1: Attacker front-runs with buy
    mev_state.recent_trades.push(RecentTrade {
        user: attacker,
        synthetic_id: 1,
        is_buy: true,
        amount: 10_000,
        slot: 100,
        price_impact: U64F64::from_num(30_000), // 3% impact
    });
    
    // Step 2: Victim's large buy
    mev_state.recent_trades.push(RecentTrade {
        user: victim,
        synthetic_id: 1,
        is_buy: true,
        amount: 100_000,
        slot: 101,
        price_impact: U64F64::from_num(50_000), // 5% impact
    });
    
    // Step 3: Attacker back-runs with sell
    mev_state.recent_trades.push(RecentTrade {
        user: attacker,
        synthetic_id: 1,
        is_buy: false,
        amount: 10_000,
        slot: 102,
        price_impact: U64F64::from_num(20_000), // 2% impact
    });
    
    // Create victim's new order
    let victim_order = QueueEntry {
        entry_id: 123,
        user: victim,
        priority_score: 1000,
        submission_slot: 103,
        submission_timestamp: 0,
        trade_data: TradeData {
            synthetic_id: 1,
            is_buy: true,
            amount: 50_000,
            leverage: U64F64::from_num(10_000_000),
            max_slippage: U64F64::from_num(20_000),
            stop_loss: None,
            take_profit: None,
        },
        status: EntryStatus::Pending,
        stake_snapshot: 1000,
        depth_boost: 5,
        bump: 0,
    };
    
    // Detect sandwich attack
    let is_sandwich = anti_mev.detect_sandwich_attack(
        &victim_order,
        &mev_state.recent_trades,
        &detector,
    ).unwrap();
    
    assert!(is_sandwich, "Should detect sandwich attack pattern");
}

#[tokio::test]
async fn test_commit_reveal_pattern() {
    let mut anti_mev = AntiMEVProtection::default();
    
    // User commits order hash
    let user = Pubkey::new_unique();
    let order_details = OrderDetails {
        market_id: Pubkey::new_unique(),
        is_buy: true,
        amount: 100_000,
        limit_price: U64F64::from_num(650_000),
    };
    let nonce = 12345u64;
    
    // Compute order hash
    let order_hash = anti_mev.compute_order_hash(&user, &order_details, nonce).unwrap();
    
    // Commit phase
    let commit_slot = 100;
    anti_mev.commit_order(&user, order_hash, commit_slot).unwrap();
    
    // Try to reveal too early (should fail)
    let early_slot = commit_slot + 1;
    let early_result = anti_mev.reveal_order(&user, &order_details, nonce, early_slot);
    assert!(early_result.is_err());
    
    // Reveal after delay (should succeed)
    let reveal_slot = commit_slot + anti_mev.reveal_delay_slots + 1;
    let reveal_result = anti_mev.reveal_order(&user, &order_details, nonce, reveal_slot);
    assert!(reveal_result.is_ok());
}

#[tokio::test]
async fn test_queue_processing_with_mev_protection() {
    let processor = QueueProcessor::default();
    let mut queue = PriorityQueue {
        is_initialized: true,
        queue_id: 1,
        max_size: 1000,
        current_size: 0,
        head_index: 0,
        tail_index: 0,
        total_pending_volume: 0,
        last_process_slot: 0,
        bump: 0,
    };
    
    let mut mev_state = MEVProtectionState {
        recent_trades: Vec::new(),
        suspicious_patterns: 0,
        last_check_slot: 0,
    };
    
    // Create mix of legitimate and suspicious orders
    let mut entries = Vec::new();
    
    // Legitimate high priority order
    entries.push(QueueEntry {
        entry_id: 1,
        user: Pubkey::new_unique(),
        priority_score: 10000,
        submission_slot: 100,
        submission_timestamp: 0,
        trade_data: TradeData {
            synthetic_id: 1,
            is_buy: true,
            amount: 5000,
            leverage: U64F64::from_num(10_000_000),
            max_slippage: U64F64::from_num(20_000),
            stop_loss: None,
            take_profit: None,
        },
        status: EntryStatus::Pending,
        stake_snapshot: 10000,
        depth_boost: 5,
        bump: 0,
    });
    
    // Suspicious order (part of sandwich)
    let suspicious_user = Pubkey::new_unique();
    entries.push(QueueEntry {
        entry_id: 2,
        user: suspicious_user,
        priority_score: 8000,
        submission_slot: 102,
        submission_timestamp: 0,
        trade_data: TradeData {
            synthetic_id: 1,
            is_buy: false,
            amount: 10000,
            leverage: U64F64::from_num(20_000_000),
            max_slippage: U64F64::from_num(50_000),
            stop_loss: None,
            take_profit: None,
        },
        status: EntryStatus::Pending,
        stake_snapshot: 5000,
        depth_boost: 3,
        bump: 0,
    });
    
    // Add suspicious pattern to MEV state
    mev_state.recent_trades.push(RecentTrade {
        user: suspicious_user,
        synthetic_id: 1,
        is_buy: true,
        amount: 10000,
        slot: 101,
        price_impact: U64F64::from_num(40_000), // 4% impact
    });
    
    // Process queue
    let result = processor.process_queue(
        &mut queue,
        &mut entries,
        &mut mev_state,
    ).unwrap();
    
    // Should process legitimate order
    assert!(result.processed_count >= 1);
    
    // Check if suspicious order was cancelled
    let suspicious_entry = entries.iter()
        .find(|e| e.entry_id == 2)
        .unwrap();
    
    // May be cancelled if detected as part of sandwich
    if suspicious_entry.status == EntryStatus::Cancelled {
        println!("Suspicious order cancelled due to MEV detection");
    }
}

#[tokio::test]
async fn test_congestion_management() {
    let mut congestion_manager = CongestionManager::default();
    let mut queue = PriorityQueue {
        is_initialized: true,
        queue_id: 1,
        max_size: 1000,
        current_size: 100,
        head_index: 0,
        tail_index: 100,
        total_pending_volume: 1_000_000,
        last_process_slot: 0,
        bump: 0,
    };
    
    // Create diverse set of entries
    let mut entries = Vec::new();
    let mut users = HashSet::new();
    
    // High stake users
    for i in 0..20 {
        let user = Pubkey::new_unique();
        users.insert(user);
        
        entries.push(QueueEntry {
            entry_id: i,
            user,
            priority_score: 10000 - i * 100, // Decreasing priority
            submission_slot: 100 + i,
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: i % 2 == 0,
                amount: 10000,
                leverage: U64F64::from_num(10_000_000),
                max_slippage: U64F64::from_num(20_000),
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 100000 - i * 1000,
            depth_boost: 5,
            bump: 0,
        });
    }
    
    // Low stake users
    for i in 0..30 {
        let user = Pubkey::new_unique();
        users.insert(user);
        
        entries.push(QueueEntry {
            entry_id: 100 + i,
            user,
            priority_score: 1000 - i * 10,
            submission_slot: 90 + i, // Some submitted earlier
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: i % 2 == 1,
                amount: 1000,
                leverage: U64F64::from_num(5_000_000),
                max_slippage: U64F64::from_num(30_000),
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 100,
            depth_boost: 2,
            bump: 0,
        });
    }
    
    // Process congested batch
    let max_batch_size = 10;
    let selected = congestion_manager.process_congested_batch(
        &mut queue,
        &mut entries,
        max_batch_size,
        200, // current slot
    ).unwrap();
    
    // Should select exactly max_batch_size orders
    assert_eq!(selected.len(), max_batch_size as usize);
    
    // Verify fairness: not all from high priority
    let high_priority_count = selected.iter()
        .filter(|e| e.priority_score > 5000)
        .count();
    
    let low_priority_count = selected.iter()
        .filter(|e| e.priority_score <= 5000)
        .count();
    
    // Should have mix (70% high, 30% low as per implementation)
    assert!(high_priority_count > low_priority_count);
    assert!(low_priority_count > 0); // Some low priority included
    
    // Verify no duplicate users (fairness)
    let unique_users: HashSet<_> = selected.iter()
        .map(|e| e.user)
        .collect();
    assert_eq!(unique_users.len(), selected.len());
}

#[tokio::test]
async fn test_fair_ordering_with_randomization() {
    let protocol = FairOrderingProtocol::new(5, true);
    
    // Create entries with different priority tiers
    let mut entries = Vec::new();
    
    // Tier 1 (highest priority)
    for i in 0..5 {
        entries.push(QueueEntry {
            entry_id: i,
            user: Pubkey::new_unique(),
            priority_score: u128::MAX / 10 * 9 + i as u128, // All in tier 9
            submission_slot: 100,
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: true,
                amount: 10000,
                leverage: U64F64::from_num(10_000_000),
                max_slippage: U64F64::from_num(20_000),
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 10000,
            depth_boost: 5,
            bump: 0,
        });
    }
    
    // Tier 2 (medium priority)
    for i in 5..10 {
        entries.push(QueueEntry {
            entry_id: i,
            user: Pubkey::new_unique(),
            priority_score: u128::MAX / 10 * 5 + i as u128, // All in tier 5
            submission_slot: 100,
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: false,
                amount: 5000,
                leverage: U64F64::from_num(5_000_000),
                max_slippage: U64F64::from_num(30_000),
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 5000,
            depth_boost: 3,
            bump: 0,
        });
    }
    
    // Create ordering state with randomness
    let ordering_state = OrderingState {
        current_epoch: 1,
        randomness_seed: [42u8; 32], // Deterministic seed for testing
        last_vrf_slot: 0,
        pending_randomness: false,
    };
    
    // Apply fair ordering
    let original_order: Vec<u128> = entries.iter().map(|e| e.entry_id).collect();
    protocol.apply_fair_ordering(&mut entries, &ordering_state).unwrap();
    let new_order: Vec<u128> = entries.iter().map(|e| e.entry_id).collect();
    
    // Verify tier ordering is preserved
    // First 5 should all be from high tier
    for i in 0..5 {
        assert!(entries[i].priority_score > u128::MAX / 10 * 8);
    }
    
    // Next 5 should all be from medium tier
    for i in 5..10 {
        assert!(entries[i].priority_score > u128::MAX / 10 * 4);
        assert!(entries[i].priority_score < u128::MAX / 10 * 6);
    }
    
    // But within tiers, order should be randomized
    println!("Original order: {:?}", original_order);
    println!("New order: {:?}", new_order);
    
    // At least some positions should have changed due to randomization
    let changes = original_order.iter()
        .zip(new_order.iter())
        .filter(|(a, b)| a != b)
        .count();
    
    // With randomization enabled, we expect some changes
    if ordering_state.randomness_seed != [0u8; 32] {
        assert!(changes > 0, "Randomization should change some positions");
    }
}

#[tokio::test]
async fn test_time_based_priority_adjustment() {
    let time_ordering = TimeBasedOrdering::default();
    
    // Test priority boost over time
    let base_priority = 1000u128;
    let submission_slot = 100u64;
    
    // Test at different time intervals
    let test_cases = vec![
        (100, 0),    // No wait time
        (110, 10),   // 10 slots wait
        (150, 50),   // 50 slots wait
        (200, 100),  // 100 slots wait
    ];
    
    for (current_slot, expected_boost_percentage) in test_cases {
        let adjusted = time_ordering.adjust_priority_by_time(
            base_priority,
            submission_slot,
            current_slot,
        );
        
        let boost = adjusted - base_priority;
        let boost_percentage = (boost * 100) / base_priority;
        
        println!(
            "After {} slots: base={}, adjusted={}, boost={}%",
            current_slot - submission_slot,
            base_priority,
            adjusted,
            boost_percentage
        );
        
        // Verify boost is approximately correct (1% per slot)
        assert_eq!(boost_percentage, expected_boost_percentage);
    }
    
    // Test staleness detection
    assert!(!time_ordering.is_stale(100, 150)); // 50 slots - not stale
    assert!(time_ordering.is_stale(100, 250));  // 150 slots - stale
}

#[tokio::test]
async fn test_fairness_metrics() {
    let mut metrics = FairnessMetrics::new();
    
    // Simulate processing batches with different user distributions
    
    // Batch 1: Good diversity
    let batch1 = vec![
        create_test_entry(1, Pubkey::new_unique(), 9000000000000000000000000000000000000, 100),
        create_test_entry(2, Pubkey::new_unique(), 5000000000000000000000000000000000000, 105),
        create_test_entry(3, Pubkey::new_unique(), 1000000000000000000000000000000000000, 95),
    ];
    
    metrics.update_batch_metrics(&batch1, 150);
    
    // Batch 2: Poor diversity (same user multiple times)
    let repeat_user = Pubkey::new_unique();
    let batch2 = vec![
        create_test_entry(4, repeat_user, 9500000000000000000000000000000000000, 120),
        create_test_entry(5, repeat_user, 9400000000000000000000000000000000000, 125),
        create_test_entry(6, Pubkey::new_unique(), 2000000000000000000000000000000000000, 110),
    ];
    
    metrics.update_batch_metrics(&batch2, 200);
    
    // Calculate fairness score
    let fairness_score = metrics.calculate_fairness_score();
    
    println!("Fairness Metrics:");
    println!("  Total processed: {}", metrics.total_orders_processed);
    println!("  Unique users: {}", metrics.unique_users_served);
    println!("  Avg wait time: {} slots", metrics.avg_wait_time_slots);
    println!("  Max wait time: {} slots", metrics.max_wait_time_slots);
    println!("  Fairness score: {}/100", fairness_score);
    
    // Verify metrics
    assert_eq!(metrics.total_orders_processed, 6);
    assert_eq!(metrics.unique_users_served, 4); // One user appeared twice
    assert!(fairness_score > 50); // Should have decent fairness
}

// Helper function to create test entries
fn create_test_entry(id: u128, user: Pubkey, priority: u128, submission_slot: u64) -> QueueEntry {
    QueueEntry {
        entry_id: id,
        user,
        priority_score: priority,
        submission_slot,
        submission_timestamp: 0,
        trade_data: TradeData {
            synthetic_id: 1,
            is_buy: true,
            amount: 1000,
            leverage: U64F64::from_num(10_000_000),
            max_slippage: U64F64::from_num(20_000),
            stop_loss: None,
            take_profit: None,
        },
        status: EntryStatus::Pending,
        stake_snapshot: 1000,
        depth_boost: 5,
        bump: 0,
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_complete_priority_queue_flow() {
        // Initialize components
        let calculator = PriorityCalculator::default();
        let processor = QueueProcessor::default();
        let anti_mev = AntiMEVProtection::default();
        let fair_ordering = FairOrderingProtocol::default();
        
        // Create queue
        let mut queue = PriorityQueue {
            is_initialized: true,
            queue_id: 1,
            max_size: 1000,
            current_size: 0,
            head_index: 0,
            tail_index: 0,
            total_pending_volume: 0,
            last_process_slot: 0,
            bump: 0,
        };
        
        // Create diverse user base
        let whale = Pubkey::new_unique();
        let regular = Pubkey::new_unique();
        let newbie = Pubkey::new_unique();
        
        let mut entries = Vec::new();
        
        // Whale order
        let whale_priority = calculator.calculate_priority(
            1_000_000, // stake
            10,        // depth
            100,       // submission
            100_000,   // volume
            105,       // current
            10_000_000, // total stake
        ).unwrap();
        
        entries.push(QueueEntry {
            entry_id: 1,
            user: whale,
            priority_score: whale_priority,
            submission_slot: 100,
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: true,
                amount: 100_000,
                leverage: U64F64::from_num(50_000_000), // 50x
                max_slippage: U64F64::from_num(10_000), // 1%
                stop_loss: Some(U64F64::from_num(900_000)), // 90%
                take_profit: Some(U64F64::from_num(1_100_000)), // 110%
            },
            status: EntryStatus::Pending,
            stake_snapshot: 1_000_000,
            depth_boost: 10,
            bump: 0,
        });
        
        // Regular user order
        let regular_priority = calculator.calculate_priority(
            10_000,    // stake
            5,         // depth
            95,        // submission (earlier)
            10_000,    // volume
            105,       // current
            10_000_000, // total stake
        ).unwrap();
        
        entries.push(QueueEntry {
            entry_id: 2,
            user: regular,
            priority_score: regular_priority,
            submission_slot: 95,
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: true,
                amount: 10_000,
                leverage: U64F64::from_num(20_000_000), // 20x
                max_slippage: U64F64::from_num(20_000), // 2%
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 10_000,
            depth_boost: 5,
            bump: 0,
        });
        
        // Update queue size
        queue.current_size = entries.len() as u32;
        queue.total_pending_volume = entries.iter().map(|e| e.trade_data.amount).sum();
        
        // Apply fair ordering
        let ordering_state = OrderingState::new();
        fair_ordering.apply_fair_ordering(&mut entries, &ordering_state).unwrap();
        
        // Process queue
        let mut mev_state = MEVProtectionState {
            recent_trades: Vec::new(),
            suspicious_patterns: 0,
            last_check_slot: 105,
        };
        
        let result = processor.process_queue(
            &mut queue,
            &mut entries,
            &mut mev_state,
        ).unwrap();
        
        // Verify processing
        assert!(result.processed_count > 0);
        assert_eq!(result.total_volume, 104_500); // 95% fill rate
        
        // Check order status
        for entry in &entries {
            if entry.status == EntryStatus::Executed {
                println!("Executed order {} for user with priority {}", 
                    entry.entry_id, 
                    entry.priority_score
                );
            }
        }
    }
    
    #[tokio::test]
    async fn test_liquidation_priority_queue() {
        // Special test for liquidation orders which have inverse priority
        // (lower distance to liquidation = higher priority)
        
        let mut liquidation_queue = Vec::new();
        
        // Position close to liquidation
        liquidation_queue.push(LiquidationOrder {
            position_id: 1,
            trader: Pubkey::new_unique(),
            risk_score: 0, // Will be calculated
            distance_to_liq: 100, // $0.01 from liquidation
            effective_leverage: 100, // 100x
            mmt_stake: 1000,
            submission_slot: 100,
            open_interest: 50_000,
        });
        
        // Position with more buffer
        liquidation_queue.push(LiquidationOrder {
            position_id: 2,
            trader: Pubkey::new_unique(),
            risk_score: 0,
            distance_to_liq: 5000, // $0.50 from liquidation
            effective_leverage: 20, // 20x
            mmt_stake: 500,
            submission_slot: 101,
            open_interest: 25_000,
        });
        
        // Calculate risk scores
        for order in &mut liquidation_queue {
            order.risk_score = order.calculate_risk_score();
        }
        
        // Sort by priority (lower score = higher priority)
        liquidation_queue.sort();
        
        // Verify ordering
        assert_eq!(liquidation_queue[0].position_id, 1); // Closest to liquidation
        assert!(liquidation_queue[0].risk_score < liquidation_queue[1].risk_score);
        
        println!("Liquidation priorities:");
        for order in &liquidation_queue {
            println!(
                "Position {}: distance={}, leverage={}, risk_score={}",
                order.position_id,
                order.distance_to_liq,
                order.effective_leverage,
                order.risk_score
            );
        }
    }
}

// Temporary struct for liquidation testing
#[derive(Debug, Clone)]
struct LiquidationOrder {
    position_id: u128,
    trader: Pubkey,
    risk_score: u64,
    distance_to_liq: u64,
    effective_leverage: u64,
    mmt_stake: u64,
    submission_slot: u64,
    open_interest: u64,
}

impl LiquidationOrder {
    fn calculate_risk_score(&self) -> u64 {
        if self.effective_leverage == 0 {
            return u64::MAX;
        }
        
        let base_score = (self.distance_to_liq as u128 * 10000 / self.effective_leverage as u128) as u64;
        
        if self.mmt_stake > 0 {
            base_score / self.mmt_stake.min(10000)
        } else {
            base_score * 100
        }
    }
}

impl Ord for LiquidationOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.risk_score.cmp(&self.risk_score)
    }
}

impl PartialOrd for LiquidationOrder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for LiquidationOrder {
    fn eq(&self, other: &Self) -> bool {
        self.position_id == other.position_id
    }
}

impl Eq for LiquidationOrder {}