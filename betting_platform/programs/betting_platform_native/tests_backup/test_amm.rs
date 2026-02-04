//! Integration tests for AMM modules

use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use borsh::BorshDeserialize;

mod test_framework;
use test_framework::*;

use betting_platform_native::{
    instruction::{BettingPlatformInstruction, TradeParams},
    amm::pmamm::trade::SwapParams,
    pda::*,
    state::amm_accounts::*,
};

#[tokio::test]
async fn test_lmsr_market_lifecycle() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create LMSR market
    let market_id = 1u128;
    let market_pda = create_test_market(&mut env, market_id, "LMSR").await.unwrap();
    
    // Create trader
    let trader = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    // Buy outcome 0
    let buy_params = TradeParams {
        market_id,
        outcome: 0,
        is_buy: true,
        amount: 100_000_000, // 100 USDC amount for buying
        shares: Some(100_000_000), // 100 shares expected
        max_cost: Some(200_000_000), // Max 200 USDC
        min_shares: Some(50_000_000), // Min 50 shares acceptable
        min_payout: None,
        max_slippage_bps: Some(500), // 5% max slippage
    };
    
    let buy_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(market_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
    ];
    
    let buy_ix = Instruction {
        program_id: env.program_id,
        accounts: buy_accounts,
        data: BettingPlatformInstruction::TradeLMSR(buy_params)
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[buy_ix], &[&trader.keypair])
        .await
        .unwrap();
    
    // Check market state updated
    let market_account = env.get_account(&market_pda).await.unwrap();
    let market = LSMRMarket::try_from_slice(&market_account.data).unwrap();
    assert!(market.shares[0] > 0);
    assert!(market.total_volume > 0);
    
    // Sell shares back
    let sell_params = TradeParams {
        market_id,
        outcome: 0,
        is_buy: false,
        amount: 50_000_000, // 50 shares to sell
        shares: Some(50_000_000), // 50 shares to sell
        max_cost: None,
        min_shares: None,
        min_payout: Some(25_000_000), // Min 25 USDC expected
        max_slippage_bps: Some(500), // 5% max slippage
    };
    
    let sell_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(market_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
    ];
    
    let sell_ix = Instruction {
        program_id: env.program_id,
        accounts: sell_accounts,
        data: BettingPlatformInstruction::TradeLMSR(sell_params)
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[sell_ix], &[&trader.keypair])
        .await
        .unwrap();
    
    // Verify shares decreased
    let market_account = env.get_account(&market_pda).await.unwrap();
    let market = LSMRMarket::try_from_slice(&market_account.data).unwrap();
    assert_eq!(market.shares[0], 50_000_000);
}

#[tokio::test]
async fn test_pmamm_liquidity_and_trading() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create PM-AMM pool with 3 outcomes
    let pool_id = 2u128;
    let pool_pda = create_test_market(&mut env, pool_id, "PM-AMM").await.unwrap();
    
    // Create liquidity provider
    let lp = env.create_user_with_tokens(20_000_000_000).await.unwrap();
    
    // Add liquidity
    let add_liquidity_params = vec![2_000_000_000, 2_000_000_000, 2_000_000_000];
    
    let (lp_position_pda, _) = LpPositionPDA::derive(&env.program_id, &lp.pubkey(), pool_id);
    
    // Create LP mint and token account (simplified for test)
    let lp_mint = Keypair::new();
    let lp_token_account = env.create_token_account(
        &lp.keypair,
        &lp_mint.pubkey(),
        &lp.pubkey(),
    ).await.unwrap();
    
    let add_liq_accounts = vec![
        AccountMeta::new(lp.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new(lp_position_pda, false),
        AccountMeta::new(lp_mint.pubkey(), false),
        AccountMeta::new(lp_token_account, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
    ];
    
    let add_liq_ix = Instruction {
        program_id: env.program_id,
        accounts: add_liq_accounts,
        data: BettingPlatformInstruction::AddLiquidityPMAMM {
            pool_id,
            amounts: add_liquidity_params.clone(),
            min_lp_tokens: None,
        }
        .try_to_vec()
        .unwrap(),
    };
    
    env.process_transaction(&[add_liq_ix], &[&lp.keypair])
        .await
        .unwrap();
    
    // Create trader and perform swap
    let trader = env.create_user_with_tokens(5_000_000_000).await.unwrap();
    
    let swap_params = SwapParams {
        pool_id,
        outcome_in: 0,
        outcome_out: 1,
        amount_in: 500_000_000, // 500 USDC worth of outcome 0
        min_amount_out: Some(400_000_000), // Expect at least 400 of outcome 1
    };
    
    let swap_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
    ];
    
    let swap_ix = Instruction {
        program_id: env.program_id,
        accounts: swap_accounts,
        data: BettingPlatformInstruction::SwapPMAMM(swap_params)
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[swap_ix], &[&trader.keypair])
        .await
        .unwrap();
    
    // Verify pool reserves updated
    let pool_account = env.get_account(&pool_pda).await.unwrap();
    let pool = PMAMMMarket::try_from_slice(&pool_account.data).unwrap();
    assert!(pool.reserves[0] > add_liquidity_params[0]); // Outcome 0 increased
    assert!(pool.reserves[1] < add_liquidity_params[1]); // Outcome 1 decreased
}

#[tokio::test]
async fn test_l2amm_continuous_distribution() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create L2-AMM pool for continuous market (e.g., price prediction)
    let pool_id = 3u128;
    let pool_pda = create_test_market(&mut env, pool_id, "L2-AMM").await.unwrap();
    
    // Create trader
    let trader = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    // Buy range position [450, 550]
    let l2_trade_params = L2TradeParams {
        pool_id,
        lower_bound: 450_000, // $450
        upper_bound: 550_000, // $550
        shares: 1_000_000_000, // 1000 shares
        is_buy: true,
        max_cost: Some(1_500_000_000), // Max 1500 USDC
    };
    
    let trade_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
    ];
    
    let trade_ix = Instruction {
        program_id: env.program_id,
        accounts: trade_accounts,
        data: BettingPlatformInstruction::TradeL2AMM(l2_trade_params)
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[trade_ix], &[&trader.keypair])
        .await
        .unwrap();
    
    // Verify distribution updated
    let pool_account = env.get_account(&pool_pda).await.unwrap();
    let pool = L2AMMPool::try_from_slice(&pool_account.data).unwrap();
    
    // Check bins in range [450, 550] have increased weight
    let bin_width = (pool.max_value - pool.min_value) / pool.distribution.len() as u64;
    let start_bin = ((450_000 - pool.min_value) / bin_width) as usize;
    let end_bin = ((550_000 - pool.min_value) / bin_width) as usize;
    
    for i in start_bin..=end_bin {
        assert!(pool.distribution[i].weight > 0);
    }
}

#[tokio::test]
async fn test_hybrid_amm_routing() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create all three AMM types for the same market
    let market_id = 4u128;
    
    // Create LMSR for initial price discovery
    let lmsr_pda = create_test_market(&mut env, market_id, "LMSR").await.unwrap();
    
    // Create PM-AMM for mature trading
    let pmamm_pda = create_test_market(&mut env, market_id + 1000, "PM-AMM").await.unwrap();
    
    // Create L2-AMM for continuous outcomes
    let l2amm_pda = create_test_market(&mut env, market_id + 2000, "L2-AMM").await.unwrap();
    
    // Create hybrid market account
    let (hybrid_pda, _) = Pubkey::find_program_address(
        &[b"hybrid_market", &market_id.to_le_bytes()],
        &env.program_id,
    );
    
    // Trader places trade through hybrid router
    let trader = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    let trade_params = TradeParams {
        market_id,
        outcome: 0,
        is_buy: true,
        amount: 100_000_000, // 100 USDC amount
        shares: Some(100_000_000), // Expected shares
        max_cost: Some(200_000_000),
        min_shares: Some(50_000_000), // Min acceptable shares
        min_payout: None,
        max_slippage_bps: Some(500), // 5% max slippage
    };
    
    let hybrid_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(hybrid_pda, false),
        AccountMeta::new(lmsr_pda, false),
        AccountMeta::new(pmamm_pda, false),
        AccountMeta::new(l2amm_pda, false),
    ];
    
    let hybrid_ix = Instruction {
        program_id: env.program_id,
        accounts: hybrid_accounts,
        data: BettingPlatformInstruction::TradeHybrid(trade_params)
            .try_to_vec()
            .unwrap(),
    };
    
    // This would route to the optimal AMM based on market conditions
    env.process_transaction(&[hybrid_ix], &[&trader.keypair])
        .await
        .unwrap();
}

#[tokio::test]
async fn test_amm_fee_collection() {
    let mut env = TestEnvironment::new().await.unwrap();
    
    // Initialize global config
    env.initialize_global_config().await.unwrap();
    
    // Create market with known fee (e.g., 2% = 200 bps)
    let market_id = 5u128;
    let market_pda = create_test_market(&mut env, market_id, "LMSR").await.unwrap();
    
    // Get initial vault balance
    let (fee_collector_pda, _) = Pubkey::find_program_address(
        &[b"fee_collector"],
        &env.program_id,
    );
    
    let initial_fees = env.get_account(&fee_collector_pda)
        .await
        .unwrap_or_default()
        .lamports;
    
    // Create trader and execute large trade
    let trader = env.create_user_with_tokens(10_000_000_000).await.unwrap();
    
    let trade_params = TradeParams {
        market_id,
        outcome: 0,
        is_buy: true,
        amount: 1_000_000_000, // 1000 USDC amount
        shares: Some(1_000_000_000), // 1000 shares expected
        max_cost: Some(2_000_000_000),
        min_shares: Some(500_000_000), // Min 500 shares acceptable
        min_payout: None,
        max_slippage_bps: Some(500), // 5% max slippage
    };
    
    let trade_accounts = vec![
        AccountMeta::new(trader.pubkey(), true),
        AccountMeta::new(market_pda, false),
        AccountMeta::new(fee_collector_pda, false),
        AccountMeta::new_readonly(env.program_id, false),
    ];
    
    let trade_ix = Instruction {
        program_id: env.program_id,
        accounts: trade_accounts,
        data: BettingPlatformInstruction::TradeLMSR(trade_params)
            .try_to_vec()
            .unwrap(),
    };
    
    env.process_transaction(&[trade_ix], &[&trader.keypair])
        .await
        .unwrap();
    
    // Verify fees collected
    let final_fees = env.get_account(&fee_collector_pda)
        .await
        .unwrap()
        .lamports;
    
    assert!(final_fees > initial_fees);
    
    // Fee should be approximately 2% of trade cost
    let fee_collected = final_fees - initial_fees;
    assert!(fee_collected > 0);
}

// Additional test cases to implement:
// - Test slippage protection
// - Test market resolution and payouts
// - Test impermanent loss calculations
// - Test oracle price feeds
// - Test circuit breakers
// - Test multi-outcome markets
// - Test edge cases (zero liquidity, maximum positions, etc.)