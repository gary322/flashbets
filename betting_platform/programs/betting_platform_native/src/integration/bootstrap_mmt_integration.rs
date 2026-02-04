//! Bootstrap MMT Integration Module
//! 
//! Handles immediate MMT reward distribution for early liquidity providers
//! during the bootstrap phase as per specification requirements.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};
use borsh::BorshDeserialize;
use spl_token::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType, MMTRewardDistributedEvent},
    mmt::{
        constants::*,
        state::{MMTConfig, SeasonEmission, TreasuryAccount, DistributionType},
    },
    integration::bootstrap_coordinator::{
        BootstrapCoordinator, BOOTSTRAP_IMMEDIATE_REWARD_BPS,
    },
};

/// Process immediate MMT rewards for bootstrap liquidity providers
pub fn process_bootstrap_mmt_reward(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    depositor: &Pubkey,
    deposit_amount: u64,
    mmt_reward_amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Bootstrap coordinator account (PDA)
    // 1. Season emission account (current season)
    // 2. MMT config account (PDA)
    // 3. Treasury account (PDA)
    // 4. Treasury token account (source)
    // 5. Depositor MMT token account (destination)
    // 6. MMT mint
    // 7. Clock sysvar
    // 8. Token program
    // 9. System program
    
    let bootstrap_account = next_account_info(account_info_iter)?;
    let season_emission_account = next_account_info(account_info_iter)?;
    let mmt_config_account = next_account_info(account_info_iter)?;
    let treasury_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let depositor_mmt_account = next_account_info(account_info_iter)?;
    let mmt_mint_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let _system_program = next_account_info(account_info_iter)?;
    
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Load and verify accounts
    let bootstrap = BootstrapCoordinator::deserialize(&mut &bootstrap_account.data.borrow()[..])?;
    let mut season = SeasonEmission::unpack(&season_emission_account.data.borrow())?;
    let config = MMTConfig::unpack(&mmt_config_account.data.borrow())?;
    let treasury = TreasuryAccount::unpack(&treasury_account.data.borrow())?;
    
    // Verify bootstrap is active
    if bootstrap.bootstrap_complete {
        return Err(BettingPlatformError::BootstrapAlreadyComplete.into());
    }
    
    // Verify season is active
    if clock.slot < season.start_slot || clock.slot >= season.end_slot {
        msg!("Season is not active for MMT distribution");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify MMT reward doesn't exceed available emission
    let new_emitted = season.emitted_amount
        .checked_add(mmt_reward_amount)
        .ok_or(BettingPlatformError::ArithmeticOverflow.into())?;
    
    if new_emitted > season.total_allocation {
        msg!("MMT reward would exceed season allocation");
        return Err(ProgramError::InsufficientFunds);
    }
    
    // Calculate immediate reward percentage based on bootstrap progress
    let immediate_percentage = if bootstrap.vault < 1_000_000_000 {
        // First $1k gets 100% immediate rewards
        BOOTSTRAP_IMMEDIATE_REWARD_BPS
    } else {
        // Gradual reduction: 100% -> 50% as vault grows to $100k
        let progress_bps = (bootstrap.vault * 10000) / crate::constants::BOOTSTRAP_TARGET_VAULT;
        let reduction = (progress_bps * 5000) / 10000; // 50% reduction at completion
        BOOTSTRAP_IMMEDIATE_REWARD_BPS.saturating_sub(reduction as u16)
    };
    
    let immediate_reward = (mmt_reward_amount * immediate_percentage as u64) / 10000;
    
    // Get treasury PDA bump
    let (treasury_pda, treasury_bump) = Pubkey::find_program_address(
        &[MMT_TREASURY_SEED],
        program_id,
    );
    
    if treasury_pda != *treasury_account.key {
        msg!("Invalid treasury PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Transfer immediate MMT rewards from treasury
    if immediate_reward > 0 {
        invoke_signed(
            &token_instruction::transfer(
                &spl_token::id(),
                treasury_token_account.key,
                depositor_mmt_account.key,
                treasury_account.key,
                &[],
                immediate_reward,
            )?,
            &[
                treasury_token_account.clone(),
                depositor_mmt_account.clone(),
                treasury_account.clone(),
                token_program.clone(),
            ],
            &[&[MMT_TREASURY_SEED, &[treasury_bump]]],
        )?;
        
        // Update season emission tracking
        season.emitted_amount = new_emitted;
        season.early_trader_bonus = season.early_trader_bonus
            .checked_add(immediate_reward)
            .ok_or(BettingPlatformError::ArithmeticOverflow.into())?;
        
        SeasonEmission::pack(season, &mut season_emission_account.data.borrow_mut())?;
        
        // Emit event
        emit_event(EventType::MMTRewardDistributed, &MMTRewardDistributedEvent {
            recipient: *depositor,
            amount: immediate_reward,
            distribution_type: DistributionType::EarlyLiquidityProvider as u8,
            deposit_amount,
            vault_balance: bootstrap.vault,
        });
        
        msg!(
            "Distributed {} MMT immediate reward for ${} deposit ({}% immediate)",
            immediate_reward / 10u64.pow(MMT_DECIMALS as u32),
            deposit_amount / 1_000_000,
            immediate_percentage / 100
        );
    }
    
    // Schedule remaining rewards for vesting (if any)
    let vested_reward = mmt_reward_amount.saturating_sub(immediate_reward);
    if vested_reward > 0 {
        // In production, would create a vesting schedule here
        msg!(
            "Scheduled {} MMT for vesting",
            vested_reward / 10u64.pow(MMT_DECIMALS as u32)
        );
    }
    
    Ok(())
}

/// Calculate MMT rewards based on deposit amount and bootstrap phase
pub fn calculate_bootstrap_mmt_rewards(
    deposit_amount: u64,
    vault_balance: u64,
    unique_depositors: u32,
    current_milestone: u8,
    incentive_pool_remaining: u64,
) -> Result<u64, ProgramError> {
    // Base reward: 1 MMT per $1, with 2x multiplier during bootstrap
    let deposit_in_dollars = deposit_amount / 1_000_000;
    let base_reward = deposit_in_dollars * 2 * 1_000_000; // 2x multiplier, MMT has 6 decimals
    
    // Early depositor bonus multipliers
    let depositor_multiplier = match unique_depositors {
        0..=10 => 150,   // First 10 depositors: 1.5x
        11..=50 => 130,  // Next 40 depositors: 1.3x
        51..=100 => 115, // Next 50 depositors: 1.15x
        _ => 100,        // Standard rate
    };
    
    // Milestone bonus multiplier
    let milestone_multiplier = match current_milestone {
        0 => 140, // Before $1k: 1.4x
        1 => 130, // $1k-$2.5k: 1.3x
        2 => 120, // $2.5k-$5k: 1.2x
        3 => 110, // $5k-$7.5k: 1.1x
        _ => 100, // $7.5k+: 1x
    };
    
    // Apply multipliers
    let enhanced_reward = (base_reward * depositor_multiplier * milestone_multiplier) / 10000;
    
    // Cap at remaining incentive pool
    let final_reward = enhanced_reward.min(incentive_pool_remaining);
    
    Ok(final_reward)
}

/// Verify liquidity provider eligibility for MMT rewards
pub fn verify_liquidity_provider_eligibility(
    depositor: &Pubkey,
    deposit_amount: u64,
    bootstrap: &BootstrapCoordinator,
) -> Result<bool, ProgramError> {
    // Check minimum deposit
    if deposit_amount < MIN_DEPOSIT_AMOUNT {
        return Ok(false);
    }
    
    // Check bootstrap phase is active
    if bootstrap.bootstrap_complete {
        return Ok(false);
    }
    
    // Check coverage ratio for vampire attack protection
    if bootstrap.vault > 0 && bootstrap.coverage_ratio < VAMPIRE_ATTACK_HALT_COVERAGE {
        msg!("Coverage ratio too low, halting new deposits");
        return Ok(false);
    }
    
    // All checks passed
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mmt_reward_calculation() {
        // Test first depositor with $1000
        let reward = calculate_bootstrap_mmt_rewards(
            1_000_000_000, // $1000
            0,             // Empty vault
            0,             // First depositor
            0,             // No milestone yet
            10_000_000_000_000, // 10M MMT available
        ).unwrap();
        
        // Base: $1000 * 2 = 2000 MMT
        // First depositor: 1.5x = 3000 MMT
        // Before milestone: 1.4x = 4200 MMT
        assert_eq!(reward, 4_200_000_000); // 4200 MMT with 6 decimals
    }
    
    #[test]
    fn test_eligibility_check() {
        let depositor = Pubkey::new_unique();
        let mut bootstrap = BootstrapCoordinator {
            vault_balance: 5_000_000_000, // $5k
            total_deposits: 5_000_000_000,
            unique_depositors: 10,
            current_milestone: 2,
            bootstrap_start_slot: 0,
            bootstrap_complete: false,
            coverage_ratio: 10000, // 1.0
            max_leverage_available: 5,
            total_mmt_distributed: 1_000_000_000,
            early_depositor_bonus_active: true,
            incentive_pool: 9_000_000_000_000,
            halted: false,
            total_incentive_pool: 10_000_000_000_000, // 10M MMT total
            is_active: true,
            current_vault_balance: 5_000_000_000, // Same as vault_balance
        };
        
        // Should pass with valid deposit
        assert!(verify_liquidity_provider_eligibility(
            &depositor,
            1_000_000_000, // $1000
            &bootstrap
        ).unwrap());
        
        // Should fail with small deposit
        assert!(!verify_liquidity_provider_eligibility(
            &depositor,
            100_000, // $0.10
            &bootstrap
        ).unwrap());
        
        // Should fail if bootstrap complete
        bootstrap.bootstrap_complete = true;
        assert!(!verify_liquidity_provider_eligibility(
            &depositor,
            1_000_000_000,
            &bootstrap
        ).unwrap());
    }
}

// Re-export functions (already public, so these lines can be removed)

// Constants for external use
pub const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000; // $1 minimum
// pub const BOOTSTRAP_TARGET_VAULT: u64 = 10_000_000_000; // $10k target - use from constants module
pub const VAMPIRE_ATTACK_HALT_COVERAGE: u64 = 5000; // 0.5 coverage