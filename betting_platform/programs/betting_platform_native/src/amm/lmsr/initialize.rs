//! LMSR market initialization

use borsh::BorshSerialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
    clock::Clock,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    error::BettingPlatformError,
    events::{Event, MarketCreated},
    pda::LmsrMarketPDA,
    state::amm_accounts::LSMRMarket,
    amm::constants::*,
};

/// Initialize a new LMSR market
pub fn process_initialize_lmsr(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    b_parameter: u64,
    num_outcomes: u8,
) -> ProgramResult {
    msg!("Initializing LMSR market");
    
    // Validate parameters
    if num_outcomes < 2 || num_outcomes > MAX_OUTCOMES {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if b_parameter < MIN_LIQUIDITY {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }
    
    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let initializer = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let oracle = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(initializer)?;
    validate_writable(market_account)?;
    
    // Validate PDA
    let (market_pda, bump) = LmsrMarketPDA::derive(program_id, market_id);
    if market_account.key != &market_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create market account
    let rent = Rent::from_account_info(rent_sysvar)?;
    let market_size = 8 + // discriminator
        16 + // market_id
        8 + // b_parameter
        1 + // num_outcomes
        4 + (num_outcomes as usize * 8) + // shares vector
        8 + // cost_basis
        1 + // state
        8 + // created_at
        8 + // last_update
        8 + // total_volume
        2 + // fee_bps
        32 + // oracle
        64; // padding
    
    let required_lamports = rent.minimum_balance(market_size);
    
    invoke(
        &solana_program::system_instruction::create_account(
            initializer.key,
            market_account.key,
            required_lamports,
            market_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            market_account.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Check initializer has enough funds for liquidity
    let total_cost = b_parameter + required_lamports;
    if **initializer.lamports.borrow() < total_cost {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }
    
    // Transfer liquidity
    **initializer.lamports.borrow_mut() -= b_parameter;
    **market_account.lamports.borrow_mut() += b_parameter;
    
    // Initialize market
    let clock = Clock::get()?;
    let market = LSMRMarket::new(market_id, b_parameter, num_outcomes, *oracle.key);
    
    // Write market data
    let mut market_with_timestamp = market;
    market_with_timestamp.created_at = clock.unix_timestamp;
    market_with_timestamp.last_update = clock.unix_timestamp;
    
    market_with_timestamp.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    
    // Emit event
    MarketCreated {
        market_id,
        amm_type: "LMSR".to_string(),
        num_outcomes,
        initial_liquidity: b_parameter,
        oracle: *oracle.key,
    }.emit();
    
    msg!("LMSR market initialized successfully");
    Ok(())
}