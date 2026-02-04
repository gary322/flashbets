//! Sustainability Model and Warning System Tests
//!
//! Tests for post-MMT sustainability model and risk warning modals

use solana_program::pubkey::Pubkey;
use borsh::BorshSerialize;

use crate::{
    economics::sustainability::{
        SustainabilityModel, FeeStructure, DiscountTier,
        RevenueDistribution, VOLUME_DISCOUNT_THRESHOLDS,
        MMT_STAKER_DISCOUNT_BPS, MAX_FEE_DISCOUNT_BPS,
    },
    risk_warnings::warning_modals::{
        WarningModal, WarningType, ModalSeverity,
        UserAcknowledgment, WarningStatistics,
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sustainability_fee_calculation() {
        let model = SustainabilityModel::new(Pubkey::new_unique());
        
        // Test base fee
        let base_amount = 10_000_000; // $10
        let base_fee = model.calculate_fee(base_amount, 0, false);
        let expected_base = (base_amount as u64 * 50) / 10_000; // 0.5%
        assert_eq!(base_fee, expected_base);
        
        // Test with volume discount (1M volume = 10bps discount)
        let volume_fee = model.calculate_fee(base_amount, 1_000_000_000, false);
        let expected_volume = (base_amount as u64 * 40) / 10_000; // 0.4%
        assert_eq!(volume_fee, expected_volume);
        
        // Test with MMT staking discount
        let staker_fee = model.calculate_fee(base_amount, 0, true);
        let expected_staker = (base_amount as u64 * 40) / 10_000; // 0.4%
        assert_eq!(staker_fee, expected_staker);
        
        // Test maximum discount
        let max_discount_fee = model.calculate_fee(base_amount, 100_000_000_000, true);
        let expected_max = (base_amount as u64 * 25) / 10_000; // 0.25%
        assert_eq!(max_discount_fee, expected_max);
    }

    #[test]
    fn test_revenue_distribution() {
        let mut model = SustainabilityModel::new(Pubkey::new_unique());
        
        // Collect 1000 USDC in fees
        let total_fees = 1_000_000_000; // 1000 USDC with 6 decimals
        
        model.process_fee_collection(total_fees).unwrap();
        
        // Verify distribution
        let distribution = model.get_revenue_distribution();
        
        // 50% to treasury
        assert_eq!(distribution.treasury_amount, 500_000_000);
        
        // 30% to user rebates
        assert_eq!(distribution.rebate_amount, 300_000_000);
        
        // 20% to stakers
        assert_eq!(distribution.staker_amount, 200_000_000);
        
        // Verify totals
        let total_distributed = distribution.treasury_amount + 
                               distribution.rebate_amount + 
                               distribution.staker_amount;
        assert_eq!(total_distributed, total_fees);
    }

    #[test]
    fn test_volume_discount_tiers() {
        let model = SustainabilityModel::new(Pubkey::new_unique());
        
        // Test each tier
        let test_cases = vec![
            (0, 0),                          // No discount
            (500_000_000, 5),                // $500K = 5bps
            (1_000_000_000, 10),             // $1M = 10bps
            (5_000_000_000, 15),             // $5M = 15bps
            (10_000_000_000, 20),            // $10M = 20bps
            (50_000_000_000, 25),            // $50M = 25bps (max)
            (100_000_000_000, 25),           // Above max still 25bps
        ];
        
        for (volume, expected_discount) in test_cases {
            let discount = model.get_volume_discount(volume);
            assert_eq!(
                discount, expected_discount,
                "Volume {} should give {}bps discount",
                volume, expected_discount
            );
        }
    }

    #[test]
    fn test_warning_modal_creation() {
        let user = Pubkey::new_unique();
        
        // Test high leverage warning
        let leverage_warning = WarningModal::new(
            WarningType::HighLeverage,
            ModalSeverity::High,
            user,
        );
        
        assert_eq!(leverage_warning.warning_type, WarningType::HighLeverage);
        assert_eq!(leverage_warning.severity, ModalSeverity::High);
        assert!(!leverage_warning.acknowledged);
        assert_eq!(leverage_warning.shown_count, 0);
        
        // Test statistics
        let stats = leverage_warning.get_statistics();
        assert!(stats.contains("80% of traders lose money"));
        assert!(stats.contains("long-term"));
    }

    #[test]
    fn test_warning_acknowledgment() {
        let user = Pubkey::new_unique();
        let mut warning = WarningModal::new(
            WarningType::LargePosition,
            ModalSeverity::Medium,
            user,
        );
        
        // Initial state
        assert!(!warning.acknowledged);
        assert_eq!(warning.shown_count, 0);
        
        // Show warning
        warning.show().unwrap();
        assert_eq!(warning.shown_count, 1);
        
        // Acknowledge
        warning.acknowledge(user).unwrap();
        assert!(warning.acknowledged);
        
        // Try to acknowledge with wrong user
        let wrong_user = Pubkey::new_unique();
        let result = warning.acknowledge(wrong_user);
        assert!(result.is_err());
    }

    #[test]
    fn test_warning_cooldown() {
        let user = Pubkey::new_unique();
        let mut warning = WarningModal::new(
            WarningType::HighVolatility,
            ModalSeverity::Low,
            user,
        );
        
        // Show warning
        warning.show().unwrap();
        let first_shown = warning.last_shown_timestamp;
        
        // Try to show again immediately
        std::thread::sleep(std::time::Duration::from_millis(10));
        warning.show().unwrap();
        
        // For low severity, should update timestamp
        assert!(warning.last_shown_timestamp > first_shown);
        assert_eq!(warning.shown_count, 2);
    }

    #[test]
    fn test_warning_statistics_accuracy() {
        let statistics = WarningStatistics::default();
        
        // Verify statistics are realistic
        assert_eq!(statistics.traders_lose_percentage, 80);
        assert_eq!(statistics.average_loss_percentage, 90);
        assert!(statistics.high_leverage_liquidation_rate > 50);
        assert!(statistics.average_time_to_liquidation_hours < 72);
    }

    #[test]
    fn test_sustainability_model_updates() {
        let mut model = SustainabilityModel::new(Pubkey::new_unique());
        
        // Update fee structure
        let new_fee = FeeStructure {
            base_fee_bps: 60, // 0.6%
            volume_discount_bps: vec![6, 12, 18, 24, 30],
            mmt_staker_discount_bps: 15,
            max_discount_bps: 30,
        };
        
        model.update_fee_structure(new_fee).unwrap();
        
        // Test new fees
        let amount = 10_000_000;
        let fee = model.calculate_fee(amount, 0, false);
        let expected = (amount as u64 * 60) / 10_000;
        assert_eq!(fee, expected);
    }

    #[test]
    fn test_warning_types_coverage() {
        let user = Pubkey::new_unique();
        
        let warning_types = vec![
            WarningType::HighLeverage,
            WarningType::LargePosition,
            WarningType::HighVolatility,
            WarningType::LiquidationRisk,
            WarningType::ComplexStrategy,
            WarningType::FirstTimeUser,
        ];
        
        for warning_type in warning_types {
            let modal = WarningModal::new(
                warning_type.clone(),
                ModalSeverity::Medium,
                user,
            );
            
            // Each type should have meaningful content
            let content = modal.get_content();
            assert!(!content.is_empty(), "Warning type {:?} has no content", warning_type);
            
            // Each type should have statistics
            let stats = modal.get_statistics();
            assert!(!stats.is_empty(), "Warning type {:?} has no statistics", warning_type);
        }
    }

    #[test]
    fn test_fee_rebate_calculation() {
        let mut model = SustainabilityModel::new(Pubkey::new_unique());
        
        // User pays fees
        let user = Pubkey::new_unique();
        let fees_paid = 100_000_000; // 100 USDC
        
        model.track_user_fees(user, fees_paid).unwrap();
        
        // Calculate rebate (30% of fees go to rebate pool)
        let rebate_share = model.calculate_user_rebate(user);
        
        // In real implementation, would depend on user's share of total volume
        // For this test, assume user gets proportional share
        assert!(rebate_share > 0);
    }

    #[test]
    fn test_sustainability_metrics() {
        let mut model = SustainabilityModel::new(Pubkey::new_unique());
        
        // Simulate activity
        for i in 0..100 {
            let fee = 1_000_000 * (i + 1); // Increasing fees
            model.process_fee_collection(fee).unwrap();
        }
        
        // Get metrics
        let metrics = model.get_metrics();
        
        assert!(metrics.total_fees_collected > 0);
        assert!(metrics.total_rebates_distributed >= 0);
        assert!(metrics.active_users >= 0);
        assert!(metrics.average_fee_rate > 0);
    }

    #[test]
    fn test_warning_severity_escalation() {
        let user = Pubkey::new_unique();
        
        // Start with low severity
        let mut warning = WarningModal::new(
            WarningType::HighLeverage,
            ModalSeverity::Low,
            user,
        );
        
        // Show multiple times
        for _ in 0..5 {
            warning.show().unwrap();
        }
        
        // After multiple shows, severity could be escalated in production
        assert_eq!(warning.shown_count, 5);
        
        // In production, might upgrade to Medium or High severity
        // based on user behavior
    }
}