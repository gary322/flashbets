//! Yield Generation Mechanism
//!
//! Strategies for generating yield through CDP, perpetuals, and other protocols

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    cdp::{CDPAccount, BorrowRequest},
    perpetual::{PerpetualPosition, PerpetualMarket, PositionType},
};

use super::{
    state::{Vault, VaultStrategy, StrategyType, Protocol},
    strategies::{execute_strategy, rebalance_positions},
};

/// Yield source tracking
#[derive(Debug, Clone)]
pub struct YieldSource {
    /// Protocol generating yield
    pub protocol: Protocol,
    
    /// Amount allocated
    pub allocated_amount: u128,
    
    /// Current APY
    pub current_apy: f64,
    
    /// Yield earned
    pub yield_earned: u128,
    
    /// Risk score (0-100)
    pub risk_score: u8,
    
    /// Last harvest time
    pub last_harvest: i64,
}

/// Generate yield for vault
pub fn generate_yield(
    vault: &mut Vault,
    oracle: &OraclePDA,
    market_conditions: &MarketConditions,
) -> Result<u128, ProgramError> {
    // Check if rebalance is needed
    let current_time = Clock::get()?.unix_timestamp;
    if current_time >= vault.next_rebalance {
        rebalance_vault_positions(vault, oracle, market_conditions)?;
        vault.next_rebalance = current_time + vault.strategy.rebalance_frequency as i64;
    }
    
    // Execute yield generation based on strategy
    let yield_amount = match &vault.strategy.strategy_type {
        StrategyType::Conservative => {
            generate_conservative_yield(vault, oracle)
        },
        StrategyType::Balanced => {
            generate_balanced_yield(vault, oracle, market_conditions)
        },
        StrategyType::Aggressive => {
            generate_aggressive_yield(vault, oracle, market_conditions)
        },
        StrategyType::Custom(custom) => {
            generate_custom_yield(vault, oracle, custom)
        }
    }?;
    
    // Update vault metrics
    vault.performance.total_yield_generated += yield_amount;
    update_vault_apy(vault, yield_amount);
    
    msg!("Generated {} yield for vault {}", yield_amount, vault.vault_id);
    
    Ok(yield_amount)
}

/// Conservative yield generation (low risk)
fn generate_conservative_yield(
    vault: &Vault,
    oracle: &OraclePDA,
) -> Result<u128, ProgramError> {
    let available_capital = vault.available_liquidity;
    let mut total_yield = 0u128;
    
    // Allocate to low-risk strategies
    // 50% lending at base rate
    let lending_allocation = available_capital / 2;
    let lending_yield = calculate_lending_yield(lending_allocation, 0.05); // 5% APY
    total_yield += lending_yield;
    
    // 30% staking
    let staking_allocation = (available_capital * 3) / 10;
    let staking_yield = calculate_staking_yield(staking_allocation, oracle);
    total_yield += staking_yield;
    
    // 20% reserve (no yield but provides liquidity)
    
    msg!("Conservative yield: lending={}, staking={}", 
         lending_yield, staking_yield);
    
    Ok(total_yield)
}

/// Balanced yield generation (moderate risk)
fn generate_balanced_yield(
    vault: &Vault,
    oracle: &OraclePDA,
    market_conditions: &MarketConditions,
) -> Result<u128, ProgramError> {
    let available_capital = vault.available_liquidity;
    let mut total_yield = 0u128;
    
    // 30% CDP with moderate leverage
    let cdp_allocation = (available_capital * 3) / 10;
    let cdp_yield = generate_cdp_yield(
        cdp_allocation,
        10, // 10x leverage
        oracle,
    )?;
    total_yield += cdp_yield;
    
    // 30% perpetuals with hedging
    let perp_allocation = (available_capital * 3) / 10;
    let perp_yield = generate_perpetual_yield(
        perp_allocation,
        market_conditions,
        true, // Use hedging
    )?;
    total_yield += perp_yield;
    
    // 20% liquidity provision
    let lp_allocation = available_capital / 5;
    let lp_yield = calculate_lp_yield(lp_allocation, oracle);
    total_yield += lp_yield;
    
    // 20% lending
    let lending_allocation = available_capital / 5;
    let lending_yield = calculate_lending_yield(lending_allocation, 0.08); // 8% APY
    total_yield += lending_yield;
    
    msg!("Balanced yield: cdp={}, perp={}, lp={}, lending={}", 
         cdp_yield, perp_yield, lp_yield, lending_yield);
    
    Ok(total_yield)
}

/// Aggressive yield generation (high risk)
fn generate_aggressive_yield(
    vault: &Vault,
    oracle: &OraclePDA,
    market_conditions: &MarketConditions,
) -> Result<u128, ProgramError> {
    let available_capital = vault.available_liquidity;
    let mut total_yield = 0u128;
    
    // 40% CDP with high leverage
    let cdp_allocation = (available_capital * 4) / 10;
    let cdp_yield = generate_cdp_yield(
        cdp_allocation,
        50, // 50x leverage
        oracle,
    )?;
    total_yield += cdp_yield;
    
    // 40% perpetuals without hedging
    let perp_allocation = (available_capital * 4) / 10;
    let perp_yield = generate_perpetual_yield(
        perp_allocation,
        market_conditions,
        false, // No hedging for max returns
    )?;
    total_yield += perp_yield;
    
    // 10% options strategies
    let options_allocation = available_capital / 10;
    let options_yield = generate_options_yield(options_allocation, oracle);
    total_yield += options_yield;
    
    // 10% arbitrage
    let arb_allocation = available_capital / 10;
    let arb_yield = generate_arbitrage_yield(arb_allocation, market_conditions);
    total_yield += arb_yield;
    
    msg!("Aggressive yield: cdp={}, perp={}, options={}, arb={}", 
         cdp_yield, perp_yield, options_yield, arb_yield);
    
    Ok(total_yield)
}

/// Custom strategy yield generation
fn generate_custom_yield(
    vault: &Vault,
    oracle: &OraclePDA,
    custom: &super::state::CustomStrategy,
) -> Result<u128, ProgramError> {
    // Implement custom strategy based on parameters
    let yield_amount = (vault.available_liquidity as f64 * 0.15) as u128; // 15% placeholder
    
    msg!("Custom strategy {} generated {} yield", 
         String::from_utf8_lossy(&custom.name), yield_amount);
    
    Ok(yield_amount)
}

/// Generate yield through CDP
fn generate_cdp_yield(
    allocation: u128,
    leverage: u16,
    oracle: &OraclePDA,
) -> Result<u128, ProgramError> {
    // Calculate potential returns with leverage
    let oracle_scalar = calculate_cdp_scalar(oracle);
    let effective_leverage = (leverage as f64 * oracle_scalar).min(1000.0) as u16;
    
    // Estimate yield based on leverage and market conditions
    let base_yield_rate = 0.10; // 10% base
    let leverage_multiplier = (effective_leverage as f64).sqrt() / 10.0;
    let risk_adjustment = 1.0 - (oracle.current_sigma * 2.0); // Reduce yield in high volatility
    
    let yield_rate = base_yield_rate * leverage_multiplier * risk_adjustment;
    let daily_yield = (allocation as f64 * yield_rate / 365.0) as u128;
    
    Ok(daily_yield)
}

/// Generate yield through perpetuals
fn generate_perpetual_yield(
    allocation: u128,
    market_conditions: &MarketConditions,
    use_hedging: bool,
) -> Result<u128, ProgramError> {
    // Funding rate arbitrage
    let funding_rate = market_conditions.avg_funding_rate;
    let funding_yield = if funding_rate.abs() > 0.0001 {
        // Take opposite side of funding
        let daily_funding = (allocation as f64 * funding_rate.abs() * 24.0) as u128;
        if use_hedging {
            daily_funding / 2 // Reduced yield due to hedging costs
        } else {
            daily_funding
        }
    } else {
        0
    };
    
    // Momentum trading
    let momentum_yield = if market_conditions.trend_strength > 0.7 {
        (allocation as f64 * 0.02) as u128 // 2% daily in strong trends
    } else {
        0
    };
    
    Ok(funding_yield + momentum_yield)
}

/// Calculate lending yield
fn calculate_lending_yield(allocation: u128, apy: f64) -> u128 {
    (allocation as f64 * apy / 365.0) as u128
}

/// Calculate staking yield
fn calculate_staking_yield(allocation: u128, oracle: &OraclePDA) -> u128 {
    // Base staking rate adjusted by oracle confidence
    let base_rate = 0.08; // 8% APY
    let confidence_multiplier = 1.0 - oracle.current_sigma;
    let effective_rate = base_rate * confidence_multiplier;
    
    (allocation as f64 * effective_rate / 365.0) as u128
}

/// Calculate LP yield
fn calculate_lp_yield(allocation: u128, oracle: &OraclePDA) -> u128 {
    // LP fees based on volatility (higher volatility = more fees)
    let base_rate = 0.15; // 15% APY base
    let volatility_multiplier = 1.0 + oracle.current_sigma;
    let effective_rate = base_rate * volatility_multiplier;
    
    (allocation as f64 * effective_rate / 365.0) as u128
}

/// Generate options yield
fn generate_options_yield(allocation: u128, oracle: &OraclePDA) -> u128 {
    // Covered call / cash secured put strategies
    let premium_rate = 0.02 * (1.0 + oracle.current_sigma); // 2% + volatility premium
    (allocation as f64 * premium_rate) as u128
}

/// Generate arbitrage yield
fn generate_arbitrage_yield(allocation: u128, market_conditions: &MarketConditions) -> u128 {
    // Cross-market arbitrage opportunities
    if market_conditions.arbitrage_opportunity > 0.001 {
        (allocation as f64 * market_conditions.arbitrage_opportunity * 10.0) as u128
    } else {
        0
    }
}

/// Calculate CDP scalar for leverage
fn calculate_cdp_scalar(oracle: &OraclePDA) -> f64 {
    let sigma = oracle.current_sigma.max(0.01);
    let cap_fused = 20.0;
    let cap_vault = 30.0;
    let base_risk = 0.25;
    
    let risk = oracle.current_prob * (1.0 - oracle.current_prob);
    let unified_scalar = (1.0 / sigma) * cap_fused;
    let premium_factor = (risk / base_risk) * cap_vault;
    
    (unified_scalar * premium_factor).min(100.0)
}

/// Rebalance vault positions
fn rebalance_vault_positions(
    vault: &mut Vault,
    oracle: &OraclePDA,
    market_conditions: &MarketConditions,
) -> Result<(), ProgramError> {
    // Check if rebalance is needed
    let current_allocations = calculate_current_allocations(vault);
    let target_allocations = calculate_target_allocations(&vault.strategy, market_conditions);
    
    for (protocol, target_pct) in target_allocations {
        let current_pct = current_allocations.get(&protocol).unwrap_or(&0.0);
        let deviation = (target_pct - current_pct).abs();
        
        if deviation > vault.strategy.diversification.rebalance_threshold as f64 / 100.0 {
            // Rebalance needed
            let rebalance_amount = (vault.total_value_locked as f64 * deviation) as u128;
            msg!("Rebalancing {:?}: {}% -> {}% ({})", 
                 protocol, current_pct * 100.0, target_pct * 100.0, rebalance_amount);
        }
    }
    
    vault.status = super::state::VaultStatus::Active;
    
    Ok(())
}

/// Calculate current allocations
fn calculate_current_allocations(vault: &Vault) -> std::collections::HashMap<Protocol, f64> {
    // In production, would calculate actual allocations
    let mut allocations = std::collections::HashMap::new();
    allocations.insert(Protocol::NativeCDP, 0.3);
    allocations.insert(Protocol::Perpetuals, 0.3);
    allocations.insert(Protocol::Lending, 0.2);
    allocations.insert(Protocol::LiquidityMining, 0.2);
    allocations
}

/// Calculate target allocations
fn calculate_target_allocations(
    strategy: &VaultStrategy,
    market_conditions: &MarketConditions,
) -> std::collections::HashMap<Protocol, f64> {
    let mut allocations = std::collections::HashMap::new();
    
    match strategy.strategy_type {
        StrategyType::Conservative => {
            allocations.insert(Protocol::Lending, 0.5);
            allocations.insert(Protocol::Staking, 0.3);
            allocations.insert(Protocol::LiquidityMining, 0.2);
        },
        StrategyType::Balanced => {
            allocations.insert(Protocol::NativeCDP, 0.3);
            allocations.insert(Protocol::Perpetuals, 0.3);
            allocations.insert(Protocol::LiquidityMining, 0.2);
            allocations.insert(Protocol::Lending, 0.2);
        },
        StrategyType::Aggressive => {
            allocations.insert(Protocol::NativeCDP, 0.4);
            allocations.insert(Protocol::Perpetuals, 0.4);
            allocations.insert(Protocol::Options, 0.1);
            allocations.insert(Protocol::LiquidityMining, 0.1);
        },
        _ => {
            // Custom strategy
            for protocol in &strategy.allowed_protocols {
                let weight = 1.0 / strategy.allowed_protocols.len() as f64;
                allocations.insert(protocol.clone(), weight);
            }
        }
    }
    
    allocations
}

/// Update vault APY
fn update_vault_apy(vault: &mut Vault, yield_amount: u128) {
    let daily_rate = yield_amount as f64 / vault.total_value_locked as f64;
    let annual_rate = daily_rate * 365.0;
    
    // Update current APY
    vault.performance.current_apy = annual_rate;
    
    // Update rolling averages (simplified)
    vault.performance.avg_7d_apy = 
        (vault.performance.avg_7d_apy * 6.0 + annual_rate) / 7.0;
    vault.performance.avg_30d_apy = 
        (vault.performance.avg_30d_apy * 29.0 + annual_rate) / 30.0;
}

/// Market conditions for yield generation
#[derive(Debug)]
pub struct MarketConditions {
    pub avg_funding_rate: f64,
    pub trend_strength: f64,
    pub arbitrage_opportunity: f64,
    pub overall_volatility: f64,
}

impl Default for MarketConditions {
    fn default() -> Self {
        Self {
            avg_funding_rate: 0.0001,
            trend_strength: 0.5,
            arbitrage_opportunity: 0.0,
            overall_volatility: 0.2,
        }
    }
}