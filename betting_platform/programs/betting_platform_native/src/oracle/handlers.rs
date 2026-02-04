//! Oracle instruction handlers for Polymarket sole oracle
//!
//! Handles all oracle-related instructions including initialization,
//! price updates, spread checks, and halt management.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    rent::Rent,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    integration::polymarket_sole_oracle::{
        PolymarketSoleOracle, PolymarketPriceData, HaltReason,
        SPREAD_HALT_THRESHOLD_BPS, POLYMARKET_POLL_INTERVAL_SLOTS,
    },
    events::{emit_event, EventType, OracleInitializedEvent, MarketHaltedEvent, MarketResumedEvent, PriceUpdateProcessed},
    cpi::system_program::create_pda_account,
};

/// Initialize Polymarket as the sole oracle
pub fn process_initialize_polymarket_sole_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    authority: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let oracle_account = next_account_info(account_info_iter)?;
    let signer = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify signer
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Derive PDA for oracle account
    let (oracle_pda, bump) = Pubkey::find_program_address(
        &[b"polymarket_sole_oracle"],
        program_id,
    );
    
    if oracle_account.key != &oracle_pda {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Create oracle account if needed
    if oracle_account.data_is_empty() {
        let space = PolymarketSoleOracle::SIZE;
        let rent = Rent::from_account_info(rent_sysvar)?;
        let lamports = rent.minimum_balance(space);
        
        create_pda_account(
            signer,
            oracle_account,
            space as u64,
            program_id,
            system_program,
            rent_sysvar,
            &[b"polymarket_sole_oracle", &[bump]],
        )?;
    }
    
    // Initialize oracle
    let mut oracle = PolymarketSoleOracle::try_from_slice(&oracle_account.data.borrow())?;
    oracle.initialize(authority)?;
    oracle.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    
    msg!("Polymarket sole oracle initialized with authority: {}", authority);
    
    // Emit event
    let event_data = OracleInitializedEvent {
        oracle_type: "PolymarketSole".to_string(),
        admin: *authority,
        timestamp: Clock::get()?.unix_timestamp,
    };
    emit_event(EventType::OracleInitialized, &event_data);
    
    Ok(())
}

/// Update price from Polymarket
pub fn process_update_polymarket_price(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 16],
    yes_price: u64,
    no_price: u64,
    volume_24h: u64,
    liquidity: u64,
    timestamp: i64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let oracle_account = next_account_info(account_info_iter)?;
    let price_data_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Load oracle
    let mut oracle = PolymarketSoleOracle::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify authority
    if !authority.is_signer || authority.key != &oracle.authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock_sysvar)?;
    let current_slot = clock.slot;
    
    // Check polling interval
    if !oracle.should_poll(current_slot) {
        return Err(BettingPlatformError::UpdateTooFrequent.into());
    }
    
    // Create price data
    let mut price_data = if price_data_account.data_is_empty() {
        PolymarketPriceData {
            market_id,
            yes_price,
            no_price,
            last_update_slot: current_slot,
            last_update_timestamp: timestamp,
            volume_24h,
            liquidity,
            is_halted: false,
            halt_reason: HaltReason::None,
        }
    } else {
        PolymarketPriceData::try_from_slice(&price_data_account.data.borrow())?
    };
    
    // Update values
    price_data.yes_price = yes_price;
    price_data.no_price = no_price;
    price_data.volume_24h = volume_24h;
    price_data.liquidity = liquidity;
    price_data.last_update_timestamp = timestamp;
    
    // Process price update (checks spread and staleness)
    oracle.process_price_update(&mut price_data, current_slot)?;
    oracle.update_poll_time(current_slot);
    
    // Save state
    oracle.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    price_data.serialize(&mut &mut price_data_account.data.borrow_mut()[..])?;
    
    msg!("Price updated for market {:?}: yes={}, no={}", 
         market_id, yes_price, no_price);
    
    // Emit event
    let market_id_32 = {
        let mut id = [0u8; 32];
        id[..16].copy_from_slice(&market_id);
        id
    };
    let event_data = PriceUpdateProcessed {
        market_id: market_id_32,
        keeper_id: [0u8; 32], // System update, no specific keeper
        timestamp: Clock::get()?.unix_timestamp,
    };
    emit_event(EventType::PriceUpdated, &event_data);
    
    Ok(())
}

/// Check and handle price spread
pub fn process_check_price_spread(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 16],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let oracle_account = next_account_info(account_info_iter)?;
    let price_data_account = next_account_info(account_info_iter)?;
    
    // Load data
    let oracle = PolymarketSoleOracle::try_from_slice(&oracle_account.data.borrow())?;
    let mut price_data = PolymarketPriceData::try_from_slice(&price_data_account.data.borrow())?;
    
    // Calculate spread
    let total_prob = price_data.yes_price + price_data.no_price;
    let spread = if total_prob > 10000 {
        total_prob - 10000
    } else {
        10000 - total_prob
    };
    
    // Check if spread exceeds threshold
    if spread > SPREAD_HALT_THRESHOLD_BPS as u64 {
        price_data.is_halted = true;
        price_data.halt_reason = HaltReason::SpreadTooHigh;
        
        // Save updated state
        price_data.serialize(&mut &mut price_data_account.data.borrow_mut()[..])?;
        
        msg!("Market {:?} halted due to {} bps spread", market_id, spread);
        
        // Emit event
        let market_id_u128 = u128::from_le_bytes(market_id);
        emit_event(EventType::MarketHalted, &MarketHaltedEvent {
            market_id: market_id_u128,
            reason: format!("SpreadTooHigh: {} bps", spread),
            timestamp: Clock::get()?.unix_timestamp,
        });
    }
    
    Ok(())
}

/// Reset oracle halt status
pub fn process_reset_oracle_halt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 16],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let oracle_account = next_account_info(account_info_iter)?;
    let price_data_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    // Load oracle
    let mut oracle = PolymarketSoleOracle::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify authority
    if !authority.is_signer || authority.key != &oracle.authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load and update price data
    let mut price_data = PolymarketPriceData::try_from_slice(&price_data_account.data.borrow())?;
    
    // Reset halt status
    oracle.set_halt_status(&mut price_data, false, HaltReason::None)?;
    
    // Save state
    oracle.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    price_data.serialize(&mut &mut price_data_account.data.borrow_mut()[..])?;
    
    msg!("Oracle halt reset for market {:?}", market_id);
    
    // Emit event
    let market_id_u128 = u128::from_le_bytes(market_id);
    emit_event(EventType::MarketResumed, &MarketResumedEvent {
        market_id: market_id_u128,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Halt market due to excessive spread
pub fn process_halt_market_due_to_spread(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 16],
) -> ProgramResult {
    // This functionality is already handled in process_check_price_spread
    // We'll delegate to that function
    process_check_price_spread(program_id, accounts, market_id)
}

/// Unhalt market after spread normalizes
pub fn process_unhalt_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 16],
) -> ProgramResult {
    // This functionality is already handled in process_reset_oracle_halt
    // We'll delegate to that function
    process_reset_oracle_halt(program_id, accounts, market_id)
}