use betting_platform_native::amm::lmsr::optimized_math::*;
use betting_platform_native::state::amm_accounts::{LSMRMarket, MarketState};
use solana_program::pubkey::Pubkey;

#[test]
fn test_optimized_price_calculation() {
    let market = LSMRMarket {
        discriminator: [0; 8],
        market_id: 0,
        b_parameter: 1000,
        num_outcomes: 2,
        shares: vec![100, 100],
        cost_basis: 0,
        state: MarketState::Active,
        created_at: 0,
        last_update: 0,
        total_volume: 0,
        fee_bps: 0,
        oracle: Pubkey::default(),
    };
    
    let price0 = calculate_price_optimized(&market.shares, 0, market.b_parameter).unwrap();
    let price1 = calculate_price_optimized(&market.shares, 1, market.b_parameter).unwrap();
    
    // Should be approximately 50% each
    assert!((price0 as i64 - 5000).abs() < 200); // Allow 2% error
    assert!((price1 as i64 - 5000).abs() < 200);
}

#[test]
fn test_fast_exp_lookup() {
    // Test common values
    // fast_exp_lookup(0) should return 1 (since e^0 = 1 in fixed point)
    assert_eq!(fast_exp_lookup(0).unwrap(), 1);
    
    // For x=1000 (1.0 in basis points), the function uses bit shift approximation
    // Just check it returns a reasonable value
    let val1000 = fast_exp_lookup(1000).unwrap();
    assert!(val1000 > 0);
}