//! Account validation framework for native Solana
//!
//! Provides comprehensive validation for all account types and operations

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
};

use crate::error::BettingPlatformError;

/// Account discriminator size (8 bytes like Anchor)
pub const DISCRIMINATOR_SIZE: usize = 8;

/// Validate that an account is owned by the expected program
pub fn validate_owner(
    account: &AccountInfo,
    expected_owner: &Pubkey,
) -> ProgramResult {
    if account.owner != expected_owner {
        msg!(
            "Account owner mismatch. Expected: {}, Actual: {}",
            expected_owner,
            account.owner
        );
        return Err(BettingPlatformError::Unauthorized.into());
    }
    Ok(())
}

/// Validate that an account is a signer
pub fn validate_signer(account: &AccountInfo) -> ProgramResult {
    if !account.is_signer {
        msg!("Account {} must be a signer", account.key);
        return Err(BettingPlatformError::Unauthorized.into());
    }
    Ok(())
}

/// Validate that an account is writable
pub fn validate_writable(account: &AccountInfo) -> ProgramResult {
    if !account.is_writable {
        msg!("Account {} must be writable", account.key);
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Validate account size
pub fn validate_size(
    account: &AccountInfo,
    expected_size: usize,
) -> ProgramResult {
    if account.data_len() != expected_size {
        msg!(
            "Account size mismatch. Expected: {}, Actual: {}",
            expected_size,
            account.data_len()
        );
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Validate that an account has enough lamports for rent
pub fn validate_rent_exempt(
    account: &AccountInfo,
    rent: &Rent,
) -> ProgramResult {
    if !rent.is_exempt(account.lamports(), account.data_len()) {
        msg!("Account {} is not rent exempt", account.key);
        return Err(ProgramError::AccountNotRentExempt);
    }
    Ok(())
}

/// Validate PDA derivation
pub fn validate_pda(
    account: &AccountInfo,
    program_id: &Pubkey,
    seeds: &[&[u8]],
) -> ProgramResult {
    let (expected_key, _bump) = Pubkey::find_program_address(seeds, program_id);
    
    if account.key != &expected_key {
        msg!(
            "PDA mismatch. Expected: {}, Actual: {}",
            expected_key,
            account.key
        );
        return Err(ProgramError::InvalidSeeds);
    }
    
    Ok(())
}

/// Validate account discriminator
pub fn validate_discriminator(
    account_data: &[u8],
    expected_discriminator: &[u8; DISCRIMINATOR_SIZE],
) -> ProgramResult {
    if account_data.len() < DISCRIMINATOR_SIZE {
        msg!("Account data too small for discriminator");
        return Err(ProgramError::InvalidAccountData);
    }
    
    let discriminator = &account_data[..DISCRIMINATOR_SIZE];
    if discriminator != expected_discriminator {
        msg!("Invalid account discriminator");
        return Err(ProgramError::InvalidAccountData);
    }
    
    Ok(())
}

/// Combined validation for program-owned accounts
pub fn validate_program_account<'a>(
    account: &'a AccountInfo<'a>,
    program_id: &Pubkey,
    expected_discriminator: &[u8; DISCRIMINATOR_SIZE],
    expected_size: usize,
    writable: bool,
) -> ProgramResult {
    // Check owner
    validate_owner(account, program_id)?;
    
    // Check writable if required
    if writable {
        validate_writable(account)?;
    }
    
    // Check size
    validate_size(account, expected_size)?;
    
    // Check discriminator
    let data = account.try_borrow_data()?;
    validate_discriminator(&data, expected_discriminator)?;
    
    Ok(())
}

/// Validate system program
pub fn validate_system_program(account: &AccountInfo) -> ProgramResult {
    if account.key != &solana_program::system_program::id() {
        msg!("Invalid system program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate token program
pub fn validate_token_program(account: &AccountInfo) -> ProgramResult {
    if account.key != &spl_token::id() {
        msg!("Invalid token program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate associated token program
pub fn validate_associated_token_program(account: &AccountInfo) -> ProgramResult {
    if account.key != &spl_associated_token_account::id() {
        msg!("Invalid associated token program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate rent sysvar
pub fn validate_rent_sysvar(account: &AccountInfo) -> ProgramResult {
    if account.key != &solana_program::sysvar::rent::id() {
        msg!("Invalid rent sysvar");
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Account validation context builder
pub struct AccountValidator<'a, 'b> {
    account: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

impl<'a, 'b> AccountValidator<'a, 'b> {
    pub fn new(account: &'a AccountInfo<'b>, program_id: &'a Pubkey) -> Self {
        Self { account, program_id }
    }
    
    pub fn owner(self, expected_owner: &Pubkey) -> Result<Self, ProgramError> {
        validate_owner(self.account, expected_owner)?;
        Ok(self)
    }
    
    pub fn signer(self) -> Result<Self, ProgramError> {
        validate_signer(self.account)?;
        Ok(self)
    }
    
    pub fn writable(self) -> Result<Self, ProgramError> {
        validate_writable(self.account)?;
        Ok(self)
    }
    
    pub fn size(self, expected_size: usize) -> Result<Self, ProgramError> {
        validate_size(self.account, expected_size)?;
        Ok(self)
    }
    
    pub fn discriminator(self, expected: &[u8; DISCRIMINATOR_SIZE]) -> Result<Self, ProgramError> {
        let data = self.account.try_borrow_data()?;
        validate_discriminator(&data, expected)?;
        Ok(self)
    }
    
    pub fn pda(self, seeds: &[&[u8]]) -> Result<Self, ProgramError> {
        validate_pda(self.account, self.program_id, seeds)?;
        Ok(self)
    }
    
    pub fn rent_exempt(self, rent: &Rent) -> Result<Self, ProgramError> {
        validate_rent_exempt(self.account, rent)?;
        Ok(self)
    }
    
    pub fn finish(self) -> &'a AccountInfo<'b> {
        self.account
    }
}

/// Helper macro for account validation
#[macro_export]
macro_rules! validate_account {
    ($account:expr, $program_id:expr) => {
        $crate::account_validation::AccountValidator::new($account, $program_id)
    };
}

/// Helper to check if account is initialized (non-zero discriminator)
pub fn is_initialized(account_data: &[u8]) -> bool {
    if account_data.len() < DISCRIMINATOR_SIZE {
        return false;
    }
    
    let discriminator = &account_data[..DISCRIMINATOR_SIZE];
    discriminator != &[0u8; DISCRIMINATOR_SIZE]
}

/// Validate token account
pub fn validate_token_account(
    account: &AccountInfo,
    mint: &Pubkey,
    owner: &Pubkey,
) -> ProgramResult {
    validate_owner(account, &spl_token::id())?;
    
    if account.data_len() != spl_token::state::Account::LEN {
        msg!("Invalid token account size");
        return Err(ProgramError::InvalidAccountData);
    }
    
    let token_account = spl_token::state::Account::unpack(&account.try_borrow_data()?)?;
    
    if &token_account.mint != mint {
        msg!("Token account mint mismatch");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if &token_account.owner != owner {
        msg!("Token account owner mismatch");
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    Ok(())
}

/// Validate mint account
pub fn validate_mint(
    account: &AccountInfo,
    expected_decimals: Option<u8>,
) -> ProgramResult {
    validate_owner(account, &spl_token::id())?;
    
    if account.data_len() != spl_token::state::Mint::LEN {
        msg!("Invalid mint account size");
        return Err(ProgramError::InvalidAccountData);
    }
    
    let mint = spl_token::state::Mint::unpack(&account.try_borrow_data()?)?;
    
    if let Some(decimals) = expected_decimals {
        if mint.decimals != decimals {
            msg!("Mint decimals mismatch");
            return Err(BettingPlatformError::InvalidInput.into());
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Epoch;
    
    #[test]
    fn test_discriminator_validation() {
        let valid_discriminator = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&valid_discriminator);
        
        assert!(validate_discriminator(&data, &valid_discriminator).is_ok());
        assert!(validate_discriminator(&data, &[0u8; 8]).is_err());
    }
    
    #[test]
    fn test_is_initialized() {
        let uninitialized = vec![0u8; 100];
        assert!(!is_initialized(&uninitialized));
        
        let mut initialized = vec![0u8; 100];
        initialized[0] = 1;
        assert!(is_initialized(&initialized));
    }
}