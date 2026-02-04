// Integration tests for Median-of-3 Oracle implementation

use solana_program_test::*;
use solana_sdk::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    system_instruction,
};
use betting_platform_native::{
    integration::{
        median_oracle::*,
        polymarket_oracle::*,
        pyth_oracle::*,
        chainlink_oracle::*,
    },
    error::BettingPlatformError,
};

#[tokio::test]
async fn test_median_oracle_initialization() {
    let mut test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let mut context = test.start_with_context().await;
    
    // Create accounts
    let median_oracle = Keypair::new();
    let polymarket_oracle = Keypair::new();
    let pyth_config = Keypair::new();
    let chainlink_config = Keypair::new();
    
    // Initialize Median Oracle
    let rent = context.banks_client.get_rent().await.unwrap();
    let space = MedianOracleState::SIZE;
    
    let create_ix = system_instruction::create_account(
        &context.payer.pubkey(),
        &median_oracle.pubkey(),
        rent.minimum_balance(space),
        space as u64,
        &betting_platform_native::id(),
    );
    
    let init_ix = betting_platform_native::instruction::initialize_median_oracle(
        &betting_platform_native::id(),
        &median_oracle.pubkey(),
        &context.payer.pubkey(),
        &polymarket_oracle.pubkey(),
        &pyth_config.pubkey(),
        &chainlink_config.pubkey(),
    );
    
    let mut transaction = Transaction::new_with_payer(
        &[create_ix, init_ix],
        Some(&context.payer.pubkey()),
    );
    
    transaction.sign(&[&context.payer, &median_oracle], context.last_blockhash);
    
    context.banks_client.process_transaction(transaction).await.unwrap();
    
    // Verify initialization
    let account = context.banks_client.get_account(median_oracle.pubkey()).await.unwrap().unwrap();
    let state = MedianOracleState::try_from_slice(&account.data).unwrap();
    
    assert_eq!(state.authority, context.payer.pubkey());
    assert_eq!(state.polymarket_oracle, polymarket_oracle.pubkey());
    assert_eq!(state.pyth_config, pyth_config.pubkey());
    assert_eq!(state.chainlink_config, chainlink_config.pubkey());
}

#[tokio::test]
async fn test_median_price_calculation() {
    // Test median calculation with all three sources
    let polymarket_data = OraclePriceData {
        source: OracleSource::Polymarket,
        price: 5000, // 0.5 in basis points
        confidence: 9500,
        timestamp: 1234567890,
        slot: 100,
    };
    
    let pyth_data = OraclePriceData {
        source: OracleSource::Pyth,
        price: 5100,
        confidence: 9800,
        timestamp: 1234567891,
        slot: 101,
    };
    
    let chainlink_data = OraclePriceData {
        source: OracleSource::Chainlink,
        price: 4900,
        confidence: 9600,
        timestamp: 1234567892,
        slot: 102,
    };
    
    let result = MedianOracleHandler::calculate_median_price(
        Some(polymarket_data),
        Some(pyth_data),
        Some(chainlink_data),
        105, // current slot
    ).unwrap();
    
    // Median of [4900, 5000, 5100] = 5000
    assert_eq!(result.median_price, 5000);
    assert_eq!(result.sources_used, 3);
    assert_eq!(result.polymarket_price, Some(5000));
    assert_eq!(result.pyth_price, Some(5100));
    assert_eq!(result.chainlink_price, Some(4900));
}

#[tokio::test]
async fn test_median_with_missing_source() {
    // Test with only 2 sources (minimum required)
    let polymarket_data = OraclePriceData {
        source: OracleSource::Polymarket,
        price: 5000,
        confidence: 9500,
        timestamp: 1234567890,
        slot: 100,
    };
    
    let pyth_data = OraclePriceData {
        source: OracleSource::Pyth,
        price: 5200,
        confidence: 9800,
        timestamp: 1234567891,
        slot: 101,
    };
    
    let result = MedianOracleHandler::calculate_median_price(
        Some(polymarket_data),
        Some(pyth_data),
        None, // Chainlink missing
        105,
    ).unwrap();
    
    // Weighted average of two sources
    // (5000 * 9500 + 5200 * 9800) / (9500 + 9800) ≈ 5102
    assert!(result.median_price > 5090 && result.median_price < 5110);
    assert_eq!(result.sources_used, 2);
}

#[tokio::test]
async fn test_insufficient_oracle_sources() {
    // Test with only 1 source (should fail)
    let polymarket_data = OraclePriceData {
        source: OracleSource::Polymarket,
        price: 5000,
        confidence: 9500,
        timestamp: 1234567890,
        slot: 100,
    };
    
    let result = MedianOracleHandler::calculate_median_price(
        Some(polymarket_data),
        None,
        None,
        105,
    );
    
    assert!(result.is_err());
    match result.err().unwrap() {
        e if e == BettingPlatformError::InsufficientOracleSources.into() => {},
        _ => panic!("Expected InsufficientOracleSources error"),
    }
}

#[tokio::test]
async fn test_stale_price_filtering() {
    // Test that stale prices are filtered out
    let fresh_data = OraclePriceData {
        source: OracleSource::Polymarket,
        price: 5000,
        confidence: 9500,
        timestamp: 1234567890,
        slot: 100,
    };
    
    let stale_data = OraclePriceData {
        source: OracleSource::Pyth,
        price: 4000, // Very different price
        confidence: 9800,
        timestamp: 1234567800,
        slot: 50, // Too old
    };
    
    let another_fresh = OraclePriceData {
        source: OracleSource::Chainlink,
        price: 5100,
        confidence: 9600,
        timestamp: 1234567892,
        slot: 102,
    };
    
    let result = MedianOracleHandler::calculate_median_price(
        Some(fresh_data),
        Some(stale_data),
        Some(another_fresh),
        105, // Current slot
    ).unwrap();
    
    // Should only use fresh data
    assert_eq!(result.sources_used, 2);
    // Median should be between 5000 and 5100, not affected by stale 4000
    assert!(result.median_price >= 5000 && result.median_price <= 5100);
}

#[tokio::test]
async fn test_pyth_oracle_integration() {
    use pyth_oracle::test_utils::create_mock_pyth_price;
    
    let pyth_price = create_mock_pyth_price(
        50_000_000, // $50 with 8 decimals
        1_000_000,  // $1 confidence interval
        1000,       // slot
    );
    
    // Serialize and deserialize to test
    let serialized = borsh::to_vec(&pyth_price).unwrap();
    let deserialized = PythPriceAccount::try_from_slice(&serialized).unwrap();
    
    assert_eq!(deserialized.price, 50_000_000);
    assert_eq!(deserialized.conf, 1_000_000);
    assert_eq!(deserialized.status, PythPriceStatus::Trading);
}

#[tokio::test]
async fn test_chainlink_oracle_integration() {
    use chainlink_oracle::test_utils::create_mock_chainlink_aggregator;
    
    let aggregator = create_mock_chainlink_aggregator(
        50_000_000_000, // $50k with 8 decimals
        8,              // decimals
        1000,           // slot
    );
    
    // Test price extraction
    let price = aggregator.get_price().unwrap();
    assert_eq!(price, 50_000_000_000);
    
    // Test confidence calculation
    let confidence = aggregator.get_confidence(1234567890);
    assert!(confidence > 9000); // Should be high confidence for fresh data
}

#[tokio::test]
async fn test_price_deviation_validation() {
    let result = MedianPriceResult {
        market_id: Pubkey::new_unique(),
        median_price: 5000,
        sources_used: 3,
        polymarket_price: Some(5000),
        pyth_price: Some(5500),       // 10% higher
        chainlink_price: Some(4500),   // 10% lower
        confidence: 9500,
        timestamp: 1234567890,
        slot: 100,
    };
    
    // This should pass as deviation is exactly 10%
    MedianOracleHandler::validate_price_deviation(&result).unwrap();
    
    let high_deviation_result = MedianPriceResult {
        market_id: Pubkey::new_unique(),
        median_price: 5000,
        sources_used: 3,
        polymarket_price: Some(5000),
        pyth_price: Some(6000),       // 20% higher
        chainlink_price: Some(4000),   // 20% lower
        confidence: 9500,
        timestamp: 1234567890,
        slot: 100,
    };
    
    // This should log a warning but not fail
    MedianOracleHandler::validate_price_deviation(&high_deviation_result).unwrap();
}

#[tokio::test]
async fn test_oracle_fallback() {
    let mut test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let mut context = test.start_with_context().await;
    
    // Create fallback handler
    let fallback = OracleFallbackHandler {
        last_good_prices: vec![
            MarketPriceFeed {
                market_id: Pubkey::new_unique(),
                polymarket_id: "test-market".to_string(),
                yes_price: 6000,
                no_price: 4000,
                mid_price: 5000,
                bid_ask_spread: 100,
                liquidity_usd: 1_000_000,
                volume_24h_usd: 500_000,
                last_trade_price: 5050,
                last_update_slot_slot: 100,
                last_update_slot_timestamp: 1234567890,
                price_confidence: 9500,
                status: PriceFeedStatus::Active,
                update_count: 10,
            }
        ],
        fallback_activated_slot: 100,
        max_fallback_duration: 300,
        decay_rate_bps: 10, // 0.1% per slot
    };
    
    // Test fallback price retrieval
    let (price, confidence) = fallback.get_fallback_price(
        &fallback.last_good_prices[0].market_id,
        110, // 10 slots later
    ).unwrap();
    
    assert_eq!(price, 5000);
    assert!(confidence < 9500); // Should have decayed
    assert!(confidence > 9400); // But not too much
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_median_calculation_performance() {
        let polymarket_data = OraclePriceData {
            source: OracleSource::Polymarket,
            price: 5000,
            confidence: 9500,
            timestamp: 1234567890,
            slot: 100,
        };
        
        let pyth_data = OraclePriceData {
            source: OracleSource::Pyth,
            price: 5100,
            confidence: 9800,
            timestamp: 1234567891,
            slot: 101,
        };
        
        let chainlink_data = OraclePriceData {
            source: OracleSource::Chainlink,
            price: 4900,
            confidence: 9600,
            timestamp: 1234567892,
            slot: 102,
        };
        
        let start = Instant::now();
        
        for _ in 0..1000 {
            let _ = MedianOracleHandler::calculate_median_price(
                Some(polymarket_data.clone()),
                Some(pyth_data.clone()),
                Some(chainlink_data.clone()),
                105,
            ).unwrap();
        }
        
        let duration = start.elapsed();
        let avg_time = duration.as_micros() / 1000;
        
        println!("Average median calculation time: {} μs", avg_time);
        assert!(avg_time < 100); // Should be very fast
    }
}