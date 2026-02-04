#[cfg(test)]
mod pm_amm_tests {
    use fixed::types::{U64F64, I64F64};
    use crate::amm::pm_amm::*;

    #[test]
    fn test_newton_raphson_convergence() {
        let state = PMAMMState::new(
            U64F64::from_num(100), // L = 100
            86400,                 // 1 day in slots
            4,                     // 4 outcomes
            0,                     // Start slot
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();

        // Test buy order
        let result = solver.solve_pm_amm_price(
            &state,
            0, // First outcome
            I64F64::from_num(10), // Buy 10 units
        ).unwrap();

        assert!(result.iterations <= 5, "Should converge in ≤5 iterations");
        assert!(result.new_price > result.old_price, "Buy should increase price");
        assert!(result.price_impact < U64F64::from_num(0.1), "Impact should be <10%");
    }

    #[test]
    fn test_uniform_lvr() {
        let mut state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            3,
            0,
        ).unwrap();

        // Advance time to test time decay
        state.current_time = 43200; // Half time elapsed

        let solver = NewtonRaphsonSolver::new();
        let result = solver.solve_pm_amm_price(
            &state,
            1,
            I64F64::from_num(5),
        ).unwrap();

        // LVR should be uniform over time
        assert!(result.lvr_cost > U64F64::from_num(0), "LVR should be positive");

        // Test that LVR increases as time approaches expiry
        state.current_time = 80000; // Near expiry
        let result2 = solver.solve_pm_amm_price(
            &state,
            1,
            I64F64::from_num(5),
        ).unwrap();

        assert!(result2.lvr_cost > result.lvr_cost, "LVR should increase near expiry");
    }

    #[test]
    fn test_multi_outcome_price_conservation() {
        let mut state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            5, // 5 outcomes
            0,
        ).unwrap();

        let pricing = MultiOutcomePricing::new();
        let solver = NewtonRaphsonSolver::new();
        
        // Update price of outcome 2
        let new_price = U64F64::from_num(0.3);
        pricing.update_all_prices(&mut state, 2, new_price, &solver).unwrap();

        // Check that prices still sum to 1
        let sum: U64F64 = state.prices.iter().copied().sum();
        let one = U64F64::from_num(1);
        let tolerance = U64F64::from_num(0.0001);
        
        assert!((sum - one).abs() < tolerance, "Prices should sum to 1");
    }

    #[test]
    fn test_price_bounds() {
        let state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            3,
            0,
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();
        
        // Test large buy order
        let result = solver.solve_pm_amm_price(
            &state,
            0,
            I64F64::from_num(1000), // Very large buy
        ).unwrap();

        // Price should be bounded between 0.001 and 0.999
        assert!(result.new_price >= U64F64::from_num(0.001), "Price too low");
        assert!(result.new_price <= U64F64::from_num(0.999), "Price too high");
    }

    #[test]
    fn test_sell_order() {
        let state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            2, // Binary market
            0,
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();
        
        // Test sell order (negative size)
        let result = solver.solve_pm_amm_price(
            &state,
            0,
            I64F64::from_num(-10), // Sell 10 units
        ).unwrap();

        assert!(result.new_price < result.old_price, "Sell should decrease price");
        assert!(result.iterations <= 5, "Should converge quickly");
    }

    #[test]
    fn test_time_decay() {
        let mut state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            2,
            0,
        ).unwrap();

        let solver = NewtonRaphsonSolver::new();
        let order_size = I64F64::from_num(10);
        
        // Test at different time points
        let time_points = vec![0, 21600, 43200, 64800, 80000];
        let mut impacts = vec![];
        
        for t in time_points {
            state.current_time = t;
            let result = solver.solve_pm_amm_price(&state, 0, order_size).unwrap();
            impacts.push(result.price_impact);
        }
        
        // Price impact should increase as time approaches expiry
        for i in 1..impacts.len() {
            assert!(impacts[i] >= impacts[i-1], "Impact should increase over time");
        }
    }
}