// Standalone test runner for Phase 11 modules
// This allows us to test our implementation in isolation

#[cfg(test)]
mod phase_11_tests {
    use anchor_lang::prelude::*;
    use std::collections::VecDeque;

    // Import our fixed types
    use crate::fixed_types::{U64F64, I64F64};
    
    // Import attack detection module
    use crate::attack_detection::{
        AttackDetector, AttackPattern, AttackType, AttackSeverity,
        TradeSnapshot, PriceMovementTracker, VolumeAnomalyDetector,
        FlashLoanDetector, CrossVerseTracker, WashTradeDetector,
        SecurityAlert, AlertType, SecurityAction, AlertData,
        PriceChange, VolumeData, PositionChange, VerseCorrelation,
        CrossVerseTrade, TraderActivity
    };
    
    // Import circuit breaker module
    use crate::circuit_breaker::{
        CircuitBreaker, BreakerState, BreakerTrigger, HaltReason,
        CoverageBreaker, PriceBreaker, VolumeBreaker, LiquidationBreaker,
        CongestionBreaker, BreakerAction
    };
    
    // Import liquidation priority module
    use crate::liquidation_priority::{
        LiquidationQueue, AtRiskPosition, ActiveLiquidation, LiquidationStatus,
        StakingTier, LiquidationConfig, LiquidationMetrics, LiquidationEngine,
        LiquidationOrder, get_staking_tier
    };
    
    use crate::errors::ErrorCode;

    #[test]
    fn test_attack_detector_price_manipulation() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        // Add previous trade for comparison
        detector.recent_trades.push_back(TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 999,
            is_buy: true,
        });
        
        // Create trade with >2% price change
        let trade = TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(103.0), // 3% increase
            leverage: 10,
            slot: 1000,
            is_buy: true,
        };
        
        let alerts = detector.process_trade(&trade, 100_000, &clock).unwrap();
        
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::PriceManipulation);
        assert_eq!(alerts[0].severity, AttackSeverity::High);
        assert_eq!(alerts[0].action, SecurityAction::ClampPrice);
        
        println!("✅ Price manipulation detection test passed");
    }

    #[test]
    fn test_circuit_breaker_coverage_halt() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        // Test low coverage trigger
        let action = breaker.check_breakers(
            U64F64::from_num(0.4), // Below 0.5 threshold
            &[],
            0,
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        match action {
            BreakerAction::Halt { reason, duration, severity } => {
                assert_eq!(reason, HaltReason::LowCoverage);
                assert_eq!(duration, 8640); // 1 hour
                assert_eq!(severity, AttackSeverity::Critical);
            },
            _ => panic!("Expected halt action for low coverage"),
        }
        
        println!("✅ Circuit breaker coverage test passed");
    }

    #[test]
    fn test_liquidation_priority_ordering() {
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
                distance_to_liquidation: U64F64::from_num(-0.01),
                risk_score: 100,
                staking_tier: StakingTier::None,
                bootstrap_priority: 0,
                time_at_risk: 100,
                is_chained: false,
                chain_depth: 0,
            },
        ];
        
        // Sort by priority
        positions.sort_by(|a, b| b.cmp(a));
        
        // Unprotected position should be first despite lower position in vec
        assert_eq!(positions[0].position_id, [2; 32]);
        assert_eq!(positions[1].position_id, [1; 32]);
        
        println!("✅ Liquidation priority ordering test passed");
    }

    #[test]
    fn test_staking_tier_determination() {
        assert_eq!(get_staking_tier(0), StakingTier::None);
        assert_eq!(get_staking_tier(100_000_000), StakingTier::Bronze);
        assert_eq!(get_staking_tier(1_000_000_000), StakingTier::Silver);
        assert_eq!(get_staking_tier(10_000_000_000), StakingTier::Gold);
        assert_eq!(get_staking_tier(100_000_000_000), StakingTier::Platinum);
        
        println!("✅ Staking tier determination test passed");
    }

    #[test]
    fn test_wash_trade_detection() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        let trader = Pubkey::new_unique();
        
        // First trade - buy
        let buy_trade = TradeSnapshot {
            trader,
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1000,
            is_buy: true,
        };
        
        detector.process_trade(&buy_trade, 100_000, &clock).unwrap();
        
        // Immediate opposite trade - sell (wash trade)
        let sell_trade = TradeSnapshot {
            trader,
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1001, // Very close
            is_buy: false,
        };
        
        let alerts = detector.process_trade(&sell_trade, 100_000, &clock).unwrap();
        
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::WashTrading);
        assert_eq!(alerts[0].action, SecurityAction::PenalizeFees);
        
        println!("✅ Wash trade detection test passed");
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
                liquidation_penalty_bps: 50, // 5bp to keeper
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
        
        println!("✅ Partial liquidation limit test passed");
    }

    #[test]
    fn test_emergency_shutdown() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        let authority = Pubkey::default();
        breaker.emergency_authority = Some(authority);
        
        // Test emergency shutdown
        let result = breaker.emergency_shutdown(&authority);
        assert!(result.is_ok());
        
        assert_eq!(breaker.state, BreakerState::EmergencyShutdown);
        assert!(breaker.emergency_authority.is_none()); // Authority burned
        
        println!("✅ Emergency shutdown test passed");
    }

    // Run all tests
    pub fn run_all_phase_11_tests() {
        println!("\n🚀 Running Phase 11 Implementation Tests\n");
        
        test_attack_detector_price_manipulation();
        test_circuit_breaker_coverage_halt();
        test_liquidation_priority_ordering();
        test_staking_tier_determination();
        test_wash_trade_detection();
        test_partial_liquidation_limit();
        test_emergency_shutdown();
        
        println!("\n✅ All Phase 11 tests passed successfully!");
        println!("\n📊 Test Summary:");
        println!("  - Attack Detection: ✅ Price manipulation, wash trading");
        println!("  - Circuit Breakers: ✅ Coverage, emergency shutdown");
        println!("  - Liquidation Priority: ✅ Ordering, partial limits, staking tiers");
    }
}

#[test]
fn run_phase_11_tests() {
    phase_11_tests::run_all_phase_11_tests();
}