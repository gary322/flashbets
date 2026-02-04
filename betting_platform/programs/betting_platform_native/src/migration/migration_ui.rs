//! Migration UI Components
//!
//! Provides user interface elements for migration wizard,
//! audit transparency, and reward visualization

use solana_program::{
    account_info::AccountInfo,
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
    migration::{MigrationWizardState, MigrationStep, PositionInfo},
};

/// Migration UI configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationUIConfig {
    /// Show audit details
    pub show_audit_details: bool,
    
    /// Show bug fix details
    pub show_bug_details: bool,
    
    /// Enable reward animations
    pub enable_animations: bool,
    
    /// Migration target percentage (default 70%)
    pub target_migration_rate: u8,
    
    /// UI theme
    pub theme: UITheme,
}

impl Default for MigrationUIConfig {
    fn default() -> Self {
        Self {
            show_audit_details: true,
            show_bug_details: true,
            enable_animations: true,
            target_migration_rate: 70,
            theme: UITheme::Dark,
        }
    }
}

/// UI theme options
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum UITheme {
    Light,
    Dark,
    Auto,
}

/// Audit transparency details
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AuditDetails {
    /// Audit firm name
    pub auditor: String,
    
    /// Audit completion date
    pub audit_date: i64,
    
    /// Critical findings count
    pub critical_findings: u8,
    
    /// High findings count
    pub high_findings: u8,
    
    /// Medium findings count
    pub medium_findings: u8,
    
    /// Low findings count
    pub low_findings: u8,
    
    /// All findings resolved
    pub all_resolved: bool,
    
    /// Audit report URL (IPFS hash)
    pub report_url: String,
}

/// Bug fix transparency
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BugFixDetails {
    /// Bug severity
    pub severity: BugSeverity,
    
    /// Bug description
    pub description: String,
    
    /// Fix description
    pub fix_description: String,
    
    /// Affected users count
    pub affected_users: u32,
    
    /// Fix verified by auditor
    pub fix_verified: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum BugSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Migration progress visualization
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationProgress {
    /// Overall migration percentage
    pub overall_percentage: u8,
    
    /// Positions migrated
    pub positions_migrated: u32,
    
    /// Total positions
    pub total_positions: u32,
    
    /// Value migrated (USD)
    pub value_migrated: u64,
    
    /// Total value (USD)
    pub total_value: u64,
    
    /// Time remaining in migration period
    pub time_remaining_slots: u64,
    
    /// Estimated rewards if complete now
    pub current_rewards: u64,
    
    /// Maximum possible rewards
    pub max_rewards: u64,
}

/// Migration wizard UI state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationWizardUI {
    /// Current wizard step
    pub step: WizardStep,
    
    /// Step progress (0-100)
    pub step_progress: u8,
    
    /// User messages
    pub messages: Vec<UIMessage>,
    
    /// Action buttons
    pub actions: Vec<UIAction>,
    
    /// Help text
    pub help_text: String,
    
    /// Show advanced options
    pub show_advanced: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum WizardStep {
    Welcome,
    ConnectWallet,
    ScanPositions,
    ReviewPositions,
    ConfirmMigration,
    Processing,
    Complete,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UIMessage {
    pub message_type: MessageType,
    pub text: String,
    pub timestamp: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum MessageType {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UIAction {
    pub action_id: String,
    pub label: String,
    pub enabled: bool,
    pub primary: bool,
}

/// Safe migrate analyzer
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SafeMigrateAnalyzer {
    /// Risk score (0-100, lower is safer)
    pub risk_score: u8,
    
    /// Risk factors
    pub risk_factors: Vec<RiskFactor>,
    
    /// Safety recommendations
    pub recommendations: Vec<String>,
    
    /// Estimated gas costs
    pub estimated_gas: u64,
    
    /// Estimated time to complete
    pub estimated_time_seconds: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RiskFactor {
    pub factor: String,
    pub severity: RiskSeverity,
    pub mitigation: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
}

/// Reward visualization
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RewardVisualization {
    /// Base rewards amount
    pub base_rewards: u64,
    
    /// Bonus multiplier (2x for migration)
    pub multiplier: f32,
    
    /// Total rewards
    pub total_rewards: u64,
    
    /// Rewards breakdown
    pub breakdown: Vec<RewardComponent>,
    
    /// Vesting schedule
    pub vesting_schedule: VestingSchedule,
    
    /// Comparison with no migration
    pub comparison: RewardComparison,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RewardComponent {
    pub name: String,
    pub amount: u64,
    pub percentage: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VestingSchedule {
    pub immediate_percentage: u8,
    pub vesting_period_days: u16,
    pub cliff_days: u16,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RewardComparison {
    pub with_migration: u64,
    pub without_migration: u64,
    pub additional_rewards: u64,
    pub percentage_increase: u16,
}

/// Migration UI implementation
impl MigrationWizardUI {
    /// Create new wizard UI
    pub fn new() -> Self {
        Self {
            step: WizardStep::Welcome,
            step_progress: 0,
            messages: vec![
                UIMessage {
                    message_type: MessageType::Info,
                    text: "Welcome to Safe Migration Wizard".to_string(),
                    timestamp: Clock::get().unwrap().unix_timestamp,
                }
            ],
            actions: vec![
                UIAction {
                    action_id: "start".to_string(),
                    label: "Start Migration".to_string(),
                    enabled: true,
                    primary: true,
                }
            ],
            help_text: "This wizard will guide you through migrating your positions safely.".to_string(),
            show_advanced: false,
        }
    }
    
    /// Update wizard step
    pub fn update_step(&mut self, step: WizardStep) {
        self.step = step;
        self.step_progress = 0;
        
        // Update actions based on step
        match &self.step {
            WizardStep::Welcome => {
                self.actions = vec![
                    UIAction {
                        action_id: "start".to_string(),
                        label: "Start Migration".to_string(),
                        enabled: true,
                        primary: true,
                    }
                ];
            }
            WizardStep::ConnectWallet => {
                self.actions = vec![
                    UIAction {
                        action_id: "connect".to_string(),
                        label: "Connect Wallet".to_string(),
                        enabled: true,
                        primary: true,
                    }
                ];
            }
            WizardStep::ScanPositions => {
                self.actions = vec![
                    UIAction {
                        action_id: "scan".to_string(),
                        label: "Scan Positions".to_string(),
                        enabled: true,
                        primary: true,
                    }
                ];
            }
            WizardStep::ReviewPositions => {
                self.actions = vec![
                    UIAction {
                        action_id: "select_all".to_string(),
                        label: "Select All".to_string(),
                        enabled: true,
                        primary: false,
                    },
                    UIAction {
                        action_id: "migrate".to_string(),
                        label: "Migrate Selected".to_string(),
                        enabled: true,
                        primary: true,
                    }
                ];
            }
            WizardStep::ConfirmMigration => {
                self.actions = vec![
                    UIAction {
                        action_id: "cancel".to_string(),
                        label: "Cancel".to_string(),
                        enabled: true,
                        primary: false,
                    },
                    UIAction {
                        action_id: "confirm".to_string(),
                        label: "Confirm Migration".to_string(),
                        enabled: true,
                        primary: true,
                    }
                ];
            }
            WizardStep::Processing => {
                self.actions = vec![]; // No actions during processing
            }
            WizardStep::Complete => {
                self.actions = vec![
                    UIAction {
                        action_id: "view_positions".to_string(),
                        label: "View New Positions".to_string(),
                        enabled: true,
                        primary: true,
                    },
                    UIAction {
                        action_id: "close".to_string(),
                        label: "Close".to_string(),
                        enabled: true,
                        primary: false,
                    }
                ];
            }
        }
    }
    
    /// Add message to UI
    pub fn add_message(&mut self, message_type: MessageType, text: String) {
        self.messages.push(UIMessage {
            message_type,
            text,
            timestamp: Clock::get().unwrap().unix_timestamp,
        });
        
        // Keep only last 10 messages
        if self.messages.len() > 10 {
            self.messages.remove(0);
        }
    }
    
    /// Update progress
    pub fn update_progress(&mut self, progress: u8) {
        self.step_progress = progress.min(100);
    }
}

/// Get sample audit details for transparency
pub fn get_sample_audit_details() -> AuditDetails {
    AuditDetails {
        auditor: "Trail of Bits".to_string(),
        audit_date: 1735689600, // Jan 1, 2025
        critical_findings: 0,
        high_findings: 2,
        medium_findings: 5,
        low_findings: 12,
        all_resolved: true,
        report_url: "ipfs://QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco".to_string(),
    }
}

/// Get sample bug fixes for transparency
pub fn get_sample_bug_fixes() -> Vec<BugFixDetails> {
    vec![
        BugFixDetails {
            severity: BugSeverity::High,
            description: "Integer overflow in leverage calculation".to_string(),
            fix_description: "Added checked math operations".to_string(),
            affected_users: 0,
            fix_verified: true,
        },
        BugFixDetails {
            severity: BugSeverity::Medium,
            description: "Race condition in order matching".to_string(),
            fix_description: "Implemented proper locking mechanism".to_string(),
            affected_users: 3,
            fix_verified: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wizard_ui_flow() {
        let mut ui = MigrationWizardUI::new();
        
        assert!(matches!(ui.step, WizardStep::Welcome));
        assert_eq!(ui.actions.len(), 1);
        
        ui.update_step(WizardStep::ScanPositions);
        assert!(matches!(ui.step, WizardStep::ScanPositions));
        
        ui.update_progress(50);
        assert_eq!(ui.step_progress, 50);
        
        ui.add_message(MessageType::Success, "Found 5 positions".to_string());
        assert_eq!(ui.messages.len(), 2); // Initial + new
    }
}