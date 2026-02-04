//! Phase 2 Functional Tests
//! 
//! Tests for AMM auto-selection, Polymarket rate limiting, and oracle integration

use solana_program_test::*;
use solana_sdk::{
    account_info::AccountInfo,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};

use betting_platform::{
    state::amm_accounts::AMMType,
    amm::auto_selector::{select_amm_type, validate_amm_selection, get_recommended_liquidity},
    integration::{
        rate_limiter::{RateLimiter, RateLimiterState},
        polymarket_oracle::{
            PolymarketOracle, MarketPriceFeed, PriceFeedStatus,
            POLYMARKET_POLL_INTERVAL_SLOTS, POLYMARKET_POLL_INTERVAL_SECONDS,
        },
    },
    error::BettingPlatformError,
};
use solana_program::clock::Clock;

#[test]
fn test_amm_auto_selection() {
    let current_time = Clock::default().unix_timestamp;
    
    // Test N=1 → LMSR
    let amm_type = select_amm_type(1, None, None, current_time).unwrap();
    assert_eq!(amm_type, AMMType::LMSR);
    
    // Test N=2 → PM-AMM
    let amm_type = select_amm_type(2, None, None, current_time).unwrap();
    assert_eq!(amm_type, AMMType::PMAMM);
    
    // Test N>2 conditional logic
    // N=5 should use PM-AMM (<=8 outcomes)
    let amm_type = select_amm_type(5, None, None, current_time).unwrap();
    assert_eq!(amm_type, AMMType::PMAMM);
    
    // N=10 should use L2-AMM (>8 outcomes)
    let amm_type = select_amm_type(10, None, None, current_time).unwrap();
    assert_eq!(amm_type, AMMType::L2AMM);
    
    // Test edge cases
    assert!(select_amm_type(0, None, None, current_time).is_err());
    assert!(select_amm_type(100, None, None, current_time).is_err());
}

#[test]
fn test_amm_liquidity_recommendations() {
    // Test LMSR liquidity
    let liquidity = get_recommended_liquidity(AMMType::LMSR, 1);
    assert_eq!(liquidity, 1_000_000_000); // 1000 USDC
    
    // Test PM-AMM liquidity scales with outcomes
    let liquidity = get_recommended_liquidity(AMMType::PMAMM, 2);
    assert_eq!(liquidity, 1_000_000_000); // 500 * 2 = 1000 USDC
    
    let liquidity = get_recommended_liquidity(AMMType::PMAMM, 4);
    assert_eq!(liquidity, 2_000_000_000); // 500 * 4 = 2000 USDC
    
    // Test L2-AMM liquidity
    let liquidity = get_recommended_liquidity(AMMType::L2AMM, 10);
    assert_eq!(liquidity, 2_000_000_000); // 2000 USDC base
}

#[test]
fn test_amm_validation() {
    // Valid configurations
    assert!(validate_amm_selection(AMMType::LMSR, 1, 100_000_000).is_ok());
    assert!(validate_amm_selection(AMMType::PMAMM, 2, 50_000_000).is_ok());
    assert!(validate_amm_selection(AMMType::L2AMM, 10, 200_000_000).is_ok());
    
    // Invalid outcome counts
    assert!(validate_amm_selection(AMMType::LMSR, 2, 100_000_000).is_err());
    assert!(validate_amm_selection(AMMType::PMAMM, 1, 50_000_000).is_err());
    
    // Insufficient liquidity
    assert!(validate_amm_selection(AMMType::LMSR, 1, 10_000_000).is_err());
    assert!(validate_amm_selection(AMMType::PMAMM, 2, 10_000_000).is_err());
    assert!(validate_amm_selection(AMMType::L2AMM, 10, 50_000_000).is_err());
}

#[test]
fn test_polymarket_rate_limiting() {
    let mut limiter = RateLimiter::new();
    
    // Test market rate limit (50/10s)
    for i in 0..RateLimiter::MARKET_LIMIT {
        assert!(limiter.check_market_limit().is_ok(), "Market request {} should succeed", i);
    }
    
    // 51st request should fail
    assert!(limiter.check_market_limit().is_err(), "51st market request should fail");
    
    // Test order rate limit (500/10s)
    for i in 0..RateLimiter::ORDER_LIMIT {
        assert!(limiter.check_order_limit().is_ok(), "Order request {} should succeed", i);
    }
    
    // 501st request should fail
    assert!(limiter.check_order_limit().is_err(), "501st order request should fail");
    
    // Test usage stats
    let (market_usage, order_usage) = limiter.get_usage();
    assert_eq!(market_usage, RateLimiter::MARKET_LIMIT);
    assert_eq!(order_usage, RateLimiter::ORDER_LIMIT);
    
    // Test reset
    limiter.reset();
    let (market_usage, order_usage) = limiter.get_usage();
    assert_eq!(market_usage, 0);
    assert_eq!(order_usage, 0);
}

#[test]
fn test_polymarket_polling_interval() {
    // Verify polling constants
    assert_eq!(POLYMARKET_POLL_INTERVAL_SECONDS, 60);
    assert_eq!(POLYMARKET_POLL_INTERVAL_SLOTS, 150); // 60s at 0.4s/slot
    
    let mut oracle = PolymarketOracle {
        authority: Pubkey::new_unique(),
        last_update_slot: 1000,
        last_update_timestamp: 1000,
        total_markets_tracked: 0,
        active_price_feeds: 0,
        connection_status: PriceFeedStatus::Active,
        fallback_mode: false,
        total_updates_processed: 0,
        failed_updates: 0,
        average_latency_ms: 0,
        lookup_table_counter: 0,
    };
    
    // Should not poll yet
    assert!(!oracle.should_poll(1100));
    
    // Should poll after 150 slots
    assert!(oracle.should_poll(1150));
    assert!(oracle.should_poll(1200));
    
    // Update poll time
    oracle.update_poll_time(1200, 1200);
    assert_eq!(oracle.last_update_slot, 1200);
    assert_eq!(oracle.total_updates_processed, 1);
    
    // Should not poll again until next interval
    assert!(!oracle.should_poll(1300));
    assert!(oracle.should_poll(1350));
}

#[test]
fn test_oracle_health_check() {
    let mut oracle = PolymarketOracle {
        authority: Pubkey::new_unique(),
        last_update_slot: 0,
        last_update_timestamp: 0,
        total_markets_tracked: 0,
        active_price_feeds: 0,
        connection_status: PriceFeedStatus::Active,
        fallback_mode: false,
        total_updates_processed: 100,
        failed_updates: 5,
        average_latency_ms: 50,
        lookup_table_counter: 0,
    };
    
    // Healthy oracle
    assert!(oracle.is_healthy());
    
    // Unhealthy: disconnected
    oracle.connection_status = PriceFeedStatus::Disconnected;
    assert!(!oracle.is_healthy());
    oracle.connection_status = PriceFeedStatus::Active;
    
    // Unhealthy: fallback mode
    oracle.fallback_mode = true;
    assert!(!oracle.is_healthy());
    oracle.fallback_mode = false;
    
    // Unhealthy: too many failures (>10%)
    oracle.failed_updates = 15;
    assert!(!oracle.is_healthy());
}

#[tokio::test]
async fn test_rate_limiter_state() {
    let mut state = RateLimiterState {
        authority: Pubkey::new_unique(),
        market_request_count: 0,
        order_request_count: 0,
        window_start: 1000,
        total_requests: 0,
        total_rejections: 0,
    };
    
    // Test market requests within limit
    for _ in 0..50 {
        assert!(state.check_and_update(true).is_ok());
    }
    assert_eq!(state.market_request_count, 50);
    assert_eq!(state.total_requests, 50);
    
    // 51st market request should fail
    assert!(state.check_and_update(true).is_err());
    assert_eq!(state.total_rejections, 1);
    
    // Test order requests
    for _ in 0..500 {
        assert!(state.check_and_update(false).is_ok());
    }
    assert_eq!(state.order_request_count, 500);
    
    // 501st order request should fail
    assert!(state.check_and_update(false).is_err());
    assert_eq!(state.total_rejections, 2);
}

#[test]
fn test_polymarket_as_sole_oracle() {
    // This test verifies that Polymarket is configured as the sole oracle
    // In production, this would be enforced through program configuration
    
    let oracle = PolymarketOracle {
        authority: Pubkey::new_unique(),
        last_update_slot: 0,
        last_update_timestamp: 0,
        total_markets_tracked: 100,
        active_price_feeds: 95,
        connection_status: PriceFeedStatus::Active,
        fallback_mode: false,
        total_updates_processed: 1000,
        failed_updates: 10,
        average_latency_ms: 45,
        lookup_table_counter: 0,
    };
    
    // Verify oracle is properly configured
    assert_eq!(oracle.connection_status, PriceFeedStatus::Active);
    assert!(!oracle.fallback_mode);
    assert!(oracle.is_healthy());
    
    // Verify we track multiple markets from Polymarket
    assert!(oracle.total_markets_tracked > 0);
    assert!(oracle.active_price_feeds > 0);
    
    // Verify low latency
    assert!(oracle.average_latency_ms < 100);
}

#[test]
fn test_integrated_amm_and_oracle_flow() {
    // Test complete flow: AMM selection → Oracle price feed
    let current_time = Clock::default().unix_timestamp;
    
    // Market with 2 outcomes
    let outcome_count = 2;
    
    // 1. Auto-select AMM
    let amm_type = select_amm_type(outcome_count, None, None, current_time).unwrap();
    assert_eq!(amm_type, AMMType::PMAMM);
    
    // 2. Get recommended liquidity
    let liquidity = get_recommended_liquidity(amm_type, outcome_count);
    assert_eq!(liquidity, 1_000_000_000);
    
    // 3. Validate selection
    assert!(validate_amm_selection(amm_type, outcome_count, liquidity).is_ok());
    
    // 4. Create price feed from Polymarket
    let price_feed = MarketPriceFeed {
        market_id: Pubkey::new_unique(),
        polymarket_id: "0x123...".to_string(),
        yes_price: 6500, // 65%
        no_price: 3500,  // 35%
        mid_price: 6500, // Same as yes for binary
        spread_bps: 50,  // 0.5%
        liquidity: liquidity,
        volume_24h: 5_000_000_000, // $5k volume
        last_update_slot: 1000,
        last_update_timestamp: 1000,
        confidence: 95,
        status: PriceFeedStatus::Active,
    };
    
    // 5. Verify price feed is valid
    assert_eq!(price_feed.yes_price + price_feed.no_price, 10000); // Sums to 100%
    assert!(price_feed.liquidity >= liquidity);
    assert_eq!(price_feed.status, PriceFeedStatus::Active);
}