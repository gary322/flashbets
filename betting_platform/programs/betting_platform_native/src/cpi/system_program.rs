//! System program CPI helpers

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    system_program,
    sysvar::Sysvar,
};

use crate::BettingPlatformError;

/// Create a new account
pub fn create_account<'a>(
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    // Verify system program
    if system_program.key != &system_program::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    invoke(
        &system_instruction::create_account(
            payer.key,
            new_account.key,
            lamports,
            space,
            owner,
        ),
        &[payer.clone(), new_account.clone()],
    )
}

/// Create account with seed
pub fn create_account_with_seed<'a>(
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    base: &AccountInfo<'a>,
    seed: &str,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = system_instruction::create_account_with_seed(
        payer.key,
        new_account.key,
        base.key,
        seed,
        lamports,
        space,
        owner,
    );
    
    if signer_seeds.is_empty() {
        invoke(
            &instruction,
            &[payer.clone(), new_account.clone(), base.clone()],
        )
    } else {
        invoke_signed(
            &instruction,
            &[payer.clone(), new_account.clone(), base.clone()],
            signer_seeds,
        )
    }
}

/// Transfer lamports
pub fn transfer<'a>(
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    lamports: u64,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &system_instruction::transfer(from.key, to.key, lamports),
        &[from.clone(), to.clone()],
    )
}

/// Transfer lamports with signer seeds (for PDAs)
pub fn transfer_signed<'a>(
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    lamports: u64,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    invoke_signed(
        &system_instruction::transfer(from.key, to.key, lamports),
        &[from.clone(), to.clone()],
        signer_seeds,
    )
}

/// Allocate space for an account
pub fn allocate<'a>(
    account: &AccountInfo<'a>,
    space: u64,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &system_instruction::allocate(account.key, space),
        &[account.clone()],
    )
}

/// Allocate space with seed
pub fn allocate_with_seed<'a>(
    account: &AccountInfo<'a>,
    base: &AccountInfo<'a>,
    seed: &str,
    space: u64,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = system_instruction::allocate_with_seed(
        account.key,
        base.key,
        seed,
        space,
        owner,
    );
    
    if signer_seeds.is_empty() {
        invoke(&instruction, &[account.clone(), base.clone()])
    } else {
        invoke_signed(
            &instruction,
            &[account.clone(), base.clone()],
            signer_seeds,
        )
    }
}

/// Assign account to program
pub fn assign<'a>(
    account: &AccountInfo<'a>,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &system_instruction::assign(account.key, owner),
        &[account.clone()],
    )
}

/// Assign with seed
pub fn assign_with_seed<'a>(
    account: &AccountInfo<'a>,
    base: &AccountInfo<'a>,
    seed: &str,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction = system_instruction::assign_with_seed(
        account.key,
        base.key,
        seed,
        owner,
    );
    
    if signer_seeds.is_empty() {
        invoke(&instruction, &[account.clone(), base.clone()])
    } else {
        invoke_signed(
            &instruction,
            &[account.clone(), base.clone()],
            signer_seeds,
        )
    }
}

/// Create PDA account
pub fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    space: u64,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    // Get rent exemption
    let rent = Rent::from_account_info(rent_sysvar)?;
    let required_lamports = rent.minimum_balance(space as usize);
    
    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            pda_account.key,
            required_lamports,
            space,
            owner,
        ),
        &[payer.clone(), pda_account.clone()],
        &[signer_seeds],
    )
}

/// Transfer lamports from PDA
pub fn transfer_from_pda<'a>(
    pda: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    lamports: u64,
    system_program: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    invoke_signed(
        &system_instruction::transfer(pda.key, to.key, lamports),
        &[pda.clone(), to.clone()],
        &[signer_seeds],
    )
}

/// Helper to check if account is owned by system program
pub fn is_system_owned(account: &AccountInfo) -> bool {
    account.owner == &system_program::ID
}

/// Helper to check if account has sufficient lamports
pub fn has_sufficient_lamports(account: &AccountInfo, required: u64) -> bool {
    **account.lamports.borrow() >= required
}

/// Close account and transfer lamports to destination
pub fn close_account<'a>(
    account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
) -> ProgramResult {
    let dest_starting_lamports = **destination.lamports.borrow();
    let account_lamports = **account.lamports.borrow();
    
    **destination.lamports.borrow_mut() = dest_starting_lamports
        .checked_add(account_lamports)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    **account.lamports.borrow_mut() = 0;
    
    Ok(())
}