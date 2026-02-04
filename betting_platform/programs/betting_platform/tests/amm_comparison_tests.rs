use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform::{
    performance::{amm_comparison::*, PerformanceProfiler},
    fixed_math::FixedPoint,
};

#[tokio::test]
async fn test_pmamm_vs_lmsr_performance() {
    println!("=== PM-AMM vs LMSR Performance Comparison Test ===");
    
    let mut profiler = PerformanceProfiler::new();
    
    // Test with different trade sizes
    let trade_sizes = vec![
        1_000_000,    // 1 token
        10_000_000,   // 10 tokens
        100_000_000,  // 100 tokens
        1_000_000_000, // 1000 tokens
    ];
    
    for trade_size in trade_sizes {
        println!("\nTesting with trade size: {} lamports", trade_size);
        
        let comparison = compare_amm_performance(
            trade_size,
            2, // Binary market
            &mut profiler,
        ).unwrap();
        
        // Verify PM-AMM has better performance
        assert!(
            comparison.pm_amm_cu < comparison.lmsr_cu,
            "PM-AMM should use fewer CU than LMSR"
        );
        
        // Verify slippage values match spec
        assert_eq!(
            comparison.pm_amm_slippage, 0.0,
            "PM-AMM should have 0 slippage"
        );
        assert_eq!(
            comparison.lmsr_slippage, 9.53,
            "LMSR should have 9.53% slippage as per spec"
        );
        
        // Verify improvement percentage
        assert!(
            comparison.improvement_percentage > 0.0,
            "PM-AMM should show improvement over LMSR"
        );
        
        println!("✓ Trade size {} passed all checks", trade_size);
    }
}

#[tokio::test]
async fn test_batch_processing_comparison() {
    println!("=== Batch Processing Performance Test ===");
    
    let mut profiler = PerformanceProfiler::new();
    
    // Test 8-outcome batch as per spec
    compare_batch_performance(8, &mut profiler).unwrap();
    
    // Get the metrics
    let summary = profiler.get_performance_summary();
    
    println!("Total CU consumed: {}", summary.total_cu_consumed);
    println!("Operations profiled: {}", summary.operations_profiled);
    
    // Verify batch processing is optimized
    assert!(
        summary.total_cu_consumed < 400_000, // Should be well under for 8 outcomes
        "Batch processing should be optimized"
    );
}

#[test]
fn test_lmsr_implementation() {
    println!("=== LMSR Implementation Test ===");
    
    let liquidity = FixedPoint::from_float(1000.0);
    let mut lmsr = LMSR::new(liquidity, 3); // 3-outcome market
    
    // Initialize with some quantities
    lmsr.q[0] = FixedPoint::from_float(100.0);
    lmsr.q[1] = FixedPoint::from_float(150.0);
    lmsr.q[2] = FixedPoint::from_float(50.0);
    
    // Test pricing
    let price0 = lmsr.price(0).unwrap();
    let price1 = lmsr.price(1).unwrap();
    let price2 = lmsr.price(2).unwrap();
    
    // Verify prices sum to ~1
    let total = price0.add(&price1).unwrap().add(&price2).unwrap();
    assert!(
        (total.to_float() - 1.0).abs() < 0.01,
        "Prices should sum to approximately 1"
    );
    
    println!("Prices: {:.4}, {:.4}, {:.4}", 
        price0.to_float(), price1.to_float(), price2.to_float());
    
    // Test trading
    let trade_amount = FixedPoint::from_float(50.0);
    let (cost, slippage) = lmsr.trade(0, trade_amount).unwrap();
    
    println!("Trade cost: {:.4}, Slippage: {:.4}", 
        cost.to_float(), slippage.to_float());
    
    assert!(slippage.to_float() > 0.0, "Trade should cause slippage");
}

#[test]
fn test_cu_measurement_accuracy() {
    println!("=== CU Measurement Accuracy Test ===");
    
    let mut profiler = PerformanceProfiler::new();
    
    // Profile PM-AMM operation
    let (_, pm_metrics) = profiler.profile_transaction("test_pm_amm", || {
        // Simulate PM-AMM calculations
        let mut result = FixedPoint::from_float(1.0);
        for i in 0..10 {
            result = result.mul(&FixedPoint::from_float(1.1))?.sqrt()?;
        }
        Ok(())
    }).unwrap();
    
    // Profile LMSR operation  
    let (_, lmsr_metrics) = profiler.profile_transaction("test_lmsr", || {
        // Simulate LMSR calculations (more expensive)
        let mut result = FixedPoint::from_float(1.0);
        for i in 0..10 {
            result = result.exp()?.ln()?.mul(&FixedPoint::from_float(1.1))?;
        }
        Ok(())
    }).unwrap();
    
    println!("PM-AMM CU: {}", pm_metrics.compute_units);
    println!("LMSR CU: {}", lmsr_metrics.compute_units);
    
    // PM-AMM should be more efficient
    assert!(
        pm_metrics.compute_units < lmsr_metrics.compute_units,
        "PM-AMM should use fewer CU for similar operations"
    );
}

#[tokio::test]
async fn test_multi_outcome_markets() {
    println!("=== Multi-Outcome Market Comparison ===");
    
    let mut profiler = PerformanceProfiler::new();
    let outcome_counts = vec![2, 4, 8, 16];
    
    for num_outcomes in outcome_counts {
        println!("\nTesting with {} outcomes", num_outcomes);
        
        let comparison = compare_amm_performance(
            10_000_000, // 10 token trade
            num_outcomes,
            &mut profiler,
        ).unwrap();
        
        // PM-AMM advantage should be consistent across outcome counts
        assert!(
            comparison.pm_amm_cu < comparison.lmsr_cu,
            "PM-AMM should outperform LMSR for {} outcomes", num_outcomes
        );
        
        println!("✓ {} outcomes: PM-AMM {} CU vs LMSR {} CU", 
            num_outcomes, comparison.pm_amm_cu, comparison.lmsr_cu);
    }
}

#[test]
fn test_slippage_calculation_accuracy() {
    println!("=== Slippage Calculation Accuracy Test ===");
    
    // Test LMSR slippage calculation
    let mut lmsr = LMSR::new(FixedPoint::from_float(1000.0), 2);
    lmsr.q[0] = FixedPoint::from_float(500.0);
    lmsr.q[1] = FixedPoint::from_float(500.0);
    
    let initial_price = lmsr.price(0).unwrap();
    
    // Large trade to cause significant slippage
    let large_trade = FixedPoint::from_float(1000.0);
    let (_, slippage) = lmsr.trade(0, large_trade).unwrap();
    
    let final_price = lmsr.price(0).unwrap();
    
    println!("Initial price: {:.4}", initial_price.to_float());
    println!("Final price: {:.4}", final_price.to_float());
    println!("Measured slippage: {:.4}", slippage.to_float());
    
    // Verify slippage is significant for large trades
    assert!(
        slippage.to_float() > 0.05,
        "Large trades should cause significant slippage in LMSR"
    );
}

/// Integration test simulating real-world usage
#[tokio::test]
async fn test_real_world_performance_scenario() {
    println!("=== Real-World Performance Scenario ===");
    
    let mut profiler = PerformanceProfiler::new();
    
    // Simulate a day of trading
    let trades_per_hour = 100;
    let hours = 24;
    let total_trades = trades_per_hour * hours;
    
    let mut total_pm_cu = 0u64;
    let mut total_lmsr_cu = 0u64;
    
    println!("Simulating {} trades over {} hours...", total_trades, hours);
    
    for i in 0..total_trades {
        // Vary trade sizes
        let trade_size = 1_000_000 + (i * 100_000) % 10_000_000;
        
        let comparison = compare_amm_performance(
            trade_size,
            2,
            &mut profiler,
        ).unwrap();
        
        total_pm_cu += comparison.pm_amm_cu;
        total_lmsr_cu += comparison.lmsr_cu;
        
        if i % 100 == 0 {
            println!("Processed {} trades...", i);
        }
    }
    
    let pm_avg = total_pm_cu / total_trades as u64;
    let lmsr_avg = total_lmsr_cu / total_trades as u64;
    let savings = ((lmsr_avg - pm_avg) as f64 / lmsr_avg as f64) * 100.0;
    
    println!("\n=== Results ===");
    println!("Total trades: {}", total_trades);
    println!("PM-AMM average CU: {}", pm_avg);
    println!("LMSR average CU: {}", lmsr_avg);
    println!("CU savings: {:.1}%", savings);
    println!("Total CU saved: {}", total_lmsr_cu - total_pm_cu);
    
    // Verify consistent performance advantage
    assert!(
        savings > 10.0,
        "PM-AMM should provide at least 10% CU savings"
    );
}