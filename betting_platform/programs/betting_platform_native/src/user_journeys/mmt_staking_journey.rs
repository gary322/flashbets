//! MMT Staking User Journey
//! 
//! Complete flow for MMT token staking, tier progression, and reward claiming

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
    mmt::{
        RewardCalculator,
        calculate_tier_from_amount, calculate_apy_for_tier,
        staking::{StakingTier},
        state::StakeAccount as MMTStakeAccount,
    },
    events::{emit_event, EventType, MMTStakeEvent, MMTRewardClaimEvent},
    math::U64F64,
};

/// MMT staking journey state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MMTStakingJourney {
    /// User public key
    pub user: Pubkey,
    
    /// Current step
    pub current_step: StakingStep,
    
    /// Staking details
    pub total_staked: u64,
    pub current_tier: StakingTier,
    pub lock_duration: u64, // in slots
    
    /// Reward tracking
    pub total_rewards_earned: u64,
    pub last_reward_claim: i64,
    pub pending_rewards: u64,
    
    /// Journey timestamps
    pub journey_start: i64,
    pub last_action: i64,
}

/// Staking journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum StakingStep {
    /// Not started
    NotStarted,
    
    /// Initialized stake account
    AccountInitialized,
    
    /// First stake made
    FirstStake,
    
    /// Tier upgraded
    TierUpgraded,
    
    /// Rewards claimed
    RewardsClaimed,
    
    /// Partial unstake
    PartialUnstake,
    
    /// Full unstake
    FullUnstake,
}

/// Initialize MMT staking journey
pub fn initialize_mmt_staking(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let stake_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify stake account is not initialized
    if stake_account.data_len() > 0 {
        return Err(BettingPlatformError::AlreadyInitialized.into());
    }
    
    msg!("Initializing MMT staking journey for user {}", user_account.key);
    
    // Create stake account
    let stake_data = MMTStakeAccount {
        discriminator: [0x53, 0x54, 0x41, 0x4B, 0x45, 0x00, 0x00, 0x00],
        is_initialized: true,
        owner: *user_account.key,
        amount_staked: 0,
        stake_timestamp: Clock::get()?.unix_timestamp,
        last_claim_slot: Clock::get()?.slot,
        accumulated_rewards: 0,
        rebate_percentage: 0,
        lock_end_slot: None,
        lock_multiplier: 10000, // 1x base
        tier: StakingTier::Bronze,
        amount: 0,
        is_locked: false,
        rewards_earned: 0,
    };
    
    // Allocate space and transfer lamports
    let space = std::mem::size_of::<MMTStakeAccount>();
    let rent = solana_program::rent::Rent::default().minimum_balance(space);
    
    solana_program::program::invoke(
        &solana_program::system_instruction::create_account(
            user_account.key,
            stake_account.key,
            rent,
            space as u64,
            program_id,
        ),
        &[user_account.clone(), stake_account.clone(), system_program.clone()],
    )?;
    
    // Save stake account
    stake_data.serialize(&mut &mut stake_account.data.borrow_mut()[..])?;
    
    msg!("MMT stake account initialized successfully");
    
    // Emit initialization event
    emit_event(EventType::MMTStake, &MMTStakeEvent {
        staker: *user_account.key,
        amount: 0, // Initial stake amount is 0
        lock_period_days: 0,
        tier: StakingTier::Bronze as u8,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Stake MMT tokens
pub fn stake_mmt_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    lock_duration_days: u32,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let stake_account = next_account_info(account_iter)?;
    let user_mmt_account = next_account_info(account_iter)?;
    let stake_pool_mmt_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    msg!("Staking {} MMT tokens for {} days", amount, lock_duration_days);
    
    // Load stake account
    let mut stake_data = MMTStakeAccount::try_from_slice(&stake_account.data.borrow())?;
    
    // Verify ownership
    if stake_data.owner != *user_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Step 1: Calculate pending rewards before stake
    msg!("Step 1: Calculating pending rewards");
    let pending_rewards = calculate_pending_rewards(&stake_data)?;
    stake_data.rewards_earned += pending_rewards;
    
    // Step 2: Transfer MMT to stake pool
    msg!("Step 2: Transferring MMT to stake pool");
    let transfer_ix = spl_token::instruction::transfer(
        &spl_token::id(),
        user_mmt_account.key,
        stake_pool_mmt_account.key,
        user_account.key,
        &[],
        amount,
    )?;
    
    solana_program::program::invoke(
        &transfer_ix,
        &[
            user_mmt_account.clone(),
            stake_pool_mmt_account.clone(),
            user_account.clone(),
            token_program.clone(),
        ],
    )?;
    
    // Step 3: Update stake amount and tier
    msg!("Step 3: Updating stake amount and tier");
    let old_tier = stake_data.tier;
    stake_data.amount_staked += amount;
    stake_data.tier = calculate_tier_from_amount(stake_data.amount_staked);
    
    // Step 4: Set lock period if specified
    if lock_duration_days > 0 {
        let slots_per_day = 216_000; // Assuming 400ms slots
        let lock_duration_slots = lock_duration_days as u64 * slots_per_day;
        stake_data.lock_end_slot = Some(Clock::get()?.slot + lock_duration_slots);
        stake_data.is_locked = true;
        
        msg!("Stake locked until slot {:?}", stake_data.lock_end_slot);
    }
    
    // Step 5: Update timestamps
    stake_data.stake_timestamp = Clock::get()?.unix_timestamp;
    stake_data.last_claim_slot = Clock::get()?.slot;
    
    // Save stake account
    stake_data.serialize(&mut &mut stake_account.data.borrow_mut()[..])?;
    
    // Step 6: Check for tier upgrade
    let tier_upgraded = stake_data.tier as u8 > old_tier as u8;
    if tier_upgraded {
        msg!("Congratulations! Tier upgraded from {:?} to {:?}", old_tier, stake_data.tier);
        
        // Emit tier upgrade as a stake event with new tier
        emit_event(EventType::MMTStake, &MMTStakeEvent {
            staker: *user_account.key,
            amount: stake_data.amount_staked,
            lock_period_days: 0, // Not changing lock period
            tier: stake_data.tier as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }
    
    // Calculate new APY
    let apy_bps = calculate_apy_for_tier(stake_data.tier);
    msg!("Current APY: {} bps ({}%)", apy_bps, apy_bps as f64 / 100.0);
    
    // Emit stake event
    emit_event(EventType::MMTStake, &MMTStakeEvent {
        staker: *user_account.key,
        amount: stake_data.amount_staked,
        lock_period_days: lock_duration_days,
        tier: stake_data.tier as u8,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("MMT staking completed successfully!");
    msg!("Total staked: {} MMT", stake_data.amount_staked);
    msg!("Current tier: {:?}", stake_data.tier);
    
    Ok(())
}

/// Claim MMT staking rewards
pub fn claim_mmt_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let stake_account = next_account_info(account_iter)?;
    let user_mmt_account = next_account_info(account_iter)?;
    let rewards_pool_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;
    let mint_authority_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    msg!("Claiming MMT staking rewards for user {}", user_account.key);
    
    // Load stake account
    let mut stake_data = MMTStakeAccount::try_from_slice(&stake_account.data.borrow())?;
    
    // Verify ownership
    if stake_data.owner != *user_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Step 1: Calculate claimable rewards
    msg!("Step 1: Calculating claimable rewards");
    let pending_rewards = calculate_pending_rewards(&stake_data)?;
    let total_claimable = pending_rewards + stake_data.rewards_earned;
    
    if total_claimable == 0 {
        msg!("No rewards to claim");
        return Ok(());
    }
    
    msg!("Claimable rewards: {} MMT", total_claimable);
    
    // Step 2: No wash trading penalty in current implementation
    let rewards_after_penalty = total_claimable;
    
    // Step 3: Mint rewards to user
    msg!("Step 2: Minting {} MMT rewards", rewards_after_penalty);
    
    let mint_ix = spl_token::instruction::mint_to(
        &spl_token::id(),
        mmt_mint_account.key,
        user_mmt_account.key,
        mint_authority_account.key,
        &[],
        rewards_after_penalty,
    )?;
    
    let bump = 1u8;
    let seed_bytes = b"mmt_mint_authority";
    let seeds = &[seed_bytes.as_ref(), &[bump]];
    solana_program::program::invoke_signed(
        &mint_ix,
        &[
            mmt_mint_account.clone(),
            user_mmt_account.clone(),
            mint_authority_account.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;
    
    // Step 4: Update stake account
    stake_data.rewards_earned = 0;
    stake_data.last_claim_slot = Clock::get()?.slot;
    stake_data.serialize(&mut &mut stake_account.data.borrow_mut()[..])?;
    
    // Emit claim event
    emit_event(EventType::MMTRewardClaim, &MMTRewardClaimEvent {
        staker: *user_account.key,
        amount: rewards_after_penalty,
        rewards_type: 1, // 1 = staking rewards
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("Rewards claimed successfully!");
    
    Ok(())
}

/// Unstake MMT tokens
pub fn unstake_mmt_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let stake_account = next_account_info(account_iter)?;
    let user_mmt_account = next_account_info(account_iter)?;
    let stake_pool_mmt_account = next_account_info(account_iter)?;
    let stake_pool_authority = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    msg!("Unstaking {} MMT tokens", amount);
    
    // Load stake account
    let mut stake_data = MMTStakeAccount::try_from_slice(&stake_account.data.borrow())?;
    
    // Verify ownership
    if stake_data.owner != *user_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Step 1: Check lock period
    if stake_data.is_locked && stake_data.lock_end_slot.is_some() && Clock::get()?.slot < stake_data.lock_end_slot.unwrap() {
        msg!("Stake is locked until slot {:?}", stake_data.lock_end_slot);
        return Err(BettingPlatformError::StakeLocked.into());
    }
    
    // Verify unstake amount
    if amount > stake_data.amount_staked {
        return Err(BettingPlatformError::InsufficientStake.into());
    }
    
    // Step 2: Calculate and save pending rewards
    let pending_rewards = calculate_pending_rewards(&stake_data)?;
    stake_data.rewards_earned += pending_rewards;
    
    // Step 3: Transfer MMT back to user
    msg!("Transferring {} MMT back to user", amount);
    
    let transfer_ix = spl_token::instruction::transfer(
        &spl_token::id(),
        stake_pool_mmt_account.key,
        user_mmt_account.key,
        stake_pool_authority.key,
        &[],
        amount,
    )?;
    
    let bump = 1u8;
    let seed_bytes = b"stake_pool_authority";
    let seeds = &[seed_bytes.as_ref(), &[bump]];
    solana_program::program::invoke_signed(
        &transfer_ix,
        &[
            stake_pool_mmt_account.clone(),
            user_mmt_account.clone(),
            stake_pool_authority.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;
    
    // Step 4: Update stake amount and tier
    let old_tier = stake_data.tier;
    stake_data.amount_staked -= amount;
    stake_data.tier = calculate_tier_from_amount(stake_data.amount_staked);
    
    // Check for tier downgrade
    if (stake_data.tier as u8) < (old_tier as u8) {
        msg!("Tier downgraded from {:?} to {:?}", old_tier, stake_data.tier);
        
        // Emit tier downgrade as a stake event with new tier
        emit_event(EventType::MMTStake, &MMTStakeEvent {
            staker: *user_account.key,
            amount: stake_data.amount_staked,
            lock_period_days: 0, // Not changing lock period
            tier: stake_data.tier as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }
    
    // Reset lock if fully unstaked
    if stake_data.amount_staked == 0 {
        stake_data.is_locked = false;
        stake_data.lock_end_slot = None;
    }
    
    // Save stake account
    stake_data.serialize(&mut &mut stake_account.data.borrow_mut()[..])?;
    
    // Emit unstake event as a stake event with reduced amount
    emit_event(EventType::MMTStake, &MMTStakeEvent {
        staker: *user_account.key,
        amount: stake_data.amount_staked, // Updated total amount
        lock_period_days: 0, // Not changing lock period
        tier: stake_data.tier as u8,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("Unstaking completed successfully!");
    msg!("Remaining staked: {} MMT", stake_data.amount_staked);
    msg!("Current tier: {:?}", stake_data.tier);
    
    Ok(())
}

/// Get staking journey status
pub fn get_staking_journey_status(
    stake_data: &MMTStakeAccount,
) -> MMTStakingJourneyStatus {
    let pending_rewards = calculate_pending_rewards(stake_data).unwrap_or(0);
    let current_apy = calculate_apy_for_tier(stake_data.tier);
    
    MMTStakingJourneyStatus {
        user: stake_data.owner,
        total_staked: stake_data.amount_staked,
        current_tier: stake_data.tier,
        is_locked: stake_data.is_locked,
        lock_end_slot: stake_data.lock_end_slot,
        total_rewards_earned: stake_data.rewards_earned,
        pending_rewards,
        current_apy_bps: current_apy,
        wash_trading_penalty_bps: 0, // No penalty in current implementation
        participation_multiplier: 100, // Default 1x multiplier
        next_tier_requirement: get_next_tier_requirement(stake_data.tier),
    }
}

/// Calculate pending rewards
fn calculate_pending_rewards(stake_data: &crate::mmt::StakeAccount) -> Result<u64, ProgramError> {
    if stake_data.amount_staked == 0 {
        return Ok(0);
    }
    
    let current_time = Clock::get()?.unix_timestamp;
    let current_slot = Clock::get()?.slot;
    let slots_elapsed = current_slot.saturating_sub(stake_data.last_claim_slot);
    let time_elapsed = slots_elapsed * 400 / 1000; // Convert slots to seconds (400ms per slot)
    
    // Convert to years (assuming 365 days)
    let years_elapsed = U64F64::from_num(time_elapsed) / U64F64::from_num(365 * 24 * 60 * 60);
    
    // Get APY for tier
    let apy_bps = calculate_apy_for_tier(stake_data.tier) as u64;
    let apy_rate = U64F64::from_num(apy_bps) / U64F64::from_num(10000);
    
    // Calculate rewards: stake * apy * time * multiplier
    let base_rewards = U64F64::from_num(stake_data.amount_staked) * apy_rate * years_elapsed;
    // Apply lock multiplier if locked
    let lock_multiplier = U64F64::from_num(stake_data.lock_multiplier.into()) / U64F64::from_num(10000);
    let total_rewards = base_rewards * lock_multiplier;
    
    Ok(total_rewards.to_num())
}

/// Get requirement for next tier
fn get_next_tier_requirement(current_tier: StakingTier) -> Option<u64> {
    match current_tier {
        StakingTier::Bronze => Some(1_000 * 10u64.pow(9)),      // 1k MMT
        StakingTier::Silver => Some(10_000 * 10u64.pow(9)),     // 10k MMT
        StakingTier::Gold => Some(100_000 * 10u64.pow(9)),      // 100k MMT
        StakingTier::Platinum => Some(1_000_000 * 10u64.pow(9)), // 1M MMT
        StakingTier::Diamond => None,                            // Max tier
    }
}

/// MMT staking journey status
#[derive(Debug)]
pub struct MMTStakingJourneyStatus {
    pub user: Pubkey,
    pub total_staked: u64,
    pub current_tier: StakingTier,
    pub is_locked: bool,
    pub lock_end_slot: Option<u64>,
    pub total_rewards_earned: u64,
    pub pending_rewards: u64,
    pub current_apy_bps: u16,
    pub wash_trading_penalty_bps: u16,
    pub participation_multiplier: u8,
    pub next_tier_requirement: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tier_requirements() {
        assert_eq!(get_next_tier_requirement(StakingTier::Bronze), Some(1_000_000_000_000));
        assert_eq!(get_next_tier_requirement(StakingTier::Silver), Some(10_000_000_000_000));
        assert_eq!(get_next_tier_requirement(StakingTier::Gold), Some(100_000_000_000_000));
        assert_eq!(get_next_tier_requirement(StakingTier::Platinum), Some(1_000_000_000_000_000));
        assert_eq!(get_next_tier_requirement(StakingTier::Diamond), None);
    }
}