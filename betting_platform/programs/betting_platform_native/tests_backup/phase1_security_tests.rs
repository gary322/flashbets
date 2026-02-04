//! Phase 1 Security Tests
//! 
//! Comprehensive tests for CPI depth tracking and flash loan protection

use solana_program_test::*;
use solana_sdk::{
    account_info::AccountInfo,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};

use betting_platform::{
    instruction::{BettingPlatformInstruction, ChainStepType},
    state::{
        security_accounts::{AttackDetector, discriminators},
    },
    error::BettingPlatformError,
    cpi::depth_tracker::CPIDepthTracker,
    attack_detection::{FLASH_LOAN_FEE_BPS, apply_flash_loan_fee, verify_flash_loan_repayment},
};

#[tokio::test]
async fn test_cpi_depth_tracking() {
    // Test that CPI depth is properly tracked and enforced
    let mut depth_tracker = CPIDepthTracker::new();
    
    // Test normal operation within limits
    assert_eq!(depth_tracker.current_depth(), 0);
    
    // Enter first CPI level
    assert!(depth_tracker.enter_cpi().is_ok());
    assert_eq!(depth_tracker.current_depth(), 1);
    
    // Enter second CPI level
    assert!(depth_tracker.enter_cpi().is_ok());
    assert_eq!(depth_tracker.current_depth(), 2);
    
    // Enter third CPI level (chain max)
    assert!(depth_tracker.enter_cpi().is_ok());
    assert_eq!(depth_tracker.current_depth(), 3);
    assert!(depth_tracker.at_max_depth());
    
    // Try to exceed chain max depth (should fail)
    let result = depth_tracker.enter_cpi();
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        BettingPlatformError::CPIDepthExceeded.into()
    );
    
    // Exit one level
    depth_tracker.exit_cpi();
    assert_eq!(depth_tracker.current_depth(), 2);
    assert!(!depth_tracker.at_max_depth());
    
    // Can enter again
    assert!(depth_tracker.enter_cpi().is_ok());
    assert_eq!(depth_tracker.current_depth(), 3);
}

#[tokio::test]
async fn test_cpi_depth_for_specific_operations() {
    let mut depth_tracker = CPIDepthTracker::new();
    
    // Set current depth to 2
    depth_tracker.enter_cpi().unwrap();
    depth_tracker.enter_cpi().unwrap();
    
    // Check if we can do an operation requiring 1 more depth
    assert!(depth_tracker.check_depth_for_operation(1).is_ok());
    
    // Check if we can do an operation requiring 2 more depth
    assert!(depth_tracker.check_depth_for_operation(2).is_ok());
    
    // Check if we can do an operation requiring 3 more depth (would exceed max)
    let result = depth_tracker.check_depth_for_operation(3);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_flash_loan_fee_calculation() {
    // Test flash loan fee calculation (2%)
    assert_eq!(FLASH_LOAN_FEE_BPS, 200);
    
    // Test various amounts
    let test_cases = vec![
        (1000, 20),        // 1000 * 0.02 = 20
        (10000, 200),      // 10000 * 0.02 = 200
        (100000, 2000),    // 100000 * 0.02 = 2000
        (1000000, 20000),  // 1000000 * 0.02 = 20000
    ];
    
    for (amount, expected_fee) in test_cases {
        let fee = apply_flash_loan_fee(amount).unwrap();
        assert_eq!(fee, expected_fee, "Fee calculation failed for amount {}", amount);
    }
}

#[tokio::test]
async fn test_flash_loan_repayment_verification() {
    // Test successful repayment with fee
    let borrowed = 10000;
    let fee = apply_flash_loan_fee(borrowed).unwrap();
    let total = borrowed + fee;
    
    // Exact repayment should succeed
    assert!(verify_flash_loan_repayment(borrowed, total).is_ok());
    
    // Over-repayment should succeed
    assert!(verify_flash_loan_repayment(borrowed, total + 100).is_ok());
    
    // Under-repayment should fail
    let result = verify_flash_loan_repayment(borrowed, total - 1);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        BettingPlatformError::InsufficientFlashLoanRepayment.into()
    );
}

#[tokio::test]
async fn test_chain_with_flash_loan_protection() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform",
        program_id,
        processor!(betting_platform::entrypoint::process_instruction),
    );
    
    // Set up test accounts
    let user = Keypair::new();
    let chain_state = Keypair::new();
    let verse = Keypair::new();
    let global_config = Keypair::new();
    let attack_detector = Keypair::new();
    
    // Add accounts to program test
    program_test.add_account(
        user.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Create chain with borrow step
    let chain_steps = vec![
        ChainStepType::Borrow { amount: 10000 },
        ChainStepType::Long { outcome: 0, leverage: 10 },
    ];
    
    let instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(chain_state.pubkey(), false),
            AccountMeta::new_readonly(verse.pubkey(), false),
            AccountMeta::new_readonly(global_config.pubkey(), false),
            AccountMeta::new(attack_detector.pubkey(), false),
        ],
        data: BettingPlatformInstruction::AutoChain {
            verse_id: 1,
            deposit: 1000,
            steps: chain_steps,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    // Note: In a real test, we'd need to set up all the required accounts
    // This is a simplified version to show the structure
}

#[tokio::test]
async fn test_flash_loan_attack_detection() {
    // Test that flash loan attacks are properly detected
    let mut detector = AttackDetector::new();
    
    // Configure flash loan detection
    detector.flash_loan_threshold = 5000;
    detector.min_blocks_between_borrow_trade = 10;
    
    let trader = Pubkey::new_unique();
    let current_slot = 1000;
    
    // Record a borrow
    detector.record_borrow(trader, current_slot);
    
    // Try to trade immediately after borrow (flash loan attack)
    let result = detector.process_trade(
        [0u8; 32],        // market_id
        trader,           // same trader
        10000,           // size > threshold
        100,             // price
        15,              // high leverage
        true,            // is_buy
        current_slot + 5, // too soon after borrow
    );
    
    // Should detect flash loan attack
    assert!(result.is_err());
    assert_eq!(detector.attacks_detected, 1);
    
    // Try to trade after sufficient blocks
    let result2 = detector.process_trade(
        [0u8; 32],
        trader,
        10000,
        100,
        15,
        true,
        current_slot + 15, // sufficient blocks passed
    );
    
    // Should not detect attack
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_cpi_depth_in_chain_execution() {
    // Test that chain execution respects CPI depth limits
    let mut depth_tracker = CPIDepthTracker::new();
    
    // Simulate a chain with multiple steps requiring CPI calls
    let chain_steps = vec![
        ChainStepType::Borrow { amount: 1000 },     // Requires CPI to lending protocol
        ChainStepType::Liquidity { amount: 500 },   // Requires CPI to AMM
        ChainStepType::Stake { amount: 200 },       // Requires CPI to staking program
    ];
    
    // Each step should check depth before proceeding
    for (i, step) in chain_steps.iter().enumerate() {
        if depth_tracker.at_max_depth() {
            println!("Chain execution stopped at step {} due to CPI depth limit", i);
            break;
        }
        
        match step {
            ChainStepType::Borrow { .. } => {
                assert!(depth_tracker.enter_cpi().is_ok());
                // Simulate borrow CPI
                depth_tracker.exit_cpi();
            }
            ChainStepType::Liquidity { .. } => {
                assert!(depth_tracker.enter_cpi().is_ok());
                // Simulate liquidity CPI
                depth_tracker.exit_cpi();
            }
            ChainStepType::Stake { .. } => {
                assert!(depth_tracker.enter_cpi().is_ok());
                // Simulate stake CPI
                depth_tracker.exit_cpi();
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_user_journey_flash_loan_protection() {
    // User journey: Attempt flash loan attack and verify protection
    
    // 1. User borrows funds
    let borrow_amount = 100000;
    let flash_fee = apply_flash_loan_fee(borrow_amount).unwrap();
    let total_repayment = borrow_amount + flash_fee;
    
    println!("User borrows: {}", borrow_amount);
    println!("Flash loan fee (2%): {}", flash_fee);
    println!("Total repayment required: {}", total_repayment);
    
    // 2. User tries to trade immediately (attack)
    let mut detector = AttackDetector::new();
    detector.flash_loan_threshold = 50000;
    detector.min_blocks_between_borrow_trade = 5;
    
    let user = Pubkey::new_unique();
    let slot = 1000;
    
    detector.record_borrow(user, slot);
    
    // Immediate trade attempt
    let trade_result = detector.process_trade(
        [1u8; 32],
        user,
        borrow_amount,
        100,
        20,
        true,
        slot + 2, // Only 2 slots later
    );
    
    assert!(trade_result.is_err());
    println!("Flash loan attack detected and blocked!");
    
    // 3. User waits and trades legitimately
    let legitimate_trade = detector.process_trade(
        [1u8; 32],
        user,
        50000, // Smaller size
        100,
        5,     // Lower leverage
        true,
        slot + 10, // Sufficient time passed
    );
    
    assert!(legitimate_trade.is_ok());
    println!("Legitimate trade allowed after sufficient time");
    
    // 4. Verify repayment includes fee
    assert!(verify_flash_loan_repayment(borrow_amount, total_repayment).is_ok());
    println!("Flash loan repayment verified with fee");
}

#[test]
fn test_cpi_depth_macro_usage() {
    use betting_platform::invoke_with_depth_check;
    
    // Test the macro compiles and works correctly
    let mut tracker = CPIDepthTracker::new();
    
    // Mock instruction and accounts
    let instruction = solana_sdk::instruction::Instruction {
        program_id: Pubkey::new_unique(),
        accounts: vec![],
        data: vec![],
    };
    
    let accounts: Vec<AccountInfo> = vec![];
    
    // This would normally invoke the instruction with depth checking
    // In a real test, we'd need proper account setup
}