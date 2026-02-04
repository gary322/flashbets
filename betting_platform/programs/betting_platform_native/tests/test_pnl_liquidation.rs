//! Tests for PnL-based liquidation system
//! 
//! Verifies that the liquidation formula correctly adjusts effective leverage
//! based on unrealized profit/loss: effective_leverage = position_leverage × (1 - unrealized_pnl_pct)

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use solana_sdk::account::Account;

use betting_platform_native::{
    state::Position,
    liquidation::{
        calculate_effective_leverage,
        calculate_liquidation_price_spec,
        calculate_margin_ratio_spec,
        helpers::should_liquidate_coverage_based,
    },
    math::U64F64,
};

#[test]
fn test_position_pnl_calculation() {
    let user = Pubkey::new_unique();
    let proposal_id = 12345u128;
    let verse_id = 67890u128;
    
    // Create a long position at $100 with 10x leverage
    let mut position = Position::new(
        user,
        proposal_id,
        verse_id,
        0, // outcome
        1_000_000_000, // $1000 size
        10, // 10x leverage
        100_000_000, // $100 entry price
        true, // is_long
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    // Test 1: Price increases to $120 (20% profit)
    position.calculate_unrealized_pnl(120_000_000).unwrap();
    assert_eq!(position.unrealized_pnl, 200_000_000); // $200 profit
    assert_eq!(position.unrealized_pnl_pct, 2000); // 20% in basis points
    
    // Test 2: Price decreases to $90 (-10% loss)
    position.calculate_unrealized_pnl(90_000_000).unwrap();
    assert_eq!(position.unrealized_pnl, -100_000_000); // $100 loss
    assert_eq!(position.unrealized_pnl_pct, -1000); // -10% in basis points
    
    // Test 3: Short position - price drops to $80 (20% profit for short)
    let mut short_position = Position::new(
        user,
        proposal_id,
        verse_id,
        1, // outcome
        1_000_000_000, // $1000 size
        10, // 10x leverage
        100_000_000, // $100 entry price
        false, // is_short
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    short_position.calculate_unrealized_pnl(80_000_000).unwrap();
    assert_eq!(short_position.unrealized_pnl, 200_000_000); // $200 profit
    assert_eq!(short_position.unrealized_pnl_pct, 2000); // 20% in basis points
}

#[test]
fn test_effective_leverage_adjustment() {
    let user = Pubkey::new_unique();
    let proposal_id = 12345u128;
    let verse_id = 67890u128;
    
    // Create position with 10x leverage
    let mut position = Position::new(
        user,
        proposal_id,
        verse_id,
        0,
        1_000_000_000, // $1000 size
        10, // 10x leverage
        100_000_000, // $100 entry price
        true, // is_long
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    // Test 1: No PnL - effective leverage = base leverage
    let effective = position.get_effective_leverage().unwrap();
    assert_eq!(effective, 10);
    
    // Test 2: 20% profit - effective leverage should decrease
    position.unrealized_pnl_pct = 2000; // 20% profit
    let effective_profit = position.get_effective_leverage().unwrap();
    assert_eq!(effective_profit, 8); // 10 * (1 - 0.2) = 8x
    
    // Test 3: 10% loss - effective leverage should increase
    position.unrealized_pnl_pct = -1000; // -10% loss
    let effective_loss = position.get_effective_leverage().unwrap();
    assert_eq!(effective_loss, 11); // 10 * (1 - (-0.1)) = 11x
    
    // Test 4: Extreme profit (90%) - should cap at minimum
    position.unrealized_pnl_pct = 9000; // 90% profit
    let effective_extreme = position.get_effective_leverage().unwrap();
    assert_eq!(effective_extreme, 1); // Minimum 1x leverage
}

#[test]
fn test_dynamic_liquidation_price() {
    let user = Pubkey::new_unique();
    let proposal_id = 12345u128;
    let verse_id = 67890u128;
    
    // Create position with 10x leverage at $100
    let mut position = Position::new(
        user,
        proposal_id,
        verse_id,
        0,
        1_000_000_000, // $1000 size
        10, // 10x leverage
        100_000_000, // $100 entry price
        true, // is_long
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    // Initial liquidation price (no PnL)
    let initial_liq_price = position.liquidation_price;
    assert_eq!(initial_liq_price, 90_000_000); // $90 (10% below entry)
    
    // Test 1: Position gains 20% - liquidation price should move further away
    position.calculate_unrealized_pnl(120_000_000).unwrap(); // $120 current price
    position.update_liquidation_price().unwrap();
    
    let new_liq_price = position.liquidation_price;
    assert!(new_liq_price < initial_liq_price); // Should be lower (safer)
    assert_eq!(new_liq_price, 87_500_000); // $87.50 (with 8x effective leverage)
    
    // Test 2: Position loses 10% - liquidation price should move closer
    position.calculate_unrealized_pnl(90_000_000).unwrap(); // $90 current price
    position.update_liquidation_price().unwrap();
    
    let loss_liq_price = position.liquidation_price;
    assert!(loss_liq_price > initial_liq_price); // Should be higher (riskier)
    assert!(loss_liq_price > 90_000_000); // Above $90 (with 11x effective leverage)
}

#[test]
fn test_should_liquidate_with_pnl() {
    let user = Pubkey::new_unique();
    let proposal_id = 12345u128;
    let verse_id = 67890u128;
    
    // Create position with 10x leverage
    let mut position = Position::new(
        user,
        proposal_id,
        verse_id,
        0,
        1_000_000_000, // $1000 size
        10, // 10x leverage
        100_000_000, // $100 entry price
        true, // is_long
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    // Test 1: Profitable position at $110 (10% profit)
    position.update_with_price(110_000_000).unwrap();
    
    // Check liquidation at $88 (would liquidate without PnL adjustment)
    assert!(!position.should_liquidate(88_000_000)); // Should NOT liquidate due to lower effective leverage
    
    // Test 2: Losing position at $95 (-5% loss)
    position.update_with_price(95_000_000).unwrap();
    
    // Check liquidation at $91 (close to liquidation)
    assert!(position.should_liquidate(90_000_000)); // Should liquidate due to higher effective leverage
}

#[test]
fn test_coverage_based_liquidation_with_pnl() {
    let user = Pubkey::new_unique();
    let proposal_id = 12345u128;
    let verse_id = 67890u128;
    
    // Create position
    let mut position = Position::new(
        user,
        proposal_id,
        verse_id,
        0,
        1_000_000_000, // $1000 size
        20, // 20x leverage
        100_000_000, // $100 entry price
        true, // is_long
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    let coverage = U64F64::from_num(0.5); // 0.5 coverage
    
    // Test 1: Position with 10% profit
    position.update_with_price(110_000_000).unwrap();
    
    // Should be less likely to liquidate due to reduced effective leverage
    let should_liq_profit = should_liquidate_coverage_based(&position, 96_000_000, coverage).unwrap();
    assert!(!should_liq_profit);
    
    // Test 2: Same position with 5% loss
    position.update_with_price(95_000_000).unwrap();
    
    // Should be more likely to liquidate due to increased effective leverage
    let should_liq_loss = should_liquidate_coverage_based(&position, 96_000_000, coverage).unwrap();
    assert!(should_liq_loss);
}

#[test]
fn test_extreme_pnl_scenarios() {
    let user = Pubkey::new_unique();
    let proposal_id = 12345u128;
    let verse_id = 67890u128;
    
    // Create high leverage position
    let mut position = Position::new(
        user,
        proposal_id,
        verse_id,
        0,
        1_000_000_000, // $1000 size
        50, // 50x leverage
        100_000_000, // $100 entry price
        true, // is_long
        Clock::get().unwrap_or_default().unix_timestamp,
    );
    
    // Test 1: Extreme profit (80%)
    position.update_with_price(180_000_000).unwrap(); // $180
    
    let effective_leverage = position.get_effective_leverage().unwrap();
    assert_eq!(effective_leverage, 10); // 50 * (1 - 0.8) = 10x
    
    // Should be very safe from liquidation
    assert!(!position.should_liquidate(150_000_000)); // Even at $150, still safe
    
    // Test 2: Small loss on high leverage (-2%)
    position.update_with_price(98_000_000).unwrap(); // $98
    
    let effective_leverage_loss = position.get_effective_leverage().unwrap();
    assert_eq!(effective_leverage_loss, 51); // 50 * (1 - (-0.02)) = 51x
    
    // Should be very close to liquidation
    assert!(position.should_liquidate(97_500_000)); // Liquidates at $97.50
}

#[test]
fn test_chain_position_with_pnl() {
    // Test effective leverage with both PnL and chain multiplier
    let base_leverage = 10;
    let chain_multiplier = Some(15000); // 1.5x
    let pnl_pct = Some(2000); // 20% profit
    
    let effective = calculate_effective_leverage(
        base_leverage,
        chain_multiplier,
        pnl_pct,
    ).unwrap();
    
    // 10 * (1 - 0.2) * 1.5 = 12x
    assert_eq!(effective, 12);
    
    // Test with loss
    let pnl_loss = Some(-1000); // -10% loss
    let effective_loss = calculate_effective_leverage(
        base_leverage,
        chain_multiplier,
        pnl_loss,
    ).unwrap();
    
    // 10 * (1 - (-0.1)) * 1.5 = 16.5x
    assert_eq!(effective_loss, 16);
}

#[test]
fn test_liquidation_formula_compliance() {
    // Verify the formula matches specification:
    // effective_leverage = position_leverage × (1 - unrealized_pnl_pct)
    
    struct TestCase {
        base_leverage: u64,
        pnl_pct: i64,
        expected_effective: u64,
    }
    
    let test_cases = vec![
        TestCase { base_leverage: 10, pnl_pct: 0, expected_effective: 10 },
        TestCase { base_leverage: 10, pnl_pct: 2000, expected_effective: 8 },
        TestCase { base_leverage: 10, pnl_pct: -1000, expected_effective: 11 },
        TestCase { base_leverage: 20, pnl_pct: 5000, expected_effective: 10 },
        TestCase { base_leverage: 5, pnl_pct: -2000, expected_effective: 6 },
        TestCase { base_leverage: 100, pnl_pct: 9000, expected_effective: 10 },
    ];
    
    for case in test_cases {
        let effective = calculate_effective_leverage(
            case.base_leverage,
            None,
            Some(case.pnl_pct),
        ).unwrap();
        
        assert_eq!(
            effective, 
            case.expected_effective,
            "Failed for leverage {} with PnL {}%",
            case.base_leverage,
            case.pnl_pct / 100
        );
    }
}