#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit_breaker::*;
    use crate::attack_detection::*;
    use crate::fixed_types::U64F64;
    use anchor_lang::prelude::*;

    #[test]
    fn test_circuit_breaker_initialization() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        let result = breaker.init(&clock);
        assert!(result.is_ok());
        
        assert_eq!(breaker.state, BreakerState::Active);
        assert_eq!(breaker.coverage_breaker.min_coverage, U64F64::from_num(0.5));
        assert_eq!(breaker.price_breaker.max_cumulative_change, U64F64::from_num(0.05));
        assert_eq!(breaker.price_breaker.window_size, 4);
        assert_eq!(breaker.volume_breaker.max_volume_multiplier, U64F64::from_num(10));
        assert_eq!(breaker.liquidation_breaker.max_liquidations_per_slot, 50);
        assert_eq!(breaker.liquidation_breaker.max_liquidation_volume_percent, U64F64::from_num(0.1));
        assert_eq!(breaker.congestion_breaker.max_slot_deviation_ms, 1500);
        assert_eq!(breaker.congestion_breaker.max_failed_tx_per_slot, 100);
        assert_eq!(breaker.cooldown_period, 720); // 5 minutes
        assert!(breaker.emergency_authority.is_some());
    }

    #[test]
    fn test_coverage_breaker_trigger() {
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
        let low_coverage = U64F64::from_num(0.4); // Below 0.5 threshold
        
        let action = breaker.check_breakers(
            low_coverage,
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
        
        // Verify state changed
        match breaker.state {
            BreakerState::Halted { start_slot, expected_resume, reason } => {
                assert_eq!(start_slot, 1000);
                assert_eq!(expected_resume, 1000 + 8640);
                assert_eq!(reason, HaltReason::LowCoverage);
            },
            _ => panic!("Expected halted state"),
        }
    }

    #[test]
    fn test_price_breaker_cumulative_trigger() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        // Create trades with >5% movement over 4 slots
        let trades = vec![
            TradeSnapshot {
                trader: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 1000,
                price: U64F64::from_num(100.0),
                leverage: 10,
                slot: 996,
                is_buy: true,
            },
            TradeSnapshot {
                trader: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 1000,
                price: U64F64::from_num(106.0), // 6% increase
                leverage: 10,
                slot: 1000,
                is_buy: true,
            },
        ];
        
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &trades,
            0,
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        match action {
            BreakerAction::Halt { reason, .. } => {
                assert_eq!(reason, HaltReason::PriceVolatility);
            },
            _ => panic!("Expected halt for price volatility"),
        }
    }

    #[test]
    fn test_liquidation_cascade_breaker() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        // Test exceeding liquidation count
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &[],
            60, // > 50 liquidations
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        match action {
            BreakerAction::Halt { reason, duration, severity } => {
                assert_eq!(reason, HaltReason::LiquidationCascade);
                assert_eq!(duration, 8640); // 1 hour
                assert_eq!(severity, AttackSeverity::Critical);
            },
            _ => panic!("Expected halt for liquidation cascade"),
        }
    }

    #[test]
    fn test_liquidation_volume_breaker() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        let total_oi = 100_000;
        let liquidation_volume = 15_000; // 15% of OI, exceeds 10% limit
        
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &[],
            10,
            liquidation_volume,
            total_oi,
            0,
            &clock,
        ).unwrap();
        
        match action {
            BreakerAction::Halt { reason, .. } => {
                assert_eq!(reason, HaltReason::LiquidationCascade);
            },
            _ => panic!("Expected halt for liquidation volume"),
        }
    }

    #[test]
    fn test_network_congestion_breaker() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        // Test excessive failed transactions
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &[],
            0,
            0,
            100_000,
            150, // > 100 failed tx
            &clock,
        ).unwrap();
        
        match action {
            BreakerAction::Halt { reason, duration, severity } => {
                assert_eq!(reason, HaltReason::NetworkCongestion);
                assert_eq!(duration, 2160); // 15 minutes
                assert_eq!(severity, AttackSeverity::High);
            },
            _ => panic!("Expected halt for network congestion"),
        }
    }

    #[test]
    fn test_breaker_cooldown_period() {
        let mut breaker = CircuitBreaker::default();
        let mut clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        // Trigger a halt
        breaker.check_breakers(
            U64F64::from_num(0.4), // Low coverage
            &[],
            0,
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        // Fast forward past halt duration
        clock.slot = 1000 + 8640 + 1;
        
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &[],
            0,
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        assert_eq!(action, BreakerAction::Resume);
        
        // Should now be in cooldown
        match breaker.state {
            BreakerState::Cooldown { end_slot } => {
                assert_eq!(end_slot, clock.slot + breaker.cooldown_period);
            },
            _ => panic!("Expected cooldown state"),
        }
        
        // Check again during cooldown
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &[],
            0,
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        assert_eq!(action, BreakerAction::InCooldown);
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
        
        // Verify system remains shut down
        let action = breaker.check_breakers(
            U64F64::from_num(1.0),
            &[],
            0,
            0,
            100_000,
            0,
            &clock,
        ).unwrap();
        
        assert_eq!(action, BreakerAction::EmergencyShutdown);
    }

    #[test]
    fn test_emergency_shutdown_unauthorized() {
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
        let wrong_authority = Pubkey::new_unique();
        breaker.emergency_authority = Some(authority);
        
        // Test with wrong authority
        let result = breaker.emergency_shutdown(&wrong_authority);
        assert!(result.is_err());
        
        // Authority should not be burned
        assert!(breaker.emergency_authority.is_some());
        assert_ne!(breaker.state, BreakerState::EmergencyShutdown);
    }

    #[test]
    fn test_normal_operation_no_halt() {
        let mut breaker = CircuitBreaker::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        breaker.init(&clock).unwrap();
        
        // All parameters within normal bounds
        let action = breaker.check_breakers(
            U64F64::from_num(0.8), // Good coverage
            &[],
            10, // Low liquidations
            5_000, // 5% of OI
            100_000,
            50, // Some failed tx but under limit
            &clock,
        ).unwrap();
        
        assert_eq!(action, BreakerAction::Continue);
        assert_eq!(breaker.state, BreakerState::Active);
    }
}