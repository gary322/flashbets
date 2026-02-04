#[cfg(test)]
mod pm_amm_user_journey {
    use fixed::types::{U64F64, I64F64};
    use crate::amm::pm_amm::*;

    #[test]
    fn test_complete_pm_amm_trading_flow() {
        println!("\n=== PM-AMM User Journey: Election Market ===\n");
        
        // Step 1: Initialize election prediction market with 4 candidates
        println!("Step 1: Initializing election market with 4 candidates");
        let mut state = PMAMMState::new(
            U64F64::from_num(10000), // L = 10,000 (high liquidity)
            864000,                  // 10 days until election
            4,                       // 4 candidates
            0,                       // Start at slot 0
        ).unwrap();
        
        println!("  Initial prices (equal probability):");
        for (i, price) in state.prices.iter().enumerate() {
            println!("    Candidate {}: {:.2}%", i, price.to_num::<f64>() * 100.0);
        }

        // Step 2: Early trading - news affects Candidate 0
        println!("\nStep 2: Breaking news boosts Candidate 0");
        let solver = NewtonRaphsonSolver::new();
        let pricing = MultiOutcomePricing::new();
        
        // Large buy order for Candidate 0
        let buy_result = solver.solve_pm_amm_price(
            &state,
            0,
            I64F64::from_num(5000), // Buy 5000 shares
        ).unwrap();
        
        println!("  Buy 5000 shares of Candidate 0:");
        println!("    Price: {:.2}% -> {:.2}%", 
            buy_result.old_price.to_num::<f64>() * 100.0,
            buy_result.new_price.to_num::<f64>() * 100.0
        );
        println!("    Price impact: {:.2}%", buy_result.price_impact.to_num::<f64>() * 100.0);
        println!("    Iterations: {}", buy_result.iterations);
        println!("    LVR cost: ${:.2}", buy_result.lvr_cost.to_num::<f64>());
        
        // Update all prices
        pricing.update_all_prices(&mut state, 0, buy_result.new_price, &solver).unwrap();
        state.volumes[0] = state.volumes[0] + U64F64::from_num(5000);
        
        println!("  Updated market prices:");
        for (i, price) in state.prices.iter().enumerate() {
            println!("    Candidate {}: {:.2}%", i, price.to_num::<f64>() * 100.0);
        }
        
        // Verify sum = 1
        let sum: U64F64 = state.prices.iter().copied().sum();
        println!("  Price sum check: {:.6}", sum.to_num::<f64>());

        // Step 3: Mid-period trading - debate performance
        println!("\nStep 3: Post-debate trading (50% time elapsed)");
        state.current_time = 432000; // 5 days passed
        
        // Candidate 2 performs well in debate
        let debate_buy = solver.solve_pm_amm_price(
            &state,
            2,
            I64F64::from_num(3000),
        ).unwrap();
        
        println!("  Buy 3000 shares of Candidate 2 (debate winner):");
        println!("    Price impact: {:.2}%", debate_buy.price_impact.to_num::<f64>() * 100.0);
        println!("    LVR cost: ${:.2} (higher due to time decay)", debate_buy.lvr_cost.to_num::<f64>());
        
        pricing.update_all_prices(&mut state, 2, debate_buy.new_price, &solver).unwrap();
        
        // Sell some Candidate 0 shares
        let sell_result = solver.solve_pm_amm_price(
            &state,
            0,
            I64F64::from_num(-2000), // Sell 2000 shares
        ).unwrap();
        
        println!("  Sell 2000 shares of Candidate 0:");
        println!("    Price: {:.2}% -> {:.2}%",
            sell_result.old_price.to_num::<f64>() * 100.0,
            sell_result.new_price.to_num::<f64>() * 100.0
        );
        
        pricing.update_all_prices(&mut state, 0, sell_result.new_price, &solver).unwrap();

        // Step 4: Late trading - polls tighten
        println!("\nStep 4: Final week trading (90% time elapsed)");
        state.current_time = 777600; // 9 days passed
        
        // Market becomes more volatile near expiry
        let late_trades = vec![
            (1, 1000),   // Small buy Candidate 1
            (3, 2000),   // Medium buy Candidate 3
            (2, -1000),  // Small sell Candidate 2
        ];
        
        for (candidate, size) in late_trades {
            let result = solver.solve_pm_amm_price(
                &state,
                candidate,
                I64F64::from_num(size),
            ).unwrap();
            
            println!("  {} {} shares of Candidate {}:",
                if size > 0 { "Buy" } else { "Sell" },
                size.abs(),
                candidate
            );
            println!("    Price impact: {:.2}%", result.price_impact.to_num::<f64>() * 100.0);
            println!("    LVR cost: ${:.2}", result.lvr_cost.to_num::<f64>());
            
            pricing.update_all_prices(&mut state, candidate, result.new_price, &solver).unwrap();
        }

        // Step 5: Final market state
        println!("\nStep 5: Final market state before election");
        println!("  Final probabilities:");
        let mut total_volume = U64F64::from_num(0);
        for (i, price) in state.prices.iter().enumerate() {
            println!("    Candidate {}: {:.2}%", i, price.to_num::<f64>() * 100.0);
            total_volume = total_volume + state.volumes[i];
        }
        
        // Calculate cross-impacts
        let cross_impacts = pricing.calculate_cross_impact(&state, 0, U64F64::from_num(0.05));
        println!("\n  Cross-market impacts from 5% move in Candidate 0:");
        for impact in cross_impacts {
            println!("    Candidate {}: {:.3}% change", 
                impact.outcome_id,
                impact.price_change.to_num::<f64>() * 100.0
            );
        }

        // Summary statistics
        println!("\n=== Market Summary ===");
        println!("  Total trading volume: ${:.0}", total_volume.to_num::<f64>());
        println!("  Time remaining: {:.1} days", (864000 - state.current_time) as f64 / 86400.0);
        println!("  Liquidity parameter: {}", state.liquidity_parameter.to_num::<f64>());
        
        // Test extreme scenario
        println!("\nBonus: Testing market limits with huge order");
        let extreme_result = solver.solve_pm_amm_price(
            &state,
            0,
            I64F64::from_num(100000), // Huge buy
        ).unwrap();
        
        println!("  Attempt to buy 100,000 shares:");
        println!("    Price would go to: {:.2}%", extreme_result.new_price.to_num::<f64>() * 100.0);
        println!("    Price bounded by [0.1%, 99.9%] constraint");
        
        assert!(extreme_result.new_price <= U64F64::from_num(0.999));
        assert!(extreme_result.new_price >= U64F64::from_num(0.001));
    }

    #[test] 
    fn test_binary_market_scenario() {
        println!("\n=== PM-AMM User Journey: Sports Betting ===\n");
        
        // Binary market: Team A vs Team B
        let mut state = PMAMMState::new(
            U64F64::from_num(5000),
            7200, // 2 hours until game ends
            2,    // Binary outcome
            0,
        ).unwrap();
        
        let solver = NewtonRaphsonSolver::new();
        let pricing = MultiOutcomePricing::new();
        
        println!("Initial odds: 50-50");
        
        // Simulate game progression
        let events = vec![
            (900, "Team A scores first", 0, 1000),
            (1800, "Team B equalizes", 1, 1500),
            (2700, "Team A red card", 1, 2000),
            (3600, "Team A scores despite red card", 0, 3000),
            (5400, "Team B misses penalty", 0, 1000),
        ];
        
        for (time, event, team, bet_size) in events {
            state.current_time = time;
            
            let result = solver.solve_pm_amm_price(
                &state,
                team,
                I64F64::from_num(bet_size),
            ).unwrap();
            
            pricing.update_all_prices(&mut state, team, result.new_price, &solver).unwrap();
            
            println!("\nTime: {} min - {}", time / 60, event);
            println!("  Bet: ${} on Team {}", bet_size, if team == 0 { "A" } else { "B" });
            println!("  Odds: A {:.1}% - B {:.1}%",
                state.prices[0].to_num::<f64>() * 100.0,
                state.prices[1].to_num::<f64>() * 100.0
            );
            println!("  LVR: ${:.2}", result.lvr_cost.to_num::<f64>());
        }
        
        println!("\nFinal market state with 30 min remaining:");
        println!("  Team A: {:.1}%", state.prices[0].to_num::<f64>() * 100.0);
        println!("  Team B: {:.1}%", state.prices[1].to_num::<f64>() * 100.0);
    }
}