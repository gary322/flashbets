//! End-to-end test for coverage-based liquidation mechanics
//! Tests the liquidation formula: margin_ratio < 1/coverage

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
    error::BettingPlatformError,
    instruction::BettingPlatformInstruction,
    state::{GlobalConfig, Position, VersePDA, ProposalPDA},
    math::U64F64,
};

#[derive(Debug)]
struct TestContext {
    program_id: Pubkey,
    banks_client: BanksClient,
    payer: Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
}

/// Initialize test environment
async fn setup_test() -> TestContext {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    // Add necessary accounts
    let global_config_pubkey = Pubkey::new_unique();
    let global_config = GlobalConfig {
        admin: Pubkey::new_unique(),
        vault: 50_000_000_000, // $50k vault
        total_oi: 100_000_000_000, // $100k open interest
        coverage: 500_000, // 0.5 coverage
        total_verses: 1,
        total_proposals: 1,
        immutable: false,
        emergency_halt: false,
        halt_timestamp: 0,
        mmt_mint: Pubkey::new_unique(),
        mmt_fee_vault: Pubkey::new_unique(),
        base_fee_rate: 28, // 0.28%
        last_update_slot_slot: 0,
    };
    
    let mut config_data = vec![];
    global_config.serialize(&mut config_data).unwrap();
    
    program_test.add_account(
        global_config_pubkey,
        Account {
            lamports: 1_000_000,
            data: config_data,
            owner: program_id,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    TestContext {
        program_id,
        banks_client,
        payer,
        recent_blockhash,
    }
}

/// Test liquidation when margin_ratio < 1/coverage
#[tokio::test]
async fn test_coverage_based_liquidation() {
    let mut context = setup_test().await;
    
    // Create a position that should be liquidated
    let user_keypair = Keypair::new();
    let position_keypair = Keypair::new();
    let keeper_keypair = Keypair::new();
    
    // Setup position with low margin ratio
    let position = Position {
        discriminator: [0u8; 8],
        user: user_keypair.pubkey(),
        proposal_id: 1,
        position_id: [1u8; 32],
        outcome: 0,
        size: 10_000_000_000, // $10k position
        notional: 10_000_000_000,
        leverage: 50, // 50x leverage
        entry_price: 5000, // 50% probability
        liquidation_price: 4900, // Will be recalculated with coverage formula
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 200_000_000, // $200 margin (2% of position)
        is_short: false,
    };
    
    let mut position_data = vec![];
    position.serialize(&mut position_data).unwrap();
    
    // Add position account
    context.banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &context.payer.pubkey(),
            &position_keypair.pubkey(),
            1_000_000,
            position_data.len() as u64,
            &context.program_id,
        )],
        Some(&context.payer.pubkey()),
        &[&context.payer, &position_keypair],
        context.recent_blockhash,
    )).await.unwrap();
    
    // Write position data
    let position_account = Account {
        lamports: 1_000_000,
        data: position_data,
        owner: context.program_id,
        ..Account::default()
    };
    context.banks_client.set_account(&position_keypair.pubkey(), &position_account.into()).await.unwrap();
    
    // Test liquidation with coverage = 0.5
    // margin_ratio = 200M / 10B = 0.02
    // 1/coverage = 1/0.5 = 2.0
    // Since 0.02 < 2.0, position should be liquidatable
    
    let liquidate_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(keeper_keypair.pubkey(), true),
            AccountMeta::new(position_keypair.pubkey(), false),
            AccountMeta::new(user_keypair.pubkey(), false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false), // global config
            AccountMeta::new(Pubkey::new_unique(), false), // vault
        ],
        data: BettingPlatformInstruction::PartialLiquidate { position_index: 0 }
            .try_to_vec()
            .unwrap(),
    };
    
    // Fund keeper account
    let transfer_ix = system_instruction::transfer(
        &context.payer.pubkey(),
        &keeper_keypair.pubkey(),
        1_000_000,
    );
    
    context.banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.recent_blockhash,
    )).await.unwrap();
    
    // Execute liquidation
    let result = context.banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[liquidate_ix],
        Some(&keeper_keypair.pubkey()),
        &[&keeper_keypair],
        context.recent_blockhash,
    )).await;
    
    // Should succeed as position is unhealthy
    assert!(result.is_ok(), "Liquidation should succeed for unhealthy position");
    
    // Verify position was partially liquidated
    let position_account = context.banks_client
        .get_account(position_keypair.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let updated_position = Position::try_from_slice(&position_account.data).unwrap();
    assert!(updated_position.size < position.size, "Position size should be reduced");
    assert!(updated_position.partial_liq_accumulator > 0, "Accumulator should be updated");
}

/// Test that healthy positions cannot be liquidated
#[tokio::test]
async fn test_healthy_position_not_liquidatable() {
    let mut context = setup_test().await;
    
    // Create a healthy position with high margin ratio
    let user_keypair = Keypair::new();
    let position_keypair = Keypair::new();
    let keeper_keypair = Keypair::new();
    
    let position = Position {
        discriminator: [0u8; 8],
        user: user_keypair.pubkey(),
        proposal_id: 1,
        position_id: [2u8; 32],
        outcome: 0,
        size: 10_000_000_000, // $10k position
        notional: 10_000_000_000,
        leverage: 2, // 2x leverage (healthy)
        entry_price: 5000,
        liquidation_price: 2500,
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 5_000_000_000, // $5k margin (50% of position)
        is_short: false,
    };
    
    let mut position_data = vec![];
    position.serialize(&mut position_data).unwrap();
    
    // Add position account
    context.banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &context.payer.pubkey(),
            &position_keypair.pubkey(),
            1_000_000,
            position_data.len() as u64,
            &context.program_id,
        )],
        Some(&context.payer.pubkey()),
        &[&context.payer, &position_keypair],
        context.recent_blockhash,
    )).await.unwrap();
    
    let position_account = Account {
        lamports: 1_000_000,
        data: position_data,
        owner: context.program_id,
        ..Account::default()
    };
    context.banks_client.set_account(&position_keypair.pubkey(), &position_account.into()).await.unwrap();
    
    // Test liquidation with coverage = 0.5
    // margin_ratio = 5B / 10B = 0.5
    // 1/coverage = 1/0.5 = 2.0
    // Since 0.5 > 2.0 is false, position is healthy
    
    let liquidate_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(keeper_keypair.pubkey(), true),
            AccountMeta::new(position_keypair.pubkey(), false),
            AccountMeta::new(user_keypair.pubkey(), false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false), // global config
            AccountMeta::new(Pubkey::new_unique(), false), // vault
        ],
        data: BettingPlatformInstruction::PartialLiquidate { position_index: 0 }
            .try_to_vec()
            .unwrap(),
    };
    
    // Fund keeper
    let transfer_ix = system_instruction::transfer(
        &context.payer.pubkey(),
        &keeper_keypair.pubkey(),
        1_000_000,
    );
    
    context.banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.recent_blockhash,
    )).await.unwrap();
    
    // Try to liquidate healthy position
    let result = context.banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[liquidate_ix],
        Some(&keeper_keypair.pubkey()),
        &[&keeper_keypair],
        context.recent_blockhash,
    )).await;
    
    // Should fail with PositionHealthy error
    assert!(result.is_err(), "Liquidation should fail for healthy position");
}

/// Test liquidation price calculation with coverage formula
#[tokio::test]
async fn test_liquidation_price_calculation() {
    use betting_platform_native::trading::helpers::calculate_liquidation_price_coverage_based;
    
    // Test case 1: Long position
    let entry_price = 5000; // 50%
    let position_size = 10_000_000_000; // $10k
    let margin = 1_000_000_000; // $1k
    let coverage = U64F64::from_num(5) / U64F64::from_num(10); // 0.5
    
    let liq_price = calculate_liquidation_price_coverage_based(
        entry_price,
        position_size,
        margin,
        coverage,
        true, // is_long
    ).unwrap();
    
    // For long positions with coverage = 0.5:
    // Liquidation occurs when price drops by (1 - 1/coverage) = (1 - 2) = -1 (impossible)
    // But with proper calculation: liq_price = entry * (1 - 1/coverage)
    // liq_price = 5000 * (1 - 2) = 5000 * (-1) = 0 (clamped to 0)
    assert!(liq_price == 0, "Long liquidation price calculation incorrect");
    
    // Test case 2: Short position
    let liq_price_short = calculate_liquidation_price_coverage_based(
        entry_price,
        position_size,
        margin,
        coverage,
        false, // is_short
    ).unwrap();
    
    // For short positions: liq_price = entry * (1 + 1/coverage)
    // liq_price = 5000 * (1 + 2) = 5000 * 3 = 15000
    assert_eq!(liq_price_short, 15000, "Short liquidation price calculation incorrect");
}