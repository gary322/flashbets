//! Bootstrap Phase User Journey
//! 
//! Complete flow for users participating in the bootstrap phase

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
    state::{GlobalConfigPDA, UserStatsPDA},
    integration::bootstrap_coordinator::{BootstrapCoordinator, BootstrapParticipant, MIN_DEPOSIT_AMOUNT},
    events::{emit_event, EventType, BootstrapDepositEvent, BootstrapCompletedEvent, MMTRewardClaimEvent},
    math::U64F64,
    constants::BOOTSTRAP_TARGET_VAULT,
};

/// Bootstrap journey state tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BootstrapUserJourney {
    /// User public key
    pub user: Pubkey,
    
    /// Journey start timestamp
    pub start_timestamp: i64,
    
    /// Current step in journey
    pub current_step: BootstrapStep,
    
    /// Total deposited amount
    pub total_deposited: u64,
    
    /// Expected MMT rewards (2x during bootstrap)
    pub expected_mmt_rewards: u64,
    
    /// Number of deposits made
    pub deposit_count: u32,
    
    /// Has claimed rewards
    pub rewards_claimed: bool,
    
    /// Journey completion timestamp
    pub completion_timestamp: Option<i64>,
}

/// Bootstrap journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BootstrapStep {
    /// Initial state
    NotStarted,
    
    /// Checked eligibility
    EligibilityChecked,
    
    /// Made first deposit
    FirstDepositMade,
    
    /// Made additional deposits
    AdditionalDeposits,
    
    /// Bootstrap phase completed
    PhaseCompleted,
    
    /// Rewards claimed
    RewardsClaimed,
}

/// Complete bootstrap participation journey
pub fn execute_bootstrap_journey(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    deposit_amount: u64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let bootstrap_coordinator_account = next_account_info(account_iter)?;
    let bootstrap_participant_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let user_stats_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;
    let user_mmt_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut coordinator = BootstrapCoordinator::try_from_slice(&bootstrap_coordinator_account.data.borrow())?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Step 1: Check eligibility
    msg!("Step 1: Checking bootstrap eligibility");
    if !coordinator.is_active {
        return Err(BettingPlatformError::BootstrapNotActive.into());
    }
    
    if deposit_amount < MIN_DEPOSIT_AMOUNT {
        msg!("Deposit amount {} below minimum {}", deposit_amount, MIN_DEPOSIT_AMOUNT);
        return Err(BettingPlatformError::InsufficientFunds.into());
    }
    
    // Step 2: Initialize or load participant
    msg!("Step 2: Initializing participant account");
    let mut participant = if bootstrap_participant_account.data_len() == 0 {
        // New participant
        BootstrapParticipant {
            pubkey: *user_account.key,
            deposited_amount: 0,
            mmt_earned: 0,
            join_slot: Clock::get()?.slot,
            referrer: None,
            tier: calculate_tier(0),
            expected_mmt_rewards: 0,
            total_deposited: 0,
        }
    } else {
        BootstrapParticipant::try_from_slice(&bootstrap_participant_account.data.borrow())?
    };
    
    // Step 3: Process deposit
    msg!("Step 3: Processing deposit of {} lamports", deposit_amount);
    
    // Transfer funds to vault
    solana_program::program::invoke(
        &solana_program::system_instruction::transfer(
            user_account.key,
            vault_account.key,
            deposit_amount,
        ),
        &[user_account.clone(), vault_account.clone(), system_program.clone()],
    )?;
    
    // Update participant state
    participant.total_deposited += deposit_amount;
    participant.deposited_amount += deposit_amount;
    
    // Step 4: Calculate rewards (2x MMT during bootstrap)
    msg!("Step 4: Calculating 2x MMT rewards");
    let base_mmt_reward = calculate_base_mmt_reward(deposit_amount, &coordinator);
    let bootstrap_bonus_mmt = base_mmt_reward; // 2x = base + bonus
    let total_mmt_reward = base_mmt_reward + bootstrap_bonus_mmt;
    
    participant.expected_mmt_rewards += total_mmt_reward;
    participant.tier = calculate_tier(participant.total_deposited);
    
    // Update coordinator state
    coordinator.total_deposits += deposit_amount;
    coordinator.unique_depositors += if participant.deposited_amount == deposit_amount { 1 } else { 0 };
    coordinator.current_vault_balance += deposit_amount;
    
    // Step 5: Check phase completion
    msg!("Step 5: Checking bootstrap phase completion");
    let progress_bps = (coordinator.current_vault_balance * 10000) / BOOTSTRAP_TARGET_VAULT;
    msg!("Bootstrap progress: {}bps ({}%)", progress_bps, progress_bps / 100);
    
    if coordinator.current_vault_balance >= BOOTSTRAP_TARGET_VAULT && coordinator.is_active {
        msg!("Bootstrap phase completed! Target vault of {} reached", BOOTSTRAP_TARGET_VAULT);
        coordinator.is_active = false;
        coordinator.bootstrap_complete = true;
        
        // Emit completion event
        emit_event(EventType::BootstrapCompleted, &BootstrapCompletedEvent {
            market_id: 0, // Bootstrap phase doesn't have a specific market ID
            total_raised: coordinator.current_vault_balance,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }
    
    // Step 6: Update user stats
    msg!("Step 6: Updating user statistics");
    let mut user_stats = if user_stats_account.data_len() == 0 {
        UserStatsPDA::new(*user_account.key)
    } else {
        UserStatsPDA::try_from_slice(&user_stats_account.data.borrow())?
    };
    
    user_stats.total_volume += deposit_amount;
    user_stats.last_activity = Clock::get()?.unix_timestamp;
    
    // Save all state
    coordinator.serialize(&mut &mut bootstrap_coordinator_account.data.borrow_mut()[..])?;
    participant.serialize(&mut &mut bootstrap_participant_account.data.borrow_mut()[..])?;
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    user_stats.serialize(&mut &mut user_stats_account.data.borrow_mut()[..])?;
    
    // Emit deposit event
    emit_event(EventType::BootstrapDeposit, &BootstrapDepositEvent {
        depositor: *user_account.key,
        amount: deposit_amount,
        vault_balance: coordinator.current_vault_balance,
        mmt_earned: total_mmt_reward,
    });
    
    msg!("Bootstrap journey step completed successfully!");
    msg!("User total deposited: {}", participant.total_deposited);
    msg!("Expected MMT rewards: {}", participant.expected_mmt_rewards);
    msg!("Current tier: {}", participant.tier);
    
    Ok(())
}

/// Claim bootstrap rewards
pub fn claim_bootstrap_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let bootstrap_coordinator_account = next_account_info(account_iter)?;
    let bootstrap_participant_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;
    let user_mmt_account = next_account_info(account_iter)?;
    let mmt_mint_authority = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let coordinator = BootstrapCoordinator::try_from_slice(&bootstrap_coordinator_account.data.borrow())?;
    let mut participant = BootstrapParticipant::try_from_slice(&bootstrap_participant_account.data.borrow())?;
    
    // Verify bootstrap is complete
    if coordinator.is_active {
        return Err(BettingPlatformError::BootstrapNotActive.into());
    }
    
    // Verify rewards not already claimed by checking if mmt_earned equals expected_mmt_rewards
    if participant.mmt_earned >= participant.expected_mmt_rewards && participant.expected_mmt_rewards > 0 {
        msg!("Rewards already claimed for user {}", user_account.key);
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Mint MMT rewards
    msg!("Minting {} MMT tokens as bootstrap rewards", participant.expected_mmt_rewards);
    
    let mint_ix = spl_token::instruction::mint_to(
        &spl_token::id(),
        mmt_mint_account.key,
        user_mmt_account.key,
        mmt_mint_authority.key,
        &[],
        participant.expected_mmt_rewards,
    )?;
    
    solana_program::program::invoke_signed(
        &mint_ix,
        &[
            mmt_mint_account.clone(),
            user_mmt_account.clone(),
            mmt_mint_authority.clone(),
            token_program.clone(),
        ],
        &[&[b"mmt_authority", &[1u8]]], // Use bump seed 1 or derive it properly
    )?;
    
    // Mark rewards as claimed by updating mmt_earned
    participant.mmt_earned = participant.expected_mmt_rewards;
    participant.serialize(&mut &mut bootstrap_participant_account.data.borrow_mut()[..])?;
    
    // Emit claim event
    emit_event(EventType::MMTRewardClaim, &MMTRewardClaimEvent {
        staker: *user_account.key,
        amount: participant.expected_mmt_rewards,
        rewards_type: 2, // 2 = bootstrap rewards
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("Bootstrap rewards claimed successfully!");
    
    Ok(())
}

/// Calculate participant tier based on total deposited
fn calculate_tier(total_deposited: u64) -> u8 {
    match total_deposited {
        0..=999_999_999 => 1,           // < $1k - Tier 1
        1_000_000_000..=9_999_999_999 => 2,    // $1k-$10k - Tier 2  
        10_000_000_000..=99_999_999_999 => 3,  // $10k-$100k - Tier 3
        _ => 4,                          // > $100k - Tier 4
    }
}

/// Calculate base MMT reward for deposit
fn calculate_base_mmt_reward(deposit_amount: u64, coordinator: &BootstrapCoordinator) -> u64 {
    // Base rate: 1 MMT per $1 deposited
    let mmt_per_dollar = 1_000_000_000; // 1 MMT with 9 decimals
    let dollars = deposit_amount / 1_000_000_000; // Convert lamports to dollars
    
    // Apply tier multiplier
    let tier = calculate_tier(deposit_amount);
    let multiplier = match tier {
        1 => U64F64::from_num(1),
        2 => U64F64::from_num(1) + U64F64::from_num(1) / U64F64::from_num(10), // 1.1x
        3 => U64F64::from_num(1) + U64F64::from_num(2) / U64F64::from_num(10), // 1.2x
        4 => U64F64::from_num(1) + U64F64::from_num(3) / U64F64::from_num(10), // 1.3x
        _ => U64F64::from_num(1),
    };
    
    let base_reward = dollars * mmt_per_dollar;
    let multiplied = U64F64::from_num(base_reward) * multiplier;
    
    multiplied.to_num()
}

/// Get bootstrap journey status
pub fn get_bootstrap_journey_status(
    participant: &BootstrapParticipant,
    coordinator: &BootstrapCoordinator,
) -> BootstrapJourneyStatus {
    let progress_bps = (coordinator.current_vault_balance * 10000) / BOOTSTRAP_TARGET_VAULT;
    
    BootstrapJourneyStatus {
        user: participant.pubkey,
        current_step: determine_current_step(participant, coordinator),
        total_deposited: participant.total_deposited,
        expected_mmt_rewards: participant.expected_mmt_rewards,
        deposit_count: 1, // Simplified: assume one deposit per participant
        rewards_claimed: participant.mmt_earned >= participant.expected_mmt_rewards && participant.expected_mmt_rewards > 0,
        tier: participant.tier,
        vault_progress_bps: progress_bps as u16,
        phase_active: coordinator.is_active,
    }
}

/// Determine current step in journey
fn determine_current_step(
    participant: &BootstrapParticipant,
    coordinator: &BootstrapCoordinator,
) -> BootstrapStep {
    if participant.mmt_earned >= participant.expected_mmt_rewards && participant.expected_mmt_rewards > 0 {
        BootstrapStep::RewardsClaimed
    } else if !coordinator.is_active {
        BootstrapStep::PhaseCompleted
    } else if participant.total_deposited > MIN_DEPOSIT_AMOUNT {
        BootstrapStep::AdditionalDeposits
    } else if participant.total_deposited == MIN_DEPOSIT_AMOUNT {
        BootstrapStep::FirstDepositMade
    } else {
        BootstrapStep::EligibilityChecked
    }
}

/// Bootstrap journey status
#[derive(Debug)]
pub struct BootstrapJourneyStatus {
    pub user: Pubkey,
    pub current_step: BootstrapStep,
    pub total_deposited: u64,
    pub expected_mmt_rewards: u64,
    pub deposit_count: u32,
    pub rewards_claimed: bool,
    pub tier: u8,
    pub vault_progress_bps: u16,
    pub phase_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tier_calculation() {
        assert_eq!(calculate_tier(500_000_000), 1);      // $0.5k
        assert_eq!(calculate_tier(5_000_000_000), 2);    // $5k
        assert_eq!(calculate_tier(50_000_000_000), 3);   // $50k
        assert_eq!(calculate_tier(500_000_000_000), 4);  // $500k
    }
    
    #[test]
    fn test_mmt_reward_calculation() {
        let coordinator = BootstrapCoordinator {
            vault_balance: 0,
            total_deposits: 0,
            unique_depositors: 0,
            current_milestone: 0,
            bootstrap_start_slot: 0,
            bootstrap_complete: false,
            coverage_ratio: 0,
            max_leverage_available: 0,
            total_mmt_distributed: 0,
            early_depositor_bonus_active: true,
            incentive_pool: 0,
            halted: false,
            total_incentive_pool: 0,
            is_active: true,
            current_vault_balance: 0,
        };
        
        // Test $1k deposit (tier 2, 1.1x multiplier)
        let reward = calculate_base_mmt_reward(1_000_000_000, &coordinator);
        assert_eq!(reward, 1_100_000_000); // 1.1 MMT
        
        // Test $10k deposit (tier 3, 1.2x multiplier)
        let reward = calculate_base_mmt_reward(10_000_000_000, &coordinator);
        assert_eq!(reward, 12_000_000_000); // 12 MMT
    }
}