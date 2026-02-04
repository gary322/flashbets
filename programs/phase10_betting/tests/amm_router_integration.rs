#![cfg(feature = "test-sbf")]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use phase10_betting::*;
use phase10_betting::amm::types::{MarketType, AMMType, AMMOverrideFlags, AMMPerformanceMetrics};
use phase10_betting::router::types::{RoutingStrategy, ChildMarket};
use phase10_betting::types::{U64F64, I64F64};

#[tokio::test]
async fn test_amm_selector_initialization() {
    let program_id = phase10_betting::id();
    let mut program_test = ProgramTest::new(
        "phase10_betting",
        program_id,
        processor!(phase10_betting::entry),
    );

    let trader = Keypair::new();
    program_test.add_account(
        trader.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut context = program_test.start_with_context().await;

    let market_id = [1u8; 32];
    let (amm_selector, _) = Pubkey::find_program_address(
        &[b"amm_selector", &market_id],
        &program_id,
    );

    let init_amm_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(amm_selector, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: phase10_betting::instruction::InitializeAmmSelector {
            market_id,
        }.data(),
    };

    let init_amm_tx = Transaction::new_signed_with_payer(
        &[init_amm_ix],
        Some(&trader.pubkey()),
        &[&trader],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(init_amm_tx).await.unwrap();

    println!("AMM Selector initialized successfully!");
}

#[tokio::test]
async fn test_amm_selection_logic() {
    use phase10_betting::amm::selector::HybridAMMSelector;

    // Test binary market
    let market_type = MarketType::Binary;
    let time_to_expiry = 86_400 * 7; // 7 days
    let amm = HybridAMMSelector::select_amm(
        &market_type,
        time_to_expiry,
        &AMMOverrideFlags::default(),
        &AMMPerformanceMetrics::default(),
    );
    assert_eq!(amm, AMMType::LMSR);

    // Test binary market close to expiry
    let time_to_expiry = 86_400 / 2; // 12 hours
    let amm = HybridAMMSelector::select_amm(
        &market_type,
        time_to_expiry,
        &AMMOverrideFlags::default(),
        &AMMPerformanceMetrics::default(),
    );
    assert_eq!(amm, AMMType::PMAMM);

    // Test multi-outcome
    let market_type = MarketType::MultiOutcome { count: 10 };
    let amm = HybridAMMSelector::select_amm(
        &market_type,
        time_to_expiry,
        &AMMOverrideFlags::default(),
        &AMMPerformanceMetrics::default(),
    );
    assert_eq!(amm, AMMType::PMAMM);

    // Test continuous
    let market_type = MarketType::Continuous {
        min: I64F64::from_num(0u32),
        max: I64F64::from_num(100u32),
        precision: 2,
    };
    let amm = HybridAMMSelector::select_amm(
        &market_type,
        time_to_expiry,
        &AMMOverrideFlags::default(),
        &AMMPerformanceMetrics::default(),
    );
    assert_eq!(amm, AMMType::L2Distribution);

    println!("AMM selection logic tests passed!");
}

#[tokio::test]
async fn test_synthetic_router_initialization() {
    let program_id = phase10_betting::id();
    let mut program_test = ProgramTest::new(
        "phase10_betting",
        program_id,
        processor!(phase10_betting::entry),
    );

    let creator = Keypair::new();
    program_test.add_account(
        creator.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut context = program_test.start_with_context().await;

    let verse_id = [2u8; 32];
    let (router, _) = Pubkey::find_program_address(
        &[b"synthetic_router", &verse_id],
        &program_id,
    );

    let init_router_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(router, false),
            AccountMeta::new(creator.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: phase10_betting::instruction::InitializeSyntheticRouter {
            verse_id,
            routing_strategy: RoutingStrategy::ProportionalLiquidity,
        }.data(),
    };

    let init_router_tx = Transaction::new_signed_with_payer(
        &[init_router_ix],
        Some(&creator.pubkey()),
        &[&creator],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(init_router_tx).await.unwrap();

    println!("Synthetic Router initialized successfully!");
}

#[tokio::test]
async fn test_routing_weights() {
    use phase10_betting::router::types::SyntheticRouter;

    let mut router = SyntheticRouter {
        router_id: [0; 32],
        verse_id: [1; 32],
        child_markets: vec![
            ChildMarket {
                market_id: "market1".to_string(),
                probability: U64F64::from_num(60u32) / U64F64::from_num(100u32), // 0.6
                volume_7d: 100_000,
                liquidity_depth: 50_000,
                last_update: 0,
                amm_type: AMMType::LMSR,
            },
            ChildMarket {
                market_id: "market2".to_string(),
                probability: U64F64::from_num(65u32) / U64F64::from_num(100u32), // 0.65
                volume_7d: 200_000,
                liquidity_depth: 100_000,
                last_update: 0,
                amm_type: AMMType::PMAMM,
            },
        ],
        routing_weights: vec![],
        aggregated_prob: U64F64::zero(),
        total_liquidity: 150_000,
        routing_strategy: RoutingStrategy::ProportionalLiquidity,
        performance: RouterPerformance::default(),
        last_update_slot: 0,
    };

    // Update weights
    router.update_weights().unwrap();
    assert_eq!(router.routing_weights.len(), 2);

    // First market should have ~33% weight, second ~67%
    assert!(router.routing_weights[0] < router.routing_weights[1]);

    // Update aggregated probability
    router.update_aggregated_probability().unwrap();

    // Should be weighted average closer to 0.65
    assert!(router.aggregated_prob > U64F64::from_num(60u32) / U64F64::from_num(100u32));
    assert!(router.aggregated_prob < U64F64::from_num(65u32) / U64F64::from_num(100u32));

    println!("Routing weight calculation tests passed!");
}

#[tokio::test]
async fn test_route_calculation() {
    use phase10_betting::router::{RouteExecutor, types::*};

    let router = SyntheticRouter {
        router_id: [0; 32],
        verse_id: [1; 32],
        child_markets: vec![
            ChildMarket {
                market_id: "market1".to_string(),
                probability: U64F64::from_num(60u32) / U64F64::from_num(100u32),
                volume_7d: 100_000,
                liquidity_depth: 50_000,
                last_update: 0,
                amm_type: AMMType::LMSR,
            },
            ChildMarket {
                market_id: "market2".to_string(),
                probability: U64F64::from_num(65u32) / U64F64::from_num(100u32),
                volume_7d: 200_000,
                liquidity_depth: 100_000,
                last_update: 0,
                amm_type: AMMType::PMAMM,
            },
        ],
        routing_weights: vec![
            U64F64::from_num(333u32) / U64F64::from_num(1000u32), // 0.333
            U64F64::from_num(667u32) / U64F64::from_num(1000u32), // 0.667
        ],
        aggregated_prob: U64F64::from_num(633u32) / U64F64::from_num(1000u32), // 0.633
        total_liquidity: 150_000,
        routing_strategy: RoutingStrategy::ProportionalLiquidity,
        performance: RouterPerformance::default(),
        last_update_slot: 0,
    };

    let route_result = RouteExecutor::calculate_route(
        &router,
        10_000, // $10k trade
        true,   // buy
    ).unwrap();

    assert_eq!(route_result.route_legs.len(), 2);
    assert_eq!(route_result.unfilled_amount, 0);

    // Check proportional allocation
    assert!(route_result.route_legs[0].size < route_result.route_legs[1].size);

    // Check fees are reasonable
    assert!(route_result.total_fees < 10_000 * 200 / 10_000); // < 2%

    println!("Route calculation tests passed!");
}

#[tokio::test]
async fn test_milestone_achievement() {
    use phase10_betting::bootstrap::MilestoneManager;
    use phase10_betting::state::{BootstrapState, BootstrapMilestone};

    let mut bootstrap_state = BootstrapState {
        current_vault_balance: 15_000 * 10u64.pow(6),
        current_coverage: U64F64::from_num(30u32) / U64F64::from_num(100u32), // 0.3
        unique_traders: 60,
        ..Default::default()
    };

    let mut milestone = BootstrapMilestone {
        index: 1,
        vault_target: 10_000 * 10u64.pow(6),
        coverage_target: U64F64::from_num(25u32) / U64F64::from_num(100u32), // 0.25
        traders_target: 50,
        mmt_bonus_pool: 50_000 * 10u64.pow(6),
        achieved: false,
        achieved_slot: 0,
        top_contributors: vec![],
    };

    let clock = Clock {
        slot: 5000,
        ..Default::default()
    };

    let top_traders = vec![
        (Pubkey::new_unique(), 1000),
        (Pubkey::new_unique(), 900),
        (Pubkey::new_unique(), 800),
    ];

    let achieved = MilestoneManager::check_and_process_milestone(
        &mut bootstrap_state,
        &mut milestone,
        top_traders,
        &clock,
    ).unwrap();

    assert!(achieved);
    assert!(milestone.achieved);
    assert_eq!(milestone.achieved_slot, 5000);
    assert_eq!(milestone.top_contributors.len(), 3);

    println!("Milestone achievement tests passed!");
}

#[tokio::test]
async fn test_slippage_estimation() {
    use phase10_betting::router::RouteExecutor;

    let market = ChildMarket {
        market_id: "test_market".to_string(),
        probability: U64F64::from_num(50u32) / U64F64::from_num(100u32),
        volume_7d: 100_000,
        liquidity_depth: 50_000,
        last_update: 0,
        amm_type: AMMType::LMSR,
    };

    // Test small trade - minimal slippage
    let small_trade = 1_000;
    let slippage = RouteExecutor::estimate_slippage(&market, small_trade);
    assert!(slippage < 10); // Less than 0.1%

    // Test large trade - higher slippage
    let large_trade = 10_000;
    let slippage = RouteExecutor::estimate_slippage(&market, large_trade);
    assert!(slippage > 10 && slippage < 1000); // Between 0.1% and 10%

    // Test edge case - no liquidity
    let no_liquidity_market = ChildMarket {
        liquidity_depth: 0,
        ..market
    };
    let slippage = RouteExecutor::estimate_slippage(&no_liquidity_market, small_trade);
    assert_eq!(slippage, 1000); // Max slippage 10%

    println!("Slippage estimation tests passed!");
}