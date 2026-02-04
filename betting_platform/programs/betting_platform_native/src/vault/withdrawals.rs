//! Withdrawal Management
//!
//! Handles user withdrawals from vaults with zero-loss protection

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
};

use super::{
    state::{Vault, VaultStatus, UserDeposit},
    deposits::{is_deposit_locked, calculate_early_withdrawal_penalty},
    insurance::{apply_zero_loss_protection, claim_insurance},
    accounting::{update_vault_accounting, record_withdrawal},
};

/// Process user withdrawal from vault
pub fn process_withdrawal(
    program_id: &Pubkey,
    vault: &mut Vault,
    deposit: &mut UserDeposit,
    shares: u128,
    oracle: &OraclePDA,
) -> Result<u128, ProgramError> {
    // Validate vault is allowing withdrawals
    if !vault.is_allowing_withdrawals() {
        msg!("Vault not allowing withdrawals: status={:?}", vault.status);
        return Err(BettingPlatformError::VaultNotAllowingWithdrawals.into());
    }
    
    // Check if deposit is locked
    if is_deposit_locked(deposit) {
        msg!("Deposit is locked until {:?}", deposit.lock_until);
        // Calculate penalty for early withdrawal
        let penalty = calculate_early_withdrawal_penalty(deposit);
        if penalty > 0 {
            msg!("Early withdrawal penalty: {} shares", penalty);
            // Apply penalty by reducing shares
            let effective_shares = shares.saturating_sub(penalty);
            return process_withdrawal_internal(vault, deposit, effective_shares, oracle);
        }
    }
    
    // Process normal withdrawal
    process_withdrawal_internal(vault, deposit, shares, oracle)
}

/// Internal withdrawal processing
fn process_withdrawal_internal(
    vault: &mut Vault,
    deposit: &mut UserDeposit,
    shares: u128,
    oracle: &OraclePDA,
) -> Result<u128, ProgramError> {
    // Validate shares
    if shares > deposit.shares {
        msg!("Insufficient shares: {} > {}", shares, deposit.shares);
        return Err(BettingPlatformError::InsufficientShares.into());
    }
    
    // Calculate withdrawal amount
    let mut withdrawal_amount = vault.calculate_withdrawal_amount(shares);
    
    // Apply zero-loss protection if enabled
    if deposit.zero_loss_protected {
        withdrawal_amount = apply_zero_loss_protection(
            withdrawal_amount,
            shares,
            deposit,
            vault,
        )?;
    }
    
    // Apply withdrawal fee
    let (net_amount, fee) = vault.apply_withdrawal_fee(withdrawal_amount);
    
    // Check vault liquidity
    if net_amount > vault.available_liquidity {
        msg!("Insufficient vault liquidity: {} > {}", 
             net_amount, vault.available_liquidity);
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }
    
    // Update vault state
    vault.total_value_locked = vault.total_value_locked.saturating_sub(withdrawal_amount);
    vault.total_shares = vault.total_shares.saturating_sub(shares);
    vault.insurance_fund += fee / 2; // Half of fee goes to insurance
    vault.update_tvl(vault.total_value_locked);
    vault.update_utilization();
    
    // Update user deposit
    deposit.shares = deposit.shares.saturating_sub(shares);
    deposit.performance.total_withdrawn += net_amount;
    
    // Calculate and update PnL
    let share_ratio = shares as f64 / (shares + deposit.shares) as f64;
    let cost_basis = (deposit.deposited_amount as f64 * share_ratio) as u128;
    let pnl = net_amount as i128 - cost_basis as i128;
    deposit.performance.realized_pnl += pnl;
    
    // Update deposit amount proportionally
    deposit.deposited_amount = ((deposit.deposited_amount as f64) * (1.0 - share_ratio)) as u128;
    
    // Update current value
    deposit.performance.current_value = vault.calculate_withdrawal_amount(deposit.shares);
    
    // Update timestamp
    deposit.last_claim = Clock::get()?.unix_timestamp;
    vault.last_update = Clock::get()?.unix_timestamp;
    
    msg!("Processed withdrawal: shares={}, amount={}, fee={}, pnl={}", 
         shares, withdrawal_amount, fee, pnl);
    
    Ok(net_amount)
}

/// Process emergency withdrawal
pub fn process_emergency_withdrawal(
    vault: &mut Vault,
    deposit: &mut UserDeposit,
) -> Result<u128, ProgramError> {
    // Emergency withdrawals bypass normal checks
    let shares = deposit.shares;
    
    // Calculate withdrawal at current value
    let withdrawal_amount = vault.calculate_withdrawal_amount(shares);
    
    // No fees in emergency
    let net_amount = withdrawal_amount;
    
    // Update vault state
    vault.total_value_locked = vault.total_value_locked.saturating_sub(withdrawal_amount);
    vault.total_shares = vault.total_shares.saturating_sub(shares);
    
    // Clear user deposit
    deposit.shares = 0;
    deposit.performance.total_withdrawn += net_amount;
    deposit.performance.current_value = 0;
    
    msg!("Emergency withdrawal: shares={}, amount={}", shares, net_amount);
    
    Ok(net_amount)
}

/// Process partial withdrawal
pub fn process_partial_withdrawal(
    program_id: &Pubkey,
    vault: &mut Vault,
    deposit: &mut UserDeposit,
    percentage: u8,
    oracle: &OraclePDA,
) -> Result<u128, ProgramError> {
    if percentage > 100 {
        return Err(BettingPlatformError::InvalidPercentage.into());
    }
    
    let shares_to_withdraw = (deposit.shares * percentage as u128) / 100;
    
    process_withdrawal(program_id, vault, deposit, shares_to_withdraw, oracle)
}

/// Batch process withdrawals
pub fn process_batch_withdrawals(
    program_id: &Pubkey,
    vault: &mut Vault,
    withdrawals: &mut [(UserDeposit, u128)],
    oracle: &OraclePDA,
) -> Result<Vec<u128>, ProgramError> {
    let mut amounts = Vec::new();
    let mut total_withdrawn = 0u128;
    let mut total_shares = 0u128;
    
    // Check total liquidity first
    let total_requested: u128 = withdrawals.iter()
        .map(|(_, shares)| vault.calculate_withdrawal_amount(*shares))
        .sum();
    
    if total_requested > vault.available_liquidity {
        msg!("Batch withdrawal exceeds liquidity: {} > {}", 
             total_requested, vault.available_liquidity);
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }
    
    for (deposit, shares) in withdrawals.iter_mut() {
        let amount = process_withdrawal(
            program_id,
            vault,
            deposit,
            *shares,
            oracle,
        )?;
        
        total_withdrawn += amount;
        total_shares += *shares;
        amounts.push(amount);
    }
    
    msg!("Batch processed {} withdrawals: total={}, shares={}", 
         withdrawals.len(), total_withdrawn, total_shares);
    
    Ok(amounts)
}

/// Calculate maximum withdrawable amount
pub fn calculate_max_withdrawable(
    deposit: &UserDeposit,
    vault: &Vault,
) -> u128 {
    if is_deposit_locked(deposit) {
        // Apply penalty to locked deposits
        let penalty = calculate_early_withdrawal_penalty(deposit);
        let effective_shares = deposit.shares.saturating_sub(penalty);
        vault.calculate_withdrawal_amount(effective_shares)
    } else {
        vault.calculate_withdrawal_amount(deposit.shares)
    }
}

/// Queue withdrawal for processing
pub fn queue_withdrawal(
    deposit: &mut UserDeposit,
    shares: u128,
    process_time: i64,
) -> Result<(), ProgramError> {
    if shares > deposit.shares {
        return Err(BettingPlatformError::InsufficientShares.into());
    }
    
    // In production, would create a withdrawal queue entry
    msg!("Queued withdrawal: {} shares for processing at {}", 
         shares, process_time);
    
    Ok(())
}

/// Cancel queued withdrawal
pub fn cancel_queued_withdrawal(
    deposit: &mut UserDeposit,
    withdrawal_id: u128,
) -> Result<(), ProgramError> {
    // In production, would remove from withdrawal queue
    msg!("Cancelled queued withdrawal: {}", withdrawal_id);
    
    Ok(())
}

/// Process queued withdrawals
pub fn process_queued_withdrawals(
    vault: &mut Vault,
    current_time: i64,
) -> Result<u32, ProgramError> {
    let mut processed = 0u32;
    
    // In production, would iterate through withdrawal queue
    // and process those ready
    
    msg!("Processed {} queued withdrawals", processed);
    
    Ok(processed)
}

/// Validate withdrawal against oracle
pub fn validate_withdrawal_with_oracle(
    amount: u128,
    oracle: &OraclePDA,
) -> Result<bool, ProgramError> {
    // Check oracle freshness
    let current_slot = Clock::get()?.slot;
    let slots_elapsed = current_slot.saturating_sub(oracle.last_update_slot);
    
    if slots_elapsed > 10 {
        msg!("Oracle data too stale for withdrawal validation");
        return Ok(false);
    }
    
    // Validate withdrawal doesn't exceed safe limits based on volatility
    let max_safe_withdrawal = (1_000_000_000.0 / oracle.current_sigma) as u128;
    
    if amount > max_safe_withdrawal {
        msg!("Withdrawal exceeds safe limit based on volatility: {} > {}", 
             amount, max_safe_withdrawal);
        return Ok(false);
    }
    
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_withdrawal_calculation() {
        let vault = Vault {
            total_value_locked: 2_000_000,
            total_shares: 1_000_000,
            ..Default::default()
        };
        
        let amount = vault.calculate_withdrawal_amount(50_000);
        assert_eq!(amount, 100_000); // 2:1 value
    }
    
    #[test]
    fn test_max_withdrawable() {
        let vault = Vault {
            total_value_locked: 1_000_000,
            total_shares: 1_000_000,
            ..Default::default()
        };
        
        let deposit = UserDeposit {
            shares: 100_000,
            lock_until: None,
            ..Default::default()
        };
        
        let max = calculate_max_withdrawable(&deposit, &vault);
        assert_eq!(max, 100_000);
    }
}