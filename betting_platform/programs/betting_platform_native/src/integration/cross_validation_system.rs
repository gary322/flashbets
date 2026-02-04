//! Cross-Validation System
//!
//! Implements comprehensive data validation across sources:
//! - Compare Polymarket data with backup oracles
//! - Flag discrepancies and anomalies
//! - Generate detailed validation reports
//! - Track data quality metrics
//!
//! Per specification: Production-grade cross-validation

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, VecDeque};

use crate::{
    error::BettingPlatformError,
    integration::{
        polymarket_api_types::PolymarketMarketResponse,
        oracle_coordinator::OracleSource,
    },
    events::{emit_event, EventType},
};

/// Validation thresholds
pub const PRICE_DEVIATION_THRESHOLD_BPS: u64 = 500; // 5%
pub const VOLUME_DEVIATION_THRESHOLD_BPS: u64 = 1000; // 10%
pub const TIMESTAMP_DRIFT_SECONDS: i64 = 300; // 5 minutes
pub const CONFIDENCE_THRESHOLD: u64 = 80; // 80% confidence required

/// Cross-validation result
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ValidationResult {
    pub market_id: [u8; 16],
    pub timestamp: i64,
    pub primary_source: OracleSource,
    pub comparison_source: OracleSource,
    pub validation_status: ValidationStatus,
    pub discrepancies: Vec<Discrepancy>,
    pub confidence_score: u64,
    pub action_required: ActionRequired,
}

impl ValidationResult {
    pub const SIZE: usize = 512;

    /// Create new validation result
    pub fn new(
        market_id: [u8; 16],
        timestamp: i64,
        primary: OracleSource,
        comparison: OracleSource,
    ) -> Self {
        Self {
            market_id,
            timestamp,
            primary_source: primary,
            comparison_source: comparison,
            validation_status: ValidationStatus::Pending,
            discrepancies: Vec::new(),
            confidence_score: 100,
            action_required: ActionRequired::None,
        }
    }

    /// Add discrepancy
    pub fn add_discrepancy(&mut self, discrepancy: Discrepancy) {
        // Update confidence based on severity before moving discrepancy
        let penalty = match discrepancy.severity() {
            Severity::Critical => 50,
            Severity::High => 30,
            Severity::Medium => 20,
            Severity::Low => 10,
        };
        
        self.discrepancies.push(discrepancy);
        self.confidence_score = self.confidence_score.saturating_sub(penalty);
        
        // Update action required
        if self.confidence_score < CONFIDENCE_THRESHOLD {
            self.action_required = ActionRequired::ManualReview;
        }
    }

    /// Finalize validation
    pub fn finalize(&mut self) {
        self.validation_status = if self.discrepancies.is_empty() {
            ValidationStatus::Passed
        } else if self.confidence_score < CONFIDENCE_THRESHOLD {
            ValidationStatus::Failed
        } else {
            ValidationStatus::PassedWithWarnings
        };
    }
}

/// Validation status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum ValidationStatus {
    Pending,
    Passed,
    PassedWithWarnings,
    Failed,
    Skipped,
}

/// Discrepancy types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum Discrepancy {
    PriceDeviation {
        primary_price: u64,
        comparison_price: u64,
        deviation_bps: u64,
    },
    VolumeDeviation {
        primary_volume: u64,
        comparison_volume: u64,
        deviation_bps: u64,
    },
    OutcomeMismatch {
        primary_outcomes: Vec<String>,
        comparison_outcomes: Vec<String>,
    },
    TimestampDrift {
        primary_timestamp: i64,
        comparison_timestamp: i64,
        drift_seconds: i64,
    },
    StatusMismatch {
        primary_status: String,
        comparison_status: String,
    },
    MissingData {
        source: OracleSource,
        field: String,
    },
}

impl Discrepancy {
    /// Get severity of discrepancy
    pub fn severity(&self) -> Severity {
        match self {
            Discrepancy::PriceDeviation { deviation_bps, .. } => {
                if *deviation_bps > 2000 { // > 20%
                    Severity::Critical
                } else if *deviation_bps > 1000 { // > 10%
                    Severity::High
                } else if *deviation_bps > 500 { // > 5%
                    Severity::Medium
                } else {
                    Severity::Low
                }
            }
            Discrepancy::OutcomeMismatch { .. } => Severity::Critical,
            Discrepancy::StatusMismatch { .. } => Severity::High,
            Discrepancy::VolumeDeviation { deviation_bps, .. } => {
                if *deviation_bps > 5000 { // > 50%
                    Severity::High
                } else if *deviation_bps > 2000 { // > 20%
                    Severity::Medium
                } else {
                    Severity::Low
                }
            }
            Discrepancy::TimestampDrift { drift_seconds, .. } => {
                if *drift_seconds > 3600 { // > 1 hour
                    Severity::High
                } else if *drift_seconds > 600 { // > 10 minutes
                    Severity::Medium
                } else {
                    Severity::Low
                }
            }
            Discrepancy::MissingData { .. } => Severity::Medium,
        }
    }
}

/// Discrepancy severity
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, PartialOrd)]
pub enum Severity {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Action required
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum ActionRequired {
    None,
    LogOnly,
    Alert,
    ManualReview,
    HaltTrading,
}

/// Cross-validation engine
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CrossValidator {
    pub validation_history: VecDeque<ValidationResult>,
    pub source_reliability: HashMap<OracleSource, SourceReliability>,
    pub alert_thresholds: AlertThresholds,
    pub total_validations: u64,
    pub failed_validations: u64,
}

impl CrossValidator {
    pub const SIZE: usize = 1024 * 32; // 32KB
    pub const MAX_HISTORY: usize = 1000;

    pub fn new() -> Self {
        Self {
            validation_history: VecDeque::new(),
            source_reliability: Self::init_reliability_scores(),
            alert_thresholds: AlertThresholds::default(),
            total_validations: 0,
            failed_validations: 0,
        }
    }

    /// Initialize reliability scores
    fn init_reliability_scores() -> HashMap<OracleSource, SourceReliability> {
        let mut scores = HashMap::new();
        
        scores.insert(OracleSource::Polymarket, SourceReliability::new(100));
        scores.insert(OracleSource::Pyth, SourceReliability::new(90));
        scores.insert(OracleSource::PythNetwork, SourceReliability::new(90));
        scores.insert(OracleSource::Chainlink, SourceReliability::new(90));
        scores.insert(OracleSource::InternalCache, SourceReliability::new(70));
        
        scores
    }

    /// Validate market data across sources
    pub fn validate_market(
        &mut self,
        primary_data: &MarketData,
        comparison_data: &MarketData,
        current_timestamp: i64,
    ) -> Result<ValidationResult, ProgramError> {
        let mut result = ValidationResult::new(
            primary_data.market_id,
            current_timestamp,
            primary_data.source.clone(),
            comparison_data.source.clone(),
        );

        // Validate prices
        self.validate_prices(primary_data, comparison_data, &mut result)?;

        // Validate volumes
        self.validate_volumes(primary_data, comparison_data, &mut result)?;

        // Validate outcomes
        self.validate_outcomes(primary_data, comparison_data, &mut result)?;

        // Validate timestamps
        self.validate_timestamps(primary_data, comparison_data, &mut result)?;

        // Validate status
        self.validate_status(primary_data, comparison_data, &mut result)?;

        // Finalize result
        result.finalize();

        // Update statistics
        self.total_validations += 1;
        if result.validation_status == ValidationStatus::Failed {
            self.failed_validations += 1;
        }

        // Update source reliability
        self.update_reliability(&result);

        // Store in history
        self.add_to_history(result.clone());

        // Check if alerts needed
        self.check_alerts(&result)?;

        Ok(result)
    }

    /// Validate price data
    fn validate_prices(
        &self,
        primary: &MarketData,
        comparison: &MarketData,
        result: &mut ValidationResult,
    ) -> Result<(), ProgramError> {
        for (outcome, primary_price) in &primary.outcome_prices {
            if let Some(comparison_price) = comparison.outcome_prices.get(outcome) {
                let deviation = calculate_deviation(*primary_price, *comparison_price);
                
                if deviation > PRICE_DEVIATION_THRESHOLD_BPS {
                    result.add_discrepancy(Discrepancy::PriceDeviation {
                        primary_price: *primary_price,
                        comparison_price: *comparison_price,
                        deviation_bps: deviation,
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate volume data
    fn validate_volumes(
        &self,
        primary: &MarketData,
        comparison: &MarketData,
        result: &mut ValidationResult,
    ) -> Result<(), ProgramError> {
        let deviation = calculate_deviation(primary.total_volume, comparison.total_volume);
        
        if deviation > VOLUME_DEVIATION_THRESHOLD_BPS {
            result.add_discrepancy(Discrepancy::VolumeDeviation {
                primary_volume: primary.total_volume,
                comparison_volume: comparison.total_volume,
                deviation_bps: deviation,
            });
        }

        Ok(())
    }

    /// Validate outcomes match
    fn validate_outcomes(
        &self,
        primary: &MarketData,
        comparison: &MarketData,
        result: &mut ValidationResult,
    ) -> Result<(), ProgramError> {
        let primary_outcomes: Vec<String> = primary.outcome_prices.keys().cloned().collect();
        let comparison_outcomes: Vec<String> = comparison.outcome_prices.keys().cloned().collect();

        if primary_outcomes != comparison_outcomes {
            result.add_discrepancy(Discrepancy::OutcomeMismatch {
                primary_outcomes,
                comparison_outcomes,
            });
        }

        Ok(())
    }

    /// Validate timestamps
    fn validate_timestamps(
        &self,
        primary: &MarketData,
        comparison: &MarketData,
        result: &mut ValidationResult,
    ) -> Result<(), ProgramError> {
        let drift = (primary.last_update - comparison.last_update).abs();
        
        if drift > TIMESTAMP_DRIFT_SECONDS {
            result.add_discrepancy(Discrepancy::TimestampDrift {
                primary_timestamp: primary.last_update,
                comparison_timestamp: comparison.last_update,
                drift_seconds: drift,
            });
        }

        Ok(())
    }

    /// Validate market status
    fn validate_status(
        &self,
        primary: &MarketData,
        comparison: &MarketData,
        result: &mut ValidationResult,
    ) -> Result<(), ProgramError> {
        if primary.status != comparison.status {
            result.add_discrepancy(Discrepancy::StatusMismatch {
                primary_status: primary.status.clone(),
                comparison_status: comparison.status.clone(),
            });
        }

        Ok(())
    }

    /// Update source reliability based on validation results
    fn update_reliability(&mut self, result: &ValidationResult) {
        // Decrease reliability for sources with discrepancies
        for discrepancy in &result.discrepancies {
            let penalty = match discrepancy.severity() {
                Severity::Critical => 10,
                Severity::High => 5,
                Severity::Medium => 2,
                Severity::Low => 1,
            };

            // Apply penalty to the source that differs from Polymarket
            if result.primary_source != OracleSource::Polymarket {
                if let Some(reliability) = self.source_reliability.get_mut(&result.primary_source) {
                    reliability.decrease_score(penalty);
                }
            }
            if result.comparison_source != OracleSource::Polymarket {
                if let Some(reliability) = self.source_reliability.get_mut(&result.comparison_source) {
                    reliability.decrease_score(penalty);
                }
            }
        }

        // Increase reliability for successful validations
        if result.validation_status == ValidationStatus::Passed {
            if let Some(reliability) = self.source_reliability.get_mut(&result.comparison_source) {
                reliability.increase_score(1);
            }
        }
    }

    /// Add result to history
    fn add_to_history(&mut self, result: ValidationResult) {
        self.validation_history.push_back(result);
        
        // Maintain size limit
        while self.validation_history.len() > Self::MAX_HISTORY {
            self.validation_history.pop_front();
        }
    }

    /// Check if alerts need to be triggered
    fn check_alerts(&self, result: &ValidationResult) -> Result<(), ProgramError> {
        // Check critical discrepancies
        let critical_count = result.discrepancies
            .iter()
            .filter(|d| d.severity() == Severity::Critical)
            .count();

        if critical_count > 0 {
            msg!("ALERT: {} critical discrepancies found for market {:?}", 
                critical_count, result.market_id);
        }

        // Check confidence threshold
        if result.confidence_score < self.alert_thresholds.min_confidence {
            msg!("ALERT: Low confidence score {} for market {:?}", 
                result.confidence_score, result.market_id);
        }

        // Check failure rate
        let failure_rate = if self.total_validations > 0 {
            (self.failed_validations * 100) / self.total_validations
        } else {
            0
        };

        if failure_rate > self.alert_thresholds.max_failure_rate {
            msg!("ALERT: High validation failure rate: {}%", failure_rate);
        }

        Ok(())
    }

    /// Generate validation report
    pub fn generate_report(&self, time_window: i64) -> ValidationReport {
        let current_time = Clock::get().unwrap().unix_timestamp;
        let cutoff_time = current_time - time_window;

        let recent_validations: Vec<&ValidationResult> = self.validation_history
            .iter()
            .filter(|v| v.timestamp >= cutoff_time)
            .collect();

        let total = recent_validations.len() as u64;
        let passed = recent_validations.iter()
            .filter(|v| v.validation_status == ValidationStatus::Passed)
            .count() as u64;
        let failed = recent_validations.iter()
            .filter(|v| v.validation_status == ValidationStatus::Failed)
            .count() as u64;

        // Count discrepancies by type
        let mut discrepancy_counts = HashMap::new();
        for validation in &recent_validations {
            for discrepancy in &validation.discrepancies {
                let key = match discrepancy {
                    Discrepancy::PriceDeviation { .. } => "PriceDeviation",
                    Discrepancy::VolumeDeviation { .. } => "VolumeDeviation",
                    Discrepancy::OutcomeMismatch { .. } => "OutcomeMismatch",
                    Discrepancy::TimestampDrift { .. } => "TimestampDrift",
                    Discrepancy::StatusMismatch { .. } => "StatusMismatch",
                    Discrepancy::MissingData { .. } => "MissingData",
                };
                *discrepancy_counts.entry(key.to_string()).or_insert(0) += 1;
            }
        }

        ValidationReport {
            timestamp: current_time,
            time_window,
            total_validations: total,
            passed_validations: passed,
            failed_validations: failed,
            success_rate: if total > 0 { (passed * 100) / total } else { 100 },
            discrepancy_counts,
            source_reliability: self.source_reliability.clone(),
            recommendations: self.generate_recommendations(&recent_validations),
        }
    }

    /// Generate recommendations based on validation results
    fn generate_recommendations(&self, validations: &[&ValidationResult]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check for consistent failures
        let failure_rate = validations.iter()
            .filter(|v| v.validation_status == ValidationStatus::Failed)
            .count() as f64 / validations.len().max(1) as f64;

        if failure_rate > 0.2 {
            recommendations.push(
                "High failure rate detected. Consider reviewing oracle configurations.".to_string()
            );
        }

        // Check for specific discrepancy patterns
        let price_deviations = validations.iter()
            .flat_map(|v| &v.discrepancies)
            .filter(|d| matches!(d, Discrepancy::PriceDeviation { .. }))
            .count();

        if price_deviations > validations.len() / 2 {
            recommendations.push(
                "Frequent price deviations detected. Verify price feed accuracy.".to_string()
            );
        }

        // Check source reliability
        for (source, reliability) in &self.source_reliability {
            if reliability.current_score < 70 {
                recommendations.push(format!(
                    "{:?} reliability below threshold ({}). Consider fallback options.",
                    source, reliability.current_score
                ));
            }
        }

        recommendations
    }
}

/// Market data for validation
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MarketData {
    pub market_id: [u8; 16],
    pub source: OracleSource,
    pub outcome_prices: HashMap<String, u64>,
    pub total_volume: u64,
    pub status: String,
    pub last_update: i64,
}

/// Source reliability tracking
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct SourceReliability {
    pub base_score: u64,
    pub current_score: u64,
    pub total_validations: u64,
    pub successful_validations: u64,
    pub last_update: i64,
}

impl SourceReliability {
    pub fn new(base_score: u64) -> Self {
        Self {
            base_score,
            current_score: base_score,
            total_validations: 0,
            successful_validations: 0,
            last_update: 0,
        }
    }

    pub fn increase_score(&mut self, amount: u64) {
        self.current_score = (self.current_score + amount).min(100);
        self.successful_validations += 1;
        self.total_validations += 1;
    }

    pub fn decrease_score(&mut self, amount: u64) {
        self.current_score = self.current_score.saturating_sub(amount);
        self.total_validations += 1;
    }

    pub fn get_success_rate(&self) -> u64 {
        if self.total_validations == 0 {
            100
        } else {
            (self.successful_validations * 100) / self.total_validations
        }
    }
}

/// Alert thresholds
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AlertThresholds {
    pub min_confidence: u64,
    pub max_failure_rate: u64,
    pub max_price_deviation: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            min_confidence: 70,
            max_failure_rate: 20, // 20%
            max_price_deviation: 1000, // 10%
        }
    }
}

/// Validation report
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ValidationReport {
    pub timestamp: i64,
    pub time_window: i64,
    pub total_validations: u64,
    pub passed_validations: u64,
    pub failed_validations: u64,
    pub success_rate: u64,
    pub discrepancy_counts: HashMap<String, u32>,
    pub source_reliability: HashMap<OracleSource, SourceReliability>,
    pub recommendations: Vec<String>,
}

/// Calculate deviation in basis points
fn calculate_deviation(value1: u64, value2: u64) -> u64 {
    if value1 == 0 || value2 == 0 {
        return 10000; // 100% deviation
    }

    let diff = if value1 > value2 {
        value1 - value2
    } else {
        value2 - value1
    };

    (diff * 10000) / value1.max(value2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_deviation_calculation() {
        assert_eq!(calculate_deviation(100, 105), 500); // 5%
        assert_eq!(calculate_deviation(100, 110), 1000); // 10%
        assert_eq!(calculate_deviation(100, 120), 2000); // 20%
        assert_eq!(calculate_deviation(0, 100), 10000); // 100%
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::new(
            [0u8; 16],
            100,
            OracleSource::Polymarket,
            OracleSource::PythNetwork,
        );

        // Add minor discrepancy
        result.add_discrepancy(Discrepancy::PriceDeviation {
            primary_price: 100,
            comparison_price: 103,
            deviation_bps: 300,
        });

        assert_eq!(result.confidence_score, 90); // -10 for low severity

        // Add critical discrepancy
        result.add_discrepancy(Discrepancy::OutcomeMismatch {
            primary_outcomes: vec!["YES".to_string()],
            comparison_outcomes: vec!["NO".to_string()],
        });

        assert_eq!(result.confidence_score, 40); // -50 for critical

        result.finalize();
        assert_eq!(result.validation_status, ValidationStatus::Failed);
        assert_eq!(result.action_required, ActionRequired::ManualReview);
    }

    #[test]
    fn test_cross_validator() {
        let mut validator = CrossValidator::new();

        let primary = MarketData {
            market_id: [0u8; 16],
            source: OracleSource::Polymarket,
            outcome_prices: vec![("YES".to_string(), 6000)].into_iter().collect(),
            total_volume: 100000,
            status: "ACTIVE".to_string(),
            last_update: 100,
        };

        let comparison = MarketData {
            market_id: [0u8; 16],
            source: OracleSource::PythNetwork,
            outcome_prices: vec![("YES".to_string(), 6300)].into_iter().collect(),
            total_volume: 95000,
            status: "ACTIVE".to_string(),
            last_update: 100,
        };

        let result = validator.validate_market(&primary, &comparison, 100).unwrap();
        
        assert_eq!(result.discrepancies.len(), 1); // Price deviation
        assert_eq!(result.validation_status, ValidationStatus::PassedWithWarnings);
    }
}