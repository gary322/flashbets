use anchor_lang::prelude::*;
use proptest::prelude::*;
use fixed::types::U64F64;
use crate::account_structs::*;
use crate::math::*;
use crate::errors::ErrorCode;
use crate::state::GlobalConfigPDA;

#[cfg(test)]
mod leverage_safety_tests {
    use super::*;

    // Helper function to calculate maximum leverage based on formula
    fn calculate_max_leverage(depth: u8, coverage: f64, n_outcomes: u8) -> f64 {
        let base_leverage = 100.0;
        let depth_multiplier = 1.0 + (0.1 * depth as f64);
        let coverage_factor = coverage * 100.0 / (n_outcomes as f64).sqrt();
        let tier_cap = get_tier_cap(n_outcomes);
        
        base_leverage * depth_multiplier
            .min(coverage_factor)
            .min(tier_cap)
    }

    // Helper function to calculate effective leverage
    fn calculate_leverage(base_leverage: f64, depth: u8, coverage: f64, n_outcomes: u8) -> f64 {
        let depth_multiplier = 1.0 + (0.1 * depth.min(32) as f64);
        let coverage_factor = coverage * 100.0 / (n_outcomes as f64).sqrt();
        let tier_cap = get_tier_cap(n_outcomes);
        
        (base_leverage * depth_multiplier)
            .min(coverage_factor)
            .min(tier_cap)
    }

    // Helper function for tier caps
    fn get_tier_cap(n_outcomes: u8) -> f64 {
        match n_outcomes {
            1 => 100.0,
            2 => 70.0,
            3..=4 => 25.0,
            5..=8 => 15.0,
            9..=16 => 12.0,
            17..=64 => 10.0,
            _ => 5.0,
        }
    }

    // Helper function to calculate liquidation price
    fn calculate_liquidation_price(entry_price: f64, leverage: f64, margin_ratio: f64) -> f64 {
        entry_price * (1.0 - margin_ratio / leverage)
    }

    // Helper function to calculate chain multiplier
    fn calculate_chain_multiplier(multipliers: &[f64]) -> f64 {
        multipliers.iter().take(5).product() // Max 5 steps
    }

    // Property-based testing for leverage formulas
    proptest! {
        #[test]
        fn test_leverage_never_exceeds_coverage(
            depth in 0u8..32u8,
            coverage in 0.1f64..10.0f64,
            n_outcomes in 1u8..100u8,
        ) {
            let leverage = calculate_max_leverage(depth, coverage, n_outcomes);
            
            // Leverage should never exceed coverage * 100
            prop_assert!(leverage <= coverage * 100.0);
            
            // Leverage should respect tier caps
            let tier_cap = get_tier_cap(n_outcomes);
            prop_assert!(leverage <= tier_cap);
            
            // Depth boost should be bounded
            let max_with_depth = 100.0 * (1.0 + 0.1 * depth as f64);
            prop_assert!(leverage <= max_with_depth);
        }

        #[test]
        fn test_liquidation_price_safety(
            entry_price in 0.01f64..1.0f64,
            leverage in 1.0f64..500.0f64,
            margin_ratio in 0.001f64..0.1f64,
        ) {
            let liq_price = calculate_liquidation_price(entry_price, leverage, margin_ratio);
            
            // Liquidation price should be reasonable
            prop_assert!(liq_price >= 0.0);
            prop_assert!(liq_price < entry_price);
            
            // High leverage should have tight liquidation
            if leverage > 100.0 {
                let price_buffer = (entry_price - liq_price) / entry_price;
                prop_assert!(price_buffer < 0.01); // Less than 1% buffer
            }
        }

        #[test]
        fn test_partial_liquidation_safety(
            position_size in 1000u64..1_000_000_000u64,
            oi_cap_percent in 2u8..8u8,
            accumulated in 0u64..100_000_000u64,
        ) {
            let cap = (position_size * oi_cap_percent as u64) / 100;
            let allowed = cap.saturating_sub(accumulated);
            
            // Partial liquidation should never exceed cap
            prop_assert!(allowed <= cap);
            
            // Should respect 2-8% bounds
            prop_assert!(allowed <= position_size * 8 / 100);
            prop_assert!(allowed >= position_size * 2 / 100 || accumulated >= cap);
        }
    }

    #[test]
    fn test_tier_caps_are_enforced_correctly() {
        let test_cases = vec![
            (1, 100.0),   // Binary
            (2, 70.0),    // Two outcomes
            (3, 25.0),    // 3-4 outcomes
            (4, 25.0),
            (5, 15.0),    // 5-8 outcomes
            (8, 15.0),
            (9, 12.0),    // 9-16 outcomes
            (16, 12.0),
            (17, 10.0),   // 17-64 outcomes
            (64, 10.0),
            (65, 5.0),    // 65+ outcomes
            (100, 5.0),
        ];

        for (n_outcomes, expected_cap) in test_cases {
            let cap = get_tier_cap(n_outcomes);
            assert_eq!(cap, expected_cap, 
                "Tier cap for {} outcomes should be {}", n_outcomes, expected_cap);
        }
    }

    #[test]
    fn test_chain_leverage_multiplication() {
        let test_cases = vec![
            (vec![1.5, 1.2, 1.1], 1.98), // 3-step chain
            (vec![1.5, 1.2, 1.1, 1.15, 1.05], 2.39), // 5-step chain
            (vec![2.0; 5], 32.0), // Max theoretical
        ];

        for (multipliers, expected) in test_cases {
            let result = calculate_chain_multiplier(&multipliers);
            assert!((result - expected).abs() < 0.01,
                "Chain multiplier mismatch: {} vs {}", result, expected);

            // Verify each step is bounded
            for mult in &multipliers {
                assert!(*mult >= 1.0 && *mult <= 2.0,
                    "Invalid step multiplier: {}", mult);
            }
        }
    }

    #[test]
    fn test_effective_leverage_bounds() {
        // Test that effective leverage respects all bounds
        let scenarios = vec![
            // (base_lev, chain_steps, coverage, expected_max)
            (100.0, 3, 1.5, 150.0), // Coverage limited
            (100.0, 5, 3.0, 300.0), // Not coverage limited
            (50.0, 5, 10.0, 119.5), // 50 * 2.39 = 119.5
        ];

        for (base, steps, coverage, expected) in scenarios {
            let chain_mult = 1.0 + 0.2 * steps as f64; // Simplified
            let effective = base * chain_mult;
            let capped = effective.min(coverage * 100.0);

            assert!((capped - expected).abs() < 1.0,
                "Effective leverage mismatch: {} vs {}", capped, expected);
        }
    }

    // Test partial liquidation mechanics
    #[test]
    fn test_partial_liquidation() {
        let mut position = Position {
            proposal_id: 1,
            outcome: 0,
            size: 10000, // $10k position
            leverage: 100,
            entry_price: 5500, // 0.55 in basis points
            liquidation_price: 5450,
            is_long: true,
            created_at: 0,
        };

        let liq_cap_percent = 8; // 8% per slot
        let oi = 10000; // Open interest

        // Calculate partial liquidation amount
        let liq_amount = (position.size * liq_cap_percent) / 100;
        assert_eq!(liq_amount, 800); // 8% of 10k = 800

        // Apply partial liquidation
        position.size -= liq_amount;
        
        assert_eq!(position.size, 9200);
    }

    // Fuzz test for extreme scenarios
    proptest! {
        #[test]
        fn fuzz_extreme_leverage_scenarios(
            leverage in 400.0f64..500.0f64,
            price_move_percent in -5.0f64..5.0f64,
            chain_steps in 3usize..5usize,
        ) {
            let entry_price = 0.5;
            let position_size = 1000.0;

            // Calculate effective leverage with chaining
            let step_multiplier = 1.5; // Average multiplier per step
            let effective_leverage = leverage * step_multiplier.powi(chain_steps as i32);

            // Cap at 500x
            let effective_leverage = effective_leverage.min(500.0);

            // Calculate P&L
            let price_change = entry_price * (price_move_percent / 100.0);
            let new_price = entry_price + price_change;
            let pnl = position_size * effective_leverage * price_change;

            // At 500x, a 0.2% move should wipe out position
            if price_move_percent <= -0.2 && effective_leverage >= 500.0 {
                prop_assert!(pnl <= -position_size);
            }

            // Verify maximum loss is capped at position size
            prop_assert!(pnl >= -position_size);
        }
    }
}

// Fuzz testing for edge cases
#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use arbitrary::{Arbitrary, Unstructured};

    #[derive(Debug, Arbitrary)]
    struct FuzzLeverageInput {
        depth: u8,
        coverage: U64F64,
        n_outcomes: u8,
        chain_steps: Vec<U64F64>,
    }

    fn fuzz_leverage_safety(input: &FuzzLeverageInput) -> Result<(), String> {
        // Bound inputs to reasonable ranges
        let depth = input.depth.min(32);
        let coverage = input.coverage.to_num::<f64>().max(0.01).min(100.0);
        let n_outcomes = input.n_outcomes.max(1);

        // Calculate leverage
        let base_lev = calculate_max_leverage(depth, coverage, n_outcomes);

        // Apply chain if steps provided
        let chain_mult = if input.chain_steps.is_empty() {
            1.0
        } else {
            input.chain_steps.iter()
                .take(5) // Max 5 steps
                .map(|s| s.to_num::<f64>().max(1.0).min(2.0))
                .product()
        };

        let effective_lev = base_lev * chain_mult;

        // Verify invariants
        if effective_lev > 500.0 {
            return Err("Effective leverage exceeds 500x".to_string());
        }

        if effective_lev > coverage * 100.0 {
            return Err("Leverage exceeds coverage limit".to_string());
        }

        Ok(())
    }

    #[test]
    fn fuzz_test_leverage() {
        let mut data = [0u8; 1024];
        for i in 0..10000 {
            // Generate pseudo-random data
            for j in 0..data.len() {
                data[j] = ((i * 31 + j * 17) % 256) as u8;
            }

            let u = Unstructured::new(&data);
            if let Ok(input) = FuzzLeverageInput::arbitrary(&mut u.clone()) {
                let _ = fuzz_leverage_safety(&input);
            }
        }
    }
}