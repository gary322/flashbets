//! Sigma Calculation Module
//!
//! Calculates volatility (standard deviation) with exponential weighting

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    msg,
    program_error::ProgramError,
};

use crate::error::BettingPlatformError;

/// Number of slots in a day (assuming 2 slots per second)
pub const SLOTS_PER_DAY: usize = 216_000;

/// Compressed history size (216 samples = 1 per 1000 slots)
pub const COMPRESSED_HISTORY_SIZE: usize = 216;

/// Alpha parameter for EWMA of volatility
pub const SIGMA_EWMA_ALPHA: f64 = 0.9;

/// Minimum sigma value to prevent division by zero
pub const MIN_SIGMA: f64 = 0.01;

/// Maximum sigma value for safety
pub const MAX_SIGMA: f64 = 1.0;

/// Sigma calculator with compressed historical data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SigmaCalculator {
    /// Compressed ring buffer of historical probabilities
    pub history: [f64; COMPRESSED_HISTORY_SIZE],
    /// Current index in ring buffer
    pub index: usize,
    /// Whether buffer is full
    pub is_full: bool,
    /// Running mean
    pub mean: f64,
    /// Running variance
    pub variance: f64,
    /// Current sigma value
    pub sigma: f64,
    /// EWMA of sigma
    pub ewma_sigma: f64,
    /// Number of samples processed
    pub sample_count: u64,
}

impl SigmaCalculator {
    pub fn new() -> Self {
        Self {
            history: [0.0; COMPRESSED_HISTORY_SIZE],
            index: 0,
            is_full: false,
            mean: 0.5, // Start with neutral probability
            variance: 0.0,
            sigma: MIN_SIGMA,
            ewma_sigma: MIN_SIGMA,
            sample_count: 0,
        }
    }

    /// Add new probability sample
    pub fn add_sample(&mut self, prob: f64) -> Result<(), ProgramError> {
        // Validate probability
        if prob < 0.0 || prob > 1.0 {
            msg!("Invalid probability for sigma calculation: {}", prob);
            return Err(BettingPlatformError::InvalidProbability.into());
        }

        // Add to ring buffer
        self.history[self.index] = prob;
        self.index = (self.index + 1) % COMPRESSED_HISTORY_SIZE;
        
        if self.index == 0 {
            self.is_full = true;
        }

        self.sample_count += 1;

        // Recalculate statistics
        self.update_statistics()?;

        Ok(())
    }

    /// Update mean, variance, and sigma
    fn update_statistics(&mut self) -> Result<(), ProgramError> {
        let count = if self.is_full { 
            COMPRESSED_HISTORY_SIZE 
        } else { 
            self.index.max(1) 
        };

        // Calculate mean
        let mut sum = 0.0;
        for i in 0..count {
            sum += self.history[i];
        }
        self.mean = sum / count as f64;

        // Calculate variance
        let mut variance_sum = 0.0;
        for i in 0..count {
            let diff = self.history[i] - self.mean;
            variance_sum += diff * diff;
        }
        
        // Use n-1 for sample variance (Bessel's correction)
        self.variance = if count > 1 {
            variance_sum / (count - 1) as f64
        } else {
            0.0
        };

        // Calculate standard deviation
        self.sigma = self.variance.sqrt().max(MIN_SIGMA).min(MAX_SIGMA);

        // Update EWMA of sigma
        if self.sample_count == 1 {
            self.ewma_sigma = self.sigma;
        } else {
            self.ewma_sigma = SIGMA_EWMA_ALPHA * self.sigma + 
                            (1.0 - SIGMA_EWMA_ALPHA) * self.ewma_sigma;
        }

        Ok(())
    }

    /// Get current sigma value with EWMA smoothing
    pub fn get_sigma(&self) -> f64 {
        self.ewma_sigma
    }

    /// Get raw sigma without smoothing
    pub fn get_raw_sigma(&self) -> f64 {
        self.sigma
    }

    /// Calculate dynamic risk cap based on sigma
    pub fn calculate_risk_cap(&self) -> f64 {
        // risk_cap = 1 + 0.5 * sigma
        1.0 + 0.5 * self.get_sigma()
    }

    /// Calculate dynamic base risk for vault
    pub fn calculate_base_risk(&self) -> f64 {
        // base_risk = 0.2 + 0.1 * sigma
        0.2 + 0.1 * self.get_sigma()
    }

    /// Calculate volatility spike factor for liquidation
    pub fn calculate_vol_adjust(&self, vol_spike_threshold: f64) -> f64 {
        // vol_adjust = max(0.1, 1 - (sigma / threshold))
        let adjust = 1.0 - (self.get_sigma() / vol_spike_threshold);
        adjust.max(0.1)
    }

    /// Calculate over-collateralization buffer requirement
    pub fn calculate_buffer_requirement(&self, base_amount: f64) -> f64 {
        // buffer = amount * (1 + sigma * 1.5)
        base_amount * (1.0 + self.get_sigma() * 1.5)
    }

    /// Check if volatility is too high for new positions
    pub fn is_high_volatility(&self, threshold: f64) -> bool {
        self.get_sigma() > threshold
    }

    /// Get volatility percentile (for risk scoring)
    pub fn get_volatility_percentile(&self) -> f64 {
        // Map sigma to percentile (rough approximation)
        // sigma 0.01 = 0%, sigma 0.5 = 50%, sigma 1.0 = 100%
        (self.get_sigma() * 100.0).min(100.0)
    }

    /// Forecast next period volatility using GARCH-like approach
    pub fn forecast_volatility(&self, periods_ahead: usize) -> f64 {
        // Simple forecast: decay towards long-term mean
        let long_term_mean = 0.2; // Typical market volatility
        let decay_rate = 0.95_f64.powi(periods_ahead as i32);
        
        self.ewma_sigma * decay_rate + long_term_mean * (1.0 - decay_rate)
    }
}

/// Batch sigma calculator for multiple markets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BatchSigmaCalculator {
    /// Market ID to sigma calculator mapping
    pub calculators: Vec<(u128, SigmaCalculator)>,
    /// Global average sigma
    pub global_sigma: f64,
}

impl BatchSigmaCalculator {
    pub fn new() -> Self {
        Self {
            calculators: Vec::new(),
            global_sigma: MIN_SIGMA,
        }
    }

    /// Add or update market sigma
    pub fn update_market(&mut self, market_id: u128, prob: f64) -> Result<f64, ProgramError> {
        // Find or create calculator for market
        let calculator = self.calculators
            .iter_mut()
            .find(|(id, _)| *id == market_id)
            .map(|(_, calc)| calc)
            .or_else(|| {
                self.calculators.push((market_id, SigmaCalculator::new()));
                self.calculators.last_mut().map(|(_, calc)| calc)
            })
            .ok_or(BettingPlatformError::InvalidMarket)?;

        // Update with new sample
        calculator.add_sample(prob)?;

        // Update global sigma
        self.update_global_sigma();

        Ok(calculator.get_sigma())
    }

    /// Update global sigma as weighted average
    fn update_global_sigma(&mut self) {
        if self.calculators.is_empty() {
            self.global_sigma = MIN_SIGMA;
            return;
        }

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for (_, calc) in &self.calculators {
            // Weight by sample count (more data = more weight)
            let weight = calc.sample_count.min(COMPRESSED_HISTORY_SIZE as u64) as f64;
            weighted_sum += calc.get_sigma() * weight;
            weight_sum += weight;
        }

        self.global_sigma = if weight_sum > 0.0 {
            (weighted_sum / weight_sum).max(MIN_SIGMA).min(MAX_SIGMA)
        } else {
            MIN_SIGMA
        };
    }

    /// Get market-specific sigma
    pub fn get_market_sigma(&self, market_id: u128) -> Option<f64> {
        self.calculators
            .iter()
            .find(|(id, _)| *id == market_id)
            .map(|(_, calc)| calc.get_sigma())
    }

    /// Check if any market has high volatility
    pub fn any_high_volatility(&self, threshold: f64) -> bool {
        self.calculators
            .iter()
            .any(|(_, calc)| calc.is_high_volatility(threshold))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigma_calculation() {
        let mut calc = SigmaCalculator::new();
        
        // Add samples with some variance
        let samples = vec![0.5, 0.52, 0.48, 0.51, 0.49, 0.53, 0.47, 0.50];
        for sample in samples {
            calc.add_sample(sample).unwrap();
        }
        
        let sigma = calc.get_sigma();
        assert!(sigma > 0.01 && sigma < 0.1);
    }

    #[test]
    fn test_risk_cap_calculation() {
        let mut calc = SigmaCalculator::new();
        calc.sigma = 0.2;
        calc.ewma_sigma = 0.2;
        
        let risk_cap = calc.calculate_risk_cap();
        assert!((risk_cap - 1.1).abs() < 0.001);
    }

    #[test]
    fn test_buffer_requirement() {
        let mut calc = SigmaCalculator::new();
        calc.sigma = 0.4;
        calc.ewma_sigma = 0.4;
        
        let buffer = calc.calculate_buffer_requirement(1000.0);
        assert!((buffer - 1600.0).abs() < 0.001); // 1000 * (1 + 0.4 * 1.5)
    }

    #[test]
    fn test_volatility_bounds() {
        let mut calc = SigmaCalculator::new();
        
        // Test with extreme values
        for _ in 0..100 {
            calc.add_sample(0.0).unwrap();
        }
        assert!(calc.get_sigma() >= MIN_SIGMA);
        
        calc = SigmaCalculator::new();
        for i in 0..100 {
            calc.add_sample(if i % 2 == 0 { 0.0 } else { 1.0 }).unwrap();
        }
        assert!(calc.get_sigma() <= MAX_SIGMA);
    }

    #[test]
    fn test_batch_calculator() {
        let mut batch = BatchSigmaCalculator::new();
        
        // Update different markets
        batch.update_market(1, 0.5).unwrap();
        batch.update_market(1, 0.52).unwrap();
        batch.update_market(2, 0.8).unwrap();
        batch.update_market(2, 0.75).unwrap();
        
        assert!(batch.get_market_sigma(1).is_some());
        assert!(batch.get_market_sigma(2).is_some());
        assert!(batch.global_sigma > MIN_SIGMA);
    }
}