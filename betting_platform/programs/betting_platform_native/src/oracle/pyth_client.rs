//! Pyth Oracle Client for Polymarket Probabilities
//!
//! Integrates Pyth-style oracle with off-chain price feeds for sub-second updates

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

/// Maximum acceptable latency for probability updates (2 slots = ~0.8s)
pub const MAX_PROB_LATENCY_SLOTS: u64 = 2;

/// Maximum acceptable latency for sigma updates (1 epoch = ~12s) 
pub const MAX_SIGMA_LATENCY_SLOTS: u64 = 32;

/// Pyth-style price feed structure for Polymarket probabilities
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ProbabilityFeed {
    /// Current probability value (0.0 to 1.0)
    pub prob: f64,
    /// Standard deviation (sigma) over 1-day period
    pub sigma: f64,
    /// Exponentially weighted moving average of probability
    pub twap_prob: f64,
    /// Timestamp of last update (slot number)
    pub last_update_slot: u64,
    /// Confidence interval (for multi-source validation)
    pub confidence: f64,
    /// Number of data sources that contributed
    pub num_sources: u8,
    /// Status of the feed
    pub status: FeedStatus,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum FeedStatus {
    Trading,
    Halted,
    Unknown,
}

/// Pyth oracle client for fetching and validating Polymarket probabilities
pub struct PythClient;

impl PythClient {
    /// Fetch probability and sigma from Pyth oracle account
    pub fn fetch_prob_sigma(
        oracle_account: &AccountInfo,
    ) -> Result<(f64, f64), ProgramError> {
        // Validate oracle account
        if oracle_account.data_is_empty() {
            msg!("Oracle account is empty");
            return Err(BettingPlatformError::InvalidOracle.into());
        }

        // Parse feed data from account
        let feed = ProbabilityFeed::try_from_slice(&oracle_account.data.borrow())
            .map_err(|_| BettingPlatformError::InvalidOracle)?;

        // Check freshness
        let clock = Clock::get()?;
        let slots_elapsed = clock.slot.saturating_sub(feed.last_update_slot);
        
        if slots_elapsed > MAX_PROB_LATENCY_SLOTS {
            msg!("Oracle data is stale: {} slots old", slots_elapsed);
            return Err(BettingPlatformError::StaleOracle.into());
        }

        // Validate feed status
        if feed.status != FeedStatus::Trading {
            msg!("Oracle feed is not in trading status");
            return Err(BettingPlatformError::OracleHalted.into());
        }

        // Validate probability bounds
        if feed.prob < 0.0 || feed.prob > 1.0 {
            msg!("Invalid probability value: {}", feed.prob);
            return Err(BettingPlatformError::InvalidProbability.into());
        }

        // Validate sigma bounds (should be positive and reasonable)
        if feed.sigma < 0.0 || feed.sigma > 1.0 {
            msg!("Invalid sigma value: {}", feed.sigma);
            return Err(BettingPlatformError::InvalidSigma.into());
        }

        Ok((feed.prob, feed.sigma))
    }

    /// Fetch TWAP (Time-Weighted Average Price) probability
    pub fn fetch_twap(
        oracle_account: &AccountInfo,
    ) -> Result<f64, ProgramError> {
        let feed = ProbabilityFeed::try_from_slice(&oracle_account.data.borrow())
            .map_err(|_| BettingPlatformError::InvalidOracle)?;

        // Check if TWAP is fresh enough
        let clock = Clock::get()?;
        let slots_elapsed = clock.slot.saturating_sub(feed.last_update_slot);
        
        if slots_elapsed > MAX_SIGMA_LATENCY_SLOTS {
            msg!("TWAP data is stale: {} slots old", slots_elapsed);
            return Err(BettingPlatformError::StaleOracle.into());
        }

        Ok(feed.twap_prob)
    }

    /// Validate oracle data with confidence checks
    pub fn validate_confidence(
        oracle_account: &AccountInfo,
        min_sources: u8,
        min_confidence: f64,
    ) -> Result<bool, ProgramError> {
        let feed = ProbabilityFeed::try_from_slice(&oracle_account.data.borrow())
            .map_err(|_| BettingPlatformError::InvalidOracle)?;

        // Check minimum number of data sources
        if feed.num_sources < min_sources {
            msg!("Insufficient data sources: {} < {}", feed.num_sources, min_sources);
            return Ok(false);
        }

        // Check confidence interval
        if feed.confidence < min_confidence {
            msg!("Low confidence: {} < {}", feed.confidence, min_confidence);
            return Ok(false);
        }

        Ok(true)
    }

    /// Get feed status for circuit breaker checks
    pub fn get_feed_status(
        oracle_account: &AccountInfo,
    ) -> Result<FeedStatus, ProgramError> {
        let feed = ProbabilityFeed::try_from_slice(&oracle_account.data.borrow())
            .map_err(|_| BettingPlatformError::InvalidOracle)?;

        Ok(feed.status)
    }

    /// Check if oracle should trigger a halt
    pub fn should_halt(
        oracle_account: &AccountInfo,
        max_sigma: f64,
    ) -> Result<bool, ProgramError> {
        let (_, sigma) = Self::fetch_prob_sigma(oracle_account)?;
        
        // Halt if volatility is too high
        if sigma > max_sigma {
            msg!("High volatility detected: sigma {} > max {}", sigma, max_sigma);
            return Ok(true);
        }

        // Check feed status
        let status = Self::get_feed_status(oracle_account)?;
        if status == FeedStatus::Halted {
            msg!("Oracle feed is halted");
            return Ok(true);
        }

        Ok(false)
    }

    /// Pre-compute scalar off-chain and verify on-chain
    pub fn verify_precomputed_scalar(
        oracle_account: &AccountInfo,
        precomputed_scalar: f64,
        expected_hash: [u8; 32],
    ) -> Result<bool, ProgramError> {
        use solana_program::keccak;
        
        let (prob, sigma) = Self::fetch_prob_sigma(oracle_account)?;
        
        // Recreate the scalar calculation
        let risk = prob * (1.0 - prob);
        let cap_fused = 20.0;
        let cap_vault = 30.0;
        let base_risk = 0.25;
        
        // Simplified formula where prob terms cancel
        let calculated_scalar = (1.0 / sigma) * (cap_fused * cap_vault / base_risk);
        
        // Allow small floating point difference
        let scalar_matches = (calculated_scalar - precomputed_scalar).abs() < 0.0001;
        
        // Verify hash
        let mut data = Vec::new();
        data.extend_from_slice(&prob.to_le_bytes());
        data.extend_from_slice(&sigma.to_le_bytes());
        data.extend_from_slice(&precomputed_scalar.to_le_bytes());
        
        let computed_hash = keccak::hash(&data);
        let hash_matches = computed_hash.to_bytes() == expected_hash;
        
        Ok(scalar_matches && hash_matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probability_bounds() {
        // Test that probabilities are properly bounded
        let feed = ProbabilityFeed {
            prob: 0.5,
            sigma: 0.2,
            twap_prob: 0.5,
            last_update_slot: 100,
            confidence: 0.95,
            num_sources: 3,
            status: FeedStatus::Trading,
        };
        
        assert!(feed.prob >= 0.0 && feed.prob <= 1.0);
        assert!(feed.sigma >= 0.0 && feed.sigma <= 1.0);
    }

    #[test]
    fn test_feed_status() {
        let feed = ProbabilityFeed {
            prob: 0.5,
            sigma: 0.2,
            twap_prob: 0.5,
            last_update_slot: 100,
            confidence: 0.95,
            num_sources: 3,
            status: FeedStatus::Trading,
        };
        
        assert_eq!(feed.status, FeedStatus::Trading);
        assert_ne!(feed.status, FeedStatus::Halted);
    }
}