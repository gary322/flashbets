//! Oracle Coordinator for Secondary Data Sources
//!
//! While Polymarket is the primary oracle, this module handles:
//! - Temporary backup oracle switching during outages
//! - Data reconciliation when primary returns
//! - Configurable oracle priorities
//! - Health scoring for data sources
//!
//! Note: Per specification, Polymarket remains the sole truth source.
//! Backups are only used temporarily during confirmed outages.

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;

use crate::{
    error::BettingPlatformError,
    integration::{
        polymarket_api_types::InternalMarketData,
        polymarket_fallback_manager::FallbackManager,
    },
};

/// Oracle source type
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OracleSource {
    Polymarket,     // Primary - sole truth
    Pyth,          // Backup 1 - Pyth Network
    PythNetwork,    // Backup 1 (alias for compatibility)
    Chainlink,      // Backup 2
    InternalCache,  // Last resort
}

impl OracleSource {
    pub fn priority(&self) -> u8 {
        match self {
            OracleSource::Polymarket => 1,
            OracleSource::Pyth | OracleSource::PythNetwork => 2,
            OracleSource::Chainlink => 3,
            OracleSource::InternalCache => 4,
        }
    }
}

/// Oracle health score
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct OracleHealth {
    pub source: OracleSource,
    pub is_available: bool,
    pub last_success_timestamp: i64,
    pub consecutive_failures: u32,
    pub average_latency_ms: u64,
    pub data_quality_score: u8, // 0-100
    pub health_score: u8,       // 0-100
}

impl OracleHealth {
    pub fn new(source: OracleSource) -> Self {
        Self {
            source,
            is_available: true,
            last_success_timestamp: 0,
            consecutive_failures: 0,
            average_latency_ms: 0,
            data_quality_score: 100,
            health_score: 100,
        }
    }

    /// Update health score based on metrics
    pub fn update_health_score(&mut self) {
        let availability_score = if self.is_available { 40 } else { 0 };
        let failure_penalty = (self.consecutive_failures * 5).min(30);
        let latency_score = if self.average_latency_ms < 100 { 20 } 
                           else if self.average_latency_ms < 500 { 10 } 
                           else { 0 };
        
        self.health_score = (availability_score + self.data_quality_score / 2 + latency_score)
            .saturating_sub(failure_penalty as u8);
    }

    /// Record successful fetch
    pub fn record_success(&mut self, timestamp: i64, latency_ms: u64) {
        self.is_available = true;
        self.last_success_timestamp = timestamp;
        self.consecutive_failures = 0;
        
        // Update average latency (exponential moving average)
        self.average_latency_ms = (self.average_latency_ms * 9 + latency_ms) / 10;
        
        self.update_health_score();
    }

    /// Record failed fetch
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        
        if self.consecutive_failures >= 3 {
            self.is_available = false;
        }
        
        self.update_health_score();
    }
}

/// Data reconciliation record
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ReconciliationRecord {
    pub market_id: [u8; 16],
    pub primary_value: u64,
    pub backup_value: u64,
    pub difference_bps: u16,
    pub timestamp: i64,
    pub action_taken: ReconciliationAction,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum ReconciliationAction {
    UsedPrimary,
    UsedBackup,
    AveragedValues,
    HaltedMarket,
}

/// Oracle coordinator state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OracleCoordinator {
    pub primary_oracle: OracleHealth,
    pub backup_oracles: Vec<OracleHealth>,
    pub active_source: OracleSource,
    pub fallback_active: bool,
    pub fallback_start_timestamp: i64,
    pub reconciliation_pending: bool,
    pub total_reconciliations: u64,
    pub significant_discrepancies: u64,
}

impl OracleCoordinator {
    pub const SIZE: usize = 1024 * 8; // 8KB
    pub const MAX_BACKUP_DURATION_SECONDS: i64 = 1800; // 30 minutes max on backup
    pub const SIGNIFICANT_DISCREPANCY_BPS: u16 = 500; // 5% difference

    pub fn new() -> Self {
        Self {
            primary_oracle: OracleHealth::new(OracleSource::Polymarket),
            backup_oracles: vec![
                OracleHealth::new(OracleSource::PythNetwork),
                OracleHealth::new(OracleSource::Chainlink),
                OracleHealth::new(OracleSource::InternalCache),
            ],
            active_source: OracleSource::Polymarket,
            fallback_active: false,
            fallback_start_timestamp: 0,
            reconciliation_pending: false,
            total_reconciliations: 0,
            significant_discrepancies: 0,
        }
    }

    /// Check if should switch to backup oracle
    pub fn evaluate_oracle_switch(&mut self, current_timestamp: i64) -> Result<bool, ProgramError> {
        // Always prefer Polymarket if available
        if self.primary_oracle.is_available && self.primary_oracle.health_score > 20 {
            if self.fallback_active {
                // Primary is back, prepare for reconciliation
                self.reconciliation_pending = true;
                msg!("Primary oracle recovered, reconciliation pending");
            }
            return Ok(false);
        }

        // Primary is down, find best backup
        let best_backup = self.backup_oracles
            .iter()
            .filter(|o| o.is_available)
            .max_by_key(|o| o.health_score);

        if let Some(backup) = best_backup {
            if !self.fallback_active {
                self.fallback_active = true;
                self.fallback_start_timestamp = current_timestamp;
                self.active_source = backup.source.clone();
                msg!("Switching to backup oracle: {:?}", self.active_source);
            }
            Ok(true)
        } else {
            // No healthy oracles available
            Err(BettingPlatformError::NoHealthyOracles.into())
        }
    }

    /// Get current data with source info
    pub fn get_market_data(
        &mut self,
        market_id: [u8; 16],
        fallback_manager: &mut FallbackManager,
        current_slot: u64,
        current_timestamp: i64,
    ) -> Result<(InternalMarketData, OracleSource, bool), ProgramError> {
        // Check if we should switch oracles
        self.evaluate_oracle_switch(current_timestamp)?;

        // Check backup duration limit
        if self.fallback_active {
            let backup_duration = current_timestamp - self.fallback_start_timestamp;
            if backup_duration > Self::MAX_BACKUP_DURATION_SECONDS {
                // Force switch to cache
                self.active_source = OracleSource::InternalCache;
                msg!("Backup duration exceeded, using cache only");
            }
        }

        match self.active_source {
            OracleSource::Polymarket => {
                // Try primary with fallback
                match fallback_manager.fetch_with_fallback(market_id, current_slot, current_timestamp) {
                    Ok((data, is_stale)) => {
                        self.primary_oracle.record_success(current_timestamp, 50);
                        Ok((data, OracleSource::Polymarket, is_stale))
                    }
                    Err(e) => {
                        self.primary_oracle.record_failure();
                        Err(e)
                    }
                }
            }
            OracleSource::Pyth | OracleSource::PythNetwork => {
                // Simulate Pyth fetch
                self.fetch_from_pyth(market_id, current_timestamp)
            }
            OracleSource::Chainlink => {
                // Simulate Chainlink fetch
                self.fetch_from_chainlink(market_id, current_timestamp)
            }
            OracleSource::InternalCache => {
                // Use cache only
                fallback_manager.fetch_with_fallback(market_id, current_slot, current_timestamp)
                    .map(|(data, is_stale)| (data, OracleSource::InternalCache, is_stale))
            }
        }
    }

    /// Reconcile data when primary returns
    pub fn reconcile_data(
        &mut self,
        market_id: [u8; 16],
        primary_data: &InternalMarketData,
        backup_data: &InternalMarketData,
        current_timestamp: i64,
    ) -> Result<ReconciliationRecord, ProgramError> {
        let price_diff = (primary_data.yes_price_bps as i64 - backup_data.yes_price_bps as i64).abs();
        let difference_bps = (price_diff * 10000 / primary_data.yes_price_bps as i64) as u16;

        let action = if difference_bps > Self::SIGNIFICANT_DISCREPANCY_BPS {
            self.significant_discrepancies += 1;
            msg!("Significant discrepancy detected: {} bps", difference_bps);
            
            // For significant differences, halt market for manual review
            ReconciliationAction::HaltedMarket
        } else if difference_bps > 100 {
            // Minor difference, use primary
            ReconciliationAction::UsedPrimary
        } else {
            // Very close, safe to use primary
            ReconciliationAction::UsedPrimary
        };

        let record = ReconciliationRecord {
            market_id,
            primary_value: primary_data.yes_price_bps,
            backup_value: backup_data.yes_price_bps,
            difference_bps,
            timestamp: current_timestamp,
            action_taken: action,
        };

        self.total_reconciliations += 1;
        self.reconciliation_pending = false;
        self.fallback_active = false;
        self.active_source = OracleSource::Polymarket;

        Ok(record)
    }

    /// Simulate Pyth Network fetch
    fn fetch_from_pyth(
        &mut self,
        market_id: [u8; 16],
        timestamp: i64,
    ) -> Result<(InternalMarketData, OracleSource, bool), ProgramError> {
        // In production, would integrate with actual Pyth
        // For now, return simulated data
        let data = InternalMarketData {
            market_id,
            yes_price_bps: 5000, // Default 50%
            no_price_bps: 5000,
            volume_24h: 0,
            liquidity: 0,
            last_update_slot: 0,
            market_type: 0,
            status: 0,
            spread_bps: 0,
        };

        self.backup_oracles[0].record_success(timestamp, 30);
        Ok((data, OracleSource::Pyth, false))
    }

    /// Simulate Chainlink fetch
    fn fetch_from_chainlink(
        &mut self,
        market_id: [u8; 16],
        timestamp: i64,
    ) -> Result<(InternalMarketData, OracleSource, bool), ProgramError> {
        // In production, would integrate with actual Chainlink
        // For now, return simulated data
        let data = InternalMarketData {
            market_id,
            yes_price_bps: 5000, // Default 50%
            no_price_bps: 5000,
            volume_24h: 0,
            liquidity: 0,
            last_update_slot: 0,
            market_type: 0,
            status: 0,
            spread_bps: 0,
        };

        self.backup_oracles[1].record_success(timestamp, 40);
        Ok((data, OracleSource::Chainlink, false))
    }

    /// Get oracle statistics
    pub fn get_stats(&self) -> OracleStats {
        OracleStats {
            primary_health: self.primary_oracle.health_score,
            active_source: self.active_source.clone(),
            fallback_active: self.fallback_active,
            total_reconciliations: self.total_reconciliations,
            significant_discrepancies: self.significant_discrepancies,
            backup_duration: if self.fallback_active {
                Clock::get().unwrap().unix_timestamp - self.fallback_start_timestamp
            } else {
                0
            },
        }
    }
}

/// Oracle statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct OracleStats {
    pub primary_health: u8,
    pub active_source: OracleSource,
    pub fallback_active: bool,
    pub total_reconciliations: u64,
    pub significant_discrepancies: u64,
    pub backup_duration: i64,
}

/// Configurable oracle priorities
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct OraclePriorityConfig {
    pub priorities: Vec<(OracleSource, u8)>,
    pub auto_switch_enabled: bool,
    pub max_backup_duration_seconds: i64,
    pub reconciliation_threshold_bps: u16,
}

impl Default for OraclePriorityConfig {
    fn default() -> Self {
        Self {
            priorities: vec![
                (OracleSource::Polymarket, 1),
                (OracleSource::Pyth, 2),
                (OracleSource::Chainlink, 3),
                (OracleSource::InternalCache, 4),
            ],
            auto_switch_enabled: true,
            max_backup_duration_seconds: 1800,
            reconciliation_threshold_bps: 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oracle_health_scoring() {
        let mut health = OracleHealth::new(OracleSource::Polymarket);
        
        // Record success
        health.record_success(100, 50);
        assert_eq!(health.health_score, 100);
        assert_eq!(health.average_latency_ms, 50);
        
        // Record failures
        health.record_failure();
        health.record_failure();
        health.record_failure();
        assert!(!health.is_available);
        assert!(health.health_score < 100);
    }

    #[test]
    fn test_oracle_switching() {
        let mut coordinator = OracleCoordinator::new();
        
        // Primary healthy - no switch
        let should_switch = coordinator.evaluate_oracle_switch(100).unwrap();
        assert!(!should_switch);
        
        // Primary unhealthy - switch to backup
        coordinator.primary_oracle.is_available = false;
        coordinator.primary_oracle.health_score = 10;
        
        let should_switch = coordinator.evaluate_oracle_switch(100).unwrap();
        assert!(should_switch);
        assert!(coordinator.fallback_active);
    }

    #[test]
    fn test_reconciliation() {
        let mut coordinator = OracleCoordinator::new();
        
        let primary_data = InternalMarketData {
            market_id: [1u8; 16],
            yes_price_bps: 6000,
            no_price_bps: 4000,
            volume_24h: 1000000,
            liquidity: 500000,
            last_update_slot: 100,
            market_type: 0,
            status: 0,
            spread_bps: 0,
        };
        
        let mut backup_data = primary_data.clone();
        backup_data.yes_price_bps = 6100; // 1% difference
        
        let record = coordinator.reconcile_data(
            [1u8; 16],
            &primary_data,
            &backup_data,
            200
        ).unwrap();
        
        assert!(matches!(record.action_taken, ReconciliationAction::UsedPrimary));
        assert_eq!(record.difference_bps, 166); // ~1.66%
    }
}