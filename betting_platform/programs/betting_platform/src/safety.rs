use anchor_lang::prelude::*;
use crate::account_structs::*;
use crate::errors::ErrorCode;
use crate::events::*;

// Safety Mechanisms - Circuit Breakers and Health Monitoring

#[derive(Accounts)]
pub struct CheckCircuitBreakers<'info> {
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub price_history: Account<'info, PriceHistory>,
}

#[derive(Accounts)]
pub struct MonitorHealth<'info> {
    #[account(mut)]
    pub user_map: Account<'info, MapEntryPDA>,
    
    #[account(mut)]
    pub price_cache: Account<'info, PriceCachePDA>,
}

pub fn check_circuit_breakers(
    ctx: Context<CheckCircuitBreakers>,
    price_movement: i64,
) -> Result<()> {
    let global = &ctx.accounts.global_config;
    let clock = Clock::get()?;

    // Check price movement limit (5% over 4 slots)
    if price_movement.abs() > 500 {
        // Check recent movements
        let recent_movements = &ctx.accounts.price_history.movements;
        let mut total_movement = price_movement;

        for movement in recent_movements.iter().rev().take(3) {
            total_movement = total_movement.saturating_add(*movement);
        }

        if total_movement.abs() > 500 {
            // Trigger halt
            let global = &mut ctx.accounts.global_config;
            global.halt_flag = true;
            global.halt_until = clock.slot + 900; // 1 hour

            emit!(CircuitBreakerEvent {
                reason: "Excessive price movement".to_string(),
                total_movement,
                halt_until: global.halt_until,
                coverage: global.coverage,
            });

            return Err(ErrorCode::CircuitBreakerTriggered.into());
        }
    }

    // Check coverage threshold
    if global.coverage < MINIMUM_COVERAGE {
        let global = &mut ctx.accounts.global_config;
        global.halt_flag = true;
        global.halt_until = clock.slot + 450; // 30 minutes

        emit!(CircuitBreakerEvent {
            reason: "Low coverage".to_string(),
            total_movement: 0,
            halt_until: global.halt_until,
            coverage: global.coverage,
        });

        return Err(ErrorCode::LowCoverage.into());
    }

    Ok(())
}

pub fn monitor_position_health(
    ctx: Context<MonitorHealth>,
) -> Result<()> {
    let user_map = &mut ctx.accounts.user_map;
    let price_cache = &ctx.accounts.price_cache;

    // Recalculate health factor
    let current_prices = vec![price_cache.last_price]; // Simplified
    let new_health = user_map.calculate_health(&current_prices);

    // Alert if health deteriorating
    if new_health < user_map.health_factor {
        let deterioration = user_map.health_factor - new_health;

        if deterioration > HEALTH_WARNING_THRESHOLD {
            emit!(HealthWarningEvent {
                user: user_map.user,
                old_health: user_map.health_factor,
                new_health,
                at_risk_positions: user_map.positions.len() as u8,
            });
        }
    }

    user_map.health_factor = new_health;

    Ok(())
}

// State consistency verification
pub fn verify_global_consistency(
    global_config: &GlobalConfigPDA,
) -> Result<()> {
    let global = global_config;

    // Verify coverage calculation
    let expected_coverage = if global.total_oi == 0 {
        u128::MAX
    } else {
        let tail_loss = calculate_tail_loss(1); // Simplified
        (global.vault as u128 * PRECISION) / (tail_loss * global.total_oi as u128)
    };

    require!(
        (global.coverage as i128 - expected_coverage as i128).abs() < 1000,
        ErrorCode::InconsistentCoverage
    );

    // Note: Vault balance and MMT supply verification would require
    // access to account data which is not available in this context.
    // These checks should be performed in the instruction handlers
    // where ctx is available.

    Ok(())
}

// Helper function - duplicated from trading.rs for now
fn calculate_tail_loss(outcome_count: u32) -> u128 {
    match outcome_count {
        1 => PRECISION,
        2..=4 => PRECISION * 2,
        5..=8 => PRECISION * 3,
        _ => PRECISION * 4,
    }
}

// Position limit checks
pub fn check_position_limits(
    user_map: &MapEntryPDA,
    new_position_size: u64,
) -> Result<()> {
    // Maximum positions per user: 50
    require!(
        user_map.positions.len() < 50,
        ErrorCode::InvalidPosition
    );

    // Minimum position size: 0.01 SOL equivalent (10_000_000 lamports)
    require!(
        new_position_size >= 10_000_000,
        ErrorCode::InvalidPosition
    );

    Ok(())
}

// Market manipulation detection
pub fn detect_market_manipulation(
    price_movements: &[i64],
    volume_spike: u64,
    normal_volume: u64,
) -> bool {
    // Check for wash trading patterns
    if volume_spike > normal_volume * 10 {
        return true;
    }

    // Check for price manipulation (rapid oscillations)
    if price_movements.len() >= 4 {
        let mut direction_changes = 0;
        for i in 1..price_movements.len() {
            if (price_movements[i] > 0) != (price_movements[i-1] > 0) {
                direction_changes += 1;
            }
        }
        
        if direction_changes >= 3 {
            return true;
        }
    }

    false
}

// Emergency pause functionality
pub fn emergency_pause(
    ctx: Context<EmergencyPause>,
    reason: String,
) -> Result<()> {
    let global = &mut ctx.accounts.global_config;
    let clock = Clock::get()?;

    global.halt_flag = true;
    global.halt_until = clock.slot + 7200; // 8 hours

    emit!(EmergencyHaltEvent {
        slot: clock.slot,
        reason,
    });

    Ok(())
}

// Resume after emergency
pub fn resume_trading(
    ctx: Context<ResumeTradingContext>,
) -> Result<()> {
    let global = &mut ctx.accounts.global_config;
    let clock = Clock::get()?;

    require!(
        clock.slot >= global.halt_until,
        ErrorCode::EmergencyHaltExpired
    );

    global.halt_flag = false;
    global.halt_until = 0;

    Ok(())
}

// Risk parameter validation
pub fn validate_risk_parameters(
    leverage: u64,
    coverage: u128,
    volatility: u64,
) -> Result<()> {
    // High leverage requires high coverage
    if leverage > 50 {
        require!(
            coverage > PRECISION * 2,
            ErrorCode::InsufficientCoverage
        );
    }

    // High volatility limits leverage
    if volatility > VOLATILITY_PRECISION / 2 {
        require!(
            leverage <= 25,
            ErrorCode::ExcessiveLeverage
        );
    }

    Ok(())
}

// Account structs for safety functions
#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
}

#[derive(Accounts)]
pub struct ResumeTradingContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_manipulation_detection() {
        // Normal trading pattern
        let normal_movements = vec![10, 15, 20, 25];
        assert!(!detect_market_manipulation(&normal_movements, 1000, 1000));

        // Wash trading pattern (10x volume spike)
        assert!(detect_market_manipulation(&normal_movements, 11000, 1000));

        // Price manipulation pattern (rapid oscillations)
        let manipulated_movements = vec![50, -50, 50, -50];
        assert!(detect_market_manipulation(&manipulated_movements, 1000, 1000));
    }

    #[test]
    fn test_risk_validation() {
        // High leverage with low coverage should fail
        assert!(validate_risk_parameters(100, PRECISION / 2, 5000).is_err());

        // High volatility with high leverage should fail
        assert!(validate_risk_parameters(50, PRECISION * 2, 8000).is_err());

        // Normal parameters should pass
        assert!(validate_risk_parameters(25, PRECISION * 2, 3000).is_ok());
    }
}