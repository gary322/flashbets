use ::phase10_betting::*;
use anchor_lang::prelude::*;

/// Simulate a complete user journey from bootstrap to trading
fn simulate_early_trader_journey() {
    println!("\n=== Early Trader Journey Simulation ===\n");

    // 1. System starts with $0 vault
    let mut bootstrap_state = BootstrapState {
        epoch: 1,
        initial_vault_balance: 0,
        current_vault_balance: 0,
        bootstrap_mmt_allocation: 2_000_000 * 10u64.pow(6),
        mmt_distributed: 0,
        unique_traders: 0,
        total_volume: 0,
        status: BootstrapStatus::Active,
        initial_coverage: U64F64::zero(),
        current_coverage: U64F64::zero(),
        target_coverage: U64F64::one(),
        start_slot: 1000,
        expected_end_slot: 1000 + 38_880_000,
        early_bonus_multiplier: U64F64::from_num(2u32),
        early_traders_count: 0,
        max_early_traders: 100,
        min_trade_size: 10 * 10u64.pow(6),
        bootstrap_fee_bps: 28,
        _padding: [0; 256],
    };

    println!("Initial state:");
    println!("  Vault balance: ${}", bootstrap_state.current_vault_balance / 10u64.pow(6));
    println!("  Coverage: {:.2}%", bootstrap_state.current_coverage.to_num::<f64>() * 100.0);
    println!("  Fee: {} bps", bootstrap_state.calculate_bootstrap_fee());
    println!("  MMT allocation: {} MMT\n", bootstrap_state.bootstrap_mmt_allocation / 10u64.pow(6));

    // 2. First trader arrives
    let mut trader1 = BootstrapTrader {
        trader: Pubkey::new_unique(),
        volume_traded: 0,
        mmt_earned: 0,
        trade_count: 0,
        is_early_trader: false,
        first_trade_slot: 0,
        avg_leverage: U64F64::zero(),
        vault_contribution: 0,
        referral_bonus: 0,
        referred_count: 0,
    };

    let clock = Clock {
        slot: 2000,
        epoch_start_timestamp: 0,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: 1234567890,
    };

    // First trade: $10,000 at 5x leverage
    let trade_volume = 10_000 * 10u64.pow(6);
    let leverage = U64F64::from_num(5u32);
    let fee_bps = bootstrap_state.calculate_bootstrap_fee(); // 28 bps
    let fee_paid = (trade_volume as u128 * fee_bps as u128 / 10_000) as u64;

    println!("Trader 1 (Early Trader) - First Trade:");
    println!("  Volume: ${}", trade_volume / 10u64.pow(6));
    println!("  Leverage: {}x", leverage.to_num::<u32>());
    println!("  Fee: ${} ({} bps)", fee_paid / 10u64.pow(6), fee_bps);

    let result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut trader1,
        trade_volume,
        fee_paid,
        leverage,
        &clock,
    ).unwrap();

    println!("  Results:");
    println!("    MMT earned: {} MMT", result.mmt_reward / 10u64.pow(6));
    println!("    Fee rebate: ${}", result.fee_rebate / 10u64.pow(6));
    println!("    Net fee to vault: ${}", result.net_fee / 10u64.pow(6));
    println!("    Is early trader: {}", result.is_early_trader);
    println!("    New vault balance: ${}", bootstrap_state.current_vault_balance / 10u64.pow(6));
    println!("    Unique traders: {}\n", bootstrap_state.unique_traders);

    // 3. Calculate new coverage after trade
    let total_oi = trade_volume * leverage.to_num::<u64>(); // $50k open interest
    bootstrap_state.current_coverage = BootstrapIncentiveEngine::calculate_bootstrap_coverage(
        bootstrap_state.current_vault_balance,
        total_oi,
        true,
    );

    println!("After trade coverage update:");
    println!("  Open Interest: ${}", total_oi / 10u64.pow(6));
    println!("  Coverage: {:.4}%", bootstrap_state.current_coverage.to_num::<f64>() * 100.0);
    println!("  New fee: {} bps\n", bootstrap_state.calculate_bootstrap_fee());

    // 4. More traders join
    for i in 2..=10 {
        let mut trader = BootstrapTrader {
            trader: Pubkey::new_unique(),
            volume_traded: 0,
            mmt_earned: 0,
            trade_count: 0,
            is_early_trader: false,
            first_trade_slot: 0,
            avg_leverage: U64F64::zero(),
            vault_contribution: 0,
            referral_bonus: 0,
            referred_count: 0,
        };

        let trade_vol = (5_000 + i * 1_000) * 10u64.pow(6);
        let lev = U64F64::from_num(3u32);
        let fee_bps = bootstrap_state.calculate_bootstrap_fee();
        let fee = (trade_vol as u128 * fee_bps as u128 / 10_000) as u64;

        let _ = BootstrapIncentiveEngine::process_bootstrap_trade(
            &mut bootstrap_state,
            &mut trader,
            trade_vol,
            fee,
            lev,
            &clock,
        ).unwrap();
    }

    println!("After 10 traders:");
    println!("  Vault balance: ${}", bootstrap_state.current_vault_balance / 10u64.pow(6));
    println!("  Total volume: ${}", bootstrap_state.total_volume / 10u64.pow(6));
    println!("  MMT distributed: {} MMT", bootstrap_state.mmt_distributed / 10u64.pow(6));
    println!("  Unique traders: {}", bootstrap_state.unique_traders);
    println!("  Early traders: {}\n", bootstrap_state.early_traders_count);

    // 5. Check milestone
    let milestones = MilestoneManager::get_bootstrap_milestones();
    let (vault_target, coverage_target, traders_target, bonus_pool) = milestones[0];
    
    println!("First Milestone Check:");
    println!("  Vault target: ${} (current: ${})", 
        vault_target / 10u64.pow(6), 
        bootstrap_state.current_vault_balance / 10u64.pow(6));
    println!("  Coverage target: {:.1}% (current: {:.4}%)", 
        coverage_target.to_num::<f64>() * 100.0,
        bootstrap_state.current_coverage.to_num::<f64>() * 100.0);
    println!("  Traders target: {} (current: {})", 
        traders_target, 
        bootstrap_state.unique_traders);
    println!("  Bonus pool: {} MMT\n", bonus_pool / 10u64.pow(6));
}

/// Simulate a synthetic router trade
fn simulate_synthetic_routing_journey() {
    println!("\n=== Synthetic Routing Journey Simulation ===\n");

    // Create a verse with multiple child markets
    let router = SyntheticRouter {
        router_id: [0; 32],
        verse_id: [1; 32],
        child_markets: vec![
            ChildMarket {
                market_id: "biden-wins-2024".to_string(),
                probability: U64F64::from_num(45u32) / U64F64::from_num(100u32), // 45%
                volume_7d: 500_000 * 10u64.pow(6),
                liquidity_depth: 200_000 * 10u64.pow(6),
                last_update: 0,
                amm_type: AMMType::LMSR,
            },
            ChildMarket {
                market_id: "dem-wins-2024".to_string(),
                probability: U64F64::from_num(48u32) / U64F64::from_num(100u32), // 48%
                volume_7d: 800_000 * 10u64.pow(6),
                liquidity_depth: 300_000 * 10u64.pow(6),
                last_update: 0,
                amm_type: AMMType::PMAMM,
            },
            ChildMarket {
                market_id: "biden-nominee-2024".to_string(),
                probability: U64F64::from_num(42u32) / U64F64::from_num(100u32), // 42%
                volume_7d: 300_000 * 10u64.pow(6),
                liquidity_depth: 150_000 * 10u64.pow(6),
                last_update: 0,
                amm_type: AMMType::LMSR,
            },
        ],
        routing_weights: vec![
            U64F64::from_num(30u32) / U64F64::from_num(100u32), // 30%
            U64F64::from_num(45u32) / U64F64::from_num(100u32), // 45%
            U64F64::from_num(25u32) / U64F64::from_num(100u32), // 25%
        ],
        aggregated_prob: U64F64::from_num(45u32) / U64F64::from_num(100u32), // ~45%
        total_liquidity: 650_000 * 10u64.pow(6),
        routing_strategy: RoutingStrategy::ProportionalLiquidity,
        performance: RouterPerformance::default(),
        last_update_slot: 0,
    };

    println!("Synthetic Verse: Biden 2024 Election Markets");
    println!("Child markets:");
    for (i, market) in router.child_markets.iter().enumerate() {
        println!("  {}: {} (prob: {:.1}%, liquidity: ${}k, weight: {:.1}%)",
            i + 1,
            market.market_id,
            market.probability.to_num::<f64>() * 100.0,
            market.liquidity_depth / 10u64.pow(6) / 1000,
            router.routing_weights[i].to_num::<f64>() * 100.0
        );
    }
    println!("Aggregated probability: {:.1}%\n", router.aggregated_prob.to_num::<f64>() * 100.0);

    // Simulate a large trade
    let trade_size = 50_000 * 10u64.pow(6); // $50k trade
    let is_buy = true;

    println!("Trade Details:");
    println!("  Size: ${}", trade_size / 10u64.pow(6));
    println!("  Direction: {}", if is_buy { "BUY" } else { "SELL" });
    println!("  Strategy: {:?}\n", router.routing_strategy);

    let route_result = RouteExecutor::calculate_route(&router, trade_size, is_buy).unwrap();

    println!("Route Execution Results:");
    println!("  Total legs: {}", route_result.route_legs.len());
    for (i, leg) in route_result.route_legs.iter().enumerate() {
        println!("  Leg {}: {} - ${} ({:.1}%)",
            i + 1,
            leg.market_id,
            leg.size / 10u64.pow(6),
            (leg.size as f64 / trade_size as f64) * 100.0
        );
        println!("    Expected price: {:.3}", leg.expected_price.to_num::<f64>());
        println!("    Expected slippage: {} bps", leg.expected_slippage_bps);
        println!("    Fee: ${}", leg.fee / 10u64.pow(6));
    }

    println!("\nSummary:");
    println!("  Total cost: ${}", route_result.total_cost / 10u64.pow(6));
    println!("  Total fees: ${}", route_result.total_fees / 10u64.pow(6));
    println!("  Avg execution price: {:.3}", route_result.avg_execution_price.to_num::<f64>());
    println!("  Total slippage: {} bps", route_result.total_slippage_bps);
    println!("  Unfilled amount: ${}\n", route_result.unfilled_amount / 10u64.pow(6));

    // Compare to individual market execution
    let individual_fee = (trade_size as u128 * 150 / 10_000) as u64; // 1.5% Polymarket fee
    let savings = individual_fee.saturating_sub(route_result.total_fees);
    
    println!("Comparison to Direct Polymarket:");
    println!("  Individual market fee: ${}", individual_fee / 10u64.pow(6));
    println!("  Synthetic route fee: ${}", route_result.total_fees / 10u64.pow(6));
    println!("  Fee savings: ${} ({:.1}%)", 
        savings / 10u64.pow(6),
        (savings as f64 / individual_fee as f64) * 100.0
    );
}

/// Simulate AMM type switching based on market conditions
fn simulate_amm_switching_journey() {
    println!("\n=== AMM Type Switching Journey ===\n");

    // Create a binary market
    let market_type = MarketType::Binary;
    let mut time_to_expiry = 86_400 * 30; // 30 days

    println!("Market: Trump vs Biden 2024");
    println!("Type: Binary\n");

    // Check AMM selection at different times
    let time_points = vec![
        (30, "30 days before"),
        (7, "7 days before"),
        (1, "1 day before"),
        (0, "12 hours before"),
    ];

    for (days, label) in time_points {
        time_to_expiry = if days == 0 { 
            86_400 / 2 // 12 hours
        } else {
            86_400 * days
        };

        let selected_amm = HybridAMMSelector::select_amm(
            &market_type,
            time_to_expiry,
            &AMMOverrideFlags::default(),
            &AMMPerformanceMetrics::default(),
        );

        println!("{} expiry:", label);
        println!("  Selected AMM: {:?}", selected_amm);
        
        match selected_amm {
            AMMType::LMSR => {
                println!("  Reason: Standard binary market, sufficient time");
                println!("  Benefits: Simple, efficient for binary outcomes");
            },
            AMMType::PMAMM => {
                println!("  Reason: Close to expiry, need time decay optimization");
                println!("  Benefits: Better handling of time decay, uniform LVR");
            },
            AMMType::L2Distribution => {
                println!("  Reason: Complex distribution");
                println!("  Benefits: Handles continuous outcomes");
            },
        }
        println!();
    }

    // Test multi-outcome market
    println!("\nMulti-Outcome Market: GOP Primary Winner");
    let multi_market = MarketType::MultiOutcome { count: 8 };
    let selected = HybridAMMSelector::select_amm(
        &multi_market,
        86_400 * 60,
        &AMMOverrideFlags::default(),
        &AMMPerformanceMetrics::default(),
    );
    println!("  Candidates: 8");
    println!("  Selected AMM: {:?}", selected);
    println!("  Reason: PM-AMM optimized for multiple discrete outcomes");
}

/// Simulate coverage ratio progression during bootstrap
fn simulate_coverage_progression() {
    println!("\n=== Coverage Progression Simulation ===\n");

    let mut vault_balance = 0u64;
    let mut total_oi = 0u64;
    
    println!("Bootstrap Coverage Progression:");
    println!("Trade | Vault($) | OI($) | Coverage(%) | Fee(bps)");
    println!("------|----------|-------|-------------|----------");

    for i in 1..=20 {
        // Each trade adds to vault through fees
        let trade_volume = 10_000 * 10u64.pow(6); // $10k trades
        let leverage = 5u64;
        let fee_bps = 28 - (i.min(25)); // Decreasing fee as coverage improves
        let fee = (trade_volume * fee_bps as u64) / 10_000;
        
        vault_balance += fee;
        total_oi += trade_volume * leverage;
        
        let coverage = BootstrapIncentiveEngine::calculate_bootstrap_coverage(
            vault_balance,
            total_oi,
            true,
        );
        
        println!("{:5} | {:8} | {:5} | {:11.4} | {:8}",
            i,
            vault_balance / 10u64.pow(6),
            total_oi / 10u64.pow(6),
            coverage.to_num::<f64>() * 100.0,
            fee_bps
        );
        
        if coverage >= U64F64::one() {
            println!("\n✅ Bootstrap complete! 100% coverage achieved.");
            break;
        }
    }
}

fn main() {
    println!("\n🚀 Phase 10 & 10.5 User Journey Simulations\n");
    println!("This simulation demonstrates the complete implementation of:");
    println!("- Bootstrap incentive system starting from $0 vault");
    println!("- Dynamic fee structure that decreases as coverage improves");
    println!("- Early trader bonuses and MMT rewards");
    println!("- Synthetic routing across multiple Polymarket child markets");
    println!("- Hybrid AMM selection based on market conditions");
    println!("- Coverage ratio progression during bootstrap");
    
    // Run all simulations
    simulate_early_trader_journey();
    simulate_synthetic_routing_journey();
    simulate_amm_switching_journey();
    simulate_coverage_progression();
    
    println!("\n✅ All simulations completed successfully!");
}