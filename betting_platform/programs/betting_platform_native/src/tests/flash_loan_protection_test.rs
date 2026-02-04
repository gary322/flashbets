//! Comprehensive flash loan protection test
//!
//! Verifies specification compliance for flash loan protection:
//! - 2% fee mechanism
//! - Minimum 2 slot delay between borrow and trade
//! - Attack detection integration

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        attack_detection::{
            flash_loan_fee::{
                FLASH_LOAN_FEE_BPS,
                apply_flash_loan_fee,
                calculate_flash_loan_total,
                verify_flash_loan_repayment,
            },
        },
        state::security_accounts::{AttackDetector, AttackType},
        error::BettingPlatformError,
    };
    use solana_program::{
        pubkey::Pubkey,
        program_error::ProgramError,
    };

    #[test]
    fn test_flash_loan_fee_constant() {
        // Verify flash loan fee is 2% as per specification
        assert_eq!(FLASH_LOAN_FEE_BPS, 200, "Flash loan fee should be 2% (200 bps)");
    }

    #[test]
    fn test_flash_loan_fee_calculation() {
        // Test fee calculation
        let principal = 100_000_000; // 100 USDC
        let fee = apply_flash_loan_fee(principal).unwrap();
        assert_eq!(fee, 2_000_000, "Fee should be 2% of principal");
        
        // Test total calculation
        let total = calculate_flash_loan_total(principal).unwrap();
        assert_eq!(total, 102_000_000, "Total should be principal + 2% fee");
        
        // Test edge cases
        let small_amount = 100;
        let small_fee = apply_flash_loan_fee(small_amount).unwrap();
        assert_eq!(small_fee, 2, "Fee should round down for small amounts");
        
        // Test large amount
        let large_amount = 10_000_000_000; // 10k USDC
        let large_fee = apply_flash_loan_fee(large_amount).unwrap();
        assert_eq!(large_fee, 200_000_000, "Fee should be 2% for large amounts");
    }

    #[test]
    fn test_flash_loan_repayment_verification() {
        let borrowed = 1_000_000_000; // 1k USDC
        
        // Test exact repayment (should pass)
        let exact_repayment = calculate_flash_loan_total(borrowed).unwrap();
        assert!(
            verify_flash_loan_repayment(borrowed, exact_repayment).is_ok(),
            "Exact repayment should be accepted"
        );
        
        // Test overpayment (should pass)
        let overpayment = exact_repayment + 1000;
        assert!(
            verify_flash_loan_repayment(borrowed, overpayment).is_ok(),
            "Overpayment should be accepted"
        );
        
        // Test underpayment (should fail)
        let underpayment = exact_repayment - 1;
        let result = verify_flash_loan_repayment(borrowed, underpayment);
        assert!(result.is_err(), "Underpayment should be rejected");
        
        match result {
            Err(e) => {
                let betting_err: Result<BettingPlatformError, _> = e.try_into();
                assert!(
                    matches!(betting_err, Ok(BettingPlatformError::InsufficientFlashLoanRepayment)),
                    "Should return InsufficientFlashLoanRepayment error"
                );
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_flash_loan_attack_detection() {
        let mut detector = AttackDetector::new();
        let attacker = Pubkey::new_unique();
        let market_id = [1u8; 32];
        
        // Verify minimum blocks between borrow and trade
        assert_eq!(
            detector.min_blocks_between_borrow_trade, 2,
            "Minimum blocks should be 2 as per specification"
        );
        
        // Record a flash loan borrow at slot 1000
        detector.record_borrow(attacker, 1000);
        
        // Attempt trade at slot 1001 (1 slot later) - should fail
        let result = detector.process_trade(
            market_id,
            attacker,
            10_000_000_000, // 10k USDC
            5000,
            10,
            true,
            1001,
        );
        
        assert!(result.is_err(), "Trade 1 slot after borrow should fail");
        assert_eq!(detector.attacks_detected, 1, "Attack should be counted");
        
        // Attempt trade at slot 1002 (2 slots later) - should succeed
        let result = detector.process_trade(
            market_id,
            attacker,
            10_000_000_000,
            5000,
            10,
            true,
            1002,
        );
        
        assert!(result.is_ok(), "Trade 2 slots after borrow should succeed");
        assert_eq!(detector.attacks_detected, 1, "No new attack should be counted");
    }

    #[test]
    fn test_flash_loan_threshold() {
        let detector = AttackDetector::new();
        
        // Verify flash loan threshold
        assert_eq!(
            detector.flash_loan_threshold, 10_000_000_000,
            "Flash loan threshold should be 10k USDC"
        );
    }

    #[test]
    fn test_borrow_record_cleanup() {
        let mut detector = AttackDetector::new();
        let borrower1 = Pubkey::new_unique();
        let borrower2 = Pubkey::new_unique();
        
        // Record multiple borrows
        detector.record_borrow(borrower1, 100);
        detector.record_borrow(borrower2, 200);
        detector.record_borrow(borrower1, 300);
        
        assert_eq!(detector.recent_borrows.len(), 3, "Should have 3 borrow records");
        
        // Record new borrow that triggers cleanup (detection window = 150 slots)
        detector.record_borrow(borrower2, 251);
        
        // Only borrows within detection window should remain
        assert!(detector.recent_borrows.len() <= 3, "Old borrows should be cleaned up");
        assert!(
            detector.recent_borrows.iter().all(|(_, slot)| *slot >= 101),
            "Only recent borrows should remain"
        );
    }

    #[test]
    fn test_flash_loan_combined_with_high_leverage() {
        let mut detector = AttackDetector::new();
        let trader = Pubkey::new_unique();
        let market_id = [2u8; 32];
        
        // Suspicious pattern: large size + high leverage
        let result = detector.process_trade(
            market_id,
            trader,
            15_000_000_000, // 15k USDC (above threshold)
            5000,
            20, // High leverage
            true,
            1000,
        );
        
        // Should succeed but increase suspicious patterns
        assert!(result.is_ok(), "Trade should succeed but be flagged as suspicious");
        
        // Multiple suspicious trades should eventually trigger detection
        for i in 1..6 {
            let _ = detector.process_trade(
                market_id,
                trader,
                15_000_000_000,
                5000 + i * 10,
                20,
                true,
                1000 + i * 10,
            );
        }
        
        // After pattern threshold, should detect attack
        let result = detector.process_trade(
            market_id,
            trader,
            15_000_000_000,
            5100,
            20,
            true,
            1100,
        );
        
        assert!(result.is_err(), "Repeated suspicious patterns should trigger detection");
        assert!(detector.attacks_detected > 0, "Attacks should be detected");
    }

    #[test]
    fn test_flash_loan_protection_integration() {
        // This test verifies the complete flash loan protection flow:
        // 1. User borrows funds
        // 2. Attack detector records the borrow
        // 3. User attempts immediate trade (blocked)
        // 4. User waits minimum slots
        // 5. User trades successfully
        // 6. User repays loan with 2% fee
        
        let mut detector = AttackDetector::new();
        let user = Pubkey::new_unique();
        let borrow_amount = 5_000_000_000; // 5k USDC
        let borrow_slot = 1000;
        
        // Step 1: Record borrow
        detector.record_borrow(user, borrow_slot);
        
        // Step 2: Attempt immediate trade (should fail)
        let immediate_trade = detector.process_trade(
            [3u8; 32],
            user,
            borrow_amount,
            5000,
            5,
            true,
            borrow_slot,
        );
        assert!(immediate_trade.is_err(), "Immediate trade should be blocked");
        
        // Step 3: Wait and trade
        let trade_slot = borrow_slot + 2;
        let trade_result = detector.process_trade(
            [3u8; 32],
            user,
            borrow_amount,
            5000,
            5,
            true,
            trade_slot,
        );
        assert!(trade_result.is_ok(), "Trade after delay should succeed");
        
        // Step 4: Calculate repayment
        let repayment_required = calculate_flash_loan_total(borrow_amount).unwrap();
        assert_eq!(repayment_required, 5_100_000_000, "Should require principal + 2%");
        
        // Step 5: Verify repayment
        assert!(
            verify_flash_loan_repayment(borrow_amount, repayment_required).is_ok(),
            "Proper repayment should be accepted"
        );
    }
}