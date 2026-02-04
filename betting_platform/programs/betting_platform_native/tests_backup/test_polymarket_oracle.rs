//! Test Polymarket as sole oracle implementation

use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};
use betting_platform_native::{
    integration::{
        median_oracle::{
            PolymarketOracleState, 
            PolymarketOracleHandler,
        },
        polymarket_oracle::{
            MarketPriceFeed,
            PriceFeedStatus,
            MAX_PRICE_AGE_SLOTS,
            PRICE_CONFIDENCE_THRESHOLD,
        },
    },
    error::BettingPlatformError,
};

/// Helper function to create a test MarketPriceFeed
fn create_test_feed(
    market_id: Pubkey,
    yes_price: u64,
    no_price: u64,
    last_update_slot: u64,
    price_confidence: u64,
) -> MarketPriceFeed {
    MarketPriceFeed {
        market_id,
        polymarket_id: "test_market".to_string(),
        yes_price,
        no_price,
        mid_price: yes_price, // Use yes price as mid for simplicity
        bid_ask_spread: 10, // 0.1% spread
        liquidity_usd: 1_000_000_00, // $1M liquidity
        volume_24h_usd: 500_000_00, // $500k volume
        last_trade_price: yes_price,
        last_update_slot,
        last_update_timestamp: 0,
        price_confidence,
        status: PriceFeedStatus::Active,
        update_count: 1,
    }
}

#[test]
fn test_polymarket_sole_oracle() {
    println!("Testing Polymarket as sole oracle source");
    
    // Create oracle state
    let oracle_state = PolymarketOracleState {
        authority: Pubkey::new_unique(),
        polymarket_oracle: Pubkey::new_unique(),
        last_update_slot: 0,
        total_markets: 0,
        active_markets: 0,
        price_updates: 0,
        failed_updates: 0,
        halted_markets: 0,
        stale_price_flags: 0,
        polling_interval_slots: PolymarketOracleState::POLLING_INTERVAL_SLOTS,
    };
    
    // Verify polling interval is 60 seconds (150 slots)
    assert_eq!(oracle_state.polling_interval_slots, 150);
    assert_eq!(PolymarketOracleState::POLLING_INTERVAL_SECONDS, 60);
    
    println!("✓ Oracle state initialized with 60-second polling interval");
}

#[test]
fn test_oracle_spread_detection() {
    println!("Testing oracle spread detection and halt mechanism");
    
    let market_id = Pubkey::new_unique();
    let current_slot = 1000;
    
    // Test case 1: Normal spread (within 10%)
    let feed_normal = create_test_feed(
        market_id,
        6000, // 60%
        4000, // 40%
        current_slot - 10,
        9500, // 95%
    );
    
    let result = PolymarketOracleHandler::get_price(&feed_normal, current_slot);
    assert!(result.is_ok());
    
    let price_result = result.unwrap();
    assert_eq!(price_result.spread_basis_points, 0); // Perfect 100% sum
    assert!(!price_result.is_halted);
    println!("✓ Normal spread accepted (0 basis points)");
    
    // Test case 2: Excessive spread (>10%)
    let feed_excessive = create_test_feed(
        market_id,
        7000, // 70%
        4500, // 45% 
        current_slot - 10,
        9500,
    );
    
    let result = PolymarketOracleHandler::get_price(&feed_excessive, current_slot);
    assert!(result.is_err());
    match result.unwrap_err() {
        ProgramError::Custom(e) if e == BettingPlatformError::ExcessivePriceMovement as u32 => {
            println!("✓ Market halted due to >10% spread");
        }
        _ => panic!("Expected ExcessivePriceMovement error"),
    }
}

#[test]
fn test_stale_price_detection() {
    println!("Testing stale price detection and flagging");
    
    let market_id = Pubkey::new_unique();
    let current_slot = 1000;
    
    // Test case 1: Fresh price
    let feed_fresh = create_test_feed(
        market_id,
        5000,
        5000,
        current_slot - 10, // 10 slots old
        9500,
    );
    
    let result = PolymarketOracleHandler::get_price(&feed_fresh, current_slot).unwrap();
    assert!(!result.is_stale);
    println!("✓ Fresh price detected (10 slots old)");
    
    // Test case 2: Stale price
    let feed_stale = create_test_feed(
        market_id,
        5000,
        5000,
        current_slot - (MAX_PRICE_AGE_SLOTS + 10), // Too old
        9500,
    );
    
    let result = PolymarketOracleHandler::get_price(&feed_stale, current_slot).unwrap();
    assert!(result.is_stale);
    println!("✓ Stale price flagged ({} slots old)", MAX_PRICE_AGE_SLOTS + 10);
}

#[test]
fn test_polling_schedule() {
    println!("Testing 60-second polling schedule");
    
    let mut oracle_state = PolymarketOracleState {
        authority: Pubkey::new_unique(),
        polymarket_oracle: Pubkey::new_unique(),
        last_update_slot: 100,
        total_markets: 0,
        active_markets: 0,
        price_updates: 0,
        failed_updates: 0,
        halted_markets: 0,
        stale_price_flags: 0,
        polling_interval_slots: 150, // 60 seconds
    };
    
    // Should not poll yet
    assert!(!oracle_state.should_poll(200)); // Only 100 slots passed
    println!("✓ No polling at 100 slots (40 seconds)");
    
    // Should poll now
    assert!(oracle_state.should_poll(250)); // 150 slots passed
    println!("✓ Polling triggered at 150 slots (60 seconds)");
    
    // Update last poll time
    oracle_state.last_update_slot = 250;
    assert!(!oracle_state.should_poll(300)); // Only 50 slots since last poll
    println!("✓ Polling schedule working correctly");
}

#[test]
fn test_price_confidence() {
    println!("Testing price confidence requirements");
    
    let market_id = Pubkey::new_unique();
    let current_slot = 1000;
    
    // Test low confidence rejection
    let feed_low_confidence = create_test_feed(
        market_id,
        5000,
        5000,
        current_slot - 10,
        9000, // 90% - below threshold
    );
    
    let result = PolymarketOracleHandler::get_price(&feed_low_confidence, current_slot);
    assert!(result.is_err());
    match result.unwrap_err() {
        ProgramError::Custom(e) if e == BettingPlatformError::InsufficientConfidence as u32 => {
            println!("✓ Low confidence price rejected (90% < 95% threshold)");
        }
        _ => panic!("Expected InsufficientConfidence error"),
    }
}

#[test]
fn test_no_other_oracles() {
    println!("Testing that only Polymarket is used (no Pyth/Chainlink)");
    
    // The calculate_median_price function should only use Polymarket
    let polymarket_price = Some(5500u64);
    let pyth_price = Some(5600u64); // Should be ignored
    let chainlink_price = Some(5400u64); // Should be ignored
    
    let result = PolymarketOracleHandler::calculate_median_price(
        polymarket_price,
        pyth_price,
        chainlink_price,
    ).unwrap();
    
    assert_eq!(result.price, 5500); // Only Polymarket price used
    assert_eq!(result.confidence, PRICE_CONFIDENCE_THRESHOLD);
    println!("✓ Only Polymarket price used, other oracles ignored");
}

fn main() {
    println!("Running Polymarket sole oracle tests...");
    test_polymarket_sole_oracle();
    test_oracle_spread_detection();
    test_stale_price_detection();
    test_polling_schedule();
    test_price_confidence();
    test_no_other_oracles();
    println!("\nAll oracle tests passed!");
}