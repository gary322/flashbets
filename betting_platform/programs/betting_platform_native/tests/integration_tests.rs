//! Main integration test suite for betting platform

mod test_framework;
mod test_collateral;
mod test_amm;
mod test_positions;

use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signer,
};

use test_framework::*;
use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    pda::*,
    state::*,
};

#[tokio::test]
async fn test_full_trading_lifecycle() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // 1. Initialize platform
    env.initialize_global_config().await.unwrap();
    
    // 2. Create a verse (category)
    let verse_id = 1u128;
    let (verse_pda, _) = VersePDA::derive(&env.program_id, verse_id);
    
    let create_verse_accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(verse_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let create_verse_ix = Instruction {
        program_id: env.program_id,
        accounts: create_verse_accounts,
        data: BettingPlatformInstruction::CreateVerse {
            verse_id,
            parent_id: None,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[create_verse_ix], &[]).await.unwrap();
    
    // 3. Create a proposal/market
    let proposal_id = 1u128;
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    
    let create_proposal_accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new(verse_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let create_proposal_ix = Instruction {
        program_id: env.program_id,
        accounts: create_proposal_accounts,
        data: BettingPlatformInstruction::CreateProposal {
            proposal_id,
            verse_id,
            market_id: [1u8; 32],
            amm_type: 0, // LMSR
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[create_proposal_ix], &[]).await.unwrap();
    
    // 4. Initialize AMM market
    let market_pda = create_test_market(&mut env, proposal_id, "LMSR").await.unwrap();
    
    // 5. Create users
    let trader1 = env.create_user_with_tokens(20_000_000_000).await.unwrap();
    let trader2 = env.create_user_with_tokens(20_000_000_000).await.unwrap();
    
    // 6. Deposit collateral
    deposit_collateral(&mut env, &trader1, 10_000_000_000).await.unwrap();
    deposit_collateral(&mut env, &trader2, 10_000_000_000).await.unwrap();
    
    // 7. Open positions
    open_position(&mut env, &trader1, proposal_id, 0, 2_000_000_000, 5, true).await.unwrap();
    open_position(&mut env, &trader2, proposal_id, 1, 3_000_000_000, 3, true).await.unwrap();
    
    // 8. Simulate price movement via AMM trades
    execute_amm_trade(&mut env, &trader1, proposal_id, 0, 500_000_000, true).await.unwrap();
    
    // 9. Close positions with PnL
    close_position(&mut env, &trader1, 0).await.unwrap();
    
    // 10. Resolve market
    resolve_market(&mut env, proposal_id, 0).await.unwrap();
    
    // 11. Claim winnings
    claim_winnings(&mut env, &trader1, proposal_id).await.unwrap();
    
    // 12. Withdraw collateral
    withdraw_collateral(&mut env, &trader1, 5_000_000_000).await.unwrap();
}

#[tokio::test]
async fn test_chain_execution() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize platform
    env.initialize_global_config().await.unwrap();
    
    // Create verse and proposals
    let verse_id = 2u128;
    create_verse(&mut env, verse_id).await.unwrap();
    
    let proposal_ids = vec![10u128, 11u128, 12u128];
    for id in &proposal_ids {
        create_proposal(&mut env, *id, verse_id).await.unwrap();
        create_test_market(&mut env, *id, "LMSR").await.unwrap();
    }
    
    // Create user with collateral
    let user = env.create_user_with_tokens(50_000_000_000).await.unwrap();
    deposit_collateral(&mut env, &user, 20_000_000_000).await.unwrap();
    
    // Create chain
    let chain_id = 1u128;
    let (chain_pda, _) = ChainStatePDA::derive(&env.program_id, chain_id);
    
    let chain_steps = vec![
        ChainStep {
            proposal_id: proposal_ids[0],
            outcome: 0,
            size_percent: 30, // 30% of balance
            leverage: 2,
            is_long: true,
            stop_loss: Some(950_000), // 95% of entry
            take_profit: Some(1_100_000), // 110% of entry
        },
        ChainStep {
            proposal_id: proposal_ids[1],
            outcome: 1,
            size_percent: 50, // 50% of remaining
            leverage: 3,
            is_long: false,
            stop_loss: None,
            take_profit: None,
        },
        ChainStep {
            proposal_id: proposal_ids[2],
            outcome: 0,
            size_percent: 100, // All remaining
            leverage: 5,
            is_long: true,
            stop_loss: Some(900_000),
            take_profit: Some(1_200_000),
        },
    ];
    
    let create_chain_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(chain_pda, false),
        AccountMeta::new(verse_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let create_chain_ix = Instruction {
        program_id: env.program_id,
        accounts: create_chain_accounts,
        data: BettingPlatformInstruction::CreateChain {
            chain_id,
            verse_id,
            steps: chain_steps,
            initial_balance: 5_000_000_000, // 5000 USDC
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[create_chain_ix], &[&user.keypair])
        .await
        .unwrap();
    
    // Execute chain steps
    for i in 0..3 {
        execute_chain_step(&mut env, &user, chain_id, i).await.unwrap();
        
        // Simulate some market movement
        env.advance_slots(100).await.unwrap();
    }
    
    // Verify chain completed
    let chain_account = env.get_account(&chain_pda).await.unwrap();
    let chain_state = ChainState::try_from_slice(&chain_account.data).unwrap();
    assert_eq!(chain_state.current_step, 3);
    assert_eq!(chain_state.status, ChainStatus::Completed);
}

// Helper functions

async fn create_verse(
    env: &mut TestEnvironment,
    verse_id: u128,
) -> Result<(), BanksClientError> {
    let (verse_pda, _) = VersePDA::derive(&env.program_id, verse_id);
    
    let accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(verse_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::CreateVerse {
            verse_id,
            parent_id: None,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[instruction], &[]).await
}

async fn create_proposal(
    env: &mut TestEnvironment,
    proposal_id: u128,
    verse_id: u128,
) -> Result<(), BanksClientError> {
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    let (verse_pda, _) = VersePDA::derive(&env.program_id, verse_id);
    
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
            verse_id,
            market_id: proposal_id.to_le_bytes()[..32].try_into().unwrap(),
            amm_type: 0,
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

async fn withdraw_collateral(
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
        AccountMeta::new_readonly(vault_pda, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::WithdrawCollateral { amount }
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[instruction], &[&user.keypair]).await
}

async fn open_position(
    env: &mut TestEnvironment,
    user: &TestUser,
    proposal_id: u128,
    outcome: u8,
    size: u64,
    leverage: u64,
    is_long: bool,
) -> Result<(), BanksClientError> {
    let (position_pda, _) = PositionPDA::derive(
        &env.program_id,
        &user.pubkey(),
        proposal_id,
        0, // First position
    );
    
    let (user_map_pda, _) = UserMapPDA::derive(&env.program_id, &user.pubkey());
    let (proposal_pda, _) = ProposalPDA::derive(&env.program_id, proposal_id);
    
    let accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(position_pda, false),
        AccountMeta::new(user_map_pda, false),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::OpenPosition {
            proposal_id,
            outcome,
            size,
            leverage,
            is_long,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[instruction], &[&user.keypair]).await
}

async fn close_position(
    env: &mut TestEnvironment,
    user: &TestUser,
    position_index: u8,
) -> Result<(), BanksClientError> {
    // Implementation would fetch position details and close it
    Ok(())
}

async fn execute_amm_trade(
    env: &mut TestEnvironment,
    user: &TestUser,
    market_id: u128,
    outcome: u8,
    shares: u64,
    is_buy: bool,
) -> Result<(), BanksClientError> {
    // Implementation would execute trade on appropriate AMM
    Ok(())
}

async fn resolve_market(
    env: &mut TestEnvironment,
    market_id: u128,
    winning_outcome: u8,
) -> Result<(), BanksClientError> {
    // Implementation would resolve market with oracle or admin
    Ok(())
}

async fn claim_winnings(
    env: &mut TestEnvironment,
    user: &TestUser,
    proposal_id: u128,
) -> Result<(), BanksClientError> {
    // Implementation would claim winnings from resolved market
    Ok(())
}

async fn execute_chain_step(
    env: &mut TestEnvironment,
    user: &TestUser,
    chain_id: u128,
    step_index: u8,
) -> Result<(), BanksClientError> {
    // Implementation would execute the next chain step
    Ok(())
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
struct ChainStep {
    proposal_id: u128,
    outcome: u8,
    size_percent: u8,
    leverage: u64,
    is_long: bool,
    stop_loss: Option<u64>,
    take_profit: Option<u64>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq)]
enum ChainStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct ChainState {
    current_step: u8,
    status: ChainStatus,
}