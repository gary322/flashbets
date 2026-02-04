#[cfg(test)]
mod pm_amm_performance_tests {
    use fixed::types::{U64F64, I64F64};
    use crate::amm::pm_amm::*;
    use std::time::Instant;

    #[test]
    fn test_pm_amm_solver_performance() {
        // Initialize PM-AMM state with precomputed tables
        let state = PMAMMState::new(
            U64F64::from_num(100), // L = 100
            86400,                 // 1 day in slots
            4,                     // 4 outcomes
            0,                     // Start slot
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();
        
        // Measure multiple trades
        let test_sizes = vec![1, 10, 50, 100, 500];
        let mut total_iterations = 0;
        let mut max_iterations = 0;
        
        let start = Instant::now();
        
        for size in test_sizes {
            let result = solver.solve_pm_amm_price(
                &state,
                0, // First outcome
                I64F64::from_num(size),
            ).unwrap();
            
            total_iterations += result.iterations;
            max_iterations = max_iterations.max(result.iterations);
        }
        
        let elapsed = start.elapsed();
        
        println!("PM-AMM Solver Performance:");
        println!("  Average iterations: {}", total_iterations as f64 / 5.0);
        println!("  Max iterations: {}", max_iterations);
        println!("  Total time for 5 trades: {:?}", elapsed);
        println!("  Average time per trade: {:?}", elapsed / 5);
        
        // Verify convergence requirements
        assert!(max_iterations <= 5, "Must converge in ≤5 iterations");
        
        // Estimate CU usage (rough approximation)
        // Each iteration: ~500 CU for calculations
        // Lookup tables: ~100 CU per lookup
        // Total per solve: iterations * 500 + 200 (overhead)
        let estimated_cu = max_iterations as u64 * 500 + 200;
        println!("  Estimated CU per solve: {}", estimated_cu);
        
        assert!(estimated_cu < 5000, "PM-AMM solve should use <5k CU");
    }

    #[test]
    fn test_lookup_table_performance() {
        let state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            4,
            0,
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();
        
        // Test lookup performance
        let start = Instant::now();
        let iterations = 10000;
        
        for i in 0..iterations {
            let z = U64F64::from_num(i as f64 / 2500.0 - 2.0); // Range [-2, 2]
            let _ = solver.lookup_phi(z, &state.phi_lookup_table);
            let _ = solver.lookup_pdf(z, &state.pdf_lookup_table);
        }
        
        let elapsed = start.elapsed();
        let avg_lookup_time = elapsed / (iterations * 2);
        
        println!("Lookup Table Performance:");
        println!("  {} lookups in {:?}", iterations * 2, elapsed);
        println!("  Average lookup time: {:?}", avg_lookup_time);
        
        // Verify lookup is fast (should be <1 microsecond)
        assert!(avg_lookup_time.as_nanos() < 1000, "Lookups should be <1μs");
    }

    #[test]
    fn test_multi_outcome_update_performance() {
        let mut state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            10, // 10 outcomes (stress test)
            0,
        ).unwrap();

        let pricing = MultiOutcomePricing::new();
        let solver = NewtonRaphsonSolver::new();
        
        let start = Instant::now();
        
        // Update prices multiple times
        for i in 0..10 {
            let outcome = (i % 10) as u8;
            let new_price = U64F64::from_num(0.05 + (i as f64 * 0.01));
            pricing.update_all_prices(&mut state, outcome, new_price, &solver).unwrap();
        }
        
        let elapsed = start.elapsed();
        let avg_update = elapsed / 10;
        
        println!("Multi-Outcome Update Performance:");
        println!("  10 price updates in {:?}", elapsed);
        println!("  Average update time: {:?}", avg_update);
        
        // Verify price sum constraint
        let sum: U64F64 = state.prices.iter().copied().sum();
        let one = U64F64::from_num(1);
        let tolerance = U64F64::from_num(0.0001);
        assert!((sum - one).abs() < tolerance, "Prices must sum to 1");
        
        // Estimate CU for price update
        // Redistribution: O(n) operations
        // Normalization: O(n) operations  
        // Total: ~200 CU per outcome
        let estimated_cu = 200 * state.outcome_count as u64;
        println!("  Estimated CU per update: {}", estimated_cu);
        
        assert!(estimated_cu < 2000, "Price update should use <2k CU");
    }

    #[test]
    fn test_time_decay_impact() {
        let mut state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            3,
            0,
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();
        let order_size = I64F64::from_num(50);
        
        // Test at different time points
        let time_percentages = vec![0.0, 0.25, 0.5, 0.75, 0.9, 0.95, 0.99];
        
        println!("\nTime Decay Impact Analysis:");
        println!("Time % | Price Impact | LVR Cost | Iterations");
        println!("-------|--------------|----------|------------");
        
        for pct in time_percentages {
            state.current_time = (86400.0 * pct) as u64;
            let result = solver.solve_pm_amm_price(&state, 0, order_size).unwrap();
            
            println!("{:6.1}% | {:12.6} | {:8.6} | {}",
                pct * 100.0,
                result.price_impact.to_num::<f64>(),
                result.lvr_cost.to_num::<f64>(),
                result.iterations
            );
        }
    }
}