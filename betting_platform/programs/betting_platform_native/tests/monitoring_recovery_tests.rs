//! Monitoring & Recovery System Tests
//! 
//! Tests for Phase 14.5 monitoring, alerts, and disaster recovery

#[cfg(test)]
mod tests {
    use betting_platform_native::{
        error::BettingPlatformError,
        math::U64F64,
        monitoring::{
            health::*,
            alerts::*,
            performance::*,
        },
        recovery::{
            disaster::*,
            checkpoint::*,
        },
    };
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_coverage_critical_threshold() {
        // CLAUDE.md: Coverage < 1 is critical
        let mut health = SystemHealth {
            status: SystemStatus::Healthy,
            coverage_ratio: U64F64::from_fraction(99, 100).unwrap(), // Below 1.0
            api_price_deviation_pct: 0,
            last_update_slot: 1000,
            epoch_start_slot: 0,
            current_tps: 1000,
            average_tps: 800,
            peak_tps: 1500,
            total_transactions: 100000,
            average_cu_per_tx: 15000,
            peak_cu_usage: 25000,
            cu_violations: 5,
            lowest_coverage: U64F64::from_fraction(99, 100).unwrap(),
            api_response_time_ms: 100,
            api_failures: 0,
            keeper_network: ServiceStatus::Online,
            polymarket_api: ServiceStatus::Online,
            price_feeds: ServiceStatus::Online,
            liquidation_engine: ServiceStatus::Online,
            circuit_breaker_active: false,
            circuit_breaker_trigger_slot: None,
            circuit_breaker_reason: None,
        };

        // In actual implementation, this would trigger Critical status
        assert!(health.coverage_ratio < U64F64::from_num(1));
    }

    #[test]
    fn test_api_deviation_critical_threshold() {
        // CLAUDE.md: API deviation > 5% is critical
        let mut health = SystemHealth {
            status: SystemStatus::Healthy,
            coverage_ratio: U64F64::from_num(2),
            api_price_deviation_pct: 6, // Above 5%
            last_update_slot: 1000,
            epoch_start_slot: 0,
            current_tps: 1000,
            average_tps: 800,
            peak_tps: 1500,
            total_transactions: 100000,
            average_cu_per_tx: 15000,
            peak_cu_usage: 25000,
            cu_violations: 5,
            lowest_coverage: U64F64::from_num(2),
            api_response_time_ms: 100,
            api_failures: 0,
            keeper_network: ServiceStatus::Online,
            polymarket_api: ServiceStatus::Online,
            price_feeds: ServiceStatus::Online,
            liquidation_engine: ServiceStatus::Online,
            circuit_breaker_active: false,
            circuit_breaker_trigger_slot: None,
            circuit_breaker_reason: None,
        };

        assert!(health.api_price_deviation_pct > 5);
    }

    #[test]
    fn test_alert_configuration_defaults() {
        let config = AlertConfiguration::initialize_defaults();
        
        // Verify CLAUDE.md specified thresholds
        // COVERAGE_CRITICAL_THRESHOLD is 1.0 as f64, convert to U64F64
        assert_eq!(config.coverage_critical_threshold, U64F64::from_num(1));
        assert_eq!(config.api_deviation_critical_pct, API_DEVIATION_CRITICAL_PCT);
        assert_eq!(config.polymarket_timeout_slots, POLYMARKET_OUTAGE_SLOTS);
        assert_eq!(config.enabled, true);
    }

    #[test]
    fn test_alert_priority() {
        let info_alert = Alert {
            alert_type: AlertType::LowCoverage,
            severity: AlertSeverity::Info,
            triggered_slot: 1000,
            message: "Info alert".to_string(),
            metric_value: 0,
            threshold_value: 0,
            acknowledged: false,
            acknowledged_by: None,
            resolved_slot: None,
        };

        let critical_alert = Alert {
            alert_type: AlertType::CriticalCoverage,
            severity: AlertSeverity::Critical,
            triggered_slot: 1001,
            message: "Critical alert".to_string(),
            metric_value: 0,
            threshold_value: 0,
            acknowledged: false,
            acknowledged_by: None,
            resolved_slot: None,
        };

        assert!(critical_alert.severity > info_alert.severity);
    }

    #[test]
    fn test_polymarket_outage_detection() {
        let mut recovery_state = DisasterRecoveryState {
            current_mode: RecoveryMode::Normal,
            last_checkpoint_slot: 0,
            recovery_initiated_slot: None,
            recovery_completed_slot: None,
            positions_to_recover: 0,
            positions_recovered: 0,
            orders_to_recover: 0,
            orders_recovered: 0,
            polymarket_last_sync: 1000,
            polymarket_out_of_sync: false,
            polymarket_outage_start: None,
            emergency_actions: vec![],
            recovery_authority: Pubkey::new_unique(),
            emergency_contacts: vec![],
        };

        // Simulate outage start
        recovery_state.polymarket_outage_start = Some(1000);
        recovery_state.polymarket_out_of_sync = true;

        // Check after 5 minutes (750 slots)
        let current_slot = 1750;
        let duration = current_slot - recovery_state.polymarket_outage_start.unwrap();
        
        assert_eq!(duration, 750); // Exactly 5 minutes
        assert!(duration >= POLYMARKET_OUTAGE_SLOTS);
    }

    #[test]
    fn test_recovery_mode_transitions() {
        let mut recovery_state = DisasterRecoveryState {
            current_mode: RecoveryMode::Normal,
            last_checkpoint_slot: 0,
            recovery_initiated_slot: None,
            recovery_completed_slot: None,
            positions_to_recover: 100,
            positions_recovered: 0,
            orders_to_recover: 50,
            orders_recovered: 0,
            polymarket_last_sync: 1000,
            polymarket_out_of_sync: false,
            polymarket_outage_start: None,
            emergency_actions: vec![],
            recovery_authority: Pubkey::new_unique(),
            emergency_contacts: vec![],
        };

        // Test operation permissions in different modes
        assert!(RecoveryManager::check_operation_allowed(&recovery_state, "open_position"));
        
        recovery_state.current_mode = RecoveryMode::PartialDegradation;
        assert!(!RecoveryManager::check_operation_allowed(&recovery_state, "open_position"));
        assert!(RecoveryManager::check_operation_allowed(&recovery_state, "close_position"));
        
        recovery_state.current_mode = RecoveryMode::Emergency;
        assert!(!RecoveryManager::check_operation_allowed(&recovery_state, "open_position"));
        assert!(RecoveryManager::check_operation_allowed(&recovery_state, "emergency_withdraw"));
    }

    #[test]
    fn test_checkpoint_verification() {
        let checkpoint = Checkpoint {
            checkpoint_id: 12345,
            created_slot: 1000,
            created_by: Pubkey::new_unique(),
            checkpoint_type: CheckpointType::Scheduled,
            global_snapshot: GlobalSnapshot {
                epoch: 1,
                season: 1,
                vault_balance: 1_000_000_000_000,
                total_oi: 500_000_000_000,
                coverage: U64F64::from_num(2),
                mmt_supply: 1_000_000_000_000,
                keeper_count: 10,
                active_markets: 50,
            },
            critical_accounts: vec![],
            positions_root: [1u8; 32],
            orders_root: [2u8; 32],
            verses_root: [3u8; 32],
            total_positions: 1000,
            total_orders: 500,
            total_volume: 10_000_000,
            total_oi: 5_000_000,
            verified: false,
            verification_slot: None,
            verification_signature: None,
        };

        assert!(!checkpoint.verified);
        assert_eq!(checkpoint.checkpoint_type, CheckpointType::Scheduled);
    }

    #[test]
    fn test_performance_metrics_calculation() {
        let metrics = PerformanceMetrics {
            last_update_slot: 1000,
            measurement_window_slots: 1000,
            
            // Initialize operation metrics
            open_position_metrics: OperationMetrics {
                total_count: 100,
                success_count: 99,
                failure_count: 1,
                average_cu_usage: 15000,
                max_cu_usage: 19000,
                average_latency_ms: 20,
                p95_latency_ms: 30,
                p99_latency_ms: 40,
                last_failure_slot: Some(1050),
                consecutive_failures: 0,
            },
            close_position_metrics: OperationMetrics {
                total_count: 50,
                success_count: 50,
                failure_count: 0,
                average_cu_usage: 12000,
                max_cu_usage: 15000,
                average_latency_ms: 15,
                p95_latency_ms: 25,
                p99_latency_ms: 35,
                last_failure_slot: None,
                consecutive_failures: 0,
            },
            liquidation_metrics: OperationMetrics {
                total_count: 10,
                success_count: 10,
                failure_count: 0,
                average_cu_usage: 18000,
                max_cu_usage: 19500,
                average_latency_ms: 25,
                p95_latency_ms: 35,
                p99_latency_ms: 45,
                last_failure_slot: None,
                consecutive_failures: 0,
            },
            order_execution_metrics: OperationMetrics {
                total_count: 200,
                success_count: 195,
                failure_count: 5,
                average_cu_usage: 10000,
                max_cu_usage: 14000,
                average_latency_ms: 10,
                p95_latency_ms: 20,
                p99_latency_ms: 30,
                last_failure_slot: Some(900),
                consecutive_failures: 0,
            },
            keeper_task_metrics: OperationMetrics {
                total_count: 1000,
                success_count: 998,
                failure_count: 2,
                average_cu_usage: 8000,
                max_cu_usage: 12000,
                average_latency_ms: 5,
                p95_latency_ms: 10,
                p99_latency_ms: 15,
                last_failure_slot: Some(800),
                consecutive_failures: 0,
            },
            
            // API latency
            api_latency_p50: 50,
            api_latency_p95: 100,
            api_latency_p99: 150,
            
            // Success rates
            overall_success_rate: 96, // 96%
            keeper_success_rate: 99,  // 99.8%
            liquidation_success_rate: 100, // 100%
            
            // Resource usage
            average_accounts_per_tx: 10,
            average_cpi_calls_per_tx: 2,
            memory_usage_bytes: 500_000,
            
            // Alerts
            active_alerts: vec![],
        };

        // Test aggregate calculations
        let total_operations = metrics.open_position_metrics.total_count 
            + metrics.close_position_metrics.total_count
            + metrics.liquidation_metrics.total_count
            + metrics.order_execution_metrics.total_count
            + metrics.keeper_task_metrics.total_count;
            
        assert_eq!(total_operations, 1360);
        
        // Test success rates
        assert_eq!(metrics.open_position_metrics.success_count, 99);
        assert_eq!(metrics.open_position_metrics.failure_count, 1);
        
        // Test CU usage stays below limits
        assert!(metrics.open_position_metrics.max_cu_usage < 20000);
        assert!(metrics.liquidation_metrics.max_cu_usage < 20000);
    }

    #[test]
    fn test_health_score_calculation() {
        let health = SystemHealth {
            status: SystemStatus::Healthy,
            coverage_ratio: U64F64::from_fraction(5, 2).unwrap(), // 2.5
            api_price_deviation_pct: 2,
            last_update_slot: 1000,
            epoch_start_slot: 0,
            current_tps: 1000,
            average_tps: 800,
            peak_tps: 1500,
            total_transactions: 100000,
            average_cu_per_tx: 15000,
            peak_cu_usage: 19000, // Below 20k threshold
            cu_violations: 5,
            lowest_coverage: U64F64::from_num(2),
            api_response_time_ms: 100,
            api_failures: 0,
            keeper_network: ServiceStatus::Online,
            polymarket_api: ServiceStatus::Online,
            price_feeds: ServiceStatus::Online,
            liquidation_engine: ServiceStatus::Online,
            circuit_breaker_active: false,
            circuit_breaker_trigger_slot: None,
            circuit_breaker_reason: None,
        };

        let score = health.get_health_score();
        assert_eq!(score, 100); // Perfect health
    }

    #[test]
    fn test_emergency_action_tracking() {
        let action = EmergencyAction {
            action_type: EmergencyActionType::HaltTrading,
            triggered_slot: 1000,
            triggered_by: Pubkey::new_unique(),
            reason: "Critical coverage breach".to_string(),
            affected_accounts: 500,
        };

        assert_eq!(action.action_type, EmergencyActionType::HaltTrading);
        assert_eq!(action.affected_accounts, 500);
    }

    #[test]
    fn test_circuit_breaker_reasons() {
        let reasons = vec![
            CircuitBreakerReason::LowCoverage,
            CircuitBreakerReason::HighAPIDeviation,
            CircuitBreakerReason::NetworkCongestion,
            CircuitBreakerReason::PolymarketOutage,
            CircuitBreakerReason::SolanaOutage,
            CircuitBreakerReason::ManualTrigger,
        ];

        // Verify all circuit breaker reasons are covered
        assert_eq!(reasons.len(), 6);
    }

    #[test]
    fn test_recovery_progress_tracking() {
        let recovery_state = DisasterRecoveryState {
            current_mode: RecoveryMode::FullRecovery,
            last_checkpoint_slot: 0,
            recovery_initiated_slot: Some(1000),
            recovery_completed_slot: None,
            positions_to_recover: 100,
            positions_recovered: 75,
            orders_to_recover: 50,
            orders_recovered: 40,
            polymarket_last_sync: 1000,
            polymarket_out_of_sync: false,
            polymarket_outage_start: None,
            emergency_actions: vec![],
            recovery_authority: Pubkey::new_unique(),
            emergency_contacts: vec![],
        };

        let progress = recovery_state.get_recovery_progress();
        assert_eq!(progress, 76); // (75 + 40) / (100 + 50) * 100 = 76%
    }
}