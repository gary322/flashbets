//! Oracle Validation Module
//!
//! TWAP validation and multi-source consensus for manipulation resistance

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;

/// Number of slots for TWAP calculation (5-10 slots = 2-4 seconds)
pub const TWAP_WINDOW_SLOTS: usize = 10;

/// Minimum number of oracle sources required for consensus
pub const MIN_ORACLE_SOURCES: usize = 3;

/// Maximum acceptable deviation between sources (1%)
pub const MAX_SOURCE_DEVIATION: f64 = 0.01;

/// Maximum acceptable TWAP deviation from spot (2%)
pub const MAX_TWAP_DEVIATION: f64 = 0.02;

/// Alpha parameter for exponential weighted moving average
pub const EWMA_ALPHA: f64 = 0.9;

/// Historical price data for TWAP calculation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceHistory {
    /// Ring buffer of historical probabilities
    pub prices: [f64; TWAP_WINDOW_SLOTS],
    /// Ring buffer of timestamps (slots)
    pub timestamps: [u64; TWAP_WINDOW_SLOTS],
    /// Current index in ring buffer
    pub index: usize,
    /// Whether buffer is full
    pub is_full: bool,
}

impl PriceHistory {
    pub fn new() -> Self {
        Self {
            prices: [0.0; TWAP_WINDOW_SLOTS],
            timestamps: [0; TWAP_WINDOW_SLOTS],
            index: 0,
            is_full: false,
        }
    }

    /// Add new price to history
    pub fn add_price(&mut self, price: f64, slot: u64) {
        self.prices[self.index] = price;
        self.timestamps[self.index] = slot;
        
        self.index = (self.index + 1) % TWAP_WINDOW_SLOTS;
        if self.index == 0 {
            self.is_full = true;
        }
    }

    /// Calculate time-weighted average price
    pub fn calculate_twap(&self) -> f64 {
        let count = if self.is_full { TWAP_WINDOW_SLOTS } else { self.index };
        if count == 0 {
            return 0.0;
        }

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for i in 0..count {
            let idx = if self.is_full {
                (self.index + i) % TWAP_WINDOW_SLOTS
            } else {
                i
            };

            // Weight by time (more recent = higher weight)
            let weight = (i + 1) as f64;
            weighted_sum += self.prices[idx] * weight;
            weight_sum += weight;
        }

        weighted_sum / weight_sum
    }

    /// Calculate exponentially weighted moving average
    pub fn calculate_ewma(&self) -> f64 {
        let count = if self.is_full { TWAP_WINDOW_SLOTS } else { self.index };
        if count == 0 {
            return 0.0;
        }

        let mut ewma = self.prices[0];
        
        for i in 1..count {
            let idx = if self.is_full {
                (self.index + i) % TWAP_WINDOW_SLOTS
            } else {
                i
            };
            
            ewma = EWMA_ALPHA * self.prices[idx] + (1.0 - EWMA_ALPHA) * ewma;
        }

        ewma
    }
}

/// Multi-source oracle aggregator
pub struct OracleValidator;

impl OracleValidator {
    /// Validate oracle data with TWAP
    pub fn validate_with_twap(
        current_prob: f64,
        price_history: &PriceHistory,
    ) -> Result<bool, ProgramError> {
        let twap = price_history.calculate_twap();
        
        // Check deviation from TWAP
        let deviation = (current_prob - twap).abs();
        if deviation > MAX_TWAP_DEVIATION {
            msg!("Price deviates too much from TWAP: {} vs {}", current_prob, twap);
            return Ok(false);
        }

        Ok(true)
    }

    /// Validate consensus from multiple oracle sources
    pub fn validate_multi_source(
        oracle_sources: &[f64],
    ) -> Result<(bool, f64), ProgramError> {
        if oracle_sources.len() < MIN_ORACLE_SOURCES {
            msg!("Insufficient oracle sources: {}", oracle_sources.len());
            return Ok((false, 0.0));
        }

        // Calculate median as consensus value
        let mut sorted = oracle_sources.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[sorted.len() / 2];

        // Check if sources agree within threshold
        let mut consensus_count = 0;
        for &price in oracle_sources {
            if (price - median).abs() <= MAX_SOURCE_DEVIATION {
                consensus_count += 1;
            }
        }

        // Require at least MIN_ORACLE_SOURCES to agree
        let has_consensus = consensus_count >= MIN_ORACLE_SOURCES;
        
        if !has_consensus {
            msg!("No consensus among oracle sources");
        }

        Ok((has_consensus, median))
    }

    /// Check for oracle manipulation attempts
    pub fn check_manipulation(
        current_prob: f64,
        last_prob: f64,
        max_jump: f64,
    ) -> Result<bool, ProgramError> {
        let jump = (current_prob - last_prob).abs();
        
        if jump > max_jump {
            msg!("Potential manipulation detected: {} jump", jump);
            return Ok(true);
        }

        Ok(false)
    }

    /// Validate oracle freshness
    pub fn validate_freshness(
        last_update_slot: u64,
        max_staleness_slots: u64,
    ) -> Result<bool, ProgramError> {
        let clock = Clock::get()?;
        let slots_elapsed = clock.slot.saturating_sub(last_update_slot);
        
        if slots_elapsed > max_staleness_slots {
            msg!("Oracle data is stale: {} slots old", slots_elapsed);
            return Ok(false);
        }

        Ok(true)
    }

    /// Combined validation with all checks
    pub fn validate_comprehensive(
        current_prob: f64,
        oracle_sources: &[f64],
        price_history: &PriceHistory,
        last_update_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Check freshness
        if !Self::validate_freshness(last_update_slot, 2)? {
            return Ok(false);
        }

        // Check TWAP deviation
        if !Self::validate_with_twap(current_prob, price_history)? {
            return Ok(false);
        }

        // Check multi-source consensus
        let (has_consensus, _) = Self::validate_multi_source(oracle_sources)?;
        if !has_consensus {
            return Ok(false);
        }

        // Check for manipulation
        let twap = price_history.calculate_twap();
        if Self::check_manipulation(current_prob, twap, 0.2)? {
            return Ok(false);
        }

        Ok(true)
    }

    /// Handle early resolution detection
    pub fn detect_early_resolution(
        current_prob: f64,
        last_prob: f64,
    ) -> Result<bool, ProgramError> {
        // Clamp probability to avoid extremes
        let clamped = current_prob.max(0.01).min(0.99);
        
        // Check for large jump indicating potential resolution
        let jump = (clamped - last_prob).abs();
        
        if jump > 0.2 {
            msg!("Early resolution detected: {} jump", jump);
            
            // Check if probability is near extremes
            if clamped < 0.05 || clamped > 0.95 {
                msg!("Probability near extreme: {}", clamped);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Calculate deviation factor for cascade protection
    pub fn calculate_deviation_factor(
        price_deviation: f64,
        dev_threshold: f64,
    ) -> f64 {
        // dev_factor = max(0.05, 1 - (dev / dev_threshold))
        let factor = 1.0 - (price_deviation / dev_threshold);
        factor.max(0.05)
    }
}

/// Oracle aggregation result
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,
    /// Consensus probability value
    pub consensus_value: f64,
    /// TWAP value
    pub twap_value: f64,
    /// EWMA value
    pub ewma_value: f64,
    /// Number of agreeing sources
    pub consensus_count: usize,
    /// Deviation from TWAP
    pub twap_deviation: f64,
    /// Is early resolution detected
    pub early_resolution: bool,
}

impl ValidationResult {
    pub fn new(
        is_valid: bool,
        consensus_value: f64,
        twap_value: f64,
        ewma_value: f64,
    ) -> Self {
        Self {
            is_valid,
            consensus_value,
            twap_value,
            ewma_value,
            consensus_count: 0,
            twap_deviation: (consensus_value - twap_value).abs(),
            early_resolution: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twap_calculation() {
        let mut history = PriceHistory::new();
        
        // Add prices
        history.add_price(0.5, 100);
        history.add_price(0.51, 101);
        history.add_price(0.52, 102);
        history.add_price(0.51, 103);
        history.add_price(0.50, 104);
        
        let twap = history.calculate_twap();
        assert!(twap > 0.5 && twap < 0.52);
    }

    #[test]
    fn test_multi_source_consensus() {
        let sources = vec![0.50, 0.51, 0.505, 0.49, 0.51];
        let (has_consensus, median) = OracleValidator::validate_multi_source(&sources).unwrap();
        
        assert!(has_consensus);
        assert!((median - 0.505).abs() < 0.001);
    }

    #[test]
    fn test_manipulation_detection() {
        assert!(OracleValidator::check_manipulation(0.5, 0.9, 0.2).unwrap());
        assert!(!OracleValidator::check_manipulation(0.5, 0.52, 0.2).unwrap());
    }

    #[test]
    fn test_ewma_calculation() {
        let mut history = PriceHistory::new();
        
        history.add_price(0.5, 100);
        history.add_price(0.6, 101);
        history.add_price(0.55, 102);
        
        let ewma = history.calculate_ewma();
        // EWMA should weight recent values more
        assert!(ewma > 0.52 && ewma < 0.58);
    }
}