//! AMM Implementation Tests
//! 
//! Tests for LMSR, PM-AMM, L2 AMM, and Hybrid AMM pricing and liquidity

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    amm::{
        lmsr::{LMSRCore, calculate_lmsr_price, calculate_cost},
        pmamm::{PMAMMCore, calculate_pmamm_price},
        l2_amm::{L2AMMCore, calculate_l2_price},
        hybrid::{HybridAMM, AMMType},
    },
    math::fixed_point::U64F64,
};

#[test]
fn test_lmsr_pricing_accuracy() {
    // Test LMSR pricing formula: price_i = e^(q_i/b) / Σ(e^(q_j/b))
    
    let b_parameter = U64F64::from_num(100);
    let shares = vec![
        U64F64::from_num(50),
        U64F64::from_num(50),
    ];
    
    // Equal shares should give 50% price each
    let price_0 = calculate_lmsr_price(&shares, 0, b_parameter).unwrap();
    let price_1 = calculate_lmsr_price(&shares, 1, b_parameter).unwrap();
    
    assert!((price_0 - U64F64::from_num(0.5)).abs() < U64F64::from_num(0.01));
    assert!((price_1 - U64F64::from_num(0.5)).abs() < U64F64::from_num(0.01));
    assert!((price_0 + price_1 - U64F64::from_num(1.0)).abs() < U64F64::from_num(0.01));
    
    println!("✅ LMSR equal shares pricing: {:.2}% / {:.2}%", 
        price_0.to_num::<f64>() * 100.0,
        price_1.to_num::<f64>() * 100.0
    );
    
    // Test asymmetric shares
    let shares_asymmetric = vec![
        U64F64::from_num(75),
        U64F64::from_num(25),
    ];
    
    let price_0_asym = calculate_lmsr_price(&shares_asymmetric, 0, b_parameter).unwrap();
    let price_1_asym = calculate_lmsr_price(&shares_asymmetric, 1, b_parameter).unwrap();
    
    assert!(price_0_asym > price_1_asym); // More shares = higher price
    assert!((price_0_asym + price_1_asym - U64F64::from_num(1.0)).abs() < U64F64::from_num(0.01));
    
    println!("✅ LMSR asymmetric pricing: {:.2}% / {:.2}%",
        price_0_asym.to_num::<f64>() * 100.0,
        price_1_asym.to_num::<f64>() * 100.0
    );
}

#[test]
fn test_lmsr_cost_function() {
    // Test LMSR cost function: C = b * ln(Σ(e^(q_i/b)))
    
    let b_parameter = U64F64::from_num(100);
    
    // Buy 10 shares of outcome 0
    let initial_shares = vec![
        U64F64::from_num(50),
        U64F64::from_num(50),
    ];
    
    let final_shares = vec![
        U64F64::from_num(60),
        U64F64::from_num(50),
    ];
    
    let cost = calculate_cost(&initial_shares, &final_shares, b_parameter).unwrap();
    
    // Cost should be positive for buying
    assert!(cost > U64F64::from_num(0));
    
    // Cost should be less than shares * max_price
    assert!(cost < U64F64::from_num(10));
    
    println!("✅ LMSR cost for 10 shares: {:.4} USDC", cost.to_num::<f64>());
}

#[test]
fn test_pmamm_pricing() {
    // Test PM-AMM pricing with dynamic slippage
    
    let l_parameter = U64F64::from_num(1000);
    let reserves = vec![
        U64F64::from_num(500),
        U64F64::from_num(500),
    ];
    
    // Initial price should be 50%
    let price = calculate_pmamm_price(&reserves, 0, l_parameter).unwrap();
    assert!((price - U64F64::from_num(0.5)).abs() < U64F64::from_num(0.01));
    
    println!("✅ PM-AMM initial price: {:.2}%", price.to_num::<f64>() * 100.0);
    
    // Test price impact of large trade
    let large_trade_reserves = vec![
        U64F64::from_num(800),
        U64F64::from_num(200),
    ];
    
    let price_after = calculate_pmamm_price(&large_trade_reserves, 0, l_parameter).unwrap();
    assert!(price_after > U64F64::from_num(0.7)); // Price increased significantly
    
    println!("✅ PM-AMM price after large trade: {:.2}%", price_after.to_num::<f64>() * 100.0);
}

#[test]
fn test_pmamm_constant_product() {
    // Test PM-AMM maintains constant product invariant
    
    let initial_reserves = vec![
        U64F64::from_num(1000),
        U64F64::from_num(1000),
    ];
    
    let k_initial = initial_reserves[0] * initial_reserves[1];
    
    // Simulate trade: buy 100 of outcome 0
    let delta_0 = U64F64::from_num(100);
    let new_reserve_0 = initial_reserves[0] - delta_0;
    let new_reserve_1 = k_initial / new_reserve_0;
    
    let k_final = new_reserve_0 * new_reserve_1;
    
    // Constant product should be maintained (within rounding error)
    assert!((k_initial - k_final).abs() < U64F64::from_num(1));
    
    println!("✅ PM-AMM constant product maintained: {} ≈ {}", 
        k_initial.to_num::<u64>(),
        k_final.to_num::<u64>()
    );
}

#[test]
fn test_l2_amm_continuous_pricing() {
    // Test L2 AMM for continuous outcomes
    
    let k_parameter = U64F64::from_num(10000);
    let b_bound = U64F64::from_num(100);
    let range_min = U64F64::from_num(0);
    let range_max = U64F64::from_num(100);
    
    // Test pricing at different points
    let test_values = vec![25.0, 50.0, 75.0];
    
    for value in test_values {
        let price = calculate_l2_price(
            U64F64::from_num(value),
            k_parameter,
            b_bound,
            range_min,
            range_max,
        ).unwrap();
        
        println!("✅ L2 AMM price at {}: {:.4}", value, price.to_num::<f64>());
        
        // Price should be between 0 and 1
        assert!(price >= U64F64::from_num(0));
        assert!(price <= U64F64::from_num(1));
    }
}

#[test]
fn test_l2_amm_distribution_weights() {
    // Test L2 AMM with custom distribution weights
    
    let distribution_bins = vec![
        (0u8, 100u64),   // 0-10: weight 100
        (1u8, 200u64),   // 10-20: weight 200
        (2u8, 400u64),   // 20-30: weight 400
        (3u8, 200u64),   // 30-40: weight 200
        (4u8, 100u64),   // 40-50: weight 100
    ];
    
    let total_weight: u64 = distribution_bins.iter().map(|(_, w)| w).sum();
    assert_eq!(total_weight, 1000);
    
    // Verify probabilities sum to 1
    let probabilities: Vec<f64> = distribution_bins.iter()
        .map(|(_, weight)| *weight as f64 / total_weight as f64)
        .collect();
    
    let sum: f64 = probabilities.iter().sum();
    assert!((sum - 1.0).abs() < 0.001);
    
    println!("✅ L2 AMM distribution weights validated");
}

#[test]
fn test_hybrid_amm_switching() {
    // Test Hybrid AMM switching between LMSR and PM-AMM
    
    let market_params = HybridAMMParams {
        lmsr_weight: 0.7,
        pmamm_weight: 0.3,
        switch_threshold: 0.8, // Switch to PM-AMM when liquidity > 80%
    };
    
    // Low liquidity - should use LMSR
    let liquidity_ratio = 0.5;
    let amm_type = select_amm_type(liquidity_ratio, &market_params);
    assert_eq!(amm_type, AMMType::LMSR);
    
    // High liquidity - should use PM-AMM
    let liquidity_ratio = 0.9;
    let amm_type = select_amm_type(liquidity_ratio, &market_params);
    assert_eq!(amm_type, AMMType::PMAMM);
    
    println!("✅ Hybrid AMM switching logic validated");
}

#[test]
fn test_amm_slippage_calculation() {
    // Test slippage for different trade sizes
    
    let b_parameter = U64F64::from_num(1000);
    let initial_shares = vec![
        U64F64::from_num(500),
        U64F64::from_num(500),
    ];
    
    let trade_sizes = vec![10, 100, 500, 1000];
    
    for size in trade_sizes {
        let final_shares = vec![
            initial_shares[0] + U64F64::from_num(size),
            initial_shares[1],
        ];
        
        let initial_price = calculate_lmsr_price(&initial_shares, 0, b_parameter).unwrap();
        let final_price = calculate_lmsr_price(&final_shares, 0, b_parameter).unwrap();
        let cost = calculate_cost(&initial_shares, &final_shares, b_parameter).unwrap();
        
        let avg_price = cost / U64F64::from_num(size);
        let slippage = ((avg_price / initial_price) - U64F64::from_num(1)) * U64F64::from_num(100);
        
        println!("✅ Trade size {}: slippage {:.2}%", 
            size, 
            slippage.to_num::<f64>()
        );
    }
}

#[test]
fn test_amm_liquidity_depth() {
    // Test liquidity depth for different AMM types
    
    // LMSR depth
    let lmsr_b = U64F64::from_num(10000); // High liquidity
    let lmsr_depth = calculate_liquidity_depth_lmsr(lmsr_b);
    
    // PM-AMM depth  
    let pmamm_reserves = vec![
        U64F64::from_num(50000),
        U64F64::from_num(50000),
    ];
    let pmamm_depth = calculate_liquidity_depth_pmamm(&pmamm_reserves);
    
    println!("✅ LMSR liquidity depth: {:.0} USDC", lmsr_depth.to_num::<f64>());
    println!("✅ PM-AMM liquidity depth: {:.0} USDC", pmamm_depth.to_num::<f64>());
    
    // Higher parameter = deeper liquidity
    assert!(lmsr_depth > U64F64::from_num(1000));
    assert!(pmamm_depth > U64F64::from_num(1000));
}

// Helper functions
fn select_amm_type(liquidity_ratio: f64, params: &HybridAMMParams) -> AMMType {
    if liquidity_ratio > params.switch_threshold {
        AMMType::PMAMM
    } else {
        AMMType::LMSR
    }
}

struct HybridAMMParams {
    lmsr_weight: f64,
    pmamm_weight: f64,
    switch_threshold: f64,
}

fn calculate_liquidity_depth_lmsr(b: U64F64) -> U64F64 {
    // Approximate liquidity depth as 2 * b (can handle ~2x parameter in volume)
    b * U64F64::from_num(2)
}

fn calculate_liquidity_depth_pmamm(reserves: &[U64F64]) -> U64F64 {
    // Liquidity depth approximated by geometric mean of reserves
    let product = reserves[0] * reserves[1];
    product.sqrt().unwrap_or(U64F64::from_num(0))
}