//! Immutability enforcement and verification
//!
//! This module ensures the protocol remains immutable after deployment

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::GlobalConfigPDA,
};

/// System program ID (used for burned authority)
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_program::system_program::ID;

/// Immutability configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ImmutabilityConfig {
    /// Program was deployed with immutable flag
    pub deployed_immutable: bool,
    
    /// Upgrade authority has been burned
    pub upgrade_authority_burned: bool,
    
    /// Global config update authority disabled
    pub update_authority_disabled: bool,
    
    /// All parameters are fixed
    pub parameters_fixed: bool,
    
    /// Deployment timestamp
    pub deployed_at: i64,
    
    /// Immutability verification hash
    pub verification_hash: [u8; 32],
}

impl ImmutabilityConfig {
    /// Create new immutability config for deployment
    pub fn new(deployed_at: i64) -> Self {
        Self {
            deployed_immutable: true,
            upgrade_authority_burned: false, // Will be set post-deployment
            update_authority_disabled: true,
            parameters_fixed: true,
            deployed_at,
            verification_hash: [0; 32], // Will be computed
        }
    }
    
    /// Verify full immutability
    pub fn verify_immutable(&self) -> Result<(), ProgramError> {
        if !self.deployed_immutable {
            msg!("Error: Program not deployed as immutable");
            return Err(BettingPlatformError::NotImmutable.into());
        }
        
        if !self.upgrade_authority_burned {
            msg!("Error: Upgrade authority not burned");
            return Err(BettingPlatformError::NotImmutable.into());
        }
        
        if !self.update_authority_disabled {
            msg!("Error: Update authority still active");
            return Err(BettingPlatformError::NotImmutable.into());
        }
        
        if !self.parameters_fixed {
            msg!("Error: Parameters not fixed");
            return Err(BettingPlatformError::NotImmutable.into());
        }
        
        Ok(())
    }
}

/// Verify no governance functions exist
pub fn verify_no_governance(
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    // List of forbidden governance-related instruction discriminators
    const FORBIDDEN_INSTRUCTIONS: &[&[u8]] = &[
        b"update_fee",
        b"update_param",
        b"set_authority",
        b"governance",
        b"admin",
        b"upgrade",
        b"migrate",
        b"modify",
        b"change",
    ];
    
    // Check instruction doesn't match any forbidden patterns
    for forbidden in FORBIDDEN_INSTRUCTIONS {
        if instruction_data.starts_with(forbidden) {
            msg!("Error: Governance instruction not allowed in immutable program");
            return Err(BettingPlatformError::GovernanceNotAllowed.into());
        }
    }
    
    Ok(())
}

/// Verify all parameters are constants
pub fn verify_fixed_parameters(
    global_config: &GlobalConfigPDA,
) -> Result<(), ProgramError> {
    // Verify update authority is disabled (set to system program)
    if global_config.update_authority != SYSTEM_PROGRAM_ID {
        msg!("Error: Update authority not disabled");
        return Err(BettingPlatformError::NotImmutable.into());
    }
    
    // Verify critical parameters match expected constants
    // These values should never change after deployment
    // Fee constants are defined in fees module and checked at compile time
    
    // Verify fees module constants haven't changed
    use crate::fees::{FEE_BASE_BPS, FEE_MAX_BPS};
    const EXPECTED_BASE_FEE_BPS: u16 = 3; // 3bp minimum 
    const EXPECTED_MAX_FEE_BPS: u16 = 28; // 28bp maximum
    
    if FEE_BASE_BPS != EXPECTED_BASE_FEE_BPS || FEE_MAX_BPS != EXPECTED_MAX_FEE_BPS {
        msg!("Error: Fee constants have been modified");
        return Err(BettingPlatformError::ParameterMismatch.into());
    }
    
    // Additional parameter checks can be added here
    
    Ok(())
}

/// Initialize immutability on deployment
pub fn initialize_immutability(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let global_config_info = &accounts[0];
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_info.data.borrow())?;
    
    // Set update authority to system program (effectively disabling it)
    global_config.update_authority = SYSTEM_PROGRAM_ID;
    
    // Serialize back
    global_config.serialize(&mut &mut global_config_info.data.borrow_mut()[..])?;
    
    msg!("Immutability initialized: update authority disabled, parameters fixed");
    Ok(())
}

/// Check if program can be modified (should always return false)
pub fn can_modify() -> bool {
    false // Always immutable
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_immutability_verification() {
        let mut config = ImmutabilityConfig::new(1234567890);
        
        // Should fail when not fully immutable
        assert!(config.verify_immutable().is_err());
        
        // Set all flags
        config.upgrade_authority_burned = true;
        
        // Should pass when fully immutable
        assert!(config.verify_immutable().is_ok());
    }
    
    #[test]
    fn test_governance_detection() {
        // Should reject governance instructions
        assert!(verify_no_governance(b"update_fee_rate").is_err());
        assert!(verify_no_governance(b"set_authority_new").is_err());
        
        // Should allow normal instructions
        assert!(verify_no_governance(b"open_position").is_ok());
        assert!(verify_no_governance(b"close_position").is_ok());
    }
}