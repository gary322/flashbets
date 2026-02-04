//! End-to-end test for Polymarket as sole oracle

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use betting_platform_native::{
    error::BettingPlatformError,
    instruction::BettingPlatformInstruction,
    integration::{
        median_oracle::{MedianOracleState, MedianOracleHandler},
        polymarket_oracle::{MarketPriceFeed, OracleSource, OraclePriceData},
    },
};

#[tokio::test]
async fn test_polymarket_sole_oracle_no_median() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create Polymarket price feed
    let market_id = Pubkey::new_unique();
    let polymarket_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 6000, // 60%
        no_price: 4000,  // 40%
        mid_price: 5000,  // Average
        bid: 5900,
        ask: 6100,
        volume_24h: 1_000_000_000_000, // $1M volume
        liquidity_depth: 100_000_000_000, // $100k liquidity
        last_update_slot_slot: 1000,
        last_update_slot_timestamp: 1234567890,
        price_confidence: 9800, // 98% confidence
        fallback_price: Some(5000),
        fallback_slot: Some(900),
    };

    // Test that only Polymarket price is used
    let result = MedianOracleHandler::fetch_median_price(
        &market_id,
        Some(&polymarket_feed),
        None, // No Pyth
        None, // No Chainlink
        1001, // Current slot
    );

    assert!(result.is_ok(), "Should succeed with only Polymarket");
    let median_result = result.unwrap();
    
    assert_eq!(median_result.sources_used, 1, "Should only use 1 source");
    assert_eq!(median_result.polymarket_price, Some(5000), "Should use Polymarket mid price");
    assert_eq!(median_result.pyth_price, None, "Should not have Pyth price");
    assert_eq!(median_result.chainlink_price, None, "Should not have Chainlink price");
    assert_eq!(median_result.median_price, 5000, "Median should equal Polymarket price");
}

#[tokio::test]
async fn test_oracle_fails_without_polymarket() {
    let market_id = Pubkey::new_unique();
    
    // Test with no Polymarket feed
    let result = MedianOracleHandler::fetch_median_price(
        &market_id,
        None, // No Polymarket
        None, // No Pyth
        None, // No Chainlink
        1000,
    );

    assert!(result.is_err(), "Should fail without Polymarket");
    match result.err().unwrap() {
        err if err == BettingPlatformError::PolymarketOracleUnavailable.into() => {
            // Expected error
        }
        _ => panic!("Should return PolymarketOracleUnavailable error"),
    }
}

#[tokio::test]
async fn test_polymarket_price_validation() {
    let market_id = Pubkey::new_unique();
    
    // Test with valid Polymarket prices
    let valid_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5500, // 55%
        no_price: 4500,  // 45%
        mid_price: 5000,
        bid: 5400,
        ask: 5600,
        volume_24h: 500_000_000_000,
        liquidity_depth: 50_000_000_000,
        last_update_slot_slot: 1000,
        last_update_slot_timestamp: 1234567890,
        price_confidence: 9500,
        fallback_price: None,
        fallback_slot: None,
    };

    // Verify yes + no = 100%
    assert_eq!(
        valid_feed.yes_price + valid_feed.no_price, 
        10000, 
        "Yes + No prices should sum to 100%"
    );

    let result = MedianOracleHandler::fetch_median_price(
        &market_id,
        Some(&valid_feed),
        None,
        None,
        1001,
    );

    assert!(result.is_ok(), "Valid prices should succeed");
}

#[tokio::test]
async fn test_stale_polymarket_price_rejected() {
    use betting_platform_native::integration::polymarket_oracle::MAX_PRICE_AGE_SLOTS;
    
    let market_id = Pubkey::new_unique();
    let current_slot = 2000;
    
    // Create stale price feed
    let stale_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5000,
        no_price: 5000,
        mid_price: 5000,
        bid: 4900,
        ask: 5100,
        volume_24h: 100_000_000_000,
        liquidity_depth: 10_000_000_000,
        last_update_slot_slot: current_slot - MAX_PRICE_AGE_SLOTS - 1, // Too old
        last_update_slot_timestamp: 1234567890,
        price_confidence: 9000,
        fallback_price: Some(5000),
        fallback_slot: Some(current_slot - MAX_PRICE_AGE_SLOTS - 2),
    };

    let result = MedianOracleHandler::fetch_median_price(
        &market_id,
        Some(&stale_feed),
        None,
        None,
        current_slot,
    );

    assert!(result.is_err(), "Should reject stale price");
    match result.err().unwrap() {
        err if err == BettingPlatformError::StaleOracleData.into() => {
            // Expected error
        }
        _ => panic!("Should return StaleOracleData error"),
    }
}

#[tokio::test]
async fn test_polymarket_confidence_threshold() {
    use betting_platform_native::integration::polymarket_oracle::PRICE_CONFIDENCE_THRESHOLD;
    
    let market_id = Pubkey::new_unique();
    
    // Test with low confidence
    let low_confidence_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5000,
        no_price: 5000,
        mid_price: 5000,
        bid: 4500,
        ask: 5500,
        volume_24h: 10_000_000_000, // Low volume
        liquidity_depth: 1_000_000_000, // Low liquidity
        last_update_slot_slot: 1000,
        last_update_slot_timestamp: 1234567890,
        price_confidence: PRICE_CONFIDENCE_THRESHOLD - 100, // Below threshold
        fallback_price: None,
        fallback_slot: None,
    };

    // Should use price despite low confidence since Polymarket is sole oracle
    let result = MedianOracleHandler::calculate_median_price(
        Some(OraclePriceData {
            source: OracleSource::Polymarket,
            price: low_confidence_feed.mid_price,
            confidence: low_confidence_feed.price_confidence,
            timestamp: low_confidence_feed.last_update_slot_timestamp,
            slot: low_confidence_feed.last_update_slot_slot,
        }),
        None,
        None,
        1001,
    );

    assert!(result.is_err(), "Should fail with low confidence");
}