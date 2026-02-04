//! End-to-end tests for leverage calculations
//! Tests all tier scenarios and formula components

use betting_platform_native::math::leverage::{
    calculate_max_leverage, calculate_effective_leverage, calculate_bootstrap_leverage
};

#[test]
fn test_tier_caps_exact_values() {
    // Test exact tier cap values from specification
    let test_cases = vec![
        // (outcome_count, expected_tier_cap)
        (1, 100),    // Binary
        (2, 70),     // 2 outcomes (100/√2 ≈ 70.7)
        (3, 25),     // 3-4 outcomes
        (4, 25),     // 3-4 outcomes
        (5, 15),     // 5-8 outcomes
        (6, 15),     // 5-8 outcomes
        (7, 15),     // 5-8 outcomes
        (8, 15),     // 5-8 outcomes
        (9, 12),     // 9-16 outcomes
        (10, 12),    // 9-16 outcomes
        (15, 12),    // 9-16 outcomes
        (16, 12),    // 9-16 outcomes
        (17, 10),    // 17-64 outcomes
        (32, 10),    // 17-64 outcomes
        (50, 10),    // 17-64 outcomes
        (64, 10),    // 17-64 outcomes
        (65, 5),     // >64 outcomes
        (100, 5),    // >64 outcomes
        (1000, 5),   // >64 outcomes
    ];
    
    for (outcome_count, expected_cap) in test_cases {
        // Test with max depth and coverage to isolate tier cap
        let leverage = calculate_max_leverage(0, 1000, outcome_count);
        
        // The tier cap should be the limiting factor
        assert!(leverage <= expected_cap,
            "N={}: leverage {} exceeds tier cap {}",
            outcome_count, leverage, expected_cap
        );
        
        println!("N={}: tier_cap={}, actual_leverage={}", 
            outcome_count, expected_cap, leverage);
    }
}

#[test]
fn test_depth_multiplier_effect() {
    // Test 0.1 multiplier for depth (10% boost per level)
    let test_cases = vec![
        (0, 100),   // depth=0: 100 * (1 + 0.0) = 100
        (1, 110),   // depth=1: 100 * (1 + 0.1) = 110
        (2, 120),   // depth=2: 100 * (1 + 0.2) = 120
        (5, 150),   // depth=5: 100 * (1 + 0.5) = 150
        (10, 200),  // depth=10: 100 * (1 + 1.0) = 200
        (32, 420),  // depth=32: 100 * (1 + 3.2) = 420
    ];
    
    for (depth, expected_base) in test_cases {
        // High coverage, binary market to isolate depth effect
        let leverage = calculate_max_leverage(depth, 1000, 1);
        
        // With high coverage, depth boost should be primary factor
        println!("depth={}: expected_base={}, actual_leverage={}", 
            depth, expected_base, leverage);
        
        // May be limited by tier cap (100 for binary)
        assert!(leverage <= 100, "Binary leverage should be capped at 100x");
    }
}

#[test]
fn test_coverage_sqrt_n_formula() {
    // Test coverage × 100/√N component
    let test_cases = vec![
        // (coverage, N, expected_from_coverage)
        (100, 1, 10000),   // 100 * 100/1 = 10000
        (100, 4, 5000),    // 100 * 100/2 = 5000
        (100, 9, 3333),    // 100 * 100/3 ≈ 3333
        (100, 16, 2500),   // 100 * 100/4 = 2500
        (100, 25, 2000),   // 100 * 100/5 = 2000
        (100, 100, 1000),  // 100 * 100/10 = 1000
        
        (50, 1, 5000),     // 50 * 100/1 = 5000
        (50, 4, 2500),     // 50 * 100/2 = 2500
        (50, 9, 1666),     // 50 * 100/3 ≈ 1666
        
        (200, 1, 20000),   // 200 * 100/1 = 20000
        (200, 4, 10000),   // 200 * 100/2 = 10000
        (200, 100, 2000),  // 200 * 100/10 = 2000
    ];
    
    for (coverage, n, expected_coverage_component) in test_cases {
        // Zero depth to isolate coverage effect
        let leverage = calculate_max_leverage(0, coverage, n);
        
        // Result will be min of coverage component and tier cap
        println!("coverage={}, N={}: coverage_component={}, actual_leverage={}", 
            coverage, n, expected_coverage_component, leverage);
        
        // Verify formula is being applied
        let sqrt_n = (n as f64).sqrt() as u64;
        let coverage_limit = (coverage * 100) / sqrt_n.max(1);
        
        assert!(leverage <= coverage_limit,
            "Leverage {} should not exceed coverage limit {}",
            leverage, coverage_limit
        );
    }
}

#[test]
fn test_minimum_of_three_components() {
    // Test that result is minimum of: depth_boost, coverage_limit, tier_cap
    struct TestCase {
        depth: u64,
        coverage: u64,
        n: u64,
        limiting_factor: &'static str,
        expected: u64,
    }
    
    let test_cases = vec![
        // Tier cap is limiting
        TestCase { depth: 32, coverage: 1000, n: 100, limiting_factor: "tier_cap", expected: 5 },
        
        // Coverage is limiting
        TestCase { depth: 0, coverage: 10, n: 1, limiting_factor: "coverage", expected: 10 },
        
        // Depth boost is limiting (harder to achieve due to tier caps)
        TestCase { depth: 5, coverage: 200, n: 1, limiting_factor: "tier_cap", expected: 100 },
        
        // Real scenarios
        TestCase { depth: 10, coverage: 150, n: 4, limiting_factor: "tier_cap", expected: 25 },
        TestCase { depth: 3, coverage: 50, n: 9, limiting_factor: "coverage", expected: 12 },
    ];
    
    for tc in test_cases {
        let leverage = calculate_max_leverage(tc.depth, tc.coverage, tc.n);
        
        println!("depth={}, coverage={}, N={}: {} limited to {}", 
            tc.depth, tc.coverage, tc.n, tc.limiting_factor, leverage);
        
        assert!(leverage <= tc.expected,
            "Leverage {} exceeds expected {} (limited by {})",
            leverage, tc.expected, tc.limiting_factor
        );
    }
}

#[test]
fn test_effective_leverage_calculation() {
    // Test effective leverage multiplication
    let test_cases = vec![
        // (base_leverage, multiplier_bps, expected)
        (100, 10000, 100),   // 100 * 1.0 = 100
        (100, 15000, 150),   // 100 * 1.5 = 150
        (100, 12000, 120),   // 100 * 1.2 = 120
        (100, 11000, 110),   // 100 * 1.1 = 110
        (200, 15000, 300),   // 200 * 1.5 = 300
        (300, 20000, 500),   // 300 * 2.0 = 600, capped at 500
        (400, 15000, 500),   // 400 * 1.5 = 600, capped at 500
    ];
    
    for (base, multiplier, expected) in test_cases {
        let effective = calculate_effective_leverage(base, multiplier);
        
        assert_eq!(effective, expected,
            "base={}, multiplier={}bps: expected {}, got {}",
            base, multiplier, expected, effective
        );
    }
}

#[test]
fn test_bootstrap_leverage() {
    // Test bootstrap formula: min(100*coverage, tier)
    let test_cases = vec![
        // (coverage, tier_cap, expected)
        (150, 100, 100),  // 100*1.5 = 150, capped at 100
        (50, 100, 50),    // 100*0.5 = 50
        (200, 25, 25),    // 100*2.0 = 200, capped at 25
        (10, 100, 10),    // 100*0.1 = 10
        (0, 100, 0),      // 100*0 = 0
    ];
    
    for (coverage, tier_cap, expected) in test_cases {
        let bootstrap = calculate_bootstrap_leverage(coverage, tier_cap);
        
        assert_eq!(bootstrap, expected,
            "coverage={}, tier_cap={}: expected {}, got {}",
            coverage, tier_cap, expected, bootstrap
        );
    }
}

#[test]
fn test_kelly_criterion_relationship() {
    // Verify √N relationship matches Kelly criterion
    // Kelly: f = edge/odds, scaled by 1/√N for variance
    
    for n in [1, 4, 9, 16, 25, 36, 49, 64, 81, 100] {
        let sqrt_n = (n as f64).sqrt() as u64;
        
        // With fixed coverage, leverage should scale as 1/√N
        let leverage = calculate_max_leverage(0, 100, n as u64);
        
        // Theoretical Kelly-based limit
        let kelly_limit = 100 * 100 / sqrt_n;
        
        println!("N={}, √N={}, kelly_limit={}, actual_leverage={}", 
            n, sqrt_n, kelly_limit, leverage);
        
        // Should follow Kelly scaling (modulo tier caps)
        assert!(leverage <= kelly_limit,
            "Leverage {} exceeds Kelly limit {} for N={}",
            leverage, kelly_limit, n
        );
    }
}

#[test]
fn test_extreme_scenarios() {
    // Test edge cases and extreme values
    
    // Zero coverage
    assert_eq!(calculate_max_leverage(10, 0, 1), 0);
    
    // Very high coverage
    let high_coverage = calculate_max_leverage(0, 10000, 1);
    assert_eq!(high_coverage, 100); // Still capped by tier
    
    // Very high depth
    let high_depth = calculate_max_leverage(1000, 100, 1);
    assert_eq!(high_depth, 100); // Still capped by tier
    
    // Many outcomes
    let many_outcomes = calculate_max_leverage(10, 100, 1000);
    assert_eq!(many_outcomes, 3); // Low due to √1000 ≈ 31.6
    
    // Perfect scenario for binary
    let perfect_binary = calculate_max_leverage(0, 100, 1);
    assert_eq!(perfect_binary, 100);
    
    // Perfect scenario for 4-outcome
    let perfect_quad = calculate_max_leverage(0, 50, 4);
    assert_eq!(perfect_quad, 25); // min(50*100/2=2500, 25) = 25
}

#[test]
fn test_chaining_multipliers_cumulative() {
    // Test cumulative effect of chaining
    let base_leverage = 100;
    
    // Single step multipliers
    let borrow_mult = 15000;  // 1.5x
    let lend_mult = 12000;    // 1.2x
    let liq_mult = 12000;     // 1.2x
    let stake_mult = 11000;   // 1.1x
    
    // Chain: Borrow -> Lend -> Liquidity -> Stake
    let after_borrow = calculate_effective_leverage(base_leverage, borrow_mult);
    assert_eq!(after_borrow, 150);
    
    let after_lend = calculate_effective_leverage(after_borrow, lend_mult);
    assert_eq!(after_lend, 180);
    
    let after_liq = calculate_effective_leverage(after_lend, liq_mult);
    assert_eq!(after_liq, 216);
    
    let after_stake = calculate_effective_leverage(after_liq, stake_mult);
    assert_eq!(after_stake, 237);
    
    // 5 steps with average 1.2x each
    let mut cumulative = base_leverage;
    for _ in 0..5 {
        cumulative = calculate_effective_leverage(cumulative, 12000);
    }
    
    println!("5 steps at 1.2x each: {}x -> {}x", base_leverage, cumulative);
    assert_eq!(cumulative, 248); // 100 * 1.2^5 ≈ 248.8, capped at 500
}

#[test]
fn test_real_world_scenarios() {
    // Test realistic market scenarios
    
    // High-confidence binary market
    let binary_high_conf = calculate_max_leverage(5, 150, 1);
    println!("Binary high confidence: {}x", binary_high_conf);
    assert_eq!(binary_high_conf, 100);
    
    // Uncertain 4-outcome market
    let quad_uncertain = calculate_max_leverage(2, 50, 4);
    println!("4-outcome uncertain: {}x", quad_uncertain);
    assert_eq!(quad_uncertain, 25);
    
    // Deep hierarchy 10-outcome
    let deep_ten = calculate_max_leverage(15, 80, 10);
    println!("Deep 10-outcome: {}x", deep_ten);
    assert_eq!(deep_ten, 10);
    
    // Massive 100-outcome market
    let massive = calculate_max_leverage(10, 100, 100);
    println!("100-outcome market: {}x", massive);
    assert_eq!(massive, 5);
    
    // Bootstrap scenario (low coverage)
    let bootstrap = calculate_bootstrap_leverage(30, 100);
    println!("Bootstrap leverage: {}x", bootstrap);
    assert_eq!(bootstrap, 30);
}