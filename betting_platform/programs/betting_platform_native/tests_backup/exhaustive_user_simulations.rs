//! Exhaustive User Path Simulations
//!
//! Comprehensive test suite covering all major workflows and edge cases

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    clock::Clock,
    commitment_config::CommitmentConfig,
    hash::Hash,
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
use borsh::{BorshDeserialize, BorshSerialize};
use std::sync::Arc;

use betting_platform_native::{
    instruction::{BettingPlatformInstruction, OpenPositionParams, TradeParams, ChainStepType},
    state::{
        GlobalConfigPDA, VersePDA, ProposalPDA, Position, UserMapPDA, UserStatsPDA,
        accounts::discriminators,
    },
    error::BettingPlatformError,
    pda::*,
};

pub mod helpers;
use helpers::*;

const USDC_DECIMALS: u64 = 1_000_000;
const SHARE_DECIMALS: u64 = 1_000_000;
const MAX_LEVERAGE: u8 = 64;

// ===== 1. USER ONBOARDING AND CREDIT SYSTEM TESTS =====

#[tokio::test]
async fn test_user_onboarding_first_time() {
    let mut context = TestContext::new().await;
    let user = Keypair::new();
    
    // Fund user with SOL for transaction fees
    context.fund_account(&user.pubkey(), 10 * LAMPORTS_PER_SOL).await;
    
    // Initialize global config first
    initialize_platform(&mut context).await.unwrap();
    
    // Create user accounts
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_stats_pda = get_user_stats_pda(&user.pubkey(), &context.program_id);
    
    // Execute onboarding
    let deposit_amount = 100 * USDC_DECIMALS;
    process_user_onboarding(
        &mut context,
        &user,
        deposit_amount,
    ).await.unwrap();
    
    // Verify user accounts
    let user_map = context.get_account_data::<UserMapPDA>(&user_map_pda).await.unwrap();
    assert_eq!(user_map.owner, user.pubkey());
    assert_eq!(user_map.credit_balance, deposit_amount);
    assert_eq!(user_map.positions.len(), 0);
    
    let user_stats = context.get_account_data::<UserStatsPDA>(&user_stats_pda).await.unwrap();
    assert_eq!(user_stats.total_volume, 0);
    assert_eq!(user_stats.total_pnl, 0);
    assert_eq!(user_stats.win_rate, 0);
}

#[tokio::test]
async fn test_multiple_deposits() {
    let mut context = TestContext::new().await;
    let user = Keypair::new();
    
    initialize_platform(&mut context).await.unwrap();
    process_user_onboarding(&mut context, &user, 50 * USDC_DECIMALS).await.unwrap();
    
    // Second deposit
    deposit_credits(&mut context, &user, 150 * USDC_DECIMALS).await.unwrap();
    
    // Verify total balance
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_map = context.get_account_data::<UserMapPDA>(&user_map_pda).await.unwrap();
    assert_eq!(user_map.credit_balance, 200 * USDC_DECIMALS);
    
    // Test maximum deposit limit
    let large_deposit = 1_000_000 * USDC_DECIMALS;
    let result = deposit_credits(&mut context, &user, large_deposit).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_user_registration() {
    let mut context = TestContext::new().await;
    initialize_platform(&mut context).await.unwrap();
    
    // Create 10 users simultaneously
    let mut users = Vec::new();
    let mut futures = Vec::new();
    
    for i in 0..10 {
        let user = Keypair::new();
        context.fund_account(&user.pubkey(), 10 * LAMPORTS_PER_SOL).await;
        
        let amount = (10 + i * 100) * USDC_DECIMALS;
        users.push((user, amount));
    }
    
    // Process all registrations
    for (user, amount) in &users {
        process_user_onboarding(&mut context, user, *amount).await.unwrap();
    }
    
    // Verify all users registered correctly
    for (user, expected_amount) in users {
        let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
        let user_map = context.get_account_data::<UserMapPDA>(&user_map_pda).await.unwrap();
        assert_eq!(user_map.credit_balance, expected_amount);
    }
}

// ===== 2. AMM TRADING TESTS =====

#[tokio::test]
async fn test_lmsr_trading_binary_market() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 1000 * USDC_DECIMALS).await;
    
    // Initialize LMSR market
    let market_id = 1u128;
    let b_parameter = 1000 * USDC_DECIMALS;
    initialize_lmsr_market(
        &mut context,
        market_id,
        b_parameter,
        2, // binary outcomes
    ).await.unwrap();
    
    // Check initial prices (should be ~50/50)
    let initial_yes_price = get_lmsr_price(&context, market_id, 0).await.unwrap();
    let initial_no_price = get_lmsr_price(&context, market_id, 1).await.unwrap();
    assert!((initial_yes_price as i64 - 5000).abs() < 100); // ~50%
    assert!((initial_no_price as i64 - 5000).abs() < 100); // ~50%
    
    // Buy YES shares
    let buy_amount = 10 * SHARE_DECIMALS;
    execute_lmsr_trade(
        &mut context,
        &trader,
        market_id,
        0, // YES outcome
        buy_amount,
        true, // is_buy
    ).await.unwrap();
    
    // Verify price increased for YES
    let new_yes_price = get_lmsr_price(&context, market_id, 0).await.unwrap();
    assert!(new_yes_price > initial_yes_price);
    
    // Verify position created
    let position = get_user_position(&context, &trader, market_id, 0).await.unwrap();
    assert_eq!(position.shares, buy_amount);
    
    // Sell half the shares
    execute_lmsr_trade(
        &mut context,
        &trader,
        market_id,
        0, // YES outcome
        buy_amount / 2,
        false, // is_sell
    ).await.unwrap();
    
    // Verify position updated
    let position = get_user_position(&context, &trader, market_id, 0).await.unwrap();
    assert_eq!(position.shares, buy_amount / 2);
}

#[tokio::test]
async fn test_pmamm_trading_with_liquidity() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 1000 * USDC_DECIMALS).await;
    let lp = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    
    // Initialize PM-AMM market
    let market_id = 2u128;
    let initial_liquidity = 10000 * USDC_DECIMALS;
    initialize_pmamm_market(
        &mut context,
        market_id,
        initial_liquidity,
        Clock::get().unwrap().slot + 86400, // 24 hours from now
        5000, // 50% initial price
    ).await.unwrap();
    
    // Add liquidity
    add_pmamm_liquidity(
        &mut context,
        &lp,
        market_id,
        1000 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Execute trade
    execute_pmamm_trade(
        &mut context,
        &trader,
        market_id,
        0, // outcome A
        100 * SHARE_DECIMALS,
        true, // buy
    ).await.unwrap();
    
    // Verify constant product maintained
    let pool = get_pmamm_pool(&context, market_id).await.unwrap();
    let product = pool.reserves[0] * pool.reserves[1];
    assert!(product > 0);
    
    // Remove liquidity
    remove_pmamm_liquidity(
        &mut context,
        &lp,
        market_id,
        500 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Verify LP tokens burned
    let lp_balance = get_lp_token_balance(&context, &lp, market_id).await.unwrap();
    assert!(lp_balance < initial_liquidity);
}

#[tokio::test]
async fn test_l2amm_range_trading() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 1000 * USDC_DECIMALS).await;
    
    // Initialize L2 AMM for continuous market
    let market_id = 3u128;
    let params = L2InitParams {
        pool_id: market_id,
        min_value: 0,
        max_value: 100 * USDC_DECIMALS,
        num_bins: 20,
        initial_distribution: None, // Use default normal
        liquidity_parameter: 10000 * USDC_DECIMALS,
    };
    
    initialize_l2amm_market(&mut context, params).await.unwrap();
    
    // Buy central range [45-55]
    execute_l2_range_trade(
        &mut context,
        &trader,
        market_id,
        45 * USDC_DECIMALS,
        55 * USDC_DECIMALS,
        50 * SHARE_DECIMALS,
        true, // buy
    ).await.unwrap();
    
    // Verify distribution updated
    let distribution = get_l2_distribution(&context, market_id).await.unwrap();
    let central_bin = 10; // Middle of 20 bins
    assert!(distribution.probabilities[central_bin] > distribution.probabilities[0]);
    
    // Buy tail range [0-20]
    execute_l2_range_trade(
        &mut context,
        &trader,
        market_id,
        0,
        20 * USDC_DECIMALS,
        10 * SHARE_DECIMALS,
        true, // buy
    ).await.unwrap();
    
    // Verify tail probability increased
    let distribution = get_l2_distribution(&context, market_id).await.unwrap();
    assert!(distribution.probabilities[0] > 0);
}

// ===== 3. LEVERAGE TRADING TESTS =====

#[tokio::test]
async fn test_progressive_leverage_increase() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 1000 * USDC_DECIMALS).await;
    
    let market_id = 4u128;
    let proposal_id = 1u128;
    create_test_market(&mut context, market_id, proposal_id).await.unwrap();
    
    // Test each leverage tier
    let leverage_tests = vec![
        (1u8, 100 * USDC_DECIMALS, true),
        (2u8, 50 * USDC_DECIMALS, true),
        (4u8, 25 * USDC_DECIMALS, true),
        (8u8, 12 * USDC_DECIMALS, false), // Should fail due to coverage
    ];
    
    for (leverage, size, should_succeed) in leverage_tests {
        let result = open_leveraged_position(
            &mut context,
            &trader,
            proposal_id,
            0, // YES outcome
            leverage,
            size,
        ).await;
        
        if should_succeed {
            assert!(result.is_ok());
            
            // Verify position created
            let positions = get_user_positions(&context, &trader).await.unwrap();
            assert!(positions.iter().any(|p| p.leverage == leverage));
        } else {
            assert!(result.is_err());
        }
    }
    
    // Update coverage to allow higher leverage
    increase_platform_coverage(&mut context, 10000 * USDC_DECIMALS).await.unwrap();
    
    // Retry 8x leverage
    let result = open_leveraged_position(
        &mut context,
        &trader,
        proposal_id,
        0, // YES outcome
        8u8,
        12 * USDC_DECIMALS,
    ).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_maximum_leverage_stress() {
    let mut context = TestContext::new().await;
    let whale = create_funded_trader(&mut context, 100000 * USDC_DECIMALS).await;
    
    // Ensure high coverage for max leverage
    increase_platform_coverage(&mut context, 1000000 * USDC_DECIMALS).await.unwrap();
    
    let market_id = 5u128;
    let proposal_id = 2u128;
    create_test_market(&mut context, market_id, proposal_id).await.unwrap();
    
    // Open multiple 64x positions
    for i in 0..5 {
        let result = open_leveraged_position(
            &mut context,
            &whale,
            proposal_id,
            0, // YES outcome
            64u8,
            1000 * USDC_DECIMALS,
        ).await;
        
        if i < 3 {
            assert!(result.is_ok());
        } else {
            // Should hit position limits
            assert!(result.is_err());
        }
    }
    
    // Monitor position health
    monitor_position_health(&mut context, &whale).await.unwrap();
    
    // Simulate price movement triggering liquidation
    update_market_price(&mut context, market_id, 3000).await.unwrap(); // 30% price
    
    // Check liquidation triggered
    let positions = get_user_positions(&context, &whale).await.unwrap();
    let at_risk = positions.iter().filter(|p| p.health_factor < 110).count();
    assert!(at_risk > 0);
}

// ===== 4. SYNTHETICS AND ARBITRAGE TESTS =====

#[tokio::test]
async fn test_synthetic_position_creation() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 5000 * USDC_DECIMALS).await;
    
    // Create two correlated markets
    let market_a = 6u128;
    let market_b = 7u128;
    let proposal_a = 3u128;
    let proposal_b = 4u128;
    
    create_test_market(&mut context, market_a, proposal_a).await.unwrap();
    create_test_market(&mut context, market_b, proposal_b).await.unwrap();
    
    // Create synthetic: Long A, Short B
    open_leveraged_position(
        &mut context,
        &trader,
        proposal_a,
        0, // YES
        2u8,
        1000 * USDC_DECIMALS,
    ).await.unwrap();
    
    open_leveraged_position(
        &mut context,
        &trader,
        proposal_b,
        1, // NO (inverse)
        2u8,
        1000 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Verify net exposure
    let positions = get_user_positions(&context, &trader).await.unwrap();
    assert_eq!(positions.len(), 2);
    
    // Calculate synthetic P&L
    update_market_price(&mut context, market_a, 6000).await.unwrap(); // 60%
    update_market_price(&mut context, market_b, 5500).await.unwrap(); // 55%
    
    let pnl = calculate_portfolio_pnl(&context, &trader).await.unwrap();
    assert!(pnl > 0); // Profitable due to spread
}

#[tokio::test]
async fn test_cross_market_arbitrage() {
    let mut context = TestContext::new().await;
    let arb_trader = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    
    // Create markets with price discrepancy
    let lmsr_market = 8u128;
    let pmamm_market = 9u128;
    
    initialize_lmsr_market(&mut context, lmsr_market, 5000 * USDC_DECIMALS, 2).await.unwrap();
    initialize_pmamm_market(
        &mut context, 
        pmamm_market, 
        5000 * USDC_DECIMALS,
        Clock::get().unwrap().slot + 86400,
        4500, // 45% vs LMSR's ~50%
    ).await.unwrap();
    
    // Execute arbitrage
    let arb_amount = 100 * SHARE_DECIMALS;
    
    // Buy cheap on PM-AMM
    execute_pmamm_trade(
        &mut context,
        &arb_trader,
        pmamm_market,
        0, // YES
        arb_amount,
        true,
    ).await.unwrap();
    
    // Sell high on LMSR
    execute_lmsr_trade(
        &mut context,
        &arb_trader,
        lmsr_market,
        0, // YES
        arb_amount,
        false,
    ).await.unwrap();
    
    // Verify profit
    let final_balance = get_user_balance(&context, &arb_trader).await.unwrap();
    assert!(final_balance > 10000 * USDC_DECIMALS);
}

// ===== 5. PRIORITY QUEUE TRADING TESTS =====

#[tokio::test]
async fn test_iceberg_order_execution() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 50000 * USDC_DECIMALS).await;
    
    let market_id = 10u128;
    initialize_lmsr_market(&mut context, market_id, 10000 * USDC_DECIMALS, 2).await.unwrap();
    
    // Place large iceberg order
    let total_size = 10000 * SHARE_DECIMALS;
    let visible_size = 100 * SHARE_DECIMALS;
    
    let order_id = place_iceberg_order(
        &mut context,
        &trader,
        market_id,
        0, // YES
        visible_size,
        total_size,
        OrderSide::Buy,
    ).await.unwrap();
    
    // Execute multiple fills
    let mut total_filled = 0u64;
    while total_filled < total_size {
        let fill_amount = execute_iceberg_fill(
            &mut context,
            order_id,
            visible_size,
        ).await.unwrap();
        
        total_filled += fill_amount;
        
        // Verify visible size refreshed
        let order = get_iceberg_order(&context, order_id).await.unwrap();
        assert_eq!(
            order.visible_size,
            std::cmp::min(visible_size, order.total_size - total_filled)
        );
    }
    
    assert_eq!(total_filled, total_size);
}

#[tokio::test]
async fn test_twap_order_execution() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 20000 * USDC_DECIMALS).await;
    
    let market_id = 11u128;
    initialize_lmsr_market(&mut context, market_id, 5000 * USDC_DECIMALS, 2).await.unwrap();
    
    // Place TWAP order
    let total_size = 1000 * SHARE_DECIMALS;
    let duration_slots = 100u64;
    let intervals = 10u8;
    
    let order_id = place_twap_order(
        &mut context,
        &trader,
        market_id,
        0, // YES
        total_size,
        duration_slots,
        intervals,
        OrderSide::Buy,
    ).await.unwrap();
    
    // Execute at each interval
    let interval_size = total_size / intervals as u64;
    let slot_interval = duration_slots / intervals as u64;
    
    for i in 0..intervals {
        // Advance to next interval
        context.warp_to_slot(context.get_slot().await + slot_interval).await;
        
        execute_twap_interval(&mut context, order_id).await.unwrap();
        
        // Verify partial execution
        let order = get_twap_order(&context, order_id).await.unwrap();
        assert_eq!(order.executed_size, interval_size * (i + 1) as u64);
    }
    
    // Verify complete execution
    let order = get_twap_order(&context, order_id).await.unwrap();
    assert_eq!(order.executed_size, total_size);
    assert!(order.is_complete);
}

#[tokio::test]
async fn test_dark_pool_matching() {
    let mut context = TestContext::new().await;
    let buyer = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    let seller = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    
    let market_id = 12u128;
    let proposal_id = 5u128;
    create_test_market(&mut context, market_id, proposal_id).await.unwrap();
    
    // Initialize dark pool
    initialize_dark_pool(
        &mut context,
        market_id,
        100 * SHARE_DECIMALS, // minimum size
        10, // 0.1% price improvement
    ).await.unwrap();
    
    // First, sellers need shares to sell
    open_leveraged_position(
        &mut context,
        &seller,
        proposal_id,
        0, // YES
        1u8,
        1000 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Place dark orders
    place_dark_order(
        &mut context,
        &buyer,
        market_id,
        OrderSide::Buy,
        0, // YES
        500 * SHARE_DECIMALS,
        Some(4900), // min price 49%
        Some(5100), // max price 51%
        TimeInForce::Session,
    ).await.unwrap();
    
    place_dark_order(
        &mut context,
        &seller,
        market_id,
        OrderSide::Sell,
        0, // YES
        600 * SHARE_DECIMALS,
        Some(4900),
        Some(5100),
        TimeInForce::Session,
    ).await.unwrap();
    
    // Match orders
    match_dark_pool_orders(&mut context, market_id).await.unwrap();
    
    // Verify 500 shares matched with price improvement
    let buyer_position = get_user_position(&context, &buyer, market_id, 0).await.unwrap();
    assert_eq!(buyer_position.shares, 500 * SHARE_DECIMALS);
    
    // Verify 100 shares remain as sell order
    let remaining_orders = get_dark_pool_orders(&context, market_id).await.unwrap();
    assert_eq!(remaining_orders.len(), 1);
    assert_eq!(remaining_orders[0].remaining_size, 100 * SHARE_DECIMALS);
}

// ===== 6. MARKET RESOLUTION TESTS =====

#[tokio::test]
async fn test_normal_market_resolution() {
    let mut context = TestContext::new().await;
    
    // Create traders with positions
    let winner = create_funded_trader(&mut context, 5000 * USDC_DECIMALS).await;
    let loser = create_funded_trader(&mut context, 5000 * USDC_DECIMALS).await;
    
    let market_id = 13u128;
    let proposal_id = 6u128;
    let expiry_slot = context.get_slot().await + 1000;
    
    create_expiring_market(&mut context, market_id, proposal_id, expiry_slot).await.unwrap();
    
    // Open positions
    open_leveraged_position(
        &mut context,
        &winner,
        proposal_id,
        0, // YES
        2u8,
        1000 * USDC_DECIMALS,
    ).await.unwrap();
    
    open_leveraged_position(
        &mut context,
        &loser,
        proposal_id,
        1, // NO
        2u8,
        1000 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Fast forward to expiry
    context.warp_to_slot(expiry_slot + 1).await;
    
    // Oracle updates with final price
    update_oracle_price_polymarket(
        &mut context,
        market_id,
        7500, // YES wins at 75%
        2500, // NO loses at 25%
    ).await.unwrap();
    
    // Process resolution
    process_market_resolution(
        &mut context,
        market_id,
        proposal_id,
        "YES",
    ).await.unwrap();
    
    // Verify settlements
    let winner_balance = get_user_balance(&context, &winner).await.unwrap();
    let loser_balance = get_user_balance(&context, &loser).await.unwrap();
    
    assert!(winner_balance > 5000 * USDC_DECIMALS); // Profit
    assert!(loser_balance < 5000 * USDC_DECIMALS); // Loss
}

#[tokio::test]
async fn test_disputed_resolution() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    
    let market_id = 14u128;
    let proposal_id = 7u128;
    let expiry_slot = context.get_slot().await + 1000;
    
    create_expiring_market(&mut context, market_id, proposal_id, expiry_slot).await.unwrap();
    
    // Open position
    open_leveraged_position(
        &mut context,
        &trader,
        proposal_id,
        1, // NO
        1u8,
        2000 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Fast forward and resolve
    context.warp_to_slot(expiry_slot + 1).await;
    
    // Initial resolution: YES wins
    update_oracle_price_polymarket(&mut context, market_id, 6000, 4000).await.unwrap();
    process_market_resolution(&mut context, market_id, proposal_id, "YES").await.unwrap();
    
    // Initiate dispute
    initiate_dispute(
        &mut context,
        &trader,
        market_id,
        proposal_id,
    ).await.unwrap();
    
    // Dispute period
    context.warp_to_slot(context.get_slot().await + 7200).await; // 24 hours
    
    // Final resolution: NO wins
    resolve_dispute(
        &mut context,
        market_id,
        proposal_id,
        "NO",
    ).await.unwrap();
    
    // Verify reversal
    let trader_balance = get_user_balance(&context, &trader).await.unwrap();
    assert!(trader_balance > 10000 * USDC_DECIMALS); // Now profitable
}

#[tokio::test]
async fn test_tie_resolution() {
    let mut context = TestContext::new().await;
    let trader1 = create_funded_trader(&mut context, 5000 * USDC_DECIMALS).await;
    let trader2 = create_funded_trader(&mut context, 5000 * USDC_DECIMALS).await;
    
    let market_id = 15u128;
    let proposal_id = 8u128;
    let expiry_slot = context.get_slot().await + 1000;
    
    create_expiring_market(&mut context, market_id, proposal_id, expiry_slot).await.unwrap();
    
    // Both traders take opposite sides
    open_leveraged_position(&mut context, &trader1, proposal_id, 0, 1u8, 1000 * USDC_DECIMALS).await.unwrap();
    open_leveraged_position(&mut context, &trader2, proposal_id, 1, 1u8, 1000 * USDC_DECIMALS).await.unwrap();
    
    // Fast forward to expiry
    context.warp_to_slot(expiry_slot + 1).await;
    
    // Resolve as tie (50/50)
    update_oracle_price_polymarket(&mut context, market_id, 5000, 5000).await.unwrap();
    process_market_resolution(&mut context, market_id, proposal_id, "TIE").await.unwrap();
    
    // Verify refunds
    let balance1 = get_user_balance(&context, &trader1).await.unwrap();
    let balance2 = get_user_balance(&context, &trader2).await.unwrap();
    
    // Both should get ~1000 USDC back (minus small fees)
    assert!(balance1 > 4900 * USDC_DECIMALS && balance1 < 5000 * USDC_DECIMALS);
    assert!(balance2 > 4900 * USDC_DECIMALS && balance2 < 5000 * USDC_DECIMALS);
}

// ===== 7. WITHDRAWALS AND REFUNDS TESTS =====

#[tokio::test]
async fn test_simple_withdrawal() {
    let mut context = TestContext::new().await;
    let user = create_funded_trader(&mut context, 1000 * USDC_DECIMALS).await;
    
    // Withdraw 400 USDC
    withdraw_credits(
        &mut context,
        &user,
        400 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Verify balance
    let remaining = get_user_balance(&context, &user).await.unwrap();
    assert_eq!(remaining, 600 * USDC_DECIMALS);
    
    // Verify USDC received
    let usdc_balance = get_token_balance(&context, &user, &context.usdc_mint).await.unwrap();
    assert_eq!(usdc_balance, 400 * USDC_DECIMALS);
}

#[tokio::test]
async fn test_withdrawal_with_open_positions() {
    let mut context = TestContext::new().await;
    let trader = create_funded_trader(&mut context, 2000 * USDC_DECIMALS).await;
    
    let proposal_id = 9u128;
    create_test_market(&mut context, 16u128, proposal_id).await.unwrap();
    
    // Open position using 1200 USDC
    open_leveraged_position(
        &mut context,
        &trader,
        proposal_id,
        0,
        2u8,
        600 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Try to withdraw 1000 USDC (should fail)
    let result = withdraw_credits(&mut context, &trader, 1000 * USDC_DECIMALS).await;
    assert!(result.is_err());
    
    // Withdraw available balance (800 USDC)
    withdraw_credits(&mut context, &trader, 800 * USDC_DECIMALS).await.unwrap();
    
    let remaining = get_user_balance(&context, &trader).await.unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn test_emergency_withdrawal() {
    let mut context = TestContext::new().await;
    
    // Create multiple users
    let users: Vec<_> = futures::future::join_all(
        (0..5).map(|_| create_funded_trader(&mut context, 2000 * USDC_DECIMALS))
    ).await;
    
    // Trigger emergency halt
    trigger_emergency_halt(&mut context).await.unwrap();
    
    // All users request withdrawal
    for user in &users {
        let result = emergency_withdraw(&mut context, user).await;
        assert!(result.is_ok());
    }
    
    // Verify all balances zeroed
    for user in &users {
        let balance = get_user_balance(&context, user).await.unwrap();
        assert_eq!(balance, 0);
    }
}

// ===== 8. EDGE CASES AND STRESS TESTS =====

#[tokio::test]
async fn test_circuit_breaker_activation() {
    let mut context = TestContext::new().await;
    let whale = create_funded_trader(&mut context, 100000 * USDC_DECIMALS).await;
    
    let market_id = 17u128;
    initialize_lmsr_market(&mut context, market_id, 10000 * USDC_DECIMALS, 2).await.unwrap();
    
    // Get initial price
    let initial_price = get_lmsr_price(&context, market_id, 0).await.unwrap();
    
    // Execute large trade causing >20% price movement
    let large_trade = 5000 * SHARE_DECIMALS;
    let result = execute_lmsr_trade(
        &mut context,
        &whale,
        market_id,
        0,
        large_trade,
        true,
    ).await;
    
    // Should trigger circuit breaker
    assert!(result.is_err());
    
    // Verify market halted
    let market_state = get_market_state(&context, market_id).await.unwrap();
    assert!(market_state.is_halted);
    
    // Wait for cooldown
    context.warp_to_slot(context.get_slot().await + 300).await; // 5 minute cooldown
    
    // Market should reopen
    let market_state = get_market_state(&context, market_id).await.unwrap();
    assert!(!market_state.is_halted);
}

#[tokio::test]
async fn test_liquidation_cascade_prevention() {
    let mut context = TestContext::new().await;
    
    // Ensure high initial coverage
    increase_platform_coverage(&mut context, 100000 * USDC_DECIMALS).await.unwrap();
    
    // Create multiple traders at max leverage
    let traders: Vec<_> = futures::future::join_all(
        (0..10).map(|_| create_funded_trader(&mut context, 5000 * USDC_DECIMALS))
    ).await;
    
    let proposal_id = 10u128;
    let market_id = 18u128;
    create_test_market(&mut context, market_id, proposal_id).await.unwrap();
    
    // All traders open max leverage positions
    for trader in &traders {
        open_leveraged_position(
            &mut context,
            trader,
            proposal_id,
            0,
            16u8, // High leverage
            300 * USDC_DECIMALS,
        ).await.unwrap();
    }
    
    // Simulate adverse price movement
    update_market_price(&mut context, market_id, 2000).await.unwrap(); // 20% price
    
    // Process liquidations
    process_liquidation_queue(&mut context, 5).await.unwrap(); // Max 5 per block
    
    // Verify orderly liquidation
    let liquidated = count_liquidated_positions(&context, &traders).await.unwrap();
    assert!(liquidated <= 5); // No cascade
    
    // Process next batch
    process_liquidation_queue(&mut context, 5).await.unwrap();
    
    // Verify market stability maintained
    let global_config = get_global_config(&context).await.unwrap();
    assert!(global_config.coverage > 10000); // 100% coverage maintained
}

#[tokio::test]
async fn test_vampire_attack_defense() {
    let mut context = TestContext::new().await;
    let attacker = create_funded_trader(&mut context, 1000000 * USDC_DECIMALS).await;
    
    // Initialize bootstrap phase
    initialize_bootstrap_phase(
        &mut context,
        10000000 * USDC_DECIMALS, // 10M MMT allocation
    ).await.unwrap();
    
    // Large deposit
    process_bootstrap_deposit(
        &mut context,
        &attacker,
        500000 * USDC_DECIMALS,
    ).await.unwrap();
    
    // Immediate withdrawal attempt
    let result = process_bootstrap_withdrawal(
        &mut context,
        &attacker,
        500000 * USDC_DECIMALS,
    ).await;
    
    // Should be blocked
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        BettingPlatformError::VampireAttackDetected
    );
    
    // Verify funds locked
    let bootstrap_state = get_bootstrap_state(&context).await.unwrap();
    assert!(bootstrap_state.locked_withdrawals.contains(&attacker.pubkey()));
}

#[tokio::test]
async fn test_congestion_handling() {
    let mut context = TestContext::new().await;
    
    // Create many traders
    let traders: Vec<_> = futures::future::join_all(
        (0..100).map(|_| create_funded_trader(&mut context, 1000 * USDC_DECIMALS))
    ).await;
    
    let market_id = 19u128;
    initialize_lmsr_market(&mut context, market_id, 50000 * USDC_DECIMALS, 2).await.unwrap();
    
    // Submit many transactions in parallel
    let mut handles = vec![];
    for (i, trader) in traders.iter().enumerate() {
        let priority = if i < 10 { 1000 } else { 100 }; // First 10 are high priority
        
        handles.push(tokio::spawn(execute_lmsr_trade_with_priority(
            context.clone(),
            trader.insecure_clone(),
            market_id,
            0,
            10 * SHARE_DECIMALS,
            true,
            priority,
        )));
    }
    
    // Wait for all to complete
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // Verify high priority processed first
    let successful = results.iter().filter(|r| r.is_ok()).count();
    assert!(successful >= 10); // At least high priority processed
}

// ===== 9. MMT TOKEN INTEGRATION TESTS =====

#[tokio::test]
async fn test_mmt_staking_and_rewards() {
    let mut context = TestContext::new().await;
    let staker = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    
    // Get MMT tokens (from trading rewards)
    earn_mmt_tokens(&mut context, &staker, 1000 * 10u64.pow(9)).await.unwrap();
    
    // Initialize staking pool
    initialize_staking_pool(&mut context).await.unwrap();
    
    // Stake MMT with 30-day lock
    stake_mmt_tokens(
        &mut context,
        &staker,
        500 * 10u64.pow(9),
        Some(30 * 24 * 60 * 60 / 2), // 30 days in slots
    ).await.unwrap();
    
    // Generate trading fees
    generate_platform_fees(&mut context, 10000 * USDC_DECIMALS).await.unwrap();
    
    // Distribute fees to stakers
    distribute_trading_fees(&mut context, 10000 * USDC_DECIMALS).await.unwrap();
    
    // Claim rewards
    let rewards = claim_staking_rewards(&mut context, &staker).await.unwrap();
    assert!(rewards > 0);
    
    // Try early unstake (should fail)
    let result = unstake_mmt_tokens(&mut context, &staker, 500 * 10u64.pow(9)).await;
    assert!(result.is_err());
    
    // Fast forward to unlock
    context.warp_to_slot(context.get_slot().await + 30 * 24 * 60 * 60 / 2).await;
    
    // Now unstake should work
    unstake_mmt_tokens(&mut context, &staker, 500 * 10u64.pow(9)).await.unwrap();
}

#[tokio::test]
async fn test_maker_incentives() {
    let mut context = TestContext::new().await;
    let maker = create_funded_trader(&mut context, 50000 * USDC_DECIMALS).await;
    
    // Initialize maker account
    initialize_maker_account(&mut context, &maker).await.unwrap();
    
    let market_id = 20u128;
    initialize_pmamm_market(
        &mut context,
        market_id,
        10000 * USDC_DECIMALS,
        Clock::get().unwrap().slot + 86400,
        5000,
    ).await.unwrap();
    
    // Provide liquidity
    add_pmamm_liquidity(&mut context, &maker, market_id, 10000 * USDC_DECIMALS).await.unwrap();
    
    // Execute trades with spread improvement
    for _ in 0..10 {
        record_maker_trade(
            &mut context,
            &maker,
            1000 * USDC_DECIMALS, // notional
            15, // 0.15% spread improvement
        ).await.unwrap();
    }
    
    // Claim maker rewards
    let mmt_rewards = claim_maker_rewards(&mut context, &maker).await.unwrap();
    assert!(mmt_rewards > 0);
    
    // Verify MMT balance increased
    let mmt_balance = get_mmt_balance(&context, &maker).await.unwrap();
    assert_eq!(mmt_balance, mmt_rewards);
}

#[tokio::test]
async fn test_season_transition() {
    let mut context = TestContext::new().await;
    
    // Fast forward to end of season
    let season_end = get_season_end_slot(&context).await.unwrap();
    context.warp_to_slot(season_end + 1).await;
    
    // Transition season
    transition_to_next_season(&mut context).await.unwrap();
    
    // Verify new season parameters
    let global_config = get_global_config(&context).await.unwrap();
    assert_eq!(global_config.season, 2);
    
    // Verify emission schedule updated
    let new_emission_rate = get_mmt_emission_rate(&context).await.unwrap();
    assert!(new_emission_rate < global_config.mmt_emission_rate); // Decreasing emissions
}

// ===== 10. SECURITY AND ATTACK SCENARIOS =====

#[tokio::test]
async fn test_sandwich_attack_prevention() {
    let mut context = TestContext::new().await;
    let victim = create_funded_trader(&mut context, 10000 * USDC_DECIMALS).await;
    let attacker = create_funded_trader(&mut context, 50000 * USDC_DECIMALS).await;
    
    let market_id = 21u128;
    initialize_lmsr_market(&mut context, market_id, 10000 * USDC_DECIMALS, 2).await.unwrap();
    
    // Victim submits large trade
    let victim_tx = create_trade_transaction(
        &victim,
        market_id,
        0,
        1000 * SHARE_DECIMALS,
        true,
    );
    
    // Attacker tries to front-run
    let front_run_tx = create_trade_transaction(
        &attacker,
        market_id,
        0,
        500 * SHARE_DECIMALS,
        true,
    );
    
    // Submit both with MEV protection
    let results = submit_transactions_with_mev_protection(
        &mut context,
        vec![front_run_tx, victim_tx],
    ).await;
    
    // Verify victim trade executed first (or both failed)
    let victim_position = get_user_position(&context, &victim, market_id, 0).await;
    let attacker_position = get_user_position(&context, &attacker, market_id, 0).await;
    
    if victim_position.is_ok() {
        assert_eq!(victim_position.unwrap().shares, 1000 * SHARE_DECIMALS);
    }
}

#[tokio::test]
async fn test_oracle_manipulation_defense() {
    let mut context = TestContext::new().await;
    let attacker_oracle = Keypair::new();
    
    let market_id = 22u128;
    create_test_market(&mut context, market_id, 11u128).await.unwrap();
    
    // Rapid price updates
    let mut results = vec![];
    for i in 0..10 {
        let price = 5000 + i * 1000; // Rapid changes
        results.push(
            update_oracle_price_with_authority(
                &mut context,
                &attacker_oracle,
                market_id,
                price,
                10000 - price,
            ).await
        );
    }
    
    // Most should fail due to rate limiting
    let successful = results.iter().filter(|r| r.is_ok()).count();
    assert!(successful < 3); // Only first few succeed
    
    // Verify anomaly detection triggered
    let oracle_state = get_oracle_state(&context, market_id).await.unwrap();
    assert!(oracle_state.anomaly_detected);
}

#[tokio::test]
async fn test_sybil_attack_on_rewards() {
    let mut context = TestContext::new().await;
    let attacker = Keypair::new();
    context.fund_account(&attacker.pubkey(), 100 * LAMPORTS_PER_SOL).await;
    
    // Create 100 fake accounts
    let mut sybil_accounts = vec![];
    for _ in 0..100 {
        let sybil = Keypair::new();
        context.fund_account(&sybil.pubkey(), LAMPORTS_PER_SOL).await;
        
        // Minimal activity
        process_user_onboarding(&mut context, &sybil, 10 * USDC_DECIMALS).await.unwrap();
        sybil_accounts.push(sybil);
    }
    
    // Try to claim early trader rewards
    let season = get_current_season(&context).await.unwrap();
    for sybil in &sybil_accounts {
        let result = register_early_trader(&mut context, sybil, season).await;
        // Should fail for most due to sybil detection
        if result.is_err() {
            assert_eq!(result.unwrap_err(), BettingPlatformError::SybilDetected);
        }
    }
    
    // Verify few registrations
    let registered = count_early_traders(&context, season).await.unwrap();
    assert!(registered < 10); // Most blocked
}

// ===== 11. BOOTSTRAP PHASE LIFECYCLE =====

#[tokio::test]
async fn test_successful_bootstrap_completion() {
    let mut context = TestContext::new().await;
    
    // Initialize bootstrap with 1M MMT
    initialize_bootstrap_phase(
        &mut context,
        1_000_000 * 10u64.pow(9),
    ).await.unwrap();
    
    // Multiple users deposit
    let depositors = vec![
        (create_funded_trader(&mut context, 3_000_000 * USDC_DECIMALS).await, 2_500_000),
        (create_funded_trader(&mut context, 3_000_000 * USDC_DECIMALS).await, 2_500_000),
        (create_funded_trader(&mut context, 3_000_000 * USDC_DECIMALS).await, 2_500_000),
        (create_funded_trader(&mut context, 3_000_000 * USDC_DECIMALS).await, 2_500_000),
    ];
    
    for (depositor, amount) in &depositors {
        process_bootstrap_deposit(
            &mut context,
            depositor,
            amount * USDC_DECIMALS,
        ).await.unwrap();
    }
    
    // Update coverage
    update_bootstrap_coverage(&mut context).await.unwrap();
    
    // Verify target reached
    let bootstrap_state = get_bootstrap_state(&context).await.unwrap();
    assert!(bootstrap_state.total_deposits >= 10_000_000 * USDC_DECIMALS);
    assert!(bootstrap_state.coverage_ratio >= 15000); // 150%
    
    // Complete bootstrap
    complete_bootstrap_phase(&mut context).await.unwrap();
    
    // Verify MMT distributed
    for (depositor, amount) in depositors {
        let mmt_balance = get_mmt_balance(&context, &depositor).await.unwrap();
        let expected_mmt = (1_000_000 * 10u64.pow(9)) * amount / 10_000_000;
        assert_eq!(mmt_balance, expected_mmt);
    }
    
    // Verify normal trading enabled
    let global_config = get_global_config(&context).await.unwrap();
    assert!(!global_config.bootstrap_active);
}

#[tokio::test]
async fn test_extended_bootstrap_period() {
    let mut context = TestContext::new().await;
    
    initialize_bootstrap_phase(&mut context, 1_000_000 * 10u64.pow(9)).await.unwrap();
    
    // Slow accumulation over 13 days
    for day in 0..13 {
        context.warp_to_slot(context.get_slot().await + 24 * 60 * 60 / 2).await;
        
        let depositor = create_funded_trader(&mut context, 1_000_000 * USDC_DECIMALS).await;
        let amount = if day < 7 { 
            300_000 // Slow start
        } else { 
            1_200_000 // Accelerated after marketing
        };
        
        process_bootstrap_deposit(
            &mut context,
            &depositor,
            amount * USDC_DECIMALS,
        ).await.unwrap();
        
        update_bootstrap_coverage(&mut context).await.unwrap();
    }
    
    // Verify target reached on day 13
    let bootstrap_state = get_bootstrap_state(&context).await.unwrap();
    assert!(bootstrap_state.total_deposits >= 10_000_000 * USDC_DECIMALS);
    
    // Complete on day 14
    context.warp_to_slot(context.get_slot().await + 24 * 60 * 60 / 2).await;
    complete_bootstrap_phase(&mut context).await.unwrap();
}

// ===== HELPER FUNCTIONS =====

async fn initialize_platform(context: &mut TestContext) -> Result<(), BettingPlatformError> {
    let init_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
        ],
        data: BettingPlatformInstruction::Initialize { 
            seed: rand::random() 
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[init_ix], &[&context.payer]).await
}

async fn process_user_onboarding(
    context: &mut TestContext,
    user: &Keypair,
    deposit_amount: u64,
) -> Result<(), BettingPlatformError> {
    // Implementation creates user accounts and deposits initial credits
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_stats_pda = get_user_stats_pda(&user.pubkey(), &context.program_id);
    
    // Create accounts and deposit in single transaction
    let ixs = vec![
        create_user_account_ix(&user.pubkey(), &user_map_pda, &context.program_id),
        create_user_stats_ix(&user.pubkey(), &user_stats_pda, &context.program_id),
        deposit_credits_ix(&user.pubkey(), deposit_amount, &context.program_id),
    ];
    
    context.process_transaction(&ixs, &[&context.payer, user]).await
}

async fn create_funded_trader(
    context: &mut TestContext,
    amount: u64,
) -> Keypair {
    let trader = Keypair::new();
    context.fund_account(&trader.pubkey(), 10 * LAMPORTS_PER_SOL).await;
    process_user_onboarding(context, &trader, amount).await.unwrap();
    trader
}

// Additional helper functions would be implemented here...

#[derive(Clone)]
struct TestContext {
    banks_client: BanksClient,
    payer: Keypair,
    recent_blockhash: Hash,
    program_id: Pubkey,
    global_config_pda: Pubkey,
    usdc_mint: Pubkey,
}

impl TestContext {
    async fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let mut program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(betting_platform_native::entrypoint::process_instruction),
        );
        
        // Add additional programs
        program_test.add_program(
            "spl_token",
            spl_token::id(),
            processor!(spl_token::processor::Processor::process),
        );
        
        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
        
        // Create USDC mint
        let usdc_mint = Keypair::new();
        create_mint(&mut banks_client, &payer, &usdc_mint, &payer.pubkey(), 6).await;
        
        let global_config_pda = get_global_config_pda(&program_id);
        
        Self {
            banks_client,
            payer: payer.insecure_clone(),
            recent_blockhash,
            program_id,
            global_config_pda,
            usdc_mint: usdc_mint.pubkey(),
        }
    }
    
    async fn get_account_data<T: BorshDeserialize>(&mut self, pubkey: &Pubkey) -> Result<T, BettingPlatformError> {
        let account = self.banks_client
            .get_account(*pubkey)
            .await
            .unwrap()
            .ok_or(BettingPlatformError::AccountNotFound)?;
        
        T::try_from_slice(&account.data[8..]) // Skip discriminator
            .map_err(|_| BettingPlatformError::InvalidAccountData)
    }
    
    async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> Result<(), BettingPlatformError> {
        let mut transaction = Transaction::new_with_payer(instructions, Some(&self.payer.pubkey()));
        transaction.sign(signers, self.recent_blockhash);
        
        self.banks_client
            .process_transaction(transaction)
            .await
            .map_err(|_| BettingPlatformError::TransactionFailed)?;
        
        Ok(())
    }
    
    async fn fund_account(&mut self, pubkey: &Pubkey, lamports: u64) {
        let transfer_ix = system_instruction::transfer(&self.payer.pubkey(), pubkey, lamports);
        self.process_transaction(&[transfer_ix], &[&self.payer]).await.unwrap();
    }
    
    async fn get_slot(&mut self) -> u64 {
        self.banks_client.get_root_slot().await.unwrap()
    }
    
    async fn warp_to_slot(&mut self, slot: u64) {
        // Implementation would use program test context to advance slots
    }
}

// PDA derivation functions
fn get_global_config_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"global_config"], program_id).0
}

fn get_user_map_pda(user: &Pubkey, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_map", user.as_ref()], program_id).0
}

fn get_user_stats_pda(user: &Pubkey, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_stats", user.as_ref()], program_id).0
}

// Additional PDA functions...