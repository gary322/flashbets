//! Reset attack detector

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
    state::security_accounts::{AttackDetector, AlertLevel},
};

pub fn process_reset_detector(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Resetting attack detector");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let attack_detector_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Production authorization: Verify authority is the platform's update authority
    let global_config_account = next_account_info(account_info_iter)?;
    let global_config = crate::state::GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    if global_config.update_authority != *authority.key {
        msg!("Unauthorized: {} is not the update authority", authority.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
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
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    let current_slot = clock.slot;
    
    // Store statistics before reset for logging
    let stats_before = (
        detector.attacks_detected,
        detector.false_positives,
        detector.suspicious_addresses.len(),
        detector.alert_level,
    );
    
    // Preserve baseline values but reset detection state
    let preserved_avg_volume = detector.avg_volume_baseline;
    let preserved_std_dev = detector.volume_std_dev;
    let preserved_thresholds = (
        detector.pattern_threshold,
        detector.flash_loan_threshold,
        detector.wash_trade_threshold,
    );
    
    // Create new detector but preserve important configuration
    let mut new_detector = AttackDetector::new();
    new_detector.avg_volume_baseline = preserved_avg_volume;
    new_detector.volume_std_dev = preserved_std_dev;
    new_detector.pattern_threshold = preserved_thresholds.0;
    new_detector.flash_loan_threshold = preserved_thresholds.1;
    new_detector.wash_trade_threshold = preserved_thresholds.2;
    
    // Log reset information
    msg!("Attack detector reset:");
    msg!("  Previous stats - Attacks: {}, False positives: {}, Suspicious addresses: {}", 
        stats_before.0, stats_before.1, stats_before.2);
    msg!("  Previous alert level: {:?}", stats_before.3);
    msg!("  Preserved volume baseline: {}", preserved_avg_volume);
    msg!("  Preserved std dev: {}", preserved_std_dev);
    msg!("  Reset at slot: {}", current_slot);
    
    // Optionally, increment false positives if this was a manual reset due to false alarms
    if stats_before.0 > 0 && stats_before.3 as u8 >= AlertLevel::High as u8 {
        new_detector.false_positives = stats_before.1 + 1;
        msg!("Incremented false positive count to: {}", new_detector.false_positives);
    }
    
    // Save reset detector state
    new_detector.serialize(&mut &mut attack_detector_account.data.borrow_mut()[..])?;
    
    msg!("Attack detector reset successfully");
    
    Ok(())
}