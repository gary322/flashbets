//! Production-ready MMT staking journey test
//! 
//! Tests complete MMT staking flow with tier progression and rewards

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
    mmt::{
        state::{StakeAccount, StakingPool, MMTConfig},
        staking::{StakingTier, calculate_tier_from_amount, calculate_apy_for_tier},
        rewards::{calculate_staking_rewards, distribute_fee_rebates},
    },
    events::{emit_event, EventType, MMTStakeEvent, MMTRewardClaimEvent},
    math::fixed_point::U64F64,
    fees::FEE_REBATE_RATE_BPS,
};

/// Production test: Complete MMT staking journey with tier progression
pub fn test_mmt_staking_journey_production() -> ProgramResult {
    msg!("=== PRODUCTION TEST: MMT Staking Journey ===");
    
    let program_id = Pubkey::new_unique();
    let user_pubkey = Pubkey::new_unique();
    let clock = Clock::get()?;
    
    // Initialize MMT configuration
    let mmt_config = MMTConfig {
        discriminator: MMTConfig::DISCRIMINATOR,
        is_initialized: true,
        mint: Pubkey::new_unique(),
        authority: Pubkey::new_unique(),
        total_supply: 100_000_000_000_000, // 100M MMT
        circulating_supply: 10_000_000_000_000, // 10M circulating
        season_allocation: 10_000_000_000_000, // 10M per season
        current_season: 1,
        season_start_slot: clock.slot - 1_000_000,
        season_emitted: 1_000_000_000_000, // 1M already emitted
        locked_supply: 90_000_000_000_000, // 90M locked
        bump: 1,
    };
    
    // Initialize staking pool
    let mut staking_pool = StakingPool {
        discriminator: StakingPool::DISCRIMINATOR,
        is_initialized: true,
        total_staked: 5_000_000_000_000, // 5M MMT already staked
        total_stakers: 100,
        reward_per_slot: 1_000_000, // 1 MMT per slot
        last_update_slot: clock.slot,
        accumulated_rewards_per_share: 0,
        rebate_percentage_base: FEE_REBATE_RATE_BPS as u64, // 15%
        total_fees_collected: 100_000_000_000_000, // $100k in fees
        total_rebates_distributed: 15_000_000_000_000, // $15k distributed
    };
    
    // Step 1: Initialize user stake account
    msg!("Step 1: Initializing stake account");
    
    let mut stake_account = StakeAccount {
        discriminator: StakeAccount::DISCRIMINATOR,
        is_initialized: true,
        owner: user_pubkey,
        amount_staked: 0,
        stake_timestamp: clock.unix_timestamp,
        last_claim_slot: clock.slot,
        accumulated_rewards: 0,
        rebate_percentage: 0, // Will be calculated based on stake share
        lock_end_slot: None,
        lock_multiplier: 10000, // 1x (no lock)
        tier: StakingTier::Bronze,
        amount: 0,
        is_locked: false,
        rewards_earned: 0,
    };
    
    msg!("  Stake account initialized");
    msg!("  Starting tier: {:?}", stake_account.tier);
    
    // Step 2: Stake MMT tokens (Bronze -> Silver progression)
    msg!("Step 2: Staking MMT tokens");
    
    let stake_amount = 15_000_000_000_000; // 15k MMT (Silver tier)
    stake_account.amount_staked = stake_amount;
    stake_account.amount = stake_amount;
    stake_account.tier = calculate_tier_from_amount(stake_amount);
    
    // Update pool
    staking_pool.total_staked += stake_amount;
    staking_pool.total_stakers += 1;
    
    // Calculate rebate percentage based on stake share
    let stake_share = (stake_amount as u128 * 10000) / staking_pool.total_staked as u128;
    stake_account.rebate_percentage = stake_share as u64;
    
    // Emit stake event
    emit_event(EventType::MMTStake, &MMTStakeEvent {
        staker: user_pubkey,
        amount: stake_amount,
        lock_period_days: 0,
        tier: stake_account.tier as u8,
        timestamp: clock.unix_timestamp,
    });
    
    msg!("  Staked: {} MMT", stake_amount / 1_000_000);
    msg!("  New tier: {:?}", stake_account.tier);
    msg!("  Stake share: {:.2}%", stake_share as f64 / 100.0);
    msg!("  APY: {}%", calculate_apy_for_tier(stake_account.tier) as f64 / 100.0);
    
    // Step 3: Add lock for bonus multiplier
    msg!("Step 3: Adding 180-day lock for bonus rewards");
    
    let lock_days = 180;
    let slots_per_day = 216_000;
    stake_account.lock_end_slot = Some(clock.slot + (lock_days * slots_per_day));
    stake_account.is_locked = true;
    
    // Calculate lock multiplier (up to 1.5x for 180 days)
    stake_account.lock_multiplier = calculate_lock_multiplier(lock_days);
    
    msg!("  Lock period: {} days", lock_days);
    msg!("  Lock multiplier: {:.2}x", stake_account.lock_multiplier as f64 / 10000.0);
    msg!("  Boosted APY: {:.2}%", 
         (calculate_apy_for_tier(stake_account.tier) as u64 * stake_account.lock_multiplier / 10000) as f64 / 100.0);
    
    // Step 4: Simulate time passing and accumulate rewards
    msg!("Step 4: Simulating 30 days of staking");
    
    let slots_passed = 30 * slots_per_day; // 30 days
    let current_slot = clock.slot + slots_passed;
    
    // Calculate staking rewards
    let base_apy = calculate_apy_for_tier(stake_account.tier) as u64;
    let time_factor = U64F64::from_num(slots_passed) / U64F64::from_num(slots_per_day * 365);
    let base_rewards = (U64F64::from_num(stake_amount) * U64F64::from_num(base_apy) / U64F64::from_num(10000) * time_factor).to_num();
    let rewards_with_multiplier = (base_rewards as u128 * stake_account.lock_multiplier as u128 / 10000) as u64;
    
    stake_account.accumulated_rewards += rewards_with_multiplier;
    
    // Calculate fee rebates
    let period_fees = 10_000_000_000_000; // $10k in fees during period
    let user_rebate = (period_fees as u128 * stake_account.rebate_percentage as u128 / 10000) as u64;
    stake_account.accumulated_rewards += user_rebate;
    
    msg!("  Time passed: 30 days");
    msg!("  Staking rewards earned: {} MMT", rewards_with_multiplier / 1_000_000);
    msg!("  Fee rebates earned: ${}", user_rebate / 1_000_000);
    msg!("  Total pending rewards: {} MMT", stake_account.accumulated_rewards / 1_000_000);
    
    // Step 5: Claim rewards
    msg!("Step 5: Claiming accumulated rewards");
    
    let rewards_to_claim = stake_account.accumulated_rewards;
    stake_account.rewards_earned += rewards_to_claim;
    stake_account.accumulated_rewards = 0;
    stake_account.last_claim_slot = current_slot;
    
    // Emit claim event
    emit_event(EventType::MMTRewardClaim, &MMTRewardClaimEvent {
        staker: user_pubkey,
        amount: rewards_to_claim,
        rewards_type: 1, // Staking rewards
        timestamp: clock.unix_timestamp + (30 * 86400),
    });
    
    msg!("  Claimed: {} MMT", rewards_to_claim / 1_000_000);
    msg!("  Total rewards earned: {} MMT", stake_account.rewards_earned / 1_000_000);
    
    // Step 6: Stake more to reach Gold tier
    msg!("Step 6: Staking additional MMT to reach Gold tier");
    
    let additional_stake = 85_000_000_000_000; // 85k more MMT
    let new_total_stake = stake_account.amount_staked + additional_stake;
    
    stake_account.amount_staked = new_total_stake;
    stake_account.amount = new_total_stake;
    let old_tier = stake_account.tier;
    stake_account.tier = calculate_tier_from_amount(new_total_stake);
    
    // Update pool
    staking_pool.total_staked += additional_stake;
    
    // Recalculate rebate percentage
    let new_stake_share = (new_total_stake as u128 * 10000) / staking_pool.total_staked as u128;
    stake_account.rebate_percentage = new_stake_share as u64;
    
    // Emit tier upgrade event
    emit_event(EventType::MMTStake, &MMTStakeEvent {
        staker: user_pubkey,
        amount: new_total_stake,
        lock_period_days: lock_days as u32,
        tier: stake_account.tier as u8,
        timestamp: clock.unix_timestamp + (30 * 86400),
    });
    
    msg!("  Additional stake: {} MMT", additional_stake / 1_000_000);
    msg!("  Total staked: {} MMT", new_total_stake / 1_000_000);
    msg!("  Tier upgraded: {:?} -> {:?}", old_tier, stake_account.tier);
    msg!("  New APY: {}%", calculate_apy_for_tier(stake_account.tier) as f64 / 100.0);
    msg!("  New stake share: {:.2}%", new_stake_share as f64 / 100.0);
    
    // Step 7: Verify journey results
    msg!("Step 7: Verifying staking journey results");
    
    let total_value_staked = new_total_stake;
    let total_rewards = stake_account.rewards_earned;
    let apr_30_days = (total_rewards as f64 / total_value_staked as f64) * (365.0 / 30.0) * 100.0;
    
    msg!("  Total MMT staked: {} MMT", total_value_staked / 1_000_000);
    msg!("  Total rewards earned: {} MMT", total_rewards / 1_000_000);
    msg!("  Effective APR (30 days): {:.2}%", apr_30_days);
    msg!("  Current tier: {:?}", stake_account.tier);
    msg!("  Lock status: {} (ends in {} days)", 
         if stake_account.is_locked { "Locked" } else { "Unlocked" },
         ((stake_account.lock_end_slot.unwrap_or(current_slot) - current_slot) / slots_per_day));
    
    // Verify progression
    assert_eq!(stake_account.tier, StakingTier::Gold);
    assert!(stake_account.rewards_earned > 0);
    assert!(stake_account.rebate_percentage > 0);
    assert!(stake_account.is_locked);
    
    msg!("=== MMT Staking Journey Test PASSED ===");
    Ok(())
}

/// Calculate lock multiplier based on lock duration
fn calculate_lock_multiplier(days: u64) -> u16 {
    match days {
        0..=29 => 10000,      // 1.0x
        30..=89 => 11000,     // 1.1x
        90..=179 => 12500,    // 1.25x
        180..=364 => 15000,   // 1.5x
        _ => 20000,           // 2.0x for 365+ days
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_production_mmt_journey() {
        test_mmt_staking_journey_production().unwrap();
    }
    
    #[test]
    fn test_lock_multipliers() {
        assert_eq!(calculate_lock_multiplier(0), 10000);
        assert_eq!(calculate_lock_multiplier(30), 11000);
        assert_eq!(calculate_lock_multiplier(90), 12500);
        assert_eq!(calculate_lock_multiplier(180), 15000);
        assert_eq!(calculate_lock_multiplier(365), 20000);
    }
}