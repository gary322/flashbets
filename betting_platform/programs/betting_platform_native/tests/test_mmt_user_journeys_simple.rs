//! Simplified User Journey Tests for MMT Token System
//! 
//! These tests avoid the Associated Token Account program issue by
//! pre-creating accounts and focusing on the core functionality

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    system_program,
    sysvar,
};
use solana_program_test::{*};
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{
    state::{Account as TokenAccount},
};
use borsh::BorshSerialize;

use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    mmt::{
        constants::*,
        state::*,
    },
    processor::process_instruction,
};

/// Simple test environment focused on core functionality
struct SimpleMMTTestEnv {
    program_id: Pubkey,
    banks_client: BanksClient,
    payer: Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
    mmt_mint: Pubkey,
}

impl SimpleMMTTestEnv {
    async fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(process_instruction),
        );

        let (banks_client, payer, recent_blockhash) = program_test.start().await;
        
        // Derive MMT mint PDA
        let (mmt_mint, _) = Pubkey::find_program_address(&[MMT_MINT_SEED], &program_id);

        SimpleMMTTestEnv {
            program_id,
            banks_client,
            payer,
            recent_blockhash,
            mmt_mint,
        }
    }

    async fn create_user_with_mmt(&mut self, _amount: u64) -> (Keypair, Pubkey) {
        let user = Keypair::new();
        
        // Airdrop SOL
        let mut transaction = Transaction::new_with_payer(
            &[system_instruction::transfer(
                &self.payer.pubkey(),
                &user.pubkey(),
                1_000_000_000, // 1 SOL
            )],
            Some(&self.payer.pubkey()),
        );
        transaction.sign(&[&self.payer], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await.unwrap();

        // Create user token account manually
        let user_token = Keypair::new();
        let rent = self.banks_client.get_rent().await.unwrap();
        let space = TokenAccount::LEN;
        
        let mut transaction = Transaction::new_with_payer(
            &[
                system_instruction::create_account(
                    &self.payer.pubkey(),
                    &user_token.pubkey(),
                    rent.minimum_balance(space),
                    space as u64,
                    &spl_token::id(),
                ),
                spl_token::instruction::initialize_account(
                    &spl_token::id(),
                    &user_token.pubkey(),
                    &self.mmt_mint,
                    &user.pubkey(),
                ).unwrap(),
            ],
            Some(&self.payer.pubkey()),
        );
        transaction.sign(&[&self.payer, &user_token], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await.unwrap();

        (user, user_token.pubkey())
    }

    async fn stake_mmt(
        &mut self,
        user: &Keypair,
        user_token: &Pubkey,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let (stake_account, _) = Pubkey::find_program_address(
            &[STAKE_ACCOUNT_SEED, user.pubkey().as_ref()],
            &self.program_id,
        );
        let (staking_pool, _) = Pubkey::find_program_address(&[STAKING_POOL_SEED], &self.program_id);
        let (stake_vault, _) = Pubkey::find_program_address(&[STAKE_VAULT_SEED], &self.program_id);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(stake_account, false),
                AccountMeta::new(staking_pool, false),
                AccountMeta::new(*user_token, false),
                AccountMeta::new(stake_vault, false),
                AccountMeta::new_readonly(self.mmt_mint, false),
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ],
            data: BettingPlatformInstruction::StakeMMT {
                amount,
                lock_period_slots: None,
            }.try_to_vec().unwrap(),
        };

        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));
        transaction.sign(&[&self.payer, user], self.recent_blockhash);
        
        self.banks_client.process_transaction(transaction).await
    }
}

#[tokio::test]
async fn test_simple_staking_flow() {
    let mut env = SimpleMMTTestEnv::new().await;
    
    // Create user with MMT tokens
    let (user, user_token) = env.create_user_with_mmt(1_000_000).await;
    
    // Stake tokens
    env.stake_mmt(&user, &user_token, 500_000).await.unwrap();
    
    // Verify stake account was created
    let (stake_account, _) = Pubkey::find_program_address(
        &[STAKE_ACCOUNT_SEED, user.pubkey().as_ref()],
        &env.program_id,
    );
    
    let stake_account_data = env.banks_client.get_account(stake_account).await.unwrap().unwrap();
    let stake = StakeAccount::unpack(&stake_account_data.data).unwrap();
    
    assert_eq!(stake.owner, user.pubkey());
    assert_eq!(stake.amount, 500_000);
    
    println!("✅ Simple staking test passed!");
}

#[tokio::test]
async fn test_simple_maker_flow() {
    let mut env = SimpleMMTTestEnv::new().await;
    
    // Create maker with MMT tokens
    let (maker, maker_token) = env.create_user_with_mmt(100_000).await;
    
    // Initialize maker account
    let (maker_account, _) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, maker.pubkey().as_ref()],
        &env.program_id,
    );
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(maker_account, false),
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeMakerAccount.try_to_vec().unwrap(),
    };
    
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer, &maker], env.recent_blockhash);
    env.banks_client.process_transaction(transaction).await.unwrap();
    
    // Verify maker account
    let maker_account_data = env.banks_client.get_account(maker_account).await.unwrap().unwrap();
    let maker_data = MakerAccount::unpack(&maker_account_data.data).unwrap();
    
    assert_eq!(maker_data.owner, maker.pubkey());
    assert_eq!(maker_data.metrics.trades_count, 0);
    
    println!("✅ Simple maker test passed!");
}