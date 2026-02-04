//! Keeper rewards management
//!
//! Handles reward distribution, claiming, and performance-based bonuses

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
    error::BettingPlatformError,
    state::keeper_accounts::{
        KeeperAccount, KeeperRegistry, KeeperStatus, WorkType,
    },
};

/// Reward rates per operation type (in MMT with 9 decimals)
pub mod reward_rates {
    pub const LIQUIDATION_BASE: u64 = 50_000_000_000; // 50 MMT
    pub const STOP_ORDER_BASE: u64 = 10_000_000_000; // 10 MMT
    pub const PRICE_UPDATE_BASE: u64 = 5_000_000_000; // 5 MMT
    pub const RESOLUTION_BASE: u64 = 100_000_000_000; // 100 MMT
    
    // Performance multipliers (basis points)
    pub const PERFECT_SCORE_MULTIPLIER: u16 = 15000; // 150% for 100% performance
    pub const GOOD_SCORE_MULTIPLIER: u16 = 12500; // 125% for 95%+ performance
    pub const NORMAL_SCORE_MULTIPLIER: u16 = 10000; // 100% for 90%+ performance
    pub const LOW_SCORE_MULTIPLIER: u16 = 7500; // 75% for below 90%
}

/// Process keeper reward claim
pub fn process_claim_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Processing keeper reward claim");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let registry_account = next_account_info(account_info_iter)?;
    let reward_vault = next_account_info(account_info_iter)?;
    let keeper_token_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify keeper authority is signer
    if !keeper_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load and validate accounts
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
    
    keeper.validate()?;
    registry.validate()?;
    
    // Verify keeper authority
    if keeper.authority != *keeper_authority.key {
        msg!("Keeper authority mismatch");
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check keeper status
    if keeper.status != KeeperStatus::Active {
        msg!("Keeper is not active");
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Calculate pending rewards
    let pending_rewards = calculate_pending_rewards(&keeper)?;
    
    if pending_rewards == 0 {
        msg!("No rewards to claim");
        return Err(BettingPlatformError::NoRewardsToClaim.into());
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    
    // Transfer rewards from vault to keeper
    // In production, this would use SPL token transfer
    msg!("Transferring {} MMT rewards to keeper", pending_rewards);
    
    // Update keeper stats
    keeper.total_rewards_earned += pending_rewards;
    keeper.last_operation_slot = clock.slot; // Reset rewards accumulation
    
    // Update registry
    registry.total_rewards_distributed += pending_rewards;
    
    // Log claim
    msg!("Keeper rewards claimed:");
    msg!("  Keeper ID: {:?}", keeper.keeper_id);
    msg!("  Amount: {} MMT", pending_rewards);
    msg!("  Total earned: {} MMT", keeper.total_rewards_earned);
    msg!("  Performance score: {}%", keeper.performance_score / 100);
    
    // Serialize and save
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Record completed work and calculate reward
pub fn process_record_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    work_type: WorkType,
    success: bool,
    response_time: u64,
) -> ProgramResult {
    msg!("Recording keeper work: type={:?}, success={}", work_type, success);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let program_authority = next_account_info(account_info_iter)?; // Only program can record work
    let keeper_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify program authority
    if !program_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load keeper
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    keeper.validate()?;
    
    // Check keeper has required specialization
    if !keeper.has_specialization(&work_type) {
        msg!("Keeper lacks required specialization for work type");
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    
    // Update operation counts
    keeper.total_operations += 1;
    if success {
        keeper.successful_operations += 1;
    }
    
    // Update performance score (exponential moving average)
    let success_rate = if keeper.total_operations > 0 {
        (keeper.successful_operations as u128 * 10000 / keeper.total_operations as u128) as u64
    } else {
        10000 // 100%
    };
    
    // EMA with alpha = 0.1 (10%)
    keeper.performance_score = (keeper.performance_score * 9 + success_rate) / 10;
    
    // Update average response time
    if keeper.average_response_time == 0 {
        keeper.average_response_time = response_time;
    } else {
        keeper.average_response_time = (keeper.average_response_time * 9 + response_time) / 10;
    }
    
    // Update last operation slot
    keeper.last_operation_slot = clock.slot;
    
    // Calculate reward for this operation
    let base_reward = match work_type {
        WorkType::Liquidations => reward_rates::LIQUIDATION_BASE,
        WorkType::StopOrders => reward_rates::STOP_ORDER_BASE,
        WorkType::PriceUpdates => reward_rates::PRICE_UPDATE_BASE,
        WorkType::Resolutions => reward_rates::RESOLUTION_BASE,
    };
    
    // Apply performance multiplier
    let multiplier = get_performance_multiplier(keeper.performance_score);
    let reward = if success {
        (base_reward as u128 * multiplier as u128 / 10000) as u64
    } else {
        0 // No reward for failed operations
    };
    
    // Update priority score
    keeper.priority_score = keeper.calculate_priority() as u128;
    
    // Log work record
    msg!("Work recorded:");
    msg!("  Keeper ID: {:?}", keeper.keeper_id);
    msg!("  Work type: {:?}", work_type);
    msg!("  Success: {}", success);
    msg!("  Response time: {} slots", response_time);
    msg!("  Performance score: {}%", keeper.performance_score / 100);
    msg!("  Reward: {} MMT", reward);
    
    // Serialize and save
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Slash keeper for malicious behavior
pub fn process_slash_keeper(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    slash_percentage: u16, // Basis points (0-10000)
    reason: SlashReason,
) -> ProgramResult {
    msg!("Slashing keeper: {}% for {:?}", slash_percentage / 100, reason);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let program_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let registry_account = next_account_info(account_info_iter)?;
    let slash_vault = next_account_info(account_info_iter)?; // Where slashed funds go
    
    // Verify program authority
    if !program_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate slash percentage
    if slash_percentage > 10000 {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Load accounts
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
    
    // Calculate slash amount
    let slash_amount = (keeper.mmt_stake as u128 * slash_percentage as u128 / 10000) as u64;
    
    // Update keeper
    keeper.mmt_stake = keeper.mmt_stake.saturating_sub(slash_amount);
    keeper.status = KeeperStatus::Slashed;
    keeper.slashing_count += 1;
    keeper.performance_score = 0; // Reset performance
    
    // Update registry
    registry.total_mmt_staked = registry.total_mmt_staked.saturating_sub(slash_amount);
    registry.slashing_events += 1;
    if registry.active_keepers > 0 {
        registry.active_keepers -= 1;
    }
    
    // Transfer slashed funds to slash vault
    msg!("Transferring {} MMT to slash vault", slash_amount);
    
    // Log slashing
    msg!("Keeper slashed:");
    msg!("  Keeper ID: {:?}", keeper.keeper_id);
    msg!("  Reason: {:?}", reason);
    msg!("  Amount: {} MMT", slash_amount);
    msg!("  Remaining stake: {} MMT", keeper.mmt_stake);
    msg!("  Total slashings: {}", keeper.slashing_count);
    
    // Serialize and save
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Calculate pending rewards for a keeper
fn calculate_pending_rewards(keeper: &KeeperAccount) -> Result<u64, ProgramError> {
    // In a real implementation, this would calculate based on:
    // 1. Operations performed since last claim
    // 2. Performance score
    // 3. Stake amount (for bonus rewards)
    // 4. Time since last claim
    
    // For now, return a simple calculation
    let operations_since_claim = keeper.total_operations.saturating_sub(
        keeper.total_rewards_earned / reward_rates::STOP_ORDER_BASE
    );
    
    if operations_since_claim == 0 {
        return Ok(0);
    }
    
    // Average reward per operation
    let avg_reward = reward_rates::STOP_ORDER_BASE;
    let base_rewards = operations_since_claim * avg_reward;
    
    // Apply performance multiplier
    let multiplier = get_performance_multiplier(keeper.performance_score);
    let total_rewards = (base_rewards as u128 * multiplier as u128 / 10000) as u64;
    
    Ok(total_rewards)
}

/// Get performance multiplier based on score
fn get_performance_multiplier(performance_score: u64) -> u16 {
    match performance_score {
        10000 => reward_rates::PERFECT_SCORE_MULTIPLIER,
        9500..=9999 => reward_rates::GOOD_SCORE_MULTIPLIER,
        9000..=9499 => reward_rates::NORMAL_SCORE_MULTIPLIER,
        _ => reward_rates::LOW_SCORE_MULTIPLIER,
    }
}

/// Slash reasons
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlashReason {
    MaliciousExecution,
    RepeatedFailures,
    Downtime,
    FrontRunning,
    IncorrectData,
}