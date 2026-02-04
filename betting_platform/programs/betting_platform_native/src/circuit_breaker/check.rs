//! Circuit breaker checks

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

pub fn process_check_breakers(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    coverage: u64,
    liquidation_count: u32,
    liquidation_volume: u64,
    total_oi: u64,
    failed_tx: u32,
) -> ProgramResult {
    msg!("Checking circuit breakers");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let circuit_breaker_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
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
    let current_slot = clock.slot;
    let current_time = clock.unix_timestamp;
    
    // First check if any breakers have expired and should be deactivated
    let expired_breakers = breaker.check_expired_breakers(current_time);
    if !expired_breakers.is_empty() {
        msg!("Deactivating expired circuit breakers:");
        for breaker_type in &expired_breakers {
            msg!("  - {:?}", breaker_type);
        }
    }
    
    // Check if system is already halted
    if breaker.is_halted() {
        msg!("System is currently halted. Active breakers:");
        if breaker.coverage_breaker_active {
            msg!("  - Coverage breaker");
        }
        if breaker.price_breaker_active {
            msg!("  - Price breaker");
        }
        if breaker.volume_breaker_active {
            msg!("  - Volume breaker");
        }
        if breaker.liquidation_breaker_active {
            msg!("  - Liquidation breaker");
        }
        if breaker.congestion_breaker_active {
            msg!("  - Congestion breaker");
        }
        if breaker.oi_rate_breaker_active {
            msg!("  - OI rate breaker");
        }
        
        // Save updated state if breakers were deactivated
        if !expired_breakers.is_empty() {
            breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        }
        
        return Err(BettingPlatformError::CircuitBreakerTriggered.into());
    }
    
    // Log current metrics
    msg!("Circuit breaker metrics:");
    msg!("  Coverage: {} (threshold: {})", coverage, breaker.coverage_threshold);
    msg!("  Liquidation count: {} (threshold: {})", liquidation_count, breaker.liquidation_cascade_threshold);
    msg!("  Failed transactions: {}/{} ({}%)", failed_tx, total_oi, 
        if total_oi > 0 { (failed_tx as u64 * 100) / total_oi } else { 0 });
    
    // Check for new breaker triggers
    let triggered = breaker.check_and_trigger(
        coverage,
        liquidation_count as u64,
        liquidation_volume,
        total_oi,
        failed_tx as u64,
        current_slot,
        current_time,
    )?;
    
    if !triggered.is_empty() {
        msg!("CIRCUIT BREAKERS TRIGGERED:");
        for breaker_type in &triggered {
            match breaker_type {
                BreakerType::Coverage => {
                    msg!("  - Coverage breaker: Coverage {} < {}", coverage, breaker.coverage_threshold);
                }
                BreakerType::Liquidation => {
                    msg!("  - Liquidation breaker: {} liquidations exceed threshold {}", 
                        liquidation_count, breaker.liquidation_cascade_threshold);
                }
                BreakerType::Congestion => {
                    let congestion_rate = if total_oi > 0 {
                        (failed_tx as u64 * 10000) / total_oi
                    } else {
                        0
                    };
                    msg!("  - Congestion breaker: {}bp failed tx rate exceeds {}bp", 
                        congestion_rate, breaker.congestion_threshold);
                }
                BreakerType::Price => {
                    msg!("  - Price breaker: Price volatility exceeded limits");
                }
                BreakerType::Volume => {
                    msg!("  - Volume breaker: Trading volume exceeded threshold");
                }
                BreakerType::OracleFailure => {
                    msg!("  - Oracle breaker: Oracle service failure detected");
                }
                BreakerType::OIRate => {
                    msg!("  - OI rate breaker: Open interest rate {} bps/slot exceeds {} bps threshold", 
                        "N/A", breaker.oi_rate_threshold);
                }
            }
        }
        
        // Save updated breaker state
        breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        
        // Halt the system
        return Err(BettingPlatformError::CircuitBreakerTriggered.into());
    }
    
    // Save updated state (in case expired breakers were deactivated)
    breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    
    msg!("All circuit breakers passed");
    
    Ok(())
}

/// Process advanced circuit breaker checks (called by CheckAdvancedBreakers instruction)
pub fn process_check_advanced_breakers(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    coverage: u64,
    liquidation_count: u64,
    liquidation_volume: u64,
    total_oi: u64,
    failed_tx: u64,
    oi_rate_per_slot: u64,
) -> ProgramResult {
    msg!("Checking advanced circuit breakers");
    
    // We need to call check_and_trigger_with_oi_rate directly for the OI rate parameter
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let circuit_breaker_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
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
    let current_slot = clock.slot;
    let current_time = clock.unix_timestamp;
    
    // First check if any breakers have expired and should be deactivated
    let expired_breakers = breaker.check_expired_breakers(current_time);
    if !expired_breakers.is_empty() {
        msg!("Deactivating expired circuit breakers:");
        for breaker_type in &expired_breakers {
            msg!("  - {:?}", breaker_type);
        }
    }
    
    // Check if system is already halted
    if breaker.is_halted() {
        msg!("System is currently halted. Active breakers:");
        if breaker.coverage_breaker_active {
            msg!("  - Coverage breaker");
        }
        if breaker.price_breaker_active {
            msg!("  - Price breaker");
        }
        if breaker.volume_breaker_active {
            msg!("  - Volume breaker");
        }
        if breaker.liquidation_breaker_active {
            msg!("  - Liquidation breaker");
        }
        if breaker.congestion_breaker_active {
            msg!("  - Congestion breaker");
        }
        if breaker.oi_rate_breaker_active {
            msg!("  - OI rate breaker");
        }
        
        // Save updated state if breakers were deactivated
        if !expired_breakers.is_empty() {
            breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        }
        
        return Err(BettingPlatformError::CircuitBreakerTriggered.into());
    }
    
    // Log current metrics
    msg!("Circuit breaker metrics:");
    msg!("  Coverage: {} (threshold: {})", coverage, breaker.coverage_threshold);
    msg!("  Liquidation count: {} (threshold: {})", liquidation_count, breaker.liquidation_cascade_threshold);
    msg!("  Failed transactions: {}/{} ({}%)", failed_tx, total_oi, 
        if total_oi > 0 { (failed_tx * 100) / total_oi } else { 0 });
    msg!("  OI rate: {} bps/slot (threshold: {} bps)", oi_rate_per_slot, breaker.oi_rate_threshold);
    
    // Check for new breaker triggers with OI rate
    let triggered = breaker.check_and_trigger_with_oi_rate(
        coverage,
        liquidation_count,
        liquidation_volume,
        total_oi,
        failed_tx,
        oi_rate_per_slot,
        current_slot,
        current_time,
    )?;
    
    if !triggered.is_empty() {
        msg!("CIRCUIT BREAKERS TRIGGERED:");
        for breaker_type in &triggered {
            match breaker_type {
                BreakerType::Coverage => {
                    msg!("  - Coverage breaker: Coverage {} < {}", coverage, breaker.coverage_threshold);
                }
                BreakerType::Liquidation => {
                    msg!("  - Liquidation breaker: {} liquidations exceed threshold {}", 
                        liquidation_count, breaker.liquidation_cascade_threshold);
                }
                BreakerType::Congestion => {
                    let congestion_rate = if total_oi > 0 {
                        (failed_tx * 10000) / total_oi
                    } else {
                        0
                    };
                    msg!("  - Congestion breaker: {}bp failed tx rate exceeds {}bp", 
                        congestion_rate, breaker.congestion_threshold);
                }
                BreakerType::Price => {
                    msg!("  - Price breaker: Price volatility exceeded limits");
                }
                BreakerType::Volume => {
                    msg!("  - Volume breaker: Trading volume exceeded threshold");
                }
                BreakerType::OracleFailure => {
                    msg!("  - Oracle breaker: Oracle service failure detected");
                }
                BreakerType::OIRate => {
                    msg!("  - OI rate breaker: {} bps/slot exceeds {} bps threshold", 
                        oi_rate_per_slot, breaker.oi_rate_threshold);
                }
            }
        }
        
        // Save updated breaker state
        breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        
        // Halt the system
        return Err(BettingPlatformError::CircuitBreakerTriggered.into());
    }
    
    // Save updated state (in case expired breakers were deactivated)
    breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    
    msg!("All circuit breakers passed");
    
    Ok(())
}