//! Phase 3 Performance Tests
//! 
//! Verifies all performance requirements including:
//! - Newton-Raphson solver (avg 4.2 iterations, max 10, <5k CU)
//! - Simpson's rule integration (10+ points, error <1e-6, <2k CU)
//! - Overall CU limits for all operations

#[cfg(test)]
mod phase3_performance_tests {
    use solana_program::{
        account_info::AccountInfo,
        clock::Clock,
        program_error::ProgramError,
        pubkey::Pubkey,
        sysvar::Sysvar,
    };
    use betting_platform_native::{
        amm::{
            pmamm::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonConfig},
            l2amm::simpson::{SimpsonIntegrator, SimpsonConfig},
        },
        math::fixed_point::U64F64,
        performance::cu_verifier::CUVerifier,
        state::amm_accounts::{PMAMMPool, PoolState},
    };

    #[test]
    fn test_newton_raphson_performance() {
        println!("\n=== Newton-Raphson Performance Test ===");
        
        // Create test pools with varying complexity
        let test_cases = vec![
            (vec![1000, 1000], vec![5000, 5000]), // 2 outcomes
            (vec![1000, 2000, 3000], vec![4000, 3500, 2500]), // 3 outcomes
            (vec![1000, 1500, 2000, 2500], vec![3000, 2500, 2500, 2000]), // 4 outcomes
        ];
        
        let mut total_iterations = 0;
        let mut solve_count = 0;
        
        for (reserves, target_probs) in test_cases {
            let pool = PMAMMPool {
                discriminator: *b"PMAMM_PL",
                pool_id: 1,
                num_outcomes: reserves.len() as u8,
                reserves: reserves.clone(),
                total_lp_supply: 1000000,
                fee_bps: 30,
                liquidity_providers: vec![],
                total_volume: 0,
                created_at: 0,
                last_update: 0,
                state: PoolState::Active,
            };
            
            let mut solver = NewtonRaphsonSolver::new();
            let result = solver.solve_for_prices(&pool, &target_probs).unwrap();
            
            println!("Outcomes: {}, Iterations: {}, Converged: {}", 
                     reserves.len(), result.iterations, result.converged);
            
            assert!(result.converged, "Solver must converge");
            assert!(result.iterations <= 10, "Iterations exceeded max: {}", result.iterations);
            
            total_iterations += result.iterations as u32;
            solve_count += 1;
            
            // Check solver statistics
            let (min, max, avg) = solver.get_iteration_stats();
            println!("  Stats - Min: {}, Max: {}, Avg: {:.2}", min, max, avg);
        }
        
        let overall_average = total_iterations as f64 / solve_count as f64;
        println!("\nOverall average iterations: {:.2}", overall_average);
        assert!(overall_average >= 3.0 && overall_average <= 5.0, 
                "Average iterations should be ~4.2, got {:.2}", overall_average);
    }

    #[test]
    fn test_simpson_integration_performance() {
        println!("\n=== Simpson's Rule Integration Test ===");
        
        // Test with different point counts
        let point_counts = vec![10, 12, 20];
        
        for num_points in point_counts {
            let config = SimpsonConfig {
                num_points,
                error_tolerance: U64F64::from_raw(4398), // ~1e-6
                max_iterations: 5,
            };
            
            let mut integrator = SimpsonIntegrator::with_config(config);
            
            // Test function: x^2
            let f = |x: U64F64| -> Result<U64F64, ProgramError> {
                x.checked_mul(x)
            };
            
            // Integrate from 0 to 1 (exact result = 1/3)
            let result = integrator.integrate(
                f,
                U64F64::from_num(0),
                U64F64::from_num(1),
            ).unwrap();
            
            println!("Points: {}, Value: {:.8}, Error: {:.2e}, Evaluations: {}, CU: {}", 
                     num_points,
                     result.value.to_num() as f64 / (1u64 << 64) as f64,
                     result.error.to_num() as f64 / (1u64 << 64) as f64,
                     result.evaluations,
                     result.cu_used);
            
            // Verify accuracy
            let expected = U64F64::from_num(1).checked_div(U64F64::from_num(3)).unwrap();
            let diff = if result.value > expected {
                result.value.checked_sub(expected).unwrap()
            } else {
                expected.checked_sub(result.value).unwrap()
            };
            
            assert!(diff < U64F64::from_raw(1_000_000), // Much less than 1e-6
                    "Integration error too large: {:?}", diff);
            
            // Verify CU usage
            assert!(result.cu_used <= 2000, 
                    "Simpson's rule exceeded 2000 CU: {}", result.cu_used);
        }
    }

    #[test]
    fn test_cu_limits_comprehensive() {
        println!("\n=== Comprehensive CU Limits Test ===");
        
        let mut verifier = CUVerifier::new();
        
        // Test all operations
        let operations = vec![
            ("LMSR Trade", verifier.measure_lmsr_trade()),
            ("L2-AMM Trade", verifier.measure_l2_trade()),
            ("PM-AMM Trade", verifier.measure_pmamm_trade()),
            ("Newton-Raphson", verifier.measure_newton_raphson()),
            ("Simpson's Rule", verifier.measure_simpson_integration()),
            ("8-Outcome Batch", verifier.measure_batch_8_outcome()),
            ("Full Trade Flow", verifier.measure_full_trade_flow()),
        ];
        
        println!("\nCU Usage Summary:");
        println!("{:<20} {:>10} {:>10} {:>10}", "Operation", "CU Used", "Limit", "Status");
        println!("{:-<50}", "");
        
        for (name, result) in operations {
            match result {
                Ok(measurement) => {
                    let limit = match measurement.operation.as_str() {
                        "NEWTON_RAPHSON" => CUVerifier::MAX_CU_NEWTON_RAPHSON,
                        "SIMPSON_INTEGRATION" => CUVerifier::MAX_CU_SIMPSON_INTEGRATION,
                        "BATCH_8_OUTCOME" => CUVerifier::MAX_CU_BATCH_8_OUTCOME,
                        _ => CUVerifier::MAX_CU_PER_TRADE,
                    };
                    
                    println!("{:<20} {:>10} {:>10} {:>10}", 
                             name,
                             measurement.compute_units_used,
                             limit,
                             if measurement.passed { "✓ PASS" } else { "✗ FAIL" });
                    
                    assert!(measurement.passed, 
                            "{} failed CU limit: {} > {}", 
                            name, measurement.compute_units_used, limit);
                },
                Err(e) => {
                    panic!("Failed to measure {}: {:?}", name, e);
                }
            }
        }
        
        // Generate and display report
        let report = verifier.generate_report();
        println!("\n{}", report);
    }

    #[test]
    fn test_performance_under_load() {
        println!("\n=== Performance Under Load Test ===");
        
        let mut verifier = CUVerifier::new();
        let iterations = 10;
        
        // Simulate repeated operations
        let mut newton_cus = Vec::new();
        let mut simpson_cus = Vec::new();
        
        for i in 0..iterations {
            // Newton-Raphson
            let newton_result = verifier.measure_newton_raphson().unwrap();
            newton_cus.push(newton_result.compute_units_used);
            
            // Simpson's rule
            let simpson_result = verifier.measure_simpson_integration().unwrap();
            simpson_cus.push(simpson_result.compute_units_used);
            
            if i % 5 == 0 {
                println!("Iteration {}: Newton {} CU, Simpson {} CU", 
                         i, newton_result.compute_units_used, simpson_result.compute_units_used);
            }
        }
        
        // Calculate statistics
        let newton_avg = newton_cus.iter().sum::<u64>() / iterations;
        let simpson_avg = simpson_cus.iter().sum::<u64>() / iterations;
        
        println!("\nPerformance Statistics:");
        println!("Newton-Raphson - Avg: {} CU, Max: {} CU", 
                 newton_avg, newton_cus.iter().max().unwrap());
        println!("Simpson's Rule - Avg: {} CU, Max: {} CU", 
                 simpson_avg, simpson_cus.iter().max().unwrap());
        
        // Verify consistency
        assert!(newton_cus.iter().all(|&cu| cu <= CUVerifier::MAX_CU_NEWTON_RAPHSON),
                "Newton-Raphson CU limit exceeded under load");
        assert!(simpson_cus.iter().all(|&cu| cu <= CUVerifier::MAX_CU_SIMPSON_INTEGRATION),
                "Simpson's rule CU limit exceeded under load");
    }

    #[test]
    fn test_newton_raphson_edge_cases() {
        println!("\n=== Newton-Raphson Edge Cases Test ===");
        
        // Test with extreme probability distributions
        let edge_cases = vec![
            vec![9000, 500, 500],    // Heavy favorite
            vec![3333, 3333, 3334],  // Nearly equal
            vec![100, 100, 9800],    // Extreme underdog
        ];
        
        for target_probs in edge_cases {
            let pool = PMAMMPool {
                discriminator: *b"PMAMM_PL",
                pool_id: 1,
                num_outcomes: 3,
                reserves: vec![1000, 1000, 1000], // Equal starting reserves
                total_lp_supply: 1000000,
                fee_bps: 30,
                liquidity_providers: vec![],
                total_volume: 0,
                created_at: 0,
                last_update: 0,
                state: PoolState::Active,
            };
            
            let mut solver = NewtonRaphsonSolver::new();
            let result = solver.solve_for_prices(&pool, &target_probs).unwrap();
            
            println!("Target probs: {:?}, Iterations: {}, Converged: {}", 
                     target_probs, result.iterations, result.converged);
            
            assert!(result.converged, "Solver must converge for edge case");
            assert!(result.iterations <= 10, "Iterations exceeded max for edge case");
            assert!(solver.is_performance_optimal(), "Performance not optimal for edge case");
        }
    }
}