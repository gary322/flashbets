use anchor_lang::prelude::*;
use crate::fixed_math::*;
use crate::chain_state::*;
use crate::account_structs::*;
use crate::errors::ErrorCode;

// Chain Safety Mechanisms

pub fn validate_chain_safety(
    verse: &VersePDA,
    steps: &[ChainStepType],
    initial_deposit: u64,
    coverage: FixedPoint,
) -> Result<()> {
    // Check maximum steps
    require!(steps.len() <= 5, ErrorCode::TooManySteps);

    // Check for cycles
    require!(!has_cycles(steps), ErrorCode::ChainCycle);

    // Simulate maximum leverage
    let max_leverage = simulate_max_leverage(steps, coverage)?;
    let max_exposure = FixedPoint::from_u64(initial_deposit)
        .mul(&max_leverage)?
        .to_u64_truncate();

    // Check against verse limits
    let verse_limit = calculate_verse_exposure_limit(verse, coverage)?;
    require!(max_exposure <= verse_limit, ErrorCode::ExceedsVerseLimit);

    // Check liquidation buffer
    let liq_buffer = calculate_liquidation_buffer(max_leverage)?;
    require!(
        liq_buffer >= MIN_LIQUIDATION_BUFFER,
        ErrorCode::InsufficientLiquidationBuffer
    );

    Ok(())
}

fn has_cycles(steps: &[ChainStepType]) -> bool {
    // Simple cycle detection - no asset should be borrowed and staked
    let has_borrow = steps.iter().any(|s| matches!(s, ChainStepType::Borrow));
    let has_stake = steps.iter().any(|s| matches!(s, ChainStepType::Stake));

    // More sophisticated cycle detection would use graph algorithms
    has_borrow && has_stake && steps.len() > 2
}

fn simulate_max_leverage(
    steps: &[ChainStepType],
    coverage: FixedPoint,
) -> Result<FixedPoint> {
    let mut leverage = FixedPoint::from_u64(1);

    for step in steps {
        let multiplier = match step {
            ChainStepType::Borrow => {
                // Conservative estimate
                FixedPoint::from_float(1.5)
            },
            ChainStepType::Liquidity => {
                FixedPoint::from_float(1.2)
            },
            ChainStepType::Stake => {
                FixedPoint::from_float(1.1)
            },
            ChainStepType::Arbitrage => {
                FixedPoint::from_float(1.05)
            },
        };

        leverage = leverage.mul(&multiplier)?;
    }

    // Apply coverage cap
    let coverage_cap = coverage.mul(&FixedPoint::from_u64(100))?;
    if leverage > coverage_cap {
        leverage = coverage_cap;
    }

    Ok(leverage)
}

pub fn calculate_verse_exposure_limit(
    verse: &VersePDA,
    coverage: FixedPoint,
) -> Result<u64> {
    // Base limit on verse depth and coverage
    let depth_factor = FixedPoint::from_u64(32).div(
        &FixedPoint::from_u64((verse.depth as u64).max(1))
    )?;
    
    let base_limit = FixedPoint::from_u64(1_000_000_000); // 1000 SOL base
    let adjusted_limit = base_limit
        .mul(&depth_factor)?
        .mul(&coverage)?;
    
    Ok(adjusted_limit.to_u64_truncate())
}

pub fn calculate_liquidation_buffer(
    leverage: FixedPoint,
) -> Result<u64> {
    // Higher leverage requires larger buffer
    let base_buffer = 500u64; // 5% base
    
    if leverage > FixedPoint::from_u64(100) {
        // Double buffer for extreme leverage
        Ok(base_buffer * 2)
    } else if leverage > FixedPoint::from_u64(50) {
        // 1.5x buffer for high leverage
        Ok((base_buffer * 3) / 2)
    } else {
        Ok(base_buffer)
    }
}

// Chain health monitoring
pub fn monitor_chain_health(
    chain_state: &ChainStatePDA,
    current_prices: &[u64],
) -> Result<u64> {
    let mut chain_health = 10000u64; // Start at 100%
    
    // Check each step's health
    for (i, step) in chain_state.step_states.iter().enumerate() {
        if step.status != StepStatus::Completed {
            continue;
        }
        
        // Simplified health check based on output vs input
        let step_health = if step.output_amount >= step.input_amount {
            10000u64 // Healthy
        } else {
            // Calculate health degradation
            let loss_ratio = ((step.input_amount - step.output_amount) * 10000) / step.input_amount;
            10000u64.saturating_sub(loss_ratio)
        };
        
        // Take minimum health across all steps
        chain_health = chain_health.min(step_health);
    }
    
    // Adjust for leverage
    let leverage_penalty = if chain_state.effective_leverage > FixedPoint::from_u64(100) {
        2000u64 // 20% penalty for extreme leverage
    } else if chain_state.effective_leverage > FixedPoint::from_u64(50) {
        1000u64 // 10% penalty for high leverage
    } else {
        0u64
    };
    
    Ok(chain_health.saturating_sub(leverage_penalty))
}

// Anti-manipulation checks
pub fn validate_chain_inputs(
    steps: &[ChainStepType],
    deposit: u64,
    user_history: &ChainUserHistory,
) -> Result<()> {
    // Check minimum deposit
    require!(
        deposit >= 100_000_000, // 0.1 SOL minimum
        ErrorCode::InvalidDeposit
    );
    
    // Check user rate limits
    let current_slot = Clock::get()?.slot;
    if let Some(last_chain_slot) = user_history.last_chain_slot {
        require!(
            current_slot >= last_chain_slot + 10, // 10 slot cooldown
            ErrorCode::TooManySteps // Reusing error for rate limit
        );
    }
    
    // Check suspicious patterns
    let borrow_count = steps.iter().filter(|s| matches!(s, ChainStepType::Borrow)).count();
    require!(
        borrow_count <= 2, // Max 2 borrow steps
        ErrorCode::ChainCycle
    );
    
    Ok(())
}

// Emergency circuit breaker for chains
pub fn check_chain_circuit_breaker(
    global_state: &GlobalChainState,
    chain_value: u64,
) -> Result<()> {
    // Check global chain limits
    require!(
        global_state.total_chains_value < global_state.max_chains_value,
        ErrorCode::ExceedsVerseLimit
    );
    
    // Check individual chain size
    require!(
        chain_value < global_state.max_chain_size,
        ErrorCode::ExceedsVerseLimit
    );
    
    // Check chain creation rate
    let current_slot = Clock::get()?.slot;
    let recent_chains = global_state.chains_created_last_100_slots;
    require!(
        recent_chains < 1000, // Max 1000 chains per 100 slots
        ErrorCode::TooManySteps
    );
    
    Ok(())
}

// Structs for chain safety

#[derive(Clone)]
pub struct ChainUserHistory {
    pub last_chain_slot: Option<u64>,
    pub total_chains_created: u64,
    pub total_value_chained: u64,
}

#[derive(Clone)]
pub struct GlobalChainState {
    pub total_chains_value: u64,
    pub max_chains_value: u64,
    pub max_chain_size: u64,
    pub chains_created_last_100_slots: u64,
}

// Verify chain invariants (for testing and monitoring)
pub fn verify_chain_invariants(chain_state: &ChainStatePDA) -> std::result::Result<(), String> {
    // Invariant 1: Steps completed <= max steps
    if chain_state.steps_completed > chain_state.max_steps {
        return Err("Steps completed exceeds max steps".to_string());
    }

    // Invariant 2: Effective leverage is product of step multipliers
    let mut calculated_leverage = FixedPoint::from_u64(1);
    for step in &chain_state.step_states {
        if step.status == StepStatus::Completed {
            calculated_leverage = calculated_leverage
                .mul(&step.leverage_multiplier)
                .map_err(|_| "Leverage calculation overflow")?;
        }
    }

    if (calculated_leverage.to_float() - chain_state.effective_leverage.to_float()).abs() > 0.001 {
        return Err("Effective leverage mismatch".to_string());
    }

    // Invariant 3: Current value matches last step output
    if let Some(last_step) = chain_state.step_states.last() {
        if last_step.status == StepStatus::Completed {
            if chain_state.current_value != last_step.output_amount {
                return Err("Current value doesn't match last step output".to_string());
            }
        }
    }

    // Invariant 4: All positions have valid IDs
    for step in &chain_state.step_states {
        if step.status == StepStatus::Completed && step.position_id.is_none() {
            return Err("Completed step missing position ID".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_detection() {
        let steps_with_cycle = vec![
            ChainStepType::Borrow,
            ChainStepType::Stake,
            ChainStepType::Borrow,
        ];
        assert!(has_cycles(&steps_with_cycle));

        let steps_without_cycle = vec![
            ChainStepType::Borrow,
            ChainStepType::Liquidity,
            ChainStepType::Arbitrage,
        ];
        assert!(!has_cycles(&steps_without_cycle));
    }

    #[test]
    fn test_leverage_simulation() {
        let steps = vec![
            ChainStepType::Borrow,    // 1.5x
            ChainStepType::Liquidity, // 1.2x
            ChainStepType::Stake,     // 1.1x
        ];
        
        let coverage = FixedPoint::from_float(1.5);
        let max_leverage = simulate_max_leverage(&steps, coverage).unwrap();
        
        // Expected: 1.5 * 1.2 * 1.1 = 1.98
        let expected = FixedPoint::from_float(1.98);
        assert!((max_leverage.to_float() - expected.to_float()).abs() < 0.01);
    }
}