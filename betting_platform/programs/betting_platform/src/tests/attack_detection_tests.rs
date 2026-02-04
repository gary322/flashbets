#[cfg(test)]
mod tests {
    use super::*;
    use crate::attack_detection::*;
    use crate::fixed_types::U64F64;
    use anchor_lang::prelude::*;
    use std::collections::VecDeque;

    #[test]
    fn test_attack_detector_initialization() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        let result = detector.init(&clock);
        assert!(result.is_ok());
        
        assert_eq!(detector.risk_level, 0);
        assert!(detector.detected_patterns.is_empty());
        assert!(detector.recent_trades.is_empty());
        assert_eq!(detector.price_tracker.max_change_per_slot, U64F64::from_num(0.02)); // 2%
        assert_eq!(detector.volume_detector.anomaly_threshold, U64F64::from_num(3.0)); // 3 std devs
        assert_eq!(detector.wash_trade_detector.min_time_between, 10); // 10 slots
        assert_eq!(detector.last_update_slot, 1000);
    }

    #[test]
    fn test_price_manipulation_detection_single_slot() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        // Add baseline trade
        detector.recent_trades.push_back(TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 999,
            is_buy: true,
        });
        
        // Trade with >2% price increase (should trigger alert)
        let malicious_trade = TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(103.0), // 3% increase
            leverage: 10,
            slot: 1000,
            is_buy: true,
        };
        
        let alerts = detector.process_trade(&malicious_trade, 100_000, &clock).unwrap();
        
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::PriceManipulation);
        assert_eq!(alerts[0].severity, AttackSeverity::High);
        assert_eq!(alerts[0].action, SecurityAction::ClampPrice);
        assert!(alerts[0].message.contains("exceeds 2% limit"));
    }

    #[test]
    fn test_cumulative_price_manipulation_detection() {
        let mut detector = AttackDetector::default();
        let mut clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        // Add baseline trade
        detector.recent_trades.push_back(TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1000,
            is_buy: true,
        });
        
        // Add trades that individually are under 2% but cumulatively exceed 5% over 4 slots
        let price_changes = vec![101.5, 103.0, 104.5, 106.0]; // Total 6% change
        
        for (i, price) in price_changes.iter().enumerate() {
            clock.slot = 1001 + i as u64;
            
            let trade = TradeSnapshot {
                trader: Pubkey::new_unique(),
                market_id: [1; 32],
                size: 1000,
                price: U64F64::from_num(*price),
                leverage: 10,
                slot: clock.slot,
                is_buy: true,
            };
            
            let alerts = detector.process_trade(&trade, 100_000, &clock).unwrap();
            
            // The last trade should trigger cumulative alert
            if i == 3 {
                assert!(alerts.iter().any(|a| 
                    a.severity == AttackSeverity::Critical && 
                    a.message.contains("5% limit")
                ));
            }
        }
    }

    #[test]
    fn test_volume_anomaly_detection() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        // Set baseline volume
        detector.volume_detector.avg_volume_7d = 10_000;
        detector.volume_detector.volume_std_dev = U64F64::from_num(1000.0);
        detector.volume_detector.current_volume = 0;
        
        // Trade with volume that exceeds 3 standard deviations
        let anomalous_trade = TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 35_000, // Way above average
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1000,
            is_buy: true,
        };
        
        let alerts = detector.process_trade(&anomalous_trade, 100_000, &clock).unwrap();
        
        assert!(alerts.iter().any(|a| a.alert_type == AlertType::VolumeAnomaly));
    }

    #[test]
    fn test_flash_loan_detection() {
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
        let vault_size = 100_000;
        
        // Large buy trade (>10% of vault)
        let buy_trade = TradeSnapshot {
            trader,
            market_id: [1; 32],
            size: 15_000, // 15% of vault
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1000,
            is_buy: true,
        };
        
        detector.process_trade(&buy_trade, vault_size, &clock).unwrap();
        
        // Opposite sell trade in same slot (flash loan pattern)
        let sell_trade = TradeSnapshot {
            trader,
            market_id: [1; 32],
            size: 15_000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1000, // Same slot!
            is_buy: false,
        };
        
        let alerts = detector.process_trade(&sell_trade, vault_size, &clock).unwrap();
        
        assert!(alerts.iter().any(|a| {
            a.alert_type == AlertType::FlashLoan &&
            a.severity == AttackSeverity::Critical &&
            a.action == SecurityAction::RevertTrades
        }));
    }

    #[test]
    fn test_wash_trading_detection() {
        let mut detector = AttackDetector::default();
        let mut clock = Clock {
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
        
        // Opposite trade too soon (within 10 slots)
        clock.slot = 1005; // Only 5 slots later
        
        let sell_trade = TradeSnapshot {
            trader,
            market_id: [1; 32],
            size: 1000,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: clock.slot,
            is_buy: false,
        };
        
        let alerts = detector.process_trade(&sell_trade, 100_000, &clock).unwrap();
        
        assert!(alerts.iter().any(|a| {
            a.alert_type == AlertType::WashTrading &&
            a.severity == AttackSeverity::High &&
            a.action == SecurityAction::PenalizeFees
        }));
    }

    #[test]
    fn test_risk_level_calculation() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        // Simulate multiple different severity alerts
        let alerts = vec![
            SecurityAlert {
                alert_type: AlertType::PriceManipulation,
                severity: AttackSeverity::High, // 50 points
                message: "Test".to_string(),
                action: SecurityAction::ClampPrice,
                data: AlertData::CumulativeChange(U64F64::from_num(0.05)),
            },
            SecurityAlert {
                alert_type: AlertType::VolumeAnomaly,
                severity: AttackSeverity::Medium, // 25 points
                message: "Test".to_string(),
                action: SecurityAction::Monitor,
                data: AlertData::VolumeData { current: 1000, average: 500 },
            },
            SecurityAlert {
                alert_type: AlertType::WashTrading,
                severity: AttackSeverity::Low, // 10 points
                message: "Test".to_string(),
                action: SecurityAction::PenalizeFees,
                data: AlertData::VolumeData { current: 1000, average: 500 },
            },
        ];
        
        detector.update_risk_level(&alerts);
        
        // 50 + 25 + 10 = 85
        assert_eq!(detector.risk_level, 85);
    }

    #[test]
    fn test_normal_trading_no_alerts() {
        let mut detector = AttackDetector::default();
        let clock = Clock {
            slot: 1000,
            epoch_start_timestamp: 0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: 1000,
        };
        
        detector.init(&clock).unwrap();
        
        // Normal trade with reasonable parameters
        let normal_trade = TradeSnapshot {
            trader: Pubkey::new_unique(),
            market_id: [1; 32],
            size: 100,
            price: U64F64::from_num(100.0),
            leverage: 10,
            slot: 1000,
            is_buy: true,
        };
        
        let alerts = detector.process_trade(&normal_trade, 100_000, &clock).unwrap();
        
        assert!(alerts.is_empty());
        assert_eq!(detector.risk_level, 0);
    }
}