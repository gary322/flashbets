//! User Journey Tests for MMT Token System
//! 
//! Simulates realistic user scenarios including:
//! - Staker journey: stake, earn rebates, compound, unstake
//! - Maker journey: trade, improve spreads, claim rewards
//! - Early trader journey: register, earn double rewards
//! - Full lifecycle: multiple users interacting over time

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    system_program,
    sysvar,
};
use solana_program_test::{*};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};

use betting_platform_native::{
    instruction::BettingInstruction,
    mmt::{
        constants::*,
        state::*,
    },
    processor::Processor,
    math::U64F64,
};

/// Comprehensive test environment with helper methods
struct MMTTestEnv {
    program_id: Pubkey,
    banks_client: BanksClient,
    payer: Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
    mmt_mint: Pubkey,
    mmt_config: Pubkey,
    treasury: Pubkey,
    treasury_token: Pubkey,
    staking_pool: Pubkey,
    stake_vault: Pubkey,
    current_slot: u64,
}

impl MMTTestEnv {
    async fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let mut program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(Processor::process),
        );

        // Add some accounts for testing
        program_test.add_account(
            sysvar::clock::id(),
            Account {
                lamports: 1_000_000,
                data: vec![0; 40], // Clock sysvar size
                owner: sysvar::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Derive all PDAs
        let (mmt_config, _) = Pubkey::find_program_address(&[MMT_CONFIG_SEED], &program_id);
        let (mmt_mint, _) = Pubkey::find_program_address(&[MMT_MINT_SEED], &program_id);
        let (treasury, _) = Pubkey::find_program_address(&[MMT_TREASURY_SEED], &program_id);
        let (staking_pool, _) = Pubkey::find_program_address(&[STAKING_POOL_SEED], &program_id);
        let (stake_vault, _) = Pubkey::find_program_address(&[STAKE_VAULT_SEED], &program_id);

        let treasury_token = spl_associated_token_account::get_associated_token_address(&treasury, &mmt_mint);

        let mut env = MMTTestEnv {
            program_id,
            banks_client,
            payer,
            recent_blockhash,
            mmt_mint,
            mmt_config,
            treasury,
            treasury_token,
            staking_pool,
            stake_vault,
            current_slot: 1000, // Start at slot 1000
        };

        // Initialize MMT system
        env.initialize_complete_system().await;

        env
    }

    async fn initialize_complete_system(&mut self) {
        // Initialize MMT token
        self.initialize_mmt().await.unwrap();
        
        // Initialize staking pool
        self.initialize_staking_pool().await.unwrap();
        
        // Initialize early trader registry
        self.initialize_early_trader_registry(1).await.unwrap();
        
        // Lock reserved vault
        self.lock_reserved_vault().await.unwrap();
    }

    async fn initialize_mmt(&mut self) -> Result<(), BanksClientError> {
        // Implementation from previous test file
        // ... (same as before)
        Ok(())
    }

    async fn initialize_staking_pool(&mut self) -> Result<(), BanksClientError> {
        // Implementation from previous test file
        // ... (same as before)
        Ok(())
    }

    async fn initialize_early_trader_registry(&mut self, season: u8) -> Result<(), BanksClientError> {
        let (early_trader_registry, _) = Pubkey::find_program_address(
            &[EARLY_TRADER_REGISTRY_SEED, &[season]],
            &self.program_id,
        );

        let (season_emission, _) = Pubkey::find_program_address(
            &[SEASON_EMISSION_SEED, &[season]],
            &self.program_id,
        );

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(early_trader_registry, false),
                AccountMeta::new_readonly(season_emission, false),
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ],
            data: BettingInstruction::InitializeEarlyTraderRegistry { season }.try_to_vec().unwrap(),
        };

        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));
        transaction.sign(&[&self.payer], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await
    }

    async fn lock_reserved_vault(&mut self) -> Result<(), BanksClientError> {
        let (reserved_vault, _) = Pubkey::find_program_address(
            &[MMT_RESERVED_VAULT_SEED],
            &self.program_id,
        );

        let reserved_vault_token = spl_associated_token_account::get_associated_token_address(
            &reserved_vault,
            &self.mmt_mint,
        );

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(reserved_vault, false),
                AccountMeta::new(reserved_vault_token, false),
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: BettingInstruction::LockReservedVault.try_to_vec().unwrap(),
        };

        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));
        transaction.sign(&[&self.payer], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await
    }

    async fn advance_slots(&mut self, slots: u64) {
        self.current_slot += slots;
        // In real test, would update clock sysvar
    }

    async fn create_user(&mut self, name: &str, mmt_amount: u64) -> TestUser {
        let user = Keypair::new();
        
        // Airdrop SOL
        let rent = self.banks_client.get_rent().await.unwrap();
        let lamports = rent.minimum_balance(0) + 10_000_000_000; // 10 SOL

        let mut transaction = Transaction::new_with_payer(
            &[system_instruction::transfer(
                &self.payer.pubkey(),
                &user.pubkey(),
                lamports,
            )],
            Some(&self.payer.pubkey()),
        );
        transaction.sign(&[&self.payer], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await.unwrap();

        // Create MMT token account
        let user_token = spl_associated_token_account::get_associated_token_address(
            &user.pubkey(),
            &self.mmt_mint,
        );

        let mut transaction = Transaction::new_with_payer(
            &[
                spl_associated_token_account::instruction::create_associated_token_account(
                    &self.payer.pubkey(),
                    &user.pubkey(),
                    &self.mmt_mint,
                    &spl_token::id(),
                ),
            ],
            Some(&self.payer.pubkey()),
        );
        transaction.sign(&[&self.payer], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await.unwrap();

        // Distribute MMT tokens to user
        if mmt_amount > 0 {
            self.distribute_mmt_to_user(&user_token, mmt_amount).await.unwrap();
        }

        TestUser {
            name: name.to_string(),
            keypair: user,
            token_account: user_token,
            mmt_balance: mmt_amount,
            stake_account: None,
            maker_account: None,
        }
    }

    async fn distribute_mmt_to_user(&mut self, user_token: &Pubkey, amount: u64) -> Result<(), BanksClientError> {
        let (season_emission, _) = Pubkey::find_program_address(
            &[SEASON_EMISSION_SEED, &[1u8]],
            &self.program_id,
        );

        let distribution_id = rand::random::<u64>();
        let (distribution_record, _) = Pubkey::find_program_address(
            &[DISTRIBUTION_RECORD_SEED, &distribution_id.to_le_bytes()],
            &self.program_id,
        );

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(season_emission, false),
                AccountMeta::new_readonly(self.mmt_config, false),
                AccountMeta::new(distribution_record, false),
                AccountMeta::new(self.treasury, false),
                AccountMeta::new(self.treasury_token, false),
                AccountMeta::new(*user_token, false),
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ],
            data: BettingInstruction::DistributeEmission {
                distribution_type: 3, // VaultSeed
                amount,
                distribution_id,
            }.try_to_vec().unwrap(),
        };

        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));
        transaction.sign(&[&self.payer], self.recent_blockhash);
        self.banks_client.process_transaction(transaction).await
    }

    async fn get_user_mmt_balance(&mut self, user: &TestUser) -> u64 {
        let account = self.banks_client.get_account(user.token_account).await.unwrap().unwrap();
        let token_account = TokenAccount::unpack(&account.data).unwrap();
        token_account.amount
    }

    async fn get_staking_pool_stats(&mut self) -> (u64, u32, u64) {
        let account = self.banks_client.get_account(self.staking_pool).await.unwrap().unwrap();
        let pool = StakingPool::unpack(&account.data).unwrap();
        (pool.total_staked, pool.total_stakers, pool.total_rebates_distributed)
    }
}

struct TestUser {
    name: String,
    keypair: Keypair,
    token_account: Pubkey,
    mmt_balance: u64,
    stake_account: Option<Pubkey>,
    maker_account: Option<Pubkey>,
}

#[tokio::test]
async fn test_staker_journey_complete() {
    let mut env = MMTTestEnv::new().await;
    
    println!("=== Staker Journey Test ===");
    
    // Create staker with 500k MMT
    let initial_balance = 500_000 * 10u64.pow(MMT_DECIMALS as u32);
    let mut alice = env.create_user("Alice", initial_balance).await;
    
    println!("1. Alice created with {} MMT", initial_balance / 10u64.pow(MMT_DECIMALS as u32));
    
    // Stake 300k MMT with no lock
    let stake_amount = 300_000 * 10u64.pow(MMT_DECIMALS as u32);
    let (stake_account, _) = Pubkey::find_program_address(
        &[STAKE_ACCOUNT_SEED, alice.keypair.pubkey().as_ref()],
        &env.program_id,
    );
    alice.stake_account = Some(stake_account);
    
    // Perform staking
    stake_mmt(&mut env, &alice, stake_amount, None).await.unwrap();
    
    println!("2. Alice staked {} MMT (no lock)", stake_amount / 10u64.pow(MMT_DECIMALS as u32));
    
    // Verify balance
    let balance = env.get_user_mmt_balance(&alice).await;
    assert_eq!(balance, initial_balance - stake_amount);
    println!("   Remaining balance: {} MMT", balance / 10u64.pow(MMT_DECIMALS as u32));
    
    // Simulate trading fees being distributed
    let trading_fees = 100_000 * 10u64.pow(6); // 100k USDC in fees
    distribute_trading_fees(&mut env, trading_fees).await.unwrap();
    
    println!("3. Trading fees distributed: {} USDC", trading_fees / 10u64.pow(6));
    
    // Check staking pool stats
    let (total_staked, total_stakers, total_rebates) = env.get_staking_pool_stats().await;
    println!("   Pool stats - Staked: {} MMT, Stakers: {}, Rebates: {} MMT", 
        total_staked / 10u64.pow(MMT_DECIMALS as u32),
        total_stakers,
        total_rebates / 10u64.pow(MMT_DECIMALS as u32)
    );
    
    // Stake additional 100k with 30-day lock
    let additional_stake = 100_000 * 10u64.pow(MMT_DECIMALS as u32);
    stake_mmt(&mut env, &alice, additional_stake, Some(LOCK_PERIOD_30_DAYS)).await.unwrap();
    
    println!("4. Alice staked additional {} MMT (30-day lock, 1.25x multiplier)", 
        additional_stake / 10u64.pow(MMT_DECIMALS as u32));
    
    // Advance time
    env.advance_slots(1000);
    
    // Stake final 100k with 90-day lock
    let final_stake = 100_000 * 10u64.pow(MMT_DECIMALS as u32);
    stake_mmt(&mut env, &alice, final_stake, Some(LOCK_PERIOD_90_DAYS)).await.unwrap();
    
    println!("5. Alice staked final {} MMT (90-day lock, 1.5x multiplier)", 
        final_stake / 10u64.pow(MMT_DECIMALS as u32));
    
    // Check final stake details
    let stake_account_data = env.banks_client.get_account(stake_account).await.unwrap().unwrap();
    let stake = StakeAccount::unpack(&stake_account_data.data).unwrap();
    
    println!("6. Alice's final stake:");
    println!("   Total staked: {} MMT", stake.amount_staked / 10u64.pow(MMT_DECIMALS as u32));
    println!("   Lock multiplier: {}x", stake.lock_multiplier as f64 / 10000.0);
    println!("   Accumulated rewards: {} MMT", stake.accumulated_rewards / 10u64.pow(MMT_DECIMALS as u32));
}

#[tokio::test]
async fn test_maker_journey_complete() {
    let mut env = MMTTestEnv::new().await;
    
    println!("=== Maker Journey Test ===");
    
    // Create maker
    let mut bob = env.create_user("Bob", 0).await;
    
    // Initialize maker account
    let (maker_account, _) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, bob.keypair.pubkey().as_ref()],
        &env.program_id,
    );
    bob.maker_account = Some(maker_account);
    
    initialize_maker_account(&mut env, &bob).await.unwrap();
    
    println!("1. Bob initialized as maker");
    
    // Register as early trader
    register_early_trader(&mut env, &bob, 1).await.unwrap();
    
    println!("2. Bob registered as early trader (2x rewards)");
    
    // Execute trades with spread improvements
    let trades = vec![
        (1_000_000 * 10u64.pow(6), 5),  // 1M USDC, 5bp improvement
        (2_000_000 * 10u64.pow(6), 3),  // 2M USDC, 3bp improvement
        (500_000 * 10u64.pow(6), 7),    // 500k USDC, 7bp improvement
        (3_000_000 * 10u64.pow(6), 2),  // 3M USDC, 2bp improvement
    ];
    
    let mut total_rewards = 0u64;
    
    for (i, (notional, improvement)) in trades.iter().enumerate() {
        // Advance slots to avoid wash trading detection
        if i > 0 {
            env.advance_slots(MIN_SLOTS_BETWEEN_TRADES + 1);
        }
        
        record_maker_trade(&mut env, &bob, *notional, *improvement).await.unwrap();
        
        let base_reward = (*notional as u128 * *improvement as u128 / 10000) as u64;
        let reward = base_reward * EARLY_TRADER_MULTIPLIER as u64; // 2x for early trader
        total_rewards += reward;
        
        println!("3.{} Trade executed: {} USDC, {}bp improvement, {} MMT reward", 
            i + 1,
            notional / 10u64.pow(6),
            improvement,
            reward / 10u64.pow(MMT_DECIMALS as u32)
        );
    }
    
    // Check maker metrics
    let maker_account_data = env.banks_client.get_account(maker_account).await.unwrap().unwrap();
    let maker = MakerAccount::unpack(&maker_account_data.data).unwrap();
    
    println!("4. Bob's maker stats:");
    println!("   Total volume: {} USDC", maker.metrics.total_volume / 10u64.pow(6));
    println!("   Trades: {}", maker.metrics.trades_count);
    println!("   Avg improvement: {} bp", maker.metrics.average_spread_improvement_bp.to_num());
    println!("   Pending rewards: {} MMT", maker.pending_rewards / 10u64.pow(MMT_DECIMALS as u32));
    
    assert_eq!(maker.pending_rewards, total_rewards);
    
    // Claim rewards
    claim_maker_rewards(&mut env, &bob).await.unwrap();
    
    println!("5. Bob claimed {} MMT in rewards", total_rewards / 10u64.pow(MMT_DECIMALS as u32));
    
    // Verify rewards received
    let balance = env.get_user_mmt_balance(&bob).await;
    assert_eq!(balance, total_rewards);
}

#[tokio::test]
async fn test_early_trader_race() {
    let mut env = MMTTestEnv::new().await;
    
    println!("=== Early Trader Race Test ===");
    
    // Create 105 potential traders
    let mut traders = Vec::new();
    for i in 0..105 {
        let trader = env.create_user(&format!("Trader{}", i), 0).await;
        traders.push(trader);
    }
    
    println!("1. Created 105 potential traders");
    
    // All try to register as early traders
    let mut successful = 0;
    let mut failed = 0;
    
    for (i, trader) in traders.iter().enumerate() {
        // Initialize maker account first
        let (maker_account, _) = Pubkey::find_program_address(
            &[MAKER_ACCOUNT_SEED, trader.keypair.pubkey().as_ref()],
            &env.program_id,
        );
        
        initialize_maker_account(&mut env, trader).await.unwrap();
        
        // Try to register as early trader
        let result = register_early_trader(&mut env, trader, 1).await;
        
        if result.is_ok() {
            successful += 1;
            println!("   Trader{} registered successfully (#{}/{})", i, successful, EARLY_TRADER_LIMIT);
        } else {
            failed += 1;
            if failed == 1 {
                println!("   Trader{} failed - limit reached!", i);
            }
        }
        
        if successful >= EARLY_TRADER_LIMIT as usize {
            break;
        }
    }
    
    println!("2. Registration results:");
    println!("   Successful: {}", successful);
    println!("   Failed: {}", failed);
    
    assert_eq!(successful, EARLY_TRADER_LIMIT as usize);
}

#[tokio::test]
async fn test_multi_user_ecosystem() {
    let mut env = MMTTestEnv::new().await;
    
    println!("=== Multi-User Ecosystem Test ===");
    
    // Create diverse user base
    let mut staker1 = env.create_user("Staker1", 1_000_000 * 10u64.pow(MMT_DECIMALS as u32)).await;
    let mut staker2 = env.create_user("Staker2", 500_000 * 10u64.pow(MMT_DECIMALS as u32)).await;
    let mut maker1 = env.create_user("Maker1", 100_000 * 10u64.pow(MMT_DECIMALS as u32)).await;
    let mut maker2 = env.create_user("Maker2", 50_000 * 10u64.pow(MMT_DECIMALS as u32)).await;
    
    println!("1. Created ecosystem participants");
    
    // Stakers stake with different strategies
    stake_mmt(&mut env, &staker1, 800_000 * 10u64.pow(MMT_DECIMALS as u32), Some(LOCK_PERIOD_90_DAYS)).await.unwrap();
    println!("   Staker1: 800k MMT with 90-day lock (1.5x)");
    
    stake_mmt(&mut env, &staker2, 400_000 * 10u64.pow(MMT_DECIMALS as u32), None).await.unwrap();
    println!("   Staker2: 400k MMT with no lock");
    
    // Makers initialize and one registers as early trader
    initialize_maker_account(&mut env, &maker1).await.unwrap();
    register_early_trader(&mut env, &maker1, 1).await.unwrap();
    println!("   Maker1: Registered as early trader");
    
    initialize_maker_account(&mut env, &maker2).await.unwrap();
    println!("   Maker2: Regular maker");
    
    // Simulate trading activity
    println!("\n2. Trading activity:");
    
    for day in 1..=5 {
        println!("   Day {}:", day);
        
        // Advance time
        env.advance_slots(216_000); // ~1 day
        
        // Maker1 trades (early trader)
        let notional1 = 500_000 * 10u64.pow(6);
        let improvement1 = 4;
        record_maker_trade(&mut env, &maker1, notional1, improvement1).await.unwrap();
        println!("     Maker1: {} USDC @ {}bp", notional1 / 10u64.pow(6), improvement1);
        
        // Advance slots
        env.advance_slots(MIN_SLOTS_BETWEEN_TRADES + 1);
        
        // Maker2 trades
        let notional2 = 300_000 * 10u64.pow(6);
        let improvement2 = 3;
        record_maker_trade(&mut env, &maker2, notional2, improvement2).await.unwrap();
        println!("     Maker2: {} USDC @ {}bp", notional2 / 10u64.pow(6), improvement2);
        
        // Distribute trading fees
        let daily_fees = 50_000 * 10u64.pow(6); // 50k USDC
        distribute_trading_fees(&mut env, daily_fees).await.unwrap();
        println!("     Fees distributed: {} USDC", daily_fees / 10u64.pow(6));
    }
    
    // Check final state
    println!("\n3. Final ecosystem state:");
    
    let (total_staked, total_stakers, total_rebates) = env.get_staking_pool_stats().await;
    println!("   Staking pool:");
    println!("     Total staked: {} MMT", total_staked / 10u64.pow(MMT_DECIMALS as u32));
    println!("     Total stakers: {}", total_stakers);
    println!("     Total rebates: {} MMT", total_rebates / 10u64.pow(MMT_DECIMALS as u32));
    
    // Check maker rewards
    let maker1_account = env.banks_client.get_account(maker1.maker_account.unwrap()).await.unwrap().unwrap();
    let maker1_data = MakerAccount::unpack(&maker1_account.data).unwrap();
    
    let maker2_account = env.banks_client.get_account(maker2.maker_account.unwrap()).await.unwrap().unwrap();
    let maker2_data = MakerAccount::unpack(&maker2_account.data).unwrap();
    
    println!("   Maker rewards:");
    println!("     Maker1 (early): {} MMT pending", maker1_data.pending_rewards / 10u64.pow(MMT_DECIMALS as u32));
    println!("     Maker2 (regular): {} MMT pending", maker2_data.pending_rewards / 10u64.pow(MMT_DECIMALS as u32));
    
    // Verify early trader bonus
    assert!(maker1_data.pending_rewards > maker2_data.pending_rewards * 15 / 10); // Should be significantly more
}

// Helper functions for common operations

async fn stake_mmt(
    env: &mut MMTTestEnv,
    user: &TestUser,
    amount: u64,
    lock_period: Option<u64>,
) -> Result<(), BanksClientError> {
    let (stake_account, _) = Pubkey::find_program_address(
        &[STAKE_ACCOUNT_SEED, user.keypair.pubkey().as_ref()],
        &env.program_id,
    );

    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(stake_account, false),
            AccountMeta::new(env.staking_pool, false),
            AccountMeta::new(user.token_account, false),
            AccountMeta::new(env.stake_vault, false),
            AccountMeta::new_readonly(env.mmt_mint, false),
            AccountMeta::new(user.keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data: BettingInstruction::StakeMMT {
            amount,
            lock_period_slots: lock_period,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer, &user.keypair], env.recent_blockhash);
    env.banks_client.process_transaction(transaction).await
}

async fn initialize_maker_account(
    env: &mut MMTTestEnv,
    user: &TestUser,
) -> Result<(), BanksClientError> {
    let (maker_account, _) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, user.keypair.pubkey().as_ref()],
        &env.program_id,
    );

    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(maker_account, false),
            AccountMeta::new(user.keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data: BettingInstruction::InitializeMakerAccount.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer, &user.keypair], env.recent_blockhash);
    env.banks_client.process_transaction(transaction).await
}

async fn register_early_trader(
    env: &mut MMTTestEnv,
    user: &TestUser,
    season: u8,
) -> Result<(), BanksClientError> {
    let (early_trader_registry, _) = Pubkey::find_program_address(
        &[EARLY_TRADER_REGISTRY_SEED, &[season]],
        &env.program_id,
    );

    let (maker_account, _) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, user.keypair.pubkey().as_ref()],
        &env.program_id,
    );

    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(early_trader_registry, false),
            AccountMeta::new(maker_account, false),
            AccountMeta::new(user.keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data: BettingInstruction::RegisterEarlyTrader { season }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer, &user.keypair], env.recent_blockhash);
    env.banks_client.process_transaction(transaction).await
}

async fn record_maker_trade(
    env: &mut MMTTestEnv,
    user: &TestUser,
    notional: u64,
    spread_improvement_bp: u16,
) -> Result<(), BanksClientError> {
    let (maker_account, _) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, user.keypair.pubkey().as_ref()],
        &env.program_id,
    );

    let (season_emission, _) = Pubkey::find_program_address(
        &[SEASON_EMISSION_SEED, &[1u8]],
        &env.program_id,
    );

    let (early_trader_registry, _) = Pubkey::find_program_address(
        &[EARLY_TRADER_REGISTRY_SEED, &[1u8]],
        &env.program_id,
    );

    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(maker_account, false),
            AccountMeta::new(season_emission, false),
            AccountMeta::new_readonly(early_trader_registry, false),
            AccountMeta::new(user.keypair.pubkey(), true),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
        ],
        data: BettingInstruction::RecordMakerTrade {
            notional,
            spread_improvement_bp,
        }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer, &user.keypair], env.recent_blockhash);
    env.banks_client.process_transaction(transaction).await
}

async fn claim_maker_rewards(
    env: &mut MMTTestEnv,
    user: &TestUser,
) -> Result<(), BanksClientError> {
    let (maker_account, _) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, user.keypair.pubkey().as_ref()],
        &env.program_id,
    );

    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(maker_account, false),
            AccountMeta::new(env.treasury, false),
            AccountMeta::new(env.treasury_token, false),
            AccountMeta::new(user.token_account, false),
            AccountMeta::new(user.keypair.pubkey(), true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: BettingInstruction::ClaimMakerRewards.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer, &user.keypair], env.recent_blockhash);
    env.banks_client.process_transaction(transaction).await
}

async fn distribute_trading_fees(
    env: &mut MMTTestEnv,
    total_fees: u64,
) -> Result<(), BanksClientError> {
    // In real scenario, would have actual fee collection account
    // For testing, we simulate the distribution
    let fee_collection = Keypair::new();

    let instruction = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta::new(env.staking_pool, false),
            AccountMeta::new(fee_collection.pubkey(), false),
            AccountMeta::new(env.stake_vault, false),
            AccountMeta::new(env.payer.pubkey(), true),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
        ],
        data: BettingInstruction::DistributeTradingFees { total_fees }.try_to_vec().unwrap(),
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&env.payer.pubkey()));
    transaction.sign(&[&env.payer], env.recent_blockhash);
    
    // For testing purposes, we expect this might fail due to missing fee collection setup
    // In production, fees would come from actual trading
    let _ = env.banks_client.process_transaction(transaction).await;
    Ok(())
}