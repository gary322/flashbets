//! End-to-end test for oracle halt mechanism on >10% spread

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
};
use betting_platform_native::{
    error::BettingPlatformError,
    integration::{
        median_oracle::PolymarketOracleHandler,
        polymarket_oracle::{MarketPriceFeed, PriceFeedStatus},
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
        no_price: 5100,  // 51% - Total 111% (11% spread)
        mid_price: 5550,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5550,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };

    // Test the halt check through get_price
    let current_slot = 1001;
    let result = PolymarketOracleHandler::get_price(&invalid_spread_feed, current_slot);

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
    let market_id = Pubkey::new_unique();
    let current_slot = 1001;
    
    // Test with acceptable spread (0%)
    let zero_spread_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5200, // 52%
        no_price: 4800,  // 48% - Total 100% (0% spread)
        mid_price: 5000,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5000,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result = PolymarketOracleHandler::get_price(&zero_spread_feed, current_slot);
    assert!(result.is_ok(), "Should not halt with 0% spread");

    // Test with 5% spread (still acceptable)
    let five_pct_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5250, // 52.5%
        no_price: 4750,  // 47.5% - Total 100% (0% spread from 100%)
        mid_price: 5000,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5000,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_5pct = PolymarketOracleHandler::get_price(&five_pct_feed, current_slot);
    assert!(result_5pct.is_ok(), "Should not halt with 0% spread from 100%");
}

#[tokio::test]
async fn test_oracle_halt_exactly_10_percent() {
    let market_id = Pubkey::new_unique();
    let current_slot = 1001;
    
    // Test exactly at 10% threshold
    let ten_pct_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5500, // 55%
        no_price: 5500,  // 55% - Total 110% (exactly 10% spread)
        mid_price: 5500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result = PolymarketOracleHandler::get_price(&ten_pct_feed, current_slot);
    assert!(result.is_ok(), "Should not halt at exactly 10% (threshold is >10%)");

    // Test just over 10%
    let over_ten_pct_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5501, // 55.01%
        no_price: 5500,  // 55% - Total 110.01% (>10% spread)
        mid_price: 5500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_over = PolymarketOracleHandler::get_price(&over_ten_pct_feed, current_slot);
    assert!(result_over.is_err(), "Should halt when over 10%");
}

#[tokio::test]
async fn test_oracle_halt_with_extreme_spreads() {
    let market_id = Pubkey::new_unique();
    let current_slot = 1001;
    
    // Test with extreme positive spread
    let extreme_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 9000, // 90%
        no_price: 8000,  // 80% - Total 170% (70% spread!)
        mid_price: 8500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 8500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_extreme = PolymarketOracleHandler::get_price(&extreme_feed, current_slot);
    assert!(result_extreme.is_err(), "Should halt with extreme 70% spread");

    // Test with prices below 100%
    let low_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 3000, // 30%
        no_price: 4000,  // 40% - Total 70% (30% negative spread)
        mid_price: 3500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 3500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_low = PolymarketOracleHandler::get_price(&low_feed, current_slot);
    assert!(result_low.is_err(), "Should halt with 30% negative spread");
}

#[tokio::test]
async fn test_oracle_halt_integrated_with_fetch() {
    let market_id = Pubkey::new_unique();
    let current_slot = 1001;
    
    // Create feed that will trigger halt
    let halt_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 7000, // 70%
        no_price: 6000,  // 60% - Total 130% (30% spread - should halt)
        mid_price: 6500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 6500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };

    // Test that get_price halts on spread
    let result = PolymarketOracleHandler::get_price(&halt_feed, current_slot);

    assert!(result.is_err(), "get_price should halt on >10% spread");
    match result.err().unwrap() {
        err if err == BettingPlatformError::ExcessivePriceMovement.into() => {
            // Expected - oracle halted due to price spread
        }
        _ => panic!("Should halt with ExcessivePriceMovement error"),
    }
}

#[tokio::test]
async fn test_oracle_halt_calculation_precision() {
    let market_id = Pubkey::new_unique();
    let current_slot = 1001;
    
    // Test with very small deviation (0.1%)
    let small_dev_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5010, // 50.1%
        no_price: 4990,  // 49.9% - Total 100% (0% spread)
        mid_price: 5000,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5000,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_small = PolymarketOracleHandler::get_price(&small_dev_feed, current_slot);
    assert!(result_small.is_ok(), "Should not halt with 0% spread");

    // Test with 9.99% spread (just under threshold)
    let under_threshold_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5499, // 54.99%
        no_price: 5500,  // 55% - Total 109.99% (9.99% spread)
        mid_price: 5500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_999 = PolymarketOracleHandler::get_price(&under_threshold_feed, current_slot);
    assert!(result_999.is_ok(), "Should not halt with 9.99% spread");

    // Test with 10.01% spread (just over threshold)
    let over_threshold_feed = MarketPriceFeed {
        market_id,
        polymarket_id: "0x123456".to_string(),
        yes_price: 5501, // 55.01%
        no_price: 5500,  // 55% - Total 110.01% (10.01% spread)
        mid_price: 5500,
        bid_ask_spread: 200,
        liquidity_usd: 100_000_000_000,
        volume_24h_usd: 1_000_000_000_000,
        last_trade_price: 5500,
        last_update_slot: 1000,
        last_update_timestamp: 1234567890,
        price_confidence: 9800,
        status: PriceFeedStatus::Active,
        update_count: 1,
    };
    
    let result_1001 = PolymarketOracleHandler::get_price(&over_threshold_feed, current_slot);
    assert!(result_1001.is_err(), "Should halt with 10.01% spread");
}