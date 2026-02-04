//! Test framework for betting platform
//!
//! Provides utilities and helpers for integration testing

use solana_program_test::{
    BanksClient, BanksClientError, ProgramTest, ProgramTestContext,
};
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};

use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    pda::*,
    state::*,
};

/// Test environment for betting platform
pub struct TestEnvironment {
    pub context: ProgramTestContext,
    pub program_id: Pubkey,
    pub authority: Keypair,
    pub usdc_mint: Pubkey,
    pub mmt_mint: Pubkey,
}

impl TestEnvironment {
    /// Create a new test environment
    pub async fn new() -> Result<Self, BanksClientError> {
        let program_id = Pubkey::new_unique();
        let mut program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            None, // Will use BPF loader in tests
        );

        // Add SPL Token program
        program_test.add_program("spl_token", spl_token::id(), None);

        // Start test
        let mut context = program_test.start_with_context().await;
        
        // Create authority
        let authority = Keypair::new();
        
        // Create USDC mint
        let usdc_mint = Keypair::new();
        let rent = context.banks_client.get_rent().await?;
        let mint_rent = rent.minimum_balance(Mint::LEN);
        
        let create_mint_ix = system_instruction::create_account(
            &context.payer.pubkey(),
            &usdc_mint.pubkey(),
            mint_rent,
            Mint::LEN as u64,
            &spl_token::id(),
        );
        
        let init_mint_ix = token_instruction::initialize_mint(
            &spl_token::id(),
            &usdc_mint.pubkey(),
            &authority.pubkey(),
            None,
            6, // USDC decimals
        )?;
        
        let mut transaction = Transaction::new_with_payer(
            &[create_mint_ix, init_mint_ix],
            Some(&context.payer.pubkey()),
        );
        transaction.sign(&[&context.payer, &usdc_mint], context.last_blockhash);
        context.banks_client.process_transaction(transaction).await?;
        
        // Create MMT mint (will be created by program)
        let (mmt_mint, _) = MmtMintPDA::derive(&program_id);
        
        Ok(Self {
            context,
            program_id,
            authority,
            usdc_mint: usdc_mint.pubkey(),
            mmt_mint,
        })
    }

    /// Initialize the global config
    pub async fn initialize_global_config(&mut self) -> Result<(), BanksClientError> {
        let (global_config_pda, _) = betting_platform_native::pda::GlobalConfigPDA::derive(&self.program_id);
        
        let accounts = vec![
            AccountMeta::new(self.context.payer.pubkey(), true),
            AccountMeta::new(global_config_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ];
        
        let instruction = Instruction {
            program_id: self.program_id,
            accounts,
            data: BettingPlatformInstruction::Initialize.try_to_vec().unwrap(),
        };
        
        self.process_transaction(&[instruction], &[]).await
    }

    /// Create a new user with token accounts
    pub async fn create_user_with_tokens(
        &mut self,
        usdc_amount: u64,
    ) -> Result<TestUser, BanksClientError> {
        let user = Keypair::new();
        
        // Airdrop SOL
        self.airdrop(&user.pubkey(), 10_000_000_000).await?;
        
        // Create USDC token account
        let usdc_account = self.create_token_account(
            &user,
            &self.usdc_mint,
            &user.pubkey(),
        ).await?;
        
        // Mint USDC to user
        self.mint_tokens(
            &self.usdc_mint,
            &usdc_account,
            usdc_amount,
            &self.authority,
        ).await?;
        
        Ok(TestUser {
            keypair: user,
            usdc_account,
        })
    }

    /// Create a token account
    pub async fn create_token_account(
        &mut self,
        owner: &Keypair,
        mint: &Pubkey,
        account_owner: &Pubkey,
    ) -> Result<Pubkey, BanksClientError> {
        let token_account = Keypair::new();
        let rent = self.context.banks_client.get_rent().await?;
        let account_rent = rent.minimum_balance(TokenAccount::LEN);
        
        let create_account_ix = system_instruction::create_account(
            &owner.pubkey(),
            &token_account.pubkey(),
            account_rent,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        );
        
        let init_account_ix = token_instruction::initialize_account(
            &spl_token::id(),
            &token_account.pubkey(),
            mint,
            account_owner,
        )?;
        
        let mut transaction = Transaction::new_with_payer(
            &[create_account_ix, init_account_ix],
            Some(&owner.pubkey()),
        );
        transaction.sign(&[owner, &token_account], self.context.last_blockhash);
        self.context.banks_client.process_transaction(transaction).await?;
        
        Ok(token_account.pubkey())
    }

    /// Mint tokens
    pub async fn mint_tokens(
        &mut self,
        mint: &Pubkey,
        to: &Pubkey,
        amount: u64,
        mint_authority: &Keypair,
    ) -> Result<(), BanksClientError> {
        let mint_to_ix = token_instruction::mint_to(
            &spl_token::id(),
            mint,
            to,
            &mint_authority.pubkey(),
            &[],
            amount,
        )?;
        
        let mut transaction = Transaction::new_with_payer(
            &[mint_to_ix],
            Some(&self.context.payer.pubkey()),
        );
        transaction.sign(&[&self.context.payer, mint_authority], self.context.last_blockhash);
        self.context.banks_client.process_transaction(transaction).await
    }

    /// Process a transaction
    pub async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let mut transaction = Transaction::new_with_payer(
            instructions,
            Some(&self.context.payer.pubkey()),
        );
        
        let mut all_signers = vec![&self.context.payer];
        all_signers.extend(signers);
        
        transaction.sign(&all_signers, self.context.last_blockhash);
        self.context.banks_client.process_transaction(transaction).await
    }

    /// Airdrop SOL to an account
    pub async fn airdrop(&mut self, to: &Pubkey, lamports: u64) -> Result<(), BanksClientError> {
        let transfer_ix = system_instruction::transfer(
            &self.context.payer.pubkey(),
            to,
            lamports,
        );
        
        let mut transaction = Transaction::new_with_payer(
            &[transfer_ix],
            Some(&self.context.payer.pubkey()),
        );
        transaction.sign(&[&self.context.payer], self.context.last_blockhash);
        self.context.banks_client.process_transaction(transaction).await
    }

    /// Get account data
    pub async fn get_account(&mut self, address: &Pubkey) -> Result<Account, BanksClientError> {
        self.context.banks_client.get_account(*address).await?
            .ok_or(BanksClientError::ClientError("Account not found"))
    }

    /// Advance slots
    pub async fn advance_slots(&mut self, slots: u64) -> Result<(), BanksClientError> {
        let current_slot = self.context.banks_client.get_root_slot().await?;
        self.context.warp_to_slot(current_slot + slots).unwrap();
        Ok(())
    }
}

/// Test user with associated accounts
pub struct TestUser {
    pub keypair: Keypair,
    pub usdc_account: Pubkey,
}

impl TestUser {
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
}

/// Create a test market
pub async fn create_test_market(
    env: &mut TestEnvironment,
    market_id: u128,
    amm_type: &str,
) -> Result<Pubkey, BanksClientError> {
    match amm_type {
        "LMSR" => create_lmsr_market(env, market_id).await,
        "PM-AMM" => create_pmamm_pool(env, market_id).await,
        "L2-AMM" => create_l2amm_pool(env, market_id).await,
        _ => Err(BanksClientError::ClientError("Invalid AMM type")),
    }
}

/// Create LMSR market
async fn create_lmsr_market(
    env: &mut TestEnvironment,
    market_id: u128,
) -> Result<Pubkey, BanksClientError> {
    let (market_pda, _) = LmsrMarketPDA::derive(&env.program_id, market_id);
    
    let accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(market_pda, false),
        AccountMeta::new_readonly(Pubkey::default(), false), // Oracle
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::InitializeLmsrMarket {
            market_id,
            b_parameter: 1_000_000_000, // 1000 USDC
            num_outcomes: 2,
        }.try_to_vec().unwrap(),
    };
    
    env.process_transaction(&[instruction], &[]).await?;
    Ok(market_pda)
}

/// Create PM-AMM pool
async fn create_pmamm_pool(
    env: &mut TestEnvironment,
    pool_id: u128,
) -> Result<Pubkey, BanksClientError> {
    let (pool_pda, _) = PmammPoolPDA::derive(&env.program_id, pool_id);
    
    // Create LP mint
    let lp_mint = Keypair::new();
    let rent = env.context.banks_client.get_rent().await?;
    let mint_rent = rent.minimum_balance(Mint::LEN);
    
    let create_mint_ix = system_instruction::create_account(
        &env.context.payer.pubkey(),
        &lp_mint.pubkey(),
        mint_rent,
        Mint::LEN as u64,
        &spl_token::id(),
    );
    
    let init_mint_ix = token_instruction::initialize_mint(
        &spl_token::id(),
        &lp_mint.pubkey(),
        &pool_pda,
        None,
        6,
    )?;
    
    let mut transaction = Transaction::new_with_payer(
        &[create_mint_ix, init_mint_ix],
        Some(&env.context.payer.pubkey()),
    );
    transaction.sign(&[&env.context.payer, &lp_mint], env.context.last_blockhash);
    env.context.banks_client.process_transaction(transaction).await?;
    
    // Create LP token account for initializer
    let lp_token_account = env.create_token_account(
        &env.context.payer,
        &lp_mint.pubkey(),
        &env.context.payer.pubkey(),
    ).await?;
    
    let accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new(lp_mint.pubkey(), false),
        AccountMeta::new(lp_token_account, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::InitializePMAMM {
            pool_id,
            num_outcomes: 3,
            initial_amounts: vec![1_000_000_000, 1_000_000_000, 1_000_000_000],
        }.try_to_vec().unwrap(),
    };
    
    env.process_transaction(&[instruction], &[]).await?;
    Ok(pool_pda)
}

/// Create L2-AMM pool
async fn create_l2amm_pool(
    env: &mut TestEnvironment,
    pool_id: u128,
) -> Result<Pubkey, BanksClientError> {
    let (pool_pda, _) = L2ammPoolPDA::derive(&env.program_id, pool_id);
    
    let accounts = vec![
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(Pubkey::default(), false), // Oracle
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];
    
    let instruction = Instruction {
        program_id: env.program_id,
        accounts,
        data: BettingPlatformInstruction::InitializeL2AMM {
            pool_id,
            min_value: 0,
            max_value: 1_000_000,
            num_bins: 20,
            liquidity_parameter: 10_000_000_000,
        }.try_to_vec().unwrap(),
    };
    
    env.process_transaction(&[instruction], &[]).await?;
    Ok(pool_pda)
}

/// Assert account balance
pub async fn assert_token_balance(
    env: &mut TestEnvironment,
    token_account: &Pubkey,
    expected_amount: u64,
) -> Result<(), BanksClientError> {
    let account = env.get_account(token_account).await?;
    let token_account_data = TokenAccount::unpack(&account.data)?;
    assert_eq!(token_account_data.amount, expected_amount);
    Ok(())
}

/// Assert SOL balance
pub async fn assert_sol_balance(
    env: &mut TestEnvironment,
    pubkey: &Pubkey,
    expected_lamports: u64,
) -> Result<(), BanksClientError> {
    let account = env.get_account(pubkey).await?;
    assert_eq!(account.lamports, expected_lamports);
    Ok(())
}