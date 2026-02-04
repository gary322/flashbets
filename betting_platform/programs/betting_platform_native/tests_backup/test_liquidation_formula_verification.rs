//! Tests for verifying the liquidation formula implementation
//! Specification: liq_price = entry_price * (1 - (margin_ratio / lev_eff))

use betting_platform_native::{
    liquidation::{
        calculate_liquidation_price_spec,
        calculate_margin_ratio_spec,
        calculate_effective_leverage,
        verify_liquidation_calculation,
    },
    trading::helpers::{
        calculate_liquidation_price,
        calculate_margin_ratio,
    },
};

#[test]
fn test_liquidation_formula_spec_compliance() {
    println!("\n=== Testing Liquidation Formula Spec Compliance ===");
    
    // Test parameters
    let entry_price = 5_000_000_000; // $5000
    let base_leverage = 10;
    let sigma = 150; // 1.5% in basis points
    let num_positions = 1;
    
    // Calculate margin ratio
    let margin_ratio = calculate_margin_ratio_spec(base_leverage, sigma, num_positions).unwrap();
    println!("Margin Ratio: {}bps ({:.2}%)", margin_ratio, margin_ratio as f64 / 100.0);
    
    // Test with no chain multiplier (effective leverage = base leverage)
    let effective_leverage = calculate_effective_leverage(base_leverage, None).unwrap();
    assert_eq!(effective_leverage, base_leverage);
    
    // Calculate liquidation price using spec formula
    let liq_price_long = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio,
        effective_leverage,
        true,
    ).unwrap();
    
    let liq_price_short = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio,
        effective_leverage,
        false,
    ).unwrap();
    
    println!("\nSpec Formula Results (10x leverage):");
    println!("  Entry Price: ${}", entry_price / 1_000_000);
    println!("  Long Liquidation:  ${} ({:.2}% buffer)", 
        liq_price_long / 1_000_000,
        ((entry_price - liq_price_long) as f64 / entry_price as f64) * 100.0
    );
    println!("  Short Liquidation: ${} ({:.2}% buffer)",
        liq_price_short / 1_000_000,
        ((liq_price_short - entry_price) as f64 / entry_price as f64) * 100.0
    );
    
    // Compare with existing implementation
    let existing_liq_long = calculate_liquidation_price(entry_price, base_leverage, true).unwrap();
    let existing_liq_short = calculate_liquidation_price(entry_price, base_leverage, false).unwrap();
    
    println!("\nExisting Formula Results:");
    println!("  Long Liquidation:  ${}", existing_liq_long / 1_000_000);
    println!("  Short Liquidation: ${}", existing_liq_short / 1_000_000);
    
    println!("\nDifference:");
    println!("  Long:  ${}", (liq_price_long as i64 - existing_liq_long as i64).abs() / 1_000_000);
    println!("  Short: ${}", (liq_price_short as i64 - existing_liq_short as i64).abs() / 1_000_000);
}

#[test]
fn test_liquidation_with_chain_multiplier() {
    println!("\n=== Testing Liquidation with Chain Multiplier ===");
    
    let entry_price = 1_000_000_000; // $1000
    let base_leverage = 10;
    let chain_multiplier = 20000; // 2x multiplier
    let sigma = 150;
    let num_positions = 1;
    
    // Calculate effective leverage with chain
    let effective_leverage = calculate_effective_leverage(base_leverage, Some(chain_multiplier)).unwrap();
    assert_eq!(effective_leverage, 20); // 10x * 2x = 20x
    
    let margin_ratio = calculate_margin_ratio_spec(base_leverage, sigma, num_positions).unwrap();
    
    // Verify liquidation calculation
    let verification = verify_liquidation_calculation(
        entry_price,
        base_leverage,
        effective_leverage,
        sigma,
        num_positions,
        true,
    ).unwrap();
    
    verification.log_details();
    
    // With 20x effective leverage, liquidation should be very close
    assert!(verification.liquidation_percentage < 1000); // Less than 10% buffer
}

#[test]
fn test_extreme_leverage_scenarios() {
    println!("\n=== Testing Extreme Leverage Scenarios ===");
    
    let entry_price = 100_000_000; // $100
    let scenarios = vec![
        (50, 50, "50x leverage"),
        (100, 100, "100x leverage"),
        (100, 500, "100x with 5x chain = 500x capped"),
    ];
    
    for (base_lev, expected_eff, desc) in scenarios {
        println!("\nScenario: {}", desc);
        
        let chain_mult = if base_lev == 100 && expected_eff == 500 {
            Some(50000) // 5x multiplier
        } else {
            None
        };
        
        let effective_leverage = calculate_effective_leverage(base_lev, chain_mult).unwrap();
        assert_eq!(effective_leverage, expected_eff);
        
        let margin_ratio = calculate_margin_ratio_spec(base_lev, 150, 1).unwrap();
        
        let liq_price = calculate_liquidation_price_spec(
            entry_price,
            margin_ratio,
            effective_leverage,
            true,
        ).unwrap();
        
        let buffer_pct = ((entry_price - liq_price) as f64 / entry_price as f64) * 100.0;
        println!("  Liquidation buffer: {:.2}%", buffer_pct);
        
        // At extreme leverage, buffer should be very small
        if effective_leverage >= 100 {
            assert!(buffer_pct < 2.0, "Buffer too large for extreme leverage");
        }
    }
}

#[test]
fn test_margin_ratio_calculation() {
    println!("\n=== Testing Margin Ratio Calculation ===");
    
    let test_cases = vec![
        (1, 1, "1x leverage, 1 position"),
        (10, 1, "10x leverage, 1 position"),
        (10, 5, "10x leverage, 5 positions"),
        (50, 1, "50x leverage, 1 position"),
        (100, 1, "100x leverage, 1 position"),
    ];
    
    for (leverage, positions, desc) in test_cases {
        let margin_ratio = calculate_margin_ratio_spec(leverage, 150, positions).unwrap();
        
        // Components:
        let base_margin = 10000 / leverage;
        let f_n = 10000 + 1000 * (positions - 1);
        let sqrt_lev = (leverage as f64).sqrt() as u64;
        let volatility = (150 * sqrt_lev * f_n) / (10000 * 10000);
        
        println!("\n{}", desc);
        println!("  Base margin (1/lev): {}bps", base_margin);
        println!("  Volatility component: {}bps", volatility);
        println!("  Total margin ratio: {}bps ({:.2}%)", margin_ratio, margin_ratio as f64 / 100.0);
        
        assert_eq!(margin_ratio, base_margin + volatility);
    }
}

#[test]
fn test_liquidation_price_boundaries() {
    println!("\n=== Testing Liquidation Price Boundaries ===");
    
    let entry_price = 1_000_000; // $1
    
    // Test with very low leverage (should have large buffer)
    let low_lev = 2;
    let margin_ratio_low = calculate_margin_ratio_spec(low_lev, 150, 1).unwrap();
    let liq_low = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio_low,
        low_lev,
        true,
    ).unwrap();
    
    let buffer_low = ((entry_price - liq_low) as f64 / entry_price as f64) * 100.0;
    println!("2x leverage liquidation buffer: {:.2}%", buffer_low);
    assert!(buffer_low > 20.0, "Low leverage should have large buffer");
    
    // Test with very high leverage (should have tiny buffer)
    let high_lev = 100;
    let margin_ratio_high = calculate_margin_ratio_spec(high_lev, 150, 1).unwrap();
    let liq_high = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio_high,
        high_lev,
        true,
    ).unwrap();
    
    let buffer_high = ((entry_price - liq_high) as f64 / entry_price as f64) * 100.0;
    println!("100x leverage liquidation buffer: {:.2}%", buffer_high);
    assert!(buffer_high < 2.0, "High leverage should have tiny buffer");
    
    // Ensure liquidation prices make sense
    assert!(liq_low < entry_price, "Long liquidation should be below entry");
    assert!(liq_high < entry_price, "Long liquidation should be below entry");
    assert!(liq_low < liq_high, "Lower leverage should liquidate at lower price");
}

fn main() {
    println!("Running liquidation formula verification tests...\n");
    
    test_liquidation_formula_spec_compliance();
    test_liquidation_with_chain_multiplier();
    test_extreme_leverage_scenarios();
    test_margin_ratio_calculation();
    test_liquidation_price_boundaries();
    
    println!("\n✅ All liquidation formula tests passed!");
}