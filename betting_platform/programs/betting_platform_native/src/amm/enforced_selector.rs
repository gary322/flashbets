//! Enforced AMM Selection
//!
//! This module ensures AMM type selection follows specification rules
//! with no user override capability

use solana_program::{
    program_error::ProgramError,
    msg,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    state::accounts::AMMType,
    error::BettingPlatformError,
    amm::auto_selector::select_amm_type,
};

/// Market parameters for AMM selection
#[derive(Debug, Clone)]
pub struct MarketParams {
    pub outcome_count: u8,
    pub outcome_type: Option<String>,
    pub expiry_time: Option<i64>,
}

/// Enforce AMM selection based on specification rules
/// No user override is allowed - the AMM type is determined solely by market parameters
pub fn enforce_amm_selection(params: &MarketParams) -> Result<AMMType, ProgramError> {
    msg!("Enforcing AMM selection for market parameters");
    
    // Get current time
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    
    // Use auto-selector with market parameters
    let outcome_type_ref = params.outcome_type.as_deref();
    let selected_amm = select_amm_type(
        params.outcome_count,
        outcome_type_ref,
        params.expiry_time,
        current_time,
    )?;
    
    msg!("Enforced AMM selection: {:?}", selected_amm);
    Ok(selected_amm)
}

/// Validate that a requested AMM type matches the enforced selection
/// Returns error if user tries to override the automatic selection
pub fn validate_no_override(
    params: &MarketParams,
    requested_amm: AMMType,
) -> Result<(), ProgramError> {
    let enforced_amm = enforce_amm_selection(params)?;
    
    if requested_amm != enforced_amm {
        msg!(
            "AMM override attempt detected! Requested: {:?}, Enforced: {:?}",
            requested_amm,
            enforced_amm
        );
        return Err(BettingPlatformError::InvalidAMMType.into());
    }
    
    Ok(())
}

/// Create market with enforced AMM selection
/// This should be the only way to create markets, preventing any override
pub fn create_market_with_enforced_amm(
    market_id: u128,
    params: MarketParams,
    liquidity: u64,
) -> Result<(AMMType, u64), ProgramError> {
    // Enforce AMM selection
    let amm_type = enforce_amm_selection(&params)?;
    
    // Get recommended liquidity for the selected AMM
    use crate::amm::auto_selector::get_recommended_liquidity;
    let recommended_liquidity = get_recommended_liquidity(amm_type, params.outcome_count);
    
    // Use provided liquidity if sufficient, otherwise use recommended
    let final_liquidity = liquidity.max(recommended_liquidity);
    
    msg!(
        "Creating market {} with enforced AMM {:?} and liquidity {}",
        market_id,
        amm_type,
        final_liquidity
    );
    
    Ok((amm_type, final_liquidity))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_no_override_allowed() {
        // Test that override attempts are rejected
        let params = MarketParams {
            outcome_count: 1,
            outcome_type: None,
            expiry_time: None,
        };
        
        // Should select LMSR for single outcome
        let enforced = enforce_amm_selection(&params).unwrap();
        assert_eq!(enforced, AMMType::LMSR);
        
        // Trying to use PM-AMM should fail
        let result = validate_no_override(&params, AMMType::PMAMM);
        assert!(result.is_err());
        
        // Using LMSR should succeed
        let result = validate_no_override(&params, AMMType::LMSR);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_continuous_enforcement() {
        // Test continuous outcome type enforcement
        let params = MarketParams {
            outcome_count: 5,
            outcome_type: Some("continuous".to_string()),
            expiry_time: None,
        };
        
        // Should select L2-AMM for continuous
        let enforced = enforce_amm_selection(&params).unwrap();
        assert_eq!(enforced, AMMType::L2AMM);
    }
}