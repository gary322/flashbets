//! Circuit breaker initialization

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
};
use borsh::BorshSerialize;

use crate::{
    error::BettingPlatformError,
    state::security_accounts::CircuitBreaker,
};

pub fn process_initialize_breaker(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing circuit breaker");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let initializer = next_account_info(account_info_iter)?; // Authority
    let circuit_breaker_account = next_account_info(account_info_iter)?; // PDA for circuit breaker
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    
    // Verify initializer is signer
    if !initializer.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive circuit breaker PDA
    let (circuit_breaker_pda, bump_seed) = Pubkey::find_program_address(
        &[b"circuit_breaker"],
        program_id,
    );
    
    // Verify PDA matches
    if circuit_breaker_pda != *circuit_breaker_account.key {
        msg!("Invalid circuit breaker PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if already initialized
    if circuit_breaker_account.data_len() > 0 {
        msg!("Circuit breaker already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate required space
    let breaker_size = std::mem::size_of::<CircuitBreaker>();
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(breaker_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            circuit_breaker_account.key,
            rent_lamports,
            breaker_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            circuit_breaker_account.clone(),
            system_program.clone(),
        ],
        &[&[b"circuit_breaker", &[bump_seed]]],
    )?;
    
    // Initialize circuit breaker with default values
    let breaker = CircuitBreaker::new();
    
    // Log configuration
    msg!("Circuit breaker initialized with thresholds:");
    msg!("  Coverage: {}%", breaker.coverage_threshold / 100);
    msg!("  Price movement: {}%", breaker.price_movement_threshold / 100);
    msg!("  Volume spike: {}x", breaker.volume_spike_threshold / 100);
    msg!("  Liquidation cascade: {} positions", breaker.liquidation_cascade_threshold);
    msg!("  Congestion: {}% failed tx", breaker.congestion_threshold / 100);
    msg!("  Cooldown period: {} slots", breaker.cooldown_period);
    
    // Serialize and save
    breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    
    msg!("Circuit breaker initialized successfully");
    
    Ok(())
}