//! Alert system for monitoring
//!
//! Configurable alerts for coverage, deviation, and congestion

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;
use crate::math::U64F64;

/// Alert configuration account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AlertConfiguration {
    pub enabled: bool,
    pub last_update_slot: u64,
    
    // Coverage alerts (as per CLAUDE.md)
    pub coverage_warning_threshold: U64F64,    // Default: 1.5
    pub coverage_critical_threshold: U64F64,   // Default: 1.0 (CLAUDE.md: coverage < 1 is critical)
    
    // API deviation alerts (as per CLAUDE.md)
    pub api_deviation_warning_pct: u8,         // Default: 3%
    pub api_deviation_critical_pct: u8,        // Default: 5% (CLAUDE.md: >5% deviation is critical)
    
    // Network congestion alerts
    pub congestion_tps_threshold: u32,         // Default: 2500
    pub congestion_cu_threshold: u32,          // Default: 1.2M per block
    
    // Polymarket outage
    pub polymarket_timeout_slots: u64,         // Default: 750 (5 min)
    
    // Alert destinations
    pub alert_pubkeys: Vec<Pubkey>,            // Keepers to notify
    pub webhook_enabled: bool,
    
    // Active alerts
    pub active_alerts: Vec<Alert>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Alert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub triggered_slot: u64,
    pub message: String,
    pub metric_value: u64,
    pub threshold_value: u64,
    pub acknowledged: bool,
    pub acknowledged_by: Option<Pubkey>,
    pub resolved_slot: Option<u64>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum AlertType {
    // Coverage alerts
    LowCoverage,
    CriticalCoverage,
    
    // API alerts
    APIDeviation,
    APITimeout,
    
    // Network alerts
    NetworkCongestion,
    HighCUUsage,
    
    // Service alerts
    PolymarketOutage,
    KeeperNetworkDegraded,
    PriceFeedStale,
    
    // Performance alerts
    LowSuccessRate,
    HighLatency,
    
    // Security alerts
    AnomalousActivity,
    RapidPriceMovement,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Alert manager for the system
pub struct AlertManager;

impl AlertManager {
    /// Check and trigger alerts based on system state
    pub fn check_alerts(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        coverage: U64F64,
        api_deviation_pct: u8,
        current_tps: u32,
        block_cu_usage: u32,
    ) -> ProgramResult {
        // Account layout:
        // 0. Alert configuration account (mut)
        // 1. System health account
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let config_account = &accounts[0];
        let _health_account = &accounts[1];
        let clock = Clock::get()?;
        
        // Deserialize configuration
        let mut config_data = config_account.try_borrow_mut_data()?;
        let mut config = AlertConfiguration::try_from_slice(&config_data)?;
        
        if !config.enabled {
            return Ok(());
        }
        
        // Copy threshold values to avoid borrow conflicts
        let coverage_critical_threshold = config.coverage_critical_threshold;
        let coverage_warning_threshold = config.coverage_warning_threshold;
        let api_deviation_critical_pct = config.api_deviation_critical_pct;
        let api_deviation_warning_pct = config.api_deviation_warning_pct;
        let congestion_tps_threshold = config.congestion_tps_threshold;
        let congestion_cu_threshold = config.congestion_cu_threshold;
        
        // Check coverage alerts
        if coverage < coverage_critical_threshold {
            Self::trigger_alert(
                &mut config,
                AlertType::CriticalCoverage,
                AlertSeverity::Critical,
                "Coverage below critical threshold",
                coverage.to_num(),
                coverage_critical_threshold.to_num(),
                clock.slot,
            )?;
        } else if coverage < coverage_warning_threshold {
            Self::trigger_alert(
                &mut config,
                AlertType::LowCoverage,
                AlertSeverity::Warning,
                "Coverage below warning threshold",
                coverage.to_num(),
                coverage_warning_threshold.to_num(),
                clock.slot,
            )?;
        }
        
        // Check API deviation
        if api_deviation_pct >= api_deviation_critical_pct {
            Self::trigger_alert(
                &mut config,
                AlertType::APIDeviation,
                AlertSeverity::Critical,
                "API price deviation exceeds critical threshold",
                api_deviation_pct as u64,
                api_deviation_critical_pct as u64,
                clock.slot,
            )?;
        } else if api_deviation_pct >= api_deviation_warning_pct {
            Self::trigger_alert(
                &mut config,
                AlertType::APIDeviation,
                AlertSeverity::Warning,
                "API price deviation exceeds warning threshold",
                api_deviation_pct as u64,
                api_deviation_warning_pct as u64,
                clock.slot,
            )?;
        }
        
        // Check network congestion
        if current_tps > congestion_tps_threshold {
            Self::trigger_alert(
                &mut config,
                AlertType::NetworkCongestion,
                AlertSeverity::Warning,
                "Network TPS exceeds congestion threshold",
                current_tps as u64,
                congestion_tps_threshold as u64,
                clock.slot,
            )?;
        }
        
        if block_cu_usage > congestion_cu_threshold {
            Self::trigger_alert(
                &mut config,
                AlertType::HighCUUsage,
                AlertSeverity::Warning,
                "Block CU usage exceeds threshold",
                block_cu_usage as u64,
                congestion_cu_threshold as u64,
                clock.slot,
            )?;
        }
        
        // Clean up old resolved alerts (keep last 20)
        Self::cleanup_old_alerts(&mut config, clock.slot)?;
        
        config.last_update_slot = clock.slot;
        
        // Serialize updated configuration
        config.serialize(&mut *config_data)?;
        
        Ok(())
    }
    
    /// Trigger a new alert
    fn trigger_alert(
        config: &mut AlertConfiguration,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: &str,
        metric_value: u64,
        threshold_value: u64,
        current_slot: u64,
    ) -> ProgramResult {
        // Check if similar alert already exists and is unresolved
        let existing = config.active_alerts.iter()
            .find(|a| a.alert_type == alert_type && a.resolved_slot.is_none());
        
        if existing.is_some() {
            return Ok(()); // Don't duplicate alerts
        }
        
        let alert = Alert {
            alert_type,
            severity,
            triggered_slot: current_slot,
            message: message.to_string(),
            metric_value,
            threshold_value,
            acknowledged: false,
            acknowledged_by: None,
            resolved_slot: None,
        };
        
        // Add to active alerts
        config.active_alerts.push(alert.clone());
        
        // Emit event
        msg!("AlertTriggered - type: {:?}, severity: {:?}, message: {}, value: {}, threshold: {}", alert_type, severity, message, metric_value, threshold_value);
        
        // Route to keepers if critical
        if severity >= AlertSeverity::Critical {
            Self::notify_keepers(config, &alert)?;
        }
        
        Ok(())
    }
    
    /// Acknowledge an alert
    pub fn acknowledge_alert(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        alert_index: usize,
    ) -> ProgramResult {
        // Account layout:
        // 0. Alert configuration account (mut)
        // 1. Keeper account (signer)
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let config_account = &accounts[0];
        let keeper_account = &accounts[1];
        let _clock = Clock::get()?;
        
        if !keeper_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize configuration
        let mut config_data = config_account.try_borrow_mut_data()?;
        let mut config = AlertConfiguration::try_from_slice(&config_data)?;
        
        // Validate alert index
        if alert_index >= config.active_alerts.len() {
            return Err(BettingPlatformError::InvalidAlertIndex.into());
        }
        
        // Acknowledge alert
        config.active_alerts[alert_index].acknowledged = true;
        config.active_alerts[alert_index].acknowledged_by = Some(*keeper_account.key);
        
        // Serialize updated configuration
        config.serialize(&mut *config_data)?;
        
        Ok(())
    }
    
    /// Resolve an alert
    pub fn resolve_alert(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        alert_index: usize,
    ) -> ProgramResult {
        // Account layout:
        // 0. Alert configuration account (mut)
        // 1. Authority account (signer)
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let config_account = &accounts[0];
        let authority_account = &accounts[1];
        let clock = Clock::get()?;
        
        if !authority_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize configuration
        let mut config_data = config_account.try_borrow_mut_data()?;
        let mut config = AlertConfiguration::try_from_slice(&config_data)?;
        
        // Validate alert index
        if alert_index >= config.active_alerts.len() {
            return Err(BettingPlatformError::InvalidAlertIndex.into());
        }
        
        // Resolve alert
        config.active_alerts[alert_index].resolved_slot = Some(clock.slot);
        
        msg!("AlertResolved - type: {:?}, duration_slots: {}", config.active_alerts[alert_index].alert_type, clock.slot - config.active_alerts[alert_index].triggered_slot);
        
        // Serialize updated configuration
        config.serialize(&mut *config_data)?;
        
        Ok(())
    }
    
    /// Notify keepers of critical alert
    fn notify_keepers(config: &AlertConfiguration, alert: &Alert) -> ProgramResult {
        msg!(
            "CRITICAL ALERT: {:?} - {} (notifying {} keepers)",
            alert.alert_type,
            alert.message,
            config.alert_pubkeys.len()
        );
        
        // In production, this would trigger CPI calls or events
        // that keepers are subscribed to
        
        Ok(())
    }
    
    /// Clean up old resolved alerts
    fn cleanup_old_alerts(config: &mut AlertConfiguration, current_slot: u64) -> ProgramResult {
        // Keep unresolved alerts and recent resolved alerts
        config.active_alerts.retain(|alert| {
            alert.resolved_slot.is_none() ||
            alert.resolved_slot.unwrap() + 7200 > current_slot // Keep for ~1 hour
        });
        
        // If still too many, keep only last 20
        if config.active_alerts.len() > 20 {
            let start = config.active_alerts.len() - 20;
            config.active_alerts = config.active_alerts[start..].to_vec();
        }
        
        Ok(())
    }
}

// Helper functions for alert analysis
impl AlertConfiguration {
    /// Get current alert summary
    pub fn get_alert_summary(&self) -> AlertSummary {
        let unresolved_count = self.active_alerts.iter()
            .filter(|a| a.resolved_slot.is_none())
            .count();
        
        let critical_count = self.active_alerts.iter()
            .filter(|a| a.resolved_slot.is_none() && a.severity >= AlertSeverity::Critical)
            .count();
        
        let acknowledged_count = self.active_alerts.iter()
            .filter(|a| a.acknowledged && a.resolved_slot.is_none())
            .count();
        
        AlertSummary {
            total_active: unresolved_count,
            critical_active: critical_count,
            acknowledged: acknowledged_count,
            most_severe: self.get_most_severe_alert(),
        }
    }
    
    /// Get most severe unresolved alert
    fn get_most_severe_alert(&self) -> Option<AlertSeverity> {
        self.active_alerts.iter()
            .filter(|a| a.resolved_slot.is_none())
            .map(|a| a.severity)
            .max()
    }
}

#[derive(Debug, Clone)]
pub struct AlertSummary {
    pub total_active: usize,
    pub critical_active: usize,
    pub acknowledged: usize,
    pub most_severe: Option<AlertSeverity>,
}

// Size calculation for Borsh
impl Alert {
    pub const MAX_MESSAGE_LEN: usize = 128;
    
    pub const SIZE: usize = 1 + // alert_type
        1 + // severity
        8 + // triggered_slot
        4 + Self::MAX_MESSAGE_LEN + // message (length + data)
        8 + // metric_value
        8 + // threshold_value
        1 + // acknowledged
        1 + 32 + // acknowledged_by Option<Pubkey>
        1 + 8; // resolved_slot Option<u64>
}

// CLAUDE.md specified thresholds
pub const COVERAGE_CRITICAL_THRESHOLD: f64 = 1.0;     // Coverage < 1 is critical
pub const API_DEVIATION_CRITICAL_PCT: u8 = 5;         // >5% deviation is critical
pub const POLYMARKET_OUTAGE_SLOTS: u64 = 750;         // 5 minutes (750 slots @ 400ms)

impl AlertConfiguration {
    /// Initialize with CLAUDE.md specified defaults
    pub fn initialize_defaults() -> Self {
        Self {
            enabled: true,
            last_update_slot: 0,
            coverage_warning_threshold: U64F64::from_num(1) + U64F64::from_num(1) / U64F64::from_num(2),
            coverage_critical_threshold: U64F64::from_num(1),
            api_deviation_warning_pct: 3,
            api_deviation_critical_pct: API_DEVIATION_CRITICAL_PCT,
            congestion_tps_threshold: 2500,
            congestion_cu_threshold: 1_200_000,
            polymarket_timeout_slots: POLYMARKET_OUTAGE_SLOTS,
            alert_pubkeys: Vec::new(),
            webhook_enabled: false,
            active_alerts: Vec::new(),
        }
    }
}