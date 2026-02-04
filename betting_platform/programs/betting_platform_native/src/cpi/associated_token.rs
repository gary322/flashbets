//! Associated Token Account (ATA) program CPI helpers

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Associated Token Account program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = spl_associated_token_account::ID;

/// Create associated token account
pub fn create_associated_token_account<'a>(
    payer: &AccountInfo<'a>,
    associated_token_account: &AccountInfo<'a>,
    wallet: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> ProgramResult {
    let instruction = spl_associated_token_account::instruction::create_associated_token_account(
        payer.key,
        wallet.key,
        mint.key,
        token_program.key,
    );
    
    invoke(
        &instruction,
        &[
            payer.clone(),
            associated_token_account.clone(),
            wallet.clone(),
            mint.clone(),
            system_program.clone(),
            token_program.clone(),
            rent_sysvar.clone(),
        ],
    )
}

/// Create associated token account idempotent (won't fail if already exists)
pub fn create_associated_token_account_idempotent<'a>(
    payer: &AccountInfo<'a>,
    associated_token_account: &AccountInfo<'a>,
    wallet: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> ProgramResult {
    // Check if account already exists
    if !associated_token_account.data_is_empty() {
        // Account exists, verify it's the correct ATA
        let expected_ata = get_associated_token_address(wallet.key, mint.key);
        if associated_token_account.key != &expected_ata {
            return Err(ProgramError::InvalidAccountData);
        }
        return Ok(());
    }
    
    create_associated_token_account(
        payer,
        associated_token_account,
        wallet,
        mint,
        system_program,
        token_program,
        rent_sysvar,
    )
}

/// Get associated token address for wallet and mint
pub fn get_associated_token_address(
    wallet: &Pubkey,
    mint: &Pubkey,
) -> Pubkey {
    get_associated_token_address_with_program_id(
        wallet,
        mint,
        &spl_token::ID,
    )
}

/// Get associated token address with specific token program
pub fn get_associated_token_address_with_program_id(
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            token_program_id.as_ref(),
            mint.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    ).0
}

/// Create ATA for PDA wallet
pub fn create_pda_associated_token_account<'a>(
    payer: &AccountInfo<'a>,
    associated_token_account: &AccountInfo<'a>,
    pda_wallet: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    pda_signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = spl_associated_token_account::instruction::create_associated_token_account(
        payer.key,
        pda_wallet.key,
        mint.key,
        token_program.key,
    );
    
    invoke_signed(
        &instruction,
        &[
            payer.clone(),
            associated_token_account.clone(),
            pda_wallet.clone(),
            mint.clone(),
            system_program.clone(),
            token_program.clone(),
            rent_sysvar.clone(),
        ],
        pda_signer_seeds,
    )
}

/// Find or create associated token account
pub fn find_or_create_associated_token_account<'a>(
    payer: &AccountInfo<'a>,
    wallet: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    associated_token_program: &AccountInfo<'a>,
) -> Result<Pubkey, ProgramError> {
    let ata_address = get_associated_token_address(wallet.key, mint.key);
    
    // Try to get account info
    let ata_account = match associated_token_program.clone() {
        account if account.key == &ata_address => {
            // ATA already provided in accounts
            if account.data_is_empty() {
                // Need to create it
                create_associated_token_account(
                    payer,
                    &account,
                    wallet,
                    mint,
                    system_program,
                    token_program,
                    rent_sysvar,
                )?;
            }
            account
        }
        _ => {
            // ATA not provided, would need to be fetched
            // In a real CPI context, we can't fetch arbitrary accounts
            return Err(ProgramError::NotEnoughAccountKeys);
        }
    };
    
    Ok(*ata_account.key)
}

/// Verify associated token account
pub fn verify_associated_token_account(
    ata: &AccountInfo,
    wallet: &Pubkey,
    mint: &Pubkey,
) -> Result<(), ProgramError> {
    let expected_ata = get_associated_token_address(wallet, mint);
    
    if ata.key != &expected_ata {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Could also verify the account data contains correct mint/owner
    // but that requires unpacking the token account
    
    Ok(())
}

/// Helper to check if account is an ATA
pub fn is_associated_token_account(
    account: &AccountInfo,
    wallet: &Pubkey,
    mint: &Pubkey,
) -> bool {
    let expected_ata = get_associated_token_address(wallet, mint);
    account.key == &expected_ata
}