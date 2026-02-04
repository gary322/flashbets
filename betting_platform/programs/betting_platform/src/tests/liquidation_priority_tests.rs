#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidation_priority::*;
    use crate::fixed_types::U64F64;
    use anchor_lang::prelude::*;
    use std::cmp::Ordering;

    #[test]
    fn test_liquidation_queue_initialization() {
        let mut queue = LiquidationQueue {
            queue_id: [1; 32],
            at_risk_positions: vec![],
            active_liquidations: vec![],
            config: LiquidationConfig::default(),
            metrics: LiquidationMetrics::default(),
            keeper_rewards_pool: 0,
            last_update_slot: 0,
        };
        
        // Initialize with CLAUDE.md parameters
        queue.config = LiquidationConfig {
            min_liquidation_size: 10 * 10u64.pow(6), // $10 minimum
            max_liquidation_per_slot: U64F64::from_num(0.08), // 8% max
            liquidation_penalty_bps: 50, // 5bp to keeper
            grace_period: 180, // ~1 minute grace
            staking_tier_boost: [0, 10, 20, 30, 50], // Priority boost per tier
            bootstrap_protection_multiplier: U64F64::from_num(1.5), // 50% more time
        };
        
        assert_eq!(queue.config.max_liquidation_per_slot, U64F64::from_num(0.08));
        assert_eq!(queue.config.liquidation_penalty_bps, 50);
        assert_eq!(queue.config.bootstrap_protection_multiplier, U64F64::from_num(1.5));
    }

    #[test]
    fn test_staking_tier_determination() {
        assert_eq!(get_staking_tier(0), StakingTier::None);
        assert_eq!(get_staking_tier(50_000_000), StakingTier::None);
        assert_eq!(get_staking_tier(100_000_000), StakingTier::Bronze);
        assert_eq!(get_staking_tier(500_000_000), StakingTier::Bronze);
        assert_eq!(get_staking_tier(1_000_000_000), StakingTier::Silver);
        assert_eq!(get_staking_tier(5_000_000_000), StakingTier::Silver);
        assert_eq!(get_staking_tier(10_000_000_000), StakingTier::Gold);
        assert_eq!(get_staking_tier(50_000_000_000), StakingTier::Gold);
        assert_eq!(get_staking_tier(100_000_000_000), StakingTier::Platinum);
        assert_eq!(get_staking_tier(1_000_000_000_000), StakingTier::Platinum);
    }

    #[test]
    fn test_risk_score_calculation() {
        // Test liquidatable position
        let score = LiquidationEngine::calculate_risk_score(
            U64F64::from_num(90.0),  // mark price
            U64F64::from_num(100.0), // entry price
            U64F64::from_num(50.0),  // 50x leverage
            true, // is_long
        );
        assert_eq!(score, 100); // Already liquidatable
        
        // Test position close to liquidation
        let score = LiquidationEngine::calculate_risk_score(
            U64F64::from_num(99.0),  // mark price
            U64F64::from_num(100.0), // entry price
            U64F64::from_num(20.0),  // 20x leverage
            true, // is_long
        );
        assert_eq!(score, 90); // <5% margin remaining
        
        // Test healthy position
        let score = LiquidationEngine::calculate_risk_score(
            U64F64::from_num(100.0), // mark price
            U64F64::from_num(100.0), // entry price
            U64F64::from_num(5.0),   // 5x leverage
            true, // is_long
        );
        assert_eq!(score, 10); // >30% margin remaining
    }

    #[test]
    fn test_priority_score_calculation() {
        // Base position with no protection
        let base_position = AtRiskPosition {
            position_id: [1; 32],
            owner: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 10_000,
            entry_price: U64F64::from_num(100),
            mark_price: U64F64::from_num(95),
            effective_leverage: U64F64::from_num(50),
            distance_to_liquidation: U64F64::from_num(0.02),
            risk_score: 80,
            staking_tier: StakingTier::None,
            bootstrap_priority: 0,
            time_at_risk: 100,
            is_chained: false,
            chain_depth: 0,
        };
        
        let base_priority = base_position.calculate_priority_score();
        
        // Position with Gold staking protection
        let mut staked_position = base_position.clone();
        staked_position.position_id = [2; 32];
        staked_position.staking_tier = StakingTier::Gold;
        
        let staked_priority = staked_position.calculate_priority_score();
        assert!(base_priority > staked_priority); // Staking provides protection
        
        // Position with bootstrap protection
        let mut bootstrap_position = base_position.clone();
        bootstrap_position.position_id = [3; 32];
        bootstrap_position.bootstrap_priority = 3;
        
        let bootstrap_priority = bootstrap_position.calculate_priority_score();
        assert!(base_priority > bootstrap_priority); // Bootstrap provides protection
        
        // High risk chained position
        let mut chained_position = base_position.clone();
        chained_position.position_id = [4; 32];
        chained_position.is_chained = true;
        chained_position.chain_depth = 3;
        chained_position.distance_to_liquidation = U64F64::from_num(0.005); // Very close
        
        let chained_priority = chained_position.calculate_priority_score();
        assert!(chained_priority > base_priority); // Chained positions have higher priority
    }

    #[test]
    fn test_liquidation_queue_ordering() {
        let mut positions = vec![
            AtRiskPosition {
                position_id: [1; 32],
                owner: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 10_000,
                entry_price: U64F64::from_num(100),
                mark_price: U64F64::from_num(95),
                effective_leverage: U64F64::from_num(50),
                distance_to_liquidation: U64F64::from_num(0.02),
                risk_score: 80,
                staking_tier: StakingTier::Gold,
                bootstrap_priority: 0,
                time_at_risk: 100,
                is_chained: false,
                chain_depth: 0,
            },
            AtRiskPosition {
                position_id: [2; 32],
                owner: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 10_000,
                entry_price: U64F64::from_num(100),
                mark_price: U64F64::from_num(90),
                effective_leverage: U64F64::from_num(50),
                distance_to_liquidation: U64F64::from_num(-0.01), // Already liquidatable
                risk_score: 100,
                staking_tier: StakingTier::None,
                bootstrap_priority: 0,
                time_at_risk: 100,
                is_chained: false,
                chain_depth: 0,
            },
            AtRiskPosition {
                position_id: [3; 32],
                owner: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 10_000,
                entry_price: U64F64::from_num(100),
                mark_price: U64F64::from_num(92),
                effective_leverage: U64F64::from_num(50),
                distance_to_liquidation: U64F64::from_num(-0.005),
                risk_score: 95,
                staking_tier: StakingTier::None,
                bootstrap_priority: 0,
                time_at_risk: 50,
                is_chained: true,
                chain_depth: 2,
            },
        ];
        
        // Sort by priority (higher score = higher priority)
        positions.sort_by(|a, b| b.cmp(a));
        
        // Verify order: chained position should be first despite lower risk score
        assert_eq!(positions[0].position_id, [3; 32]); // Chained position
        assert_eq!(positions[1].position_id, [2; 32]); // Unprotected high risk
        assert_eq!(positions[2].position_id, [1; 32]); // Gold tier protected
    }

    #[test]
    fn test_partial_liquidation_limit() {
        let mut queue = LiquidationQueue {
            queue_id: [1; 32],
            at_risk_positions: vec![],
            active_liquidations: vec![],
            config: LiquidationConfig {
                min_liquidation_size: 10,
                max_liquidation_per_slot: U64F64::from_num(0.08), // 8% max
                liquidation_penalty_bps: 50,
                grace_period: 180,
                staking_tier_boost: [0, 10, 20, 30, 50],
                bootstrap_protection_multiplier: U64F64::from_num(1.5),
            },
            metrics: LiquidationMetrics::default(),
            keeper_rewards_pool: 10_000,
            last_update_slot: 0,
        };
        
        // Add large position
        queue.at_risk_positions.push(AtRiskPosition {
            position_id: [1; 32],
            owner: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 10_000,
            entry_price: U64F64::from_num(100),
            mark_price: U64F64::from_num(90),
            effective_leverage: U64F64::from_num(50),
            distance_to_liquidation: U64F64::from_num(-0.01),
            risk_score: 100,
            staking_tier: StakingTier::None,
            bootstrap_priority: 0,
            time_at_risk: 100,
            is_chained: false,
            chain_depth: 0,
        });
        
        let orders = LiquidationEngine::process_queue(&mut queue, 10, 1000).unwrap();
        
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].liquidation_amount, 800); // 8% of 10,000
        assert_eq!(orders[0].keeper_reward, 4); // 0.05% of 800
    }

    #[test]
    fn test_keeper_reward_calculation() {
        let queue = LiquidationQueue {
            queue_id: [1; 32],
            at_risk_positions: vec![],
            active_liquidations: vec![],
            config: LiquidationConfig {
                min_liquidation_size: 10,
                max_liquidation_per_slot: U64F64::from_num(0.08),
                liquidation_penalty_bps: 50, // 5bp
                grace_period: 180,
                staking_tier_boost: [0, 10, 20, 30, 50],
                bootstrap_protection_multiplier: U64F64::from_num(1.5),
            },
            metrics: LiquidationMetrics::default(),
            keeper_rewards_pool: 10_000,
            last_update_slot: 0,
        };
        
        // Test various liquidation amounts
        let test_amounts = vec![1_000, 10_000, 100_000, 1_000_000];
        
        for amount in test_amounts {
            let reward = (amount as u128 * queue.config.liquidation_penalty_bps as u128 / 10_000) as u64;
            let expected = amount * 5 / 10_000; // 0.05%
            assert_eq!(reward, expected);
        }
    }

    #[test]
    fn test_add_remove_at_risk_positions() {
        let mut queue = LiquidationQueue {
            queue_id: [1; 32],
            at_risk_positions: vec![],
            active_liquidations: vec![],
            config: LiquidationConfig::default(),
            metrics: LiquidationMetrics::default(),
            keeper_rewards_pool: 0,
            last_update_slot: 0,
        };
        
        let position = AtRiskPosition {
            position_id: [1; 32],
            owner: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 10_000,
            entry_price: U64F64::from_num(100),
            mark_price: U64F64::from_num(95),
            effective_leverage: U64F64::from_num(50),
            distance_to_liquidation: U64F64::from_num(0.02),
            risk_score: 80,
            staking_tier: StakingTier::None,
            bootstrap_priority: 0,
            time_at_risk: 100,
            is_chained: false,
            chain_depth: 0,
        };
        
        // Add position
        LiquidationEngine::add_at_risk_position(&mut queue, position.clone()).unwrap();
        assert_eq!(queue.at_risk_positions.len(), 1);
        
        // Update existing position
        let mut updated_position = position.clone();
        updated_position.risk_score = 90;
        LiquidationEngine::add_at_risk_position(&mut queue, updated_position).unwrap();
        assert_eq!(queue.at_risk_positions.len(), 1);
        assert_eq!(queue.at_risk_positions[0].risk_score, 90);
        
        // Remove position
        LiquidationEngine::remove_position(&mut queue, [1; 32]).unwrap();
        assert_eq!(queue.at_risk_positions.len(), 0);
    }

    #[test]
    fn test_liquidation_metrics_tracking() {
        let mut queue = LiquidationQueue {
            queue_id: [1; 32],
            at_risk_positions: vec![],
            active_liquidations: vec![],
            config: LiquidationConfig {
                min_liquidation_size: 10,
                max_liquidation_per_slot: U64F64::from_num(0.08),
                liquidation_penalty_bps: 50,
                grace_period: 180,
                staking_tier_boost: [0, 10, 20, 30, 50],
                bootstrap_protection_multiplier: U64F64::from_num(1.5),
            },
            metrics: LiquidationMetrics::default(),
            keeper_rewards_pool: 10_000,
            last_update_slot: 0,
        };
        
        // Add multiple positions
        for i in 0..3 {
            queue.at_risk_positions.push(AtRiskPosition {
                position_id: [i; 32],
                owner: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 1000 * (i as u64 + 1),
                entry_price: U64F64::from_num(100),
                mark_price: U64F64::from_num(90),
                effective_leverage: U64F64::from_num(50),
                distance_to_liquidation: U64F64::from_num(-0.01),
                risk_score: 100,
                staking_tier: StakingTier::None,
                bootstrap_priority: 0,
                time_at_risk: 100,
                is_chained: false,
                chain_depth: 0,
            });
        }
        
        let orders = LiquidationEngine::process_queue(&mut queue, 10, 1000).unwrap();
        
        assert_eq!(orders.len(), 3);
        assert_eq!(queue.metrics.total_liquidations, 3);
        
        let total_volume: u64 = orders.iter().map(|o| o.liquidation_amount).sum();
        assert_eq!(queue.metrics.total_volume_liquidated, total_volume);
        assert_eq!(queue.metrics.avg_liquidation_size, total_volume / 3);
    }
}