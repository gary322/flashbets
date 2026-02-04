//! Tests for liquidation formula implementation
//! Verifies: MR = 1/lev + sigma * sqrt(lev) * f(n)

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    trading::helpers::{calculate_margin_ratio, calculate_liquidation_price},
    keeper_liquidation::{SIGMA_FACTOR, LIQ_CAP_MIN, LIQ_CAP_MAX},
};

#[tokio::test]
async fn test_margin_ratio_calculation() {
    // Test single position (n=1)
    let margin_ratio = calculate_margin_ratio(10, 1).unwrap();
    
    // Expected: 1/10 + sigma * sqrt(10) * f(1)
    // Base margin = 10000/10 = 1000 bps
    // f(1) = 10000 + 1000 * 0 = 10000
    // sqrt(10) ≈ 3
    // volatility = (150 * 3 * 10000) / (10000 * 10000) = 0.045 bps
    let expected_base = 1000; // 10% in bps
    
    assert!(margin_ratio >= expected_base, "Margin ratio should include base margin");
    println!("Margin ratio for 10x leverage, 1 position: {} bps", margin_ratio);
}

#[tokio::test]
async fn test_margin_ratio_with_multiple_positions() {
    // Test with multiple positions
    let margin_ratio_1 = calculate_margin_ratio(10, 1).unwrap();
    let margin_ratio_5 = calculate_margin_ratio(10, 5).unwrap();
    
    // f(5) > f(1), so margin ratio should be higher
    assert!(margin_ratio_5 > margin_ratio_1, "More positions should require higher margin");
    
    println!("Margin ratio comparison:");
    println!("  1 position:  {} bps", margin_ratio_1);
    println!("  5 positions: {} bps", margin_ratio_5);
}

#[tokio::test]
async fn test_liquidation_price_calculation() {
    let entry_price = 50000; // $0.50 in basis points
    let leverage = 10;
    
    // Test long position
    let liq_price_long = calculate_liquidation_price(entry_price, leverage, true).unwrap();
    assert!(liq_price_long < entry_price, "Long liquidation price should be below entry");
    
    // Test short position
    let liq_price_short = calculate_liquidation_price(entry_price, leverage, false).unwrap();
    assert!(liq_price_short > entry_price, "Short liquidation price should be above entry");
    
    println!("Liquidation prices for entry at $0.50:");
    println!("  Long:  ${:.4}", liq_price_long as f64 / 10000.0);
    println!("  Short: ${:.4}", liq_price_short as f64 / 10000.0);
}

#[tokio::test]
async fn test_high_leverage_margin_requirements() {
    // Test extreme leverages
    let leverages = vec![1, 5, 10, 20, 50, 100];
    
    println!("\nMargin requirements by leverage:");
    for lev in leverages {
        let margin_ratio = calculate_margin_ratio(lev, 1).unwrap();
        let margin_pct = margin_ratio as f64 / 100.0;
        println!("  {}x leverage: {:.2}% margin required", lev, margin_pct);
        
        // Verify minimum margin is respected
        assert!(margin_ratio >= 10000 / lev, "Base margin calculation error");
    }
}

#[tokio::test]
async fn test_sigma_factor_impact() {
    // Verify SIGMA_FACTOR is applied correctly
    assert_eq!(SIGMA_FACTOR, 150, "SIGMA_FACTOR should be 150 bps (1.5)");
    
    // Calculate with different sigma values
    let base_leverage = 20;
    let margin_ratio = calculate_margin_ratio(base_leverage, 1).unwrap();
    
    // Base margin = 10000/20 = 500 bps (5%)
    // With sigma factor, should be higher
    assert!(margin_ratio > 500, "Margin should include volatility component");
    
    println!("\nSigma factor impact on 20x leverage:");
    println!("  Base margin: 5.00%");
    println!("  With sigma:  {:.2}%", margin_ratio as f64 / 100.0);
}

#[tokio::test]
async fn test_liquidation_constants() {
    // Verify all constants match specification
    assert_eq!(LIQ_CAP_MIN, 200, "LIQ_CAP_MIN should be 200 bps (2%)");
    assert_eq!(LIQ_CAP_MAX, 800, "LIQ_CAP_MAX should be 800 bps (8%)");
    
    println!("\nLiquidation constants verified:");
    println!("  SIGMA_FACTOR: {} bps", SIGMA_FACTOR);
    println!("  LIQ_CAP_MIN:  {} bps", LIQ_CAP_MIN);
    println!("  LIQ_CAP_MAX:  {} bps", LIQ_CAP_MAX);
}

#[tokio::test]
async fn test_edge_cases() {
    // Test edge case: leverage = 1
    let margin_ratio = calculate_margin_ratio(1, 1).unwrap();
    assert_eq!(margin_ratio, 10000 + (SIGMA_FACTOR * 1 * 10000) / (10000 * 10000), 
        "Leverage 1 should give 100% + volatility component");
    
    // Test edge case: many positions
    let margin_ratio_many = calculate_margin_ratio(10, 100).unwrap();
    println!("\nEdge case - 100 positions at 10x leverage:");
    println!("  Margin required: {:.2}%", margin_ratio_many as f64 / 100.0);
    
    // Verify it's reasonable (not exceeding 100%)
    assert!(margin_ratio_many < 10000, "Margin ratio should not exceed 100%");
}

#[tokio::test]
async fn test_liquidation_symmetry() {
    let entry_price = 100000; // $1.00
    let leverage = 10;
    
    let liq_long = calculate_liquidation_price(entry_price, leverage, true).unwrap();
    let liq_short = calculate_liquidation_price(entry_price, leverage, false).unwrap();
    
    // Calculate distances from entry
    let long_distance = entry_price - liq_long;
    let short_distance = liq_short - entry_price;
    
    // With proper formula, distances might not be exactly equal due to volatility component
    // but should be similar
    let ratio = long_distance as f64 / short_distance as f64;
    assert!(ratio > 0.9 && ratio < 1.1, "Liquidation distances should be similar");
    
    println!("\nLiquidation symmetry test:");
    println!("  Entry: $1.00");
    println!("  Long liquidation:  ${:.4} (distance: ${:.4})", 
        liq_long as f64 / 10000.0, long_distance as f64 / 10000.0);
    println!("  Short liquidation: ${:.4} (distance: ${:.4})", 
        liq_short as f64 / 10000.0, short_distance as f64 / 10000.0);
}