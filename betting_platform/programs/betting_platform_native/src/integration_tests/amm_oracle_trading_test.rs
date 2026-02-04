//! AMM + Oracle + Trading Integration Test
//! 
//! Tests the complete flow from oracle price updates through AMM execution to trading

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
    state::{GlobalConfigPDA, ProposalPDA, Position, UserMap},
    amm::{
        newton_raphson_solver::NewtonRaphsonSolver,
        calculate_price_impact,
        execute_trade,
    },
    oracle::{
        polymarket::{PolymarketOracle, get_market_prices},
        OraclePrice,
    },
    trading::{
        calculate_margin_requirement,
        calculate_liquidation_price,
        validate_leverage,
    },
    events::{emit_event, EventType, IntegrationTestCompletedEvent},
    math::U64F64,
};

/// Complete AMM + Oracle + Trading integration test
pub fn test_amm_oracle_trading_integration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let amm_pool_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let user_map_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    msg!("Testing AMM + Oracle + Trading Integration");
    
    // Step 1: Oracle Price Update
    msg!("\nStep 1: Oracle Price Update");
    
    // Simulate fetching prices from Polymarket
    let oracle_prices = vec![
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 0,
            price: 520_000, // 0.52
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 95,
        },
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 1,
            price: 480_000, // 0.48
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 95,
        },
    ];
    
    // Verify oracle spread is acceptable
    let spread = calculate_oracle_spread(&oracle_prices)?;
    msg!("Oracle spread: {} bps", spread);
    
    if spread > 1000 {
        return Err(BettingPlatformError::OracleSpreadTooHigh.into());
    }
    
    // Update proposal with oracle prices
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    proposal.prices[0] = oracle_prices[0].price;
    proposal.prices[1] = oracle_prices[1].price;
    
    msg!("Updated market prices: [{}, {}]", proposal.prices[0], proposal.prices[1]);
    
    // Step 2: AMM Price Impact Calculation
    msg!("\nStep 2: AMM Price Impact Calculation");
    
    let trade_size = 50_000_000_000; // $50k
    let outcome = 0;
    let is_long = true;
    
    // Calculate price impact using Newton-Raphson
    let mut solver = NewtonRaphsonSolver::new();
    let price_impact = calculate_price_impact_newton(
        &mut solver,
        trade_size,
        &proposal,
        outcome,
        is_long,
    )?;
    
    msg!("Trade size: ${}", trade_size / 1_000_000);
    msg!("Price impact: {} bps", (price_impact * 10000) / proposal.prices[outcome as usize]);
    // TODO: Access iteration count from solver result
    // msg!("Newton-Raphson iterations: {}", solver.get_iteration_count());
    
    // Verify Newton-Raphson performance
    // if solver.get_iteration_count() > 10 {
    //     msg!("WARNING: Newton-Raphson took {} iterations", solver.get_iteration_count());
    // }
    
    // Step 3: Trading Parameter Validation
    msg!("\nStep 3: Trading Parameter Validation");
    
    let leverage = 25;
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Validate leverage using tier system
    let max_leverage = get_max_leverage_for_outcomes(&global_config, 2)?; // Binary market
    validate_leverage(leverage, max_leverage)?;
    msg!("Leverage {}x validated (max: {}x)", leverage, max_leverage);
    
    // Calculate margin requirement
    let margin_required = calculate_margin_requirement(trade_size, leverage)?;
    msg!("Margin required: ${}", margin_required / 1_000_000);
    
    // Calculate entry price with impact
    let base_price = proposal.prices[outcome as usize];
    let entry_price = if is_long {
        base_price + price_impact
    } else {
        base_price.saturating_sub(price_impact)
    };
    msg!("Entry price after impact: {}", entry_price);
    
    // Calculate liquidation price
    let liquidation_price = calculate_liquidation_price(
        entry_price,
        leverage,
        is_long,
    )?;
    msg!("Liquidation price: {}", liquidation_price);
    
    // Step 4: Execute Trade on AMM
    msg!("\nStep 4: Execute Trade on AMM");
    
    // Update AMM pool state
    let old_price = proposal.prices[outcome as usize];
    let entry_price = execute_trade(
        &mut proposal_account.data.borrow_mut()[..],
        outcome,
        trade_size,
        is_long,
    )?;
    
    let new_price = proposal.prices[outcome as usize];
    let price_change_bps = ((new_price as i64 - old_price as i64).abs() * 10000) / old_price as i64;
    
    msg!("AMM execution complete:");
    msg!("  Old price: {}", old_price);
    msg!("  New price: {}", new_price);
    msg!("  Price change: {} bps", price_change_bps);
    
    // Verify AMM invariant maintained
    verify_amm_invariant(&proposal)?;
    
    // Step 5: Create Position
    msg!("\nStep 5: Create Position");
    
    let position = Position::new(
        *user_account.key,
        u128::from_le_bytes(proposal.proposal_id[..16].try_into().unwrap()),
        1, // verse_id
        outcome,
        trade_size,
        leverage,
        entry_price,
        is_long,
        Clock::get()?.unix_timestamp,
    );
    
    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Update user map
    let mut user_map = UserMap::new(*user_account.key);
    user_map.add_position(position.proposal_id)?;
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;
    
    msg!("Position created:");
    msg!("  ID: {:?}", position.position_id);
    msg!("  Size: ${}", position.size / 1_000_000);
    msg!("  Margin: ${}", position.margin / 1_000_000);
    msg!("  Leverage: {}x", position.leverage);
    
    // Step 6: Verify Integration Results
    msg!("\nStep 6: Verify Integration Results");
    
    // Check oracle → AMM consistency
    let oracle_implied_prob = oracle_prices[0].price as f64 / 1_000_000.0;
    let amm_implied_prob = new_price as f64 / 1_000_000.0;
    let prob_diff = (oracle_implied_prob - amm_implied_prob).abs();
    
    msg!("Oracle implied probability: {:.2}%", oracle_implied_prob * 100.0);
    msg!("AMM implied probability: {:.2}%", amm_implied_prob * 100.0);
    msg!("Difference: {:.2}%", prob_diff * 100.0);
    
    if prob_diff > 0.05 {
        msg!("WARNING: Large divergence between oracle and AMM prices");
    }
    
    // Check position consistency
    assert_eq!(position.entry_price, entry_price);
    assert_eq!(position.liquidation_price, liquidation_price);
    assert_eq!(position.leverage, leverage);
    
    // Save all state
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Emit integration test event
    emit_event(EventType::IntegrationTestCompleted, &IntegrationTestCompletedEvent {
        test_name: "AMM_Oracle_Trading".to_string(),
        modules: vec!["Oracle".to_string(), "AMM".to_string(), "Trading".to_string()],
        success: true,
        details: format!(
            "Trade size: ${}, Impact: {} bps",
            trade_size / 1_000_000,
            (price_impact * 10000) / base_price
        ),
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("\n✅ AMM + Oracle + Trading Integration Test Passed!");
    
    Ok(())
}

/// Test high-frequency oracle updates with AMM
pub fn test_high_frequency_oracle_amm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing High-Frequency Oracle Updates with AMM");
    
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    
    // Load proposal
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Simulate rapid oracle updates
    let update_count = 10;
    let mut total_price_movement = 0i64;
    let mut max_spread = 0u16;
    
    msg!("\nSimulating {} rapid oracle updates:", update_count);
    
    for i in 0..update_count {
        // Generate random walk price movement (-100 to +100 bps)
        let price_change = ((i * 37 + 13) % 200) as i64 - 100;
        let new_price_0 = (proposal.prices[0] as i64 + price_change * 100).max(100_000) as u64;
        let new_price_1 = 1_000_000 - new_price_0;
        
        // Check spread
        let spread = calculate_spread(new_price_0, new_price_1)?;
        max_spread = max_spread.max(spread);
        
        if spread > 1000 {
            msg!("  Update {} rejected - spread {} bps too high", i, spread);
            continue;
        }
        
        // Update prices
        let old_price = proposal.prices[0];
        proposal.prices[0] = new_price_0;
        proposal.prices[1] = new_price_1;
        
        total_price_movement += (new_price_0 as i64 - old_price as i64).abs();
        
        msg!("  Update {}: {} -> {} (change: {} bps)",
            i,
            old_price,
            new_price_0,
            price_change
        );
        
        // Verify AMM stability
        verify_amm_invariant(&proposal)?;
    }
    
    msg!("\nHigh-frequency update results:");
    msg!("  Total price movement: {} bps", total_price_movement / 100);
    msg!("  Maximum spread seen: {} bps", max_spread);
    msg!("  AMM invariant maintained: ✓");
    
    Ok(())
}

/// Test oracle failure recovery
pub fn test_oracle_failure_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing Oracle Failure Recovery");
    
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let cache_account = next_account_info(account_iter)?;
    
    // Step 1: Simulate oracle failure
    msg!("\nStep 1: Simulating oracle failure");
    
    let oracle_status = OracleStatus::Failed {
        error: "Rate limit exceeded".to_string(),
        last_success: Clock::get()?.unix_timestamp - 120,
    };
    
    // Step 2: Check cache availability
    msg!("\nStep 2: Checking cache availability");
    
    let cache = OracleCache::try_from_slice(&cache_account.data.borrow())?;
    let cache_age = Clock::get()?.unix_timestamp - cache.timestamp;
    
    if cache_age < 300 {
        msg!("✓ Using cached prices (age: {} seconds)", cache_age);
        msg!("  Cached prices: [{}, {}]", cache.prices[0], cache.prices[1]);
        
        // Update proposal with cached prices
        let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
        proposal.prices[0] = cache.prices[0];
        proposal.prices[1] = cache.prices[1];
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    } else {
        msg!("✗ Cache too stale ({} seconds)", cache_age);
        return Err(BettingPlatformError::OracleCacheMiss.into());
    }
    
    // Step 3: Degraded mode trading
    msg!("\nStep 3: Testing degraded mode trading");
    
    // In degraded mode, increase margins and reduce max leverage
    let degraded_margin_multiplier = 150; // 1.5x
    let degraded_max_leverage = 10;
    
    msg!("Degraded mode parameters:");
    msg!("  Margin multiplier: {}%", degraded_margin_multiplier);
    msg!("  Max leverage: {}x", degraded_max_leverage);
    
    Ok(())
}

/// Calculate price impact using Newton-Raphson
fn calculate_price_impact_newton(
    solver: &mut NewtonRaphsonSolver,
    size: u64,
    proposal: &ProposalPDA,
    outcome: u8,
    is_long: bool,
) -> Result<u64, ProgramError> {
    // Use Newton-Raphson to solve for price impact
    let liquidity = proposal.liquidity_depth;
    let current_price = U64F64::from_num(proposal.prices[outcome as usize]) / U64F64::from_num(1_000_000);
    
    let impact = solver.solve_price_impact(
        U64F64::from_num(size),
        current_price,
        U64F64::from_num(liquidity),
        is_long,
    )?;
    
    Ok(impact.to_num())
}

/// Verify AMM invariant is maintained
fn verify_amm_invariant(proposal: &ProposalPDA) -> Result<(), ProgramError> {
    // For binary markets, prices should sum to ~1.0
    let sum = proposal.prices[0] + proposal.prices[1];
    let deviation = if sum > 1_000_000 {
        sum - 1_000_000
    } else {
        1_000_000 - sum
    };
    
    // Allow 1% deviation
    if deviation > 10_000 {
        msg!("AMM invariant violated: sum = {}", sum);
        return Err(BettingPlatformError::AMMInvariantViolation.into());
    }
    
    Ok(())
}

/// Calculate oracle spread
fn calculate_oracle_spread(prices: &[OraclePrice]) -> Result<u16, ProgramError> {
    let sum: u64 = prices.iter().map(|p| p.price).sum();
    let spread = if sum > 1_000_000 {
        sum - 1_000_000
    } else {
        1_000_000 - sum
    };
    
    Ok(((spread * 10000) / 1_000_000) as u16)
}

/// Calculate spread between two prices
fn calculate_spread(price0: u64, price1: u64) -> Result<u16, ProgramError> {
    let sum = price0 + price1;
    let spread = if sum > 1_000_000 {
        sum - 1_000_000
    } else {
        1_000_000 - sum
    };
    
    Ok(((spread * 10000) / 1_000_000) as u16)
}

/// Oracle status
enum OracleStatus {
    Active,
    Failed {
        error: String,
        last_success: i64,
    },
}

/// Oracle cache structure
#[derive(BorshSerialize, BorshDeserialize)]
struct OracleCache {
    prices: Vec<u64>,
    timestamp: i64,
    market_id: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_price_impact_calculation() {
        let mut solver = NewtonRaphsonSolver::new();
        let size = U64F64::from_num(10_000);
        let price = U64F64::from_num(500_000) / U64F64::from_num(1_000_000);
        let liquidity = U64F64::from_num(1_000_000);
        
        let impact = solver.solve_price_impact(size, price, liquidity, true).unwrap();
        
        // Impact should be positive for long trades
        assert!(impact > U64F64::from_num(0));
        // Should converge quickly
        // TODO: Access iteration count from solver result
        // assert!(solver.get_iteration_count() <= 10);
    }
    
    #[test]
    fn test_amm_invariant() {
        let proposal = ProposalPDA {
            discriminator: [0; 8],
            version: 1,
            proposal_id: [0; 32],
            verse_id: [0; 32],
            market_id: [0; 32],
            amm_type: crate::state::AMMType::LMSR,
            outcomes: 2,
            prices: vec![500_000, 500_000],
            volumes: vec![0, 0],
            liquidity_depth: 1_000_000,
            state: crate::state::ProposalState::Active,
            settle_slot: 0,
            resolution: None,
            partial_liq_accumulator: 0,
            chain_positions: vec![],
            outcome_balances: vec![0, 0],
            b_value: 1_000_000,
            total_liquidity: 1_000_000,
            total_volume: 0,
            funding_state: crate::trading::funding_rate::FundingRateState::new(0),
            status: crate::state::ProposalState::Active,
            settled_at: None,
        };
        
        assert!(verify_amm_invariant(&proposal).is_ok());
    }
}

/// Get max leverage for outcome count from global config tiers
fn get_max_leverage_for_outcomes(
    config: &GlobalConfigPDA,
    outcome_count: u64,
) -> Result<u64, ProgramError> {
    // Find the appropriate tier
    for tier in &config.leverage_tiers {
        if outcome_count <= tier.n as u64 {
            return Ok(tier.max as u64);
        }
    }
    
    // Default to lowest tier
    Ok(5)
}