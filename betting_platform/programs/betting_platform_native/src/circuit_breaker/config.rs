//! Circuit breaker configuration

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::str::FromStr;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::security_accounts::CircuitBreaker,
};

pub fn process_update_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_cooldown_period: Option<u64>,
    new_coverage_halt_duration: Option<u64>,
    new_price_halt_duration: Option<u64>,
    new_volume_halt_duration: Option<u64>,
    new_liquidation_halt_duration: Option<u64>,
    new_congestion_halt_duration: Option<u64>,
    new_oi_rate_halt_duration: Option<u64>,
) -> ProgramResult {
    msg!("Updating circuit breaker configuration");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let circuit_breaker_account = next_account_info(account_info_iter)?;
    let global_config_account = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Production authorization: Verify authority is one of the authorized governance keys
    verify_governance_authority(authority, program_id, global_config_account)?;
    
    // Verify circuit breaker PDA
    let (circuit_breaker_pda, _) = Pubkey::find_program_address(
        &[b"circuit_breaker"],
        program_id,
    );
    
    if circuit_breaker_pda != *circuit_breaker_account.key {
        msg!("Invalid circuit breaker PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Load and validate circuit breaker
    let mut breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
    breaker.validate()?;
    
    // Store old values for logging
    let old_config = (
        breaker.cooldown_period,
        breaker.coverage_halt_duration,
        breaker.price_halt_duration,
        breaker.volume_halt_duration,
        breaker.liquidation_halt_duration,
        breaker.congestion_halt_duration,
        breaker.oi_rate_halt_duration,
    );
    
    // Update cooldown period if provided
    if let Some(cooldown) = new_cooldown_period {
        if cooldown < 30 || cooldown > 1800 { // 12 seconds to 12 minutes
            msg!("Invalid cooldown period: must be between 30 and 1800 slots");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.cooldown_period = cooldown;
    }
    
    // Update halt durations if provided
    if let Some(duration) = new_coverage_halt_duration {
        if duration < 60 || duration > 14400 { // 1 minute to 4 hours
            msg!("Invalid coverage halt duration: must be between 60 and 14400 seconds");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.coverage_halt_duration = duration;
    }
    
    if let Some(duration) = new_price_halt_duration {
        if duration < 60 || duration > 7200 { // 1 minute to 2 hours
            msg!("Invalid price halt duration: must be between 60 and 7200 seconds");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.price_halt_duration = duration;
    }
    
    if let Some(duration) = new_volume_halt_duration {
        if duration < 60 || duration > 10800 { // 1 minute to 3 hours
            msg!("Invalid volume halt duration: must be between 60 and 10800 seconds");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.volume_halt_duration = duration;
    }
    
    if let Some(duration) = new_liquidation_halt_duration {
        if duration < 60 || duration > 14400 { // 1 minute to 4 hours
            msg!("Invalid liquidation halt duration: must be between 60 and 14400 seconds");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.liquidation_halt_duration = duration;
    }
    
    if let Some(duration) = new_congestion_halt_duration {
        if duration < 30 || duration > 3600 { // 30 seconds to 1 hour
            msg!("Invalid congestion halt duration: must be between 30 and 3600 seconds");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.congestion_halt_duration = duration;
    }
    
    if let Some(duration) = new_oi_rate_halt_duration {
        if duration < 60 || duration > 7200 { // 1 minute to 2 hours
            msg!("Invalid OI rate halt duration: must be between 60 and 7200 seconds");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        breaker.oi_rate_halt_duration = duration;
    }
    
    // Log configuration changes
    msg!("Circuit breaker configuration updated:");
    if breaker.cooldown_period != old_config.0 {
        msg!("  Cooldown period: {} -> {} slots", old_config.0, breaker.cooldown_period);
    }
    if breaker.coverage_halt_duration != old_config.1 {
        msg!("  Coverage halt: {} -> {} seconds", old_config.1, breaker.coverage_halt_duration);
    }
    if breaker.price_halt_duration != old_config.2 {
        msg!("  Price halt: {} -> {} seconds", old_config.2, breaker.price_halt_duration);
    }
    if breaker.volume_halt_duration != old_config.3 {
        msg!("  Volume halt: {} -> {} seconds", old_config.3, breaker.volume_halt_duration);
    }
    if breaker.liquidation_halt_duration != old_config.4 {
        msg!("  Liquidation halt: {} -> {} seconds", old_config.4, breaker.liquidation_halt_duration);
    }
    if breaker.congestion_halt_duration != old_config.5 {
        msg!("  Congestion halt: {} -> {} seconds", old_config.5, breaker.congestion_halt_duration);
    }
    if breaker.oi_rate_halt_duration != old_config.6 {
        msg!("  OI rate halt: {} -> {} seconds", old_config.6, breaker.oi_rate_halt_duration);
    }
    
    // Save updated configuration
    breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    
    msg!("Circuit breaker configuration updated successfully");
    
    Ok(())
}

/// Verify that the authority is an authorized governance key
fn verify_governance_authority(
    authority: &AccountInfo,
    program_id: &Pubkey,
    global_config_account: &AccountInfo,
) -> Result<(), ProgramError> {
    // Load global config to check update authority
    let global_config = crate::state::GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Verify authority is the platform's update authority
    if authority.key != &global_config.update_authority {
        msg!("Unauthorized: {} is not the update authority", authority.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Additional check: derive governance PDA for circuit breaker operations
    let (governance_pda, _) = Pubkey::find_program_address(
        &[b"governance", b"circuit_breaker"],
        program_id,
    );
    
    // Allow either update authority or governance PDA
    if authority.key == &governance_pda || authority.key == &global_config.update_authority {
        return Ok(());
    }
    
    msg!("Unauthorized: {} is not an authorized governance signer", authority.key);
    Err(BettingPlatformError::UnauthorizedAccess.into())
}