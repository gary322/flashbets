//! Circuit breaker implementation

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

pub fn process_check_circuit_breakers(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    price_movement: i64,
) -> ProgramResult {
    msg!("Checking circuit breakers for price movement");
    
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
    let current_time = clock.unix_timestamp;
    
    // Check if system is already halted
    if breaker.is_halted() {
        msg!("System is currently halted due to active circuit breakers");
        return Err(BettingPlatformError::CircuitBreakerTriggered.into());
    }
    
    // Calculate absolute price movement in basis points
    let price_movement_bp = price_movement.abs() as u64;
    
    msg!("Price movement check: {} bp (threshold: {} bp)", 
        price_movement_bp, breaker.price_movement_threshold);
    
    // Check if price movement exceeds threshold
    if price_movement_bp > breaker.price_movement_threshold as u64 {
        msg!("PRICE CIRCUIT BREAKER TRIGGERED!");
        msg!("Price movement {} bp exceeds threshold {} bp", 
            price_movement_bp, breaker.price_movement_threshold);
        
        // Activate price breaker
        breaker.price_breaker_active = true;
        breaker.price_activated_at = Some(current_time);
        breaker.last_trigger_slot = clock.slot;
        breaker.total_triggers += 1;
        
        // Save updated state
        breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        
        return Err(BettingPlatformError::ExcessivePriceMovement.into());
    }
    
    // Check and deactivate expired breakers
    let expired = breaker.check_expired_breakers(current_time);
    if !expired.is_empty() {
        msg!("Deactivating {} expired circuit breakers", expired.len());
        breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    }
    
    msg!("Price movement within acceptable range");
    
    Ok(())
}