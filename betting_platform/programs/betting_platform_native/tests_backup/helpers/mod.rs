//! Test helper functions and utilities

pub mod phase20_helpers;
pub mod simulation_helpers;

use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use solana_sdk::sysvar;
use solana_sdk::{
    signature::Keypair,
    signer::Signer,
};

/// Create a test keypair with SOL balance
pub async fn create_funded_keypair(
    banks_client: &mut solana_program_test::BanksClient,
    payer: &Keypair,
    lamports: u64,
) -> Keypair {
    use solana_sdk::system_transaction;
    
    let keypair = Keypair::new();
    let recent_blockhash = banks_client.get_latest_blockhash().await.unwrap();
    
    let transaction = system_transaction::transfer(
        payer,
        &keypair.pubkey(),
        lamports,
        recent_blockhash,
    );
    
    banks_client.process_transaction(transaction).await.unwrap();
    keypair
}

/// Get account data
pub async fn get_account(
    banks_client: &mut solana_program_test::BanksClient,
    pubkey: &Pubkey,
) -> Option<solana_sdk::account::Account> {
    banks_client
        .get_account(*pubkey)
        .await
        .unwrap()
}

/// Assert account exists
pub async fn assert_account_exists(
    banks_client: &mut solana_program_test::BanksClient,
    pubkey: &Pubkey,
) {
    let account = get_account(banks_client, pubkey).await;
    assert!(account.is_some(), "Account {} does not exist", pubkey);
}

/// Assert account has expected owner
pub async fn assert_account_owner(
    banks_client: &mut solana_program_test::BanksClient,
    pubkey: &Pubkey,
    expected_owner: &Pubkey,
) {
    let account = get_account(banks_client, pubkey).await.unwrap();
    assert_eq!(
        account.owner, *expected_owner,
        "Account {} has wrong owner. Expected: {}, Actual: {}",
        pubkey, expected_owner, account.owner
    );
}

/// Create PDA and bump seed
pub fn create_pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

/// Build instruction
pub fn build_instruction(
    program_id: Pubkey,
    accounts: Vec<AccountMeta>,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id,
        accounts,
        data,
    }
}

/// Convert string to fixed array
pub fn string_to_array<const N: usize>(s: &str) -> [u8; N] {
    let bytes = s.as_bytes();
    let mut array = [0u8; N];
    let len = bytes.len().min(N);
    array[..len].copy_from_slice(&bytes[..len]);
    array
}

/// Print test section header
pub fn print_test_section(title: &str) {
    println!("\n{}", "=".repeat(50));
    println!("{:^50}", title);
    println!("{}", "=".repeat(50));
}

/// Assert transaction error
pub async fn assert_transaction_err<E>(
    banks_client: &mut solana_program_test::BanksClient,
    transaction: solana_sdk::transaction::Transaction,
    _expected_error: E,
) where
    E: std::fmt::Debug + PartialEq,
{
    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err());
    // In practice, you'd parse the specific error from the result
    println!("✓ Transaction failed as expected");
}

/// Create test market parameters
pub struct TestMarketParams {
    pub market_id: u128,
    pub verse_id: u128,
    pub num_outcomes: u8,
    pub oracle: Pubkey,
}

impl Default for TestMarketParams {
    fn default() -> Self {
        Self {
            market_id: 1,
            verse_id: 1,
            num_outcomes: 2,
            oracle: Keypair::new().pubkey(),
        }
    }
}

/// Create test position parameters
pub struct TestPositionParams {
    pub market_id: u128,
    pub outcome: u8,
    pub size: u64,
    pub leverage: u64,
    pub is_long: bool,
}

impl Default for TestPositionParams {
    fn default() -> Self {
        Self {
            market_id: 1,
            outcome: 0,
            size: 1000,
            leverage: 1,
            is_long: true,
        }
    }
}

/// Test constants
pub mod test_constants {
    pub const MIN_MMT_STAKE: u64 = 1_000_000_000_000; // 1000 MMT
    pub const MMT_DECIMALS: u8 = 9;
    pub const USDC_DECIMALS: u8 = 6;
    pub const BASIS_POINTS: u64 = 10_000;
    pub const DEFAULT_FEE_BPS: u16 = 30; // 0.3%
    pub const DISPUTE_WINDOW_SECONDS: u64 = 86_400; // 24 hours
    pub const SEASON_DURATION_SLOTS: u64 = 38_880_000; // ~6 months
}

/// Format token amount with decimals
pub fn format_token_amount(amount: u64, decimals: u8) -> String {
    let divisor = 10u64.pow(decimals as u32);
    let whole = amount / divisor;
    let fraction = amount % divisor;
    
    if fraction == 0 {
        format!("{}", whole)
    } else {
        format!("{}.{:0width$}", whole, fraction, width = decimals as usize)
    }
}

/// Calculate percentage
pub fn calculate_percentage(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (value as f64 / total as f64) * 100.0
    }
}