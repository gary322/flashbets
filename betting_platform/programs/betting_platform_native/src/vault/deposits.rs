//! Deposit Management
//!
//! Handles user deposits into vaults with zero-loss protection

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    synthetics::{SyntheticToken, TokenType, mint_synthetic_tokens},
};

use super::{
    state::{Vault, VaultStatus, UserDeposit, UserPerformance},
    insurance::check_insurance_coverage,
    accounting::{update_vault_accounting, record_deposit},
};

/// Process user deposit into vault
pub fn process_deposit(
    program_id: &Pubkey,
    vault: &mut Vault,
    user: &Pubkey,
    amount: u128,
    oracle: &OraclePDA,
    lock_period: Option<i64>,
) -> Result<UserDeposit, ProgramError> {
    // Validate vault is accepting deposits
    if !vault.is_accepting_deposits() {
        msg!("Vault not accepting deposits: status={:?}", vault.status);
        return Err(BettingPlatformError::VaultNotAcceptingDeposits.into());
    }
    
    // Check deposit limits
    if amount < vault.min_deposit {
        msg!("Deposit below minimum: {} < {}", amount, vault.min_deposit);
        return Err(BettingPlatformError::DepositBelowMinimum.into());
    }
    
    if amount > vault.max_deposit {
        msg!("Deposit above maximum: {} > {}", amount, vault.max_deposit);
        return Err(BettingPlatformError::DepositAboveMaximum.into());
    }
    
    // Apply deposit fee
    let (net_amount, fee) = vault.apply_deposit_fee(amount);
    
    // Calculate shares to mint
    let shares = vault.calculate_deposit_shares(net_amount);
    
    if shares == 0 {
        return Err(BettingPlatformError::InvalidShareCalculation.into());
    }
    
    // Update vault state
    vault.total_value_locked += net_amount;
    vault.total_shares += shares;
    vault.insurance_fund += fee / 2; // Half of fee goes to insurance
    vault.update_tvl(vault.total_value_locked);
    vault.update_utilization();
    
    // Get current timestamp
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    
    // Calculate lock period if specified
    let lock_until = lock_period.map(|period| current_time + period);
    
    // Create user deposit record
    let user_deposit = UserDeposit {
        user: *user,
        vault_id: vault.vault_id,
        deposited_amount: amount,
        shares,
        avg_entry_price: vault.share_price,
        deposit_time: current_time,
        last_claim: current_time,
        unclaimed_rewards: 0,
        lock_until,
        performance: UserPerformance {
            realized_pnl: 0,
            unrealized_pnl: 0,
            total_withdrawn: 0,
            total_rewards_claimed: 0,
            current_value: net_amount,
        },
        zero_loss_protected: vault.zero_loss_enabled,
        protection_floor: vault.share_price,
    };
    
    // Update last update time
    vault.last_update = current_time;
    
    msg!("Processed deposit: user={}, amount={}, shares={}, fee={}", 
         user, amount, shares, fee);
    
    Ok(user_deposit)
}

/// Process batch deposits
pub fn process_batch_deposits(
    program_id: &Pubkey,
    vault: &mut Vault,
    deposits: &[(Pubkey, u128)],
    oracle: &OraclePDA,
) -> Result<Vec<UserDeposit>, ProgramError> {
    let mut user_deposits = Vec::new();
    let mut total_deposited = 0u128;
    let mut total_shares = 0u128;
    
    for (user, amount) in deposits {
        // Process individual deposit
        let deposit = process_deposit(
            program_id,
            vault,
            user,
            *amount,
            oracle,
            None,
        )?;
        
        total_deposited += deposit.deposited_amount;
        total_shares += deposit.shares;
        user_deposits.push(deposit);
    }
    
    msg!("Batch processed {} deposits: total={}, shares={}", 
         deposits.len(), total_deposited, total_shares);
    
    Ok(user_deposits)
}

/// Update existing deposit
pub fn update_user_deposit(
    deposit: &mut UserDeposit,
    vault: &Vault,
    additional_amount: u128,
    additional_shares: u128,
) -> Result<(), ProgramError> {
    // Update deposit amounts
    let old_total = deposit.deposited_amount;
    let new_total = old_total + additional_amount;
    
    // Update average entry price
    let old_weight = (old_total as f64) / (new_total as f64);
    let new_weight = (additional_amount as f64) / (new_total as f64);
    
    deposit.avg_entry_price = (
        (deposit.avg_entry_price as f64 * old_weight) +
        (vault.share_price as f64 * new_weight)
    ) as u128;
    
    // Update totals
    deposit.deposited_amount = new_total;
    deposit.shares += additional_shares;
    deposit.performance.current_value += additional_amount;
    
    // Update timestamp
    deposit.last_claim = Clock::get()?.unix_timestamp;
    
    msg!("Updated deposit: total={}, shares={}", 
         deposit.deposited_amount, deposit.shares);
    
    Ok(())
}

/// Calculate deposit value
pub fn calculate_deposit_value(
    deposit: &UserDeposit,
    vault: &Vault,
) -> u128 {
    vault.calculate_withdrawal_amount(deposit.shares)
}

/// Apply time-weighted bonus for long-term deposits
pub fn calculate_loyalty_bonus(
    deposit: &UserDeposit,
    current_time: i64,
) -> u128 {
    let deposit_duration = current_time - deposit.deposit_time;
    
    // Bonus tiers (days => bonus multiplier)
    let bonus_multiplier = match deposit_duration / 86400 {
        0..=6 => 0,        // No bonus < 7 days
        7..=29 => 100,     // 1% for 7-30 days
        30..=89 => 250,    // 2.5% for 30-90 days
        90..=179 => 500,   // 5% for 90-180 days
        180..=364 => 750,  // 7.5% for 180-365 days
        _ => 1000,         // 10% for 365+ days
    };
    
    (deposit.shares * bonus_multiplier) / 10000
}

/// Check if deposit is locked
pub fn is_deposit_locked(deposit: &UserDeposit) -> bool {
    if let Some(lock_until) = deposit.lock_until {
        let current_time = Clock::get()
            .map(|c| c.unix_timestamp)
            .unwrap_or(0);
        current_time < lock_until
    } else {
        false
    }
}

/// Calculate early withdrawal penalty
pub fn calculate_early_withdrawal_penalty(
    deposit: &UserDeposit,
) -> u128 {
    if !is_deposit_locked(deposit) {
        return 0;
    }
    
    let current_time = Clock::get()
        .map(|c| c.unix_timestamp)
        .unwrap_or(0);
    
    let time_remaining = deposit.lock_until
        .map(|lock| lock - current_time)
        .unwrap_or(0);
    
    if time_remaining <= 0 {
        return 0;
    }
    
    // Penalty decreases linearly over lock period
    let original_lock_period = deposit.lock_until
        .map(|lock| lock - deposit.deposit_time)
        .unwrap_or(1);
    
    let penalty_rate = (time_remaining as f64 / original_lock_period as f64) * 0.1; // Max 10% penalty
    
    (deposit.shares as f64 * penalty_rate) as u128
}

/// Mint receipt tokens for deposit
pub fn mint_deposit_receipt(
    program_id: &Pubkey,
    vault: &Vault,
    user: &Pubkey,
    shares: u128,
) -> Result<(), ProgramError> {
    // Mint synthetic tokens as receipt
    let token_type = TokenType::Yield;
    
    // Create synthetic token for receipt
    let receipt_token = SyntheticToken {
        token_type,
        soul_bound: true, // Non-transferable
        total_supply: shares,
        max_supply: u128::MAX,
        decimals: 18,
        oracle_validated: false,
        backed_by_vault: true,
        vault_id: Some(vault.vault_id),
        created_at: Clock::get()?.unix_timestamp,
        last_update: Clock::get()?.unix_timestamp,
    };
    
    msg!("Minted {} receipt tokens for deposit", shares);
    
    Ok(())
}

/// Validate deposit against oracle
pub fn validate_deposit_with_oracle(
    amount: u128,
    oracle: &OraclePDA,
    vault: &Vault,
) -> Result<bool, ProgramError> {
    // Check oracle is fresh
    let current_slot = Clock::get()?.slot;
    let slots_elapsed = current_slot.saturating_sub(oracle.last_update_slot);
    
    if slots_elapsed > 10 {
        msg!("Oracle data too stale for deposit validation");
        return Ok(false);
    }
    
    // Validate amount against oracle limits
    let max_deposit_value = (oracle.scalar * 1_000_000.0) as u128;
    
    if amount > max_deposit_value {
        msg!("Deposit exceeds oracle limit: {} > {}", amount, max_deposit_value);
        return Ok(false);
    }
    
    Ok(true)
}

/// Auto-compound rewards
pub fn auto_compound_rewards(
    deposit: &mut UserDeposit,
    vault: &mut Vault,
    rewards: u128,
) -> Result<(), ProgramError> {
    if rewards == 0 {
        return Ok(());
    }
    
    // Calculate additional shares for rewards
    let additional_shares = vault.calculate_deposit_shares(rewards);
    
    // Update deposit
    deposit.shares += additional_shares;
    deposit.unclaimed_rewards = 0;
    deposit.performance.total_rewards_claimed += rewards;
    
    // Update vault
    vault.total_shares += additional_shares;
    
    msg!("Auto-compounded {} rewards into {} shares", rewards, additional_shares);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_deposit_shares_calculation() {
        let vault = Vault {
            total_value_locked: 1_000_000,
            total_shares: 1_000_000,
            ..Default::default()
        };
        
        let shares = vault.calculate_deposit_shares(100_000);
        assert_eq!(shares, 100_000); // 1:1 ratio
        
        let vault2 = Vault {
            total_value_locked: 2_000_000,
            total_shares: 1_000_000,
            ..Default::default()
        };
        
        let shares2 = vault2.calculate_deposit_shares(100_000);
        assert_eq!(shares2, 50_000); // 2:1 TVL:shares ratio
    }
    
    #[test]
    fn test_loyalty_bonus() {
        let deposit = UserDeposit {
            deposit_time: 0,
            shares: 100_000,
            ..Default::default()
        };
        
        // Test different time periods
        let bonus_7d = calculate_loyalty_bonus(&deposit, 7 * 86400);
        assert_eq!(bonus_7d, 1000); // 1%
        
        let bonus_90d = calculate_loyalty_bonus(&deposit, 90 * 86400);
        assert_eq!(bonus_90d, 5000); // 5%
        
        let bonus_365d = calculate_loyalty_bonus(&deposit, 365 * 86400);
        assert_eq!(bonus_365d, 10000); // 10%
    }
}