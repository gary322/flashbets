//! Integration tests for position management

use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signer,
};

// Test framework is in a separate file
#[path = "test_framework.rs"]
mod test_framework;
use test_framework::*;

use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    pda::*,
    state::{Position, UserMap, Proposal},
};

#[tokio::test]
async fn test_open_close_position() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create a proposal/market
    let proposal_id = 1u128;
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    
    // Create the proposal account (simplified)
    create_test_proposal(&mut env, proposal_id).await.unwrap();
    
    // Create user with collateral
    let user = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    // Deposit collateral first
    deposit_collateral(&mut env, &user, 5_000_000_000).await.unwrap();
    
    // Open position
    let (position_pda, _) = PositionPDA::derive(
        &env.program_id,
        &user.pubkey(),
        proposal_id,
        0, // First position
    );
    
    let (user_map_pda, _) = UserMapPDA::derive(&env.program_id, &user.pubkey());
    
    let open_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(position_pda, false),
        AccountMeta::new(user_map_pda, false),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let open_ix = Instruction {
        program_id: env.program_id,
        accounts: open_accounts,
        data: BettingPlatformInstruction::OpenPosition {
            proposal_id,
            outcome: 0,
            size: 1_000_000_000, // 1000 USDC
            leverage: 5,
            is_long: true,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[open_ix], &[&user.keypair])
        .await
        .unwrap();
    
    // Verify position created
    let position_account = env.get_account(&position_pda).await.unwrap();
    let position = Position::try_from_slice(&position_account.data).unwrap();
    assert_eq!(position.size, 1_000_000_000);
    assert_eq!(position.leverage, 5);
    assert!(position.is_long);
    
    // Verify user map updated
    let user_map_account = env.get_account(&user_map_pda).await.unwrap();
    let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
    assert_eq!(user_map.position_count, 1);
    
    // Close position
    let close_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(position_pda, false),
        AccountMeta::new(user_map_pda, false),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
    ];
    
    let close_ix = Instruction {
        program_id: env.program_id,
        accounts: close_accounts,
        data: BettingPlatformInstruction::ClosePosition {
            position_id: position.position_id,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[close_ix], &[&user.keypair])
        .await
        .unwrap();
    
    // Verify position closed
    let position_account = env.get_account(&position_pda).await.unwrap();
    let position = Position::try_from_slice(&position_account.data).unwrap();
    assert_eq!(position.size, 0); // Position closed
    
    // Verify user map updated
    let user_map_account = env.get_account(&user_map_pda).await.unwrap();
    let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
    assert_eq!(user_map.position_count, 0);
}

#[tokio::test]
async fn test_position_leverage_limits() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create proposal
    let proposal_id = 2u128;
    create_test_proposal(&mut env, proposal_id).await.unwrap();
    
    // Create user with collateral
    let user = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    deposit_collateral(&mut env, &user, 1_000_000_000).await.unwrap();
    
    // Try to open position with excessive leverage (>25x)
    let (position_pda, _) = PositionPDA::derive(
        &env.program_id,
        &user.pubkey(),
        proposal_id,
        0,
    );
    
    let (user_map_pda, _) = UserMapPDA::derive(&env.program_id, &user.pubkey());
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    
    let open_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(position_pda, false),
        AccountMeta::new(user_map_pda, false),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let open_ix = Instruction {
        program_id: env.program_id,
        accounts: open_accounts,
        data: BettingPlatformInstruction::OpenPosition {
            proposal_id,
            outcome: 0,
            size: 500_000_000,
            leverage: 30, // Exceeds max leverage
            is_long: true,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    let result = env.process_transaction(&[open_ix], &[&user.keypair]).await;
    
    // Should fail due to leverage limit
    assert!(result.is_err());
}

#[tokio::test]
async fn test_position_margin_requirements() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create proposal
    let proposal_id = 3u128;
    create_test_proposal(&mut env, proposal_id).await.unwrap();
    
    // Create user with limited collateral
    let user = env.create_user_with_tokens(1_000_000_000).await.unwrap();
    deposit_collateral(&mut env, &user, 100_000_000).await.unwrap(); // Only 100 USDC
    
    // Try to open position requiring more margin
    let (position_pda, _) = PositionPDA::derive(
        &env.program_id,
        &user.pubkey(),
        proposal_id,
        0,
    );
    
    let (user_map_pda, _) = UserMapPDA::derive(&env.program_id, &user.pubkey());
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    
    let open_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(position_pda, false),
        AccountMeta::new(user_map_pda, false),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    // Position requires 200 USDC margin (1000 size / 5 leverage)
    let open_ix = Instruction {
        program_id: env.program_id,
        accounts: open_accounts,
        data: BettingPlatformInstruction::OpenPosition {
            proposal_id,
            outcome: 0,
            size: 1_000_000_000,
            leverage: 5,
            is_long: true,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    let result = env.process_transaction(&[open_ix], &[&user.keypair]).await;
    
    // Should fail due to insufficient margin
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_positions() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create multiple proposals
    let proposal_ids = vec![4u128, 5u128, 6u128];
    for id in &proposal_ids {
        create_test_proposal(&mut env, *id).await.unwrap();
    }
    
    // Create user with ample collateral
    let user = env.create_user_with_tokens(50_000_000_000).await.unwrap();
    deposit_collateral(&mut env, &user, 20_000_000_000).await.unwrap();
    
    let (user_map_pda, _) = UserMapPDA::derive(&env.program_id, &user.pubkey());
    
    // Open multiple positions
    for (index, proposal_id) in proposal_ids.iter().enumerate() {
        let (position_pda, _) = PositionPDA::derive(
            &env.program_id,
            &user.pubkey(),
            *proposal_id,
            index as u8,
        );
        
        let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, *proposal_id);
        
        let open_accounts = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(position_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(proposal_pda, false),
            AccountMeta::new_readonly(env.program_id, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ];
        
        let open_ix = Instruction {
            program_id: env.program_id,
            accounts: open_accounts,
            data: BettingPlatformInstruction::OpenPosition {
                proposal_id: *proposal_id,
                outcome: (index % 2) as u8,
                size: 1_000_000_000,
                leverage: 3 + index as u64,
                is_long: index % 2 == 0,
            }
            .try_to_vec()
            .unwrap(),
        };
        
        env.process_transaction(&[open_ix], &[&user.keypair])
            .await
            .unwrap();
    }
    
    // Verify user map shows 3 positions
    let user_map_account = env.get_account(&user_map_pda).await.unwrap();
    let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
    assert_eq!(user_map.position_count, 3);
}

// Helper functions

async fn create_test_proposal(
    env: &mut TestEnvironment,
    proposal_id: u128,
) -> Result<(), BanksClientError> {
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    let (verse_pda, _) = VersePDA::derive(&env.program_id, 1); // Default verse
    
    let accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new(verse_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::CreateProposal {
            proposal_id,
            verse_id: 1,
            market_id: [0u8; 32],
            amm_type: 0, // LMSR
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[instruction], &[]).await
}

async fn deposit_collateral(
    env: &mut TestEnvironment,
    user: &TestUser,
    amount: u64,
) -> Result<(), BanksClientError> {
    let (vault_pda, _) = CollateralVaultPDA::derive(&env.program_id);
    let vault_usdc = spl_associated_token_account::get_associated_token_address(
        &vault_pda,
        &env.usdc_mint,
    );
    
    let accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(user.usdc_account, false),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new(vault_usdc, false),
        AccountMeta::new_readonly(env.usdc_mint, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(spl_associated_token_account::id(), false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::DepositCollateral { amount }
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[instruction], &[&user.keypair]).await
}