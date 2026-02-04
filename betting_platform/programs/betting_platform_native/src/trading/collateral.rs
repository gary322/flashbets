//! Collateral management using CPI
//!
//! Handles USDC collateral deposits, withdrawals, and management

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    cpi::{associated_token, spl_token},
    error::BettingPlatformError,
    events::{CollateralDeposited, CollateralWithdrawn, Event},
    pda::CollateralVaultPDA,
    state::CollateralVault,
};

/// USDC mint address on mainnet
pub const USDC_MINT: Pubkey = solana_program::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

/// Process collateral deposit
pub fn process_deposit_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    msg!("Processing collateral deposit of {} USDC", amount);
    
    let account_info_iter = &mut accounts.iter();
    
    let depositor = next_account_info(account_info_iter)?;
    let depositor_usdc_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let vault_usdc_account = next_account_info(account_info_iter)?;
    let usdc_mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let associated_token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(depositor)?;
    validate_writable(vault_account)?;
    validate_writable(vault_usdc_account)?;
    
    // Verify USDC mint
    if usdc_mint.key != &USDC_MINT {
        return Err(BettingPlatformError::InvalidMint.into());
    }
    
    // Derive and verify vault PDA
    let (vault_pda, bump) = CollateralVaultPDA::derive(program_id);
    if vault_account.key != &vault_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create vault's associated token account if needed
    let vault_ata = associated_token::get_associated_token_address(
        &vault_pda,
        &USDC_MINT,
    );
    
    if vault_usdc_account.key != &vault_ata {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create ATA if it doesn't exist
    if vault_usdc_account.data_is_empty() {
        associated_token::create_associated_token_account(
            depositor,
            vault_usdc_account,
            vault_account,
            usdc_mint,
            system_program,
            token_program,
            rent_sysvar,
        )?;
    }
    
    // Transfer USDC from depositor to vault
    spl_token::transfer(
        depositor_usdc_account,
        vault_usdc_account,
        depositor,
        amount,
        token_program,
        &[],  // No signer seeds needed for user transfer
    )?;
    
    // Load or initialize vault state
    let mut vault = if vault_account.data_len() > 0 {
        CollateralVault::try_from_slice(&vault_account.data.borrow())?
    } else {
        CollateralVault {
            total_deposits: 0,
            total_borrowed: 0,
            depositor_count: 0,
            last_update: Clock::get()?.unix_timestamp,
        }
    };
    
    // Update vault state
    vault.total_deposits = vault.total_deposits
        .checked_add(amount)
        .ok_or(BettingPlatformError::MathOverflow)?;
    vault.depositor_count += 1;
    vault.last_update = Clock::get()?.unix_timestamp;
    
    // Save vault state
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Emit event
    CollateralDeposited {
        depositor: *depositor.key,
        amount,
        total_deposits: vault.total_deposits,
        timestamp: Clock::get()?.unix_timestamp,
    }
    .emit();
    
    msg!("Collateral deposit successful");
    Ok(())
}

/// Process collateral withdrawal
pub fn process_withdraw_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    msg!("Processing collateral withdrawal of {} USDC", amount);
    
    let account_info_iter = &mut accounts.iter();
    
    let withdrawer = next_account_info(account_info_iter)?;
    let withdrawer_usdc_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let vault_usdc_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(withdrawer)?;
    validate_writable(vault_account)?;
    validate_writable(vault_usdc_account)?;
    validate_writable(withdrawer_usdc_account)?;
    
    // Verify vault PDA
    let (vault_pda, bump) = CollateralVaultPDA::derive(program_id);
    if vault_account.key != &vault_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load vault state
    let mut vault = CollateralVault::try_from_slice(&vault_account.data.borrow())?;
    
    // Check available balance
    let available = vault.total_deposits
        .checked_sub(vault.total_borrowed)
        .ok_or(BettingPlatformError::InsufficientBalance)?;
    
    if amount > available {
        return Err(BettingPlatformError::InsufficientCollateral.into());
    }
    
    // Transfer USDC from vault to withdrawer
    let vault_seeds = &[
        b"collateral_vault".as_ref(),
        &[bump],
    ];
    spl_token::transfer(
        vault_usdc_account,
        withdrawer_usdc_account,
        vault_authority,
        amount,
        token_program,
        &[vault_seeds],
    )?;
    
    // Update vault state
    vault.total_deposits = vault.total_deposits
        .checked_sub(amount)
        .ok_or(BettingPlatformError::Underflow)?;
    vault.last_update = Clock::get()?.unix_timestamp;
    
    // Save vault state
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Emit event
    CollateralWithdrawn {
        withdrawer: *withdrawer.key,
        amount,
        total_deposits: vault.total_deposits,
        timestamp: Clock::get()?.unix_timestamp,
    }
    .emit();
    
    msg!("Collateral withdrawal successful");
    Ok(())
}

/// Borrow collateral for trading (internal use)
pub fn borrow_collateral_internal(
    vault: &mut CollateralVault,
    amount: u64,
) -> Result<(), ProgramError> {
    let available = vault.total_deposits
        .checked_sub(vault.total_borrowed)
        .ok_or(BettingPlatformError::InsufficientBalance)?;
    
    if amount > available {
        return Err(BettingPlatformError::InsufficientCollateral.into());
    }
    
    vault.total_borrowed = vault.total_borrowed
        .checked_add(amount)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    Ok(())
}

/// Return borrowed collateral (internal use)
pub fn return_collateral_internal(
    vault: &mut CollateralVault,
    amount: u64,
) -> Result<(), ProgramError> {
    vault.total_borrowed = vault.total_borrowed
        .checked_sub(amount)
        .ok_or(BettingPlatformError::Underflow)?;
    
    Ok(())
}