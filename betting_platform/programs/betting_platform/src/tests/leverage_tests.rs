#[cfg(test)]
mod leverage_tests {
    use super::super::*;
    use crate::trading::*;
    use crate::account_structs::{self, LeverageTier};
    use crate::fixed_math::{self, PRECISION};

    #[test]
    fn test_leverage_calculation_tier_caps() {
        // Test tier caps for different outcome counts
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 1), 100);
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 2), 70);
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 4), 25);
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 6), 15);
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 10), 12);
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 20), 10);
        assert_eq!(calculate_max_leverage(u128::MAX, 0, 65), 5);
    }

    #[test]
    fn test_leverage_calculation_depth_boost() {
        // Test depth boost with infinite coverage
        assert_eq!(calculate_max_leverage(u128::MAX, 10, 1), 100); // Capped at 100
        assert_eq!(calculate_max_leverage(u128::MAX, 5, 4), 25); // Capped by tier
        
        // Test depth boost formula
        let depth_5_boost = calculate_max_leverage(u128::MAX, 5, 100); // High outcome count
        assert_eq!(depth_5_boost, 5); // Should be capped by tier (5 for >64 outcomes)
    }

    #[test]
    fn test_leverage_calculation_coverage_limit() {
        let low_coverage = PRECISION / 2;
        assert_eq!(calculate_max_leverage(low_coverage, 0, 1), 50);
        
        let medium_coverage = PRECISION;
        assert_eq!(calculate_max_leverage(medium_coverage, 0, 1), 100);
        
        // With multiple outcomes
        let coverage = PRECISION * 2;
        let leverage_2_outcomes = calculate_max_leverage(coverage, 0, 2);
        assert!(leverage_2_outcomes <= 70); // Tier cap
    }

    #[test]
    fn test_required_collateral_calculation() {
        let position_size = 1_000_000_000; // 1000 USDC
        let leverage = 10;
        
        // High coverage scenario
        let high_coverage = PRECISION * 2;
        let collateral_high = calculate_required_collateral(position_size, leverage, high_coverage);
        assert_eq!(collateral_high, 100_000_000); // 100 USDC (position_size / leverage)
        
        // Low coverage scenario (should require more collateral)
        let low_coverage = PRECISION / 2;
        let collateral_low = calculate_required_collateral(position_size, leverage, low_coverage);
        assert!(collateral_low > collateral_high);
        assert!(collateral_low <= 150_000_000); // Should not exceed 150% of base
    }

    #[test]
    fn test_liquidation_price_calculation() {
        let entry_price = 1_000_000; // $1.00
        let leverage = 10;
        
        // Long position
        let liq_price_long = calculate_liquidation_price(entry_price, leverage, true, PRECISION);
        assert!(liq_price_long < entry_price); // Should be below entry for longs
        assert!(liq_price_long >= 900_000); // Should not be more than 10% below
        
        // Short position
        let liq_price_short = calculate_liquidation_price(entry_price, leverage, false, PRECISION);
        assert!(liq_price_short > entry_price); // Should be above entry for shorts
        assert!(liq_price_short <= 1_100_000); // Should not be more than 10% above
    }

    #[test]
    fn test_coverage_calculation() {
        // Test with no open interest
        let coverage_no_oi = calculate_coverage(1_000_000, 0, 1);
        assert_eq!(coverage_no_oi, u128::MAX);
        
        // Test with normal values
        let vault = 10_000_000_000; // 10,000 USDC
        let total_oi = 1_000_000_000; // 1,000 USDC
        let coverage = calculate_coverage(vault, total_oi, 1);
        assert!(coverage > PRECISION); // Should be > 1.0
        
        // Test with multiple outcomes
        let coverage_multi = calculate_coverage(vault, total_oi, 4);
        assert!(coverage_multi < coverage); // Higher tail loss = lower coverage
    }

    #[test]
    fn test_tail_loss_calculation() {
        assert_eq!(calculate_tail_loss(1), PRECISION);
        assert_eq!(calculate_tail_loss(3), PRECISION * 2);
        assert_eq!(calculate_tail_loss(6), PRECISION * 3);
        assert_eq!(calculate_tail_loss(10), PRECISION * 4);
    }

    #[test]
    fn test_edge_cases() {
        // Test zero leverage
        let collateral = calculate_required_collateral(1_000_000, 0, PRECISION);
        assert_eq!(collateral, 1_000_000); // Should return position size
        
        // Test zero coverage
        let max_lev = calculate_max_leverage(0, 0, 1);
        assert_eq!(max_lev, 0);
        
        // Test extreme values
        let huge_coverage = u128::MAX / 2;
        let leverage = calculate_max_leverage(huge_coverage, 32, 1);
        assert_eq!(leverage, 100); // Should still be capped at 100
    }

    #[test]
    fn test_leverage_with_real_scenarios() {
        struct Scenario {
            name: &'static str,
            coverage: u128,
            depth: u8,
            outcomes: u32,
            expected_max: u64,
        }
        
        let scenarios = vec![
            Scenario {
                name: "Binary market, good coverage",
                coverage: PRECISION * 2,
                depth: 0,
                outcomes: 1,
                expected_max: 100,
            },
            Scenario {
                name: "Multi-outcome, medium coverage",
                coverage: PRECISION,
                depth: 5,
                outcomes: 4,
                expected_max: 25,
            },
            Scenario {
                name: "High outcome, low coverage",
                coverage: PRECISION / 2,
                depth: 0,
                outcomes: 20,
                expected_max: 10,
            },
            Scenario {
                name: "Deep hierarchy, good coverage",
                coverage: PRECISION * 3,
                depth: 10,
                outcomes: 2,
                expected_max: 70,
            },
        ];
        
        for scenario in scenarios {
            let actual = calculate_max_leverage(
                scenario.coverage,
                scenario.depth,
                scenario.outcomes
            );
            assert_eq!(
                actual,
                scenario.expected_max,
                "Failed for scenario: {}",
                scenario.name
            );
        }
    }

    #[test]
    fn test_position_health_calculation() {
        let mut map_entry = MapEntryPDA {
            user: Default::default(),
            verse_id: 1,
            positions: vec![
                Position {
                    proposal_id: 1,
                    outcome: 0,
                    size: 1_000_000,
                    leverage: 10,
                    entry_price: 1_000_000,
                    liquidation_price: 950_000,
                    is_long: true,
                    created_at: 0,
                }
            ],
            total_collateral: 100_000,
            total_borrowed: 0,
            last_update: 0,
            realized_pnl: 0,
            unrealized_pnl: 0,
            health_factor: 10_000,
        };
        
        // Price at entry - should be healthy
        let health = map_entry.calculate_health(&[1_000_000]);
        assert!(health > 5_000); // Should be well above 50%
        
        // Price moved against position
        let health_bad = map_entry.calculate_health(&[960_000]);
        assert!(health_bad < health); // Health should decrease
        
        // Price near liquidation
        let health_critical = map_entry.calculate_health(&[951_000]);
        assert!(health_critical < 1_000); // Should be very low
    }

    #[test]
    fn test_leverage_safety_margins() {
        // Ensure safety margins are working
        let position_size = 10_000_000_000; // 10,000 USDC
        let leverage = 50;
        
        // Low coverage should require much more collateral
        let low_cov_collateral = calculate_required_collateral(
            position_size,
            leverage,
            PRECISION / 4
        );
        
        let high_cov_collateral = calculate_required_collateral(
            position_size,
            leverage,
            PRECISION * 2
        );
        
        // Low coverage should require at least 1.5x more collateral
        assert!(low_cov_collateral >= (high_cov_collateral * 3) / 2);
    }
}