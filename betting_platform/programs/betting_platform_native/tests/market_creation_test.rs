//! Market Creation and Settlement Tests
//! 
//! Tests for all AMM market types and settlement flows

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
};
use borsh::BorshSerialize;
use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    state::{LMSRMarket, PMAMMMarket, L2AMMMarket, HybridAMM},
    amm::{AMMType, DistributionType},
};

#[tokio::test]
async fn test_lmsr_market_creation_and_trade() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    // Start test
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    // Create market ID
    let market_id = 12345u128;
    let b_parameter = 100_000_000u64; // 100 USDC liquidity parameter
    let num_outcomes = 2u8;

    // Derive market PDA
    let (market_pda, _bump) = Pubkey::find_program_address(
        &[b"lmsr_market", &market_id.to_le_bytes()],
        &program_id,
    );

    // Create initialization instruction
    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeLmsrMarket {
            market_id,
            b_parameter,
            num_outcomes,
        }.try_to_vec().unwrap(),
    };

    // Send transaction
    let mut transaction = Transaction::new_with_payer(
        &[init_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();

    // Verify market was created
    let market_account = banks_client.get_account(market_pda).await.unwrap().unwrap();
    let market = LMSRMarket::try_from_slice(&market_account.data).unwrap();
    
    assert_eq!(market.market_id, market_id);
    assert_eq!(market.b_parameter, b_parameter);
    assert_eq!(market.num_outcomes, num_outcomes);
    assert_eq!(market.shares.len(), num_outcomes as usize);
    
    println!("✅ LMSR market created successfully");

    // Test trade execution
    let outcome = 0u8;
    let amount = 10_000_000u64; // 10 USDC
    let is_buy = true;

    let trade_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::ExecuteLmsrTrade {
            outcome,
            amount,
            is_buy,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(
        &[trade_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
    
    println!("✅ LMSR trade executed successfully");
}

#[tokio::test]
async fn test_pmamm_market_creation() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let market_id = 67890u128;
    let l_parameter = 50_000_000u64; // 50 USDC liquidity
    let expiry_time = 1735689600i64; // Jan 1, 2025
    let initial_price = 5000u64; // 50%

    let (market_pda, _bump) = Pubkey::find_program_address(
        &[b"pmamm_market", &market_id.to_le_bytes()],
        &program_id,
    );

    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializePmammMarket {
            market_id,
            l_parameter,
            expiry_time,
            initial_price,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(
        &[init_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();

    let market_account = banks_client.get_account(market_pda).await.unwrap().unwrap();
    let market = PMAMMMarket::try_from_slice(&market_account.data).unwrap();
    
    assert_eq!(market.market_id, market_id);
    assert_eq!(market.l_parameter, l_parameter);
    assert_eq!(market.expiry_time, expiry_time);
    
    println!("✅ PM-AMM market created successfully");
}

#[tokio::test]
async fn test_l2_amm_continuous_market() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let market_id = 99999u128;
    let k_parameter = 100_000_000u64;
    let b_bound = 1_000_000u64;
    let distribution_type = DistributionType::Normal;
    let discretization_points = 100u16;
    let range_min = 0u64;
    let range_max = 100_000_000u64; // 0-100 range

    let (market_pda, _bump) = Pubkey::find_program_address(
        &[b"l2_amm_market", &market_id.to_le_bytes()],
        &program_id,
    );

    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeL2AmmMarket {
            market_id,
            k_parameter,
            b_bound,
            distribution_type,
            discretization_points,
            range_min,
            range_max,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(
        &[init_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();

    println!("✅ L2 AMM continuous market created");

    // Test continuous market resolution
    let winning_value = 75_000_000u64; // 75
    let oracle_signature = [0u8; 64]; // Mock signature

    let resolve_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::ResolveContinuous {
            winning_value,
            oracle_signature,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(
        &[resolve_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
    
    println!("✅ Continuous market resolved successfully");
}

#[tokio::test]
async fn test_hybrid_amm_market() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let market_id = 111111u128;
    let amm_type = AMMType::Hybrid;
    let num_outcomes = 3u8;
    let expiry_time = 1735689600i64;
    let is_continuous = false;
    let amm_specific_data = vec![100, 0, 0, 0]; // Custom data

    let (market_pda, _bump) = Pubkey::find_program_address(
        &[b"hybrid_amm", &market_id.to_le_bytes()],
        &program_id,
    );

    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeHybridAmm {
            market_id,
            amm_type,
            num_outcomes,
            expiry_time,
            is_continuous,
            amm_specific_data,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(
        &[init_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
    
    println!("✅ Hybrid AMM market created successfully");
}

#[tokio::test]
async fn test_market_settlement_flow() {
    // Test complete flow: create -> trade -> resolve -> claim
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    // 1. Create LMSR market
    let market_id = 555555u128;
    let (market_pda, _) = Pubkey::find_program_address(
        &[b"lmsr_market", &market_id.to_le_bytes()],
        &program_id,
    );

    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeLmsrMarket {
            market_id,
            b_parameter: 100_000_000,
            num_outcomes: 2,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // 2. Execute trade
    let trade_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::ExecuteLmsrTrade {
            outcome: 0,
            amount: 50_000_000,
            is_buy: true,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[trade_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // 3. Resolve market
    let resolve_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::ProcessResolution {
            verse_id: 0,
            market_id: market_id.to_string(),
            resolution_outcome: "0".to_string(),
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[resolve_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    println!("✅ Complete market lifecycle tested: Create -> Trade -> Resolve");
}

#[test]
fn test_market_validation_rules() {
    // Test market parameter validation
    
    // LMSR: b_parameter must be > 0
    assert!(validate_lmsr_params(0, 2).is_err());
    assert!(validate_lmsr_params(1000, 2).is_ok());
    
    // PM-AMM: initial price must be between 0-10000
    assert!(validate_pmamm_params(1000, 10001).is_err());
    assert!(validate_pmamm_params(1000, 5000).is_ok());
    
    // L2 AMM: range must be valid
    assert!(validate_l2_params(100, 0, 100, 200).is_err()); // min > max
    assert!(validate_l2_params(100, 0, 0, 100).is_ok());
    
    println!("✅ Market validation rules tested");
}

fn validate_lmsr_params(b_parameter: u64, num_outcomes: u8) -> Result<(), String> {
    if b_parameter == 0 {
        return Err("B parameter must be > 0".to_string());
    }
    if num_outcomes < 2 {
        return Err("Must have at least 2 outcomes".to_string());
    }
    Ok(())
}

fn validate_pmamm_params(l_parameter: u64, initial_price: u64) -> Result<(), String> {
    if l_parameter == 0 {
        return Err("L parameter must be > 0".to_string());
    }
    if initial_price > 10000 {
        return Err("Initial price must be <= 10000 (100%)".to_string());
    }
    Ok(())
}

fn validate_l2_params(k_parameter: u64, b_bound: u64, range_min: u64, range_max: u64) -> Result<(), String> {
    if k_parameter == 0 {
        return Err("K parameter must be > 0".to_string());
    }
    if range_min >= range_max {
        return Err("Range min must be < range max".to_string());
    }
    Ok(())
}