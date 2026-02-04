//! Cascade Liquidation Edge Case Testing
//! 
//! Tests cascade liquidation scenarios and circuit breaker activation

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
    state::{GlobalConfigPDA, ProposalPDA, Position},
    liquidation::{should_liquidate_coverage_based, calculate_liquidation_amount},
    circuit_breaker::CircuitBreaker,
    math::U64F64,
    state::security_accounts::CircuitBreakerType,
    events::{emit_event, EventType, CascadeLiquidationDetectedEvent, CascadeRecoveredEvent, SystemWideHaltEvent},
};

/// Test cascade liquidation scenario
pub fn test_cascade_liquidation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let global_config_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let circuit_breaker_account = next_account_info(account_iter)?;
    
    msg!("Testing cascade liquidation scenario");
    
    // Load accounts
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut circuit_breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
    
    // Setup: Create multiple leveraged positions
    let positions = vec![
        create_test_position(1, 495_000, 50, 100_000_000_000), // 50x, $100k
        create_test_position(2, 496_000, 40, 80_000_000_000),  // 40x, $80k
        create_test_position(3, 497_000, 30, 60_000_000_000),  // 30x, $60k
        create_test_position(4, 498_000, 25, 50_000_000_000),  // 25x, $50k
        create_test_position(5, 499_000, 20, 40_000_000_000),  // 20x, $40k
    ];
    
    msg!("Created {} test positions with varying leverage", positions.len());
    
    // Calculate total open interest
    let total_oi: u64 = positions.iter().map(|p| p.size).sum();
    global_config.total_oi = total_oi as u128;
    
    msg!("Total open interest: ${}", total_oi / 1_000_000);
    
    // Step 1: Simulate initial price drop
    msg!("\nStep 1: Simulating 2% price drop");
    let initial_price = 500_000;
    let shocked_price = 490_000; // 2% drop
    
    proposal.prices[0] = shocked_price;
    proposal.prices[1] = 1_000_000 - shocked_price;
    
    // Step 2: Check which positions should liquidate
    msg!("\nStep 2: Checking liquidation status");
    let mut liquidation_count = 0;
    let mut liquidation_volume = 0u64;
    
    for position in &positions {
        if shocked_price <= position.liquidation_price {
            liquidation_count += 1;
            liquidation_volume += position.size;
            msg!("  Position {} LIQUIDATED (liq price: {})", 
                position.proposal_id, position.liquidation_price);
        } else {
            msg!("  Position {} safe (liq price: {})", 
                position.proposal_id, position.liquidation_price);
        }
    }
    
    let liquidation_rate = (liquidation_count * 10000) / positions.len();
    msg!("\nLiquidation rate: {} bps ({}/{})", 
        liquidation_rate, liquidation_count, positions.len());
    
    // Step 3: Simulate cascade effect
    msg!("\nStep 3: Simulating cascade effect");
    
    // First wave liquidations cause additional selling pressure
    let cascade_price_impact = calculate_cascade_impact(liquidation_volume, &proposal)?;
    let cascade_price = shocked_price.saturating_sub(cascade_price_impact);
    
    msg!("Cascade selling pressure: {} price impact", cascade_price_impact);
    msg!("New price after cascade: {}", cascade_price);
    
    // Check second wave liquidations
    let mut second_wave_count = 0;
    let mut second_wave_volume = 0u64;
    
    for position in &positions {
        if cascade_price <= position.liquidation_price && shocked_price > position.liquidation_price {
            second_wave_count += 1;
            second_wave_volume += position.size;
            msg!("  Position {} liquidated in CASCADE", position.proposal_id);
        }
    }
    
    msg!("\nSecond wave liquidations: {}", second_wave_count);
    msg!("Total liquidations: {}", liquidation_count + second_wave_count);
    
    // Step 4: Check circuit breaker activation
    let total_liquidation_rate = ((liquidation_count + second_wave_count) * 10000) / positions.len();
    const CASCADE_THRESHOLD_BPS: usize = 3000; // 30% threshold
    
    if total_liquidation_rate > CASCADE_THRESHOLD_BPS {
        msg!("\nStep 4: CASCADE DETECTED - Activating circuit breaker!");
        msg!("Liquidation rate {} bps exceeds {} bps threshold", 
            total_liquidation_rate, CASCADE_THRESHOLD_BPS);
        
        // Activate circuit breaker
        circuit_breaker.is_active = true;
        circuit_breaker.breaker_type = Some(CircuitBreakerType::Liquidation);
        circuit_breaker.triggered_at = Some(Clock::get()?.slot);
        circuit_breaker.reason = Some(format!("Cascade liquidation {} bps", total_liquidation_rate));
        
        // Halt market
        proposal.state = crate::state::ProposalState::Paused;
        
        // Save state
        circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
        
        emit_event(EventType::CascadeLiquidationDetected, &CascadeLiquidationDetectedEvent {
            market_id: proposal.market_id,
            initial_liquidations: liquidation_count as u32,
            cascade_liquidations: second_wave_count as u32,
            total_liquidation_rate_bps: total_liquidation_rate as u16,
            circuit_breaker_activated: true,
            timestamp: Clock::get()?.unix_timestamp,
        });
    } else {
        msg!("\nStep 4: Liquidation rate within acceptable limits");
    }
    
    // Step 5: Test recovery mechanism
    msg!("\nStep 5: Testing cascade recovery");
    
    if circuit_breaker.is_active {
        // Simulate stabilization period
        let slots_elapsed = 100;
        let price_recovered = 495_000; // Partial recovery
        
        msg!("After {} slots, price recovers to {}", slots_elapsed, price_recovered);
        
        // Check if cascade has stopped
        let mut remaining_at_risk = 0;
        for position in &positions {
            if price_recovered <= position.liquidation_price && position.size > 0 {
                remaining_at_risk += 1;
            }
        }
        
        let risk_rate = (remaining_at_risk * 10000) / positions.len();
        
        if risk_rate < 1000 { // Less than 10% at risk
            msg!("Risk stabilized at {} bps - lifting circuit breaker", risk_rate);
            
            circuit_breaker.is_active = false;
            circuit_breaker.resolved_at = Some((Clock::get()?.slot + slots_elapsed) as i64);
            proposal.state = crate::state::ProposalState::Active;
            
            emit_event(EventType::CascadeRecovered, &CascadeRecoveredEvent {
                market_id: proposal.market_id,
                recovery_price: price_recovered,
                remaining_at_risk: remaining_at_risk as u32,
                duration_slots: slots_elapsed,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
    }
    
    msg!("\nCascade liquidation test completed");
    
    Ok(())
}

/// Test cascade prevention mechanisms
pub fn test_cascade_prevention(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing cascade prevention mechanisms");
    
    // Mechanism 1: Partial liquidations
    msg!("\nMechanism 1: Partial liquidations");
    
    let position = create_test_position(1, 495_000, 50, 100_000_000_000);
    let partial_percentage = 3000; // 30% partial liquidation
    
    let liquidation_amount = (position.size * partial_percentage as u64) / 10000;
    let remaining = position.size - liquidation_amount;
    
    msg!("  Original position: ${}", position.size / 1_000_000);
    msg!("  Partial liquidation: ${} ({}%)", 
        liquidation_amount / 1_000_000, partial_percentage / 100);
    msg!("  Remaining position: ${}", remaining / 1_000_000);
    
    // Check if partial liquidation improves health
    let new_leverage = position.leverage * partial_percentage as u64 / 10000;
    msg!("  New effective leverage: {}x", new_leverage);
    
    // Mechanism 2: Liquidation speed limits
    msg!("\nMechanism 2: Liquidation speed limits");
    
    const MAX_LIQUIDATIONS_PER_SLOT: u32 = 5;
    let pending_liquidations = 20;
    
    let slots_needed = (pending_liquidations + MAX_LIQUIDATIONS_PER_SLOT - 1) / MAX_LIQUIDATIONS_PER_SLOT;
    msg!("  {} liquidations will take {} slots", pending_liquidations, slots_needed);
    msg!("  This provides {} seconds for market recovery", 
        slots_needed as f64 * 0.4);
    
    // Mechanism 3: Dynamic margin requirements
    msg!("\nMechanism 3: Dynamic margin requirements");
    
    let base_margin_rate = 200; // 2%
    let volatility_multiplier = 150; // 1.5x during high volatility
    let cascade_multiplier = 200; // 2x during cascade risk
    
    let normal_margin = base_margin_rate;
    let volatile_margin = (base_margin_rate * volatility_multiplier) / 100;
    let cascade_margin = (base_margin_rate * cascade_multiplier) / 100;
    
    msg!("  Normal market: {}% margin", normal_margin as f64 / 100.0);
    msg!("  High volatility: {}% margin", volatile_margin as f64 / 100.0);
    msg!("  Cascade risk: {}% margin", cascade_margin as f64 / 100.0);
    
    // Mechanism 4: Insurance fund activation
    msg!("\nMechanism 4: Insurance fund activation");
    
    let insurance_fund_size: i64 = 10_000_000_000_000i64; // $10M
    let max_coverage_per_event: i64 = 1_000_000_000_000i64; // $1M
    
    msg!("  Insurance fund: ${}", insurance_fund_size / 1_000_000);
    msg!("  Max coverage per event: ${}", max_coverage_per_event / 1_000_000);
    
    // Simulate insurance payout
    let liquidation_shortfall: i64 = 500_000_000_000i64; // $500k
    if liquidation_shortfall <= max_coverage_per_event {
        msg!("  ✓ Insurance covers ${} shortfall", liquidation_shortfall / 1_000_000);
    }
    
    Ok(())
}

/// Test cross-market cascade
pub fn test_cross_market_cascade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing cross-market cascade effects");
    
    // Simulate correlated markets
    let markets = vec![
        ("BTC-USD", 500_000, 0.0),   // Reference market
        ("ETH-USD", 500_000, 0.85),  // 85% correlation
        ("SOL-USD", 500_000, 0.75),  // 75% correlation
        ("AVAX-USD", 500_000, 0.65), // 65% correlation
    ];
    
    // Initial shock in BTC
    let btc_shock = -5.0; // -5% price shock
    msg!("Initial shock: BTC {} %", btc_shock);
    
    // Calculate cascade effects
    msg!("\nCascade propagation:");
    
    for (market, base_price, correlation) in &markets {
        let market_shock = btc_shock * correlation;
        let new_price = (*base_price as f64 * (1.0 + market_shock / 100.0)) as u64;
        
        msg!("  {} correlation {:.0}%: {:.1}% shock -> price {}",
            market, correlation * 100.0, market_shock, new_price);
        
        // Check if market needs protection
        if market_shock.abs() > 3.0 {
            msg!("    ⚠️  Market protection activated");
        }
    }
    
    // Calculate system-wide impact
    let avg_correlation = 0.675; // Average of non-BTC correlations
    let system_impact = btc_shock * avg_correlation;
    
    msg!("\nSystem-wide impact: {:.1}%", system_impact);
    
    if system_impact.abs() > 2.5 {
        msg!("⚠️  SYSTEM-WIDE CIRCUIT BREAKER ACTIVATED");
        
        emit_event(EventType::SystemWideHalt, &SystemWideHaltEvent {
            trigger_market: "BTC-USD".to_string(),
            initial_shock_bps: (btc_shock * 100.0) as i16,
            affected_markets: markets.len() as u32,
            system_impact_bps: (system_impact * 100.0) as i16,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }
    
    Ok(())
}

/// Create test position
fn create_test_position(
    id: u128,
    liquidation_price: u64,
    leverage: u64,
    size: u64,
) -> Position {
    Position {
        discriminator: [0; 8],
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        user: Pubkey::default(),
        proposal_id: id,
        position_id: [id as u8; 32],
        outcome: 0,
        size,
        notional: size,
        leverage,
        entry_price: 500_000,
        liquidation_price,
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 0,
        last_mark_price: 500_000,
        unrealized_pnl: 0,
        unrealized_pnl_pct: 0,
        verse_id: 0,
        margin: size / leverage,
        collateral: 0,
        is_short: false,
        cross_margin_enabled: false,
        entry_funding_index: Some(U64F64::from_num(0)),
    }
}

/// Calculate cascade price impact
fn calculate_cascade_impact(
    liquidation_volume: u64,
    proposal: &ProposalPDA,
) -> Result<u64, ProgramError> {
    // Simplified model: 0.1% price impact per $1M liquidated
    let impact_per_million = 100; // 0.01% in basis points
    let millions_liquidated = liquidation_volume / 1_000_000_000_000;
    
    let total_impact_bps = millions_liquidated * impact_per_million;
    let price_impact = (proposal.prices[0] * total_impact_bps) / 1_000_000;
    
    Ok(price_impact)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cascade_detection() {
        let positions = vec![
            create_test_position(1, 495_000, 50, 100_000_000_000),
            create_test_position(2, 496_000, 40, 80_000_000_000),
            create_test_position(3, 497_000, 30, 60_000_000_000),
            create_test_position(4, 498_000, 25, 50_000_000_000),
            create_test_position(5, 499_000, 20, 40_000_000_000),
        ];
        
        // Count liquidations at 490k price
        let test_price = 490_000;
        let liquidations = positions.iter()
            .filter(|p| test_price <= p.liquidation_price)
            .count();
        
        assert_eq!(liquidations, 5); // All positions liquidated
        
        // Test cascade threshold
        let liquidation_rate = (liquidations * 10000) / positions.len();
        assert!(liquidation_rate > 3000); // Should trigger cascade protection
    }
}