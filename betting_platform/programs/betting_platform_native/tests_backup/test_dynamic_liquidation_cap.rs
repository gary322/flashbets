//! Tests for dynamic liquidation cap calculation
//! Verifies: clamp(LIQ_CAP_MIN, SIGMA_FACTOR*σ, LIQ_CAP_MAX)*OI

use solana_program_test::*;
use betting_platform_native::{
    keeper_liquidation::{LiquidationKeeper, LIQ_CAP_MIN, LIQ_CAP_MAX, SIGMA_FACTOR},
    math::U64F64,
};

#[tokio::test]
async fn test_dynamic_cap_basic() {
    let open_interest = 1_000_000_000; // 1000 USDC
    let volatility = U64F64::from_num(20); // 20% volatility
    
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(volatility, open_interest)
        .unwrap();
    
    // Expected: clamp(200, 150 * 20, 800) * 1000 / 10000
    // = clamp(200, 3000, 800) * 1000 / 10000
    // = 800 * 1000 / 10000 = 80 USDC
    
    println!("Dynamic cap for 20% volatility, 1000 USDC OI: {} USDC", cap / 1_000_000);
    assert!(cap > 0, "Cap should be positive");
}

#[tokio::test]
async fn test_cap_clamping_min() {
    let open_interest = 1_000_000_000;
    let low_volatility = U64F64::from_num(1); // 1% volatility
    
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(low_volatility, open_interest)
        .unwrap();
    
    // With very low volatility, should clamp to LIQ_CAP_MIN
    let expected_cap = (LIQ_CAP_MIN as u128 * open_interest as u128 / 10000) as u64;
    
    assert_eq!(cap, expected_cap, "Should clamp to minimum cap");
    println!("Low volatility cap: {} USDC (clamped to {}%)", 
        cap / 1_000_000, LIQ_CAP_MIN as f64 / 100.0);
}

#[tokio::test]
async fn test_cap_clamping_max() {
    let open_interest = 1_000_000_000;
    let high_volatility = U64F64::from_num(100); // 100% volatility
    
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(high_volatility, open_interest)
        .unwrap();
    
    // With very high volatility, should clamp to LIQ_CAP_MAX
    let expected_cap = (LIQ_CAP_MAX as u128 * open_interest as u128 / 10000) as u64;
    
    assert_eq!(cap, expected_cap, "Should clamp to maximum cap");
    println!("High volatility cap: {} USDC (clamped to {}%)", 
        cap / 1_000_000, LIQ_CAP_MAX as f64 / 100.0);
}

#[tokio::test]
async fn test_cap_scaling_with_oi() {
    let volatility = U64F64::from_num(30); // 30% volatility
    let oi_values = vec![
        100_000_000,     // 100 USDC
        1_000_000_000,   // 1,000 USDC
        10_000_000_000,  // 10,000 USDC
        100_000_000_000, // 100,000 USDC
    ];
    
    println!("\nDynamic cap scaling with Open Interest (30% volatility):");
    for oi in oi_values {
        let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(volatility, oi).unwrap();
        let cap_pct = (cap as f64 / oi as f64) * 100.0;
        println!("  OI: ${:>7} -> Cap: ${:>7} ({:.1}%)", 
            oi / 1_000_000, cap / 1_000_000, cap_pct);
        
        // Cap percentage should be consistent
        assert!(cap_pct >= LIQ_CAP_MIN as f64 / 100.0 && cap_pct <= LIQ_CAP_MAX as f64 / 100.0,
            "Cap percentage out of bounds");
    }
}

#[tokio::test]
async fn test_volatility_impact() {
    let open_interest = 10_000_000_000; // 10,000 USDC
    let volatilities = vec![5, 10, 20, 30, 40, 50, 60];
    
    println!("\nCap vs Volatility (OI = $10,000):");
    println!("Volatility | Cap Amount | Cap %");
    println!("-----------|------------|------");
    
    for vol in volatilities {
        let volatility = U64F64::from_num(vol);
        let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(volatility, open_interest)
            .unwrap();
        let cap_pct = (cap as f64 / open_interest as f64) * 100.0;
        
        println!("{:>9}% | ${:>9} | {:>5.1}%", vol, cap / 1_000_000, cap_pct);
    }
}

#[tokio::test]
async fn test_sigma_factor_in_cap() {
    let open_interest = 1_000_000_000;
    
    // Test at the boundary where SIGMA_FACTOR * volatility = LIQ_CAP_MIN
    // 150 * volatility = 200 => volatility = 200/150 = 1.33%
    let boundary_vol = U64F64::from_num(200) / U64F64::from_num(150);
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(boundary_vol, open_interest)
        .unwrap();
    
    let expected = (LIQ_CAP_MIN as u128 * open_interest as u128 / 10000) as u64;
    assert_eq!(cap, expected, "At boundary, should equal minimum cap");
    
    println!("\nBoundary test (volatility = {:.2}%):", boundary_vol.to_num() as f64);
    println!("  Expected cap: {} USDC", expected / 1_000_000);
    println!("  Actual cap:   {} USDC", cap / 1_000_000);
}

#[tokio::test]
async fn test_extreme_values() {
    // Test with zero volatility
    let zero_vol = U64F64::from_num(0);
    let oi = 1_000_000_000;
    
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(zero_vol, oi).unwrap();
    assert_eq!(cap, (LIQ_CAP_MIN as u128 * oi as u128 / 10000) as u64,
        "Zero volatility should give minimum cap");
    
    // Test with very large OI
    let large_oi = u64::MAX / 10000; // Prevent overflow
    let normal_vol = U64F64::from_num(25);
    
    let large_cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(normal_vol, large_oi);
    assert!(large_cap.is_ok(), "Should handle large OI without overflow");
    
    println!("\nExtreme value tests passed:");
    println!("  Zero volatility: {} USDC cap", cap / 1_000_000);
    println!("  Large OI: calculation successful");
}

#[tokio::test]
async fn test_cap_per_slot_enforcement() {
    // Verify that caps are per-slot as specified (2-8% OI/slot)
    let oi = 50_000_000_000; // 50,000 USDC
    let volatility = U64F64::from_num(35); // 35% volatility
    
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(volatility, oi).unwrap();
    let cap_pct = (cap as f64 / oi as f64) * 100.0;
    
    println!("\nPer-slot cap enforcement test:");
    println!("  Open Interest: $50,000");
    println!("  Volatility: 35%");
    println!("  Liquidation cap: ${} ({:.1}% of OI)", cap / 1_000_000, cap_pct);
    println!("  ✓ Within 2-8% range per slot");
    
    assert!(cap_pct >= 2.0 && cap_pct <= 8.0, "Cap not within 2-8% range");
}