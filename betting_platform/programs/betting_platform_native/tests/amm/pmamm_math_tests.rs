use betting_platform_native::amm::pmamm::math::*;
use betting_platform_native::state::amm_accounts::{PMAMMPool, MarketState};
use betting_platform_native::math::U128F128;
use solana_program::pubkey::Pubkey;

fn create_test_pool() -> PMAMMPool {
    PMAMMPool {
        discriminator: [112, 78, 45, 209, 156, 34, 89, 167], // PMAMM_MARKET discriminator
        market_id: 1,
        pool_id: 1,
        l_parameter: 6000,
        expiry_time: 1735689600,
        num_outcomes: 3,
        reserves: vec![1000, 2000, 3000],
        total_liquidity: 6000,
        total_lp_supply: 1000000,
        liquidity_providers: 1, // u32 count, not Vec
        state: MarketState::Active,
        initial_price: 5000,
        probabilities: vec![3333, 3333, 3334], // Sum to 10000
        fee_bps: 30,
        oracle: Pubkey::new_unique(),
        total_volume: 0,
        created_at: 1704067200,
        last_update: 1704067200,
    }
}

#[test]
fn test_calculate_invariant() {
    let reserves = vec![1000, 2000, 3000];
    let k = calculate_invariant(&reserves).unwrap();
    
    // K should be 1000 * 2000 * 3000 = 6,000,000,000
    let expected = U128F128::from_num(6_000_000_000u128);
    let diff = if k > expected { k - expected } else { expected - k };
    assert!(diff < U128F128::from_num(1u128));
}

#[test]
fn test_swap_output() {
    let pool = create_test_pool();
    
    // Swap 100 of outcome 0 for outcome 1
    let (output, fee) = calculate_swap_output(&pool, 0, 1, 100).unwrap();
    
    // Fee should be 0.3% of 100 = 0.3 (rounds to 0)
    assert_eq!(fee, 0);
    
    // Output should maintain constant product
    // New reserves would be [1100, 2000-output]
    // 1100 * (2000-output) ≈ 1000 * 2000
    assert!(output > 0 && output < 200);
}

#[test]
fn test_slippage_pmamm() {
    let pool = create_test_pool();
    
    // Test with order_size=10, tau=0.1 (1000 bps)
    let slippage = calculate_slippage_pmamm(&pool, 0, 1, 10, 1000).unwrap();
    
    // According to spec, for order=10, LVR=0.05, tau=0.1, delta ~9.8
    // With our reserves [1000, 2000], this should be close to that
    assert!(slippage >= 8 && slippage <= 11, "Slippage {} not in expected range", slippage);
    
    // Test that it's ~15% less than LMSR (11.5)
    let lmsr_equivalent = 115; // 11.5 scaled by 10
    let reduction_percent = ((lmsr_equivalent - slippage * 10) * 100) / lmsr_equivalent;
    assert!(reduction_percent >= 10 && reduction_percent <= 20, 
            "Reduction {} not around 15%", reduction_percent);
}

#[test]
fn test_probabilities() {
    let pool = create_test_pool();
    let probs = calculate_probabilities(&pool).unwrap();
    
    assert_eq!(probs.len(), 3);
    
    // Sum should be approximately 10000 (100%)
    let sum: u64 = probs.iter().sum();
    assert!((sum as i64 - 10000).abs() < 100);
    
    // Higher reserves should have lower probability
    assert!(probs[0] > probs[1]);
    assert!(probs[1] > probs[2]);
}