//! Integration tests for MMT token lifecycle and PM-AMM with tables

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    clock::Clock,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
    transport::TransportError,
};
use betting_platform_native::{
    instruction::*,
    state::{
        mmt_state::*,
        amm_accounts::PMAMMMarket,
    },
    math::{U64F64, tables::NormalDistributionTables},
};
use borsh::BorshDeserialize;

/// Test environment setup
struct TestEnvironment {
    context: ProgramTestContext,
    mmt_config: Pubkey,
    staking_pool: Pubkey,
    normal_tables: Pubkey,
    treasury: Pubkey,
    maker_registry: Pubkey,
}

impl TestEnvironment {
    async fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let mut program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(betting_platform_native::processor::process_instruction),
        );

        // Add system accounts
        program_test.add_account(
            Pubkey::new_unique(),
            Account {
                lamports: 1_000_000_000,
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        let mut context = program_test.start_with_context().await;

        // Derive PDAs
        let (mmt_config, _) = Pubkey::find_program_address(&[b"mmt_config"], &program_id);
        let (staking_pool, _) = Pubkey::find_program_address(&[b"staking_pool"], &program_id);
        let (normal_tables, _) = Pubkey::find_program_address(&[b"normal_tables"], &program_id);
        let (treasury, _) = Pubkey::find_program_address(&[b"mmt_treasury"], &program_id);
        let (maker_registry, _) = Pubkey::find_program_address(&[b"maker_registry"], &program_id);

        Self {
            context,
            mmt_config,
            staking_pool,
            normal_tables,
            treasury,
            maker_registry,
        }
    }

    /// Initialize MMT token system
    async fn initialize_mmt(&mut self) -> Result<(), TransportError> {
        let instruction = BettingPlatformInstruction::InitializeMMTToken;
        
        let accounts = vec![
            AccountMeta::new(self.mmt_config, false),
            AccountMeta::new(self.context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(rent::id(), false),
        ];

        let transaction = Transaction::new_signed_with_payer(
            &[Instruction::new_with_borsh(
                self.context.payer.pubkey(),
                &instruction,
                accounts,
            )],
            Some(&self.context.payer.pubkey()),
            &[&self.context.payer],
            self.context.last_blockhash,
        );

        self.context.banks_client.process_transaction(transaction).await
    }

    /// Initialize staking pool
    async fn initialize_staking_pool(&mut self) -> Result<(), TransportError> {
        let instruction = BettingPlatformInstruction::InitializeStakingPool;
        
        let accounts = vec![
            AccountMeta::new(self.staking_pool, false),
            AccountMeta::new_readonly(self.mmt_config, false),
            AccountMeta::new(self.context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ];

        let transaction = Transaction::new_signed_with_payer(
            &[Instruction::new_with_borsh(
                self.context.payer.pubkey(),
                &instruction,
                accounts,
            )],
            Some(&self.context.payer.pubkey()),
            &[&self.context.payer],
            self.context.last_blockhash,
        );

        self.context.banks_client.process_transaction(transaction).await
    }

    /// Initialize normal distribution tables
    async fn initialize_tables(&mut self) -> Result<(), TransportError> {
        let instruction = BettingPlatformInstruction::InitializeNormalTables;
        
        let accounts = vec![
            AccountMeta::new(self.normal_tables, false),
            AccountMeta::new(self.context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ];

        let transaction = Transaction::new_signed_with_payer(
            &[Instruction::new_with_borsh(
                self.context.payer.pubkey(),
                &instruction,
                accounts,
            )],
            Some(&self.context.payer.pubkey()),
            &[&self.context.payer],
            self.context.last_blockhash,
        );

        self.context.banks_client.process_transaction(transaction).await
    }

    /// Populate tables with values
    async fn populate_tables(&mut self) -> Result<(), TransportError> {
        // Generate table values
        let table_values = generate_table_values();
        
        // Populate in chunks to avoid transaction size limits
        let chunk_size = 50;
        for (i, chunk) in table_values.chunks(chunk_size).enumerate() {
            let instruction = BettingPlatformInstruction::PopulateTablesChunk {
                start_index: i * chunk_size,
                values: chunk.to_vec(),
            };
            
            let accounts = vec![
                AccountMeta::new(self.normal_tables, false),
                AccountMeta::new_readonly(self.context.payer.pubkey(), true),
            ];

            let transaction = Transaction::new_signed_with_payer(
                &[Instruction::new_with_borsh(
                    self.context.payer.pubkey(),
                    &instruction,
                    accounts,
                )],
                Some(&self.context.payer.pubkey()),
                &[&self.context.payer],
                self.context.last_blockhash,
            );

            self.context.banks_client.process_transaction(transaction).await?;
        }
        
        Ok(())
    }

    /// Create a test user with MMT tokens
    async fn create_user_with_mmt(&mut self, amount: u64) -> Result<Keypair, TransportError> {
        let user = Keypair::new();
        
        // Airdrop SOL for fees
        let airdrop_tx = system_instruction::transfer(
            &self.context.payer.pubkey(),
            &user.pubkey(),
            1_000_000_000, // 1 SOL
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[airdrop_tx],
            Some(&self.context.payer.pubkey()),
            &[&self.context.payer],
            self.context.last_blockhash,
        );
        
        self.context.banks_client.process_transaction(transaction).await?;
        
        // TODO: Mint MMT tokens to user
        // This would involve calling the mint instruction with proper authority
        
        Ok(user)
    }
}

/// Integration test: Full MMT lifecycle
#[tokio::test]
async fn test_full_mmt_lifecycle() {
    let mut env = TestEnvironment::new().await;
    
    // Phase 1: System initialization
    println!("Phase 1: Initializing MMT system...");
    env.initialize_mmt().await.expect("Failed to initialize MMT");
    env.initialize_staking_pool().await.expect("Failed to initialize staking pool");
    env.initialize_tables().await.expect("Failed to initialize tables");
    env.populate_tables().await.expect("Failed to populate tables");
    
    // Verify initialization
    let mmt_config_account = env.context.banks_client
        .get_account(env.mmt_config)
        .await
        .expect("Failed to get MMT config")
        .expect("MMT config not found");
    
    let mmt_config = MMTConfig::try_from_slice(&mmt_config_account.data)
        .expect("Failed to deserialize MMT config");
    
    assert_eq!(mmt_config.total_supply, 100_000_000 * 10u64.pow(9));
    assert_eq!(mmt_config.season_allocation, 10_000_000 * 10u64.pow(9));
    assert_eq!(mmt_config.current_season, 1);
    
    // Phase 2: User onboarding and staking
    println!("Phase 2: User onboarding and staking...");
    let staker1 = env.create_user_with_mmt(100_000 * 10u64.pow(9)).await
        .expect("Failed to create staker1");
    let staker2 = env.create_user_with_mmt(50_000 * 10u64.pow(9)).await
        .expect("Failed to create staker2");
    
    // Stake MMT tokens
    let stake_instruction1 = BettingPlatformInstruction::StakeMMT {
        amount: 80_000 * 10u64.pow(9),
        lock_period_slots: None,
    };
    
    let stake_accounts1 = vec![
        AccountMeta::new(staker1.pubkey(), true),
        AccountMeta::new(env.staking_pool, false),
        AccountMeta::new_readonly(env.mmt_config, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let stake_tx1 = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &stake_instruction1,
            stake_accounts1,
        )],
        Some(&staker1.pubkey()),
        &[&staker1],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(stake_tx1).await
        .expect("Failed to stake MMT for staker1");
    
    // Phase 3: Market maker activity
    println!("Phase 3: Market maker activity...");
    let maker1 = env.create_user_with_mmt(10_000 * 10u64.pow(9)).await
        .expect("Failed to create maker1");
    
    // Register as early trader
    let register_instruction = BettingPlatformInstruction::RegisterEarlyTrader { season: 1 };
    let register_accounts = vec![
        AccountMeta::new(maker1.pubkey(), true),
        AccountMeta::new(env.maker_registry, false),
        AccountMeta::new_readonly(env.mmt_config, false),
    ];
    
    let register_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &register_instruction,
            register_accounts,
        )],
        Some(&maker1.pubkey()),
        &[&maker1],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(register_tx).await
        .expect("Failed to register early trader");
    
    // Record maker trades
    for i in 0..5 {
        let notional = 1_000_000 * (i + 1); // Increasing notional
        let spread_improvement = 2 + (i % 3); // 2-4 bp
        
        let trade_instruction = BettingPlatformInstruction::RecordMakerTrade {
            notional,
            spread_improvement_bp: spread_improvement,
        };
        
        let trade_accounts = vec![
            AccountMeta::new(maker1.pubkey(), true),
            AccountMeta::new(env.mmt_config, false),
            AccountMeta::new_readonly(env.treasury, false),
        ];
        
        let trade_tx = Transaction::new_signed_with_payer(
            &[Instruction::new_with_borsh(
                env.context.payer.pubkey(),
                &trade_instruction,
                trade_accounts,
            )],
            Some(&maker1.pubkey()),
            &[&maker1],
            env.context.last_blockhash,
        );
        
        env.context.banks_client.process_transaction(trade_tx).await
            .expect(&format!("Failed to record maker trade {}", i));
    }
    
    // Phase 4: Fee distribution
    println!("Phase 4: Fee distribution...");
    let total_fees = 10_000_000; // 10 USDC in fees
    
    let distribute_instruction = BettingPlatformInstruction::DistributeTradingFees { total_fees };
    let distribute_accounts = vec![
        AccountMeta::new(env.staking_pool, false),
        AccountMeta::new_readonly(env.mmt_config, false),
        AccountMeta::new_readonly(env.treasury, false),
    ];
    
    let distribute_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &distribute_instruction,
            distribute_accounts,
        )],
        Some(&env.context.payer.pubkey()),
        &[&env.context.payer],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(distribute_tx).await
        .expect("Failed to distribute trading fees");
    
    // Phase 5: PM-AMM trading with tables
    println!("Phase 5: PM-AMM trading with tables...");
    
    // Create PM-AMM market
    let market_id = 1u128;
    let pmamm_instruction = BettingPlatformInstruction::InitializePmammMarket {
        market_id,
        l_parameter: 10_000,
        expiry_time: Clock::get().unwrap().unix_timestamp + 86400 * 30, // 30 days
        initial_price: 5000, // 50%
    };
    
    let (pmamm_market, _) = Pubkey::find_program_address(
        &[b"pmamm_market", &market_id.to_le_bytes()],
        &env.context.payer.pubkey(),
    );
    
    let pmamm_accounts = vec![
        AccountMeta::new(pmamm_market, false),
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    
    let pmamm_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &pmamm_instruction,
            pmamm_accounts,
        )],
        Some(&env.context.payer.pubkey()),
        &[&env.context.payer],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(pmamm_tx).await
        .expect("Failed to create PM-AMM market");
    
    // Execute trades using tables
    let trader = env.create_user_with_mmt(50_000).await
        .expect("Failed to create trader");
    
    let trade_instruction = BettingPlatformInstruction::ExecutePmammTrade {
        outcome: 1,
        amount: 1000,
        is_buy: true,
    };
    
    let trade_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(pmamm_market, false),
        AccountMeta::new_readonly(env.normal_tables, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let trade_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &trade_instruction,
            trade_accounts,
        )],
        Some(&trader.pubkey()),
        &[&trader],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(trade_tx).await
        .expect("Failed to execute PM-AMM trade");
    
    // Verify market state after trade
    let market_account = env.context.banks_client
        .get_account(pmamm_market)
        .await
        .expect("Failed to get market account")
        .expect("Market account not found");
    
    let market = PMAMMMarket::try_from_slice(&market_account.data)
        .expect("Failed to deserialize market");
    
    assert!(market.total_volume > 0);
    
    println!("All integration tests passed!");
}

/// Integration test: Season transition
#[tokio::test]
async fn test_season_transition() {
    let mut env = TestEnvironment::new().await;
    
    // Initialize system
    env.initialize_mmt().await.expect("Failed to initialize MMT");
    
    // Fast forward time to end of season
    let slots_to_advance = 38_880_000; // 6 months worth of slots
    env.context.warp_to_slot(slots_to_advance).unwrap();
    
    // Transition season
    let transition_instruction = BettingPlatformInstruction::TransitionSeason;
    let transition_accounts = vec![
        AccountMeta::new(env.mmt_config, false),
        AccountMeta::new(env.context.payer.pubkey(), true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    
    let transition_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &transition_instruction,
            transition_accounts,
        )],
        Some(&env.context.payer.pubkey()),
        &[&env.context.payer],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(transition_tx).await
        .expect("Failed to transition season");
    
    // Verify season 2 is active
    let mmt_config_account = env.context.banks_client
        .get_account(env.mmt_config)
        .await
        .expect("Failed to get MMT config")
        .expect("MMT config not found");
    
    let mmt_config = MMTConfig::try_from_slice(&mmt_config_account.data)
        .expect("Failed to deserialize MMT config");
    
    assert_eq!(mmt_config.current_season, 2);
}

/// Integration test: Staking with lock periods
#[tokio::test]
async fn test_staking_with_locks() {
    let mut env = TestEnvironment::new().await;
    
    // Initialize system
    env.initialize_mmt().await.expect("Failed to initialize MMT");
    env.initialize_staking_pool().await.expect("Failed to initialize staking pool");
    
    // Create staker
    let staker = env.create_user_with_mmt(100_000 * 10u64.pow(9)).await
        .expect("Failed to create staker");
    
    // Stake with 90-day lock for 1.5x rewards
    let lock_period = 90 * 24 * 60 * 60 / 0.4; // 90 days in slots
    let stake_instruction = BettingPlatformInstruction::StakeMMT {
        amount: 50_000 * 10u64.pow(9),
        lock_period_slots: Some(lock_period as u64),
    };
    
    let stake_accounts = vec![
        AccountMeta::new(staker.pubkey(), true),
        AccountMeta::new(env.staking_pool, false),
        AccountMeta::new_readonly(env.mmt_config, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let stake_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &stake_instruction,
            stake_accounts,
        )],
        Some(&staker.pubkey()),
        &[&staker],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(stake_tx).await
        .expect("Failed to stake with lock");
    
    // Try to unstake before lock period (should fail)
    let unstake_instruction = BettingPlatformInstruction::UnstakeMMT {
        amount: 10_000 * 10u64.pow(9),
    };
    
    let unstake_accounts = vec![
        AccountMeta::new(staker.pubkey(), true),
        AccountMeta::new(env.staking_pool, false),
        AccountMeta::new_readonly(env.mmt_config, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    
    let unstake_tx = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &unstake_instruction,
            unstake_accounts,
        )],
        Some(&staker.pubkey()),
        &[&staker],
        env.context.last_blockhash,
    );
    
    let result = env.context.banks_client.process_transaction(unstake_tx).await;
    assert!(result.is_err(), "Unstaking should fail during lock period");
    
    // Fast forward past lock period
    env.context.warp_to_slot(lock_period as u64 + 1).unwrap();
    
    // Now unstaking should work
    let unstake_tx2 = Transaction::new_signed_with_payer(
        &[Instruction::new_with_borsh(
            env.context.payer.pubkey(),
            &unstake_instruction,
            unstake_accounts.clone(),
        )],
        Some(&staker.pubkey()),
        &[&staker],
        env.context.last_blockhash,
    );
    
    env.context.banks_client.process_transaction(unstake_tx2).await
        .expect("Failed to unstake after lock period");
}

/// Integration test: PM-AMM with different market conditions
#[tokio::test]
async fn test_pmamm_market_conditions() {
    let mut env = TestEnvironment::new().await;
    
    // Initialize tables
    env.initialize_tables().await.expect("Failed to initialize tables");
    env.populate_tables().await.expect("Failed to populate tables");
    
    // Test different market scenarios
    let scenarios = vec![
        (10_000, 86400 * 30, 5000),  // High liquidity, 30 days, 50% price
        (1_000, 86400 * 7, 7000),    // Low liquidity, 7 days, 70% price
        (5_000, 86400 * 1, 2000),    // Medium liquidity, 1 day, 20% price
    ];
    
    for (i, (liquidity, expiry, initial_price)) in scenarios.iter().enumerate() {
        let market_id = (i + 1) as u128;
        
        // Create market
        let create_instruction = BettingPlatformInstruction::InitializePmammMarket {
            market_id,
            l_parameter: *liquidity,
            expiry_time: Clock::get().unwrap().unix_timestamp + expiry,
            initial_price: *initial_price,
        };
        
        let (market_pubkey, _) = Pubkey::find_program_address(
            &[b"pmamm_market", &market_id.to_le_bytes()],
            &env.context.payer.pubkey(),
        );
        
        let create_accounts = vec![
            AccountMeta::new(market_pubkey, false),
            AccountMeta::new(env.context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        
        let create_tx = Transaction::new_signed_with_payer(
            &[Instruction::new_with_borsh(
                env.context.payer.pubkey(),
                &create_instruction,
                create_accounts,
            )],
            Some(&env.context.payer.pubkey()),
            &[&env.context.payer],
            env.context.last_blockhash,
        );
        
        env.context.banks_client.process_transaction(create_tx).await
            .expect(&format!("Failed to create market {}", i));
        
        // Execute trades
        let trader = env.create_user_with_mmt(10_000).await
            .expect("Failed to create trader");
        
        for j in 0..3 {
            let trade_instruction = BettingPlatformInstruction::ExecutePmammTrade {
                outcome: (j % 2) as u8,
                amount: 100 * (j + 1),
                is_buy: j % 2 == 0,
            };
            
            let trade_accounts = vec![
                AccountMeta::new(trader.pubkey(), true),
                AccountMeta::new(market_pubkey, false),
                AccountMeta::new_readonly(env.normal_tables, false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ];
            
            let trade_tx = Transaction::new_signed_with_payer(
                &[Instruction::new_with_borsh(
                    env.context.payer.pubkey(),
                    &trade_instruction,
                    trade_accounts,
                )],
                Some(&trader.pubkey()),
                &[&trader],
                env.context.last_blockhash,
            );
            
            env.context.banks_client.process_transaction(trade_tx).await
                .expect(&format!("Failed trade {} in market {}", j, i));
        }
    }
}

/// Helper function to generate table values
fn generate_table_values() -> Vec<TableValues> {
    let mut values = Vec::new();
    
    for i in 0..801 {
        let x = (i as f64 - 400.0) / 100.0;
        
        // Use approximations for testing
        let cdf = normal_cdf(x);
        let pdf = normal_pdf(x);
        let erf = erf_approx(x);
        
        values.push(TableValues {
            x: i as i32 - 400,
            cdf: U64F64::from_num((cdf * 10000.0) as u64),
            pdf: U64F64::from_num((pdf * 10000.0) as u64),
            erf: U64F64::from_num(((erf + 1.0) * 5000.0) as u64),
        });
    }
    
    values
}

// Math approximations for testing
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf_approx(x / std::f64::consts::SQRT_2))
}

fn normal_pdf(x: f64) -> f64 {
    (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-x * x / 2.0).exp()
}

fn erf_approx(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    
    sign * y
}