//! Native instruction processor for the betting platform
//!
//! This module handles all instruction processing using native Solana patterns
//! instead of Anchor framework.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    errors::ErrorCode,
    state::*,
    account_structs::*,
    events::*,
};

/// Instruction enum for the betting platform
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum BettingInstruction {
    /// Initialize the platform
    Initialize { seed: u128 },
    
    /// Initialize genesis parameters
    InitializeGenesis,
    
    /// Initialize MMT token
    InitializeMmt,
    
    /// Atomic genesis initialization
    GenesisAtomic,
    
    /// Emergency halt (within 100 slots of genesis)
    EmergencyHalt,
    
    /// Initialize price cache
    InitializePriceCache { verse_id: u128 },
    
    /// Update price cache
    UpdatePriceCache { verse_id: u128, new_price: u64 },
    
    /// Process resolution
    ProcessResolution { 
        verse_id: u128, 
        market_id: String, 
        resolution_outcome: String 
    },
    
    /// Open trading position
    OpenPosition {
        verse_id: u128,
        outcome: u8,
        amount: u64,
        leverage: u8,
    },
    
    /// Close trading position
    ClosePosition {
        position_index: u8,
    },
    
    /// Initialize LMSR market
    InitializeLmsr {
        market_id: u128,
        b_parameter: u64,
        num_outcomes: u8,
    },
    
    /// Execute LMSR trade
    ExecuteLmsrTrade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
    
    /// Initialize PM-AMM market
    InitializePmAmm {
        market_id: u128,
        l_parameter: u64,
        expiry_time: i64,
        initial_price: u64,
    },
    
    /// Execute PM-AMM trade
    ExecutePmAmmTrade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
    
    /// Initialize L2 AMM market
    InitializeL2Amm {
        market_id: u128,
        k_parameter: u64,
        b_bound: u64,
        distribution_type: u8,
        discretization_points: u16,
        range_min: u64,
        range_max: u64,
    },
    
    /// Execute L2 trade
    ExecuteL2Trade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
}

/// Process instruction using native Solana patterns
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = BettingInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    
    msg!("Processing instruction: {:?}", instruction);
    
    match instruction {
        BettingInstruction::Initialize { seed } => {
            process_initialize(program_id, accounts, seed)
        },
        BettingInstruction::InitializeGenesis => {
            process_initialize_genesis(program_id, accounts)
        },
        BettingInstruction::InitializeMmt => {
            process_initialize_mmt(program_id, accounts)
        },
        BettingInstruction::GenesisAtomic => {
            process_genesis_atomic(program_id, accounts)
        },
        BettingInstruction::EmergencyHalt => {
            process_emergency_halt(program_id, accounts)
        },
        BettingInstruction::InitializePriceCache { verse_id } => {
            process_initialize_price_cache(program_id, accounts, verse_id)
        },
        BettingInstruction::UpdatePriceCache { verse_id, new_price } => {
            process_update_price_cache(program_id, accounts, verse_id, new_price)
        },
        BettingInstruction::ProcessResolution { verse_id, market_id, resolution_outcome } => {
            process_resolution(program_id, accounts, verse_id, market_id, resolution_outcome)
        },
        BettingInstruction::OpenPosition { verse_id, outcome, amount, leverage } => {
            process_open_position(program_id, accounts, verse_id, outcome, amount, leverage)
        },
        BettingInstruction::ClosePosition { position_index } => {
            process_close_position(program_id, accounts, position_index)
        },
        BettingInstruction::InitializeLmsr { market_id, b_parameter, num_outcomes } => {
            process_initialize_lmsr(program_id, accounts, market_id, b_parameter, num_outcomes)
        },
        BettingInstruction::ExecuteLmsrTrade { outcome, amount, is_buy } => {
            process_lmsr_trade(program_id, accounts, outcome, amount, is_buy)
        },
        BettingInstruction::InitializePmAmm { market_id, l_parameter, expiry_time, initial_price } => {
            process_initialize_pmamm(program_id, accounts, market_id, l_parameter, expiry_time, initial_price)
        },
        BettingInstruction::ExecutePmAmmTrade { outcome, amount, is_buy } => {
            process_pmamm_trade(program_id, accounts, outcome, amount, is_buy)
        },
        BettingInstruction::InitializeL2Amm { 
            market_id, k_parameter, b_bound, distribution_type, 
            discretization_points, range_min, range_max 
        } => {
            process_initialize_l2amm(
                program_id, accounts, market_id, k_parameter, b_bound, 
                distribution_type, discretization_points, range_min, range_max
            )
        },
        BettingInstruction::ExecuteL2Trade { outcome, amount, is_buy } => {
            process_l2_trade(program_id, accounts, outcome, amount, is_buy)
        },
    }
}

/// Process initialize instruction
fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    seed: u128,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let global_config_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let rent_info = next_account_info(account_info_iter)?;
    
    // Verify program ownership
    if global_config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify authority is signer
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Initialize global config
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_info.data.borrow())?;
    global_config.epoch = 1;
    global_config.coverage = u128::MAX; // Start with infinite coverage
    global_config.vault = 0; // $0 bootstrap
    global_config.total_oi = 0;
    global_config.halt_flag = false;
    global_config.fee_base = 300; // 3bp in basis points (0.03%)
    global_config.fee_slope = 2500; // 25bp
    
    // Serialize back to account
    global_config.serialize(&mut &mut global_config_info.data.borrow_mut()[..])?;
    
    msg!("Platform initialized with seed: {}", seed);
    Ok(())
}

/// Process initialize genesis instruction
fn process_initialize_genesis(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let global_config_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    
    // Verify program ownership
    if global_config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify authority is signer
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = Clock::get()?;
    
    // Initialize genesis parameters
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_info.data.borrow())?;
    global_config.epoch = 1;
    global_config.season = 1;
    global_config.vault = 0;
    global_config.total_oi = 0;
    global_config.coverage = u128::MAX;
    global_config.fee_base = 300;
    global_config.fee_slope = 2500;
    global_config.halt_flag = false;
    global_config.genesis_slot = clock.slot;
    global_config.season_start_slot = clock.slot;
    global_config.season_end_slot = clock.slot + 38_880_000; // ~6 months
    
    // MMT configuration
    global_config.mmt_total_supply = 100_000_000 * 10u64.pow(9); // 100M with 9 decimals
    global_config.mmt_current_season = 10_000_000 * 10u64.pow(9); // 10M for current season
    global_config.mmt_emission_rate = global_config.mmt_current_season / 38_880_000; // Per slot
    
    // Serialize back
    global_config.serialize(&mut &mut global_config_info.data.borrow_mut()[..])?;
    
    // Emit genesis event (in native Solana, we log instead of emit)
    msg!("Genesis initialized at slot {} for epoch {} season {}", 
        clock.slot, global_config.epoch, global_config.season);
    
    Ok(())
}

/// Process emergency halt instruction
fn process_emergency_halt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let global_config_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    
    // Verify program ownership
    if global_config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify authority is signer
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = Clock::get()?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_info.data.borrow())?;
    
    // Only allowed within first 100 slots of genesis
    if clock.slot >= global_config.genesis_slot + 100 {
        return Err(ErrorCode::EmergencyHaltExpired.into());
    }
    
    global_config.halt_flag = true;
    global_config.serialize(&mut &mut global_config_info.data.borrow_mut()[..])?;
    
    msg!("Emergency halt activated at slot {}", clock.slot);
    Ok(())
}

// Additional instruction processors would follow the same pattern...
// For brevity, I'm showing the key pattern conversions

/// Helper function to verify PDA
fn verify_pda(
    expected_seeds: &[&[u8]],
    program_id: &Pubkey,
    pda_info: &AccountInfo,
) -> ProgramResult {
    let (pda, _bump) = Pubkey::find_program_address(expected_seeds, program_id);
    if pda != *pda_info.key {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(())
}

// Stub implementations for remaining processors
fn process_initialize_mmt(_program_id: &Pubkey, _accounts: &[AccountInfo]) -> ProgramResult {
    Ok(())
}

fn process_genesis_atomic(_program_id: &Pubkey, _accounts: &[AccountInfo]) -> ProgramResult {
    Ok(())
}

fn process_initialize_price_cache(_program_id: &Pubkey, _accounts: &[AccountInfo], _verse_id: u128) -> ProgramResult {
    Ok(())
}

fn process_update_price_cache(_program_id: &Pubkey, _accounts: &[AccountInfo], _verse_id: u128, _new_price: u64) -> ProgramResult {
    Ok(())
}

fn process_resolution(_program_id: &Pubkey, _accounts: &[AccountInfo], _verse_id: u128, _market_id: String, _resolution_outcome: String) -> ProgramResult {
    Ok(())
}

fn process_open_position(_program_id: &Pubkey, _accounts: &[AccountInfo], _verse_id: u128, _outcome: u8, _amount: u64, _leverage: u8) -> ProgramResult {
    Ok(())
}

fn process_close_position(_program_id: &Pubkey, _accounts: &[AccountInfo], _position_index: u8) -> ProgramResult {
    Ok(())
}

fn process_initialize_lmsr(_program_id: &Pubkey, _accounts: &[AccountInfo], _market_id: u128, _b_parameter: u64, _num_outcomes: u8) -> ProgramResult {
    Ok(())
}

fn process_lmsr_trade(_program_id: &Pubkey, _accounts: &[AccountInfo], _outcome: u8, _amount: u64, _is_buy: bool) -> ProgramResult {
    Ok(())
}

fn process_initialize_pmamm(_program_id: &Pubkey, _accounts: &[AccountInfo], _market_id: u128, _l_parameter: u64, _expiry_time: i64, _initial_price: u64) -> ProgramResult {
    Ok(())
}

fn process_pmamm_trade(_program_id: &Pubkey, _accounts: &[AccountInfo], _outcome: u8, _amount: u64, _is_buy: bool) -> ProgramResult {
    Ok(())
}

fn process_initialize_l2amm(_program_id: &Pubkey, _accounts: &[AccountInfo], _market_id: u128, _k_parameter: u64, _b_bound: u64, _distribution_type: u8, _discretization_points: u16, _range_min: u64, _range_max: u64) -> ProgramResult {
    Ok(())
}

fn process_l2_trade(_program_id: &Pubkey, _accounts: &[AccountInfo], _outcome: u8, _amount: u64, _is_buy: bool) -> ProgramResult {
    Ok(())
}