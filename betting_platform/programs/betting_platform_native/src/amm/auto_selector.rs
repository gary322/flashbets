//! AMM Auto-Selection Logic
//! 
//! Automatically selects the appropriate AMM type based on outcome count
//! Per specification:
//! - N=1 → LMSR
//! - N=2 → PM-AMM
//! - N>2 → PM-AMM or L2 based on market characteristics

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::{
    state::accounts::AMMType,
    error::BettingPlatformError,
};

/// Automatically select AMM type based on number of outcomes
pub fn select_amm_type(
    outcome_count: u8,
    outcome_type: Option<&str>,
    expiry_time: Option<i64>,
    current_time: i64,
) -> Result<AMMType, ProgramError> {
    msg!("Selecting AMM type for {} outcomes, type: {:?}", outcome_count, outcome_type);
    
    match outcome_count {
        0 => {
            msg!("Invalid outcome count: 0");
            Err(BettingPlatformError::InvalidOutcomeCount.into())
        },
        1 => {
            msg!("Selected LMSR for single outcome");
            Ok(AMMType::LMSR)
        },
        2 => {
            msg!("Selected PM-AMM for binary outcome");
            Ok(AMMType::PMAMM)
        },
        3..=64 => {
            // Check if this is a continuous outcome type
            if let Some(otype) = outcome_type {
                if otype == "range" || otype == "continuous" || otype == "distribution" {
                    msg!("Selected L2-norm AMM for continuous outcome type");
                    return Ok(AMMType::L2AMM);
                }
            }
            
            // Per specification: 2≤N≤64 → PM-AMM
            msg!("Selected PM-AMM for {} outcomes", outcome_count);
            Ok(AMMType::PMAMM)
        },
        65..=100 => {
            // Per specification: continuous → L2
            msg!("Selected L2-norm AMM for {} outcomes (>64)", outcome_count);
            Ok(AMMType::L2AMM)
        },
        _ => {
            msg!("Too many outcomes: {}", outcome_count);
            Err(BettingPlatformError::TooManyOutcomes.into())
        }
    }
}

/// Determine if L2-norm AMM should be used for multi-outcome markets
pub fn should_use_l2_norm(outcome_count: u8) -> bool {
    // Per specification:
    // - Use PM-AMM for 2≤N≤64
    // - Use L2 for >64 outcomes or continuous distributions
    
    outcome_count > 64
}

/// Get recommended liquidity parameter for AMM type
pub fn get_recommended_liquidity(amm_type: AMMType, outcome_count: u8) -> u64 {
    match amm_type {
        AMMType::LMSR => {
            // LMSR: b parameter scales with expected volume
            1_000_000_000 // 1000 USDC base
        },
        AMMType::PMAMM => {
            // PM-AMM: liquidity scales with outcome count
            500_000_000 * outcome_count as u64 // 500 USDC per outcome
        },
        AMMType::L2AMM => {
            // L2: k parameter for continuous distributions
            2_000_000_000 // 2000 USDC base
        },
        AMMType::Hybrid => {
            // Hybrid: base liquidity on optimal AMM type
            if outcome_count == 1 {
                1_000_000_000 // Use LMSR base
            } else if outcome_count <= 8 {
                500_000_000 * outcome_count as u64 // Use PM-AMM base
            } else {
                2_000_000_000 // Use L2 base
            }
        }
    }
}

/// Validate AMM selection for given parameters
pub fn validate_amm_selection(
    amm_type: AMMType,
    outcome_count: u8,
    liquidity: u64,
) -> Result<(), ProgramError> {
    // Check outcome count compatibility
    match (amm_type, outcome_count) {
        (AMMType::LMSR, 1) => {},
        (AMMType::PMAMM, 2..=64) => {},
        (AMMType::L2AMM, 2..=100) => {},
        (AMMType::Hybrid, 1..=100) => {}, // Hybrid supports all outcome counts
        _ => {
            msg!("Invalid AMM type {:?} for {} outcomes", amm_type, outcome_count);
            return Err(BettingPlatformError::InvalidAMMType.into());
        }
    }
    
    // Check minimum liquidity
    let min_liquidity = match amm_type {
        AMMType::LMSR => 100_000_000,     // 100 USDC min
        AMMType::PMAMM => 50_000_000,     // 50 USDC min
        AMMType::L2AMM => 200_000_000,   // 200 USDC min
        AMMType::Hybrid => 100_000_000,   // 100 USDC min (same as LMSR)
    };
    
    if liquidity < min_liquidity {
        msg!("Insufficient liquidity: {} < {}", liquidity, min_liquidity);
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }
    
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_amm_selection() {
        let current_time = 1000000i64;
        
        // Basic tests - Per specification: N=1→LMSR, 2≤N≤64→PM-AMM, >64→L2
        assert_eq!(select_amm_type(1, None, None, current_time).unwrap(), AMMType::LMSR);
        assert_eq!(select_amm_type(2, None, None, current_time).unwrap(), AMMType::PMAMM);
        assert_eq!(select_amm_type(5, None, None, current_time).unwrap(), AMMType::PMAMM);
        assert_eq!(select_amm_type(10, None, None, current_time).unwrap(), AMMType::PMAMM);
        assert_eq!(select_amm_type(64, None, None, current_time).unwrap(), AMMType::PMAMM);
        assert_eq!(select_amm_type(65, None, None, current_time).unwrap(), AMMType::L2AMM);
        assert_eq!(select_amm_type(100, None, None, current_time).unwrap(), AMMType::L2AMM);
        assert!(select_amm_type(0, None, None, current_time).is_err());
        assert!(select_amm_type(101, None, None, current_time).is_err());
        
        // Test continuous outcome type
        assert_eq!(select_amm_type(5, Some("range"), None, current_time).unwrap(), AMMType::L2AMM);
        assert_eq!(select_amm_type(5, Some("continuous"), None, current_time).unwrap(), AMMType::L2AMM);
        assert_eq!(select_amm_type(5, Some("distribution"), None, current_time).unwrap(), AMMType::L2AMM);
        
        // Test that expiry time doesn't affect selection (per specification)
        let near_expiry = current_time + 3600; // 1 hour away
        let far_expiry = current_time + 90000; // 25 hours away
        assert_eq!(select_amm_type(10, None, Some(near_expiry), current_time).unwrap(), AMMType::PMAMM);
        assert_eq!(select_amm_type(10, None, Some(far_expiry), current_time).unwrap(), AMMType::PMAMM);
        assert_eq!(select_amm_type(70, None, Some(near_expiry), current_time).unwrap(), AMMType::L2AMM);
        assert_eq!(select_amm_type(70, None, Some(far_expiry), current_time).unwrap(), AMMType::L2AMM);
    }
}