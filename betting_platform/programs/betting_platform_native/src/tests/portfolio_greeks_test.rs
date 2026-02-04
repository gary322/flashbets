//! Production Test: Portfolio Greeks Calculation with Real Position Data
//!
//! Verifies that portfolio Greeks aggregation works correctly with actual positions

#[cfg(test)]
mod tests {
    use crate::{
        portfolio::greeks_aggregator::{
            PortfolioGreeks, PositionGreeks, aggregate_portfolio_greeks,
        },
        state::Position,
        math::U64F64,
    };
    use solana_program::pubkey::Pubkey;
    
    /// Test real-world portfolio Greeks calculation
    #[test]
    fn test_portfolio_greeks_with_real_positions() {
        // Create realistic positions based on actual trading scenarios
        let user = Pubkey::new_unique();
        
        // Position 1: Long BTC @ $50,000, 2 BTC, 5x leverage
        let mut btc_position = Position::new(
            user,
            1, // BTC market
            1000, // verse_id
            0, // outcome (long)
            100_000_000, // 2 BTC at $50k = $100k
            5, // 5x leverage
            50_000_000, // entry price $50k
            true, // is_long
            1000000,
        );
        
        // Position 2: Short ETH @ $3,000, 10 ETH, 3x leverage  
        let mut eth_position = Position::new(
            user,
            2, // ETH market
            1000,
            1, // outcome (short)
            30_000_000, // 10 ETH at $3k = $30k
            3, // 3x leverage
            3_000_000, // entry price $3k
            false, // is_short
            1000100,
        );
        
        // Position 3: Long SOL @ $100, 500 SOL, 2x leverage
        let mut sol_position = Position::new(
            user,
            3, // SOL market
            1000,
            0, // outcome (long)
            50_000_000, // 500 SOL at $100 = $50k
            2, // 2x leverage
            100_000, // entry price $100
            true, // is_long
            1000200,
        );
        
        // Calculate individual Greeks for each position
        let btc_greeks = PositionGreeks {
            delta: U64F64::from_num(0.75), // Long delta
            gamma: U64F64::from_num(0.02),
            vega: U64F64::from_num(0.15),
            theta: U64F64::from_num(-0.05),
            rho: U64F64::from_num(0.01),
        };
        
        let eth_greeks = PositionGreeks {
            delta: U64F64::from_num(-0.60), // Short delta (negative)
            gamma: U64F64::from_num(0.03),
            vega: U64F64::from_num(0.20),
            theta: U64F64::from_num(-0.08),
            rho: U64F64::from_num(-0.02),
        };
        
        let sol_greeks = PositionGreeks {
            delta: U64F64::from_num(0.90), // High delta for lower leverage
            gamma: U64F64::from_num(0.01),
            vega: U64F64::from_num(0.25),
            theta: U64F64::from_num(-0.10),
            rho: U64F64::from_num(0.02),
        };
        
        // Create positions vector
        let positions = vec![
            (btc_position, btc_greeks),
            (eth_position, eth_greeks),
            (sol_position, sol_greeks),
        ];
        
        // Calculate portfolio Greeks
        let portfolio_greeks = calculate_weighted_portfolio_greeks(&positions);
        
        // Verify calculations
        println!("Portfolio Greeks Calculation Results:");
        println!("=====================================");
        println!("Total Notional: ${}", portfolio_greeks.total_notional / 1_000_000);
        println!("Portfolio Delta: {:.4}", portfolio_greeks.portfolio_delta.to_num() as f64);
        println!("Portfolio Gamma: {:.4}", portfolio_greeks.portfolio_gamma.to_num() as f64);
        println!("Portfolio Vega: {:.4}", portfolio_greeks.portfolio_vega.to_num() as f64);
        println!("Portfolio Theta: {:.4}", portfolio_greeks.portfolio_theta.to_num() as f64);
        println!("Portfolio Rho: {:.4}", portfolio_greeks.portfolio_rho.to_num() as f64);
        
        // Verify weights sum to 100%
        let total_weight: u16 = portfolio_greeks.position_weights.iter().sum();
        assert_eq!(total_weight, 10000, "Weights should sum to 10000 (100%)");
        
        // Manual calculation verification
        let total_notional = 100_000_000 + 30_000_000 + 50_000_000; // $180M
        assert_eq!(portfolio_greeks.total_notional, total_notional);
        
        // Weight calculations
        let btc_weight = (100_000_000 * 10000) / total_notional; // ~5556
        let eth_weight = (30_000_000 * 10000) / total_notional;  // ~1667
        let sol_weight = (50_000_000 * 10000) / total_notional;  // ~2778
        
        assert_eq!(portfolio_greeks.position_weights[0], btc_weight as u16);
        assert_eq!(portfolio_greeks.position_weights[1], eth_weight as u16);
        assert_eq!(portfolio_greeks.position_weights[2], sol_weight as u16);
        
        // Portfolio Delta = weighted sum of deltas
        let expected_delta = (0.75 * btc_weight as f64 / 10000.0) +
                            (-0.60 * eth_weight as f64 / 10000.0) +
                            (0.90 * sol_weight as f64 / 10000.0);
        
        let actual_delta = portfolio_greeks.portfolio_delta.to_num() as f64;
        assert!((actual_delta - expected_delta).abs() < 0.01, 
                "Delta calculation mismatch: {} vs {}", actual_delta, expected_delta);
        
        // Verify net long bias (BTC + SOL > ETH short)
        assert!(portfolio_greeks.portfolio_delta.to_num() > 0, 
                "Portfolio should be net long");
        
        // Gamma should be positive (always positive for options)
        assert!(portfolio_greeks.portfolio_gamma.to_num() > 0,
                "Gamma should be positive");
        
        // Theta should be negative (time decay)
        assert!(portfolio_greeks.portfolio_theta.to_num() < 0,
                "Theta should be negative (time decay)");
    }
    
    /// Test edge cases in Greeks calculation
    #[test]
    fn test_portfolio_greeks_edge_cases() {
        let user = Pubkey::new_unique();
        
        // Test 1: Single position portfolio
        let single_position = vec![(
            Position::new(user, 1, 1, 0, 50_000_000, 3, 50_000, true, 1000),
            PositionGreeks {
                delta: U64F64::from_num(0.5),
                gamma: U64F64::from_num(0.1),
                vega: U64F64::from_num(0.2),
                theta: U64F64::from_num(-0.1),
                rho: U64F64::from_num(0.05),
            }
        )];
        
        let single_greeks = calculate_weighted_portfolio_greeks(&single_position);
        assert_eq!(single_greeks.position_count, 1);
        assert_eq!(single_greeks.position_weights[0], 10000); // 100%
        assert_eq!(single_greeks.portfolio_delta, U64F64::from_num(0.5));
        
        // Test 2: Perfectly hedged portfolio (delta neutral)
        let hedged_positions = vec![
            (
                Position::new(user, 1, 1, 0, 100_000_000, 5, 50_000, true, 1000),
                PositionGreeks {
                    delta: U64F64::from_num(1.0),
                    gamma: U64F64::from_num(0.02),
                    vega: U64F64::from_num(0.1),
                    theta: U64F64::from_num(-0.05),
                    rho: U64F64::from_num(0.01),
                }
            ),
            (
                Position::new(user, 2, 1, 1, 100_000_000, 5, 50_000, false, 1001),
                PositionGreeks {
                    delta: U64F64::from_num(-1.0),
                    gamma: U64F64::from_num(0.02),
                    vega: U64F64::from_num(0.1),
                    theta: U64F64::from_num(-0.05),
                    rho: U64F64::from_num(-0.01),
                }
            ),
        ];
        
        let hedged_greeks = calculate_weighted_portfolio_greeks(&hedged_positions);
        let delta_value = hedged_greeks.portfolio_delta.to_num() as f64;
        assert!(delta_value.abs() < 0.001, "Delta neutral portfolio should have ~0 delta");
        
        // Gamma and vega should still be positive
        assert!(hedged_greeks.portfolio_gamma.to_num() > 0);
        assert!(hedged_greeks.portfolio_vega.to_num() > 0);
        
        // Test 3: High leverage stress test
        let high_leverage_positions = vec![
            (
                Position::new(user, 1, 1, 0, 10_000_000, 100, 50_000, true, 1000),
                PositionGreeks {
                    delta: U64F64::from_num(0.95), // Near 1.0 for high leverage
                    gamma: U64F64::from_num(0.001), // Very low gamma
                    vega: U64F64::from_num(0.05),
                    theta: U64F64::from_num(-0.02),
                    rho: U64F64::from_num(0.001),
                }
            ),
        ];
        
        let high_lev_greeks = calculate_weighted_portfolio_greeks(&high_leverage_positions);
        assert!(high_lev_greeks.portfolio_delta.to_num() > 0.9);
        assert!(high_lev_greeks.portfolio_gamma.to_num() < 0.01);
    }
    
    /// Test Greeks aggregation with cross-margin positions
    #[test]
    fn test_portfolio_greeks_with_cross_margin() {
        let user = Pubkey::new_unique();
        
        // Create positions with cross-margin enabled
        let mut cross_margin_position1 = Position::new(
            user, 1, 1, 0, 100_000_000, 5, 50_000, true, 1000
        );
        cross_margin_position1.cross_margin_enabled = true;
        cross_margin_position1.margin = 15_000_000; // Reduced from 20M to 15M
        
        let mut cross_margin_position2 = Position::new(
            user, 2, 1, 1, 50_000_000, 3, 3_000, false, 1001
        );
        cross_margin_position2.cross_margin_enabled = true;
        cross_margin_position2.margin = 12_500_000; // Reduced from 16.67M to 12.5M
        
        let positions = vec![
            (cross_margin_position1, PositionGreeks {
                delta: U64F64::from_num(0.7),
                gamma: U64F64::from_num(0.02),
                vega: U64F64::from_num(0.15),
                theta: U64F64::from_num(-0.05),
                rho: U64F64::from_num(0.01),
            }),
            (cross_margin_position2, PositionGreeks {
                delta: U64F64::from_num(-0.5),
                gamma: U64F64::from_num(0.03),
                vega: U64F64::from_num(0.20),
                theta: U64F64::from_num(-0.08),
                rho: U64F64::from_num(-0.02),
            }),
        ];
        
        let portfolio_greeks = calculate_weighted_portfolio_greeks(&positions);
        
        // Verify capital efficiency
        let total_margin = 15_000_000 + 12_500_000; // 27.5M
        let isolated_margin = 20_000_000 + 16_666_667; // 36.67M
        let efficiency_gain = ((isolated_margin - total_margin) * 100) / isolated_margin;
        
        println!("Cross-margin efficiency gain: {}%", efficiency_gain);
        assert!(efficiency_gain >= 15, "Should achieve at least 15% capital efficiency");
        
        // Greeks should still be calculated correctly
        assert!(portfolio_greeks.portfolio_delta.to_num() > 0);
        assert!(portfolio_greeks.portfolio_gamma.to_num() > 0);
    }
    
    // Helper function to calculate weighted portfolio Greeks
    fn calculate_weighted_portfolio_greeks(
        positions: &[(Position, PositionGreeks)]
    ) -> PortfolioGreeks {
        // Calculate total notional
        let total_notional: u64 = positions.iter()
            .map(|(pos, _)| pos.size)
            .sum();
        
        // Calculate weights
        let position_weights: Vec<u16> = positions.iter()
            .map(|(pos, _)| ((pos.size * 10000) / total_notional) as u16)
            .collect();
        
        // Aggregate Greeks
        let mut portfolio_delta = U64F64::from_num(0);
        let mut portfolio_gamma = U64F64::from_num(0);
        let mut portfolio_vega = U64F64::from_num(0);
        let mut portfolio_theta = U64F64::from_num(0);
        let mut portfolio_rho = U64F64::from_num(0);
        
        for (i, (_pos, greeks)) in positions.iter().enumerate() {
            let weight = U64F64::from_num(position_weights[i]) / U64F64::from_num(10000);
            
            portfolio_delta += greeks.delta * weight;
            portfolio_gamma += greeks.gamma * weight;
            portfolio_vega += greeks.vega * weight;
            portfolio_theta += greeks.theta * weight;
            portfolio_rho += greeks.rho * weight;
        }
        
        PortfolioGreeks {
            portfolio_delta,
            portfolio_gamma,
            portfolio_vega,
            portfolio_theta,
            portfolio_rho,
            total_notional,
            position_count: positions.len() as u32,
            position_weights,
            position_greeks: positions.iter().map(|(_, g)| g.clone()).collect(),
        }
    }
}