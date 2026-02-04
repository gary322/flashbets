//! Attack detector initialization

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
    state::AttackDetector,
};

pub fn process_initialize_detector(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing attack detector");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let initializer = next_account_info(account_info_iter)?; // Authority
    let attack_detector_account = next_account_info(account_info_iter)?; // PDA for attack detector
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    
    // Verify initializer is signer
    if !initializer.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive attack detector PDA
    let (attack_detector_pda, bump_seed) = Pubkey::find_program_address(
        &[b"attack_detector"],
        program_id,
    );
    
    // Verify PDA matches
    if attack_detector_pda != *attack_detector_account.key {
        msg!("Invalid attack detector PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if already initialized
    if attack_detector_account.data_len() > 0 {
        msg!("Attack detector already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate required space
    let detector_size = std::mem::size_of::<AttackDetector>() + 
        AttackDetector::TRADE_BUFFER_SIZE * std::mem::size_of::<crate::state::security_accounts::TradeRecord>() +
        10 * 32; // Space for 10 suspicious addresses
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(detector_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            attack_detector_account.key,
            rent_lamports,
            detector_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            attack_detector_account.clone(),
            system_program.clone(),
        ],
        &[&[b"attack_detector", &[bump_seed]]],
    )?;
    
    // Initialize attack detector
    let mut detector = AttackDetector::new();
    
    // Serialize and save
    detector.serialize(&mut &mut attack_detector_account.data.borrow_mut()[..])?;
    
    msg!("Attack detector initialized successfully");
    msg!("Detection window: {} slots", detector.detection_window);
    msg!("Pattern threshold: {}", detector.pattern_threshold);
    msg!("Flash loan threshold: {} lamports", detector.flash_loan_threshold);
    msg!("Wash trade threshold: {} bps", detector.wash_trade_threshold);
    
    Ok(())
}