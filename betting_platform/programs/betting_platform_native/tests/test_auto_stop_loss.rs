//! Tests for auto stop-loss functionality

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform_native::{
    instruction::{BettingInstruction, OpenPositionParams},
    state::{GlobalConfig, ProposalPDAs, Verse, Position},
    trading::auto_stop_loss::{AUTO_STOP_LOSS_THRESHOLD_BPS, AUTO_STOP_LOSS_MIN_LEVERAGE},
};
use borsh::BorshSerialize;

#[tokio::test]
async fn test_auto_stop_loss_creation() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );

    // Setup test accounts
    let user = Keypair::new();
    let proposal_id = 1u128;
    let outcome = 0u8;
    let size = 1_000_000u64; // 1 USDC
    let leverage = 75u8; // High leverage that should trigger auto stop-loss

    // Create global config
    let global_config = GlobalConfig::default();
    let global_config_pubkey = Pubkey::new_unique();
    test.add_account(
        global_config_pubkey,
        Account {
            lamports: 1_000_000,
            data: global_config.try_to_vec().unwrap(),
            owner: program_id,
            ..Account::default()
        },
    );

    // Create proposal
    let proposal = ProposalPDAs {
        proposal_id,
        verse_id: 1,
        n: 2,
        prices: vec![5000, 5000], // 50%
        volumes: vec![0, 0],
        ..Default::default()
    };
    let proposal_pubkey = Pubkey::new_unique();
    test.add_account(
        proposal_pubkey,
        Account {
            lamports: 1_000_000,
            data: proposal.try_to_vec().unwrap(),
            owner: program_id,
            ..Account::default()
        },
    );

    // Start test
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    // Create open position instruction with high leverage
    let params = OpenPositionParams {
        proposal_id,
        outcome,
        size,
        leverage,
        chain_id: None,
    };

    let instruction = BettingInstruction::OpenPosition(params);
    
    // Expected accounts for open position
    let position_pubkey = Pubkey::new_unique();
    let stop_loss_pubkey = Pubkey::new_unique();
    let vault_pubkey = Pubkey::new_unique();
    let user_map_pubkey = Pubkey::new_unique();
    let verse_pubkey = Pubkey::new_unique();
    let system_program = solana_sdk::system_program::id();
    let rent_sysvar = solana_sdk::sysvar::rent::id();

    let accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(global_config_pubkey, false),
        AccountMeta::new(proposal_pubkey, false),
        AccountMeta::new(verse_pubkey, false),
        AccountMeta::new(position_pubkey, false),
        AccountMeta::new(vault_pubkey, false),
        AccountMeta::new(user_map_pubkey, false),
        AccountMeta::new_readonly(system_program, false),
        AccountMeta::new_readonly(rent_sysvar, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
        AccountMeta::new(Pubkey::default(), false), // Cross-margin account (optional)
        AccountMeta::new(stop_loss_pubkey, false), // Auto stop-loss account
    ];

    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts,
            data: instruction.try_to_vec().unwrap(),
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &user], recent_blockhash);

    // Execute transaction
    let result = banks_client.process_transaction(transaction).await;
    
    // Verify transaction succeeded
    assert!(result.is_ok(), "Transaction failed: {:?}", result);

    // Verify stop-loss was created
    let stop_loss_account = banks_client
        .get_account(stop_loss_pubkey)
        .await
        .expect("Failed to get stop-loss account")
        .expect("Stop-loss account not found");

    // Deserialize and verify stop-loss order
    use betting_platform_native::state::order_accounts::StopOrder;
    let stop_loss_order = StopOrder::try_from_slice(&stop_loss_account.data)
        .expect("Failed to deserialize stop-loss order");

    // Verify stop-loss parameters
    assert_eq!(stop_loss_order.user, user.pubkey());
    assert!(stop_loss_order.is_active);
    assert_eq!(stop_loss_order.size, size);
    
    // Verify trigger price is 0.1% below entry (for long position)
    let entry_price = 5000u64;
    let expected_trigger = entry_price - (entry_price * AUTO_STOP_LOSS_THRESHOLD_BPS / 10000);
    assert_eq!(stop_loss_order.trigger_price, expected_trigger);
}

#[test]
fn test_stop_loss_price_calculation() {
    use betting_platform_native::trading::auto_stop_loss::calculate_stop_loss_price;
    
    // Test long position stop-loss (0.1% below entry)
    let entry_price = 10_000u64;
    let stop_price_long = calculate_stop_loss_price(entry_price, true, AUTO_STOP_LOSS_THRESHOLD_BPS);
    assert_eq!(stop_price_long, 9_990); // 10000 - (10000 * 10 / 10000) = 9990
    
    // Test short position stop-loss (0.1% above entry)
    let stop_price_short = calculate_stop_loss_price(entry_price, false, AUTO_STOP_LOSS_THRESHOLD_BPS);
    assert_eq!(stop_price_short, 10_010); // 10000 + (10000 * 10 / 10000) = 10010
}

#[test]
fn test_leverage_threshold() {
    use betting_platform_native::trading::auto_stop_loss::needs_auto_stop_loss;
    
    // Test below threshold
    assert!(!needs_auto_stop_loss(49));
    
    // Test at threshold
    assert!(needs_auto_stop_loss(50));
    
    // Test above threshold
    assert!(needs_auto_stop_loss(100));
}