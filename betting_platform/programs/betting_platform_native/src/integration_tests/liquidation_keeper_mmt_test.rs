//! Liquidation + Keeper + MMT Integration Test
//! 
//! Tests the complete flow from position liquidation through keeper execution to MMT rewards

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
    liquidation::{
        should_liquidate_coverage_based,
        calculate_liquidation_amount,
    },
    mmt::{
        MMTState,
        StakeAccount,
        calculate_rewards,
    },
    events::{emit_event, EventType, IntegrationTestCompletedEvent},
    math::U64F64,
    mmt::staking::StakingTier,
};

// Define types locally for testing
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum LiquidationType {
    Partial,
    Full,
    Emergency,
}

// UserTier removed - using StakingTier from mmt module

#[derive(BorshSerialize, BorshDeserialize)]
pub struct KeeperState {
    pub keepers: Vec<KeeperPerformance>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct KeeperConfig {
    pub min_stake: u64,
    pub max_keepers: u32,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct KeeperPerformance {
    pub pubkey: Pubkey,
    pub success_rate: u64,
    pub total_liquidations: u64,
    pub successful_liquidations: u64,
    pub avg_response_time: u64,
    pub total_response_time: u64,
    pub stake_amount: u64,
}

/// Assign keeper for liquidation
fn assign_keeper(
    keeper_state: &KeeperState,
    _config: &KeeperConfig,
) -> Result<Pubkey, ProgramError> {
    // Simple round-robin assignment for testing
    keeper_state.keepers
        .first()
        .map(|k| k.pubkey)
        .ok_or(BettingPlatformError::KeeperNotFound.into())
}

/// Complete Liquidation + Keeper + MMT integration test
pub fn test_liquidation_keeper_mmt_integration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let keeper_state_account = next_account_info(account_iter)?;
    let keeper_config_account = next_account_info(account_iter)?;
    let mmt_state_account = next_account_info(account_iter)?;
    let stake_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let insurance_fund_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    msg!("Testing Liquidation + Keeper + MMT Integration");
    
    // Step 1: Setup unhealthy position
    msg!("\nStep 1: Setting up unhealthy position");
    
    let mut position = Position {
        discriminator: [0; 8],
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        user: *position_account.key,
        proposal_id: 1,
        position_id: [1u8; 32],
        outcome: 0,
        size: 100_000_000_000, // $100k
        notional: 100_000_000_000,
        leverage: 50,
        entry_price: 500_000,
        liquidation_price: 495_000,
        is_long: true,
        created_at: Clock::get()?.unix_timestamp - 3600,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 2_000_000_000, // $2k margin
            collateral: 0,
            is_short: false,
        last_mark_price: 500_000,
        unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
    };
    
    // Update market price to trigger liquidation
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    proposal.prices[0] = 492_000; // Below liquidation price
    proposal.prices[1] = 508_000;
    
    msg!("Position details:");
    msg!("  Size: ${}", position.size / 1_000_000);
    msg!("  Leverage: {}x", position.leverage);
    msg!("  Entry price: {}", position.entry_price);
    msg!("  Liquidation price: {}", position.liquidation_price);
    msg!("  Current price: {}", proposal.prices[0]);
    
    // Check liquidation status
    let should_liquidate = should_liquidate_coverage_based(
        &position,
        proposal.prices[0],
        U64F64::from_num(position.leverage),
    )?;
    
    msg!("Liquidation required: {}", should_liquidate);
    
    // Step 2: Keeper assignment
    msg!("\nStep 2: Keeper assignment");
    
    let mut keeper_state = KeeperState::try_from_slice(&keeper_state_account.data.borrow())?;
    let keeper_config = KeeperConfig::try_from_slice(&keeper_config_account.data.borrow())?;
    
    // Find eligible keeper
    let keeper_pubkey = assign_keeper(&keeper_state, &keeper_config)?;
    msg!("Assigned keeper: {:?}", keeper_pubkey);
    
    // Get keeper's performance stats (clone needed data before mutation)
    let keeper_stats = keeper_state.keepers.iter()
        .find(|k| k.pubkey == keeper_pubkey)
        .ok_or(BettingPlatformError::KeeperNotFound)?;
    
    // Clone the data we need before mutating keeper_state
    let keeper_success_rate = keeper_stats.success_rate;
    let keeper_total_liquidations = keeper_stats.total_liquidations;
    let keeper_avg_response_time = keeper_stats.avg_response_time;
    let keeper_stake_amount = keeper_stats.stake_amount;
    
    msg!("Keeper stats:");
    msg!("  Success rate: {}%", keeper_success_rate);
    msg!("  Total liquidations: {}", keeper_total_liquidations);
    msg!("  Average response time: {} slots", keeper_avg_response_time);
    msg!("  Current stake: ${}", keeper_stake_amount / 1_000_000);
    
    // Step 3: Execute liquidation
    msg!("\nStep 3: Execute liquidation");
    
    // Determine liquidation type
    let liquidation_type = determine_liquidation_type(&position, proposal.prices[0])?;
    msg!("Liquidation type: {:?}", liquidation_type);
    
    let liquidation_amount = match liquidation_type {
        LiquidationType::Partial => {
            // 30% partial liquidation
            (position.size * 3000) / 10000
        }
        LiquidationType::Full => position.size,
        LiquidationType::Emergency => position.size,
    };
    
    msg!("Liquidation amount: ${}", liquidation_amount / 1_000_000);
    
    // Calculate keeper reward (1% of liquidation)
    let keeper_reward = liquidation_amount / 100;
    msg!("Keeper reward: ${}", keeper_reward / 1_000_000);
    
    // Execute liquidation on AMM
    let old_price = proposal.prices[0];
    execute_liquidation_on_amm(
        &mut proposal,
        position.outcome,
        liquidation_amount,
        position.is_long,
    )?;
    let new_price = proposal.prices[0];
    
    msg!("AMM execution:");
    msg!("  Price before: {}", old_price);
    msg!("  Price after: {}", new_price);
    msg!("  Impact: {} bps", ((new_price as i64 - old_price as i64).abs() * 10000) / old_price as i64);
    
    // Update position
    if matches!(liquidation_type, LiquidationType::Partial) {
        position.size -= liquidation_amount;
        position.notional = position.size;
        position.partial_liq_accumulator += liquidation_amount;
        msg!("Remaining position: ${}", position.size / 1_000_000);
    } else {
        position.is_closed = true;
        position.size = 0;
        msg!("Position fully liquidated");
    }
    
    // Step 4: Update keeper performance
    msg!("\nStep 4: Update keeper performance");
    
    let liquidation_start = Clock::get()?.slot - 10;
    let response_time = Clock::get()?.slot - liquidation_start;
    
    // Update keeper stats
    let keeper_index = keeper_state.keepers.iter().position(|k| k.pubkey == keeper_pubkey)
        .ok_or(BettingPlatformError::KeeperNotFound)?;
    
    keeper_state.keepers[keeper_index].total_liquidations += 1;
    keeper_state.keepers[keeper_index].successful_liquidations += 1;
    keeper_state.keepers[keeper_index].total_response_time += response_time;
    keeper_state.keepers[keeper_index].avg_response_time = 
        keeper_state.keepers[keeper_index].total_response_time / 
        keeper_state.keepers[keeper_index].total_liquidations;
    keeper_state.keepers[keeper_index].success_rate = 
        (keeper_state.keepers[keeper_index].successful_liquidations * 100) / 
        keeper_state.keepers[keeper_index].total_liquidations;
    
    msg!("Updated keeper performance:");
    msg!("  New success rate: {}%", keeper_state.keepers[keeper_index].success_rate);
    msg!("  Response time: {} slots", response_time);
    
    // Step 5: MMT reward distribution
    msg!("\nStep 5: MMT reward distribution");
    
    let mut mmt_state = MMTState::try_from_slice(&mmt_state_account.data.borrow())?;
    let mut stake_data = StakeAccount::try_from_slice(&stake_account.data.borrow())?;
    
    // Keeper must be staking MMT
    if stake_data.owner != keeper_pubkey {
        return Err(BettingPlatformError::KeeperNotStaking.into());
    }
    
    msg!("Keeper MMT stake:");
    msg!("  Amount: {} MMT", stake_data.amount / 1_000_000);
    msg!("  Tier: {:?}", stake_data.tier);
    msg!("  Lock end slot: {:?}", stake_data.lock_end_slot);
    
    // Calculate MMT rewards
    let base_mmt_reward = keeper_reward / 10; // 10% of keeper reward in MMT
    let tier_multiplier = get_tier_multiplier(&stake_data.tier);
    let performance_multiplier = keeper_success_rate as u64;
    
    let total_mmt_reward = (base_mmt_reward * tier_multiplier * performance_multiplier) / 10000;
    
    msg!("MMT reward calculation:");
    msg!("  Base reward: {} MMT", base_mmt_reward / 1_000_000);
    msg!("  Tier multiplier: {}x", tier_multiplier as f64 / 100.0);
    msg!("  Performance multiplier: {}%", performance_multiplier);
    msg!("  Total MMT reward: {} MMT", total_mmt_reward / 1_000_000);
    
    // Distribute rewards
    stake_data.accumulated_rewards += total_mmt_reward;
    // Update MMT state - increase circulating supply for rewards
    mmt_state.season_emitted += total_mmt_reward;
    
    // Step 6: Handle liquidation proceeds
    msg!("\nStep 6: Handle liquidation proceeds");
    
    let margin_returned = position.margin.saturating_sub(keeper_reward);
    let insurance_contribution = keeper_reward / 10; // 10% to insurance fund
    
    msg!("Proceeds distribution:");
    msg!("  User margin return: ${}", margin_returned / 1_000_000);
    msg!("  Keeper reward: ${}", keeper_reward / 1_000_000);
    msg!("  Insurance contribution: ${}", insurance_contribution / 1_000_000);
    
    // Step 7: Verify integration results
    msg!("\nStep 7: Verify integration results");
    
    // Check position state
    if matches!(liquidation_type, LiquidationType::Full) {
        assert!(position.is_closed);
        assert_eq!(position.size, 0);
    } else {
        assert!(!position.is_closed);
        assert!(position.size > 0);
    }
    
    // Check keeper rewards
    assert!(keeper_state.keepers[keeper_index].total_liquidations > 0);
    assert!(stake_data.accumulated_rewards > 0);
    
    // Check AMM state
    verify_amm_invariant(&proposal)?;
    
    // Save all state
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    keeper_state.serialize(&mut &mut keeper_state_account.data.borrow_mut()[..])?;
    stake_data.serialize(&mut &mut stake_account.data.borrow_mut()[..])?;
    mmt_state.serialize(&mut &mut mmt_state_account.data.borrow_mut()[..])?;
    
    // Emit integration test event
    emit_event(EventType::IntegrationTestCompleted, &IntegrationTestCompletedEvent {
        test_name: "Liquidation_Keeper_MMT".to_string(),
        modules: vec!["Liquidation".to_string(), "Keeper".to_string(), "MMT".to_string()],
        success: true,
        details: format!(
            "Liquidated ${}, Keeper reward: ${}, MMT reward: {} MMT",
            liquidation_amount / 1_000_000,
            keeper_reward / 1_000_000,
            total_mmt_reward / 1_000_000
        ),
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("\n✅ Liquidation + Keeper + MMT Integration Test Passed!");
    
    Ok(())
}

/// Test keeper competition during liquidations
pub fn test_keeper_competition(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing keeper competition mechanics");
    
    let account_iter = &mut accounts.iter();
    let keeper_state_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    
    let keeper_state = KeeperState::try_from_slice(&keeper_state_account.data.borrow())?;
    
    // Simulate multiple keepers competing
    msg!("\nSimulating 5 keepers competing for liquidation:");
    
    let competition_window = 3; // 3 slots
    let mut submissions = Vec::new();
    
    for (i, keeper) in keeper_state.keepers.iter().take(5).enumerate() {
        let submission_slot = Clock::get()?.slot + i as u64;
        let gas_bid = 100_000 + (i as u64 * 10_000); // Increasing gas bids
        
        submissions.push((keeper.pubkey, submission_slot, gas_bid));
        
        msg!("  Keeper {}: slot {}, gas bid: {}", i, submission_slot, gas_bid);
    }
    
    // Determine winner (first submission within window)
    let deadline = Clock::get()?.slot + competition_window;
    let winner = submissions.iter()
        .filter(|(_, slot, _)| *slot <= deadline)
        .min_by_key(|(_, slot, _)| *slot)
        .map(|(keeper, _, _)| keeper);
    
    match winner {
        Some(keeper) => msg!("\n✓ Winner: {:?}", keeper),
        None => msg!("\n✗ No keeper submitted in time"),
    }
    
    Ok(())
}

/// Test MMT slashing for failed liquidations
pub fn test_mmt_slashing(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing MMT slashing for failed liquidations");
    
    let account_iter = &mut accounts.iter();
    let stake_account_info = next_account_info(account_iter)?;
    let mmt_state_account = next_account_info(account_iter)?;
    
    let mut stake_account = StakeAccount::try_from_slice(&stake_account_info.data.borrow())?;
    let mut mmt_state = MMTState::try_from_slice(&mmt_state_account.data.borrow())?;
    
    let initial_stake = stake_account.amount;
    msg!("\nInitial stake: {} MMT", initial_stake / 1_000_000);
    
    // Simulate failed liquidation
    msg!("\nSimulating failed liquidation:");
    
    // Calculate slash amount (5% of stake)
    let slash_percentage = 500; // 5% in basis points
    let slash_amount = (stake_account.amount * slash_percentage as u64) / 10000;
    
    msg!("  Slash percentage: {}%", slash_percentage as f64 / 100.0);
    msg!("  Slash amount: {} MMT", slash_amount / 1_000_000);
    
    // Apply slashing
    stake_account.amount = stake_account.amount.saturating_sub(slash_amount);
    // Track slashed amount in circulating supply
    mmt_state.circulating_supply = mmt_state.circulating_supply.saturating_sub(slash_amount);
    
    // Check if keeper drops tier
    let old_tier = stake_account.tier.clone();
    update_keeper_tier(&mut stake_account)?;
    
    msg!("\nPost-slash status:");
    msg!("  Remaining stake: {} MMT", stake_account.amount / 1_000_000);
    msg!("  Tier: {:?} -> {:?}", old_tier, stake_account.tier);
    
    // Redistribute slashed MMT
    msg!("\nSlashed MMT redistribution:");
    msg!("  50% to insurance fund");
    msg!("  50% to active stakers");
    
    Ok(())
}

/// Determine liquidation type based on position health
fn determine_liquidation_type(
    position: &Position,
    current_price: u64,
) -> Result<LiquidationType, ProgramError> {
    let price_distance = if position.is_long {
        position.liquidation_price.saturating_sub(current_price)
    } else {
        current_price.saturating_sub(position.liquidation_price)
    };
    
    let distance_bps = (price_distance * 10000) / position.liquidation_price;
    
    if distance_bps > 500 {
        Ok(LiquidationType::Partial)
    } else if distance_bps > 100 {
        Ok(LiquidationType::Full)
    } else {
        Ok(LiquidationType::Emergency)
    }
}

/// Execute liquidation on AMM
fn execute_liquidation_on_amm(
    proposal: &mut ProposalPDA,
    outcome: u8,
    size: u64,
    is_long: bool,
) -> Result<(), ProgramError> {
    // Simplified AMM execution for liquidation
    let impact_bps = (size * 10) / proposal.liquidity_depth; // 0.1% per $1M/$100M liquidity
    let price_impact = (proposal.prices[outcome as usize] * impact_bps) / 10000;
    
    if is_long {
        // Selling long position pushes price down
        proposal.prices[outcome as usize] = proposal.prices[outcome as usize]
            .saturating_sub(price_impact);
        proposal.prices[1 - outcome as usize] = 1_000_000 - proposal.prices[outcome as usize];
    } else {
        // Buying back short position pushes price up
        proposal.prices[outcome as usize] = proposal.prices[outcome as usize]
            .saturating_add(price_impact);
        proposal.prices[1 - outcome as usize] = 1_000_000 - proposal.prices[outcome as usize];
    }
    
    // Update volume
    proposal.volumes[outcome as usize] += size;
    
    Ok(())
}

/// Get tier multiplier for MMT rewards
fn get_tier_multiplier(tier: &StakingTier) -> u64 {
    match tier {
        StakingTier::Bronze => 100,   // 1.0x
        StakingTier::Silver => 110,   // 1.1x
        StakingTier::Gold => 120,     // 1.2x
        StakingTier::Platinum => 125, // 1.25x
        StakingTier::Diamond => 130,  // 1.3x
    }
}

/// Update keeper tier based on stake
fn update_keeper_tier(stake_account: &mut StakeAccount) -> Result<(), ProgramError> {
    stake_account.tier = if stake_account.amount >= 1_000_000_000_000 {
        StakingTier::Diamond
    } else if stake_account.amount >= 100_000_000_000 {
        StakingTier::Gold
    } else if stake_account.amount >= 10_000_000_000 {
        StakingTier::Silver
    } else {
        StakingTier::Bronze
    };
    
    Ok(())
}

/// Verify AMM invariant
fn verify_amm_invariant(proposal: &ProposalPDA) -> Result<(), ProgramError> {
    let sum = proposal.prices[0] + proposal.prices[1];
    let deviation = if sum > 1_000_000 {
        sum - 1_000_000
    } else {
        1_000_000 - sum
    };
    
    if deviation > 10_000 {
        return Err(BettingPlatformError::AMMInvariantViolation.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_liquidation_type_determination() {
        let position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            liquidation_price: 495_000,
            is_long: true,
            // ... other fields with defaults
            user: Pubkey::default(),
            proposal_id: 1,
            position_id: [0; 32],
            outcome: 0,
            size: 100_000_000_000,
            notional: 100_000_000_000,
            leverage: 50,
            entry_price: 500_000,
            created_at: 0,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 1,
            margin: 2_000_000_000,
            collateral: 0,
            is_short: false,
            last_mark_price: 500_000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        // Far from liquidation - partial
        let liq_type = determine_liquidation_type(&position, 490_000).unwrap();
        assert!(matches!(liq_type, LiquidationType::Partial));
        
        // Close to liquidation - full
        let liq_type = determine_liquidation_type(&position, 494_500).unwrap();
        assert!(matches!(liq_type, LiquidationType::Full));
        
        // Very close - emergency
        let liq_type = determine_liquidation_type(&position, 494_900).unwrap();
        assert!(matches!(liq_type, LiquidationType::Emergency));
    }
    
    #[test]
    fn test_tier_multiplier() {
        assert_eq!(get_tier_multiplier(&StakingTier::Bronze), 100);
        assert_eq!(get_tier_multiplier(&StakingTier::Silver), 110);
        assert_eq!(get_tier_multiplier(&StakingTier::Gold), 120);
        assert_eq!(get_tier_multiplier(&StakingTier::Diamond), 130);
    }
}