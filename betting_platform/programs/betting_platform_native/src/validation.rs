//! Additional validation utilities

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

/// Validate account is owned by the expected program
pub fn validate_program_owner(
    account: &AccountInfo,
    expected_owner: &Pubkey,
) -> Result<(), ProgramError> {
    if account.owner != expected_owner {
        return Err(BettingPlatformError::InvalidAccountOwner.into());
    }
    Ok(())
}

/// Validate account has expected discriminator
pub fn validate_discriminator(
    account_data: &[u8],
    expected: &[u8; 8],
) -> Result<(), ProgramError> {
    if account_data.len() < 8 {
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    
    if &account_data[..8] != expected {
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    
    Ok(())
}

/// Validate authority account
pub fn is_authority(
    account: &AccountInfo,
    authority: &Pubkey,
) -> Result<(), ProgramError> {
    if account.key != authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    if !account.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    Ok(())
}

/// Validate account owner (alias for validate_program_owner)
pub fn validate_account_owner(
    account: &AccountInfo,
    expected_owner: &Pubkey,
) -> Result<(), ProgramError> {
    validate_program_owner(account, expected_owner)
}