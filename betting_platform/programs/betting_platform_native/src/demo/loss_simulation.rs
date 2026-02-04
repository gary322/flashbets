//! Loss Simulation for Risk Education
//! 
//! Demonstrates the impact of high leverage through simulated trading scenarios.
//! Shows users how quickly positions can be liquidated with different leverage levels.

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
    constants::{LEVERAGE_PRECISION, PARTIAL_LIQUIDATION_BPS},
    error::BettingPlatformError,
    events::{EventType, Event},
    state::accounts::{Position, discriminators},
    math::leverage::calculate_effective_leverage,
    define_event,
};

/// Simulation scenarios
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum SimulationScenario {
    /// Conservative leverage (10x) with 10% adverse move
    Conservative,
    /// Moderate leverage (50x) with 2% adverse move  
    Moderate,
    /// High leverage (100x) with 1% adverse move
    High,
    /// Extreme leverage (500x) with 0.2% adverse move
    Extreme,
    /// Cascading liquidation scenario
    CascadeLiquidation,
    /// Volatile market with multiple swings
    VolatileMarket,
}

/// Simulation result showing what happened
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SimulationResult {
    /// Scenario that was simulated
    pub scenario: SimulationScenario,
    /// Starting collateral
    pub initial_collateral: u64,
    /// Initial position size
    pub initial_position_size: u64,
    /// Leverage used
    pub leverage: u64,
    /// Price movements simulated
    pub price_movements: Vec<PriceMovement>,
    /// Final position value (0 if liquidated)
    pub final_position_value: u64,
    /// Final collateral (0 if liquidated)
    pub final_collateral: u64,
    /// Was position liquidated
    pub was_liquidated: bool,
    /// At what step liquidation occurred
    pub liquidation_step: Option<u8>,
    /// Total loss percentage
    pub total_loss_pct: i64,
    /// Educational message
    pub lesson: String,
}

/// Price movement in simulation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct PriceMovement {
    /// Step number
    pub step: u8,
    /// Price change percentage (basis points, can be negative)
    pub price_change_bps: i64,
    /// Position value after this move
    pub position_value: u64,
    /// Margin ratio after this move
    pub margin_ratio: u64,
    /// Was partial liquidation triggered
    pub partial_liquidation: bool,
}

/// Demo account for loss simulation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DemoSimulationAccount {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// User running simulation
    pub user: Pubkey,
    /// Current simulation (if any)
    pub current_simulation: Option<SimulationResult>,
    /// Total simulations run
    pub simulations_run: u32,
    /// Scenarios completed
    pub scenarios_completed: u8,
    /// Has completed all educational scenarios
    pub education_complete: bool,
    /// Last simulation timestamp
    pub last_simulation: i64,
}

impl DemoSimulationAccount {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        1 + 500 + // current_simulation (Option + max SimulationResult size)
        4 + // simulations_run
        1 + // scenarios_completed
        1 + // education_complete
        8; // last_simulation

    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::DEMO_ACCOUNT,
            user,
            current_simulation: None,
            simulations_run: 0,
            scenarios_completed: 0,
            education_complete: false,
            last_simulation: 0,
        }
    }
}

/// Run loss simulation
pub fn run_loss_simulation(
    scenario: SimulationScenario,
    initial_collateral: u64,
) -> Result<SimulationResult, ProgramError> {
    let (leverage, price_movements) = match scenario {
        SimulationScenario::Conservative => (
            10 * LEVERAGE_PRECISION,
            vec![-200, -300, -500], // -2%, -3%, -5% = -10% total
        ),
        SimulationScenario::Moderate => (
            50 * LEVERAGE_PRECISION,
            vec![-100, -100], // -1%, -1% = -2% total
        ),
        SimulationScenario::High => (
            100 * LEVERAGE_PRECISION,
            vec![-50, -50], // -0.5%, -0.5% = -1% total
        ),
        SimulationScenario::Extreme => (
            500 * LEVERAGE_PRECISION,
            vec![-20], // -0.2% = instant liquidation
        ),
        SimulationScenario::CascadeLiquidation => (
            200 * LEVERAGE_PRECISION,
            vec![-30, -20, -10, -10, -10], // Gradual decline
        ),
        SimulationScenario::VolatileMarket => (
            100 * LEVERAGE_PRECISION,
            vec![50, -70, 30, -80, 20, -60], // Wild swings
        ),
    };

    let initial_position_size = initial_collateral
        .checked_mul(leverage)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(LEVERAGE_PRECISION)
        .ok_or(BettingPlatformError::MathOverflow)?;

    let mut position_value = initial_position_size;
    let mut collateral = initial_collateral;
    let mut was_liquidated = false;
    let mut liquidation_step = None;
    let mut movements = Vec::new();

    // Simulate each price movement
    for (step, &price_change_bps) in price_movements.iter().enumerate() {
        // Calculate new position value
        let price_factor = 10000i64 + price_change_bps;
        position_value = (position_value as i128 * price_factor as i128 / 10000) as u64;

        // Calculate unrealized PnL
        let pnl = position_value as i64 - initial_position_size as i64;
        
        // Update collateral
        if pnl < 0 && pnl.abs() as u64 > collateral {
            // Liquidation!
            was_liquidated = true;
            liquidation_step = Some(step as u8);
            position_value = 0;
            collateral = 0;
        } else {
            collateral = (collateral as i64 + pnl) as u64;
        }

        // Calculate margin ratio
        let margin_ratio = if position_value > 0 {
            collateral * 10000 / position_value
        } else {
            0
        };

        // Check for partial liquidation (8% per slot)
        let partial_liq = margin_ratio > 0 && margin_ratio < 200; // < 2% margin

        movements.push(PriceMovement {
            step: step as u8,
            price_change_bps,
            position_value,
            margin_ratio,
            partial_liquidation: partial_liq,
        });

        if was_liquidated {
            break;
        }
    }

    // Calculate total loss percentage
    let total_loss_pct = if initial_collateral > 0 {
        ((initial_collateral as i64 - collateral as i64) * 10000 / initial_collateral as i64)
    } else {
        0
    };

    // Generate educational lesson
    let lesson = match scenario {
        SimulationScenario::Conservative => {
            "With 10x leverage, a 10% adverse move wipes out your entire position. \
            Even 'conservative' leverage can be risky in volatile markets."
        }
        SimulationScenario::Moderate => {
            "At 50x leverage, just a 2% price movement against you results in total loss. \
            This is why most traders lose money with high leverage."
        }
        SimulationScenario::High => {
            "100x leverage means a mere 1% adverse move liquidates your position. \
            The market can move 1% in seconds during news events."
        }
        SimulationScenario::Extreme => {
            "500x leverage is essentially gambling - a 0.2% move (common in crypto) \
            instantly liquidates you. This is why we require education before allowing it."
        }
        SimulationScenario::CascadeLiquidation => {
            "Partial liquidations can cascade - each one reduces your margin, \
            making the next liquidation more likely. It's a downward spiral."
        }
        SimulationScenario::VolatileMarket => {
            "In volatile markets, you can be liquidated even if price eventually \
            moves in your favor. Leverage amplifies both gains AND losses."
        }
    }.to_string();

    Ok(SimulationResult {
        scenario,
        initial_collateral,
        initial_position_size,
        leverage,
        price_movements: movements,
        final_position_value: position_value,
        final_collateral: collateral,
        was_liquidated,
        liquidation_step,
        total_loss_pct,
        lesson,
    })
}

/// Process demo simulation request
pub fn process_run_demo_simulation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    scenario: SimulationScenario,
    initial_collateral: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let simulation_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate collateral amount (demo uses fake money)
    if initial_collateral == 0 || initial_collateral > 100_000_000_000 { // Max $100k demo
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Load simulation account
    let mut sim_account = DemoSimulationAccount::try_from_slice(&simulation_account.data.borrow())?;
    
    // Verify ownership
    if sim_account.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Run simulation
    let result = run_loss_simulation(scenario, initial_collateral)?;
    
    msg!("=== LOSS SIMULATION RESULT ===");
    msg!("Scenario: {:?}", scenario);
    msg!("Leverage: {}x", result.leverage / LEVERAGE_PRECISION);
    msg!("Initial: ${}", initial_collateral / 1_000_000);
    msg!("Final: ${}", result.final_collateral / 1_000_000);
    msg!("Loss: {}%", result.total_loss_pct / 100);
    msg!("Liquidated: {}", result.was_liquidated);
    msg!("Lesson: {}", result.lesson);
    
    // Update account
    sim_account.current_simulation = Some(result.clone());
    sim_account.simulations_run += 1;
    sim_account.last_simulation = Clock::get()?.unix_timestamp;
    
    // Track scenario completion
    let scenario_bit = 1u8 << (scenario as u8);
    if sim_account.scenarios_completed & scenario_bit == 0 {
        sim_account.scenarios_completed |= scenario_bit;
    }
    
    // Check if all scenarios complete
    if sim_account.scenarios_completed == 0b00111111 { // All 6 scenarios
        sim_account.education_complete = true;
        msg!("Congratulations! You've completed all educational scenarios.");
    }
    
    // Save account
    sim_account.serialize(&mut &mut simulation_account.data.borrow_mut()[..])?;
    
    // Emit event
    let event = SimulationCompleted {
        user: *user.key,
        scenario,
        leverage: result.leverage,
        was_liquidated: result.was_liquidated,
        loss_percentage: result.total_loss_pct,
        timestamp: sim_account.last_simulation,
    };
    event.emit();
    
    Ok(())
}

/// Get simulation recommendations based on user's planned leverage
pub fn get_recommended_simulation(target_leverage: u64) -> SimulationScenario {
    match target_leverage / LEVERAGE_PRECISION {
        0..=20 => SimulationScenario::Conservative,
        21..=75 => SimulationScenario::Moderate,
        76..=150 => SimulationScenario::High,
        151..=300 => SimulationScenario::CascadeLiquidation,
        _ => SimulationScenario::Extreme,
    }
}

/// Format simulation result for display
pub fn format_simulation_result(result: &SimulationResult) -> String {
    let mut output = String::new();
    
    output.push_str(&format!(
        "📊 {} Leverage Simulation ({}x)\n",
        match result.scenario {
            SimulationScenario::Conservative => "Conservative",
            SimulationScenario::Moderate => "Moderate",
            SimulationScenario::High => "High",
            SimulationScenario::Extreme => "EXTREME",
            SimulationScenario::CascadeLiquidation => "Cascade",
            SimulationScenario::VolatileMarket => "Volatile",
        },
        result.leverage / LEVERAGE_PRECISION
    ));
    
    output.push_str(&format!(
        "💰 Started with: ${:.2}\n",
        result.initial_collateral as f64 / 1_000_000.0
    ));
    
    output.push_str("\n📈 Price Movements:\n");
    for movement in &result.price_movements {
        let emoji = if movement.price_change_bps < 0 { "📉" } else { "📈" };
        output.push_str(&format!(
            "  Step {}: {} {:+.2}% → Value: ${:.2}",
            movement.step + 1,
            emoji,
            movement.price_change_bps as f64 / 100.0,
            movement.position_value as f64 / 1_000_000.0
        ));
        
        if movement.partial_liquidation {
            output.push_str(" ⚠️ PARTIAL LIQ!");
        }
        output.push_str("\n");
    }
    
    output.push_str(&format!(
        "\n💸 Final Result: ${:.2} ({:+.1}%)\n",
        result.final_collateral as f64 / 1_000_000.0,
        result.total_loss_pct as f64 / 100.0
    ));
    
    if result.was_liquidated {
        output.push_str(&format!(
            "🔴 LIQUIDATED at step {}!\n",
            result.liquidation_step.unwrap_or(0) + 1
        ));
    }
    
    output.push_str(&format!("\n📚 Lesson: {}", result.lesson));
    
    output
}

// Events
define_event!(SimulationCompleted, EventType::DemoSimulation, {
    user: Pubkey,
    scenario: SimulationScenario,
    leverage: u64,
    was_liquidated: bool,
    loss_percentage: i64,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extreme_leverage_simulation() {
        let result = run_loss_simulation(
            SimulationScenario::Extreme,
            10_000_000, // $10 collateral
        ).unwrap();

        assert!(result.was_liquidated);
        assert_eq!(result.liquidation_step, Some(0)); // First move kills it
        assert_eq!(result.final_collateral, 0);
        assert_eq!(result.total_loss_pct, 10000); // 100% loss
    }

    #[test]
    fn test_conservative_simulation() {
        let result = run_loss_simulation(
            SimulationScenario::Conservative,
            10_000_000, // $10 collateral
        ).unwrap();

        // Should survive some moves but eventually liquidate
        assert!(result.was_liquidated);
        assert!(result.liquidation_step.unwrap() > 0);
    }

    #[test]
    fn test_volatile_market_simulation() {
        let result = run_loss_simulation(
            SimulationScenario::VolatileMarket,
            10_000_000, // $10 collateral
        ).unwrap();

        // High leverage in volatile market = liquidation despite some positive moves
        assert!(result.price_movements.len() > 2);
        assert!(result.was_liquidated);
    }
}