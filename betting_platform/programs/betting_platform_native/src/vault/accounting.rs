//! Vault Accounting
//!
//! Accounting and record keeping for vault operations

use solana_program::{
    clock::Clock,
    msg,
    program_error::ProgramError,
    sysvar::Sysvar,
};

use super::state::{Vault, VaultEpoch, UserDeposit};

/// Update vault accounting after operations
pub fn update_vault_accounting(
    vault: &mut Vault,
    operation: AccountingOperation,
) -> Result<(), ProgramError> {
    match operation {
        AccountingOperation::Deposit(amount) => {
            vault.total_value_locked += amount;
        },
        AccountingOperation::Withdrawal(amount) => {
            vault.total_value_locked = vault.total_value_locked.saturating_sub(amount);
        },
        AccountingOperation::YieldGenerated(amount) => {
            vault.total_value_locked += amount;
            vault.performance.total_yield_generated += amount;
        },
        AccountingOperation::FeesCollected(amount) => {
            vault.performance.total_fees_earned += amount;
        },
    }
    
    // Update share price
    vault.share_price = vault.calculate_share_price();
    
    // Update timestamp
    vault.last_update = Clock::get()?.unix_timestamp;
    
    Ok(())
}

/// Accounting operations
#[derive(Debug)]
pub enum AccountingOperation {
    Deposit(u128),
    Withdrawal(u128),
    YieldGenerated(u128),
    FeesCollected(u128),
}

/// Record deposit transaction
pub fn record_deposit(
    vault: &Vault,
    user_deposit: &UserDeposit,
    amount: u128,
) -> Result<(), ProgramError> {
    msg!("Recording deposit: user={}, amount={}, shares={}", 
         user_deposit.user, amount, user_deposit.shares);
    
    // In production, would write to transaction log
    
    Ok(())
}

/// Record withdrawal transaction
pub fn record_withdrawal(
    vault: &Vault,
    user_deposit: &UserDeposit,
    amount: u128,
    shares: u128,
) -> Result<(), ProgramError> {
    msg!("Recording withdrawal: user={}, amount={}, shares={}", 
         user_deposit.user, amount, shares);
    
    // In production, would write to transaction log
    
    Ok(())
}

/// Close vault epoch and calculate performance
pub fn close_epoch(
    vault: &mut Vault,
) -> Result<VaultEpoch, ProgramError> {
    let current_time = Clock::get()?.unix_timestamp;
    
    let epoch = VaultEpoch {
        epoch_number: vault.epoch,
        start_time: vault.last_update,
        end_time: current_time,
        starting_tvl: 0, // Would track from previous epoch
        ending_tvl: vault.total_value_locked,
        yield_generated: vault.performance.total_yield_generated,
        fees_collected: vault.performance.total_fees_earned,
        performance_fee: calculate_performance_fee(vault),
        start_share_price: 0, // Would track from previous epoch
        end_share_price: vault.share_price,
        total_deposits: 0, // Would track during epoch
        total_withdrawals: 0, // Would track during epoch
    };
    
    // Increment epoch
    vault.epoch += 1;
    
    msg!("Closed epoch {}: TVL={}, yield={}", 
         epoch.epoch_number, epoch.ending_tvl, epoch.yield_generated);
    
    Ok(epoch)
}

/// Calculate performance fee
fn calculate_performance_fee(vault: &Vault) -> u128 {
    if vault.share_price <= vault.high_water_mark {
        return 0;
    }
    
    let profit = vault.share_price - vault.high_water_mark;
    (profit * vault.performance_fee as u128) / 10000
}

/// Calculate management fee
pub fn calculate_management_fee(
    vault: &Vault,
    duration_days: u64,
) -> u128 {
    let annual_fee = (vault.total_value_locked * vault.management_fee as u128) / 10000;
    (annual_fee * duration_days as u128) / 365
}