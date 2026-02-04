//! Leverage validation with risk quiz integration
//!
//! Ensures users have passed the mandatory risk quiz before using high leverage

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::BorshDeserialize;

use crate::{
    error::BettingPlatformError,
    risk_warnings::check_leverage_allowed,
    constants::{MAX_LEVERAGE, MAX_LEVERAGE_NO_QUIZ},
};

/// Validate leverage with risk quiz check
pub fn validate_leverage_with_risk_check(
    user: &Pubkey,
    requested_leverage: u8,
    max_system_leverage: u8,
    risk_quiz_account: Option<&AccountInfo>,
) -> Result<(), ProgramError> {
    // Basic leverage validation
    if requested_leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    if requested_leverage > max_system_leverage {
        return Err(BettingPlatformError::LeverageTooHigh.into());
    }
    
    // Check if leverage requires risk quiz
    if requested_leverage > MAX_LEVERAGE_NO_QUIZ {
        match risk_quiz_account {
            Some(quiz_account) => {
                // Check if user can use this leverage
                let allowed = check_leverage_allowed(
                    user,
                    requested_leverage,
                    &quiz_account.data.borrow(),
                )?;
                
                if !allowed {
                    return Err(BettingPlatformError::RiskQuizRequired.into());
                }
                
                // Additional warning for extreme leverage (>100x)
                if requested_leverage > 100 {
                    solana_program::msg!("⚠️ WARNING: Using {}x leverage - extreme risk of total loss!", requested_leverage);
                }
            }
            None => {
                // No quiz account provided but high leverage requested
                return Err(BettingPlatformError::RiskQuizRequired.into());
            }
        }
    }
    
    Ok(())
}

/// Get maximum allowed leverage for user
pub fn get_max_allowed_leverage(
    user: &Pubkey,
    risk_quiz_account: Option<&AccountInfo>,
) -> Result<u8, ProgramError> {
    match risk_quiz_account {
        Some(quiz_account) => {
            use crate::risk_warnings::leverage_quiz::RiskQuizState;
            let quiz_state = RiskQuizState::try_from_slice(&quiz_account.data.borrow())?;
            
            if quiz_state.user != *user {
                return Err(BettingPlatformError::InvalidAmount.into());
            }
            
            Ok(quiz_state.get_allowed_leverage())
        }
        None => {
            // No quiz completed, only basic leverage allowed
            Ok(MAX_LEVERAGE_NO_QUIZ)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_no_quiz_leverage_limit() {
        let user = Pubkey::new_unique();
        
        // Without quiz, only 10x allowed
        let result = validate_leverage_with_risk_check(
            &user,
            10,
            100,
            None,
        );
        assert!(result.is_ok());
        
        // 11x should fail without quiz
        let result = validate_leverage_with_risk_check(
            &user,
            11,
            100,
            None,
        );
        assert!(result.is_err());
    }
}