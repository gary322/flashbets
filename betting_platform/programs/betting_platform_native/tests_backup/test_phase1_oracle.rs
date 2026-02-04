//! Phase 1: Oracle System - Comprehensive Unit Tests
//!
//! Tests for Polymarket sole oracle implementation with spread detection,
//! stale price detection, and 60-second polling intervals.

use betting_platform_native::{
    integration::{
        polymarket_oracle::{
            PolymarketOracle, PolymarketPriceData, OracleConfig,
            STALE_PRICE_THRESHOLD_SLOTS, PRICE_SPREAD_HALT_THRESHOLD,
        },
    },
    error::BettingPlatformError,
};
use solana_program::{
    clock::Clock,
    pubkey::Pubkey,
};
use solana_program_test::*;

#[tokio::test]
async fn test_polymarket_oracle_initialization() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    
    // Test initial state
    assert!(!oracle.is_halted);
    assert_eq!(oracle.last_update_slot, 0);
    assert_eq!(oracle.consecutive_errors, 0);
    
    // Initialize oracle
    let config = OracleConfig {
        authority: Pubkey::new_unique(),
        update_interval_slots: 150, // 60 seconds
        max_price_age_slots: 300,
        spread_threshold_bps: 1000, // 10%
        enabled: true,
    };
    
    oracle.initialize(config.clone()).unwrap();
    
    assert_eq!(oracle.config.update_interval_slots, 150);
    assert_eq!(oracle.config.spread_threshold_bps, 1000);
    assert!(oracle.config.enabled);
}

#[tokio::test]
async fn test_price_spread_detection() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    // Test normal spread (5%)
    let price_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 5250, // 52.50%
        no_price: 5000,  // 50.00%
        spread_bps: 250, // 2.5% spread
        timestamp: 1234567890,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    let result = oracle.update_price(&price_data, 100);
    assert!(result.is_ok());
    assert!(!oracle.is_halted);
    
    // Test excessive spread (>10%)
    let high_spread_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 6000, // 60%
        no_price: 4500,  // 45%
        spread_bps: 1500, // 15% spread - should trigger halt
        timestamp: 1234567890,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    let result = oracle.update_price(&high_spread_data, 200);
    assert!(result.is_err());
    assert!(oracle.is_halted);
    assert_eq!(oracle.halt_reason, Some("Price spread exceeds 10% threshold".to_string()));
}

#[tokio::test]
async fn test_stale_price_detection() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    // Initial price update
    let price_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 5000,
        no_price: 5000,
        spread_bps: 0,
        timestamp: 1000,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    oracle.update_price(&price_data, 100).unwrap();
    assert_eq!(oracle.last_update_slot, 100);
    
    // Check staleness just before threshold
    let is_stale = oracle.is_price_stale(100 + STALE_PRICE_THRESHOLD_SLOTS - 1);
    assert!(!is_stale);
    
    // Check staleness at threshold
    let is_stale = oracle.is_price_stale(100 + STALE_PRICE_THRESHOLD_SLOTS);
    assert!(is_stale);
    
    // Try to use stale price
    let stale_price = oracle.get_price("test_market", 100 + STALE_PRICE_THRESHOLD_SLOTS + 1);
    assert!(stale_price.is_err());
}

#[tokio::test]
async fn test_60_second_polling_interval() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    let price_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 5000,
        no_price: 5000,
        spread_bps: 0,
        timestamp: 1000,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    // First update at slot 100
    oracle.update_price(&price_data, 100).unwrap();
    
    // Try update too soon (should fail)
    let result = oracle.update_price(&price_data, 149);
    assert!(result.is_err());
    
    // Update at correct interval (150 slots = 60 seconds)
    let result = oracle.update_price(&price_data, 250);
    assert!(result.is_ok());
    assert_eq!(oracle.last_update_slot, 250);
}

#[tokio::test]
async fn test_oracle_error_handling() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    // Simulate consecutive errors
    for i in 1..=5 {
        oracle.record_error(100 + i * 10);
        assert_eq!(oracle.consecutive_errors, i);
    }
    
    // Should halt after 5 consecutive errors
    assert!(oracle.is_halted);
    assert_eq!(oracle.halt_reason, Some("Too many consecutive errors".to_string()));
    
    // Reset should clear errors
    oracle.reset_halt(Pubkey::new_unique()).unwrap();
    assert!(!oracle.is_halted);
    assert_eq!(oracle.consecutive_errors, 0);
}

#[tokio::test]
async fn test_price_validation() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    // Test invalid prices (don't sum to ~100%)
    let invalid_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 4000, // 40%
        no_price: 4000,  // 40% - Total 80%, invalid
        spread_bps: 0,
        timestamp: 1000,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    let result = oracle.update_price(&invalid_data, 100);
    assert!(result.is_err());
    
    // Test valid prices
    let valid_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 4950, // 49.50%
        no_price: 5050,  // 50.50% - Total 100%, valid
        spread_bps: 100,
        timestamp: 1000,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    let result = oracle.update_price(&valid_data, 100);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multiple_market_tracking() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    // Update prices for multiple markets
    let markets = vec!["market1", "market2", "market3"];
    
    for (i, market_id) in markets.iter().enumerate() {
        let price_data = PolymarketPriceData {
            market_id: market_id.to_string(),
            yes_price: 5000 + (i as u64 * 100),
            no_price: 5000 - (i as u64 * 100),
            spread_bps: 0,
            timestamp: 1000 + i as i64,
            volume_24h: 1_000_000,
            liquidity: 500_000,
        };
        
        oracle.update_price(&price_data, 100).unwrap();
    }
    
    // Verify all prices are stored
    for (i, market_id) in markets.iter().enumerate() {
        let price = oracle.get_price(market_id, 100).unwrap();
        assert_eq!(price.yes_price, 5000 + (i as u64 * 100));
        assert_eq!(price.no_price, 5000 - (i as u64 * 100));
    }
}

#[tokio::test]
async fn test_oracle_websocket_health() {
    let oracle = PolymarketOracle::new(Pubkey::new_unique());
    
    // Test health check at different slots
    assert_eq!(oracle.get_websocket_health(100), 100); // Perfect health
    assert_eq!(oracle.get_websocket_health(1000), 20); // Degraded
    assert_eq!(oracle.get_websocket_health(10000), 0); // Dead
}

#[tokio::test]
async fn test_fallback_mechanism() {
    let mut oracle = PolymarketOracle::new(Pubkey::new_unique());
    oracle.initialize(OracleConfig::default()).unwrap();
    
    // Set up primary price
    let price_data = PolymarketPriceData {
        market_id: "test_market".to_string(),
        yes_price: 5000,
        no_price: 5000,
        spread_bps: 0,
        timestamp: 1000,
        volume_24h: 1_000_000,
        liquidity: 500_000,
    };
    
    oracle.update_price(&price_data, 100).unwrap();
    
    // Simulate websocket failure - should use last known price
    let current_slot = 100 + STALE_PRICE_THRESHOLD_SLOTS / 2;
    let price = oracle.get_price_with_fallback("test_market", current_slot).unwrap();
    assert_eq!(price.yes_price, 5000);
    assert!(price.is_fallback);
}

/// Test helper to create a mock oracle configuration
fn create_test_config() -> OracleConfig {
    OracleConfig {
        authority: Pubkey::new_unique(),
        update_interval_slots: 150,
        max_price_age_slots: 300,
        spread_threshold_bps: 1000,
        enabled: true,
    }
}