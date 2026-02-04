#[cfg(test)]
mod deployment_tests {
    use betting_platform::deployment::*;
    use anchor_lang::prelude::*;

    #[test]
    fn test_deployment_manager_creation() {
        let deployer = DeploymentManager::new();
        
        assert_eq!(deployer.program_id, Pubkey::default());
        assert_eq!(deployer.upgrade_authority, None);
        assert_eq!(deployer.deployment_slot, 0);
    }

    #[test]
    fn test_deployment_flow() {
        let mut deployer = DeploymentManager::new();
        let program_id = Pubkey::new_unique();
        
        let result = deployer.deploy_immutable_program(program_id);
        
        assert!(result.is_ok());
        assert_eq!(deployer.program_id, program_id);
    }

    #[test]
    fn test_genesis_config_default() {
        let genesis = GenesisConfig::default();
        
        assert_eq!(genesis.fee_base, 3);
        assert_eq!(genesis.fee_slope, 25);
        assert_eq!(genesis.initial_coverage, 0.0);
        assert_eq!(genesis.mmt_supply, 100_000_000 * 10u128.pow(9));
        assert_eq!(genesis.emission_per_slot, 100);
        assert_eq!(genesis.season_duration, 38_880_000);
    }

    #[test]
    fn test_genesis_config_validation() {
        let mut genesis = GenesisConfig::default();
        
        // Test valid configuration
        let result = genesis.validate_config();
        assert!(result.is_ok());
        
        // Test invalid configurations
        genesis.fee_base = 0;
        let result = genesis.validate_config();
        assert!(result.is_err());
        
        genesis.fee_base = 3;
        genesis.mmt_supply = 50_000_000 * 10u128.pow(9); // Wrong supply
        let result = genesis.validate_config();
        assert!(result.is_err());
    }

    #[test]
    fn test_bootstrap_incentives_activation() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        // Test initial state
        assert!(!config.bootstrap_mode);
        
        // Test activation
        let result = incentives.activate_launch_incentives(&mut config);
        assert!(result.is_ok());
        
        assert!(config.bootstrap_mode);
        assert_eq!(config.bootstrap_trade_count, 0);
        assert_eq!(config.bootstrap_max_trades, 100);
        assert_eq!(config.maker_bonus_multiplier, 2.0);
        assert!(config.liquidity_mining_active);
        
        // Test double activation should fail
        let result2 = incentives.activate_launch_incentives(&mut config);
        assert!(result2.is_err());
    }

    #[test]
    fn test_double_mmt_rewards() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        incentives.activate_launch_incentives(&mut config).unwrap();
        
        // Test first 100 trades get double rewards
        let base_reward = 1000u64;
        
        for i in 0..100 {
            config.bootstrap_trade_count = i;
            let reward = incentives.calculate_mmt_reward(base_reward, &config);
            assert_eq!(reward, 2000, "Trade {} should get double reward", i);
        }
        
        // Trade 101 should get normal reward
        config.bootstrap_trade_count = 100;
        let reward = incentives.calculate_mmt_reward(base_reward, &config);
        assert_eq!(reward, 1000, "Trade 101 should get normal reward");
    }

    #[test]
    fn test_bootstrap_counter_increment() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        incentives.activate_launch_incentives(&mut config).unwrap();
        
        // Test counter increment
        for i in 0..100 {
            assert_eq!(config.bootstrap_trade_count, i);
            incentives.increment_bootstrap_counter(&mut config).unwrap();
        }
        
        assert_eq!(config.bootstrap_trade_count, 100);
    }

    #[test]
    fn test_launch_monitor_creation() {
        let program_id = Pubkey::new_unique();
        let vault_pubkey = Pubkey::new_unique();
        
        let monitor = LaunchMonitor::new(
            program_id,
            vault_pubkey,
            Some("https://webhook.example.com".to_string()),
        );
        
        // Test initial metrics
        let vault_balance = monitor.check_vault_balance().unwrap();
        assert_eq!(vault_balance, 0, "Vault should start at $0");
        
        let coverage = monitor.calculate_coverage().unwrap();
        assert_eq!(coverage, 0.0, "Coverage should start at 0");
    }

    #[test]
    fn test_alert_system() {
        let alert_system = AlertSystem::new(None);
        
        // Test different alert levels
        let result = alert_system.send_alert(
            AlertLevel::Info,
            "Test info alert",
        );
        assert!(result.is_ok());
        
        let result = alert_system.send_alert(
            AlertLevel::Critical,
            "Test critical alert",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new(100);
        
        let metrics = MetricsData {
            vault_balance: 0,
            coverage_ratio: 0.0,
            tps: 100.0,
            keeper_status: KeeperStatus::Healthy,
            timestamp: 1000,
        };
        
        collector.record_metrics(metrics.clone());
        
        let latest = collector.get_latest_metrics();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().tps, 100.0);
    }

    #[test]
    fn test_bootstrap_stats() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        incentives.activate_launch_incentives(&mut config).unwrap();
        config.bootstrap_trade_count = 25;
        
        let stats = incentives.get_bootstrap_stats(&config);
        
        assert!(stats.is_active);
        assert_eq!(stats.trades_completed, 25);
        assert_eq!(stats.trades_remaining, 75);
        assert_eq!(stats.current_maker_bonus, 2.0);
        assert!(stats.double_mmt_active);
    }

    #[test]
    fn test_liquidity_mining_calculation() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        incentives.activate_launch_incentives(&mut config).unwrap();
        
        let liquidity = 1_000_000u64;
        let duration = 432_000u64; // 1 day in slots
        
        let reward = incentives.calculate_liquidity_mining_reward(
            liquidity,
            duration,
            &config,
        );
        
        // Should get boosted rewards during bootstrap
        assert!(reward > 0);
    }

    #[test]
    fn test_health_checker() {
        let mut health_checker = HealthChecker::new();
        
        let check_fn = Arc::new(|| -> Result<bool> { Ok(true) });
        health_checker.add_check("test_check".to_string(), check_fn);
        
        let results = health_checker.run_checks();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "test_check");
        assert!(results[0].1);
    }

    #[cfg(test)]
    mod performance_tests {
        use super::*;
        use std::time::Instant;

        #[test]
        fn test_deployment_performance() {
            let start = Instant::now();
            
            let deployer = DeploymentManager::new();
            
            let duration = start.elapsed();
            assert!(duration.as_millis() < 100, "Deployment manager creation should be fast");
        }

        #[test]
        fn test_incentive_calculation_performance() {
            let incentives = BootstrapIncentives::default();
            let config = GlobalConfig::default();
            
            let start = Instant::now();
            
            // Run 10000 calculations
            for _ in 0..10000 {
                let _ = incentives.calculate_mmt_reward(1000, &config);
            }
            
            let duration = start.elapsed();
            assert!(duration.as_millis() < 10, "Incentive calculations should be very fast");
        }
    }
}

use std::sync::Arc;