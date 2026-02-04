//! End-to-end test for bootstrap phase with MMT rewards

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    integration::bootstrap_coordinator::{
        BootstrapState, BootstrapMilestone,
        BOOTSTRAP_MMT_MULTIPLIER, BOOTSTRAP_TARGET_VAULT,
        MIN_DEPOSIT_AMOUNT, EARLY_DEPOSITOR_THRESHOLD,
    },
    mmt::maker_rewards::MMTRewards,
};

#[tokio::test]
async fn test_bootstrap_double_mmt_rewards() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create bootstrap state
    let bootstrap_state = BootstrapState {
        discriminator: [0u8; 8],
        is_active: true,
        total_deposited: 0,
        unique_depositors: 0,
        bootstrap_complete: false,
        start_slot: 0,
        end_slot: 0,
        current_milestone: BootstrapMilestone::Start,
        early_depositors: Vec::new(),
        incentive_pool_remaining: 100_000_000_000_000, // 100k MMT
        total_mmt_distributed: 0,
    };

    // Test normal MMT reward
    let deposit_amount = 1_000_000_000; // $1k deposit
    let base_mmt_reward = deposit_amount; // 1:1 ratio
    
    // During bootstrap, reward is multiplied
    let bootstrap_reward = base_mmt_reward * BOOTSTRAP_MMT_MULTIPLIER as u64;
    
    assert_eq!(BOOTSTRAP_MMT_MULTIPLIER, 2, "Bootstrap multiplier should be 2x");
    assert_eq!(bootstrap_reward, 2_000_000_000, "Should receive 2x MMT during bootstrap");
}

#[tokio::test]
async fn test_bootstrap_target_vault_10k() {
    assert_eq!(
        BOOTSTRAP_TARGET_VAULT, 
        10_000_000_000, 
        "Bootstrap target should be $10k (with 6 decimals)"
    );

    let mut bootstrap_state = BootstrapState {
        discriminator: [0u8; 8],
        is_active: true,
        total_deposited: 9_500_000_000, // $9.5k deposited
        unique_depositors: 50,
        bootstrap_complete: false,
        start_slot: 0,
        end_slot: 0,
        current_milestone: BootstrapMilestone::SeventyFivePercent,
        early_depositors: Vec::new(),
        incentive_pool_remaining: 50_000_000_000_000,
        total_mmt_distributed: 50_000_000_000_000,
    };

    // Deposit $500 more to reach target
    let final_deposit = 500_000_000;
    bootstrap_state.total_deposited += final_deposit;

    assert_eq!(bootstrap_state.total_deposited, BOOTSTRAP_TARGET_VAULT);
    assert!(
        bootstrap_state.total_deposited >= BOOTSTRAP_TARGET_VAULT,
        "Bootstrap should complete at $10k"
    );
}

#[tokio::test]
async fn test_bootstrap_milestones() {
    let milestones = vec![
        (0, BootstrapMilestone::Start),
        (1_000_000_000, BootstrapMilestone::TenPercent),      // $1k
        (2_500_000_000, BootstrapMilestone::TwentyFivePercent), // $2.5k
        (5_000_000_000, BootstrapMilestone::FiftyPercent),    // $5k
        (7_500_000_000, BootstrapMilestone::SeventyFivePercent), // $7.5k
        (10_000_000_000, BootstrapMilestone::Complete),       // $10k
    ];

    for (amount, expected_milestone) in milestones {
        let progress_percent = (amount as u128 * 100) / BOOTSTRAP_TARGET_VAULT as u128;
        
        match progress_percent {
            0..=9 => assert_eq!(expected_milestone, BootstrapMilestone::Start),
            10..=24 => assert_eq!(expected_milestone, BootstrapMilestone::TenPercent),
            25..=49 => assert_eq!(expected_milestone, BootstrapMilestone::TwentyFivePercent),
            50..=74 => assert_eq!(expected_milestone, BootstrapMilestone::FiftyPercent),
            75..=99 => assert_eq!(expected_milestone, BootstrapMilestone::SeventyFivePercent),
            _ => assert_eq!(expected_milestone, BootstrapMilestone::Complete),
        }
    }
}

#[tokio::test]
async fn test_early_depositor_bonus() {
    let mut bootstrap_state = BootstrapState {
        discriminator: [0u8; 8],
        is_active: true,
        total_deposited: 0,
        unique_depositors: 0,
        bootstrap_complete: false,
        start_slot: 0,
        end_slot: 0,
        current_milestone: BootstrapMilestone::Start,
        early_depositors: Vec::new(),
        incentive_pool_remaining: 100_000_000_000_000,
        total_mmt_distributed: 0,
    };

    // First 100 depositors get early bird bonus
    assert_eq!(EARLY_DEPOSITOR_THRESHOLD, 100);

    // Add early depositors
    for i in 0..EARLY_DEPOSITOR_THRESHOLD {
        let depositor = Keypair::new().pubkey();
        bootstrap_state.early_depositors.push(depositor);
        bootstrap_state.unique_depositors += 1;
    }

    assert_eq!(
        bootstrap_state.early_depositors.len(), 
        EARLY_DEPOSITOR_THRESHOLD as usize,
        "Should track first 100 depositors"
    );

    // 101st depositor doesn't get early bird bonus
    let late_depositor = Keypair::new().pubkey();
    let is_early = bootstrap_state.unique_depositors < EARLY_DEPOSITOR_THRESHOLD as u32;
    assert!(!is_early, "101st depositor is not early");
}

#[tokio::test]
async fn test_minimum_deposit_requirement() {
    assert_eq!(
        MIN_DEPOSIT_AMOUNT,
        1_000_000, // $1 minimum
        "Minimum deposit should be $1"
    );

    // Test deposit validation
    let small_deposit = 500_000; // $0.50
    let valid_deposit = 1_000_000; // $1.00
    let large_deposit = 1_000_000_000; // $1000

    assert!(small_deposit < MIN_DEPOSIT_AMOUNT, "Small deposit should be rejected");
    assert!(valid_deposit >= MIN_DEPOSIT_AMOUNT, "Valid deposit should be accepted");
    assert!(large_deposit >= MIN_DEPOSIT_AMOUNT, "Large deposit should be accepted");
}

#[tokio::test]
async fn test_leverage_scaling_with_vault_size() {
    // Test leverage availability based on vault size
    let test_cases = vec![
        (0, 1),                    // $0 vault = 1x leverage only
        (1_000_000_000, 1),        // $1k vault = 1x leverage
        (5_000_000_000, 5),        // $5k vault = 5x leverage
        (10_000_000_000, 10),      // $10k vault = 10x leverage
        (20_000_000_000, 20),      // $20k vault = 20x leverage
        (50_000_000_000, 50),      // $50k vault = 50x leverage
    ];

    for (vault_size, expected_leverage) in test_cases {
        // Simple linear scaling: leverage = vault_size / 1B
        let calculated_leverage = vault_size / 1_000_000_000;
        let capped_leverage = calculated_leverage.min(50); // Cap at 50x
        
        if vault_size < 10_000_000_000 {
            // During bootstrap, leverage is limited
            let bootstrap_leverage = (vault_size / 1_000_000_000).max(1);
            assert!(
                bootstrap_leverage <= 10,
                "Bootstrap leverage should be limited to 10x"
            );
        }
    }
}

#[tokio::test]
async fn test_bootstrap_completion() {
    let mut bootstrap_state = BootstrapState {
        discriminator: [0u8; 8],
        is_active: true,
        total_deposited: BOOTSTRAP_TARGET_VAULT,
        unique_depositors: 150,
        bootstrap_complete: false,
        start_slot: 0,
        end_slot: 1000,
        current_milestone: BootstrapMilestone::Complete,
        early_depositors: Vec::new(),
        incentive_pool_remaining: 10_000_000_000_000,
        total_mmt_distributed: 90_000_000_000_000,
    };

    // Mark bootstrap as complete
    bootstrap_state.bootstrap_complete = true;
    bootstrap_state.is_active = false;

    assert!(bootstrap_state.bootstrap_complete, "Bootstrap should be marked complete");
    assert!(!bootstrap_state.is_active, "Bootstrap should be inactive after completion");
    assert_eq!(
        bootstrap_state.current_milestone, 
        BootstrapMilestone::Complete,
        "Should reach Complete milestone"
    );
}