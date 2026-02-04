#!/usr/bin/env rust-script
//! Test Flash Loan Fee Implementation
//! 
//! This test verifies the 2% flash loan fee mechanism works correctly

use std::fmt;

#[derive(Debug, PartialEq)]
struct ProgramError(&'static str);

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Flash loan fee in basis points (2% = 200 bps)
const FLASH_LOAN_FEE_BPS: u16 = 200;

/// Apply flash loan fee to an amount
fn apply_flash_loan_fee(amount: u64) -> Result<u64, ProgramError> {
    // Calculate fee: amount * fee_bps / 10000
    let fee = amount
        .checked_mul(FLASH_LOAN_FEE_BPS as u64)
        .ok_or(ProgramError("MathOverflow"))?
        .checked_div(10000)
        .ok_or(ProgramError("MathOverflow"))?;
    
    println!("Flash loan fee calculated: {} on amount {}", fee, amount);
    Ok(fee)
}

/// Calculate total amount including flash loan fee
fn calculate_flash_loan_total(principal: u64) -> Result<u64, ProgramError> {
    let fee = apply_flash_loan_fee(principal)?;
    let total = principal
        .checked_add(fee)
        .ok_or(ProgramError("MathOverflow"))?;
    
    println!("Flash loan total: {} (principal: {}, fee: {})", total, principal, fee);
    Ok(total)
}

/// Verify flash loan repayment includes required fee
fn verify_flash_loan_repayment(
    borrowed: u64,
    repaid: u64,
) -> Result<(), ProgramError> {
    let required_total = calculate_flash_loan_total(borrowed)?;
    
    if repaid < required_total {
        println!("Insufficient flash loan repayment: repaid {}, required {}", repaid, required_total);
        return Err(ProgramError("InsufficientFlashLoanRepayment"));
    }
    
    Ok(())
}

fn test_fee_calculation() {
    println!("\n=== Testing Flash Loan Fee Calculation ===");
    
    // Test various amounts
    let test_amounts = vec![
        (1_000_000, 20_000),        // 1M -> 20K fee (2%)
        (10_000_000, 200_000),      // 10M -> 200K fee
        (100_000_000, 2_000_000),   // 100M -> 2M fee
        (500_000_000, 10_000_000),  // 500M -> 10M fee
    ];
    
    for (amount, expected_fee) in test_amounts {
        let fee = apply_flash_loan_fee(amount).unwrap();
        assert_eq!(fee, expected_fee);
        println!("✓ Amount {} -> Fee {} (2%)", amount, fee);
    }
    
    // Test edge cases
    let fee = apply_flash_loan_fee(1).unwrap();
    assert_eq!(fee, 0); // 1 * 200 / 10000 = 0 (rounds down)
    println!("✓ Minimum amount handling: 1 -> 0 fee");
    
    let fee = apply_flash_loan_fee(49).unwrap();
    assert_eq!(fee, 0); // 49 * 200 / 10000 = 0.98 -> 0
    println!("✓ Rounding down: 49 -> 0 fee");
    
    let fee = apply_flash_loan_fee(50).unwrap();
    assert_eq!(fee, 1); // 50 * 200 / 10000 = 1
    println!("✓ Minimum fee threshold: 50 -> 1 fee");
}

fn test_total_calculation() {
    println!("\n=== Testing Total Amount Calculation ===");
    
    let test_cases = vec![
        (1_000_000, 1_020_000),     // 1M + 20K
        (10_000_000, 10_200_000),   // 10M + 200K
        (100_000_000, 102_000_000), // 100M + 2M
    ];
    
    for (principal, expected_total) in test_cases {
        let total = calculate_flash_loan_total(principal).unwrap();
        assert_eq!(total, expected_total);
        println!("✓ Principal {} -> Total {} (includes 2% fee)", principal, total);
    }
}

fn test_repayment_verification() {
    println!("\n=== Testing Repayment Verification ===");
    
    // Test exact repayment
    let borrowed = 1_000_000;
    let required = calculate_flash_loan_total(borrowed).unwrap();
    assert!(verify_flash_loan_repayment(borrowed, required).is_ok());
    println!("✓ Exact repayment accepted: borrowed {}, repaid {}", borrowed, required);
    
    // Test overpayment (should be accepted)
    let overpayment = required + 1000;
    assert!(verify_flash_loan_repayment(borrowed, overpayment).is_ok());
    println!("✓ Overpayment accepted: borrowed {}, repaid {}", borrowed, overpayment);
    
    // Test underpayment (should fail)
    let underpayment = required - 1;
    let result = verify_flash_loan_repayment(borrowed, underpayment);
    assert_eq!(result, Err(ProgramError("InsufficientFlashLoanRepayment")));
    println!("✓ Underpayment rejected: borrowed {}, repaid {} (required {})", 
        borrowed, underpayment, required);
}

fn test_overflow_protection() {
    println!("\n=== Testing Overflow Protection ===");
    
    // Test maximum safe value
    let max_safe = u64::MAX / 200; // Largest value that won't overflow in fee calculation
    let fee = apply_flash_loan_fee(max_safe).unwrap();
    println!("✓ Max safe amount {} -> fee {}", max_safe, fee);
    
    // Test overflow scenario
    let overflow_amount = u64::MAX;
    let result = apply_flash_loan_fee(overflow_amount);
    assert_eq!(result, Err(ProgramError("MathOverflow")));
    println!("✓ Overflow protection triggered for amount {}", overflow_amount);
}

fn test_economic_disincentive() {
    println!("\n=== Testing Economic Disincentive ===");
    
    // Simulate flash loan arbitrage scenarios
    let scenarios = vec![
        ("Small arbitrage", 1_000_000, 15_000),    // 1.5% profit
        ("Medium arbitrage", 10_000_000, 190_000),  // 1.9% profit
        ("Large arbitrage", 100_000_000, 2_100_000), // 2.1% profit
    ];
    
    for (name, loan_amount, profit) in scenarios {
        let fee = apply_flash_loan_fee(loan_amount).unwrap();
        let net_profit = profit as i64 - fee as i64;
        
        println!("\n{} scenario:", name);
        println!("  Loan: {}", loan_amount);
        println!("  Gross profit: {}", profit);
        println!("  Flash loan fee: {} (2%)", fee);
        println!("  Net profit: {}", net_profit);
        
        if net_profit > 0 {
            println!("  ✓ Profitable (profit > 2% threshold)");
        } else {
            println!("  ✗ Unprofitable (profit < 2% threshold)");
        }
    }
}

fn main() {
    println!("Flash Loan Fee Test Suite");
    println!("=========================");
    println!("Fee Rate: {} basis points ({}%)", FLASH_LOAN_FEE_BPS, FLASH_LOAN_FEE_BPS as f64 / 100.0);
    
    test_fee_calculation();
    test_total_calculation();
    test_repayment_verification();
    test_overflow_protection();
    test_economic_disincentive();
    
    println!("\n✅ All flash loan fee tests passed!");
    println!("\nSummary:");
    println!("- 2% fee correctly applied to all flash loans");
    println!("- Fee calculation handles edge cases properly");
    println!("- Repayment verification working correctly");
    println!("- Overflow protection in place");
    println!("- Economic disincentive effective for <2% arbitrage");
}