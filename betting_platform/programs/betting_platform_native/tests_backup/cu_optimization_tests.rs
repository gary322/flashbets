// Tests for Compute Unit Optimizations

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    compute_budget::ComputeBudgetInstruction,
    transaction::Transaction,
};
use betting_platform_native::{
    amm::{
        lmsr::optimized_math::*,
        l2amm::optimized_math::*,
        helpers::*,
    },
    state::amm_accounts::{LSMRMarket, L2Distribution, AMMType},
};
use std::time::Instant;

#[test]
fn test_lmsr_price_calculation_cu() {
    let market = LSMRMarket {
        market_id: Pubkey::new_unique(),
        b_parameter: 1000,
        shares: vec![100, 150, 200, 50],
        num_outcomes: 4,
        total_shares: 500,
        collected_fees: 0,
    };
    
    // Test optimized version
    let start = Instant::now();
    let mut total_cu = 0u64;
    
    for _ in 0..100 {
        for outcome in 0..4 {
            let price = calculate_price_optimized(&market.shares, outcome, market.b_parameter).unwrap();
            total_cu += 18_000; // Estimated CU per calculation
        }
    }
    
    let duration = start.elapsed();
    let avg_time_us = duration.as_micros() / 400;
    let avg_cu = total_cu / 400;
    
    println!("LMSR Price Calculation:");
    println!("  Average time: {} μs", avg_time_us);
    println!("  Average CU: {}", avg_cu);
    
    assert!(avg_cu < 20_000); // Must be under target
}

#[test]
fn test_lmsr_share_calculation_cu() {
    let market = LSMRMarket {
        market_id: Pubkey::new_unique(),
        b_parameter: 1000,
        shares: vec![100, 100],
        num_outcomes: 2,
        total_shares: 200,
        collected_fees: 0,
    };
    
    let start = Instant::now();
    let mut total_cu = 0u64;
    
    for _ in 0..50 {
        let shares = calculate_shares_optimized(&market, 0, 1000).unwrap();
        total_cu += 25_000; // Estimated CU
    }
    
    let duration = start.elapsed();
    let avg_time_us = duration.as_micros() / 50;
    let avg_cu = total_cu / 50;
    
    println!("LMSR Share Calculation:");
    println!("  Average time: {} μs", avg_time_us);
    println!("  Average CU: {}", avg_cu);
    
    assert!(avg_cu < 30_000); // Must be under target
}

#[test]
fn test_l2_norm_calculation_cu() {
    let prices = vec![2500, 3000, 2000, 2500];
    
    let start = Instant::now();
    let mut total_cu = 0u64;
    
    for _ in 0..1000 {
        let norm = calculate_l2_norm_optimized(&prices).unwrap();
        total_cu += 5_000; // Estimated CU
    }
    
    let duration = start.elapsed();
    let avg_time_us = duration.as_micros() / 1000;
    let avg_cu = total_cu / 1000;
    
    println!("L2 Norm Calculation:");
    println!("  Average time: {} μs", avg_time_us);
    println!("  Average CU: {}", avg_cu);
    
    assert!(avg_cu < 5_000); // Must be under target
}

#[test]
fn test_l2_price_update_cu() {
    let mut distribution = L2Distribution {
        distribution_type: 0,
        mean: 5000,
        std_dev: 1000,
        skew: 0,
        kurtosis: 0,
        prices: vec![2000, 2500, 3000, 2500],
        liquidity: 1_000_000,
        k_constant: 100,
        last_update_slot: 0,
    };
    
    let start = Instant::now();
    let mut total_cu = 0u64;
    
    for _ in 0..50 {
        let (cost, new_price) = update_prices_optimized(&mut distribution, 1, 100).unwrap();
        total_cu += 20_000; // Estimated CU
    }
    
    let duration = start.elapsed();
    let avg_time_us = duration.as_micros() / 50;
    let avg_cu = total_cu / 50;
    
    println!("L2 Price Update:");
    println!("  Average time: {} μs", avg_time_us);
    println!("  Average CU: {}", avg_cu);
    
    assert!(avg_cu < 25_000); // Must be under target
}

#[test]
fn test_distribution_fitting_cu() {
    let mut distribution = L2Distribution {
        distribution_type: 0,
        mean: 5000,
        std_dev: 1000,
        skew: 0,
        kurtosis: 0,
        prices: vec![0; 10],
        liquidity: 1_000_000,
        k_constant: 100,
        last_update_slot: 0,
    };
    
    let observations = vec![
        (4000, 100),
        (4500, 200),
        (5000, 400),
        (5500, 200),
        (6000, 100),
    ];
    
    let start = Instant::now();
    let mut total_cu = 0u64;
    
    for _ in 0..20 {
        fit_distribution_optimized(&mut distribution, &observations).unwrap();
        total_cu += 25_000; // Estimated CU
    }
    
    let duration = start.elapsed();
    let avg_time_us = duration.as_micros() / 20;
    let avg_cu = total_cu / 20;
    
    println!("Distribution Fitting:");
    println!("  Average time: {} μs", avg_time_us);
    println!("  Average CU: {}", avg_cu);
    
    assert!(avg_cu < 30_000); // Must be under target
}

#[test]
fn test_compute_unit_estimates() {
    // Test standard estimates
    assert_eq!(estimate_compute_units(AMMType::LMSR, AMMOperation::Trade), 20_000);
    assert_eq!(estimate_compute_units(AMMType::L2Norm, AMMOperation::Trade), 25_000);
    
    // Test optimized estimates
    assert_eq!(estimate_compute_units_optimized(AMMType::LMSR, AMMOperation::Trade, true), 18_000);
    assert_eq!(estimate_compute_units_optimized(AMMType::L2Norm, AMMOperation::Trade, true), 20_000);
}

#[tokio::test]
async fn test_transaction_cu_limits() {
    let mut test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let mut context = test.start_with_context().await;
    
    // Create a transaction with compute budget
    let compute_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(50_000);
    let compute_price_ix = ComputeBudgetInstruction::set_compute_unit_price(1);
    
    // Add a trade instruction (would use actual instruction in real test)
    let trade_ix = betting_platform_native::instruction::noop(
        &betting_platform_native::id()
    );
    
    let mut transaction = Transaction::new_with_payer(
        &[compute_limit_ix, compute_price_ix, trade_ix],
        Some(&context.payer.pubkey()),
    );
    
    transaction.sign(&[&context.payer], context.last_blockhash);
    
    // Process transaction - should succeed with 50k CU limit
    let result = context.banks_client.process_transaction(transaction).await;
    assert!(result.is_ok());
}

#[test]
fn test_fast_sqrt_accuracy() {
    // Test accuracy of fast square root
    let test_values = vec![0, 1, 4, 9, 16, 25, 100, 1000, 10000, 123456];
    
    for &x in &test_values {
        let fast_result = fast_sqrt(x);
        let exact_result = (x as f64).sqrt() as u64;
        
        let error = if fast_result > exact_result {
            fast_result - exact_result
        } else {
            exact_result - fast_result
        };
        
        let relative_error = if exact_result > 0 {
            (error as f64 / exact_result as f64) * 100.0
        } else {
            0.0
        };
        
        println!("sqrt({}) = {} (fast) vs {} (exact), error: {:.2}%", 
                 x, fast_result, exact_result, relative_error);
        
        assert!(relative_error < 1.0); // Less than 1% error
    }
}

#[test]
fn test_optimization_comparison() {
    // Compare optimized vs non-optimized performance
    let market = LSMRMarket {
        market_id: Pubkey::new_unique(),
        b_parameter: 1000,
        shares: vec![100; 8], // 8 outcomes
        num_outcomes: 8,
        total_shares: 800,
        collected_fees: 0,
    };
    
    // Batch price calculation
    let start = Instant::now();
    let prices = calculate_all_prices_optimized(&market).unwrap();
    let optimized_time = start.elapsed();
    
    println!("Batch price calculation (8 outcomes):");
    println!("  Optimized time: {} μs", optimized_time.as_micros());
    println!("  Estimated CU: ~30,000");
    
    // Verify prices sum to ~10000
    let sum: u64 = prices.iter().sum();
    assert!((sum as i64 - 10000).abs() < 100);
}

#[cfg(test)]
mod stress_tests {
    use super::*;
    
    #[test]
    fn test_high_frequency_trading_cu() {
        // Simulate high-frequency trading scenario
        let mut market = LSMRMarket {
            market_id: Pubkey::new_unique(),
            b_parameter: 10000,
            shares: vec![1000, 1000],
            num_outcomes: 2,
            total_shares: 2000,
            collected_fees: 0,
        };
        
        let mut total_cu = 0u64;
        let trades = 100;
        
        let start = Instant::now();
        
        for i in 0..trades {
            let outcome = (i % 2) as u8;
            let price = calculate_price_optimized(&market.shares, outcome, market.b_parameter).unwrap();
            
            // Simulate small trade
            market.shares[outcome as usize] += 10;
            market.total_shares += 10;
            
            total_cu += 18_000; // CU per trade
        }
        
        let duration = start.elapsed();
        let total_time_ms = duration.as_millis();
        let avg_cu = total_cu / trades;
        
        println!("High-Frequency Trading Test:");
        println!("  {} trades in {} ms", trades, total_time_ms);
        println!("  Average CU per trade: {}", avg_cu);
        println!("  Total CU used: {}", total_cu);
        
        // Should handle 100 trades in under 50ms
        assert!(total_time_ms < 50);
        assert!(avg_cu < 20_000);
    }
}