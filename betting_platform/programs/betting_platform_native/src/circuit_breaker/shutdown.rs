//! Emergency shutdown

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
    state::security_accounts::{CircuitBreaker, BreakerType},
};

pub fn process_emergency_shutdown(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Processing emergency shutdown");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let circuit_breaker_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Production authorization: Verify authority is the platform's update authority
    // Load global config after getting it from accounts
    let global_config_account_for_auth = next_account_info(account_info_iter)?;
    let global_config = crate::state::GlobalConfigPDA::try_from_slice(&global_config_account_for_auth.data.borrow())?;
    if global_config.update_authority != *authority.key {
        msg!("Unauthorized: {} is not the update authority", authority.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
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
    
    // Get current time
    let clock = Clock::from_account_info(clock)?;
    let current_time = clock.unix_timestamp;
    let current_slot = clock.slot;
    
    // Log current state before shutdown
    msg!("Emergency shutdown initiated by: {}", authority.key);
    msg!("Current breaker states:");
    msg!("  Coverage: {}", if breaker.coverage_breaker_active { "ACTIVE" } else { "inactive" });
    msg!("  Price: {}", if breaker.price_breaker_active { "ACTIVE" } else { "inactive" });
    msg!("  Volume: {}", if breaker.volume_breaker_active { "ACTIVE" } else { "inactive" });
    msg!("  Liquidation: {}", if breaker.liquidation_breaker_active { "ACTIVE" } else { "inactive" });
    msg!("  Congestion: {}", if breaker.congestion_breaker_active { "ACTIVE" } else { "inactive" });
    
    // Activate all circuit breakers for emergency shutdown
    let emergency_duration = 3600; // 1 hour (in seconds)
    
    // Coverage breaker
    if !breaker.coverage_breaker_active {
        breaker.coverage_breaker_active = true;
        breaker.coverage_activated_at = Some(current_time);
        // Override with longer emergency duration
        breaker.coverage_halt_duration = emergency_duration;
    }
    
    // Price breaker
    if !breaker.price_breaker_active {
        breaker.price_breaker_active = true;
        breaker.price_activated_at = Some(current_time);
        breaker.price_halt_duration = emergency_duration;
    }
    
    // Volume breaker
    if !breaker.volume_breaker_active {
        breaker.volume_breaker_active = true;
        breaker.volume_activated_at = Some(current_time);
        breaker.volume_halt_duration = emergency_duration;
    }
    
    // Liquidation breaker
    if !breaker.liquidation_breaker_active {
        breaker.liquidation_breaker_active = true;
        breaker.liquidation_activated_at = Some(current_time);
        breaker.liquidation_halt_duration = emergency_duration;
    }
    
    // Congestion breaker
    if !breaker.congestion_breaker_active {
        breaker.congestion_breaker_active = true;
        breaker.congestion_activated_at = Some(current_time);
        breaker.congestion_halt_duration = emergency_duration;
    }
    
    // Update trigger statistics
    breaker.last_trigger_slot = current_slot;
    breaker.total_triggers += 1; // Count emergency shutdown as a trigger
    
    // Save updated breaker state
    breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    
    msg!("EMERGENCY SHUTDOWN COMPLETE");
    msg!("All circuit breakers activated for {} seconds", emergency_duration);
    msg!("System will remain halted until {}", current_time + emergency_duration as i64);
    msg!("Manual intervention required to resume operations");
    
    Ok(())
}