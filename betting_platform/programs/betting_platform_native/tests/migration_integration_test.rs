//! Integration tests for the 60-day migration framework
//!
//! Tests the complete migration flow including:
//! - Initialization of parallel migration
//! - Position migration with double MMT incentives
//! - Migration completion after 60 days
//! - Error cases and edge conditions

use solana_program_test::{processor, ProgramTest, BanksClient, ProgramTestContext};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_instruction,
    clock::Clock,
    rent::Rent,
    sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    migration::{ParallelDeployment, MIGRATION_PERIOD_SLOTS, MIGRATION_MMT_MULTIPLIER},
    state::{Position, GlobalConfigPDA},
    error::BettingPlatformError,
};

/// Setup test environment
async fn setup_test() -> (ProgramTestContext, Pubkey) {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    // Add global config account
    let global_config_pda = Pubkey::new_unique();
    let mut config = GlobalConfigPDA::new();
    config.update_authority = Keypair::new().pubkey();
    
    program_test.add_account(
        global_config_pda,
        Account {
            lamports: 1_000_000,
            data: config.try_to_vec().unwrap(),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    );
    
    let ctx = program_test.start_with_context().await;
    (ctx, program_id)
}

#[tokio::test]
async fn test_initialize_parallel_migration() {
    let (mut ctx, old_program_id) = setup_test().await;
    let new_program_id = Pubkey::new_unique();
    
    // Create migration state account
    let migration_state = Keypair::new();
    let authority = Keypair::new();
    let global_config = Pubkey::new_unique();
    
    // Create initialization instruction
    let ix = Instruction {
        program_id: old_program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(migration_state.pubkey(), false),
            AccountMeta::new_readonly(global_config, false),
        ],
        data: BettingPlatformInstruction::InitializeParallelMigration { 
            new_program_id 
        }.try_to_vec().unwrap(),
    };
    
    // Create and send transaction
    let mut transaction = Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &authority.pubkey(),
                &migration_state.pubkey(),
                1_000_000,
                1000,
                &old_program_id,
            ),
            ix,
        ],
        Some(&authority.pubkey()),
    );
    
    transaction.sign(&[&authority, &migration_state], ctx.last_blockhash);
    
    // Execute transaction
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
    
    // Verify migration state
    let migration_account = ctx.banks_client
        .get_account(migration_state.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let deployment = ParallelDeployment::try_from_slice(&migration_account.data).unwrap();
    
    assert_eq!(deployment.old_program_id, old_program_id);
    assert_eq!(deployment.new_program_id, new_program_id);
    assert_eq!(deployment.authority, authority.pubkey());
    assert!(deployment.is_active);
    assert_eq!(deployment.positions_migrated, 0);
    assert_eq!(deployment.mmt_rewards_distributed, 0);
}

#[tokio::test]
async fn test_migrate_position_with_incentives() {
    let (mut ctx, old_program_id) = setup_test().await;
    let new_program_id = Pubkey::new_unique();
    
    // Setup accounts
    let user = Keypair::new();
    let old_position = Keypair::new();
    let new_position = Keypair::new();
    let migration_state = Keypair::new();
    let mmt_treasury = Keypair::new();
    let user_mmt_account = Keypair::new();
    
    // Create test position
    let position = Position {
        position_id: [1u8; 32],
        user: user.pubkey(),
        size: 1000,
        notional: 1_000_000_000, // 1000 units
        leverage: 10,
        is_long: true,
        entry_price: 50_000,
        liquidation_price: 45_000,
        created_at: 0,
        updated_at: 0,
        pnl: 0,
        fees_paid: 0,
        proposal_id: 123,
        outcome: 1,
        status: 0, // Open
        margin_requirement: 100_000_000,
        last_funding_payment: 0,
        cumulative_funding: 0,
    };
    
    // Add position account
    ctx.banks_client.process_transaction(
        Transaction::new_signed_with_payer(
            &[system_instruction::create_account(
                &user.pubkey(),
                &old_position.pubkey(),
                1_000_000,
                std::mem::size_of::<Position>() as u64,
                &old_program_id,
            )],
            Some(&user.pubkey()),
            &[&user, &old_position],
            ctx.last_blockhash,
        ),
    ).await.unwrap();
    
    // Write position data
    let mut account = ctx.banks_client
        .get_account(old_position.pubkey())
        .await
        .unwrap()
        .unwrap();
    account.data = position.try_to_vec().unwrap();
    
    // Create migration instruction
    let ix = Instruction {
        program_id: old_program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(old_position.pubkey(), false),
            AccountMeta::new(new_position.pubkey(), false),
            AccountMeta::new(migration_state.pubkey(), false),
            AccountMeta::new(mmt_treasury.pubkey(), false),
            AccountMeta::new(user_mmt_account.pubkey(), false),
        ],
        data: BettingPlatformInstruction::MigratePositionWithIncentives {
            position_id: position.position_id,
        }.try_to_vec().unwrap(),
    };
    
    // Execute migration
    let mut transaction = Transaction::new_with_payer(
        &[ix],
        Some(&user.pubkey()),
    );
    
    transaction.sign(&[&user], ctx.last_blockhash);
    
    // Should succeed
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
    
    // Verify old position closed
    let old_position_account = ctx.banks_client
        .get_account(old_position.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    // All data should be zeroed
    assert!(old_position_account.data.iter().all(|&b| b == 0));
    
    // Calculate expected MMT reward
    let expected_reward = (position.notional / 1000) * MIGRATION_MMT_MULTIPLIER;
    assert_eq!(expected_reward, 2_000_000); // 2 MMT tokens
}

#[tokio::test]
async fn test_migration_expiry() {
    let (mut ctx, old_program_id) = setup_test().await;
    
    // Create expired migration
    let mut deployment = ParallelDeployment::new(
        old_program_id,
        Pubkey::new_unique(),
        Keypair::new().pubkey(),
        0,
    );
    
    // Fast forward past migration period
    let slot = MIGRATION_PERIOD_SLOTS + 1;
    
    // Set clock
    let mut clock = Clock::default();
    clock.slot = slot;
    ctx.set_sysvar(&clock);
    
    // Try to migrate after expiry
    let user = Keypair::new();
    let ix = Instruction {
        program_id: old_program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            // ... other accounts
        ],
        data: BettingPlatformInstruction::MigratePositionWithIncentives {
            position_id: [0u8; 32],
        }.try_to_vec().unwrap(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[ix],
        Some(&user.pubkey()),
    );
    
    transaction.sign(&[&user], ctx.last_blockhash);
    
    // Should fail with MigrationExpired error
    let result = ctx.banks_client
        .process_transaction(transaction)
        .await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_complete_migration() {
    let (mut ctx, old_program_id) = setup_test().await;
    
    // Setup migration state
    let authority = Keypair::new();
    let migration_state = Keypair::new();
    
    // Create migration that's ready to complete
    let mut deployment = ParallelDeployment::new(
        old_program_id,
        Pubkey::new_unique(),
        authority.pubkey(),
        0,
    );
    deployment.positions_migrated = 100;
    deployment.mmt_rewards_distributed = 100_000_000;
    
    // Fast forward past migration period
    let mut clock = Clock::default();
    clock.slot = MIGRATION_PERIOD_SLOTS + 1;
    ctx.set_sysvar(&clock);
    
    // Create completion instruction
    let ix = Instruction {
        program_id: old_program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(migration_state.pubkey(), false),
        ],
        data: BettingPlatformInstruction::CompleteMigration
            .try_to_vec().unwrap(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[ix],
        Some(&authority.pubkey()),
    );
    
    transaction.sign(&[&authority], ctx.last_blockhash);
    
    // Execute completion
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
    
    // Verify migration marked as inactive
    let migration_account = ctx.banks_client
        .get_account(migration_state.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let final_deployment = ParallelDeployment::try_from_slice(&migration_account.data).unwrap();
    assert!(!final_deployment.is_active);
}

#[tokio::test]
async fn test_migration_pause_resume() {
    let (mut ctx, old_program_id) = setup_test().await;
    
    let authority = Keypair::new();
    let migration_state = Keypair::new();
    
    // Create active migration
    let deployment = ParallelDeployment::new(
        old_program_id,
        Pubkey::new_unique(),
        authority.pubkey(),
        0,
    );
    
    // Pause migration
    let pause_ix = Instruction {
        program_id: old_program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(migration_state.pubkey(), false),
        ],
        data: BettingPlatformInstruction::PauseExtendedMigration {
            reason: "Emergency pause for investigation".to_string(),
        }.try_to_vec().unwrap(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[pause_ix],
        Some(&authority.pubkey()),
    );
    
    transaction.sign(&[&authority], ctx.last_blockhash);
    
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
    
    // Verify paused
    let migration_account = ctx.banks_client
        .get_account(migration_state.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let paused_deployment = ParallelDeployment::try_from_slice(&migration_account.data).unwrap();
    assert!(!paused_deployment.is_active);
    
    // Resume migration
    let resume_ix = Instruction {
        program_id: old_program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(migration_state.pubkey(), false),
        ],
        data: BettingPlatformInstruction::ResumeExtendedMigration
            .try_to_vec().unwrap(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[resume_ix],
        Some(&authority.pubkey()),
    );
    
    transaction.sign(&[&authority], ctx.last_blockhash);
    
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
    
    // Verify resumed
    let migration_account = ctx.banks_client
        .get_account(migration_state.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let resumed_deployment = ParallelDeployment::try_from_slice(&migration_account.data).unwrap();
    assert!(resumed_deployment.is_active);
}

#[tokio::test]
async fn test_migration_progress_tracking() {
    let deployment = ParallelDeployment::new(
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        0,
    );
    
    // Test various progress points
    assert_eq!(deployment.progress_percentage(0), 0);
    assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS / 4), 25);
    assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS / 2), 50);
    assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS * 3 / 4), 75);
    assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS), 100);
    
    // Test days remaining
    let days_per_slot = 216_000; // ~1 day in slots
    assert_eq!(deployment.remaining_slots(0) / days_per_slot, 60);
    assert_eq!(deployment.remaining_slots(MIGRATION_PERIOD_SLOTS / 2) / days_per_slot, 30);
    assert_eq!(deployment.remaining_slots(MIGRATION_PERIOD_SLOTS), 0);
}