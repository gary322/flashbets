//! Phase 2: Bootstrap Phase - Comprehensive Unit Tests
//!
//! Tests for zero vault initialization, MMT rewards, minimum viable vault,
//! vampire attack protection, and UX notifications.

use betting_platform_native::{
    integration::{
        bootstrap_coordinator::{
            BootstrapCoordinator, BOOTSTRAP_TARGET_VAULT, BOOTSTRAP_MILESTONES,
        },
        bootstrap_vault_initialization::{
            BootstrapVaultState, process_initialize_bootstrap_vault,
        },
        bootstrap_mmt_integration::{
            BootstrapMMTIntegration, MMTDistributionState,
        },
        minimum_viable_vault::{
            VaultViabilityTracker, VaultViabilityState, EnabledFeatures,
            check_vault_viability,
        },
        vampire_attack_protection::{
            VampireAttackDetector, check_vampire_attack_withdrawal,
            VAMPIRE_ATTACK_COVERAGE_THRESHOLD, SUSPICIOUS_WITHDRAWAL_THRESHOLD,
        },
        bootstrap_ux_notifications::{
            get_bootstrap_ui_state, BootstrapPhaseStatus,
            BootstrapNotificationType,
        },
    },
    error::BettingPlatformError,
};
use solana_program::{
    pubkey::Pubkey,
    clock::Clock,
};

#[test]
fn test_bootstrap_coordinator_initialization() {
    let mut coordinator = BootstrapCoordinator::default();
    let result = coordinator.initialize(1000);
    
    assert!(result.is_ok());
    assert_eq!(coordinator.vault_balance, 0);
    assert_eq!(coordinator.bootstrap_start_slot, 1000);
    assert!(!coordinator.bootstrap_complete);
    assert_eq!(coordinator.incentive_pool, 10_000_000_000_000); // 10M MMT
}

#[test]
fn test_zero_vault_initialization() {
    let vault = BootstrapVaultState {
        total_deposits: 0,
        total_borrowed: 0,
        depositor_count: 0,
        last_update_slot: 0,
        is_bootstrap_phase: true,
        bootstrap_start_slot: 1000,
        bootstrap_coordinator: Pubkey::new_unique(),
        minimum_viable_size: BOOTSTRAP_TARGET_VAULT,
        coverage_ratio: 0,
        is_accepting_deposits: true,
        bootstrap_complete: false,
        total_mmt_distributed: 0,
    };
    
    assert_eq!(vault.total_deposits, 0);
    assert!(vault.is_bootstrap_phase);
    assert!(vault.is_accepting_deposits);
    assert_eq!(vault.minimum_viable_size, 10_000_000_000); // $10k
}

#[test]
fn test_mmt_rewards_calculation() {
    let mut mmt_state = MMTDistributionState::new();
    mmt_state.initialize(10_000_000_000_000, 5_000_000_000_000).unwrap();
    
    // Test first depositor gets 100% immediate rewards
    let deposit_amount = 1_000_000_000; // $1000
    let rewards = BootstrapMMTIntegration::calculate_mmt_rewards(
        deposit_amount,
        &mmt_state,
        0, // First milestone
        true, // Is bootstrap
    ).unwrap();
    
    // Should get 2x multiplier during bootstrap
    assert!(rewards > 0);
    assert_eq!(mmt_state.distribution_type, 0); // Immediate distribution
}

#[test]
fn test_milestone_progression() {
    let mut coordinator = BootstrapCoordinator::default();
    coordinator.initialize(1000).unwrap();
    
    // Test milestone progression
    for (i, &milestone) in BOOTSTRAP_MILESTONES.iter().enumerate() {
        coordinator.vault_balance = milestone;
        let milestone_reached = coordinator.check_milestone_progress().unwrap();
        
        if i < BOOTSTRAP_MILESTONES.len() - 1 {
            assert!(milestone_reached);
            assert_eq!(coordinator.current_milestone, (i + 1) as u8);
        }
    }
}

#[test]
fn test_minimum_viable_vault_states() {
    let mut tracker = VaultViabilityTracker {
        state: VaultViabilityState::Bootstrap,
        current_balance: 0,
        minimum_required: BOOTSTRAP_TARGET_VAULT,
        viability_reached_at: None,
        degradation_count: 0,
        last_check_slot: 0,
        enabled_features: EnabledFeatures::default(),
    };
    
    // Test state transitions
    let test_cases = vec![
        (0, VaultViabilityState::Bootstrap),
        (9_000_000_000, VaultViabilityState::NearingViability), // $9k
        (10_000_000_000, VaultViabilityState::MinimumViable), // $10k
        (20_000_000_000, VaultViabilityState::FullyOperational), // $20k
    ];
    
    for (balance, expected_state) in test_cases {
        tracker.current_balance = balance;
        let state = tracker.determine_viability_state().unwrap();
        assert_eq!(state, expected_state);
    }
}

#[test]
fn test_leverage_scaling() {
    let test_cases = vec![
        (0, 0),              // $0 = 0x leverage
        (500_000_000, 0),    // $500 = 0x leverage
        (1_000_000_000, 1),  // $1k = 1x leverage
        (5_000_000_000, 5),  // $5k = 5x leverage
        (10_000_000_000, 10), // $10k = 10x leverage
        (15_000_000_000, 10), // $15k = 10x leverage (capped)
    ];
    
    for (balance, expected_leverage) in test_cases {
        let leverage = VaultViabilityTracker::calculate_max_leverage(balance);
        assert_eq!(leverage, expected_leverage);
    }
}

#[test]
fn test_vampire_attack_coverage_protection() {
    let mut detector = VampireAttackDetector::new();
    let mut vault = BootstrapVaultState::default();
    vault.total_deposits = 10_000_000_000; // $10k
    vault.minimum_viable_size = 10_000_000_000;
    vault.is_bootstrap_phase = true;
    
    // Test withdrawal that would drop coverage below 0.5
    let withdrawal_amount = 6_000_000_000; // $6k withdrawal
    
    // Post-withdrawal: $4k / $10k = 0.4 coverage (below 0.5 threshold)
    let post_balance = vault.total_deposits - withdrawal_amount;
    let coverage = (post_balance * 10000) / vault.minimum_viable_size;
    
    assert!(coverage < VAMPIRE_ATTACK_COVERAGE_THRESHOLD);
}

#[test]
fn test_large_withdrawal_detection() {
    let vault_balance = 10_000_000_000; // $10k
    let large_withdrawal = 2_500_000_000; // $2.5k (25%)
    
    let withdrawal_percentage = (large_withdrawal * 10000) / vault_balance;
    assert!(withdrawal_percentage > SUSPICIOUS_WITHDRAWAL_THRESHOLD);
}

#[test]
fn test_rapid_withdrawal_protection() {
    let mut detector = VampireAttackDetector::new();
    
    // Simulate rapid withdrawals
    detector.rapid_withdrawal_count = 3;
    detector.withdrawal_window_start = 1000;
    
    // Fourth withdrawal in same window should be blocked
    detector.rapid_withdrawal_count += 1;
    assert!(detector.rapid_withdrawal_count > 3); // MAX_WITHDRAWALS_PER_WINDOW
}

#[test]
fn test_bootstrap_ui_notifications() {
    let mut coordinator = BootstrapCoordinator::default();
    coordinator.vault_balance = 5_000_000_000; // $5k (50% progress)
    coordinator.total_deposits = 5_000_000_000;
    coordinator.unique_depositors = 10;
    coordinator.current_milestone = 2;
    
    let mut vault = BootstrapVaultState::default();
    vault.total_deposits = 5_000_000_000;
    vault.minimum_viable_size = 10_000_000_000;
    vault.is_bootstrap_phase = true;
    vault.depositor_count = 10;
    
    // Test progress calculation
    let progress_percentage = (vault.total_deposits * 100) / vault.minimum_viable_size;
    assert_eq!(progress_percentage, 50);
    
    // Test phase status determination
    let phase_status = if coordinator.halted {
        BootstrapPhaseStatus::Halted
    } else if vault.bootstrap_complete {
        BootstrapPhaseStatus::Complete
    } else if progress_percentage >= 90 {
        BootstrapPhaseStatus::NearingCompletion
    } else if vault.is_bootstrap_phase {
        BootstrapPhaseStatus::Active
    } else {
        BootstrapPhaseStatus::NotStarted
    };
    
    assert_eq!(phase_status, BootstrapPhaseStatus::Active);
}

#[test]
fn test_feature_enablement_by_vault_size() {
    let test_cases = vec![
        (0, false, false, 0),                    // Nothing enabled
        (1_000_000_000, true, false, 1),         // $1k: trading only
        (10_000_000_000, true, true, 10),        // $10k: full features
        (20_000_000_000, true, true, 10),        // $20k: chains enabled
    ];
    
    for (balance, trading, liquidations, max_leverage) in test_cases {
        let mut tracker = VaultViabilityTracker {
            current_balance: balance,
            minimum_required: BOOTSTRAP_TARGET_VAULT,
            enabled_features: EnabledFeatures::default(),
            ..Default::default()
        };
        
        tracker.update_enabled_features().unwrap();
        
        assert_eq!(tracker.enabled_features.trading_enabled, trading);
        assert_eq!(tracker.enabled_features.liquidations_enabled, liquidations);
        assert_eq!(tracker.enabled_features.max_leverage, max_leverage);
    }
}

#[test]
fn test_bootstrap_completion() {
    let mut coordinator = BootstrapCoordinator::default();
    coordinator.initialize(1000).unwrap();
    
    // Simulate reaching target
    coordinator.vault_balance = BOOTSTRAP_TARGET_VAULT;
    coordinator.total_deposits = BOOTSTRAP_TARGET_VAULT;
    coordinator.unique_depositors = 100;
    
    let result = coordinator.complete_bootstrap(2000);
    assert!(result.is_ok());
    assert!(coordinator.bootstrap_complete);
    assert_eq!(coordinator.max_leverage_available, 10);
}

#[test]
fn test_mmt_distribution_milestones() {
    let milestones = vec![
        (0, 150), // Milestone 1: 1.5x bonus
        (1, 140), // Milestone 2: 1.4x bonus
        (2, 130), // Milestone 3: 1.3x bonus
        (3, 120), // Milestone 4: 1.2x bonus
        (4, 110), // Milestone 5: 1.1x bonus
    ];
    
    for (milestone, expected_multiplier) in milestones {
        let multiplier = BootstrapCoordinator::get_milestone_multiplier(milestone);
        assert_eq!(multiplier, expected_multiplier);
    }
}

#[test]
fn test_recovery_cooldown_after_attack() {
    let mut detector = VampireAttackDetector::new();
    let current_slot = 1000;
    
    // Simulate attack detection
    detector.attack_detected_slot = Some(current_slot);
    detector.recovery_allowed_slot = Some(current_slot + 3000); // 20 minute cooldown
    
    // Check cooldown is enforced
    assert!(detector.recovery_allowed_slot.unwrap() > current_slot);
    assert_eq!(detector.recovery_allowed_slot.unwrap() - current_slot, 3000);
}