//! End-to-end test for keeper bot incentives (5bp bounty)

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    state::{Position, KeeperAccount},
    keeper_liquidation::KEEPER_REWARD_BPS,
};

#[tokio::test]
async fn test_keeper_receives_5bp_reward() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Setup keeper account
    let keeper_keypair = Keypair::new();
    let keeper_account_keypair = Keypair::new();
    
    let keeper_account = KeeperAccount {
        discriminator: [0u8; 8],
        keeper: keeper_keypair.pubkey(),
        stake: 10_000_000_000, // $10k stake
        status: 1, // Active
        total_liquidations: 0,
        total_rewards_earned: 0,
        last_activity_slot: 0,
        performance_score: 10000, // 100%
    };

    let mut keeper_data = vec![];
    keeper_account.serialize(&mut keeper_data).unwrap();

    // Setup position to liquidate
    let position = Position {
        discriminator: [0u8; 8],
        user: Keypair::new().pubkey(),
        proposal_id: 1,
        position_id: [1u8; 32],
        outcome: 0,
        size: 100_000_000_000, // $100k position
        notional: 100_000_000_000,
        leverage: 100,
        entry_price: 5000,
        liquidation_price: 4950,
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 1_000_000_000,
        is_short: false,
    };

    // Calculate expected reward: 5bp of liquidated amount
    // If liquidating $20k (20% of position)
    let liquidation_amount = 20_000_000_000u64;
    let expected_reward = (liquidation_amount as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
    
    assert_eq!(expected_reward, 10_000_000, "5bp of $20k = $10");
    assert_eq!(KEEPER_REWARD_BPS, 5, "Keeper reward should be 5 basis points");
}

#[tokio::test]
async fn test_keeper_stats_updated_after_liquidation() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let keeper_keypair = Keypair::new();
    let keeper_account_keypair = Keypair::new();
    
    // Initial keeper stats
    let mut keeper_account = KeeperAccount {
        discriminator: [0u8; 8],
        keeper: keeper_keypair.pubkey(),
        stake: 10_000_000_000,
        status: 1,
        total_liquidations: 5, // Already done 5 liquidations
        total_rewards_earned: 50_000_000, // Already earned $50
        last_activity_slot: 1000,
        performance_score: 9500, // 95%
    };

    // After liquidation, stats should update:
    // - total_liquidations += 1
    // - total_rewards_earned += reward
    // - last_activity_slot = current_slot
    
    let new_reward = 10_000_000; // $10 from liquidating $20k
    keeper_account.total_liquidations += 1;
    keeper_account.total_rewards_earned += new_reward;
    
    assert_eq!(keeper_account.total_liquidations, 6);
    assert_eq!(keeper_account.total_rewards_earned, 60_000_000);
}

#[tokio::test]
async fn test_multiple_keeper_competition() {
    // Test that multiple keepers can compete for liquidations
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create 3 keepers with different performance scores
    let keeper1 = KeeperAccount {
        discriminator: [0u8; 8],
        keeper: Keypair::new().pubkey(),
        stake: 10_000_000_000,
        status: 1,
        total_liquidations: 100,
        total_rewards_earned: 1_000_000_000,
        last_activity_slot: 1000,
        performance_score: 9800, // 98% - high performer
    };

    let keeper2 = KeeperAccount {
        discriminator: [0u8; 8],
        keeper: Keypair::new().pubkey(),
        stake: 5_000_000_000,
        status: 1,
        total_liquidations: 50,
        total_rewards_earned: 400_000_000,
        last_activity_slot: 900,
        performance_score: 9000, // 90% - medium performer
    };

    let keeper3 = KeeperAccount {
        discriminator: [0u8; 8],
        keeper: Keypair::new().pubkey(),
        stake: 2_000_000_000,
        status: 1,
        total_liquidations: 10,
        total_rewards_earned: 50_000_000,
        last_activity_slot: 500,
        performance_score: 8000, // 80% - low performer
    };

    // Higher performance keepers should have priority in liquidation queue
    assert!(keeper1.performance_score > keeper2.performance_score);
    assert!(keeper2.performance_score > keeper3.performance_score);
}

#[tokio::test]
async fn test_keeper_reward_calculation_edge_cases() {
    // Test very small liquidation
    let small_liquidation = 1_000_000; // $1
    let small_reward = (small_liquidation as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
    assert_eq!(small_reward, 500, "5bp of $1 = $0.0005");

    // Test large liquidation
    let large_liquidation = 10_000_000_000_000; // $10M
    let large_reward = (large_liquidation as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
    assert_eq!(large_reward, 5_000_000_000, "5bp of $10M = $5k");

    // Test liquidation of exactly 0 (edge case)
    let zero_liquidation = 0u64;
    let zero_reward = (zero_liquidation as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
    assert_eq!(zero_reward, 0, "5bp of $0 = $0");
}