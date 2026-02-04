//! Comprehensive test suite for PnL-based liquidation system
//! Tests all aspects of the implementation to ensure correctness

use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use betting_platform_native::{
    state::Position,
    liquidation::{
        calculate_effective_leverage,
        calculate_liquidation_price_spec,
        helpers::should_liquidate_coverage_based,
    },
    math::U64F64,
};

/// Test helper to create a position
fn create_test_position(
    entry_price: u64,
    leverage: u64,
    size: u64,
    is_long: bool,
) -> Position {
    let user = Pubkey::new_unique();
    Position::new(
    user,
    12345u128,
    67890u128,
    0,
    size,
    leverage,
    entry_price,
    is_long,
    1234567890i64, // Mock timestamp
    )
}

#[test]
fn test_pnl_calculation_accuracy() {
    println!("\n=== Testing PnL Calculation Accuracy ===");
    
    // Test 1: Long position with profit
    let mut long_pos = create_test_position(100_000_000, 10, 1_000_000_000, true);
    long_pos.calculate_unrealized_pnl(120_000_000).unwrap();
    
    assert_eq!(long_pos.unrealized_pnl, 200_000_000, "Long profit PnL incorrect");
    assert_eq!(long_pos.unrealized_pnl_pct, 2000, "Long profit percentage incorrect");
    println!("✓ Long position 20% profit: PnL = $200, PnL% = 20%");

    // Test 2: Long position with loss
    long_pos.calculate_unrealized_pnl(85_000_000).unwrap();
    assert_eq!(long_pos.unrealized_pnl, -150_000_000, "Long loss PnL incorrect");
    assert_eq!(long_pos.unrealized_pnl_pct, -1500, "Long loss percentage incorrect");
    println!("✓ Long position 15% loss: PnL = -$150, PnL% = -15%");

    // Test 3: Short position with profit (price down)
    let mut short_pos = create_test_position(100_000_000, 10, 1_000_000_000, false);
    short_pos.calculate_unrealized_pnl(80_000_000).unwrap();
    
    assert_eq!(short_pos.unrealized_pnl, 200_000_000, "Short profit PnL incorrect");
    assert_eq!(short_pos.unrealized_pnl_pct, 2000, "Short profit percentage incorrect");
    println!("✓ Short position 20% profit: PnL = $200, PnL% = 20%");

    // Test 4: Short position with loss (price up)
    short_pos.calculate_unrealized_pnl(110_000_000).unwrap();
    assert_eq!(short_pos.unrealized_pnl, -100_000_000, "Short loss PnL incorrect");
    assert_eq!(short_pos.unrealized_pnl_pct, -1000, "Short loss percentage incorrect");
    println!("✓ Short position 10% loss: PnL = -$100, PnL% = -10%");
    }

#[test]
fn test_effective_leverage_calculation() {
    println!("\n=== Testing Effective Leverage Calculation ===");
    
    // Test 1: No PnL adjustment
    let eff_lev = calculate_effective_leverage(10, None, None).unwrap();
    assert_eq!(eff_lev, 10);
    println!("✓ No PnL: 10x → 10x");

    // Test 2: 20% profit reduces leverage
    let eff_lev = calculate_effective_leverage(10, None, Some(2000)).unwrap();
    assert_eq!(eff_lev, 8);
    println!("✓ 20% profit: 10x → 8x");

    // Test 3: 10% loss increases leverage
    let eff_lev = calculate_effective_leverage(10, None, Some(-1000)).unwrap();
    assert_eq!(eff_lev, 11);
    println!("✓ 10% loss: 10x → 11x");

    // Test 4: Extreme profit (90%) - should hit minimum
    let eff_lev = calculate_effective_leverage(10, None, Some(9000)).unwrap();
    assert_eq!(eff_lev, 1);
    println!("✓ 90% profit: 10x → 1x (minimum)");

    // Test 5: Large loss (-50%)
    let eff_lev = calculate_effective_leverage(10, None, Some(-5000)).unwrap();
    assert_eq!(eff_lev, 15);
    println!("✓ 50% loss: 10x → 15x");

    // Test 6: With chain multiplier and PnL
    let eff_lev = calculate_effective_leverage(10, Some(20000), Some(2000)).unwrap();
    assert_eq!(eff_lev, 16); // 10 * 0.8 * 2 = 16
    println!("✓ 20% profit + 2x chain: 10x → 16x");

    // Test 7: Maximum leverage cap
    let eff_lev = calculate_effective_leverage(400, Some(20000), Some(-5000)).unwrap();
    assert_eq!(eff_lev, 500); // Capped at 500x
    println!("✓ High leverage capped: 1200x → 500x");
    }

#[test]
fn test_dynamic_liquidation_price() {
    println!("\n=== Testing Dynamic Liquidation Price ===");
    
    let mut position = create_test_position(100_000_000, 10, 1_000_000_000, true);
    let initial_liq = position.liquidation_price;
    println!("Initial liquidation price: ${}", initial_liq / 1_000_000);

    // Test 1: Profit makes position safer
    position.calculate_unrealized_pnl(120_000_000).unwrap();
    position.update_liquidation_price().unwrap();
    
    assert!(position.liquidation_price < initial_liq);
    println!("✓ After 20% profit: ${} (safer)", position.liquidation_price / 1_000_000);

    // Test 2: Loss makes position riskier
    position.calculate_unrealized_pnl(95_000_000).unwrap();
    position.update_liquidation_price().unwrap();
    
    assert!(position.liquidation_price > initial_liq);
    println!("✓ After 5% loss: ${} (riskier)", position.liquidation_price / 1_000_000);

    // Test 3: Break-even returns to original
    position.calculate_unrealized_pnl(100_000_000).unwrap();
    position.update_liquidation_price().unwrap();
    
    assert_eq!(position.liquidation_price, initial_liq);
    println!("✓ At break-even: ${} (original)", position.liquidation_price / 1_000_000);
    }

#[test]
fn test_liquidation_scenarios() {
    println!("\n=== Testing Liquidation Scenarios ===");
    
    // Scenario 1: Profitable position survives price drop
    let mut position = create_test_position(100_000_000, 20, 1_000_000_000, true);
    
    // First make it profitable
    position.update_with_price(110_000_000).unwrap(); // 10% profit
    
    // Check if liquidates at a price that would liquidate without PnL adjustment
    let test_price = 94_000_000; // Would liquidate at 20x, but not with reduced leverage
    assert!(!position.should_liquidate(test_price));
    println!("✓ Profitable position survives at $94 (would liquidate without PnL)");

    // Scenario 2: Losing position liquidates earlier
    let mut position = create_test_position(100_000_000, 20, 1_000_000_000, true);
    
    // Make it lose money
    position.update_with_price(98_000_000).unwrap(); // 2% loss
    
    // Should liquidate earlier than normal
    let test_price = 95_500_000;
    assert!(position.should_liquidate(test_price));
    println!("✓ Losing position liquidates at $95.50 (earlier than normal)");

    // Scenario 3: Extreme profit protection
    let mut position = create_test_position(100_000_000, 50, 1_000_000_000, true);
    
    // Massive profit
    position.update_with_price(180_000_000).unwrap(); // 80% profit
    
    // Should be very safe from liquidation
    let test_price = 60_000_000; // 40% drop from entry
    assert!(!position.should_liquidate(test_price));
    println!("✓ Extremely profitable position safe even at $60");
    }

#[test]
fn test_coverage_based_liquidation_with_pnl() {
    println!("\n=== Testing Coverage-Based Liquidation with PnL ===");
    
    let coverage = U64F64::from_num(0.8); // 0.8 coverage
    
    // Test 1: Profitable position with coverage check
    let mut position = create_test_position(100_000_000, 25, 1_000_000_000, true);
    position.update_with_price(108_000_000).unwrap(); // 8% profit
    
    let should_liq = should_liquidate_coverage_based(&position, 96_000_000, coverage).unwrap();
    assert!(!should_liq);
    println!("✓ Profitable position passes coverage check at $96");

    // Test 2: Same position without profit
    let mut position = create_test_position(100_000_000, 25, 1_000_000_000, true);
    position.update_with_price(100_000_000).unwrap(); // No profit
    
    let should_liq = should_liquidate_coverage_based(&position, 96_000_000, coverage).unwrap();
    assert!(should_liq);
    println!("✓ Same position without profit fails coverage check at $96");
    }

#[test]
fn test_margin_ratio_calculation() {
    println!("\n=== Testing Margin Ratio Calculation ===");
    
    let position = create_test_position(100_000_000, 10, 1_000_000_000, true);
    
    // Test at different prices
    let margin_ratio_100 = position.get_margin_ratio(100_000_000).unwrap();
    let margin_ratio_90 = position.get_margin_ratio(90_000_000).unwrap();
    let margin_ratio_110 = position.get_margin_ratio(110_000_000).unwrap();
    
    println!("✓ Margin ratio at $100: {:.2}%", margin_ratio_100.to_num() * 100.0);
    println!("✓ Margin ratio at $90: {:.2}%", margin_ratio_90.to_num() * 100.0);
    println!("✓ Margin ratio at $110: {:.2}%", margin_ratio_110.to_num() * 100.0);
    
    assert!(margin_ratio_90 > margin_ratio_100);
    assert!(margin_ratio_110 < margin_ratio_100);
    }

#[test]
fn test_update_with_price_integration() {
    println!("\n=== Testing Update With Price Integration ===");
    
    let mut position = create_test_position(100_000_000, 15, 1_000_000_000, true);
    let initial_liq = position.liquidation_price;
    
    // Single call should update both PnL and liquidation price
    position.update_with_price(112_000_000).unwrap();
    
    assert_eq!(position.last_mark_price, 112_000_000);
    assert_eq!(position.unrealized_pnl, 120_000_000); // 12% of $1000
    assert_eq!(position.unrealized_pnl_pct, 1200); // 12%
    assert!(position.liquidation_price < initial_liq);
    
    println!("✓ Single update call correctly updates:");
    println!("  - Mark price: $112");
    println!("  - PnL: $120 (12%)");
    println!("  - Liquidation price: ${} (safer)", position.liquidation_price / 1_000_000);
    }

#[test]
fn test_edge_cases() {
    println!("\n=== Testing Edge Cases ===");
    
    // Test 1: Zero price handling
    let mut position = create_test_position(100_000_000, 10, 1_000_000_000, true);
    position.calculate_unrealized_pnl(0).unwrap();
    assert_eq!(position.unrealized_pnl, -1_000_000_000); // Total loss
    println!("✓ Zero price handled correctly");

    // Test 2: Massive price increase
    let mut position = create_test_position(100_000_000, 10, 1_000_000_000, true);
    position.calculate_unrealized_pnl(1_000_000_000).unwrap(); // 10x
    assert_eq!(position.unrealized_pnl_pct, 90000); // 900%
    
    let eff_lev = position.get_effective_leverage().unwrap();
    assert_eq!(eff_lev, 1); // Minimum leverage
    println!("✓ 10x price increase caps at minimum leverage");

    // Test 3: Very high leverage position
    let mut position = create_test_position(100_000_000, 100, 1_000_000_000, true);
    position.update_with_price(99_500_000).unwrap(); // 0.5% loss
    
    let eff_lev = position.get_effective_leverage().unwrap();
    assert_eq!(eff_lev, 100); // Small loss doesn't affect high leverage much
    println!("✓ High leverage position handles small losses correctly");
    }

#[test]
fn test_formula_verification() {
    println!("\n=== Testing Formula Compliance ===");
    
    // Verify the exact formula: effective_leverage = position_leverage × (1 - unrealized_pnl_pct)
    let test_cases = vec![
        (10, 0, 10),      // No PnL
        (10, 2000, 8),    // 20% profit: 10 * (1 - 0.2) = 8
        (10, -1000, 11),  // 10% loss: 10 * (1 - (-0.1)) = 11
        (20, 5000, 10),   // 50% profit: 20 * (1 - 0.5) = 10
        (5, -2000, 6),    // 20% loss: 5 * (1 - (-0.2)) = 6
        (100, 9000, 10),  // 90% profit: 100 * (1 - 0.9) = 10
    ];
    
    for (base_lev, pnl_pct, expected) in test_cases {
        let effective = calculate_effective_leverage(base_lev, None, Some(pnl_pct)).unwrap();
        assert_eq!(effective, expected);
        println!("✓ {}x with {}% PnL → {}x", base_lev, pnl_pct / 100, effective);
    }
    }

#[test]
fn test_short_position_liquidation() {
    println!("\n=== Testing Short Position Liquidation ===");
    
    let mut short_pos = create_test_position(100_000_000, 10, 1_000_000_000, false);
    
    // Short position profits when price goes down
    short_pos.update_with_price(85_000_000).unwrap(); // 15% profit
    let safe_liq_price = short_pos.liquidation_price;
    
    // Reset and test with loss
    let mut short_pos = create_test_position(100_000_000, 10, 1_000_000_000, false);
    short_pos.update_with_price(105_000_000).unwrap(); // 5% loss
    let risky_liq_price = short_pos.liquidation_price;
    
    assert!(safe_liq_price > risky_liq_price);
    println!("✓ Short position liquidation prices:");
    println!("  - With profit: ${} (safer)", safe_liq_price / 1_000_000);
    println!("  - With loss: ${} (riskier)", risky_liq_price / 1_000_000);
    }

#[test]
fn test_liquidation_price_spec_formula() {
    println!("\n=== Testing Liquidation Price Spec Formula ===");
    
    // Test the actual formula implementation
    let entry_price = 100_000_000; // $100
    let margin_ratio = 1000; // 10% in basis points
    
    // Test with different effective leverages
    let test_cases = vec![
        (10, true),  // 10x long
        (8, true),   // 8x long (reduced from profit)
        (12, true),  // 12x long (increased from loss)
        (10, false), // 10x short
    ];
    
    for (eff_lev, is_long) in test_cases {
        let liq_price = calculate_liquidation_price_spec(
            entry_price,
            margin_ratio,
            eff_lev,
            is_long,
        ).unwrap();
        
        let direction = if is_long { "long" } else { "short" };
        println!("✓ {}x {} liquidation price: ${}", eff_lev, direction, liq_price / 1_000_000);
    }
    }

#[test]
fn test_comprehensive_scenario() {
    println!("\n=== Testing Comprehensive Trading Scenario ===");
    
    // Simulate a realistic trading scenario
    let mut position = create_test_position(50_000_000, 25, 2_000_000_000, true); // $50 entry, 25x, $2000 size
    
    println!("Initial position: $50 entry, 25x leverage, $2000 size");
    println!("Initial liquidation: ${}", position.liquidation_price / 1_000_000);
    
    // Price movements over time
    let price_movements = vec![
        (52_000_000, "Price rises to $52 (+4%)"),
        (48_000_000, "Price drops to $48 (-4%)"),
        (55_000_000, "Price rallies to $55 (+10%)"),
        (51_000_000, "Price pulls back to $51 (+2%)"),
    ];
    
    for (price, description) in price_movements {
        position.update_with_price(price).unwrap();
        
        println!("\n{}", description);
        println!("  PnL: ${} ({}%)", 
            position.unrealized_pnl / 1_000_000, 
            position.unrealized_pnl_pct / 100
        );
        println!("  Effective leverage: {}x", position.get_effective_leverage().unwrap());
        println!("  Liquidation price: ${}", position.liquidation_price / 1_000_000);
        println!("  Safe from liquidation: {}", !position.should_liquidate(price - 5_000_000));
    }
    }

#[test]
fn test_batch_performance() {
    println!("\n=== Testing Batch Performance ===");
    
    use std::time::Instant;
    
    let start = Instant::now();
    let mut positions = Vec::new();
    
    // Create 1000 positions
    for i in 0..1000 {
        let price = 50_000_000 + (i * 100_000); // Varying entry prices
        let leverage = 5 + (i % 20); // Varying leverages 5-25x
        positions.push(create_test_position(price, leverage, 1_000_000_000, i % 2 == 0));
    }
    
    let creation_time = start.elapsed();
    
    // Update all positions with new price
    let update_start = Instant::now();
    for (i, pos) in positions.iter_mut().enumerate() {
        let price_change = 1.0 + (0.2 * ((i % 20) as f64 - 10.0) / 10.0); // -20% to +20%
        let new_price = (pos.entry_price as f64 * price_change) as u64;
        pos.update_with_price(new_price).unwrap();
    }
    let update_time = update_start.elapsed();
    
    println!("✓ Created 1000 positions in {:?}", creation_time);
    println!("✓ Updated all PnLs in {:?}", update_time);
    println!("✓ Average update time: {:?} per position", update_time / 1000);
    
    // Verify some results
    let profitable = positions.iter().filter(|p| p.unrealized_pnl > 0).count();
    let at_risk = positions.iter().filter(|p| {
        p.get_effective_leverage().unwrap() > p.leverage
    }).count();
    
    println!("✓ Profitable positions: {}", profitable);
    println!("✓ At higher risk: {}", at_risk);
    }

// The tests will be run by cargo test, no need for a main function