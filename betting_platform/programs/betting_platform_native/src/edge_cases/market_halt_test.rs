//! Market Halt Edge Case Testing
//! 
//! Tests behavior when markets are halted due to coverage or circuit breaker

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{GlobalConfigPDA, ProposalPDA, VersePDA, VerseStatus, security_accounts::{CircuitBreaker, CircuitBreakerType}},
    events::{emit_event, EventType, MarketResumedEvent, MarketHaltedEvent, EmergencyHaltEvent},
};

/// Test market halt due to low coverage
pub fn test_coverage_based_halt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let global_config_account = next_account_info(account_iter)?;
    let verse_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let circuit_breaker_account = next_account_info(account_iter)?;
    
    msg!("Testing coverage-based market halt");
    
    // Load accounts
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    let mut verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut circuit_breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
    
    // Step 1: Calculate current coverage
    msg!("Step 1: Calculating platform coverage");
    let coverage_ratio = if global_config.total_oi > 0 {
        (global_config.vault * 10000) / global_config.total_oi
    } else {
        10000 // 100% if no OI
    };
    
    msg!("Current coverage: {} bps ({}%)", coverage_ratio, coverage_ratio / 100);
    msg!("Vault: {}, Total OI: {}", global_config.vault, global_config.total_oi);
    
    // Step 2: Check if coverage below threshold (50%)
    const HALT_THRESHOLD_BPS: u128 = 5000;
    
    if coverage_ratio < HALT_THRESHOLD_BPS {
        msg!("Step 2: Coverage below threshold - triggering halt");
        
        // Update verse status
        verse.status = VerseStatus::Halted;
        verse.last_update_slot = Clock::get()?.slot;
        
        // Halt all proposals in verse
        proposal.state = crate::state::ProposalState::Paused;
        
        // Activate circuit breaker
        circuit_breaker.is_active = true;
        circuit_breaker.breaker_type = Some(CircuitBreakerType::Coverage);
        circuit_breaker.triggered_at = Some(Clock::get()?.slot);
        circuit_breaker.triggered_by = Some(*global_config_account.key);
        circuit_breaker.reason = Some(format!("Coverage {} < {} bps", coverage_ratio, HALT_THRESHOLD_BPS));
        
        // Save state
        verse.serialize(&mut &mut verse_account.data.borrow_mut()[..])?;
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
        circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        
        // Emit halt event
        emit_event(EventType::MarketHalted, &MarketHaltedEvent {
            market_id: u128::from_le_bytes(proposal.market_id[0..16].try_into().unwrap()),
            reason: "Low coverage ratio".to_string(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("Market halted due to low coverage!");
        
        // Step 3: Test trading attempt on halted market
        msg!("Step 3: Testing trade attempt on halted market");
        
        // This should fail
        match attempt_trade_on_halted_market(&proposal) {
            Err(BettingPlatformError::MarketHalted) => {
                msg!("✓ Trade correctly rejected on halted market");
            }
            Ok(_) => {
                msg!("✗ ERROR: Trade succeeded on halted market!");
                return Err(ProgramError::InvalidAccountData);
            }
            Err(e) => {
                msg!("✗ Unexpected error: {:?}", e);
                return Err(e.into());
            }
        }
        
        // Step 4: Test recovery when coverage improves
        msg!("Step 4: Simulating coverage improvement");
        
        // Simulate vault increase
        global_config.vault = global_config.total_oi * 7 / 10; // 70% coverage
        let new_coverage = (global_config.vault * 10000) / global_config.total_oi;
        
        if new_coverage >= HALT_THRESHOLD_BPS {
            msg!("Coverage improved to {} bps - lifting halt", new_coverage);
            
            // Restore market
            verse.status = VerseStatus::Active;
            proposal.state = crate::state::ProposalState::Active;
            circuit_breaker.is_active = false;
            circuit_breaker.resolved_at = Some(Clock::get()?.slot as i64);
            
            emit_event(EventType::MarketResumed, &MarketResumedEvent {
                market_id: u128::from_le_bytes(proposal.market_id[0..16].try_into().unwrap()),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
        
        // Save updated state
        global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
        verse.serialize(&mut &mut verse_account.data.borrow_mut()[..])?;
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
        circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    } else {
        msg!("Coverage healthy at {} bps - no halt needed", coverage_ratio);
    }
    
    msg!("Coverage-based halt test completed");
    
    Ok(())
}

/// Test circuit breaker activation
pub fn test_circuit_breaker_activation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    trigger_type: CircuitBreakerTrigger,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let circuit_breaker_account = next_account_info(account_iter)?;
    let verse_account = next_account_info(account_iter)?;
    let authority_account = next_account_info(account_iter)?;
    
    msg!("Testing circuit breaker activation: {:?}", trigger_type);
    
    // Load accounts
    let mut circuit_breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
    let mut verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
    
    // Verify authority
    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    match trigger_type {
        CircuitBreakerTrigger::PriceVolatility { change_bps } => {
            msg!("Testing price volatility trigger: {} bps change", change_bps);
            
            if change_bps > 2000 { // 20% threshold
                msg!("Price change exceeds threshold - activating circuit breaker");
                
                circuit_breaker.is_active = true;
                circuit_breaker.breaker_type = Some(CircuitBreakerType::Price);
                circuit_breaker.triggered_at = Some(Clock::get()?.slot);
                circuit_breaker.triggered_by = Some(*authority_account.key);
                circuit_breaker.reason = Some(format!("Price volatility {} bps", change_bps));
                
                // Halt verse
                verse.status = VerseStatus::Halted;
                
                emit_event(EventType::CircuitBreakerTriggered, &CircuitBreakerTriggeredEvent {
                    verse_id: verse.verse_id,
                    breaker_type: Some(CircuitBreakerType::Price),
                    reason: circuit_breaker.reason.clone(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            }
        }
        
        CircuitBreakerTrigger::LiquidationCascade { liquidation_rate_bps } => {
            msg!("Testing liquidation cascade trigger: {} bps rate", liquidation_rate_bps);
            
            if liquidation_rate_bps > 500 { // 5% threshold
                msg!("Liquidation rate exceeds threshold - activating circuit breaker");
                
                circuit_breaker.is_active = true;
                circuit_breaker.breaker_type = Some(CircuitBreakerType::Liquidation);
                circuit_breaker.triggered_at = Some(Clock::get()?.slot);
                circuit_breaker.triggered_by = Some(*authority_account.key);
                circuit_breaker.reason = Some(format!("Liquidation cascade {} bps", liquidation_rate_bps));
                
                // Halt verse
                verse.status = VerseStatus::Halted;
                
                emit_event(EventType::CircuitBreakerTriggered, &CircuitBreakerTriggeredEvent {
                    verse_id: verse.verse_id,
                    breaker_type: Some(CircuitBreakerType::Liquidation),
                    reason: circuit_breaker.reason.clone(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            }
        }
        
        CircuitBreakerTrigger::OracleFailure => {
            msg!("Testing oracle failure trigger");
            
            circuit_breaker.is_active = true;
            circuit_breaker.breaker_type = Some(CircuitBreakerType::OracleFailure);
            circuit_breaker.triggered_at = Some(Clock::get()?.slot);
            circuit_breaker.triggered_by = Some(*authority_account.key);
            circuit_breaker.reason = Some("Oracle unavailable".to_string());
            
            // Halt verse
            verse.status = VerseStatus::Halted;
            
            emit_event(EventType::CircuitBreakerTriggered, &CircuitBreakerTriggeredEvent {
                verse_id: verse.verse_id,
                breaker_type: Some(CircuitBreakerType::OracleFailure),
                reason: circuit_breaker.reason.clone(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
        
        CircuitBreakerTrigger::EmergencyHalt { reason } => {
            msg!("Testing emergency halt: {}", reason);
            
            circuit_breaker.is_active = true;
            circuit_breaker.breaker_type = Some(CircuitBreakerType::Coverage);
            circuit_breaker.triggered_at = Some(Clock::get()?.slot);
            circuit_breaker.triggered_by = Some(*authority_account.key);
            circuit_breaker.reason = Some(reason);
            
            // Halt verse and all children
            verse.status = VerseStatus::Halted;
            
            emit_event(EventType::EmergencyHaltEvent, &EmergencyHaltEvent {
                slot: Clock::get()?.slot,
                reason: circuit_breaker.reason.clone().unwrap_or_else(|| "Emergency halt".to_string()),
            });
        }
    }
    
    // Save state
    circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    verse.serialize(&mut &mut verse_account.data.borrow_mut()[..])?;
    
    msg!("Circuit breaker test completed");
    
    Ok(())
}

/// Test auto-recovery mechanism
pub fn test_auto_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let circuit_breaker_account = next_account_info(account_iter)?;
    let verse_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    
    msg!("Testing auto-recovery mechanism");
    
    // Load accounts
    let mut circuit_breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
    let mut verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    if !circuit_breaker.is_active {
        msg!("Circuit breaker not active - nothing to recover");
        return Ok(());
    }
    
    // Check recovery conditions based on breaker type
    let can_recover = match circuit_breaker.breaker_type.unwrap_or(CircuitBreakerType::Coverage) {
        CircuitBreakerType::Coverage => {
            // Check if coverage improved
            let coverage = (global_config.vault * 10000) / global_config.total_oi;
            coverage >= 5000 // 50% threshold
        }
        CircuitBreakerType::Price => {
            // Check if cooldown period passed (100 slots)
            circuit_breaker.triggered_at.is_some() && Clock::get()?.slot > circuit_breaker.triggered_at.unwrap() + 100
        }
        CircuitBreakerType::Liquidation => {
            // Check if liquidations have slowed
            // In production, would check actual liquidation metrics
            circuit_breaker.triggered_at.is_some() && Clock::get()?.slot > circuit_breaker.triggered_at.unwrap() + 200
        }
        CircuitBreakerType::OracleFailure => {
            // Would check oracle health in production
            false // Requires manual intervention
        }
        CircuitBreakerType::Volume => {
            // Check if volume has normalized (150 slots cooldown)
            circuit_breaker.triggered_at.is_some() && Clock::get()?.slot > circuit_breaker.triggered_at.unwrap() + 150
        }
        CircuitBreakerType::Congestion => {
            // Check if network congestion has cleared (50 slots cooldown)
            circuit_breaker.triggered_at.is_some() && Clock::get()?.slot > circuit_breaker.triggered_at.unwrap() + 50
        }
    };
    
    if can_recover {
        msg!("Recovery conditions met - lifting circuit breaker");
        
        circuit_breaker.is_active = false;
        circuit_breaker.resolved_at = Some(Clock::get()?.slot as i64);
        
        // Restore verse
        verse.status = VerseStatus::Active;
        
        // Save state
        circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        verse.serialize(&mut &mut verse_account.data.borrow_mut()[..])?;
        
        emit_event(EventType::CircuitBreakerReset, &RecoveryCompleteEvent {
            verse_id: verse.verse_id,
            breaker_type: circuit_breaker.breaker_type,
            duration_slots: (circuit_breaker.resolved_at.unwrap() - circuit_breaker.triggered_at.unwrap() as i64) as u64,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("Auto-recovery completed successfully");
    } else {
        msg!("Recovery conditions not met - circuit breaker remains active");
    }
    
    Ok(())
}

/// Attempt trade on halted market (should fail)
fn attempt_trade_on_halted_market(proposal: &ProposalPDA) -> Result<(), BettingPlatformError> {
    if !proposal.is_active() {
        return Err(BettingPlatformError::MarketHalted);
    }
    Ok(())
}

/// Circuit breaker trigger types
#[derive(Debug)]
pub enum CircuitBreakerTrigger {
    PriceVolatility { change_bps: u16 },
    LiquidationCascade { liquidation_rate_bps: u16 },
    OracleFailure,
    EmergencyHalt { reason: String },
}



/// Circuit breaker triggered event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CircuitBreakerTriggeredEvent {
    pub verse_id: u128,
    pub breaker_type: Option<CircuitBreakerType>,
    pub reason: Option<String>,
    pub timestamp: i64,
}

/// Recovery complete event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct RecoveryCompleteEvent {
    pub verse_id: u128,
    pub breaker_type: Option<CircuitBreakerType>,
    pub duration_slots: u64,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_coverage_calculation() {
        // Test normal coverage
        let vault = 1_000_000;
        let total_oi = 2_000_000;
        let coverage = ((vault as i64) * 10000) / (total_oi as i64);
        assert_eq!(coverage, 5000); // 50%
        
        // Test low coverage
        let vault = 400_000;
        let total_oi = 2_000_000;
        let coverage = ((vault as i64) * 10000) / (total_oi as i64);
        assert_eq!(coverage, 2000); // 20%
        assert!(coverage < 5000); // Should trigger halt
    }
}