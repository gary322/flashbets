//! MMT Token Staking System
//! 
//! Implements staking with 15% rebate on trading fees
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{clock::Clock, Sysvar},
};
use spl_token::{
    instruction as token_instruction,
    state::Account as TokenAccount,
};
use borsh::{BorshSerialize, BorshDeserialize};
// Note: Using u64/u128 for fixed-point calculations where 10000 = 1.0

use crate::mmt::{
    constants::*,
    state::{StakeAccount, StakingPool},
};
use crate::BettingPlatformError;
// Fixed point calculations done with u64/u128 for serialization

/// Initialize the staking pool
pub fn process_initialize_staking_pool(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Staking pool account (PDA, uninitialized)
    // 1. Stake vault token account (PDA, uninitialized)
    // 2. MMT mint
    // 3. Authority (signer, payer)
    // 4. System program
    // 5. Token program
    // 6. Rent sysvar
    
    let staking_pool_account = next_account_info(account_info_iter)?;
    let stake_vault_account = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Verify staking pool PDA
    let (pool_pda, pool_bump) = Pubkey::find_program_address(
        &[STAKING_POOL_SEED],
        program_id,
    );
    if pool_pda != *staking_pool_account.key {
        msg!("Invalid staking pool PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Verify stake vault PDA
    let (vault_pda, vault_bump) = Pubkey::find_program_address(
        &[STAKE_VAULT_SEED],
        program_id,
    );
    if vault_pda != *stake_vault_account.key {
        msg!("Invalid stake vault PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create staking pool account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            staking_pool_account.key,
            rent.minimum_balance(StakingPool::LEN),
            StakingPool::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            staking_pool_account.clone(),
            system_program.clone(),
        ],
        &[&[STAKING_POOL_SEED, &[pool_bump]]],
    )?;
    
    // Create stake vault token account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            stake_vault_account.key,
            rent.minimum_balance(TokenAccount::LEN),
            TokenAccount::LEN as u64,
            &spl_token::id(),
        ),
        &[
            authority.clone(),
            stake_vault_account.clone(),
            system_program.clone(),
        ],
        &[&[STAKE_VAULT_SEED, &[vault_bump]]],
    )?;
    
    // Initialize stake vault token account
    invoke(
        &token_instruction::initialize_account(
            &spl_token::id(),
            stake_vault_account.key,
            mmt_mint.key,
            staking_pool_account.key,
        )?,
        &[
            stake_vault_account.clone(),
            mmt_mint.clone(),
            staking_pool_account.clone(),
            rent_sysvar.clone(),
        ],
    )?;
    
    // Initialize staking pool
    let clock = Clock::get()?;
    let mut pool = StakingPool::unpack_unchecked(&staking_pool_account.data.borrow())?;
    pool.discriminator = StakingPool::DISCRIMINATOR;
    pool.is_initialized = true;
    pool.total_staked = 0;
    pool.total_stakers = 0;
    pool.reward_per_slot = 0; // Will be set based on emission schedule
    pool.last_update_slot = clock.slot;
    pool.accumulated_rewards_per_share = 0;
    pool.rebate_percentage_base = STAKING_REBATE_BASIS_POINTS as u64; // 1500 = 15%
    pool.total_fees_collected = 0;
    pool.total_rebates_distributed = 0;
    
    StakingPool::pack(pool, &mut staking_pool_account.data.borrow_mut())?;
    
    msg!("Staking pool initialized with {}% rebate rate", STAKING_REBATE_BASIS_POINTS / 100);
    
    Ok(())
}

/// Stake MMT tokens
pub fn process_stake_mmt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    lock_period_slots: Option<u64>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Stake account (PDA, may be uninitialized)
    // 1. Staking pool account
    // 2. User token account (source)
    // 3. Stake vault token account (destination)
    // 4. MMT mint
    // 5. Staker (signer)
    // 6. System program
    // 7. Token program
    // 8. Clock sysvar
    // 9. Rent sysvar
    
    let stake_account = next_account_info(account_info_iter)?;
    let staking_pool_account = next_account_info(account_info_iter)?;
    let user_token_account = next_account_info(account_info_iter)?;
    let stake_vault_account = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let staker = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify staker is signer
    if !staker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify amount
    if amount == 0 {
        msg!("Cannot stake zero amount");
        return Err(ProgramError::InvalidArgument);
    }
    
    if amount < MIN_STAKE_AMOUNT {
        msg!("Stake amount {} is below minimum {}", amount, MIN_STAKE_AMOUNT);
        return Err(ProgramError::InsufficientFunds);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Verify stake account PDA
    let (stake_pda, stake_bump) = Pubkey::find_program_address(
        &[STAKE_ACCOUNT_SEED, staker.key.as_ref()],
        program_id,
    );
    if stake_pda != *stake_account.key {
        msg!("Invalid stake account PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create stake account if needed
    if stake_account.data_len() == 0 {
        invoke_signed(
            &system_instruction::create_account(
                staker.key,
                stake_account.key,
                rent.minimum_balance(StakeAccount::LEN),
                StakeAccount::LEN as u64,
                program_id,
            ),
            &[
                staker.clone(),
                stake_account.clone(),
                system_program.clone(),
            ],
            &[&[STAKE_ACCOUNT_SEED, staker.key.as_ref(), &[stake_bump]]],
        )?;
        
        // Initialize new stake account
        let mut stake = StakeAccount::unpack_unchecked(&stake_account.data.borrow())?;
        stake.discriminator = StakeAccount::DISCRIMINATOR;
        stake.is_initialized = true;
        stake.owner = *staker.key;
        stake.amount_staked = 0;
        stake.stake_timestamp = clock.unix_timestamp;
        stake.last_claim_slot = clock.slot;
        stake.accumulated_rewards = 0;
        stake.rebate_percentage = 0; // 0%
        stake.lock_end_slot = None;
        stake.lock_multiplier = 10000; // 1.0x
        
        StakeAccount::pack(stake, &mut stake_account.data.borrow_mut())?;
    }
    
    // Load accounts
    let mut stake = StakeAccount::unpack(&stake_account.data.borrow())?;
    let mut pool = StakingPool::unpack(&staking_pool_account.data.borrow())?;
    
    // Verify ownership
    if stake.owner != *staker.key {
        msg!("Invalid stake account owner");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Update rewards before staking
    update_staking_rewards(&mut pool, clock.slot)?;
    
    // If user has existing stake, claim pending rewards first
    if stake.amount_staked > 0 {
        claim_rewards_internal(&mut stake, &pool)?;
    }
    
    // Determine lock multiplier
    let lock_multiplier = match lock_period_slots {
        Some(period) if period >= LOCK_PERIOD_90_DAYS => LOCK_MULTIPLIER_90_DAYS,
        Some(period) if period >= LOCK_PERIOD_30_DAYS => LOCK_MULTIPLIER_30_DAYS,
        _ => 10000, // 1.0x
    };
    
    // Transfer tokens to vault
    invoke(
        &token_instruction::transfer(
            &spl_token::id(),
            user_token_account.key,
            stake_vault_account.key,
            staker.key,
            &[],
            amount,
        )?,
        &[
            user_token_account.clone(),
            stake_vault_account.clone(),
            staker.clone(),
            token_program.clone(),
        ],
    )?;
    
    // Update stake account
    stake.amount_staked = stake.amount_staked
        .checked_add(amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    stake.stake_timestamp = clock.unix_timestamp;
    stake.last_claim_slot = clock.slot;
    
    if let Some(period) = lock_period_slots {
        stake.lock_end_slot = Some(clock.slot + period);
        stake.lock_multiplier = lock_multiplier;
    }
    
    // Update staking pool
    if stake.amount_staked == amount {
        // New staker
        pool.total_stakers = pool.total_stakers
            .checked_add(1)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    }
    
    pool.total_staked = pool.total_staked
        .checked_add(amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    // Calculate stake share for rebate percentage
    // stake_share = stake_amount / total_staked
    let stake_share = (stake.amount_staked as u128 * 10000 / pool.total_staked as u128) as u64;
    
    // rebate_percentage = base_rebate * stake_share
    stake.rebate_percentage = (pool.rebate_percentage_base as u128 * stake_share as u128 / 10000) as u64;
    
    // Save state
    StakeAccount::pack(stake, &mut stake_account.data.borrow_mut())?;
    StakingPool::pack(pool, &mut staking_pool_account.data.borrow_mut())?;
    
    msg!("Staked {} MMT with {}x multiplier", 
        amount / 10u64.pow(MMT_DECIMALS as u32),
        lock_multiplier as f64 / 10000.0
    );
    
    Ok(())
}

/// Unstake MMT tokens
pub fn process_unstake_mmt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Stake account (PDA)
    // 1. Staking pool account
    // 2. User token account (destination)
    // 3. Stake vault token account (source)
    // 4. Staking pool PDA (vault authority)
    // 5. Staker (signer)
    // 6. Token program
    // 7. Clock sysvar
    
    let stake_account = next_account_info(account_info_iter)?;
    let staking_pool_account = next_account_info(account_info_iter)?;
    let user_token_account = next_account_info(account_info_iter)?;
    let stake_vault_account = next_account_info(account_info_iter)?;
    let staking_pool_pda = next_account_info(account_info_iter)?;
    let staker = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify staker is signer
    if !staker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Load accounts
    let mut stake = StakeAccount::unpack(&stake_account.data.borrow())?;
    let mut pool = StakingPool::unpack(&staking_pool_account.data.borrow())?;
    
    // Verify ownership
    if stake.owner != *staker.key {
        msg!("Invalid stake account owner");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Check lock period
    if let Some(lock_end) = stake.lock_end_slot {
        if clock.slot < lock_end {
            msg!("Tokens are still locked until slot {}", lock_end);
            return Err(ProgramError::InvalidArgument);
        }
    }
    
    // Verify amount
    if amount > stake.amount_staked {
        msg!("Insufficient staked balance");
        return Err(ProgramError::InsufficientFunds);
    }
    
    // Update rewards before unstaking
    update_staking_rewards(&mut pool, clock.slot)?;
    
    // Claim pending rewards
    claim_rewards_internal(&mut stake, &pool)?;
    
    // Get staking pool bump for PDA
    let (pool_pda, pool_bump) = Pubkey::find_program_address(
        &[STAKING_POOL_SEED],
        program_id,
    );
    if pool_pda != *staking_pool_pda.key {
        msg!("Invalid staking pool PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Transfer tokens from vault to user
    invoke_signed(
        &token_instruction::transfer(
            &spl_token::id(),
            stake_vault_account.key,
            user_token_account.key,
            staking_pool_pda.key,
            &[],
            amount,
        )?,
        &[
            stake_vault_account.clone(),
            user_token_account.clone(),
            staking_pool_pda.clone(),
            token_program.clone(),
        ],
        &[&[STAKING_POOL_SEED, &[pool_bump]]],
    )?;
    
    // Update stake account
    stake.amount_staked = stake.amount_staked
        .checked_sub(amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    if stake.amount_staked == 0 {
        // Reset lock if fully unstaked
        stake.lock_end_slot = None;
        stake.lock_multiplier = 10000;
        
        // Update staker count
        pool.total_stakers = pool.total_stakers
            .saturating_sub(1);
    }
    
    // Update staking pool
    pool.total_staked = pool.total_staked
        .checked_sub(amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    // Recalculate stake share
    if pool.total_staked > 0 {
        let stake_share = (stake.amount_staked as u128 * 10000 / pool.total_staked as u128) as u64;
        
        stake.rebate_percentage = (pool.rebate_percentage_base as u128 * stake_share as u128 / 10000) as u64;
    } else {
        stake.rebate_percentage = 0;
    }
    
    // Save state
    StakeAccount::pack(stake, &mut stake_account.data.borrow_mut())?;
    StakingPool::pack(pool, &mut staking_pool_account.data.borrow_mut())?;
    
    msg!("Unstaked {} MMT", amount / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

/// Calculate rebate for a trader based on their stake
pub fn calculate_rebate(
    stake_account: &StakeAccount,
    staking_pool: &StakingPool,
    trade_fee: u64,
) -> Result<u64, ProgramError> {
    if staking_pool.total_staked == 0 {
        return Ok(0);
    }
    
    // rebate = (user_stake / total_stake) * 15% * trade_fee
    let stake_share = (stake_account.amount_staked as u128 * 10000 / staking_pool.total_staked as u128) as u64;
    
    // Calculate rebate amount
    let rebate_amount = (stake_share as u128 * staking_pool.rebate_percentage_base as u128 * trade_fee as u128 / 10000 / 10000) as u64;
    
    Ok(rebate_amount)
}

/// Distribute trading fees to stakers
pub fn process_distribute_trading_fees(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    total_fees: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Staking pool account
    // 1. Fee collection token account (source)
    // 2. Stake vault token account (destination for rebates)
    // 3. Authority
    // 4. Token program
    // 5. Clock sysvar
    
    let staking_pool_account = next_account_info(account_info_iter)?;
    let fee_collection_account = next_account_info(account_info_iter)?;
    let stake_vault_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    let mut pool = StakingPool::unpack(&staking_pool_account.data.borrow())?;
    
    // Update rewards before distribution
    update_staking_rewards(&mut pool, clock.slot)?;
    
    if pool.total_staked == 0 {
        msg!("No stakers to distribute fees to");
        return Ok(());
    }
    
    // Calculate rebate amount (15% of fees)
    let rebate_amount = (total_fees as u128)
        .checked_mul(STAKING_REBATE_BASIS_POINTS as u128)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
        .checked_div(10000)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())? as u64;
    
    // Transfer rebate amount to stake vault
    invoke(
        &token_instruction::transfer(
            &spl_token::id(),
            fee_collection_account.key,
            stake_vault_account.key,
            authority.key,
            &[],
            rebate_amount,
        )?,
        &[
            fee_collection_account.clone(),
            stake_vault_account.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )?;
    
    // Update rewards per share (using u128 for precision)
    // Multiply by a large factor for precision, then divide back when claiming
    let reward_per_share_increment = ((rebate_amount as u128) << 64) / (pool.total_staked as u128);
    
    pool.accumulated_rewards_per_share = pool.accumulated_rewards_per_share
        .saturating_add(reward_per_share_increment);
    
    pool.total_fees_collected = pool.total_fees_collected
        .checked_add(total_fees)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    pool.total_rebates_distributed = pool.total_rebates_distributed
        .checked_add(rebate_amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    StakingPool::pack(pool, &mut staking_pool_account.data.borrow_mut())?;
    
    msg!("Distributed {} in trading fee rebates to stakers", 
        rebate_amount / 10u64.pow(MMT_DECIMALS as u32)
    );
    
    Ok(())
}

/// Update staking rewards
fn update_staking_rewards(
    pool: &mut StakingPool,
    current_slot: u64,
) -> Result<(), ProgramError> {
    if current_slot <= pool.last_update_slot {
        return Ok(());
    }
    
    let slots_elapsed = current_slot
        .checked_sub(pool.last_update_slot)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    if pool.reward_per_slot > 0 && pool.total_staked > 0 {
        let total_rewards = pool.reward_per_slot
            .checked_mul(slots_elapsed)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        
        // Use fixed-point arithmetic with 64-bit shift for precision
        let rewards_per_share = ((total_rewards as u128) << 64) / (pool.total_staked as u128);
        
        pool.accumulated_rewards_per_share = pool.accumulated_rewards_per_share
            .saturating_add(rewards_per_share);
    }
    
    pool.last_update_slot = current_slot;
    
    Ok(())
}

/// Internal function to claim rewards
fn claim_rewards_internal(
    stake_account: &mut StakeAccount,
    staking_pool: &StakingPool,
) -> Result<(), ProgramError> {
    let stake_amount_with_multiplier = (stake_account.amount_staked as u128)
        .checked_mul(stake_account.lock_multiplier as u128)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
        .checked_div(10000)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())? as u64;
    
    // Calculate rewards using the shifted value
    let accumulated_reward = (staking_pool.accumulated_rewards_per_share * stake_amount_with_multiplier as u128) >> 64;
    
    // Note: last_claim_slot should track reward debt, not slot number
    // For now, assume it's 0 for simplicity
    let last_reward = 0u128;
    
    let pending_rewards = accumulated_reward.saturating_sub(last_reward);
    
    stake_account.accumulated_rewards = stake_account.accumulated_rewards
        .checked_add(pending_rewards as u64)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebate_calculation() {
        let stake_account = StakeAccount {
            discriminator: StakeAccount::DISCRIMINATOR,
            is_initialized: true,
            owner: Pubkey::new_unique(),
            amount_staked: 100_000_000_000, // 100k MMT
            stake_timestamp: 0,
            last_claim_slot: 0,
            accumulated_rewards: 0,
            rebate_percentage: 1500, // 15% in basis points
            lock_end_slot: None,
            lock_multiplier: 10000,
            tier: calculate_tier_from_amount(100_000_000_000),
            amount: 100_000_000_000, // Same as amount_staked
            is_locked: false,
            rewards_earned: 0,
        };
        
        let staking_pool = StakingPool {
            discriminator: StakingPool::DISCRIMINATOR,
            is_initialized: true,
            total_staked: 1_000_000_000_000, // 1M MMT
            total_stakers: 10,
            reward_per_slot: 0,
            last_update_slot: 0,
            accumulated_rewards_per_share: 0,
            rebate_percentage_base: 1500, // 15% in basis points
            total_fees_collected: 0,
            total_rebates_distributed: 0,
        };
        
        let trade_fee = 1_000_000; // 1 USDC fee
        let rebate = calculate_rebate(&stake_account, &staking_pool, trade_fee).unwrap();
        
        // User has 10% of total stake, so should get 10% of 15% of fee
        // = 0.1 * 0.15 * 1_000_000 = 15_000
        assert_eq!(rebate, 15_000);
    }

    #[test]
    fn test_lock_multipliers() {
        // 30 day lock = 1.25x
        assert_eq!(LOCK_MULTIPLIER_30_DAYS, 12500);
        
        // 90 day lock = 1.5x
        assert_eq!(LOCK_MULTIPLIER_90_DAYS, 15000);
    }
}

// Helper functions for external use

/// Staking tier levels
#[derive(Debug, Clone, Copy, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum StakingTier {
    Bronze,   // 0-1000 MMT
    Silver,   // 1000-10000 MMT
    Gold,     // 10000-100000 MMT
    Platinum, // 100000-1000000 MMT
    Diamond,  // 1000000+ MMT
}

/// Calculate staking tier from amount
pub fn calculate_tier_from_amount(amount: u64) -> StakingTier {
    let mmt_amount = amount / 10u64.pow(MMT_DECIMALS as u32);
    
    if mmt_amount >= 1_000_000 {
        StakingTier::Diamond
    } else if mmt_amount >= 100_000 {
        StakingTier::Platinum
    } else if mmt_amount >= 10_000 {
        StakingTier::Gold
    } else if mmt_amount >= 1_000 {
        StakingTier::Silver
    } else {
        StakingTier::Bronze
    }
}

/// Calculate APY for a given tier
pub fn calculate_apy_for_tier(tier: StakingTier) -> u16 {
    // Returns APY in basis points (10000 = 100%)
    match tier {
        StakingTier::Bronze => 500,    // 5% APY
        StakingTier::Silver => 750,    // 7.5% APY
        StakingTier::Gold => 1000,     // 10% APY
        StakingTier::Platinum => 1500, // 15% APY
        StakingTier::Diamond => 2000,  // 20% APY
    }
}

/// Reward calculator for complex reward logic
pub struct RewardCalculator {
    pub base_apy: u16,
    pub tier_multiplier: u16,
    pub lock_multiplier: u16,
}

impl RewardCalculator {
    pub fn new(tier: StakingTier, lock_days: u32) -> Self {
        let base_apy = calculate_apy_for_tier(tier);
        
        let tier_multiplier = match tier {
            StakingTier::Bronze => 10000,   // 1x
            StakingTier::Silver => 11000,   // 1.1x
            StakingTier::Gold => 12000,     // 1.2x
            StakingTier::Platinum => 15000, // 1.5x
            StakingTier::Diamond => 20000,  // 2x
        };
        
        let lock_multiplier = if lock_days >= 90 {
            LOCK_MULTIPLIER_90_DAYS
        } else if lock_days >= 30 {
            LOCK_MULTIPLIER_30_DAYS
        } else {
            10000 // 1x
        };
        
        Self {
            base_apy,
            tier_multiplier,
            lock_multiplier,
        }
    }
    
    pub fn calculate_rewards(&self, staked_amount: u64, days: u32) -> u64 {
        // Calculate daily rate from APY
        let daily_rate = self.base_apy as u128 * self.tier_multiplier as u128 * self.lock_multiplier as u128
            / (10000 * 10000 * 365);
        
        // Calculate rewards
        let rewards = (staked_amount as u128 * daily_rate * days as u128) / 10000;
        
        rewards as u64
    }
}

/// Export MMTStakeAccount as an alias
pub type MMTStakeAccount = StakeAccount;