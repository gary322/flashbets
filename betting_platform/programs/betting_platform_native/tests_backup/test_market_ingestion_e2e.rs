//! End-to-end tests for market ingestion from Polymarket
//! Tests all production scenarios including failures, disputes, and recovery

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::str::FromStr;

use betting_platform_native::{
    entrypoint::process_instruction,
    error::BettingPlatformError,
    market_ingestion::{
        MarketIngestionState, PolymarketMarketData, HaltReason,
        INGESTION_INTERVAL_SLOTS, MAX_FAILURE_SLOTS, BATCH_SIZE,
    },
    state::GlobalConfigPDA,
};

/// Create program test environment
fn create_test_env() -> ProgramTest {
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(process_instruction),
    );
    
    // Set initial clock
    program_test.set_compute_max_units(1_400_000);
    
    program_test
}

/// Initialize market ingestion system
async fn initialize_ingestion(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    let ingestion_state = Keypair::new();
    let global_config = Keypair::new();
    
    // Create ingestion state account
    let rent = banks_client.get_rent().await?;
    let space = MarketIngestionState::SIZE;
    let lamports = rent.minimum_balance(space);
    
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &ingestion_state.pubkey(),
        lamports,
        space as u64,
        &betting_platform_native::id(),
    );
    
    // Initialize global config
    let mut global_config_data = GlobalConfigPDA::default();
    global_config_data.update_authority = payer.pubkey();
    
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&payer.pubkey()),
        &[payer, &ingestion_state],
        recent_blockhash,
    )).await?;
    
    // Initialize ingestion state
    let mut state = MarketIngestionState::new(payer.pubkey());
    state.serialize(&mut banks_client.get_account(ingestion_state.pubkey()).await?.unwrap().data)?;
    
    Ok(ingestion_state.pubkey())
}

/// Create realistic market data batch
fn create_market_batch(start_id: u32, count: u32) -> Vec<PolymarketMarketData> {
    let mut markets = Vec::new();
    
    for i in 0..count {
        let id = start_id + i;
        
        // Vary market characteristics realistically
        let yes_price = match id % 10 {
            0..=2 => 7500 + (id % 1000),     // High confidence yes
            3..=5 => 2500 + (id % 1000),     // High confidence no
            6..=7 => 4500 + (id % 500),      // Uncertain
            8 => 5000,                        // Perfect 50/50
            _ => 6000 + (id % 1000),         // Slight yes bias
        };
        
        let no_price = 10000 - yes_price; // Ensure sum = 10000
        
        // Vary titles to test verse classification
        let title = match id % 20 {
            0..=4 => format!("Will BTC reach ${}k by end of year?", 100 + id % 50),
            5..=9 => format!("Will ETH price exceed ${} by December?", 5000 + id * 100),
            10..=12 => format!("US Election 2024: Will {} win?", if id % 2 == 0 { "Candidate A" } else { "Candidate B" }),
            13..=15 => format!("Will {} win the championship?", if id % 3 == 0 { "Team A" } else if id % 3 == 1 { "Team B" } else { "Team C" }),
            16..=17 => format!("Will inflation be above {}% in Q4?", 2 + id % 3),
            _ => format!("Generic market {} outcome", id),
        };
        
        markets.push(PolymarketMarketData {
            id: format!("0x{:064x}", id),
            title,
            description: format!("Market {} description with detailed rules", id),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price,
            no_price,
            volume_24h: 100_000 * (id as u64 % 100 + 1),
            liquidity: 50_000 * (id as u64 % 50 + 1),
            resolved: false,
            resolution: None,
            disputed: id % 100 == 99, // 1% dispute rate
            dispute_reason: if id % 100 == 99 {
                Some("Unclear resolution criteria".to_string())
            } else {
                None
            },
        });
    }
    
    markets
}

#[tokio::test]
async fn test_market_ingestion_happy_path() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Initialize ingestion
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Create market batch
    let markets = create_market_batch(0, 100);
    let batch = MarketBatch { markets };
    let batch_data = batch.try_to_vec().unwrap();
    
    // Process first batch
    let process_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false), // Global config
        ],
        data: [vec![0x01], batch_data].concat(), // 0x01 = process ingestion
    };
    
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("Failed to process market batch");
    
    // Verify state update
    let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    
    assert_eq!(state.total_markets_ingested, 99); // 100 - 1 disputed
    assert_eq!(state.consecutive_failures, 0);
    assert!(!state.is_halted);
}

#[tokio::test]
async fn test_ingestion_interval_enforcement() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Process first batch
    let markets = create_market_batch(0, 10);
    let batch = MarketBatch { markets };
    let batch_data = batch.try_to_vec().unwrap();
    
    let process_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data: [vec![0x01], batch_data.clone()].concat(),
    };
    
    // First ingestion should succeed
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix.clone()],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("First ingestion should succeed");
    
    // Immediate retry should fail (TooEarly)
    let result = banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix.clone()],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await;
    
    assert!(result.is_err());
    
    // Advance clock by INGESTION_INTERVAL_SLOTS
    let mut clock = banks_client.get_sysvar::<Clock>().await.unwrap();
    clock.slot += INGESTION_INTERVAL_SLOTS;
    program_test.context.set_sysvar(&clock);
    
    // Now it should succeed
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("Second ingestion after interval should succeed");
}

#[tokio::test]
async fn test_failure_handling_and_halt() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Create invalid batch data to trigger failures
    let invalid_data = vec![0xFF; 100]; // Invalid borsh data
    
    let process_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data: [vec![0x01], invalid_data].concat(),
    };
    
    // Process multiple failures
    let mut clock = banks_client.get_sysvar::<Clock>().await.unwrap();
    let start_slot = clock.slot;
    
    for i in 0..5 {
        // Try to process invalid data
        let _ = banks_client.process_transaction(Transaction::new_signed_with_payer(
            &[process_ix.clone()],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        )).await;
        
        // Check state
        let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
        let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
        
        assert_eq!(state.consecutive_failures, i + 1);
        
        // Advance clock
        clock.slot += INGESTION_INTERVAL_SLOTS;
        program_test.context.set_sysvar(&clock);
    }
    
    // Advance past MAX_FAILURE_SLOTS
    clock.slot = start_slot + MAX_FAILURE_SLOTS + 1;
    program_test.context.set_sysvar(&clock);
    
    // Next attempt should halt the system
    let _ = banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await;
    
    // Verify system is halted
    let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    
    assert!(state.is_halted);
    assert_eq!(state.halt_reason, HaltReason::ExtendedFailure);
}

#[tokio::test]
async fn test_dispute_handling() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Create batch with disputed markets
    let mut markets = create_market_batch(0, 10);
    markets[2].disputed = true;
    markets[2].dispute_reason = Some("Resolution criteria unclear".to_string());
    markets[5].disputed = true;
    markets[5].dispute_reason = Some("Oracle malfunction".to_string());
    
    let batch = MarketBatch { markets };
    let batch_data = batch.try_to_vec().unwrap();
    
    let process_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data: [vec![0x01], batch_data].concat(),
    };
    
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("Should handle disputed markets");
    
    // Verify state - should process 8 markets, skip 2 disputed
    let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    
    assert_eq!(state.total_markets_ingested, 8);
}

#[tokio::test]
async fn test_admin_resume_after_halt() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Force system into halted state
    let mut state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let mut state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    state.is_halted = true;
    state.halt_reason = HaltReason::ExtendedFailure;
    state.serialize(&mut state_account.data)?;
    banks_client.set_account(&ingestion_state_pubkey, &state_account);
    
    // Try to resume as admin
    let resume_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true), // Admin
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false), // Global config
        ],
        data: vec![0x02], // 0x02 = resume ingestion
    };
    
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[resume_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("Admin should be able to resume");
    
    // Verify resumed
    let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    
    assert!(!state.is_halted);
    assert_eq!(state.halt_reason, HaltReason::None);
    assert_eq!(state.consecutive_failures, 0);
}

#[tokio::test]
async fn test_large_batch_processing() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Test maximum batch size
    let markets = create_market_batch(0, BATCH_SIZE);
    let batch = MarketBatch { markets };
    let batch_data = batch.try_to_vec().unwrap();
    
    let process_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data: [vec![0x01], batch_data].concat(),
    };
    
    let start = std::time::Instant::now();
    
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("Should handle maximum batch size");
    
    let duration = start.elapsed();
    println!("Processed {} markets in {:?}", BATCH_SIZE, duration);
    
    // Verify all processed
    let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    
    // Account for any disputed markets
    assert!(state.total_markets_ingested > 900);
    assert!(state.total_markets_ingested <= BATCH_SIZE as u64);
}

#[tokio::test]
async fn test_price_validation() {
    let mut program_test = create_test_env();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    let ingestion_state_pubkey = initialize_ingestion(&mut banks_client, &payer, recent_blockhash)
        .await
        .expect("Failed to initialize ingestion");
    
    // Create markets with invalid prices
    let mut markets = vec![
        PolymarketMarketData {
            id: "1".to_string(),
            title: "Valid market".to_string(),
            description: "Description".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 6000,
            no_price: 4000, // Sum = 10000, valid
            volume_24h: 100000,
            liquidity: 50000,
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        },
        PolymarketMarketData {
            id: "2".to_string(),
            title: "Invalid market - price sum too high".to_string(),
            description: "Description".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 7000,
            no_price: 4000, // Sum = 11000, invalid
            volume_24h: 100000,
            liquidity: 50000,
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        },
        PolymarketMarketData {
            id: "3".to_string(),
            title: "Invalid market - price sum too low".to_string(),
            description: "Description".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 4000,
            no_price: 5000, // Sum = 9000, invalid
            volume_24h: 100000,
            liquidity: 50000,
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        },
    ];
    
    let batch = MarketBatch { markets };
    let batch_data = batch.try_to_vec().unwrap();
    
    let process_ix = Instruction {
        program_id: betting_platform_native::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ingestion_state_pubkey, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data: [vec![0x01], batch_data].concat(),
    };
    
    banks_client.process_transaction(Transaction::new_signed_with_payer(
        &[process_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    )).await.expect("Should process batch with some invalid markets");
    
    // Only 1 valid market should be processed
    let state_account = banks_client.get_account(ingestion_state_pubkey).await.unwrap().unwrap();
    let state = MarketIngestionState::try_from_slice(&state_account.data).unwrap();
    
    assert_eq!(state.total_markets_ingested, 1);
}

#[derive(BorshSerialize, BorshDeserialize)]
struct MarketBatch {
    markets: Vec<PolymarketMarketData>,
}