//! SPL Token program CPI helpers

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

use spl_token::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};

use super::depth_tracker::CPIDepthTracker;
use crate::invoke_with_depth_check;

/// SPL Token program ID
pub const TOKEN_PROGRAM_ID: Pubkey = spl_token::ID;

/// Create and initialize a new SPL Token mint
pub fn create_mint<'a>(
    payer: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    mint_authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
    decimals: u8,
    token_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    depth_tracker: &mut CPIDepthTracker,
) -> ProgramResult {
    // Get rent exemption
    let rent = Rent::from_account_info(rent_sysvar)?;
    let mint_rent = rent.minimum_balance(Mint::LEN);
    
    // Create mint account
    invoke_with_depth_check!(
        depth_tracker,
        &system_instruction::create_account(
            payer.key,
            mint.key,
            mint_rent,
            Mint::LEN as u64,
            &TOKEN_PROGRAM_ID,
        ),
        &[payer.clone(), mint.clone(), system_program.clone()]
    )?;
    
    // Initialize mint
    invoke_with_depth_check!(
        depth_tracker,
        &token_instruction::initialize_mint(
            &TOKEN_PROGRAM_ID,
            mint.key,
            mint_authority,
            freeze_authority,
            decimals,
        )?,
        &[mint.clone(), rent_sysvar.clone()]
    )?;
    
    Ok(())
}

/// Create a new SPL Token account
pub fn create_token_account<'a>(
    payer: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    owner: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    // Get rent exemption
    let rent = Rent::from_account_info(rent_sysvar)?;
    let account_rent = rent.minimum_balance(TokenAccount::LEN);
    
    // Create token account
    invoke(
        &system_instruction::create_account(
            payer.key,
            token_account.key,
            account_rent,
            TokenAccount::LEN as u64,
            &TOKEN_PROGRAM_ID,
        ),
        &[payer.clone(), token_account.clone(), system_program.clone()],
    )?;
    
    // Initialize token account
    invoke(
        &token_instruction::initialize_account(
            &TOKEN_PROGRAM_ID,
            token_account.key,
            mint.key,
            owner.key,
        )?,
        &[
            token_account.clone(),
            mint.clone(),
            owner.clone(),
            rent_sysvar.clone(),
        ],
    )?;
    
    Ok(())
}

/// Transfer SPL tokens
pub fn transfer<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = token_instruction::transfer(
        &TOKEN_PROGRAM_ID,
        source.key,
        destination.key,
        authority.key,
        &[],
        amount,
    )?;
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[source.clone(), destination.clone(), authority.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[source.clone(), destination.clone(), authority.clone()],
            signer_seeds,
        )
    }
}

/// Mint new tokens
pub fn mint_to<'a>(
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    mint_authority: &AccountInfo<'a>,
    amount: u64,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = token_instruction::mint_to(
        &TOKEN_PROGRAM_ID,
        mint.key,
        destination.key,
        mint_authority.key,
        &[],
        amount,
    )?;
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[mint.clone(), destination.clone(), mint_authority.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[mint.clone(), destination.clone(), mint_authority.clone()],
            signer_seeds,
        )
    }
}

/// Burn tokens
pub fn burn<'a>(
    token_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = token_instruction::burn(
        &TOKEN_PROGRAM_ID,
        token_account.key,
        mint.key,
        authority.key,
        &[],
        amount,
    )?;
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[token_account.clone(), mint.clone(), authority.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[token_account.clone(), mint.clone(), authority.clone()],
            signer_seeds,
        )
    }
}

/// Approve token delegation
pub fn approve<'a>(
    source: &AccountInfo<'a>,
    delegate: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = token_instruction::approve(
        &TOKEN_PROGRAM_ID,
        source.key,
        delegate.key,
        authority.key,
        &[],
        amount,
    )?;
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[source.clone(), delegate.clone(), authority.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[source.clone(), delegate.clone(), authority.clone()],
            signer_seeds,
        )
    }
}

/// Revoke token delegation
pub fn revoke<'a>(
    source: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = token_instruction::revoke(
        &TOKEN_PROGRAM_ID,
        source.key,
        authority.key,
        &[],
    )?;
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[source.clone(), authority.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[source.clone(), authority.clone()],
            signer_seeds,
        )
    }
}

/// Close token account
pub fn close_account<'a>(
    account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = token_instruction::close_account(
        &TOKEN_PROGRAM_ID,
        account.key,
        destination.key,
        authority.key,
        &[],
    )?;
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[account.clone(), destination.clone(), authority.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[account.clone(), destination.clone(), authority.clone()],
            signer_seeds,
        )
    }
}

/// Sync native SOL to wrapped SOL balance
pub fn sync_native<'a>(
    account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> ProgramResult {
    let instruction = token_instruction::sync_native(&TOKEN_PROGRAM_ID, account.key)?;
    
    invoke(&instruction, &[account.clone()])
}

/// Helper to get token account data
pub fn get_token_account_data(account: &AccountInfo) -> Result<TokenAccount, ProgramError> {
    TokenAccount::unpack(&account.data.borrow())
}

/// Helper to get mint data
pub fn get_mint_data(mint: &AccountInfo) -> Result<Mint, ProgramError> {
    Mint::unpack(&mint.data.borrow())
}