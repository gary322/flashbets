//! Bootstrap Phase UX Notifications Module
//!
//! Provides structured notification data for UI components to display
//! bootstrap phase status, progress, and alerts to users.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    integration::{
        bootstrap_coordinator::BootstrapCoordinator,
        bootstrap_vault_initialization::BootstrapVaultState,
        minimum_viable_vault::{VaultViabilityTracker, VaultViabilityState},
        vampire_attack_protection::VampireAttackDetector,
    },
};

/// Bootstrap notification types for UI
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BootstrapNotificationType {
    Info = 0,
    Success = 1,
    Warning = 2,
    Alert = 3,
    Critical = 4,
}

/// Bootstrap UI banner state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BootstrapBannerState {
    /// Current phase status
    pub phase_status: BootstrapPhaseStatus,
    /// Progress towards minimum viable vault
    pub progress_percentage: u64,
    /// Current vault balance
    pub current_balance: u64,
    /// Target balance ($10k)
    pub target_balance: u64,
    /// Time remaining estimate (slots)
    pub estimated_time_remaining: Option<u64>,
    /// Number of depositors
    pub depositor_count: u32,
    /// MMT rewards distributed
    pub mmt_distributed: u64,
    /// MMT rewards remaining
    pub mmt_remaining: u64,
    /// Active notifications
    pub notifications: Vec<BootstrapNotification>,
    /// Feature availability
    pub features_enabled: FeatureStatus,
    /// Security status
    pub security_status: SecurityStatus,
}

/// Bootstrap phase status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BootstrapPhaseStatus {
    NotStarted = 0,
    Active = 1,
    NearingCompletion = 2,
    Complete = 3,
    Halted = 4,
}

/// Individual notification
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BootstrapNotification {
    pub notification_type: BootstrapNotificationType,
    pub title: String,
    pub message: String,
    pub action_required: bool,
    pub timestamp: i64,
    pub expires_at: Option<i64>,
}

/// Feature availability status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct FeatureStatus {
    pub deposits_enabled: bool,
    pub trading_enabled: bool,
    pub leverage_available: u8,
    pub liquidations_enabled: bool,
    pub chains_enabled: bool,
    pub withdrawals_enabled: bool,
}

/// Security status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SecurityStatus {
    pub coverage_ratio: u64,
    pub vampire_protection_active: bool,
    pub recent_attacks: u32,
    pub risk_level: RiskLevel,
}

/// Risk level indicator
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Get current bootstrap UI state for display
pub fn get_bootstrap_ui_state(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Result<BootstrapBannerState, ProgramError> {
    let account_info_iter = &mut accounts.iter();
    
    let coordinator_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let viability_tracker_account = next_account_info(account_info_iter)?;
    let detector_account = next_account_info(account_info_iter)?;
    
    let clock = Clock::get()?;
    
    // Load all states
    let coordinator = BootstrapCoordinator::deserialize(&mut &coordinator_account.data.borrow()[..])?;
    let vault = BootstrapVaultState::deserialize(&mut &vault_account.data.borrow()[..])?;
    let tracker = VaultViabilityTracker::deserialize(&mut &viability_tracker_account.data.borrow()[..])?;
    let detector = VampireAttackDetector::deserialize(&mut &detector_account.data.borrow()[..])?;
    
    // Calculate progress
    let progress_percentage = if vault.minimum_viable_size > 0 {
        (vault.total_deposits * 100) / vault.minimum_viable_size
    } else {
        0
    };
    
    // Determine phase status
    let phase_status = determine_phase_status(&coordinator, &vault, progress_percentage);
    
    // Build notifications
    let notifications = build_notifications(
        &coordinator,
        &vault,
        &tracker,
        &detector,
        progress_percentage,
        clock.unix_timestamp,
    );
    
    // Get feature status
    let features_enabled = FeatureStatus {
        deposits_enabled: vault.is_accepting_deposits && !coordinator.halted,
        trading_enabled: tracker.enabled_features.trading_enabled,
        leverage_available: tracker.enabled_features.max_leverage,
        liquidations_enabled: tracker.enabled_features.liquidations_enabled,
        chains_enabled: tracker.enabled_features.chain_positions_enabled,
        withdrawals_enabled: detector.recovery_allowed_slot.map_or(true, |slot| clock.slot >= slot),
    };
    
    // Calculate security status
    let coverage_ratio = vault.coverage_ratio;
    let risk_level = calculate_risk_level(coverage_ratio, &detector);
    
    let security_status = SecurityStatus {
        coverage_ratio,
        vampire_protection_active: detector.protection_active,
        recent_attacks: detector.total_attacks_prevented,
        risk_level,
    };
    
    // Estimate time remaining
    let estimated_time_remaining = estimate_completion_time(&vault, &coordinator);
    
    Ok(BootstrapBannerState {
        phase_status,
        progress_percentage,
        current_balance: vault.total_deposits,
        target_balance: vault.minimum_viable_size,
        estimated_time_remaining,
        depositor_count: vault.depositor_count,
        mmt_distributed: vault.total_mmt_distributed,
        mmt_remaining: coordinator.total_incentive_pool.saturating_sub(vault.total_mmt_distributed),
        notifications,
        features_enabled,
        security_status,
    })
}

/// Determine current phase status
fn determine_phase_status(
    coordinator: &BootstrapCoordinator,
    vault: &BootstrapVaultState,
    progress_percentage: u64,
) -> BootstrapPhaseStatus {
    if coordinator.halted {
        BootstrapPhaseStatus::Halted
    } else if vault.bootstrap_complete {
        BootstrapPhaseStatus::Complete
    } else if progress_percentage >= 90 {
        BootstrapPhaseStatus::NearingCompletion
    } else if vault.is_bootstrap_phase {
        BootstrapPhaseStatus::Active
    } else {
        BootstrapPhaseStatus::NotStarted
    }
}

/// Build notification list based on current state
fn build_notifications(
    coordinator: &BootstrapCoordinator,
    vault: &BootstrapVaultState,
    tracker: &VaultViabilityTracker,
    detector: &VampireAttackDetector,
    progress_percentage: u64,
    current_timestamp: i64,
) -> Vec<BootstrapNotification> {
    let mut notifications = Vec::new();
    
    // Bootstrap phase notification
    if vault.is_bootstrap_phase && !vault.bootstrap_complete {
        if progress_percentage < 50 {
            notifications.push(BootstrapNotification {
                notification_type: BootstrapNotificationType::Info,
                title: "Bootstrap Phase Active".to_string(),
                message: format!(
                    "Help us reach $10k to unlock full platform features! Current: ${:.2}k",
                    vault.total_deposits as f64 / 1_000_000_000.0
                ),
                action_required: false,
                timestamp: current_timestamp,
                expires_at: None,
            });
        } else if progress_percentage >= 90 {
            notifications.push(BootstrapNotification {
                notification_type: BootstrapNotificationType::Success,
                title: "Almost There!".to_string(),
                message: format!(
                    "Only ${:.2}k to go! Platform will be fully operational soon.",
                    (vault.minimum_viable_size - vault.total_deposits) as f64 / 1_000_000_000.0
                ),
                action_required: false,
                timestamp: current_timestamp,
                expires_at: None,
            });
        }
    }
    
    // MMT rewards notification
    if coordinator.total_incentive_pool > vault.total_mmt_distributed {
        let remaining_mmt = coordinator.total_incentive_pool - vault.total_mmt_distributed;
        notifications.push(BootstrapNotification {
            notification_type: BootstrapNotificationType::Success,
            title: "MMT Rewards Available".to_string(),
            message: format!(
                "{} MMT remaining for early depositors!",
                remaining_mmt / 1_000_000 // Convert to display units
            ),
            action_required: false,
            timestamp: current_timestamp,
            expires_at: None,
        });
    }
    
    // Vampire attack warning
    if detector.attack_detected_slot.is_some() {
        notifications.push(BootstrapNotification {
            notification_type: BootstrapNotificationType::Critical,
            title: "Security Alert".to_string(),
            message: "Vampire attack detected. Withdrawals temporarily restricted.".to_string(),
            action_required: false,
            timestamp: current_timestamp,
            expires_at: detector.recovery_allowed_slot.map(|_| current_timestamp + 1200), // 20 minutes
        });
    }
    
    // Coverage warning
    if vault.coverage_ratio < 7000 { // Below 0.7
        notifications.push(BootstrapNotification {
            notification_type: BootstrapNotificationType::Warning,
            title: "Low Coverage Warning".to_string(),
            message: format!(
                "Coverage ratio at {:.1}. Platform features may be restricted.",
                vault.coverage_ratio as f64 / 10000.0
            ),
            action_required: false,
            timestamp: current_timestamp,
            expires_at: None,
        });
    }
    
    // Feature unlock notifications
    match tracker.state {
        VaultViabilityState::NearingViability => {
            notifications.push(BootstrapNotification {
                notification_type: BootstrapNotificationType::Info,
                title: "Trading Unlocked Soon".to_string(),
                message: "Basic trading features will be available once we reach $10k".to_string(),
                action_required: false,
                timestamp: current_timestamp,
                expires_at: None,
            });
        },
        VaultViabilityState::MinimumViable => {
            notifications.push(BootstrapNotification {
                notification_type: BootstrapNotificationType::Success,
                title: "Platform Operational!".to_string(),
                message: "Trading, leverage, and liquidations are now enabled.".to_string(),
                action_required: false,
                timestamp: current_timestamp,
                expires_at: Some(current_timestamp + 3600), // 1 hour
            });
        },
        VaultViabilityState::FullyOperational => {
            notifications.push(BootstrapNotification {
                notification_type: BootstrapNotificationType::Success,
                title: "All Features Unlocked!".to_string(),
                message: "Chain positions and advanced features are now available.".to_string(),
                action_required: false,
                timestamp: current_timestamp,
                expires_at: Some(current_timestamp + 3600), // 1 hour
            });
        },
        VaultViabilityState::Degraded => {
            notifications.push(BootstrapNotification {
                notification_type: BootstrapNotificationType::Alert,
                title: "Vault Degraded".to_string(),
                message: "Vault has fallen below minimum. Only position closing allowed.".to_string(),
                action_required: true,
                timestamp: current_timestamp,
                expires_at: None,
            });
        },
        _ => {}
    }
    
    notifications
}

/// Calculate risk level based on system state
fn calculate_risk_level(coverage_ratio: u64, detector: &VampireAttackDetector) -> RiskLevel {
    if detector.attack_detected_slot.is_some() {
        RiskLevel::Critical
    } else if coverage_ratio < 5000 { // Below 0.5
        RiskLevel::High
    } else if coverage_ratio < 7000 { // Below 0.7
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

/// Estimate time to completion based on recent deposit rate
fn estimate_completion_time(
    vault: &BootstrapVaultState,
    coordinator: &BootstrapCoordinator,
) -> Option<u64> {
    if vault.bootstrap_complete || vault.total_deposits >= vault.minimum_viable_size {
        return None;
    }
    
    // Simple estimation based on progress
    // In production, this would use historical deposit rates
    let remaining_amount = vault.minimum_viable_size - vault.total_deposits;
    let average_deposit = if vault.depositor_count > 0 {
        vault.total_deposits / vault.depositor_count as u64
    } else {
        1_000_000_000 // Default $1k average
    };
    
    // Estimate deposits needed
    let deposits_needed = remaining_amount / average_deposit;
    
    // Rough estimate: 1 deposit per 100 slots (~40 seconds)
    Some(deposits_needed * 100)
}

/// Format notification for display
pub fn format_notification_display(notification: &BootstrapNotification) -> String {
    let icon = match notification.notification_type {
        BootstrapNotificationType::Info => "ℹ️",
        BootstrapNotificationType::Success => "✅",
        BootstrapNotificationType::Warning => "⚠️",
        BootstrapNotificationType::Alert => "🚨",
        BootstrapNotificationType::Critical => "🛑",
    };
    
    format!("{} {} - {}", icon, notification.title, notification.message)
}

/// Get milestone progress for UI display
pub fn get_milestone_progress(current_balance: u64) -> Vec<(String, bool)> {
    vec![
        ("$1k - Basic Trading".to_string(), current_balance >= 1_000_000_000),
        ("$2.5k - 2.5x Leverage".to_string(), current_balance >= 2_500_000_000),
        ("$5k - 5x Leverage".to_string(), current_balance >= 5_000_000_000),
        ("$7.5k - 7.5x Leverage".to_string(), current_balance >= 7_500_000_000),
        ("$10k - Full Platform".to_string(), current_balance >= 10_000_000_000),
        ("$20k - Chain Positions".to_string(), current_balance >= 20_000_000_000),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_phase_status_determination() {
        let mut coordinator = BootstrapCoordinator {
            halted: false,
            ..Default::default()
        };
        let mut vault = BootstrapVaultState {
            is_bootstrap_phase: true,
            bootstrap_complete: false,
            ..Default::default()
        };
        
        // Test active phase
        assert_eq!(
            determine_phase_status(&coordinator, &vault, 50),
            BootstrapPhaseStatus::Active
        );
        
        // Test nearing completion
        assert_eq!(
            determine_phase_status(&coordinator, &vault, 92),
            BootstrapPhaseStatus::NearingCompletion
        );
        
        // Test complete
        vault.bootstrap_complete = true;
        assert_eq!(
            determine_phase_status(&coordinator, &vault, 100),
            BootstrapPhaseStatus::Complete
        );
        
        // Test halted
        coordinator.halted = true;
        assert_eq!(
            determine_phase_status(&coordinator, &vault, 50),
            BootstrapPhaseStatus::Halted
        );
    }
    
    #[test]
    fn test_risk_level_calculation() {
        let mut detector = VampireAttackDetector::new();
        
        // Test low risk
        assert_eq!(calculate_risk_level(8000, &detector), RiskLevel::Low);
        
        // Test medium risk
        assert_eq!(calculate_risk_level(6000, &detector), RiskLevel::Medium);
        
        // Test high risk
        assert_eq!(calculate_risk_level(4000, &detector), RiskLevel::High);
        
        // Test critical risk (attack detected)
        detector.attack_detected_slot = Some(1000);
        assert_eq!(calculate_risk_level(8000, &detector), RiskLevel::Critical);
    }
    
    #[test]
    fn test_milestone_progress() {
        let milestones = get_milestone_progress(5_500_000_000); // $5.5k
        
        assert_eq!(milestones[0].1, true);  // $1k reached
        assert_eq!(milestones[1].1, true);  // $2.5k reached
        assert_eq!(milestones[2].1, true);  // $5k reached
        assert_eq!(milestones[3].1, false); // $7.5k not reached
        assert_eq!(milestones[4].1, false); // $10k not reached
        assert_eq!(milestones[5].1, false); // $20k not reached
    }
}