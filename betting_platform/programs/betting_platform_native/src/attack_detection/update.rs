//! Update attack detection baseline

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::security_accounts::AttackDetector,
};

pub fn process_update_baseline(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_avg_volume: u64,
    new_std_dev: u64,
) -> ProgramResult {
    msg!("Updating volume baseline for attack detection");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let attack_detector_account = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
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
    
    // Validate new baseline values
    if new_avg_volume == 0 {
        msg!("Invalid average volume: cannot be zero");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if new_std_dev > new_avg_volume {
        msg!("Invalid standard deviation: cannot exceed average volume");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Store old values for logging
    let old_avg_volume = detector.avg_volume_baseline;
    let old_std_dev = detector.volume_std_dev;
    
    // Update baseline values
    detector.avg_volume_baseline = new_avg_volume;
    detector.volume_std_dev = new_std_dev;
    
    // Calculate percentage changes for monitoring
    let avg_change_pct = if old_avg_volume > 0 {
        ((new_avg_volume as i128 - old_avg_volume as i128).abs() * 100) / old_avg_volume as i128
    } else {
        100
    };
    
    // Log the update
    msg!("Volume baseline updated:");
    msg!("  Average volume: {} -> {} ({}% change)", 
        old_avg_volume, new_avg_volume, avg_change_pct);
    msg!("  Standard deviation: {} -> {}", old_std_dev, new_std_dev);
    
    // Adjust thresholds based on new baseline if needed
    if new_avg_volume > old_avg_volume * 2 || new_avg_volume < old_avg_volume / 2 {
        // Significant change in baseline, adjust flash loan threshold
        detector.flash_loan_threshold = new_avg_volume.saturating_mul(100); // 100x average as threshold
        msg!("Adjusted flash loan threshold to: {}", detector.flash_loan_threshold);
    }
    
    // Save updated detector state
    detector.serialize(&mut &mut attack_detector_account.data.borrow_mut()[..])?;
    
    msg!("Attack detector baseline updated successfully");
    
    Ok(())
}