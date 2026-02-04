use anchor_lang::prelude::*;
use crate::state::*;
use crate::circuit_breaker::*;
use crate::attack_detection::*;
use crate::errors::*;
use crate::fixed_types::U64F64;

#[derive(Accounts)]
pub struct InitializeCircuitBreaker<'info> {
    #[account(
        init,
        payer = authority,
        space = CircuitBreaker::LEN,
        seeds = [b"circuit_breaker", verse.key().as_ref()],
        bump
    )]
    pub circuit_breaker: Account<'info, CircuitBreaker>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct CheckBreakers<'info> {
    #[account(
        mut,
        seeds = [b"circuit_breaker", verse.key().as_ref()],
        bump
    )]
    pub circuit_breaker: Account<'info, CircuitBreaker>,
    
    #[account(
        seeds = [b"attack_detector", verse.key().as_ref()],
        bump
    )]
    pub attack_detector: Account<'info, AttackDetector>,
    
    pub verse: Account<'info, Verse>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct EmergencyShutdown<'info> {
    #[account(
        mut,
        seeds = [b"circuit_breaker", verse.key().as_ref()],
        bump
    )]
    pub circuit_breaker: Account<'info, CircuitBreaker>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(
        constraint = emergency_authority.key() == circuit_breaker.emergency_authority.unwrap() @ crate::errors::ErrorCode::UnauthorizedEmergency
    )]
    pub emergency_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateBreakerConfig<'info> {
    #[account(
        mut,
        seeds = [b"circuit_breaker", verse.key().as_ref()],
        bump
    )]
    pub circuit_breaker: Account<'info, CircuitBreaker>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(
        constraint = authority.key() == verse.authority @ crate::errors::ErrorCode::Unauthorized
    )]
    pub authority: Signer<'info>,
}

// Instruction implementations
pub fn initialize_circuit_breaker(ctx: Context<InitializeCircuitBreaker>) -> Result<()> {
    let breaker = &mut ctx.accounts.circuit_breaker;
    let clock = &ctx.accounts.clock;
    
    breaker.init(clock)?;
    
    // Set emergency authority to the verse authority initially
    breaker.emergency_authority = Some(ctx.accounts.authority.key());
    
    msg!("Circuit breaker initialized for verse: {}", ctx.accounts.verse.key());
    
    Ok(())
}

pub fn check_breakers(
    ctx: Context<CheckBreakers>,
    coverage: u64, // Fixed point representation
    liquidation_count: u64,
    liquidation_volume: u64,
    total_oi: u64,
    failed_tx: u64,
) -> Result<()> {
    let breaker = &mut ctx.accounts.circuit_breaker;
    let detector = &ctx.accounts.attack_detector;
    let clock = &ctx.accounts.clock;
    
    // Convert coverage to fixed point
    let coverage_fp = U64F64::from_num(coverage) / U64F64::from_num(1_000_000);
    
    // Get recent trades from attack detector
    let recent_trades: Vec<TradeSnapshot> = detector.recent_trades.iter().cloned().collect();
    
    // Check all breakers
    let action = breaker.check_breakers(
        coverage_fp,
        &recent_trades,
        liquidation_count,
        liquidation_volume,
        total_oi,
        failed_tx,
        clock,
    )?;
    
    // Handle breaker action
    match action {
        BreakerAction::Halt { reason, duration, severity } => {
            msg!("CIRCUIT BREAKER TRIGGERED!");
            msg!("Reason: {:?}", reason);
            msg!("Duration: {} slots", duration);
            msg!("Severity: {:?}", severity);
            
            emit!(CircuitBreakerEvent {
                verse: ctx.accounts.verse.key(),
                reason,
                duration,
                severity,
                slot: clock.slot,
            });
            
            return Err(crate::errors::ErrorCode::CircuitBreakerTriggered.into());
        },
        BreakerAction::Resume => {
            msg!("Circuit breaker resuming normal operation");
            
            emit!(CircuitBreakerResumeEvent {
                verse: ctx.accounts.verse.key(),
                slot: clock.slot,
            });
        },
        BreakerAction::RemainHalted => {
            return Err(crate::errors::ErrorCode::SystemHalted.into());
        },
        BreakerAction::InCooldown => {
            msg!("Circuit breaker in cooldown period");
        },
        BreakerAction::EmergencyShutdown => {
            return Err(crate::errors::ErrorCode::SystemHalted.into());
        },
        BreakerAction::Continue => {
            // Normal operation
        },
    }
    
    Ok(())
}

pub fn emergency_shutdown(ctx: Context<EmergencyShutdown>) -> Result<()> {
    let breaker = &mut ctx.accounts.circuit_breaker;
    
    breaker.emergency_shutdown(&ctx.accounts.emergency_authority.key())?;
    
    msg!("EMERGENCY SHUTDOWN ACTIVATED BY: {}", ctx.accounts.emergency_authority.key());
    
    emit!(EmergencyShutdownEvent {
        verse: ctx.accounts.verse.key(),
        authority: ctx.accounts.emergency_authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

pub fn update_breaker_config(
    ctx: Context<UpdateBreakerConfig>,
    new_cooldown_period: Option<u64>,
    new_coverage_halt_duration: Option<u64>,
    new_price_halt_duration: Option<u64>,
    new_volume_halt_duration: Option<u64>,
    new_liquidation_halt_duration: Option<u64>,
    new_congestion_halt_duration: Option<u64>,
) -> Result<()> {
    let breaker = &mut ctx.accounts.circuit_breaker;
    
    // Update configurations if provided
    if let Some(cooldown) = new_cooldown_period {
        breaker.cooldown_period = cooldown;
    }
    
    if let Some(duration) = new_coverage_halt_duration {
        breaker.coverage_breaker.halt_duration = duration;
    }
    
    if let Some(duration) = new_price_halt_duration {
        breaker.price_breaker.halt_duration = duration;
    }
    
    if let Some(duration) = new_volume_halt_duration {
        breaker.volume_breaker.halt_duration = duration;
    }
    
    if let Some(duration) = new_liquidation_halt_duration {
        breaker.liquidation_breaker.halt_duration = duration;
    }
    
    if let Some(duration) = new_congestion_halt_duration {
        breaker.congestion_breaker.halt_duration = duration;
    }
    
    msg!("Circuit breaker configuration updated");
    
    Ok(())
}

// Events
#[event]
pub struct CircuitBreakerEvent {
    pub verse: Pubkey,
    pub reason: HaltReason,
    pub duration: u64,
    pub severity: AttackSeverity,
    pub slot: u64,
}

#[event]
pub struct CircuitBreakerResumeEvent {
    pub verse: Pubkey,
    pub slot: u64,
}

#[event]
pub struct EmergencyShutdownEvent {
    pub verse: Pubkey,
    pub authority: Pubkey,
    pub timestamp: i64,
}