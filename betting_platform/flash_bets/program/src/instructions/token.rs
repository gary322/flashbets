use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::invoke,
    program::invoke_signed,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
    program_error::ProgramError,
    program_pack::Pack,
};
use spl_token::instruction::{transfer, transfer_checked};
use crate::errors::FlashError;

/// Transfer SPL tokens for flash bet placement
pub fn transfer_tokens<'a>(
    token_program: &AccountInfo<'a>,
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    let transfer_instruction = transfer(
        token_program.key,
        from.key,
        to.key,
        authority.key,
        &[authority.key],
        amount,
    )?;

    invoke(
        &transfer_instruction,
        &[from.clone(), to.clone(), authority.clone(), token_program.clone()],
    )?;

    Ok(())
}

/// Transfer SPL tokens with checked decimals
pub fn transfer_tokens_checked<'a>(
    token_program: &AccountInfo<'a>,
    from: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    decimals: u8,
) -> ProgramResult {
    let transfer_instruction = transfer_checked(
        token_program.key,
        from.key,
        mint.key,
        to.key,
        authority.key,
        &[authority.key],
        amount,
        decimals,
    )?;

    invoke(
        &transfer_instruction,
        &[
            from.clone(),
            mint.clone(),
            to.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )?;

    Ok(())
}

/// Transfer SPL tokens with PDA authority
pub fn transfer_with_pda<'a>(
    token_program: &AccountInfo<'a>,
    from: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    amount: u64,
    decimals: u8,
    pda_seeds: &[&[u8]],
    pda_bump: u8,
) -> ProgramResult {
    let binding = [pda_bump];
    let seeds = &[&pda_seeds[..], &[&binding]].concat();
    
    let transfer_instruction = transfer_checked(
        token_program.key,
        from.key,
        mint.key,
        to.key,
        pda_account.key,
        &[],
        amount,
        decimals,
    )?;

    invoke_signed(
        &transfer_instruction,
        &[
            from.clone(),
            mint.clone(),
            to.clone(),
            pda_account.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;

    Ok(())
}

/// Process flash bet payout with PDA authority
pub fn process_payout<'a>(
    token_program: &AccountInfo<'a>,
    verse_vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    winner_account: &AccountInfo<'a>,
    verse_authority: &AccountInfo<'a>,
    payout_amount: u64,
    decimals: u8,
    verse_id: u128,
    bump: u8,
) -> ProgramResult {
    // Derive PDA seeds for verse authority
    let verse_id_bytes = verse_id.to_le_bytes();
    let seeds = &[
        b"flash_verse",
        verse_id_bytes.as_ref(),
    ];

    // Transfer payout from vault to winner
    transfer_with_pda(
        token_program,
        verse_vault,
        mint,
        winner_account,
        verse_authority,
        payout_amount,
        decimals,
        seeds,
        bump,
    )?;

    Ok(())
}

/// Calculate proportional payout with house edge
pub fn calculate_payout(
    position_amount: u64,
    total_winning_stake: u64,
    total_pool: u64,
) -> Result<u64, ProgramError> {
    // Apply 2% house edge
    let available_pool = total_pool
        .checked_mul(98)
        .ok_or(FlashError::InsufficientLiquidity)?
        .checked_div(100)
        .ok_or(FlashError::InsufficientLiquidity)?;

    // Calculate proportional share
    let payout = (position_amount as u128)
        .checked_mul(available_pool as u128)
        .ok_or(FlashError::InsufficientLiquidity)?
        .checked_div(total_winning_stake as u128)
        .ok_or(FlashError::InsufficientLiquidity)?;

    // Ensure payout doesn't exceed u64::MAX
    if payout > u64::MAX as u128 {
        return Err(FlashError::InsufficientLiquidity.into());
    }

    Ok(payout as u64)
}

/// Calculate leverage multiplier based on amount and tau
pub fn calculate_leverage_multiplier(amount: u64, tau: f64, base_leverage: u8) -> u16 {
    // Higher amounts with lower tau get better leverage
    let tau_factor = if tau > 0.0 { 1.0 / tau } else { 1.0 };
    let amount_factor = (amount as f64 / 1000.0).min(10.0); // Cap amount factor at 10x
    
    let multiplier = (base_leverage as f64) * (1.0 + tau_factor * 0.1 + amount_factor * 0.05);
    
    // Cap at 500x total leverage
    multiplier.min(500.0) as u16
}

/// Validate token account ownership
pub fn validate_token_account(
    account: &AccountInfo,
    _expected_owner: &Pubkey,
    _expected_mint: Option<&Pubkey>,
) -> ProgramResult {
    if account.data_is_empty() {
        return Err(FlashError::InvalidAmount.into());
    }

    // Basic validation - in production would parse full token account data
    if account.owner != &spl_token::id() {
        return Err(FlashError::InvalidAmount.into());
    }

    Ok(())
}

/// Create associated token account if needed
pub fn create_associated_token_account_if_needed<'a>(
    funding_account: &AccountInfo<'a>,
    associated_account: &AccountInfo<'a>,
    owner: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    associated_token_program: &AccountInfo<'a>,
) -> ProgramResult {
    // If account already exists, skip creation
    if associated_account.data_len() > 0 {
        return Ok(());
    }

    // Create associated token account
    let create_ata_instruction = spl_associated_token_account::instruction::create_associated_token_account(
        funding_account.key,
        owner.key,
        mint.key,
        token_program.key,
    );

    invoke(
        &create_ata_instruction,
        &[
            funding_account.clone(),
            associated_account.clone(),
            owner.clone(),
            mint.clone(),
            system_program.clone(),
            token_program.clone(),
            associated_token_program.clone(),
        ],
    )?;

    Ok(())
}

/// Get the minimum balance for a token account
pub fn get_token_account_rent() -> Result<u64, ProgramError> {
    let rent = Rent::get()?;
    Ok(rent.minimum_balance(spl_token::state::Account::LEN))
}