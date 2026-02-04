//! Fallback Mechanism for Oracle Failures
//!
//! Provides fallback to legacy leverage system when oracle fails

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{FusedMigrationFlags, LeverageTier},
    constants::*,
    oracle::MIN_SIGMA,
};

/// Fallback handler for oracle failures
pub struct FallbackHandler;

/// Fallback reason tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum FallbackReason {
    OracleStale,
    OracleHalted,
    InvalidProbability,
    InvalidSigma,
    NoConsensus,
    HighVolatility,
    CircuitBreaker,
    ManualOverride,
}

/// Fallback event for monitoring
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct FallbackEvent {
    pub slot: u64,
    pub reason: FallbackReason,
    pub market_id: u128,
    pub attempted_leverage: u16,
    pub fallback_leverage: u16,
    pub oracle_prob: Option<f64>,
    pub oracle_sigma: Option<f64>,
}

impl FallbackHandler {
    /// Check if should fallback to legacy system
    pub fn should_fallback(
        oracle_account: &AccountInfo,
        migration_flags: &FusedMigrationFlags,
    ) -> Result<(bool, Option<FallbackReason>), ProgramError> {
        // Check if fallback is allowed
        if !migration_flags.should_fallback() {
            return Ok((false, None));
        }
        
        // Check oracle data validity
        if oracle_account.data_is_empty() {
            msg!("Oracle account empty, triggering fallback");
            return Ok((true, Some(FallbackReason::OracleStale)));
        }
        
        // Try to parse oracle data
        let oracle_result = super::pyth_client::ProbabilityFeed::try_from_slice(
            &oracle_account.data.borrow()
        );
        
        match oracle_result {
            Ok(feed) => {
                // Check feed status
                if feed.status != super::pyth_client::FeedStatus::Trading {
                    msg!("Oracle feed halted, triggering fallback");
                    return Ok((true, Some(FallbackReason::OracleHalted)));
                }
                
                // Check data freshness
                let clock = Clock::get()?;
                let slots_elapsed = clock.slot.saturating_sub(feed.last_update_slot);
                
                if slots_elapsed > super::pyth_client::MAX_PROB_LATENCY_SLOTS {
                    msg!("Oracle data stale: {} slots old", slots_elapsed);
                    return Ok((true, Some(FallbackReason::OracleStale)));
                }
                
                // Check probability bounds
                if feed.prob < 0.0 || feed.prob > 1.0 {
                    msg!("Invalid probability: {}", feed.prob);
                    return Ok((true, Some(FallbackReason::InvalidProbability)));
                }
                
                // Check sigma bounds
                if feed.sigma < 0.0 || feed.sigma > 1.0 {
                    msg!("Invalid sigma: {}", feed.sigma);
                    return Ok((true, Some(FallbackReason::InvalidSigma)));
                }
                
                // Check for high volatility
                if feed.sigma > VOL_SPIKE_THRESHOLD {
                    msg!("High volatility detected: sigma {}", feed.sigma);
                    return Ok((true, Some(FallbackReason::HighVolatility)));
                }
                
                // Check consensus
                if feed.num_sources < super::validation::MIN_ORACLE_SOURCES {
                    msg!("Insufficient oracle sources: {}", feed.num_sources);
                    return Ok((true, Some(FallbackReason::NoConsensus)));
                }
                
                // All checks passed
                Ok((false, None))
            }
            Err(_) => {
                msg!("Failed to parse oracle data, triggering fallback");
                Ok((true, Some(FallbackReason::OracleStale)))
            }
        }
    }
    
    /// Calculate legacy leverage based on coverage
    pub fn calculate_legacy_leverage(
        coverage_ratio: f64,
        leverage_tiers: &[LeverageTier],
        n_positions: u32,
    ) -> Result<u16, ProgramError> {
        // Find applicable tier
        let mut max_leverage = 5u16; // Default minimum
        
        for tier in leverage_tiers {
            if n_positions <= tier.n {
                max_leverage = tier.max as u16;
                break;
            }
        }
        
        // Apply coverage-based adjustment
        let coverage_factor = if coverage_ratio > 1.0 {
            1.0
        } else if coverage_ratio > 0.5 {
            0.8
        } else if coverage_ratio > 0.25 {
            0.6
        } else {
            0.4
        };
        
        let adjusted_leverage = (max_leverage as f64 * coverage_factor) as u16;
        Ok(adjusted_leverage.max(1))
    }
    
    /// Record fallback event
    pub fn record_fallback(
        event: FallbackEvent,
        migration_flags: &mut FusedMigrationFlags,
    ) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
        migration_flags.trigger_fallback(clock.slot);
        
        msg!(
            "Fallback triggered at slot {} for market {} due to {:?}",
            event.slot,
            event.market_id,
            event.reason
        );
        
        Ok(())
    }
    
    /// Check if should re-enable fused system after fallback
    pub fn check_recovery(
        oracle_account: &AccountInfo,
        migration_flags: &FusedMigrationFlags,
        min_recovery_slots: u64,
    ) -> Result<bool, ProgramError> {
        // Don't recover if oracle-only mode
        if migration_flags.oracle_only {
            return Ok(false);
        }
        
        // Check if enough time has passed since last fallback
        let clock = Clock::get()?;
        let slots_since_fallback = clock.slot.saturating_sub(migration_flags.last_fallback_slot);
        
        if slots_since_fallback < min_recovery_slots {
            return Ok(false);
        }
        
        // Check if oracle is healthy now
        let (should_fallback, _) = Self::should_fallback(oracle_account, migration_flags)?;
        
        Ok(!should_fallback)
    }
    
    /// Execute leverage calculation with fallback
    pub fn calculate_leverage_with_fallback(
        oracle_account: &AccountInfo,
        migration_flags: &mut FusedMigrationFlags,
        coverage_ratio: f64,
        leverage_tiers: &[LeverageTier],
        n_positions: u32,
        market_id: u128,
    ) -> Result<(u16, bool), ProgramError> {
        // Check if should use fallback
        let (use_fallback, reason) = Self::should_fallback(oracle_account, migration_flags)?;
        
        if use_fallback {
            // Calculate legacy leverage
            let leverage = Self::calculate_legacy_leverage(
                coverage_ratio,
                leverage_tiers,
                n_positions,
            )?;
            
            // Record fallback event
            let clock = Clock::get()?;
            let event = FallbackEvent {
                slot: clock.slot,
                reason: reason.unwrap_or(FallbackReason::ManualOverride),
                market_id,
                attempted_leverage: 0, // Would have been fused leverage
                fallback_leverage: leverage,
                oracle_prob: None,
                oracle_sigma: None,
            };
            
            Self::record_fallback(event, migration_flags)?;
            
            Ok((leverage, true)) // true indicates fallback was used
        } else {
            // Use fused leverage calculation
            let feed = super::pyth_client::ProbabilityFeed::try_from_slice(
                &oracle_account.data.borrow()
            )?;
            
            // Calculate fused leverage
            let prob = feed.prob.max(PROB_MIN_CLAMP).min(PROB_MAX_CLAMP);
            let sigma = feed.sigma.max(MIN_SIGMA);
            let risk = prob * (1.0 - prob);
            
            let unified_scalar = (1.0 / sigma) * CAP_FUSED;
            let premium_factor = (risk / BASE_RISK) * CAP_VAULT;
            let total_scalar = (unified_scalar * premium_factor).min(1000.0);
            
            let leverage = (BASE_LEVERAGE as f64 * total_scalar / 100.0) as u16;
            let capped_leverage = leverage.min(MAX_FUSED_LEVERAGE as u16);
            
            Ok((capped_leverage, false)) // false indicates fused was used
        }
    }
}

/// Automatic fallback configuration
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct AutoFallbackConfig {
    /// Enable automatic fallback
    pub enabled: bool,
    
    /// Max consecutive failures before fallback
    pub max_failures: u32,
    
    /// Recovery period in slots
    pub recovery_slots: u64,
    
    /// Gradual recovery (increase fused percentage slowly)
    pub gradual_recovery: bool,
    
    /// Recovery increment percentage
    pub recovery_increment: u8,
}

impl AutoFallbackConfig {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_failures: 3,
            recovery_slots: 432, // ~3 minutes
            gradual_recovery: true,
            recovery_increment: 5, // 5% at a time
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fallback_detection() {
        // Test various fallback scenarios
        let mut flags = FusedMigrationFlags::new(Pubkey::default());
        flags.legacy_enabled = true;
        flags.parallel_mode = true;
        
        // Should allow fallback in parallel mode
        assert!(flags.should_fallback());
        
        // Should not allow fallback in oracle-only mode
        flags.oracle_only = true;
        flags.legacy_enabled = false;
        assert!(!flags.should_fallback());
    }
    
    #[test]
    fn test_legacy_leverage_calculation() {
        let tiers = vec![
            LeverageTier { n: 1, max: 100 },
            LeverageTier { n: 2, max: 70 },
            LeverageTier { n: 4, max: 25 },
        ];
        
        // Test with full coverage
        let leverage = FallbackHandler::calculate_legacy_leverage(
            1.5, // coverage > 1.0
            &tiers,
            1, // 1 position
        ).unwrap();
        assert_eq!(leverage, 100); // Full tier leverage
        
        // Test with low coverage
        let leverage = FallbackHandler::calculate_legacy_leverage(
            0.2, // coverage < 0.25
            &tiers,
            1,
        ).unwrap();
        assert_eq!(leverage, 40); // 100 * 0.4
    }
}