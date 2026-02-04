// Phase 20 Test Helpers
// Common utilities for integration testing

use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    system_program,
};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use borsh::BorshSerialize;

use betting_platform_native::{
    integration::{
        IntegrationInstruction, MarketUpdate, 
        SystemCoordinator, SystemHealthMonitor, BootstrapCoordinator,
    },
    state::accounts::{GlobalConfig, SystemStatus},
};

/// Create a test program context
pub async fn create_test_context() -> (BanksClient, Keypair, solana_sdk::hash::Hash, Pubkey) {
    let program_id = Pubkey::from_str("BettingP1atform11111111111111111111111111111").unwrap();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let (banks_client, payer, recent_blockhash) = test.start().await;
    (banks_client, payer, recent_blockhash, program_id)
}

/// Create and fund a test account
pub async fn create_funded_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    lamports: u64,
    recent_blockhash: solana_sdk::hash::Hash,
) -> Keypair {
    let account = Keypair::new();
    
    let tx = solana_sdk::system_transaction::transfer(
        payer,
        &account.pubkey(),
        lamports,
        recent_blockhash,
    );
    
    banks_client.process_transaction(tx).await.unwrap();
    account
}

/// Initialize system coordinator
pub async fn initialize_coordinator(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    admin: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
    program_id: Pubkey,
) -> Pubkey {
    let coordinator = Keypair::new();
    let rent = banks_client.get_rent().await.unwrap();
    let space = SystemCoordinator::SIZE;
    let lamports = rent.minimum_balance(space);
    
    // Create coordinator account
    let create_account_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &coordinator.pubkey(),
        lamports,
        space as u64,
        &program_id,
    );
    
    // Initialize coordinator
    let init_data = IntegrationInstruction::InitializeCoordinator {
        amm_engine: Pubkey::new_unique(),
        routing_engine: Pubkey::new_unique(),
        queue_processor: Pubkey::new_unique(),
        keeper_registry: Pubkey::new_unique(),
        health_monitor: Pubkey::new_unique(),
        correlation_calc: Pubkey::new_unique(),
    };
    
    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(coordinator.pubkey(), false),
            AccountMeta::new_readonly(admin.pubkey(), true),
        ],
        data: init_data.try_to_vec().unwrap(),
    };
    
    let mut tx = Transaction::new_with_payer(
        &[create_account_ix, init_ix],
        Some(&payer.pubkey()),
    );
    tx.sign(&[payer, &coordinator, admin], recent_blockhash);
    
    banks_client.process_transaction(tx).await.unwrap();
    
    coordinator.pubkey()
}

/// Initialize health monitor
pub async fn initialize_health_monitor(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    admin: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
    program_id: Pubkey,
) -> Pubkey {
    let monitor = Keypair::new();
    let rent = banks_client.get_rent().await.unwrap();
    let space = SystemHealthMonitor::SIZE;
    let lamports = rent.minimum_balance(space);
    
    // Create monitor account
    let create_account_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &monitor.pubkey(),
        lamports,
        space as u64,
        &program_id,
    );
    
    // Initialize monitor
    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(monitor.pubkey(), false),
            AccountMeta::new_readonly(admin.pubkey(), true),
        ],
        data: vec![0x10], // Initialize health monitor instruction
    };
    
    let mut tx = Transaction::new_with_payer(
        &[create_account_ix, init_ix],
        Some(&payer.pubkey()),
    );
    tx.sign(&[payer, &monitor, admin], recent_blockhash);
    
    banks_client.process_transaction(tx).await.unwrap();
    
    monitor.pubkey()
}

/// Initialize bootstrap coordinator
pub async fn initialize_bootstrap(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    admin: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
    program_id: Pubkey,
) -> Pubkey {
    let bootstrap = Keypair::new();
    let rent = banks_client.get_rent().await.unwrap();
    let space = BootstrapCoordinator::SIZE;
    let lamports = rent.minimum_balance(space);
    
    // Create bootstrap account
    let create_account_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &bootstrap.pubkey(),
        lamports,
        space as u64,
        &program_id,
    );
    
    // Initialize bootstrap
    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bootstrap.pubkey(), false),
            AccountMeta::new_readonly(admin.pubkey(), true),
        ],
        data: vec![0x20], // Initialize bootstrap instruction
    };
    
    let mut tx = Transaction::new_with_payer(
        &[create_account_ix, init_ix],
        Some(&payer.pubkey()),
    );
    tx.sign(&[payer, &bootstrap, admin], recent_blockhash);
    
    banks_client.process_transaction(tx).await.unwrap();
    
    bootstrap.pubkey()
}

/// Process a bootstrap deposit
pub async fn process_bootstrap_deposit(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    depositor: &Keypair,
    bootstrap_pubkey: Pubkey,
    amount: u64,
    recent_blockhash: solana_sdk::hash::Hash,
    program_id: Pubkey,
) -> Result<(), BanksClientError> {
    let mut data = vec![0x21]; // Bootstrap deposit instruction
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(1); // is_new_depositor = true
    
    let deposit_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_pubkey, false),
            AccountMeta::new(depositor.pubkey(), true),
            AccountMeta::new(Pubkey::new_unique(), false), // vault
            AccountMeta::new(Pubkey::new_unique(), false), // mmt_mint
        ],
        data,
    };
    
    let mut tx = Transaction::new_with_payer(
        &[deposit_ix],
        Some(&payer.pubkey()),
    );
    tx.sign(&[payer, depositor], recent_blockhash);
    
    banks_client.process_transaction(tx).await
}

/// Create test market updates
pub fn create_test_market_updates(count: usize) -> Vec<MarketUpdate> {
    (0..count)
        .map(|i| MarketUpdate {
            market_id: Pubkey::new_unique(),
            yes_price: 5000 + (i as u64 * 500),
            no_price: 5000 - (i as u64 * 500),
            volume_24h: 1_000_000_000_000 + (i as u64 * 100_000_000_000),
            liquidity: 500_000_000_000 + (i as u64 * 50_000_000_000),
            timestamp: 1234567890 + i as i64,
        })
        .collect()
}

/// Run a health check
pub async fn run_health_check(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    monitor_pubkey: Pubkey,
    recent_blockhash: solana_sdk::hash::Hash,
    program_id: Pubkey,
) -> Result<(), BanksClientError> {
    let health_check_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(monitor_pubkey, false),
        ],
        data: vec![0x11], // Run health check instruction
    };
    
    let mut tx = Transaction::new_with_payer(
        &[health_check_ix],
        Some(&payer.pubkey()),
    );
    tx.sign(&[payer], recent_blockhash);
    
    banks_client.process_transaction(tx).await
}

/// Verify system status
pub async fn verify_system_status(
    banks_client: &mut BanksClient,
    coordinator_pubkey: Pubkey,
    expected_status: SystemStatus,
) -> bool {
    let account = banks_client
        .get_account(coordinator_pubkey)
        .await
        .unwrap()
        .unwrap();
    
    let coordinator = SystemCoordinator::try_from_slice(&account.data).unwrap();
    coordinator.global_config.status == expected_status
}

/// Verify bootstrap progress
pub async fn verify_bootstrap_progress(
    banks_client: &mut BanksClient,
    bootstrap_pubkey: Pubkey,
) -> (u64, bool, u64) {
    let account = banks_client
        .get_account(bootstrap_pubkey)
        .await
        .unwrap()
        .unwrap();
    
    let bootstrap = BootstrapCoordinator::try_from_slice(&account.data).unwrap();
    (
        bootstrap.vault_balance,
        bootstrap.bootstrap_complete,
        bootstrap.max_leverage_available,
    )
}

use std::str::FromStr;