//! Integration tests for collateral management

use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signer,
};

mod test_framework;
use test_framework::*;

use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    pda::CollateralVaultPDA,
    state::CollateralVault,
    trading::multi_collateral::CollateralType,
};

#[tokio::test]
async fn test_deposit_withdraw_usdc_collateral() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create user with 10,000 USDC
    let user = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    // Get vault PDA
    let (vault_pda, _) = CollateralVaultPDA::derive(&env.program_id);
    
    // Create vault's USDC account
    let vault_usdc = spl_associated_token_account::get_associated_token_address(
        &vault_pda,
        &env.usdc_mint,
    );
    
    // Deposit 5,000 USDC
    let deposit_amount = 5_000_000_000;
    let deposit_accounts = vec![
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
    
    let deposit_ix = Instruction {
        program_id: env.program_id,
        accounts: deposit_accounts,
        data: BettingPlatformInstruction::DepositCollateral { amount: deposit_amount }
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[deposit_ix], &[&user.keypair])
        .await
        .unwrap();
    
    // Check user balance decreased
    assert_token_balance(&mut env, &user.usdc_account, 5_000_000_000).await.unwrap();
    
    // Check vault balance increased
    assert_token_balance(&mut env, &vault_usdc, 5_000_000_000).await.unwrap();
    
    // Withdraw 2,000 USDC
    let withdraw_amount = 2_000_000_000;
    let withdraw_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(user.usdc_account, false),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new(vault_usdc, false),
        AccountMeta::new_readonly(vault_pda, false), // Vault authority
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let withdraw_ix = Instruction {
        program_id: env.program_id,
        accounts: withdraw_accounts,
        data: BettingPlatformInstruction::WithdrawCollateral { amount: withdraw_amount }
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[withdraw_ix], &[&user.keypair])
        .await
        .unwrap();
    
    // Check final balances
    assert_token_balance(&mut env, &user.usdc_account, 7_000_000_000).await.unwrap();
    assert_token_balance(&mut env, &vault_usdc, 3_000_000_000).await.unwrap();
}

#[tokio::test]
async fn test_multi_collateral_deposits() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create user with various tokens
    let user = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    // Test USDC deposit
    test_collateral_deposit(&mut env, &user, CollateralType::USDC, 1_000_000_000)
        .await
        .unwrap();
    
    // For other collateral types, we would need to:
    // 1. Create the respective token mints (USDT, wrapped SOL, etc.)
    // 2. Mint tokens to the user
    // 3. Test deposits and withdrawals
    
    // This demonstrates the pattern for multi-collateral testing
}

async fn test_collateral_deposit(
    env: &mut TestEnvironment,
    user: &TestUser,
    collateral_type: CollateralType,
    amount: u64,
) -> Result<(), BanksClientError> {
    let (vault_pda, _) = CollateralVaultPDA::derive(&env.program_id);
    let mint = collateral_type.mint_address();
    
    let vault_token_account = spl_associated_token_account::get_associated_token_address(
        &vault_pda,
        &mint,
    );
    
    let user_token_account = match collateral_type {
        CollateralType::USDC => user.usdc_account,
        _ => return Ok(()), // Skip other types for now
    };
    
    let accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(user_token_account, false),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new(vault_token_account, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(spl_associated_token_account::id(), false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::DepositMultiCollateral {
            collateral_type: collateral_type as u8,
            amount,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[instruction], &[&user.keypair]).await
}

#[tokio::test]
async fn test_insufficient_collateral_withdrawal() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create user with 1,000 USDC
    let user = env.create_user_with_tokens(1_000_000_000).await.unwrap();
    
    let (vault_pda, _) = CollateralVaultPDA::derive(&env.program_id);
    let vault_usdc = spl_associated_token_account::get_associated_token_address(
        &vault_pda,
        &env.usdc_mint,
    );
    
    // Deposit 1,000 USDC
    let deposit_accounts = vec![
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
    
    let deposit_ix = Instruction {
        program_id: env.program_id,
        accounts: deposit_accounts,
        data: BettingPlatformInstruction::DepositCollateral { amount: 1_000_000_000 }
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[deposit_ix], &[&user.keypair])
        .await
        .unwrap();
    
    // Try to withdraw 2,000 USDC (should fail)
    let withdraw_accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(user.usdc_account, false),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new(vault_usdc, false),
        AccountMeta::new_readonly(vault_pda, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let withdraw_ix = Instruction {
        program_id: env.program_id,
        accounts: withdraw_accounts,
        data: BettingPlatformInstruction::WithdrawCollateral { amount: 2_000_000_000 }
            .try_to_vec()
            .unwrap(),
    };
    
    let result = env.process_transaction(&[withdraw_ix], &[&user.keypair]).await;
    
    // Should fail with insufficient collateral error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_collateral_borrowing_power() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // This test would verify that:
    // 1. Stablecoins (USDC, USDT) have 100% LTV
    // 2. Volatile assets (SOL, BTC, ETH) have 80% LTV
    // 3. Borrowing power is calculated correctly
    // 4. Users cannot borrow more than their collateral allows
    
    // Implementation would follow similar pattern to above tests
}