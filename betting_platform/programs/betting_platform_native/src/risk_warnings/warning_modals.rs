//! Warning Modal System
//!
//! Implements mandatory warning modals for high-risk actions
//! Based on specification: "80% lose long-term, like casino but with skill edge"

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
    risk_warnings::RiskLevel,
};

/// Warning modal types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum WarningModalType {
    /// High leverage warning (>50x)
    HighLeverage,
    /// Extreme leverage warning (500x)
    ExtremeLeverage500x,
    /// First time user warning
    FirstTimeUser,
    /// Large position warning (>$10k)
    LargePosition,
    /// Chain position warning
    ChainPosition,
    /// Liquidation imminent warning
    LiquidationImminent,
    /// Statistics disclosure
    StatisticsDisclosure,
}

/// Warning modal content
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct WarningModal {
    /// Modal type
    pub modal_type: WarningModalType,
    
    /// Title
    pub title: String,
    
    /// Main warning message
    pub message: String,
    
    /// Statistics to display
    pub statistics: Vec<StatisticDisplay>,
    
    /// Action buttons
    pub actions: Vec<ModalAction>,
    
    /// Severity level
    pub severity: ModalSeverity,
    
    /// Dismissible
    pub dismissible: bool,
    
    /// Requires acknowledgment
    pub requires_acknowledgment: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StatisticDisplay {
    pub label: String,
    pub value: String,
    pub highlight: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ModalAction {
    pub label: String,
    pub action_type: ActionType,
    pub primary: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum ActionType {
    Proceed,
    Cancel,
    LearnMore,
    TakeQuiz,
    ReduceLeverage,
    AcceptExtremRisk,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum ModalSeverity {
    Info,
    Warning,
    Danger,
    Critical,
}

/// User modal acknowledgment tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ModalAcknowledgment {
    /// User pubkey
    pub user: Pubkey,
    
    /// Modal type acknowledged
    pub modal_type: WarningModalType,
    
    /// Timestamp of acknowledgment
    pub acknowledged_at: i64,
    
    /// Version of modal acknowledged
    pub modal_version: u16,
    
    /// User checked "Don't show again"
    pub dont_show_again: bool,
}

impl WarningModal {
    /// Create high leverage warning modal
    pub fn high_leverage_warning(leverage: u8) -> Self {
        Self {
            modal_type: WarningModalType::HighLeverage,
            title: format!("⚠️ Extreme Risk: {}x Leverage", leverage),
            message: "You are about to use extremely high leverage. This significantly increases your risk of total loss.".to_string(),
            statistics: vec![
                StatisticDisplay {
                    label: "Loss probability".to_string(),
                    value: "80% of users lose money long-term".to_string(),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Liquidation risk".to_string(),
                    value: format!("{}% price move = total loss", 100.0 / leverage as f64),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Comparison".to_string(),
                    value: "Worse odds than casino blackjack (49% win rate)".to_string(),
                    highlight: false,
                },
            ],
            actions: vec![
                ModalAction {
                    label: "Reduce Leverage".to_string(),
                    action_type: ActionType::ReduceLeverage,
                    primary: true,
                },
                ModalAction {
                    label: "I Understand the Risks".to_string(),
                    action_type: ActionType::Proceed,
                    primary: false,
                },
            ],
            severity: ModalSeverity::Critical,
            dismissible: false,
            requires_acknowledgment: true,
        }
    }
    
    /// Create extreme 500x leverage warning modal
    pub fn extreme_leverage_500x_warning() -> Self {
        Self {
            modal_type: WarningModalType::ExtremeLeverage500x,
            title: "🚨 EXTREME DANGER: 500x Leverage".to_string(),
            message: "Risk: 100% loss on -0.2%, OK?".to_string(),
            statistics: vec![
                StatisticDisplay {
                    label: "Total loss threshold".to_string(),
                    value: "0.2% price movement".to_string(),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Liquidation probability".to_string(),
                    value: "EXTREMELY HIGH - This is essentially gambling".to_string(),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Recommended alternative".to_string(),
                    value: "Consider 10-50x leverage instead".to_string(),
                    highlight: false,
                },
            ],
            actions: vec![
                ModalAction {
                    label: "CANCEL - Too Risky".to_string(),
                    action_type: ActionType::Cancel,
                    primary: true,
                },
                ModalAction {
                    label: "I Accept Total Loss Risk".to_string(),
                    action_type: ActionType::AcceptExtremRisk,
                    primary: false,
                },
            ],
            severity: ModalSeverity::Critical,
            dismissible: false,
            requires_acknowledgment: true,
        }
    }
    
    /// Create statistics disclosure modal
    pub fn statistics_disclosure() -> Self {
        Self {
            modal_type: WarningModalType::StatisticsDisclosure,
            title: "📊 Platform Statistics Disclosure".to_string(),
            message: "Transparency about user outcomes on our platform:".to_string(),
            statistics: vec![
                StatisticDisplay {
                    label: "Long-term profitable users".to_string(),
                    value: "Only 20%".to_string(),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Average user return".to_string(),
                    value: "-4.53% (negative)".to_string(),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Short-term winners".to_string(),
                    value: "78% (but most lose long-term)".to_string(),
                    highlight: false,
                },
                StatisticDisplay {
                    label: "Skill vs luck".to_string(),
                    value: "Like casino with skill edge".to_string(),
                    highlight: false,
                },
            ],
            actions: vec![
                ModalAction {
                    label: "View Educational Resources".to_string(),
                    action_type: ActionType::LearnMore,
                    primary: true,
                },
                ModalAction {
                    label: "I Understand".to_string(),
                    action_type: ActionType::Proceed,
                    primary: false,
                },
            ],
            severity: ModalSeverity::Warning,
            dismissible: false,
            requires_acknowledgment: true,
        }
    }
    
    /// Create first time user warning
    pub fn first_time_warning() -> Self {
        Self {
            modal_type: WarningModalType::FirstTimeUser,
            title: "👋 Welcome! Important Information".to_string(),
            message: "Before you start trading, please understand the risks:".to_string(),
            statistics: vec![
                StatisticDisplay {
                    label: "New user loss rate".to_string(),
                    value: "90% lose money in first month".to_string(),
                    highlight: true,
                },
                StatisticDisplay {
                    label: "Recommended starting leverage".to_string(),
                    value: "10x or less".to_string(),
                    highlight: false,
                },
                StatisticDisplay {
                    label: "Education completion bonus".to_string(),
                    value: "+50% success rate".to_string(),
                    highlight: false,
                },
            ],
            actions: vec![
                ModalAction {
                    label: "Start Tutorial".to_string(),
                    action_type: ActionType::LearnMore,
                    primary: true,
                },
                ModalAction {
                    label: "Take Risk Quiz".to_string(),
                    action_type: ActionType::TakeQuiz,
                    primary: false,
                },
                ModalAction {
                    label: "Skip (Not Recommended)".to_string(),
                    action_type: ActionType::Proceed,
                    primary: false,
                },
            ],
            severity: ModalSeverity::Info,
            dismissible: false,
            requires_acknowledgment: true,
        }
    }
    
    /// Check if modal should be shown
    pub fn should_show(
        modal_type: &WarningModalType,
        user_state: &UserModalState,
    ) -> bool {
        // Always show critical warnings
        if matches!(modal_type, WarningModalType::LiquidationImminent) {
            return true;
        }
        
        // Check if user has acknowledged and selected "don't show again"
        if let Some(ack) = user_state.acknowledgments.iter()
            .find(|a| a.modal_type == *modal_type && a.dont_show_again) {
            // Still show if modal version has been updated
            return ack.modal_version < Self::current_version(modal_type);
        }
        
        // Show based on conditions
        match modal_type {
            WarningModalType::FirstTimeUser => user_state.trades_count == 0,
            WarningModalType::StatisticsDisclosure => user_state.last_disclosure_shown
                .map(|t| Clock::get().unwrap().unix_timestamp - t > 30 * 24 * 60 * 60) // 30 days
                .unwrap_or(true),
            _ => true,
        }
    }
    
    /// Get current modal version
    fn current_version(modal_type: &WarningModalType) -> u16 {
        match modal_type {
            WarningModalType::HighLeverage => 2,
            WarningModalType::ExtremeLeverage500x => 1,
            WarningModalType::FirstTimeUser => 1,
            WarningModalType::LargePosition => 1,
            WarningModalType::ChainPosition => 1,
            WarningModalType::LiquidationImminent => 1,
            WarningModalType::StatisticsDisclosure => 2,
        }
    }
}

/// User modal state tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UserModalState {
    /// User pubkey
    pub user: Pubkey,
    
    /// Modal acknowledgments
    pub acknowledgments: Vec<ModalAcknowledgment>,
    
    /// Total trades count
    pub trades_count: u64,
    
    /// Last statistics disclosure shown
    pub last_disclosure_shown: Option<i64>,
    
    /// Has completed tutorial
    pub tutorial_completed: bool,
}

impl UserModalState {
    pub const SIZE: usize = 32 + // user
        4 + 1024 + // acknowledgments vector (max ~10 entries)
        8 + // trades_count
        9 + // last_disclosure_shown
        1; // tutorial_completed
        
    pub fn new(user: Pubkey) -> Self {
        Self {
            user,
            acknowledgments: Vec::new(),
            trades_count: 0,
            last_disclosure_shown: None,
            tutorial_completed: false,
        }
    }
    
    /// Record modal acknowledgment
    pub fn acknowledge_modal(
        &mut self,
        modal_type: WarningModalType,
        dont_show_again: bool,
    ) -> ProgramResult {
        let acknowledgment = ModalAcknowledgment {
            user: self.user,
            modal_type: modal_type.clone(),
            acknowledged_at: Clock::get()?.unix_timestamp,
            modal_version: WarningModal::current_version(&modal_type),
            dont_show_again,
        };
        
        // Remove old acknowledgment if exists
        self.acknowledgments.retain(|a| a.modal_type != modal_type);
        
        // Add new acknowledgment
        self.acknowledgments.push(acknowledgment);
        
        // Update disclosure timestamp if applicable
        if modal_type == WarningModalType::StatisticsDisclosure {
            self.last_disclosure_shown = Some(Clock::get()?.unix_timestamp);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_high_leverage_warning() {
        let modal = WarningModal::high_leverage_warning(100);
        assert_eq!(modal.modal_type, WarningModalType::HighLeverage);
        assert_eq!(modal.severity, ModalSeverity::Critical);
        assert!(!modal.dismissible);
        assert!(modal.requires_acknowledgment);
        assert_eq!(modal.statistics.len(), 3);
    }
    
    #[test]
    fn test_statistics_disclosure() {
        let modal = WarningModal::statistics_disclosure();
        assert_eq!(modal.statistics[0].value, "Only 20%");
        assert_eq!(modal.statistics[1].value, "-4.53% (negative)");
    }
}