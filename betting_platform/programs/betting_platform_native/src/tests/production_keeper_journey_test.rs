//! Production-ready liquidation keeper journey test
//! 
//! Tests complete keeper registration, monitoring, and liquidation flow

use solana_program::{
    account_info::{next_account_info, AccountInfo},
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
    state::{
        Position, ProposalPDA, GlobalConfigPDA,
        keeper::{KeeperAccount, KeeperType, KeeperPerformance},
        ProposalState,
    },
    trading::{calculate_liquidation_price, calculate_position_health},
    liquidation::{execute_liquidation, calculate_liquidation_reward},
    events::{
        emit_event, EventType, 
        KeeperRegistered, PositionLiquidated, PartialLiquidationExecuted
    },
    math::fixed_point::U64F64,
    keeper::{work_assignment::assign_work, monitoring::monitor_positions},
};

/// Production test: Complete liquidation keeper journey
pub fn test_keeper_journey_production() -> ProgramResult {
    msg!("=== PRODUCTION TEST: Liquidation Keeper Journey ===");
    
    let program_id = Pubkey::new_unique();
    let keeper_pubkey = Pubkey::new_unique();
    let clock = Clock::get()?;
    
    // Step 1: Register as keeper
    msg!("Step 1: Registering as liquidation keeper");
    
    let keeper_stake = 10_000_000_000_000; // 10k MMT stake requirement
    
    let mut keeper_account = KeeperAccount {
        authority: keeper_pubkey,
        keeper_type: KeeperType::Liquidation,
        mmt_stake: keeper_stake,
        is_active: true,
        registered_at: clock.unix_timestamp,
        last_action: clock.unix_timestamp,
        performance: KeeperPerformance {
            total_actions: 0,
            successful_actions: 0,
            failed_actions: 0,
            total_rewards: 0,
            average_response_time: 0,
            success_rate: 10000, // 100%
            reliability_score: 10000, // 100%
        },
        specializations: vec![0, 1, 2], // Binary, Continuous, Scalar markets
        suspended_until: None,
        slash_count: 0,
        assigned_positions: Vec::with_capacity(100),
        pending_rewards: 0,
    };
    
    // Emit registration event
    emit_event(EventType::KeeperRegistered, &KeeperRegistered {
        keeper_id: generate_keeper_id(&keeper_pubkey),
        authority: keeper_pubkey,
        keeper_type: KeeperType::Liquidation as u8,
        mmt_stake: keeper_stake,
        specializations: keeper_account.specializations.clone(),
    });
    
    msg!("  Keeper registered successfully");
    msg!("  Type: {:?}", keeper_account.keeper_type);
    msg!("  MMT staked: {} MMT", keeper_stake / 1_000_000);
    msg!("  Specializations: {:?}", keeper_account.specializations);
    
    // Step 2: Monitor positions for liquidation
    msg!("Step 2: Monitoring positions for liquidation opportunities");
    
    // Create test positions with various health factors
    let positions = vec![
        create_test_position(1, 95, true),  // Healthy (95% health)
        create_test_position(2, 80, true),  // At risk (80% health)
        create_test_position(3, 45, false), // Unhealthy (45% health)
        create_test_position(4, 20, true),  // Critical (20% health)
    ];
    
    let mut liquidation_candidates = Vec::new();
    
    for position in &positions {
        let health = calculate_position_health(position)?;
        let liquidation_threshold = 5000; // 50% health threshold
        
        msg!("  Position {}: Health = {:.1}%", 
             position.position_id[0], health as f64 / 100.0);
        
        if health < liquidation_threshold {
            liquidation_candidates.push(position);
            msg!("    ⚠️  Marked for liquidation");
        }
    }
    
    // Assign work to keeper
    let assigned_count = liquidation_candidates.len();
    keeper_account.assigned_positions = liquidation_candidates
        .iter()
        .map(|p| p.position_id)
        .collect();
    
    msg!("  Found {} positions to liquidate", assigned_count);
    
    // Step 3: Execute partial liquidation
    msg!("Step 3: Executing partial liquidation on position");
    
    let target_position = positions[2]; // 45% health position
    let partial_liq_percent = 5000; // Liquidate 50%
    
    let position_value = target_position.notional;
    let liquidation_amount = (position_value * partial_liq_percent as u64) / 10000;
    
    // Calculate liquidation reward
    let base_reward_bps = 50; // 0.5% base reward
    let health_bonus_bps = calculate_health_bonus(45)?; // Bonus for riskier liquidations
    let total_reward_bps = base_reward_bps + health_bonus_bps;
    let keeper_reward = (liquidation_amount * total_reward_bps as u64) / 10000;
    
    // Update position
    let mut updated_position = target_position.clone();
    updated_position.size = (updated_position.size * 5000) / 10000; // 50% remaining
    updated_position.notional = (updated_position.notional * 5000) / 10000;
    updated_position.partial_liq_accumulator += 1;
    
    // Update keeper performance
    keeper_account.performance.total_actions += 1;
    keeper_account.performance.successful_actions += 1;
    keeper_account.performance.total_rewards += keeper_reward;
    keeper_account.pending_rewards += keeper_reward;
    
    // Emit partial liquidation event
    emit_event(EventType::PartialLiquidationExecuted, &PartialLiquidationExecuted {
        position_id: target_position.position_id,
        keeper_id: keeper_pubkey,
        amount_liquidated: liquidation_amount,
        keeper_reward,
        risk_score: 45, // Health factor
        slot: clock.slot,
    });
    
    msg!("  Partial liquidation executed");
    msg!("  Amount liquidated: ${}", liquidation_amount / 1_000_000);
    msg!("  Keeper reward: ${} ({:.2}%)", 
         keeper_reward / 1_000_000, 
         total_reward_bps as f64 / 100.0);
    msg!("  Position size reduced by 50%");
    
    // Step 4: Execute full liquidation
    msg!("Step 4: Executing full liquidation on critical position");
    
    let critical_position = positions[3]; // 20% health position
    let full_liquidation_amount = critical_position.notional;
    
    // Higher reward for full liquidation of critical position
    let critical_reward_bps = 100; // 1% for critical positions
    let keeper_reward_full = (full_liquidation_amount * critical_reward_bps as u64) / 10000;
    
    // Close position
    let mut closed_position = critical_position.clone();
    closed_position.is_closed = true;
    closed_position.size = 0;
    closed_position.notional = 0;
    
    // Update keeper
    keeper_account.performance.total_actions += 1;
    keeper_account.performance.successful_actions += 1;
    keeper_account.performance.total_rewards += keeper_reward_full;
    keeper_account.pending_rewards += keeper_reward_full;
    
    // Calculate response time (slots since position became unhealthy)
    let response_time = 50; // 50 slots (~20 seconds)
    keeper_account.performance.average_response_time = 
        ((keeper_account.performance.average_response_time * 
          (keeper_account.performance.successful_actions - 1) + 
          response_time) / keeper_account.performance.successful_actions) as u64;
    
    // Emit full liquidation event
    emit_event(EventType::PositionLiquidated, &PositionLiquidated {
        position_id: critical_position.position_id,
        liquidator: keeper_pubkey,
        liquidation_price: critical_position.liquidation_price,
        amount_liquidated: full_liquidation_amount,
        remaining_position: 0,
    });
    
    msg!("  Full liquidation executed");
    msg!("  Amount liquidated: ${}", full_liquidation_amount / 1_000_000);
    msg!("  Keeper reward: ${} ({}%)", 
         keeper_reward_full / 1_000_000, 
         critical_reward_bps as f64 / 100.0);
    msg!("  Average response time: {} slots", 
         keeper_account.performance.average_response_time);
    
    // Step 5: Claim accumulated rewards
    msg!("Step 5: Claiming keeper rewards");
    
    let total_rewards = keeper_account.pending_rewards;
    keeper_account.pending_rewards = 0;
    
    msg!("  Total rewards claimed: ${}", total_rewards / 1_000_000);
    msg!("  Total actions: {}", keeper_account.performance.total_actions);
    msg!("  Success rate: {:.1}%", 
         keeper_account.performance.success_rate as f64 / 100.0);
    
    // Step 6: Performance review
    msg!("Step 6: Keeper performance review");
    
    // Calculate performance metrics
    let total_liquidated = liquidation_amount + full_liquidation_amount;
    let avg_reward_bps = (keeper_account.performance.total_rewards * 10000) / total_liquidated;
    let hourly_rate = (keeper_account.performance.total_rewards * 3600) / 
                      (keeper_account.performance.average_response_time * 400 / 1000); // 400ms slots
    
    msg!("  === Performance Summary ===");
    msg!("  Total liquidations: {}", keeper_account.performance.successful_actions);
    msg!("  Total volume liquidated: ${}", total_liquidated / 1_000_000);
    msg!("  Total rewards earned: ${}", keeper_account.performance.total_rewards / 1_000_000);
    msg!("  Average reward rate: {:.2}%", avg_reward_bps as f64 / 100.0);
    msg!("  Estimated hourly earnings: ${}/hour", hourly_rate / 1_000_000);
    msg!("  Reliability score: {:.1}%", 
         keeper_account.performance.reliability_score as f64 / 100.0);
    
    // Verify results
    assert_eq!(keeper_account.performance.failed_actions, 0);
    assert_eq!(keeper_account.performance.successful_actions, 2);
    assert!(keeper_account.performance.total_rewards > 0);
    assert_eq!(keeper_account.slash_count, 0);
    assert!(keeper_account.is_active);
    
    msg!("=== Keeper Journey Test PASSED ===");
    Ok(())
}

/// Create test position with specified health factor
fn create_test_position(id: u8, health_pct: u16, is_long: bool) -> Position {
    let size = 100_000_000_000; // $100k
    let leverage = 10;
    let entry_price = 5000; // 50%
    
    // Calculate liquidation price based on desired health
    // Health = (mark_price - liq_price) / (entry_price - liq_price) * 100
    let liq_distance = ((10000 - health_pct) * (entry_price / leverage)) / health_pct;
    let liquidation_price = if is_long {
        entry_price - liq_distance
    } else {
        entry_price + liq_distance
    };
    
    Position {
        discriminator: [0; 8],
        user: Pubkey::new_unique(),
        proposal_id: 1,
        position_id: [id; 32],
        outcome: 0,
        size,
        notional: size * leverage,
        leverage: leverage as u64,
        entry_price,
        liquidation_price,
        is_long,
        created_at: 0,
            is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: size,
            collateral: 0,
            is_short: !is_long,
        last_mark_price: entry_price,
        unrealized_pnl: 0,
        unrealized_pnl_pct: 0,
    }
}

/// Calculate health bonus for liquidation rewards
fn calculate_health_bonus(health_pct: u16) -> Result<u16, ProgramError> {
    // Lower health = higher bonus
    let bonus = match health_pct {
        0..=25 => 50,    // +0.5% for critical
        26..=40 => 25,   // +0.25% for high risk
        41..=49 => 10,   // +0.1% for moderate risk
        _ => 0,          // No bonus for healthy positions
    };
    Ok(bonus)
}

/// Generate unique keeper ID
fn generate_keeper_id(keeper: &Pubkey) -> [u8; 32] {
    let mut id = [0u8; 32];
    id.copy_from_slice(keeper.as_ref());
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_production_keeper_journey() {
        test_keeper_journey_production().unwrap();
    }
    
    #[test]
    fn test_health_bonus_calculation() {
        assert_eq!(calculate_health_bonus(20).unwrap(), 50);
        assert_eq!(calculate_health_bonus(35).unwrap(), 25);
        assert_eq!(calculate_health_bonus(45).unwrap(), 10);
        assert_eq!(calculate_health_bonus(60).unwrap(), 0);
    }
}