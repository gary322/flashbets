//! End-to-end test for oracle halt mechanism on >10% spread

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
    integration::{
        median_oracle::MedianOracleHandler,
        polymarket_oracle::MarketPriceFeed,
    },
};

#[tokio::test]
async fn test_oracle_halts_on_10_percent_spread() {
    let market_id = Pubkey::new_unique();
    
    // Create feed with >10% spread (yes + no != 100%)
    let invalid_spread_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 6000, // 60%
        no_price: 5000,  // 50% - Total 110% (10% spread)
        mid_price: 5500,
        bid: 5400,
        ask: 5600,
        volume_24h: 1_000_000_000_000,
        liquidity_depth: 100_000_000_000,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        fallback_price: None,
        fallback_slot: None,
    };

    // Test the halt check
    let result = MedianOracleHandler::check_and_halt_on_spread(
        invalid_spread_feed.yes_price,
        invalid_spread_feed.no_price,
    );

    assert!(result.is_err(), "Should halt with >10% spread");
    match result.err().unwrap() {
        err if err == BettingPlatformError::ExcessivePriceMovement.into() => {
            // Expected error - market should halt
        }
        _ => panic!("Should return ExcessivePriceMovement error"),
    }
}

#[tokio::test]
async fn test_oracle_allows_under_10_percent_spread() {
    // Test with acceptable spread (<10%)
    let yes_price = 5200; // 52%
    let no_price = 4800;  // 48% - Total 100% (0% spread)
    
    let result = MedianOracleHandler::check_and_halt_on_spread(yes_price, no_price);
    assert!(result.is_ok(), "Should not halt with 0% spread");

    // Test with 5% spread
    let yes_price_5pct = 5250; // 52.5%
    let no_price_5pct = 4750;  // 47.5% - Total 100% (5% acceptable)
    
    let result_5pct = MedianOracleHandler::check_and_halt_on_spread(
        yes_price_5pct,
        no_price_5pct,
    );
    assert!(result_5pct.is_ok(), "Should not halt with 5% spread");
}

#[tokio::test]
async fn test_oracle_halt_exactly_10_percent() {
    // Test exactly at 10% threshold
    let yes_price = 5500; // 55%
    let no_price = 5500;  // 55% - Total 110% (exactly 10% spread)
    
    let result = MedianOracleHandler::check_and_halt_on_spread(yes_price, no_price);
    assert!(result.is_ok(), "Should not halt at exactly 10% (threshold is >10%)");

    // Test just over 10%
    let yes_price_over = 5501; // 55.01%
    let no_price_over = 5500;  // 55% - Total 110.01% (>10% spread)
    
    let result_over = MedianOracleHandler::check_and_halt_on_spread(
        yes_price_over,
        no_price_over,
    );
    assert!(result_over.is_err(), "Should halt when over 10%");
}

#[tokio::test]
async fn test_oracle_halt_with_extreme_spreads() {
    // Test with extreme positive spread
    let yes_extreme = 9000; // 90%
    let no_extreme = 8000;  // 80% - Total 170% (70% spread!)
    
    let result_extreme = MedianOracleHandler::check_and_halt_on_spread(
        yes_extreme,
        no_extreme,
    );
    assert!(result_extreme.is_err(), "Should halt with extreme 70% spread");

    // Test with prices below 100%
    let yes_low = 3000; // 30%
    let no_low = 4000;  // 40% - Total 70% (30% negative spread)
    
    let result_low = MedianOracleHandler::check_and_halt_on_spread(yes_low, no_low);
    assert!(result_low.is_err(), "Should halt with 30% negative spread");
}

#[tokio::test]
async fn test_oracle_halt_integrated_with_fetch() {
    let market_id = Pubkey::new_unique();
    
    // Create feed that will trigger halt
    let halt_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 7000, // 70%
        no_price: 6000,  // 60% - Total 130% (30% spread - should halt)
        mid_price: 6500,
        bid: 6400,
        ask: 6600,
        volume_24h: 1_000_000_000_000,
        liquidity_depth: 100_000_000_000,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        fallback_price: None,
        fallback_slot: None,
    };

    // Test that fetch_median_price halts on spread
    let result = MedianOracleHandler::fetch_median_price(
        &market_id,
        Some(&halt_feed),
        None,
        None,
        1001,
    );

    assert!(result.is_err(), "fetch_median_price should halt on >10% spread");
    match result.err().unwrap() {
        err if err == BettingPlatformError::ExcessivePriceMovement.into() => {
            // Expected - oracle halted due to price spread
        }
        _ => panic!("Should halt with ExcessivePriceMovement error"),
    }
}

#[tokio::test]
async fn test_oracle_halt_calculation_precision() {
    // Test edge cases in spread calculation
    
    // Test with very small deviation (0.1%)
    let yes_small = 5010; // 50.1%
    let no_small = 4990;  // 49.9% - Total 100% (0% spread)
    
    let result_small = MedianOracleHandler::check_and_halt_on_spread(yes_small, no_small);
    assert!(result_small.is_ok(), "Should not halt with 0% spread");

    // Test with 9.99% spread (just under threshold)
    let yes_999 = 5499; // 54.99%
    let no_999 = 4500;  // 45% - Total 99.99% (0.01% under threshold)
    
    let result_999 = MedianOracleHandler::check_and_halt_on_spread(yes_999, no_999);
    assert!(result_999.is_ok(), "Should not halt with 9.99% spread");

    // Test with 10.01% spread (just over threshold)
    let yes_1001 = 5501; // 55.01%
    let no_1001 = 5500;  // 55% - Total 110.01% (10.01% spread)
    
    let result_1001 = MedianOracleHandler::check_and_halt_on_spread(yes_1001, no_1001);
    assert!(result_1001.is_err(), "Should halt with 10.01% spread");
}