//! Process trade for security checks

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::security_accounts::{AttackDetector, AttackType},
};

pub fn process_trade_security(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    size: u64,
    price: u64,
    leverage: u64,
    is_buy: bool,
) -> ProgramResult {
    msg!("Processing trade security check");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let trader = next_account_info(account_info_iter)?;
    let attack_detector_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify trader is signer
    if !trader.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify attack detector PDA
    let (attack_detector_pda, _) = Pubkey::find_program_address(
        &[b"attack_detector"],
        program_id,
    );
    
    if attack_detector_pda != *attack_detector_account.key {
        msg!("Invalid attack detector PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Load and validate attack detector
    let mut detector = AttackDetector::try_from_slice(&attack_detector_account.data.borrow())?;
    detector.validate()?;
    
    // Get current slot from clock
    let clock = Clock::from_account_info(clock)?;
    let current_slot = clock.slot;
    
    // Convert market_id to [u8; 32]
    let mut market_id_bytes = [0u8; 32];
    market_id_bytes[..16].copy_from_slice(&market_id.to_le_bytes());
    
    // Process the trade through attack detector
    match detector.process_trade(
        market_id_bytes,
        *trader.key,
        size,
        price,
        leverage,
        is_buy,
        current_slot,
    ) {
        Ok(attack_type) => {
            if attack_type != AttackType::None {
                msg!("Warning: Potential attack detected - {:?}", attack_type);
            }
            
            // Log trade statistics
            msg!("Trade processed: size={}, price={}, leverage={}, is_buy={}", 
                size, price, leverage, is_buy);
            msg!("Attack detector stats: detected={}, false_positives={}", 
                detector.attacks_detected, detector.false_positives);
        }
        Err(e) => {
            if e == ProgramError::Custom(6094) { // AttackDetected
                msg!("Attack detected! Trader: {}", trader.key);
                msg!("Market ID: {:?}", market_id);
                msg!("Trade details: size={}, price={}, leverage={}", size, price, leverage);
                msg!("Suspicious patterns exceeded threshold");
                
                // Save updated detector state even on attack detection
                detector.serialize(&mut &mut attack_detector_account.data.borrow_mut()[..])?;
                
                return Err(BettingPlatformError::AttackDetected.into());
            } else {
                return Err(e);
            }
        }
    }
    
    // Update alert level based on recent activity
    if detector.attacks_detected > 10 {
        detector.alert_level = crate::state::security_accounts::AlertLevel::Critical;
    } else if detector.attacks_detected > 5 {
        detector.alert_level = crate::state::security_accounts::AlertLevel::High;
    } else if detector.attacks_detected > 2 {
        detector.alert_level = crate::state::security_accounts::AlertLevel::Elevated;
    }
    
    // Save updated detector state
    detector.serialize(&mut &mut attack_detector_account.data.borrow_mut()[..])?;
    
    msg!("Trade security check passed. Alert level: {:?}", detector.alert_level);
    
    Ok(())
}