//! Insurance and Zero-Loss Guarantee
//!
//! Implements zero-loss protection and insurance fund management

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
    state::{Vault, UserDeposit},
};

/// Insurance fund state
#[derive(Debug)]
pub struct InsuranceFund {
    /// Total fund value
    pub total_value: u128,
    
    /// Reserved for claims
    pub reserved_amount: u128,
    
    /// Available for new coverage
    pub available_amount: u128,
    
    /// Total claims paid
    pub total_claims_paid: u128,
    
    /// Number of claims
    pub claim_count: u64,
    
    /// Coverage ratio
    pub coverage_ratio: f64,
    
    /// Target coverage ratio
    pub target_coverage_ratio: f64,
    
    /// Premium rate (basis points)
    pub premium_rate: u16,
}

/// Zero-loss protection status
#[derive(Debug, Clone, PartialEq)]
pub enum ProtectionStatus {
    /// Fully protected
    Active,
    /// Partially protected
    Partial(f64),
    /// Not protected
    Inactive,
    /// Claimed
    Claimed,
}

/// Apply zero-loss protection to withdrawal
pub fn apply_zero_loss_protection(
    withdrawal_amount: u128,
    shares: u128,
    deposit: &UserDeposit,
    vault: &Vault,
) -> Result<u128, ProgramError> {
    // Check if protection is active
    if !deposit.zero_loss_protected {
        return Ok(withdrawal_amount);
    }
    
    // Calculate protected value
    let protected_value = calculate_protected_value(deposit, vault);
    
    // If current value is above protected value, no protection needed
    if withdrawal_amount >= protected_value {
        return Ok(withdrawal_amount);
    }
    
    // Calculate protection amount needed
    let protection_amount = protected_value - withdrawal_amount;
    
    // Check if insurance fund can cover
    if protection_amount > vault.insurance_fund {
        msg!("Insurance fund insufficient: {} > {}", 
             protection_amount, vault.insurance_fund);
        // Partial protection
        let covered = vault.insurance_fund;
        return Ok(withdrawal_amount + covered);
    }
    
    msg!("Applied zero-loss protection: {} added to withdrawal", protection_amount);
    
    Ok(protected_value)
}

/// Calculate protected value for deposit
pub fn calculate_protected_value(
    deposit: &UserDeposit,
    vault: &Vault,
) -> u128 {
    // Protection floor is the minimum of:
    // 1. Original deposit amount
    // 2. Protection floor price * shares
    
    let floor_value = (deposit.protection_floor * deposit.shares) / 1_000_000_000_000_000_000;
    let original_value = deposit.deposited_amount;
    
    floor_value.min(original_value)
}

/// Check if insurance coverage is available
pub fn check_insurance_coverage(
    amount: u128,
    vault: &Vault,
) -> Result<bool, ProgramError> {
    // Calculate required coverage
    let required_coverage = calculate_required_coverage(amount, vault);
    
    // Check if insurance fund has enough
    let has_coverage = vault.insurance_fund >= required_coverage;
    
    msg!("Insurance coverage check: required={}, available={}, covered={}", 
         required_coverage, vault.insurance_fund, has_coverage);
    
    Ok(has_coverage)
}

/// Calculate required coverage amount
fn calculate_required_coverage(
    amount: u128,
    vault: &Vault,
) -> u128 {
    // Coverage requirement based on risk parameters
    let risk_multiplier = vault.risk_params.risk_score as f64 / 100.0;
    let base_coverage = amount / 10; // 10% base coverage
    
    ((base_coverage as f64) * (1.0 + risk_multiplier)) as u128
}

/// Claim insurance for loss
pub fn claim_insurance(
    deposit: &mut UserDeposit,
    vault: &mut Vault,
    loss_amount: u128,
) -> Result<u128, ProgramError> {
    // Validate claim
    if !deposit.zero_loss_protected {
        return Err(BettingPlatformError::NotEligibleForInsurance.into());
    }
    
    // Check if already claimed
    if deposit.performance.realized_pnl >= 0 {
        return Err(BettingPlatformError::NoLossToClaim.into());
    }
    
    let claimable = loss_amount.min(vault.insurance_fund);
    
    // Update insurance fund
    vault.insurance_fund = vault.insurance_fund.saturating_sub(claimable);
    
    // Update deposit records
    deposit.performance.realized_pnl += claimable as i128;
    
    msg!("Insurance claim processed: {} paid from fund", claimable);
    
    Ok(claimable)
}

/// Fund the insurance pool
pub fn fund_insurance_pool(
    vault: &mut Vault,
    amount: u128,
) -> Result<(), ProgramError> {
    vault.insurance_fund += amount;
    
    msg!("Funded insurance pool with {}, new total: {}", 
         amount, vault.insurance_fund);
    
    Ok(())
}

/// Calculate insurance premium
pub fn calculate_insurance_premium(
    deposit_amount: u128,
    vault: &Vault,
    protection_level: ProtectionLevel,
) -> u128 {
    let base_premium = match protection_level {
        ProtectionLevel::Full => deposit_amount / 100,    // 1% for full
        ProtectionLevel::Partial(pct) => (deposit_amount * pct as u128) / 10000,
        ProtectionLevel::None => 0,
    };
    
    // Adjust for vault risk
    let risk_adjustment = 1.0 + (vault.risk_params.risk_score as f64 / 200.0);
    
    ((base_premium as f64) * risk_adjustment) as u128
}

/// Protection levels
#[derive(Debug, Clone)]
pub enum ProtectionLevel {
    /// Full protection (100% of deposit)
    Full,
    /// Partial protection (percentage)
    Partial(u16),
    /// No protection
    None,
}

/// Update insurance fund metrics
pub fn update_insurance_metrics(
    fund: &mut InsuranceFund,
    vault: &Vault,
) {
    fund.total_value = vault.insurance_fund;
    fund.available_amount = fund.total_value.saturating_sub(fund.reserved_amount);
    
    // Calculate coverage ratio
    if vault.total_value_locked > 0 {
        fund.coverage_ratio = fund.total_value as f64 / vault.total_value_locked as f64;
    } else {
        fund.coverage_ratio = 0.0;
    }
}

/// Reserve insurance for pending claims
pub fn reserve_insurance_amount(
    fund: &mut InsuranceFund,
    amount: u128,
) -> Result<(), ProgramError> {
    if amount > fund.available_amount {
        return Err(BettingPlatformError::InsufficientInsuranceFund.into());
    }
    
    fund.reserved_amount += amount;
    fund.available_amount = fund.available_amount.saturating_sub(amount);
    
    msg!("Reserved {} in insurance fund", amount);
    
    Ok(())
}

/// Release reserved insurance
pub fn release_reserved_insurance(
    fund: &mut InsuranceFund,
    amount: u128,
) {
    fund.reserved_amount = fund.reserved_amount.saturating_sub(amount);
    fund.available_amount += amount;
    
    msg!("Released {} from reserved insurance", amount);
}

/// Process insurance claim with validation
pub fn process_insurance_claim(
    deposit: &mut UserDeposit,
    vault: &mut Vault,
    fund: &mut InsuranceFund,
    claim_amount: u128,
) -> Result<u128, ProgramError> {
    // Validate eligibility
    if !validate_insurance_claim(deposit, claim_amount) {
        return Err(BettingPlatformError::InvalidInsuranceClaim.into());
    }
    
    // Calculate payout
    let payout = calculate_insurance_payout(
        claim_amount,
        deposit,
        vault,
        fund,
    )?;
    
    // Process payout
    fund.total_claims_paid += payout;
    fund.claim_count += 1;
    vault.insurance_fund = vault.insurance_fund.saturating_sub(payout);
    
    msg!("Processed insurance claim: {} paid out", payout);
    
    Ok(payout)
}

/// Validate insurance claim
fn validate_insurance_claim(
    deposit: &UserDeposit,
    claim_amount: u128,
) -> bool {
    // Check protection is active
    if !deposit.zero_loss_protected {
        return false;
    }
    
    // Check for actual loss
    if deposit.performance.realized_pnl >= 0 {
        return false;
    }
    
    // Check claim amount matches loss
    let actual_loss = deposit.performance.realized_pnl.abs() as u128;
    if claim_amount > actual_loss {
        return false;
    }
    
    true
}

/// Calculate insurance payout
fn calculate_insurance_payout(
    claim_amount: u128,
    deposit: &UserDeposit,
    vault: &Vault,
    fund: &InsuranceFund,
) -> Result<u128, ProgramError> {
    // Check available funds
    let available = fund.available_amount.min(vault.insurance_fund);
    
    if available == 0 {
        return Err(BettingPlatformError::InsuranceFundDepleted.into());
    }
    
    // Calculate payout (may be partial if fund insufficient)
    let payout = claim_amount.min(available);
    
    // Apply deductible if configured
    let deductible = deposit.deposited_amount / 100; // 1% deductible
    let final_payout = payout.saturating_sub(deductible);
    
    Ok(final_payout)
}

/// Rebalance insurance fund
pub fn rebalance_insurance_fund(
    vault: &mut Vault,
    fund: &mut InsuranceFund,
    target_ratio: f64,
) -> Result<(), ProgramError> {
    let target_value = (vault.total_value_locked as f64 * target_ratio) as u128;
    
    if vault.insurance_fund < target_value {
        // Need to increase fund
        let needed = target_value - vault.insurance_fund;
        msg!("Insurance fund needs {} more to reach target ratio", needed);
        
        // In production, would trigger funding mechanism
    } else if vault.insurance_fund > target_value * 2 {
        // Excess insurance, can redistribute
        let excess = vault.insurance_fund - target_value;
        msg!("Insurance fund has {} excess", excess);
        
        // Could redistribute to yield generation
    }
    
    fund.target_coverage_ratio = target_ratio;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_protection_calculation() {
        let deposit = UserDeposit {
            deposited_amount: 1000,
            shares: 1000,
            protection_floor: 1_000_000_000_000_000_000, // 1e18
            zero_loss_protected: true,
            ..Default::default()
        };
        
        let vault = Vault {
            share_price: 900_000_000_000_000_000, // 0.9e18 (10% loss)
            ..Default::default()
        };
        
        let protected = calculate_protected_value(&deposit, &vault);
        assert_eq!(protected, 1000); // Full protection
    }
    
    #[test]
    fn test_insurance_premium() {
        let vault = Vault {
            risk_params: super::super::state::RiskParameters {
                risk_score: 50,
                ..Default::default()
            },
            ..Default::default()
        };
        
        let premium = calculate_insurance_premium(
            10000,
            &vault,
            ProtectionLevel::Full,
        );
        
        // 1% base + 25% risk adjustment = 1.25%
        assert_eq!(premium, 125);
    }
}