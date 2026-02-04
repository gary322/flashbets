// Comprehensive tests for CU verification system
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    system_program,
};
use solana_program_test::{processor, tokio, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
    compute_budget::ComputeBudgetInstruction,
};
use borsh::{BorshDeserialize, BorshSerialize};

use betting_platform_native::{
    performance::cu_verifier::{CUVerifier, CUMeasurement},
    amm::{
        lmsr::optimized_math::calculate_price_optimized,
        l2amm::optimized_math::calculate_l2_price_optimized,
    },
    error::BettingPlatformError,
    processor::process_instruction,
};

#[derive(BorshSerialize, BorshDeserialize)]
struct TestInstruction {
    variant: u8,
    data: Vec<u8>,
}

async fn setup_test() -> ProgramTestContext {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(process_instruction),
    );
    
    program_test.start_with_context().await
}

#[tokio::test]
async fn test_lmsr_cu_under_50k() {
    let mut context = setup_test().await;
    let mut verifier = CUVerifier::new();
    
    // Test LMSR trade CU usage
    let measurement = verifier.measure_lmsr_trade().unwrap();
    
    println!("LMSR Trade CU Usage:");
    println!("  Initial Balance: {}", measurement.initial_cu_balance);
    println!("  Final Balance: {}", measurement.final_cu_balance);
    println!("  CU Used: {}", measurement.cu_used);
    println!("  Operation: {}", measurement.operation_name);
    
    // Verify it's under 50k CU (actually should be ~20k with optimizations)
    assert!(measurement.cu_used < 50_000, "LMSR trade exceeded 50k CU limit");
    assert!(measurement.cu_used < 25_000, "LMSR optimization not effective enough");
}

#[tokio::test]
async fn test_l2amm_cu_under_50k() {
    let mut verifier = CUVerifier::new();
    
    // Test L2AMM trade CU usage
    let measurement = verifier.measure_l2amm_trade().unwrap();
    
    println!("L2AMM Trade CU Usage:");
    println!("  Initial Balance: {}", measurement.initial_cu_balance);
    println!("  Final Balance: {}", measurement.final_cu_balance);
    println!("  CU Used: {}", measurement.cu_used);
    println!("  Operation: {}", measurement.operation_name);
    
    // Verify it's under 50k CU (should be ~25k with optimizations)
    assert!(measurement.cu_used < 50_000, "L2AMM trade exceeded 50k CU limit");
    assert!(measurement.cu_used < 30_000, "L2AMM optimization not effective enough");
}

#[tokio::test]
async fn test_full_trade_flow_cu() {
    let mut verifier = CUVerifier::new();
    
    // Test complete trade flow including all operations
    let measurement = verifier.measure_full_trade_flow().unwrap();
    
    println!("Full Trade Flow CU Usage:");
    println!("  Initial Balance: {}", measurement.initial_cu_balance);
    println!("  Final Balance: {}", measurement.final_cu_balance);
    println!("  CU Used: {}", measurement.cu_used);
    println!("  Operation: {}", measurement.operation_name);
    
    // Full flow should still be under 50k
    assert!(measurement.cu_used < 50_000, "Full trade flow exceeded 50k CU limit");
}

#[tokio::test]
async fn test_cu_report_generation() {
    let mut verifier = CUVerifier::new();
    
    // Measure various operations
    verifier.measure_lmsr_trade().unwrap();
    verifier.measure_l2amm_trade().unwrap();
    verifier.measure_full_trade_flow().unwrap();
    
    // Generate report
    let report = verifier.generate_report();
    
    // Verify report contains all measurements
    assert!(report.contains("LMSR Trade"));
    assert!(report.contains("L2AMM Trade"));
    assert!(report.contains("Full Trade Flow"));
    assert!(report.contains("Average CU"));
    assert!(report.contains("Max CU"));
    assert!(report.contains("Min CU"));
}

#[tokio::test]
async fn test_optimized_lmsr_calculations() {
    // Test that optimized LMSR calculations produce correct results
    let shares = vec![1000, 2000, 1500];
    let b_parameter = 1000;
    
    // Test for each outcome
    for outcome in 0..shares.len() {
        let price = calculate_price_optimized(&shares, outcome as u8, b_parameter).unwrap();
        
        // Price should be between 0 and 10000 (0% to 100%)
        assert!(price > 0 && price <= 10000);
        
        // Sum of prices should be approximately 10000 (100%)
        if outcome == shares.len() - 1 {
            let mut total_price = 0u64;
            for i in 0..shares.len() {
                let p = calculate_price_optimized(&shares, i as u8, b_parameter).unwrap();
                total_price += p;
            }
            // Allow small rounding error
            assert!(total_price >= 9900 && total_price <= 10100);
        }
    }
}

#[tokio::test]
async fn test_optimized_l2amm_calculations() {
    use betting_platform_native::amm::l2amm::state::DistributionParams;
    
    let params = DistributionParams {
        mean: 5000,      // 50%
        variance: 1000,   // 10% std dev
        skewness: 0,      // No skew
        kurtosis: 0,      // Normal kurtosis
    };
    
    // Test price calculation at different points
    let test_points = vec![2500, 5000, 7500]; // 25%, 50%, 75%
    
    for point in test_points {
        let price = calculate_l2_price_optimized(point, &params).unwrap();
        
        // Price should be valid
        assert!(price > 0 && price <= 10000);
        
        // At mean, price should be highest (for normal distribution)
        if point == params.mean {
            let price_lower = calculate_l2_price_optimized(point - 1000, &params).unwrap();
            let price_higher = calculate_l2_price_optimized(point + 1000, &params).unwrap();
            assert!(price >= price_lower && price >= price_higher);
        }
    }
}

#[tokio::test]
async fn test_cu_limits_enforcement() {
    let verifier = CUVerifier::new();
    
    // Test that operations correctly detect when they would exceed limits
    let result = verifier.verify_under_limit(45_000, 50_000);
    assert!(result.is_ok());
    
    let result = verifier.verify_under_limit(55_000, 50_000);
    assert!(result.is_err());
    
    if let Err(e) = result {
        match e.downcast_ref::<BettingPlatformError>() {
            Some(BettingPlatformError::ComputeUnitLimitExceeded) => (),
            _ => panic!("Expected ComputeUnitLimitExceeded error"),
        }
    }
}

#[tokio::test]
async fn test_parallel_cu_measurements() {
    use std::sync::{Arc, Mutex};
    use tokio::task;
    
    let measurements = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];
    
    // Spawn multiple parallel measurements
    for i in 0..5 {
        let measurements_clone = Arc::clone(&measurements);
        let handle = task::spawn(async move {
            let mut verifier = CUVerifier::new();
            let measurement = if i % 2 == 0 {
                verifier.measure_lmsr_trade().unwrap()
            } else {
                verifier.measure_l2amm_trade().unwrap()
            };
            
            measurements_clone.lock().unwrap().push(measurement);
        });
        handles.push(handle);
    }
    
    // Wait for all measurements
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all measurements are under limit
    let measurements = measurements.lock().unwrap();
    for measurement in measurements.iter() {
        assert!(measurement.cu_used < 50_000);
    }
}

#[tokio::test]
async fn test_cu_optimization_effectiveness() {
    let mut verifier = CUVerifier::new();
    
    // Measure optimized vs non-optimized (simulated)
    let optimized = verifier.measure_lmsr_trade().unwrap();
    
    // Simulate non-optimized by measuring with more complex operations
    let non_optimized_estimate = 50_000u64; // Original estimate
    
    let improvement = ((non_optimized_estimate - optimized.cu_used) as f64 / non_optimized_estimate as f64) * 100.0;
    
    println!("CU Optimization Results:");
    println!("  Original: {} CU", non_optimized_estimate);
    println!("  Optimized: {} CU", optimized.cu_used);
    println!("  Improvement: {:.1}%", improvement);
    
    // Should see at least 50% improvement
    assert!(improvement >= 50.0);
}