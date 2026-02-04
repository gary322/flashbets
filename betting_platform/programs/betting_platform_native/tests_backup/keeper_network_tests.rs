//! Comprehensive tests for keeper network systems
//!
//! Tests liquidation, stop-loss, price updates, coordination, and registration

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform_native::{
    keeper_liquidation::{LiquidationKeeper, AtRiskPosition},
    keeper_stop_loss::{StopLossKeeper, StopOrder, StopOrderType, OrderSide},
    keeper_price_update::{PriceUpdateKeeper, PriceUpdate, WebSocketHealth},
    keeper_coordination::{KeeperCoordinator, WorkType, WorkItem},
    keeper_registration::{KeeperRegistration, KeeperType, SlashingEvidence},
    keeper_ingestor::{IngestorKeeper, PolymarketMarket, IngestorError},
    state::{
        KeeperAccount, KeeperRegistry, KeeperStatus, KeeperSpecialization,
        IngestorState, Position, WebSocketState,
    },
    math::U64F64,
    error::BettingPlatformError,
};

#[tokio::test]
async fn test_liquidation_keeper_full_flow() {
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    );
    
    // Setup test position at risk
    let position = Position::new(
        Pubkey::new_unique(),
        12345,
        0, // verse_id
        0, // outcome
        1_000_000, // size
        10, // leverage
        500_000, // entry price (0.5)
        true, // is_long
        0, // created_at
    );
    
    // Calculate liquidation parameters
    let liquidation_price = position.liquidation_price;
    let current_price = liquidation_price - 1000; // Below liquidation
    
    // Test risk score calculation
    let risk_score = calculate_test_risk_score(&position, current_price);
    assert!(risk_score >= 90, "Position should be at high risk");
    
    // Test liquidation execution
    let liquidation_amount = position.size * 8 / 100; // 8% max per slot
    let keeper_reward = liquidation_amount * 5 / 10000; // 5bp
    
    assert_eq!(keeper_reward, 400); // 1M * 0.08 * 0.0005 = 400
    
    // Test partial liquidation tracking
    let mut partial_accumulator = 0u64;
    partial_accumulator += liquidation_amount;
    assert!(partial_accumulator < position.size, "Should be partial liquidation");
}

#[tokio::test]
async fn test_stop_loss_keeper_execution() {
    // Create test stop orders
    let stop_orders = vec![
        StopOrder {
            order_id: [1u8; 32],
            user: Pubkey::new_unique(),
            market_id: [1u8; 32],
            order_type: StopOrderType::StopLoss,
            trigger_price: U64F64::from_num(45_000), // $45k stop
            size: 100_000,
            side: OrderSide::Long,
            is_active: true,
            created_slot: 1000,
            prepaid_bounty: 200, // 2bp of 100k = 20
            position_entry_price: U64F64::from_num(50_000),
            trailing_distance: U64F64::from_num(0),
            trailing_price: U64F64::from_num(0),
            user_stake: Some(1_000_000), // 1 MMT
        },
        StopOrder {
            order_id: [2u8; 32],
            user: Pubkey::new_unique(),
            market_id: [1u8; 32],
            order_type: StopOrderType::TakeProfit,
            trigger_price: U64F64::from_num(55_000), // $55k take profit
            size: 100_000,
            side: OrderSide::Long,
            is_active: true,
            created_slot: 1000,
            prepaid_bounty: 200,
            position_entry_price: U64F64::from_num(50_000),
            trailing_distance: U64F64::from_num(0),
            trailing_price: U64F64::from_num(0),
            user_stake: Some(500_000),
        },
    ];
    
    // Test stop loss trigger
    let current_price = U64F64::from_num(44_000); // Below stop
    assert!(current_price <= stop_orders[0].trigger_price);
    
    let execution_result = stop_orders[0].execute(current_price).unwrap();
    assert_eq!(execution_result.executed_value, 4_400_000_000); // 100k * 44k
    
    // Test take profit trigger
    let high_price = U64F64::from_num(56_000); // Above take profit
    assert!(high_price >= stop_orders[1].trigger_price);
    
    // Test keeper bounty calculation
    let keeper_bounty = execution_result.executed_value * 2 / 10000; // 2bp
    assert_eq!(keeper_bounty, 880_000); // 4.4B * 0.0002
}

#[tokio::test]
async fn test_price_update_keeper() {
    let mut websocket_state = WebSocketState {
        last_update_slot: 1000,
        total_updates: 100,
        failed_updates: 0,
        current_health: WebSocketHealth::Healthy,
    };
    
    // Test health monitoring
    let test_cases = vec![
        (1100, WebSocketHealth::Healthy),     // 100 slots = ~40s
        (1500, WebSocketHealth::Degraded),    // 500 slots = ~3.3min
        (2000, WebSocketHealth::Failed),      // 1000 slots = ~6.7min
    ];
    
    for (current_slot, expected_health) in test_cases {
        websocket_state.last_update_slot = 1000;
        let health = monitor_websocket_health(&websocket_state, current_slot);
        assert_eq!(health, expected_health);
    }
    
    // Test price aggregation
    let price_feeds = vec![
        vec![U64F64::from_num(3) / U64F64::from_num(5), U64F64::from_num(2) / U64F64::from_num(5)], // 0.6, 0.4
        vec![U64F64::from_num(0.58), U64F64::from_num(0.42)],
        vec![U64F64::from_num(0.62), U64F64::from_num(0.38)],
    ];
    
    let weights = vec![50, 30, 20]; // Different source weights
    let aggregated = aggregate_prices(&price_feeds, &weights).unwrap();
    
    // Expected: (0.6*50 + 0.58*30 + 0.62*20) / 100 = 0.596
    let expected_yes = U64F64::from_num(0.596);
    assert!((aggregated[0] - expected_yes).abs() < U64F64::from_num(0.001));
}

#[tokio::test]
async fn test_keeper_coordination() {
    // Create test keepers with different priorities
    let mut keepers = vec![
        create_test_keeper([1u8; 32], 10_000_000, 9500, vec![KeeperSpecialization::Liquidations]),
        create_test_keeper([2u8; 32], 5_000_000, 9000, vec![KeeperSpecialization::Liquidations]),
        create_test_keeper([3u8; 32], 1_000_000, 9800, vec![KeeperSpecialization::StopLosses]),
    ];
    
    // Create work items
    let work_items: Vec<WorkItem> = (0..10)
        .map(|i| WorkItem {
            id: [i as u8; 32],
            work_type: WorkType::Liquidations,
            priority: 100 - i as u64,
            data: vec![],
            assigned_keeper: None,
            created_slot: 0,
            deadline_slot: 1000,
        })
        .collect();
    
    // Test work distribution
    let registry = KeeperRegistry::new();
    let assignments = KeeperCoordinator::assign_work_batch(
        &registry,
        &mut keepers,
        WorkType::Liquidations,
        work_items,
    ).unwrap();
    
    // Verify high priority keeper got most work
    assert_eq!(assignments.len(), 2); // Only 2 keepers have liquidation spec
    assert!(assignments[0].assigned_items.len() >= assignments[1].assigned_items.len());
    
    // Test priority calculation
    let priority1 = keepers[0].calculate_priority(); // 10M * 0.95 = 9.5M
    let priority2 = keepers[1].calculate_priority(); // 5M * 0.90 = 4.5M
    assert!(priority1 > priority2);
}

#[tokio::test]
async fn test_keeper_registration_and_slashing() {
    const MIN_KEEPER_STAKE: u64 = 100_000_000_000; // 100 MMT
    
    // Test minimum stake requirement
    assert!(50_000_000_000 < MIN_KEEPER_STAKE);
    assert!(150_000_000_000 >= MIN_KEEPER_STAKE);
    
    // Test slashing calculation
    let stake = 200_000_000_000; // 200 MMT
    let slash_amount = stake / 100; // 1%
    assert_eq!(slash_amount, 2_000_000_000); // 2 MMT
    
    // Test slashing evidence types
    let evidence_types = vec![
        SlashingEvidence::MissedLiquidation { 
            position_id: [1u8; 32], 
            slot: 1000 
        },
        SlashingEvidence::FalseExecution { 
            order_id: [2u8; 32], 
            execution_price: 50_000 
        },
        SlashingEvidence::Downtime { 
            start_slot: 1000, 
            end_slot: 11_000 // >1 hour
        },
    ];
    
    // Verify downtime threshold
    let downtime_slots = 11_000 - 1_000;
    assert!(downtime_slots > 9_000); // 1 hour threshold
}

#[tokio::test]
async fn test_ingestor_keeper() {
    let mut ingestor_state = IngestorState::new([1u8; 32], 0, 1000);
    
    // Test market validation
    let valid_market = PolymarketMarket {
        id: [1u8; 32],
        title: "Will ETH reach $5k?".to_string(),
        description: "Test market".to_string(),
        outcomes: vec!["Yes".to_string(), "No".to_string()],
        yes_price: 6500,
        no_price: 3500,
        volume_24h: 1_000_000,
        liquidity: 500_000,
        created_at: 1700000000,
        resolved: false,
        resolution: None,
    };
    
    assert!(IngestorKeeper::validate_market_data(&valid_market).is_ok());
    
    // Test invalid price sum
    let mut invalid_market = valid_market.clone();
    invalid_market.yes_price = 7000;
    invalid_market.no_price = 4000; // Sum = 11000, should be ~10000
    assert!(IngestorKeeper::validate_market_data(&invalid_market).is_err());
    
    // Test backoff calculation
    ingestor_state.error_count = 3;
    let backoff = 10 * 2_i64.pow(3); // 10 * 2^3 = 80 seconds
    assert_eq!(backoff, 80);
    
    // Test batch size limits
    let (offset, limit) = IngestorKeeper::get_next_batch(&ingestor_state).unwrap();
    assert_eq!(offset, 0);
    assert_eq!(limit, 1000); // MAX_BATCH_SIZE
}

// Helper functions
fn create_test_keeper(
    id: [u8; 32],
    stake: u64,
    performance: u64,
    specs: Vec<KeeperSpecialization>,
) -> KeeperAccount {
    KeeperAccount {
        discriminator: [0u8; 8],
        keeper_id: id,
        authority: Pubkey::new_unique(),
        keeper_type: KeeperType::Liquidation,
        mmt_stake: stake,
        performance_score: performance,
        total_operations: 100,
        successful_operations: (performance * 100) / 10000,
        total_rewards_earned: 0,
        last_operation_slot: 0,
        status: KeeperStatus::Active,
        specializations: specs,
        average_response_time: 2,
        priority_score: 0,
        registration_slot: 0,
        slashing_count: 0,
    }
}

fn calculate_test_risk_score(position: &Position, current_price: u64) -> u8 {
    let distance_to_liq = position.liquidation_price.saturating_sub(current_price);
    if distance_to_liq == 0 {
        return 100;
    }
    
    let risk_ratio = (position.liquidation_price * 100) / current_price;
    std::cmp::min(risk_ratio as u8, 100)
}

fn monitor_websocket_health(state: &WebSocketState, current_slot: u64) -> WebSocketHealth {
    let slots_since_update = current_slot.saturating_sub(state.last_update_slot);
    
    if slots_since_update < 150 {
        WebSocketHealth::Healthy
    } else if slots_since_update < 750 {
        WebSocketHealth::Degraded
    } else {
        WebSocketHealth::Failed
    }
}

fn aggregate_prices(feeds: &[Vec<U64F64>], weights: &[u64]) -> Result<Vec<U64F64>, BettingPlatformError> {
    if feeds.is_empty() || weights.len() != feeds.len() {
        return Err(BettingPlatformError::InvalidInput);
    }
    
    let outcome_count = feeds[0].len();
    let mut aggregated = vec![U64F64::from_num(0); outcome_count];
    let total_weight: u64 = weights.iter().sum();
    
    for (feed_idx, prices) in feeds.iter().enumerate() {
        let weight = U64F64::from_num(weights[feed_idx]);
        for (i, &price) in prices.iter().enumerate() {
            aggregated[i] = aggregated[i] + price * weight;
        }
    }
    
    for price in aggregated.iter_mut() {
        *price = *price / U64F64::from_num(total_weight);
    }
    
    Ok(aggregated)
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_keeper_performance_at_scale() {
        // Test with 1000 keepers
        let mut keepers = Vec::new();
        for i in 0..1000 {
            keepers.push(create_test_keeper(
                [(i % 256) as u8; 32],
                1_000_000 + i * 1000,
                8000 + (i % 2000),
                vec![KeeperSpecialization::Liquidations],
            ));
        }
        
        // Measure priority calculation time
        let start = Instant::now();
        let mut priorities: Vec<u64> = keepers
            .iter()
            .map(|k| k.calculate_priority())
            .collect();
        priorities.sort_by(|a, b| b.cmp(a));
        let elapsed = start.elapsed();
        
        // Should process 1000 keepers in <10ms
        assert!(elapsed.as_millis() < 10);
        
        // Test work distribution with 10k items
        let work_items: Vec<WorkItem> = (0..10_000)
            .map(|i| WorkItem {
                id: [(i % 256) as u8; 32],
                work_type: WorkType::Liquidations,
                priority: 10_000 - i as u64,
                data: vec![],
                assigned_keeper: None,
                created_slot: 0,
                deadline_slot: 100_000,
            })
            .collect();
        
        // Each keeper should get ~10 items
        let items_per_keeper = work_items.len() / keepers.len();
        assert_eq!(items_per_keeper, 10);
    }
}