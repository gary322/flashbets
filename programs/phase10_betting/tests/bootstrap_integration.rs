#![cfg(feature = "test-sbf")]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_spl::token::{self, Token, TokenAccount, Mint};
use anchor_spl::associated_token::AssociatedToken;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use phase10_betting::*;

#[tokio::test]
async fn test_bootstrap_initialization() {
    let program_id = phase10_betting::id();
    let mut program_test = ProgramTest::new(
        "phase10_betting",
        program_id,
        processor!(phase10_betting::entry),
    );

    // Add accounts
    let admin = Keypair::new();
    program_test.add_account(
        admin.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut context = program_test.start_with_context().await;

    // Derive PDAs
    let (bootstrap_state, _) = Pubkey::find_program_address(
        &[b"bootstrap_state"],
        &program_id,
    );

    let (global_state, _) = Pubkey::find_program_address(
        &[b"global_state"],
        &program_id,
    );

    // Initialize global state first
    let init_global_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(global_state, false),
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: phase10_betting::instruction::InitializeGlobalState {}.data(),
    };

    let init_global_tx = Transaction::new_signed_with_payer(
        &[init_global_ix],
        Some(&admin.pubkey()),
        &[&admin],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(init_global_tx).await.unwrap();

    // Initialize bootstrap
    let init_bootstrap_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_state, false),
            AccountMeta::new(global_state, false),
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(solana_program::clock::id(), false),
        ],
        data: phase10_betting::instruction::InitializeBootstrap {}.data(),
    };

    let init_bootstrap_tx = Transaction::new_signed_with_payer(
        &[init_bootstrap_ix],
        Some(&admin.pubkey()),
        &[&admin],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(init_bootstrap_tx).await.unwrap();

    // Verify bootstrap state
    let bootstrap_account = context.banks_client
        .get_account(bootstrap_state)
        .await
        .unwrap()
        .unwrap();

    let bootstrap_data = bootstrap_account.data;
    assert!(bootstrap_data.len() > 8); // Has discriminator + data

    println!("Bootstrap initialized successfully!");
}

#[tokio::test]
async fn test_bootstrap_trader_registration() {
    let program_id = phase10_betting::id();
    let mut program_test = ProgramTest::new(
        "phase10_betting",
        program_id,
        processor!(phase10_betting::entry),
    );

    // Setup
    let admin = Keypair::new();
    let trader = Keypair::new();
    
    program_test.add_account(
        admin.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    
    program_test.add_account(
        trader.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut context = program_test.start_with_context().await;

    // Initialize bootstrap first (similar to above)
    // ... (bootstrap initialization code)

    // Register trader
    let (trader_state, _) = Pubkey::find_program_address(
        &[b"bootstrap_trader", trader.pubkey().as_ref()],
        &program_id,
    );

    let (bootstrap_state, _) = Pubkey::find_program_address(
        &[b"bootstrap_state"],
        &program_id,
    );

    let register_trader_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(trader_state, false),
            AccountMeta::new(bootstrap_state, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: phase10_betting::instruction::RegisterBootstrapTrader {}.data(),
    };

    let register_trader_tx = Transaction::new_signed_with_payer(
        &[register_trader_ix],
        Some(&trader.pubkey()),
        &[&trader],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(register_trader_tx).await.unwrap();

    // Verify trader state
    let trader_account = context.banks_client
        .get_account(trader_state)
        .await
        .unwrap()
        .unwrap();

    assert!(trader_account.data.len() > 8);
    println!("Trader registered successfully!");
}

#[tokio::test]
async fn test_bootstrap_fee_calculation() {
    // Unit test for fee calculation
    use phase10_betting::state::BootstrapState;
    use phase10_betting::types::U64F64;

    let mut bootstrap_state = BootstrapState {
        current_coverage: U64F64::zero(),
        ..Default::default()
    };

    // Test at 0% coverage
    let fee = bootstrap_state.calculate_bootstrap_fee();
    assert_eq!(fee, 28); // Max fee

    // Test at 50% coverage
    bootstrap_state.current_coverage = U64F64::from_num(1u32) / U64F64::from_num(2u32);
    let fee = bootstrap_state.calculate_bootstrap_fee();
    assert!(fee > 3 && fee < 28);

    // Test at 100% coverage
    bootstrap_state.current_coverage = U64F64::one();
    let fee = bootstrap_state.calculate_bootstrap_fee();
    assert_eq!(fee, 3); // Min fee

    println!("Bootstrap fee calculation tests passed!");
}

#[tokio::test]
async fn test_early_trader_rewards() {
    // Unit test for reward calculation
    use phase10_betting::state::{BootstrapState, IncentiveTier};
    use phase10_betting::types::U64F64;

    let bootstrap_state = BootstrapState {
        early_bonus_multiplier: U64F64::from_num(2u32),
        ..Default::default()
    };

    let tier = IncentiveTier {
        min_volume: 0,
        reward_multiplier: U64F64::from_num(3u32) / U64F64::from_num(2u32), // 1.5x
        fee_rebate_bps: 5,
        liquidation_priority: 3,
        advanced_features: false,
    };

    let trade_volume = 1000 * 10u64.pow(6); // $1000

    // Early trader gets 2x * 1.5x = 3x rewards
    let reward = bootstrap_state.calculate_mmt_reward(trade_volume, true, &tier);
    let expected = (trade_volume * 100 / 10_000) * 3; // 1% base * 3x
    assert_eq!(reward, expected);

    // Regular trader gets 1.5x rewards
    let reward = bootstrap_state.calculate_mmt_reward(trade_volume, false, &tier);
    let expected = ((trade_volume * 100 / 10_000) as f64 * 1.5) as u64;
    assert_eq!(reward, expected);

    println!("Early trader reward tests passed!");
}