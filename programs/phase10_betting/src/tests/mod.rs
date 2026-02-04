#[cfg(test)]
mod bootstrap_tests {
    use super::*;
    use crate::bootstrap::*;
    use crate::state::*;
    use crate::types::U64F64;
    use anchor_lang::prelude::*;
    
    #[test]
    fn test_bootstrap_initialization() {
        let mut bootstrap_state = BootstrapState::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1234567890,
        };
        
        bootstrap_state.init(&clock).unwrap();
        
        assert_eq!(bootstrap_state.epoch, 1);
        assert_eq!(bootstrap_state.initial_vault_balance, 0);
        assert_eq!(bootstrap_state.bootstrap_mmt_allocation, 2_000_000 * 10u64.pow(6));
        assert_eq!(bootstrap_state.status, BootstrapStatus::Active);
        assert_eq!(bootstrap_state.early_bonus_multiplier, U64F64::from_num(2));
        assert_eq!(bootstrap_state.max_early_traders, 100);
    }
    
    #[test]
    fn test_bootstrap_fee_calculation() {
        let mut bootstrap_state = BootstrapState::default();
        
        // Test at 0% coverage
        bootstrap_state.current_coverage = U64F64::zero();
        assert_eq!(bootstrap_state.calculate_bootstrap_fee(), 28);
        
        // Test at 50% coverage
        bootstrap_state.current_coverage = U64F64::from_num(0.5);
        let fee = bootstrap_state.calculate_bootstrap_fee();
        assert!(fee > 3 && fee < 28);
        
        // Test at 100% coverage
        bootstrap_state.current_coverage = U64F64::one();
        assert_eq!(bootstrap_state.calculate_bootstrap_fee(), 3);
    }
    
    #[test]
    fn test_early_trader_rewards() {
        let bootstrap_state = BootstrapState {
            early_bonus_multiplier: U64F64::from_num(2),
            ..Default::default()
        };
        
        let tier = IncentiveTier {
            min_volume: 0,
            reward_multiplier: U64F64::from_num(1.5),
            fee_rebate_bps: 5,
            liquidation_priority: 3,
            advanced_features: false,
        };
        
        let trade_volume = 1000 * 10u64.pow(6); // $1000
        
        // Early trader gets 2x * 1.5x = 3x rewards
        let reward = bootstrap_state.calculate_mmt_reward(trade_volume, true, &tier);
        let expected = (trade_volume * 100 / 10_000) * 3; // 1% base * 3x
        assert_eq!(reward, expected);
        
        // Regular trader gets 1.5x rewards
        let reward = bootstrap_state.calculate_mmt_reward(trade_volume, false, &tier);
        let expected = ((trade_volume * 100 / 10_000) as f64 * 1.5) as u64; // 1% base * 1.5x
        assert_eq!(reward, expected);
    }
}

#[cfg(test)]
mod amm_tests {
    use super::*;
    use crate::amm::*;
    use crate::types::U64F64;
    
    #[test]
    fn test_amm_selection() {
        // Test binary market
        let market_type = MarketType::Binary;
        let time_to_expiry = 86_400 * 7; // 7 days
        let amm = HybridAMMSelector::select_amm(
            &market_type,
            time_to_expiry,
            &AMMOverrideFlags::default(),
            &AMMPerformanceMetrics::default(),
        );
        assert_eq!(amm, AMMType::LMSR);
        
        // Test binary market close to expiry
        let time_to_expiry = 86_400 / 2; // 12 hours
        let amm = HybridAMMSelector::select_amm(
            &market_type,
            time_to_expiry,
            &AMMOverrideFlags::default(),
            &AMMPerformanceMetrics::default(),
        );
        assert_eq!(amm, AMMType::PMAMM);
        
        // Test multi-outcome
        let market_type = MarketType::MultiOutcome { count: 10 };
        let amm = HybridAMMSelector::select_amm(
            &market_type,
            time_to_expiry,
            &AMMOverrideFlags::default(),
            &AMMPerformanceMetrics::default(),
        );
        assert_eq!(amm, AMMType::PMAMM);
        
        // Test continuous
        let market_type = MarketType::Continuous {
            min: crate::types::I64F64::from_num(0),
            max: crate::types::I64F64::from_num(100),
            precision: 2,
        };
        let amm = HybridAMMSelector::select_amm(
            &market_type,
            time_to_expiry,
            &AMMOverrideFlags::default(),
            &AMMPerformanceMetrics::default(),
        );
        assert_eq!(amm, AMMType::L2Distribution);
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;
    use crate::router::*;
    use crate::amm::AMMType;
    use crate::types::U64F64;
    
    #[test]
    fn test_synthetic_routing() {
        let mut router = SyntheticRouter {
            router_id: [0; 32],
            verse_id: [1; 32],
            child_markets: vec![
                ChildMarket {
                    market_id: "market1".to_string(),
                    probability: U64F64::from_num(0.6),
                    volume_7d: 100_000,
                    liquidity_depth: 50_000,
                    last_update: 0,
                    amm_type: AMMType::LMSR,
                },
                ChildMarket {
                    market_id: "market2".to_string(),
                    probability: U64F64::from_num(0.65),
                    volume_7d: 200_000,
                    liquidity_depth: 100_000,
                    last_update: 0,
                    amm_type: AMMType::PMAMM,
                },
            ],
            routing_weights: vec![],
            aggregated_prob: U64F64::zero(),
            total_liquidity: 150_000,
            routing_strategy: RoutingStrategy::ProportionalLiquidity,
            performance: RouterPerformance::default(),
            last_update_slot: 0,
        };
        
        // Update weights
        router.update_weights().unwrap();
        assert_eq!(router.routing_weights.len(), 2);
        
        // First market should have ~33% weight, second ~67%
        assert!(router.routing_weights[0] < router.routing_weights[1]);
        
        // Update aggregated probability
        router.update_aggregated_probability().unwrap();
        
        // Should be weighted average closer to 0.65
        assert!(router.aggregated_prob > U64F64::from_num(0.6));
        assert!(router.aggregated_prob < U64F64::from_num(0.65));
    }
    
    #[test]
    fn test_route_calculation() {
        let router = SyntheticRouter {
            router_id: [0; 32],
            verse_id: [1; 32],
            child_markets: vec![
                ChildMarket {
                    market_id: "market1".to_string(),
                    probability: U64F64::from_num(0.6),
                    volume_7d: 100_000,
                    liquidity_depth: 50_000,
                    last_update: 0,
                    amm_type: AMMType::LMSR,
                },
                ChildMarket {
                    market_id: "market2".to_string(),
                    probability: U64F64::from_num(0.65),
                    volume_7d: 200_000,
                    liquidity_depth: 100_000,
                    last_update: 0,
                    amm_type: AMMType::PMAMM,
                },
            ],
            routing_weights: vec![
                U64F64::from_num(0.333),
                U64F64::from_num(0.667),
            ],
            aggregated_prob: U64F64::from_num(0.633),
            total_liquidity: 150_000,
            routing_strategy: RoutingStrategy::ProportionalLiquidity,
            performance: RouterPerformance::default(),
            last_update_slot: 0,
        };
        
        let route_result = RouteExecutor::calculate_route(
            &router,
            10_000, // $10k trade
            true,   // buy
        ).unwrap();
        
        assert_eq!(route_result.route_legs.len(), 2);
        assert_eq!(route_result.unfilled_amount, 0);
        
        // Check proportional allocation
        assert!(route_result.route_legs[0].size < route_result.route_legs[1].size);
        
        // Check fees are reasonable
        assert!(route_result.total_fees < 10_000 * 200 / 10_000); // < 2%
    }
}