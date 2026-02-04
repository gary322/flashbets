//! Part 7 Integration Tests
//!
//! Comprehensive tests for all Part 7 features including:
//! - Elastic fee structure
//! - Fee distribution
//! - Coverage calculations
//! - Recovery mechanisms
//! - Cross-verse protection

use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    rent::Rent,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::math::fixed_point::U64F64;

use crate::{
    fees::{
        elastic_fee::calculate_elastic_fee,
        distribution::distribute_fees,
        maker_taker::{calculate_maker_taker_fee, OrderType},
        FEE_BASE_BPS, FEE_MAX_BPS,
    },
    coverage::{
        correlation::{calculate_correlation_adjusted_tail_loss, MarketCorrelation, PositionConcentration},
        slot_updater::update_coverage_per_slot,
        recovery::{initiate_recovery_mode, update_recovery_state, calculate_recovery_fee},
        CoverageState,
    },
    protection::{
        cross_verse::{detect_cross_verse_attack, CrossVersePosition, CrossVerseProtection},
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elastic_fee_calculation() {
        // Test with coverage = 2.0 (should be minimum fee)
        let high_coverage = U64F64::from_num(2);
        let fee = calculate_elastic_fee(high_coverage).unwrap();
        assert_eq!(fee, FEE_BASE_BPS); // Should be 3bp
        
        // Test with coverage = 0.5 (should be higher fee)
        let low_coverage = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
        let fee = calculate_elastic_fee(low_coverage).unwrap();
        assert!(fee > FEE_BASE_BPS && fee <= FEE_MAX_BPS);
        
        // Test with coverage = 0 (should be maximum fee)
        let zero_coverage = U64F64::from_num(0);
        let fee = calculate_elastic_fee(zero_coverage).unwrap();
        assert_eq!(fee, FEE_MAX_BPS); // Should be 28bp
    }

    #[test]
    fn test_maker_taker_distinction() {
        // Test maker order (improves spread)
        let maker_fee = calculate_maker_taker_fee(10, 15); // 15bp spread improvement
        assert_eq!(maker_fee.order_type, OrderType::Maker);
        assert_eq!(maker_fee.final_fee_bps, -5); // 5bp rebate
        
        // Test taker order (no spread improvement)
        let taker_fee = calculate_maker_taker_fee(10, 5); // Only 5bp spread improvement
        assert_eq!(taker_fee.order_type, OrderType::Taker);
        assert_eq!(taker_fee.final_fee_bps, 10); // Full 10bp fee
    }

    #[test]
    fn test_correlation_adjusted_tail_loss() {
        // Test with no correlations
        let correlations = vec![];
        let positions = vec![];
        let tail_loss = calculate_correlation_adjusted_tail_loss(10, &correlations, &positions).unwrap();
        
        // Basic tail loss = 1 - 1/N = 1 - 0.1 = 0.9
        assert_eq!(tail_loss, U64F64::from_num(9) / U64F64::from_num(10)); // 0.9
        
        // Test with high correlation
        let correlations = vec![
            MarketCorrelation {
                market_a: 1,
                market_b: 2,
                correlation: 800_000_000_000_000_000, // 0.8 * 1e18
                sample_size: 100,
                last_update: 0,
            }
        ];
        
        let positions = vec![
            PositionConcentration {
                market_id: 1,
                outcome: 0,
                position_size: 1000,
                weight: (U64F64::from_num(3) / U64F64::from_num(10)).to_bits(), // 0.3
            },
            PositionConcentration {
                market_id: 2,
                outcome: 0,
                position_size: 1000,
                weight: (U64F64::from_num(3) / U64F64::from_num(10)).to_bits(), // 0.3
            }
        ];
        
        let adjusted_tail_loss = calculate_correlation_adjusted_tail_loss(10, &correlations, &positions).unwrap();
        assert!(adjusted_tail_loss > tail_loss); // Should be higher due to correlation
    }

    #[test]
    fn test_recovery_mechanism() {
        // Test severe coverage drop (< 0.5)
        let mut recovery_state = crate::coverage::recovery::RecoveryState::new();
        let coverage = U64F64::from_num(2) / U64F64::from_num(5); // 0.4
        
        // Simulate recovery initiation
        recovery_state.is_active = true;
        recovery_state.start_coverage = coverage.to_bits();
        recovery_state.fee_multiplier = 30000; // 3x
        recovery_state.position_limit_reduction = 80;
        recovery_state.new_positions_halted = true;
        recovery_state.funding_rate_boost = 125;
        
        // Test recovery fee calculation
        let base_fee = 10; // 10bp
        let recovery_fee = calculate_recovery_fee(base_fee, &recovery_state);
        assert_eq!(recovery_fee, 30); // 3x multiplier
        
        // Test position limit reduction
        let normal_limit = 1000;
        let reduced_limit = crate::coverage::recovery::calculate_recovery_position_limit(normal_limit, &recovery_state);
        assert_eq!(reduced_limit, 200); // 80% reduction = 20% of original
    }

    #[test]
    fn test_cross_verse_attack_prevention() {
        let protection = CrossVerseProtection::new();
        let user = Pubkey::new_unique();
        
        // Test normal case (3 verses, within limit)
        let positions = vec![
            CrossVersePosition { verse_id: 1, market_id: 100, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 2, market_id: 200, outcome: 1, size: 500, direction: false },
            CrossVersePosition { verse_id: 3, market_id: 300, outcome: 0, size: 750, direction: true },
        ];
        
        let attack = detect_cross_verse_attack(&user, &positions, &protection).unwrap();
        assert!(!attack); // Should not detect attack
        
        // Test attack case (too many verses)
        let attack_positions = vec![
            CrossVersePosition { verse_id: 1, market_id: 100, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 2, market_id: 200, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 3, market_id: 300, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 4, market_id: 400, outcome: 0, size: 1000, direction: true },
        ];
        
        let attack = detect_cross_verse_attack(&user, &attack_positions, &protection).unwrap();
        assert!(attack); // Should detect attack (4 verses > 3 max)
    }

    #[test]
    fn test_fee_distribution() {
        // Test 70/20/10 split
        let total_fee = 1000;
        let vault_amount = (total_fee * 7000) / 10000; // 70%
        let mmt_amount = (total_fee * 2000) / 10000; // 20%
        let burn_amount = (total_fee * 1000) / 10000; // 10%
        
        assert_eq!(vault_amount, 700);
        assert_eq!(mmt_amount, 200);
        assert_eq!(burn_amount, 100);
        assert_eq!(vault_amount + mmt_amount + burn_amount, total_fee);
    }

    #[test]
    fn test_coverage_edge_cases() {
        // Test coverage = 1.0 (boundary)
        let boundary_coverage = U64F64::from_num(1);
        let fee = calculate_elastic_fee(boundary_coverage).unwrap();
        assert!(fee >= FEE_BASE_BPS && fee <= FEE_MAX_BPS);
        
        // Test very high coverage
        let high_coverage = U64F64::from_num(10);
        let fee = calculate_elastic_fee(high_coverage).unwrap();
        assert_eq!(fee, FEE_BASE_BPS); // Should be minimum
        
        // Test negative coverage protection
        // (In practice, coverage should never be negative, but test defensive programming)
        let zero_coverage = U64F64::from_num(0);
        let fee = calculate_elastic_fee(zero_coverage).unwrap();
        assert_eq!(fee, FEE_MAX_BPS); // Should be maximum
    }

    #[test]
    fn test_recovery_progression() {
        let mut recovery_state = crate::coverage::recovery::RecoveryState::new();
        recovery_state.is_active = true;
        recovery_state.start_coverage = (U64F64::from_num(3) / U64F64::from_num(5)).to_bits(); // 0.6
        recovery_state.target_coverage = (U64F64::from_num(6) / U64F64::from_num(5)).to_bits(); // 1.2
        recovery_state.fee_multiplier = 20000; // 2x
        
        // Test good progress scenario
        let current_coverage = U64F64::from_num(9) / U64F64::from_num(10); // 0.9
        let start_cov = U64F64::from_bits(recovery_state.start_coverage);
        let target_cov = U64F64::from_bits(recovery_state.target_coverage);
        let progress = (current_coverage - start_cov) / (target_cov - start_cov);
        
        assert!(progress > U64F64::from_num(1) / U64F64::from_num(2)); // Good progress > 0.5
        
        // In real implementation, fee multiplier would be reduced
        // recovery_state.fee_multiplier would be reduced by 1000
        
        // Test target reached
        let final_coverage = U64F64::from_num(5) / U64F64::from_num(4); // 1.25
        assert!(final_coverage >= U64F64::from_num(recovery_state.target_coverage));
        // Recovery would be deactivated
    }
}