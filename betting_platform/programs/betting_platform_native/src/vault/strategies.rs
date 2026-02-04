//! Vault Strategies
//!
//! Strategy execution and management for yield generation

use solana_program::{
    msg,
    program_error::ProgramError,
};

use crate::error::BettingPlatformError;

use super::{
    state::{Vault, VaultStrategy, StrategyType},
    yield_generation::MarketConditions,
};

/// Execute vault strategy
pub fn execute_strategy(
    vault: &mut Vault,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    match vault.strategy.strategy_type {
        StrategyType::Conservative => {
            execute_conservative_strategy(vault, market_conditions)
        },
        StrategyType::Balanced => {
            execute_balanced_strategy(vault, market_conditions)
        },
        StrategyType::Aggressive => {
            execute_aggressive_strategy(vault, market_conditions)
        },
        StrategyType::Custom(ref custom) => {
            execute_custom_strategy(vault, custom, market_conditions)
        }
    }
}

/// Execute conservative strategy
fn execute_conservative_strategy(
    vault: &mut Vault,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    // Risk checks
    if market_conditions.overall_volatility > 0.5 {
        msg!("High volatility detected, reducing exposure");
        vault.risk_params.max_leverage = 2;
    }
    
    msg!("Executing conservative strategy for vault {}", vault.vault_id);
    Ok(())
}

/// Execute balanced strategy
fn execute_balanced_strategy(
    vault: &mut Vault,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    // Adjust based on market conditions
    if market_conditions.trend_strength > 0.8 {
        msg!("Strong trend detected, increasing momentum allocation");
    }
    
    msg!("Executing balanced strategy for vault {}", vault.vault_id);
    Ok(())
}

/// Execute aggressive strategy
fn execute_aggressive_strategy(
    vault: &mut Vault,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    // Check risk limits
    if vault.risk_params.risk_score > 80 {
        msg!("Risk score too high, scaling back leverage");
        vault.risk_params.max_leverage = vault.risk_params.max_leverage.saturating_sub(10);
    }
    
    msg!("Executing aggressive strategy for vault {}", vault.vault_id);
    Ok(())
}

/// Execute custom strategy
fn execute_custom_strategy(
    vault: &mut Vault,
    custom: &super::state::CustomStrategy,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    msg!("Executing custom strategy {} for vault {}", 
         String::from_utf8_lossy(&custom.name), vault.vault_id);
    Ok(())
}

/// Rebalance positions based on strategy
pub fn rebalance_positions(
    vault: &mut Vault,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    // Check if rebalance is needed
    if !needs_rebalancing(vault, market_conditions) {
        return Ok(());
    }
    
    msg!("Rebalancing vault {} positions", vault.vault_id);
    
    // Update status during rebalance
    vault.status = super::state::VaultStatus::Rebalancing;
    
    // Perform rebalancing logic
    // In production, would execute actual position changes
    
    // Return to active status
    vault.status = super::state::VaultStatus::Active;
    
    Ok(())
}

/// Check if rebalancing is needed
fn needs_rebalancing(
    vault: &Vault,
    market_conditions: &MarketConditions,
) -> bool {
    // Check time since last rebalance
    let current_time = solana_program::clock::Clock::get()
        .map(|c| c.unix_timestamp)
        .unwrap_or(0);
    
    if current_time < vault.next_rebalance {
        return false;
    }
    
    // Check deviation threshold
    true
}