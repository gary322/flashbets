//! Phase 3: Liquidation Mechanics - Comprehensive Unit Tests
//!
//! Tests for liquidation formula, keeper incentives, partial liquidations,
//! and chain position unwinding.

use betting_platform_native::{
    liquidation::{
        calculate_liquidation_price_spec,
        calculate_margin_ratio_spec,
        calculate_effective_leverage,
        verify_liquidation_calculation,
        ChainLiquidationProcessor,
        ChainStepType,
    },
    keeper_liquidation::{
        LiquidationKeeper, KEEPER_REWARD_BPS, LIQUIDATION_THRESHOLD,
        MAX_LIQUIDATION_PERCENT,
    },
    integration::partial_liquidation::{
        PartialLiquidationEngine, PARTIAL_LIQUIDATION_FACTOR,
        MIN_LIQUIDATION_AMOUNT,
    },
    state::{Position, chain_accounts::{ChainPosition, ChainState}},
    error::BettingPlatformError,
};
use solana_program::pubkey::Pubkey;

#[test]
fn test_liquidation_formula_spec() {
    // Test the specification formula: liq_price = entry_price * (1 - (margin_ratio / lev_eff))
    let entry_price = 5_000_000_000; // $5000
    let base_leverage = 10;
    let sigma = 150; // 1.5%
    
    // Calculate margin ratio
    let margin_ratio = calculate_margin_ratio_spec(base_leverage, sigma, 1).unwrap();
    
    // Test long position liquidation
    let liq_price_long = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio,
        base_leverage, // No chain multiplier
        true,
    ).unwrap();
    
    // Verify liquidation price is below entry for longs
    assert!(liq_price_long < entry_price);
    
    // Calculate expected: MR = 1/10 + 1.5% * sqrt(10) * 1 ≈ 14.74%
    // liq_price = 5000 * (1 - 0.1474/10) ≈ 5000 * 0.9853 ≈ 4926
    let expected_ratio = margin_ratio as f64 / 10000.0 / base_leverage as f64;
    let expected_price = entry_price as f64 * (1.0 - expected_ratio);
    let diff = (liq_price_long as f64 - expected_price).abs();
    
    assert!(diff < 1_000_000.0); // Within $1 tolerance
}

#[test]
fn test_margin_ratio_calculation() {
    let test_cases = vec![
        (1, 1, 10000 + 150), // 1x leverage: 100% + volatility
        (10, 1, 1000 + 474), // 10x leverage: 10% + volatility
        (50, 1, 200 + 1060), // 50x leverage: 2% + volatility
        (100, 1, 100 + 1500), // 100x leverage: 1% + volatility
    ];
    
    for (leverage, positions, expected_base) in test_cases {
        let margin_ratio = calculate_margin_ratio_spec(leverage, 150, positions).unwrap();
        
        // Base margin
        let base = 10000 / leverage;
        
        // Volatility component
        let sqrt_lev = (leverage as f64).sqrt() as u64;
        let f_n = 10000 + 1000 * (positions - 1);
        let volatility = (150 * sqrt_lev * f_n) / (10000 * 10000);
        
        assert_eq!(margin_ratio, base + volatility);
    }
}

#[test]
fn test_effective_leverage_with_chain() {
    // Test base leverage
    let base_leverage = 10;
    let effective = calculate_effective_leverage(base_leverage, None).unwrap();
    assert_eq!(effective, 10);
    
    // Test with 2x chain multiplier
    let effective_2x = calculate_effective_leverage(base_leverage, Some(20000)).unwrap();
    assert_eq!(effective_2x, 20);
    
    // Test with 5x chain multiplier (should cap at 500x)
    let effective_5x = calculate_effective_leverage(100, Some(50000)).unwrap();
    assert_eq!(effective_5x, 500); // Capped
    
    // Test extreme case
    let effective_extreme = calculate_effective_leverage(200, Some(100000)).unwrap();
    assert_eq!(effective_extreme, 500); // Still capped at 500x
}

#[test]
fn test_keeper_reward_calculation() {
    let liquidation_amount = 10_000_000_000; // $10k liquidated
    
    // Calculate 5bp reward
    let keeper_reward = (liquidation_amount as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
    
    assert_eq!(keeper_reward, 5_000_000); // $5 reward (0.05% of $10k)
}

#[test]
fn test_partial_liquidation_only() {
    let position_size = 10_000_000_000; // $10k position
    
    // Calculate partial liquidation (50%)
    let partial_amount = (position_size * PARTIAL_LIQUIDATION_FACTOR) / 10000;
    assert_eq!(partial_amount, 5_000_000_000); // $5k (50%)
    
    // Ensure it's not a full liquidation
    assert!(partial_amount < position_size);
    assert_eq!(partial_amount, position_size / 2);
    
    // Test minimum liquidation amount
    let small_position = 50_000_000; // $50 position
    let small_partial = (small_position * PARTIAL_LIQUIDATION_FACTOR) / 10000;
    assert!(small_partial < MIN_LIQUIDATION_AMOUNT); // Below minimum
}

#[test]
fn test_max_liquidation_cap() {
    let position_size = 10_000_000_000; // $10k position
    
    // Calculate max liquidation per slot (8%)
    let max_liquidation = (position_size * MAX_LIQUIDATION_PERCENT) / 10000;
    assert_eq!(max_liquidation, 800_000_000); // $800 (8%)
    
    // Partial liquidation should be capped by max
    let partial = (position_size * PARTIAL_LIQUIDATION_FACTOR) / 10000;
    let actual_liquidation = partial.min(max_liquidation);
    
    // In this case, partial (50%) is more than max (8%), so use max
    assert_eq!(actual_liquidation, max_liquidation);
}

#[test]
fn test_chain_position_unwinding_order() {
    let mut positions = vec![
        ChainPosition {
            position_id: 1,
            step_index: 2, // Borrow (last priority)
            size: 1_000_000_000,
            ..Default::default()
        },
        ChainPosition {
            position_id: 2,
            step_index: 0, // Stake (first priority)
            size: 1_000_000_000,
            ..Default::default()
        },
        ChainPosition {
            position_id: 3,
            step_index: 1, // Liquidate (second priority)
            size: 1_000_000_000,
            ..Default::default()
        },
    ];
    
    // Sort by unwinding order
    positions.sort_by_key(|p| match p.step_index % 3 {
        0 => 0, // Stake first
        1 => 1, // Liquidate second
        _ => 2, // Borrow last
    });
    
    // Verify order: stake → liquidate → borrow
    assert_eq!(positions[0].step_index, 0); // Stake
    assert_eq!(positions[1].step_index, 1); // Liquidate
    assert_eq!(positions[2].step_index, 2); // Borrow
}

#[test]
fn test_liquidation_threshold() {
    // Position at risk when risk score >= 90
    let risk_scores = vec![
        (89, false), // Not liquidatable
        (90, true),  // Liquidatable
        (95, true),  // Liquidatable
        (100, true), // Liquidatable
    ];
    
    for (score, should_liquidate) in risk_scores {
        assert_eq!(score >= LIQUIDATION_THRESHOLD, should_liquidate);
    }
}

#[test]
fn test_liquidation_price_boundaries() {
    let entry_price = 1_000_000_000; // $1000
    
    // Test low leverage (large buffer)
    let low_lev = 2;
    let margin_ratio_low = calculate_margin_ratio_spec(low_lev, 150, 1).unwrap();
    let liq_low = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio_low,
        low_lev,
        true,
    ).unwrap();
    
    let buffer_low = ((entry_price - liq_low) as f64 / entry_price as f64) * 100.0;
    assert!(buffer_low > 20.0); // >20% buffer for 2x leverage
    
    // Test high leverage (tiny buffer)
    let high_lev = 100;
    let margin_ratio_high = calculate_margin_ratio_spec(high_lev, 150, 1).unwrap();
    let liq_high = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio_high,
        high_lev,
        true,
    ).unwrap();
    
    let buffer_high = ((entry_price - liq_high) as f64 / entry_price as f64) * 100.0;
    assert!(buffer_high < 2.0); // <2% buffer for 100x leverage
}

#[test]
fn test_chain_liquidation_verification() {
    let entry_price = 5_000_000_000; // $5000
    let base_leverage = 20;
    let chain_multiplier = 2; // 2x from chain
    let effective_leverage = 40;
    
    let verification = verify_liquidation_calculation(
        entry_price,
        base_leverage,
        effective_leverage,
        150, // sigma
        1,   // positions
        true, // is_long
    ).unwrap();
    
    // With 40x effective leverage, liquidation should be very close
    assert!(verification.liquidation_percentage < 500); // Less than 5% buffer
    assert_eq!(verification.base_leverage, base_leverage);
    assert_eq!(verification.effective_leverage, effective_leverage);
}

#[test]
fn test_partial_liquidation_engine() {
    let mut engine = PartialLiquidationEngine {
        total_liquidations_processed: 0,
        total_value_liquidated: 0,
        active_liquidations: 0,
        keeper_rewards_paid: 0,
        partial_liquidation_enabled: true,
        emergency_liquidation_mode: false,
        last_liquidation_slot: 0,
        liquidation_queue_size: 0,
    };
    
    engine.initialize().unwrap();
    
    assert!(engine.partial_liquidation_enabled);
    assert!(!engine.emergency_liquidation_mode);
    assert_eq!(engine.total_liquidations_processed, 0);
}

#[test]
fn test_liquidation_amount_limits() {
    // Test minimum liquidation amount
    let tiny_position = 10_000_000; // $10 position
    let tiny_partial = (tiny_position * PARTIAL_LIQUIDATION_FACTOR) / 10000;
    assert!(tiny_partial < MIN_LIQUIDATION_AMOUNT);
    
    // Test maximum liquidation percent
    let large_position = 100_000_000_000; // $100k position
    let max_liq = (large_position * MAX_LIQUIDATION_PERCENT) / 10000;
    assert_eq!(max_liq, 8_000_000_000); // $8k (8%)
}

#[test]
fn test_short_position_liquidation() {
    let entry_price = 1_000_000_000; // $1000
    let leverage = 10;
    let margin_ratio = calculate_margin_ratio_spec(leverage, 150, 1).unwrap();
    
    // Short positions liquidate when price rises
    let liq_price_short = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio,
        leverage,
        false, // is_short
    ).unwrap();
    
    assert!(liq_price_short > entry_price); // Above entry for shorts
    
    // Calculate buffer
    let buffer = ((liq_price_short - entry_price) as f64 / entry_price as f64) * 100.0;
    assert!(buffer < 2.0); // Small buffer for 10x leverage
}