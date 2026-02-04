//! Dark pool initialization

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::BorshSerialize;

use crate::{
    error::BettingPlatformError,
    state::order_accounts::DarkPool,
};

pub fn process_initialize_dark_pool(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    min_order_size: u64,
    price_improvement_bps: u16,
) -> ProgramResult {
    msg!("Initializing dark pool for market {}", market_id);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let initializer = next_account_info(account_info_iter)?;
    let dark_pool_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify initializer is signer
    if !initializer.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive dark pool PDA
    let market_id_bytes = market_id.to_le_bytes();
    let (dark_pool_pda, bump_seed) = Pubkey::find_program_address(
        &[b"dark_pool", &market_id_bytes],
        program_id,
    );
    
    // Verify PDA matches
    if dark_pool_pda != *dark_pool_account.key {
        msg!("Invalid dark pool PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if already initialized
    if dark_pool_account.data_len() > 0 {
        msg!("Dark pool already initialized for market {}", market_id);
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Validate parameters
    if min_order_size == 0 {
        msg!("Minimum order size must be greater than 0");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if price_improvement_bps == 0 || price_improvement_bps > 1000 { // Max 10%
        msg!("Price improvement must be between 1 and 1000 basis points");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock)?;
    let current_time = clock.unix_timestamp;
    
    // Calculate required space
    let dark_pool_size = std::mem::size_of::<DarkPool>();
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(dark_pool_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            dark_pool_account.key,
            rent_lamports,
            dark_pool_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            dark_pool_account.clone(),
            system_program.clone(),
        ],
        &[&[b"dark_pool", &market_id_bytes, &[bump_seed]]],
    )?;
    
    // Initialize dark pool
    let dark_pool = DarkPool::new(
        market_id,
        min_order_size,
        price_improvement_bps,
        current_time,
    );
    
    // Log configuration
    msg!("Dark pool initialized:");
    msg!("  Market ID: {}", market_id);
    msg!("  Minimum order size: {}", min_order_size);
    msg!("  Price improvement: {} bps", price_improvement_bps);
    msg!("  Created at: {}", current_time);
    
    // Serialize and save
    dark_pool.serialize(&mut &mut dark_pool_account.data.borrow_mut()[..])?;
    
    msg!("Dark pool initialized successfully");
    
    Ok(())
}