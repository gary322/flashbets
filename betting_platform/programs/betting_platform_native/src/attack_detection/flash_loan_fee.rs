//! Flash Loan Fee Implementation
//! 
//! Implements 2% fee for flash loan protection per specification

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    msg,
};
use crate::error::BettingPlatformError;

/// Flash loan fee in basis points (2% = 200 bps)
pub const FLASH_LOAN_FEE_BPS: u16 = 200;

/// Apply flash loan fee to an amount
pub fn apply_flash_loan_fee(amount: u64) -> Result<u64, ProgramError> {
    // Calculate fee: amount * fee_bps / 10000
    let fee = amount
        .checked_mul(FLASH_LOAN_FEE_BPS as u64)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(10000)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    msg!("Flash loan fee calculated: {} on amount {}", fee, amount);
    Ok(fee)
}

/// Calculate total amount including flash loan fee
pub fn calculate_flash_loan_total(principal: u64) -> Result<u64, ProgramError> {
    let fee = apply_flash_loan_fee(principal)?;
    let total = principal
        .checked_add(fee)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    msg!("Flash loan total: {} (principal: {}, fee: {})", total, principal, fee);
    Ok(total)
}

/// Verify flash loan repayment includes required fee
pub fn verify_flash_loan_repayment(
    borrowed: u64,
    repaid: u64,
) -> Result<(), ProgramError> {
    let required_total = calculate_flash_loan_total(borrowed)?;
    
    if repaid < required_total {
        msg!("Insufficient flash loan repayment: repaid {}, required {}", repaid, required_total);
        return Err(BettingPlatformError::InsufficientFlashLoanRepayment.into());
    }
    
    Ok(())
}

/// Process flash loan with fee
pub fn process_flash_loan(
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    // This would integrate with the existing flash loan detection
    // For now, just calculate and log the fee
    let fee = apply_flash_loan_fee(amount)?;
    let total = calculate_flash_loan_total(amount)?;
    
    msg!("Processing flash loan: amount={}, fee={}, total={}", amount, fee, total);
    
    // In production, this would:
    // 1. Transfer the requested amount to borrower
    // 2. Set up repayment tracking
    // 3. Verify repayment includes fee in same transaction
    
    Ok(())
}