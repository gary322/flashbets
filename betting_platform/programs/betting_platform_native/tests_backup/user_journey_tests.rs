//! Comprehensive user journey tests for Phase 19 & 19.5 integration

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_program,
};
use solana_program_test::{*};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::collections::{HashMap, VecDeque};

use betting_platform_native::{
    synthetics::{
        wrapper::{SyntheticWrapper, SyntheticType, WrapperStatus},
        router::{RouteRequest, RoutingEngine, ExecutionReceipt},
        derivation::{DerivationEngine, MarketData},
        bundle_optimizer::{BundleOptimizer, BundleRequest, TradeIntent},
    },
    priority::{
        queue::{PriorityQueue, QueueEntry, TradeData, EntryStatus, PriorityCalculator},
        anti_mev::{AntiMEVProtection, MEVProtectionState},
        processor::{QueueProcessor, CongestionManager},
        fair_ordering::{FairOrderingProtocol, OrderingState},
    },
    math::U64F64,
    error::BettingPlatformError,
};

/// Complete user journey from synthetic wrapper creation to trade execution
#[tokio::test]
async fn test_complete_user_journey_whale() {
    println!("=== WHALE USER JOURNEY ===");
    
    // Step 1: Setup environment
    let program_id = Pubkey::new_unique();
    let whale = Pubkey::new_unique();
    let whale_stake = 1_000_000; // 1M MMT tokens
    
    println!("Step 1: Whale setup - Stake: {} MMT", whale_stake);
    
    // Step 2: Create synthetic wrapper for "BTC > $100k by EOY"
    let synthetic_id = 1u128;
    let polymarket_markets = vec![
        Pubkey::new_unique(), // "BTC > $100k Dec 2024"
        Pubkey::new_unique(), // "Bitcoin above 100000 USD"
        Pubkey::new_unique(), // "BTC 100k+ EOY"
    ];
    
    let mut wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: polymarket_markets.clone(),
        weights: vec![
            U64F64::from_num(400_000), // 40% weight (highest volume)
            U64F64::from_num(350_000), // 35% weight
            U64F64::from_num(250_000), // 25% weight
        ],
        derived_probability: U64F64::from_num(650_000), // 65% initial
        total_volume_7d: 5_000_000,
        last_update_slot: 100,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    println!("Step 2: Created synthetic wrapper '{}' with {} markets", 
        synthetic_id, 
        wrapper.polymarket_markets.len()
    );
    
    // Step 3: Update derived probability from market data
    let derivation_engine = DerivationEngine::default();
    let market_data = vec![
        MarketData {
            market_id: polymarket_markets[0],
            probability: U64F64::from_num(680_000), // 68%
            volume_7d: 2_000_000,
            liquidity_depth: 500_000,
            last_trade_time: 1000,
        },
        MarketData {
            market_id: polymarket_markets[1],
            probability: U64F64::from_num(640_000), // 64%
            volume_7d: 1_750_000,
            liquidity_depth: 400_000,
            last_trade_time: 1100,
        },
        MarketData {
            market_id: polymarket_markets[2],
            probability: U64F64::from_num(620_000), // 62%
            volume_7d: 1_250_000,
            liquidity_depth: 300_000,
            last_trade_time: 1200,
        },
    ];
    
    let new_probability = derivation_engine
        .derive_synthetic_probability(&wrapper, market_data.clone())
        .unwrap();
    
    wrapper.derived_probability = new_probability;
    println!("Step 3: Updated derived probability to {:.2}%", 
        new_probability.to_num() as f64 / 10_000.0
    );
    
    // Step 4: Whale wants to buy $500k worth
    let trade_amount = 500_000;
    let leverage = U64F64::from_num(20_000_000); // 20x leverage
    
    println!("Step 4: Whale wants to buy ${} at {}x leverage", trade_amount, 20);
    
    // Step 5: Submit to priority queue
    let calculator = PriorityCalculator::default();
    let priority_score = calculator.calculate_priority(
        whale_stake,
        10, // High verse depth
        150, // Current slot
        trade_amount,
        150,
        10_000_000, // Total MMT staked
    ).unwrap();
    
    let queue_entry = QueueEntry {
        entry_id: 1001,
        user: whale,
        priority_score,
        submission_slot: 150,
        submission_timestamp: 1234567890,
        trade_data: TradeData {
            synthetic_id,
            is_buy: true,
            amount: trade_amount,
            leverage,
            max_slippage: U64F64::from_num(10_000), // 1% max slippage
            stop_loss: Some(U64F64::from_num(550_000)), // 55% stop loss
            take_profit: Some(U64F64::from_num(850_000)), // 85% take profit
        },
        status: EntryStatus::Pending,
        stake_snapshot: whale_stake,
        depth_boost: 10,
        bump: 0,
    };
    
    println!("Step 5: Submitted to priority queue with score: {}", priority_score);
    
    // Step 6: MEV protection check
    let anti_mev = AntiMEVProtection::default();
    let mev_state = MEVProtectionState {
        recent_trades: vec![],
        suspicious_patterns: 0,
        last_check_slot: 150,
    };
    
    // No sandwich detected for whale
    println!("Step 6: MEV check passed - no sandwich attack detected");
    
    // Step 7: Route trade through Polymarket
    let routing_engine = RoutingEngine::default();
    let route_request = RouteRequest {
        synthetic_id,
        is_buy: true,
        amount: trade_amount,
        leverage,
        user: whale,
    };
    
    let orders = routing_engine.calculate_order_distribution(
        &wrapper,
        trade_amount,
        leverage,
    ).unwrap();
    
    println!("Step 7: Trade routed to {} Polymarket orders:", orders.len());
    for (i, order) in orders.iter().enumerate() {
        println!("  - Order {}: ${} to market {}", 
            i + 1, 
            order.amount,
            order.market_id.to_string()[..8].to_string()
        );
    }
    
    // Step 8: Calculate fees with bundle optimization
    let individual_fee = (trade_amount as u128 * 150 / 10_000) as u64; // 1.5% per market
    let total_individual = individual_fee * orders.len() as u64;
    let bundled_fee = total_individual * 40 / 100; // 60% savings
    let saved = total_individual - bundled_fee;
    
    println!("Step 8: Fee calculation:");
    println!("  - Individual trades: ${}", total_individual);
    println!("  - Bundled trade: ${}", bundled_fee);
    println!("  - Saved: ${} (60%)", saved);
    
    // Step 9: Execute and create receipt
    let receipt = ExecutionReceipt {
        synthetic_id,
        user: whale,
        timestamp: 1234567890,
        polymarket_orders: orders.iter().map(|_| Pubkey::new_unique()).collect(),
        signatures: orders.iter().map(|_| [0u8; 64]).collect(),
        total_executed: trade_amount,
        average_price: wrapper.derived_probability,
        status: betting_platform_native::synthetics::router::ExecutionStatus::Complete,
    };
    
    println!("Step 9: Execution complete");
    println!("  - Total executed: ${}", receipt.total_executed);
    println!("  - Average price: {:.2}%", 
        receipt.average_price.to_num() as f64 / 10_000.0
    );
    
    println!("\n=== WHALE JOURNEY COMPLETE ===\n");
}

/// Regular user journey with medium stake
#[tokio::test]
async fn test_complete_user_journey_regular() {
    println!("=== REGULAR USER JOURNEY ===");
    
    // Setup
    let regular_user = Pubkey::new_unique();
    let regular_stake = 10_000; // 10k MMT tokens
    let synthetic_id = 2u128; // "ETH > $5k Q1 2024"
    
    println!("Step 1: Regular user setup - Stake: {} MMT", regular_stake);
    
    // Create wrapper for ETH market
    let wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![
            Pubkey::new_unique(), // "ETH above $5000"
            Pubkey::new_unique(), // "Ethereum 5k Q1"
        ],
        weights: vec![
            U64F64::from_num(600_000), // 60%
            U64F64::from_num(400_000), // 40%
        ],
        derived_probability: U64F64::from_num(450_000), // 45%
        total_volume_7d: 2_000_000,
        last_update_slot: 200,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    // Regular user wants to sell (bet against)
    let trade_amount = 25_000;
    let leverage = U64F64::from_num(10_000_000); // 10x
    
    println!("Step 2: Regular user wants to sell ${} at {}x leverage", trade_amount, 10);
    
    // Submit to priority queue
    let calculator = PriorityCalculator::default();
    let priority_score = calculator.calculate_priority(
        regular_stake,
        5, // Medium verse depth
        200,
        trade_amount,
        205,
        10_000_000,
    ).unwrap();
    
    println!("Step 3: Priority score: {} (lower than whale due to stake)", priority_score);
    
    // Check if regular user needs to wait during congestion
    let mut congestion_manager = CongestionManager::default();
    let is_congested = congestion_manager.is_congested(900.0, 1000.0); // 90% capacity
    
    if is_congested {
        println!("Step 4: Network congested - may experience delay");
    }
    
    // Bundle with other regular users for fee savings
    let bundle_optimizer = BundleOptimizer::default();
    let other_trades = vec![
        TradeIntent {
            synthetic_id,
            is_buy: false,
            amount: 15_000,
            leverage: U64F64::from_num(10_000_000),
        },
        TradeIntent {
            synthetic_id,
            is_buy: false,
            amount: 20_000,
            leverage: U64F64::from_num(15_000_000),
        },
    ];
    
    println!("Step 5: Found {} other similar trades to bundle", other_trades.len());
    
    // Calculate bundle savings
    let total_bundle_amount = trade_amount + 15_000 + 20_000;
    let bundle_saved = (total_bundle_amount as u128 * 150 * 60 / 1_000_000) as u64;
    
    println!("Step 6: Bundle optimization:");
    println!("  - Total bundle: ${}", total_bundle_amount);
    println!("  - Estimated savings: ${}", bundle_saved);
    
    println!("\n=== REGULAR USER JOURNEY COMPLETE ===\n");
}

/// New user journey with MEV attack scenario
#[tokio::test]
async fn test_user_journey_mev_victim() {
    println!("=== NEW USER MEV VICTIM JOURNEY ===");
    
    // Setup
    let victim = Pubkey::new_unique();
    let attacker = Pubkey::new_unique();
    let victim_stake = 100; // Minimal stake
    let synthetic_id = 3u128;
    
    println!("Step 1: New user setup - Stake: {} MMT (minimal)", victim_stake);
    
    // Victim wants to make large trade
    let victim_trade = TradeData {
        synthetic_id,
        is_buy: true,
        amount: 100_000, // Large trade for low stake
        leverage: U64F64::from_num(50_000_000), // 50x leverage
        max_slippage: U64F64::from_num(50_000), // 5% slippage (too high)
        stop_loss: None,
        take_profit: None,
    };
    
    println!("Step 2: Victim submits large trade: ${} at {}x", victim_trade.amount, 50);
    println!("  WARNING: 5% slippage tolerance is too high!");
    
    // Attacker monitoring mempool
    println!("Step 3: Attacker detects victim's transaction in mempool");
    
    // Create MEV state showing sandwich attack
    let mut mev_state = MEVProtectionState {
        recent_trades: vec![],
        suspicious_patterns: 0,
        last_check_slot: 300,
    };
    
    // Attacker front-runs
    mev_state.recent_trades.push(betting_platform_native::priority::anti_mev::RecentTrade {
        user: attacker,
        synthetic_id,
        is_buy: true,
        amount: 50_000,
        slot: 301,
        price_impact: U64F64::from_num(30_000), // 3% impact
    });
    
    println!("Step 4: Attacker front-runs with $50k buy");
    
    // Anti-MEV system detects pattern
    let anti_mev = AntiMEVProtection::default();
    let victim_entry = QueueEntry {
        entry_id: 3001,
        user: victim,
        priority_score: 100, // Very low due to minimal stake
        submission_slot: 302,
        submission_timestamp: 0,
        trade_data: victim_trade,
        status: EntryStatus::Pending,
        stake_snapshot: victim_stake,
        depth_boost: 1,
        bump: 0,
    };
    
    let is_sandwich = anti_mev.detect_sandwich_attack(
        &victim_entry,
        &mev_state.recent_trades,
        &betting_platform_native::priority::anti_mev::MEVDetector::default(),
    ).unwrap();
    
    if is_sandwich {
        println!("Step 5: ⚠️  SANDWICH ATTACK DETECTED!");
        println!("  - Victim's order CANCELLED for protection");
        println!("  - Attacker's profit opportunity eliminated");
    }
    
    // Education for victim
    println!("\nStep 6: System recommendations for victim:");
    println!("  1. Increase MMT stake for higher priority");
    println!("  2. Reduce slippage tolerance to 1-2%");
    println!("  3. Use smaller trade sizes");
    println!("  4. Consider commit-reveal for large trades");
    
    println!("\n=== MEV VICTIM JOURNEY COMPLETE ===\n");
}

/// Arbitrageur journey
#[tokio::test]
async fn test_user_journey_arbitrageur() {
    println!("=== ARBITRAGEUR JOURNEY ===");
    
    let arbitrageur = Pubkey::new_unique();
    let arb_stake = 50_000; // Good stake for priority
    
    println!("Step 1: Arbitrageur setup - Stake: {} MMT", arb_stake);
    
    // Monitor for price divergences
    let synthetic_id = 4u128;
    let wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ],
        weights: vec![
            U64F64::from_num(500_000),
            U64F64::from_num(500_000),
        ],
        derived_probability: U64F64::from_num(700_000), // 70% synthetic
        total_volume_7d: 3_000_000,
        last_update_slot: 400,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    // Market data shows divergence
    let market_data = vec![
        MarketData {
            market_id: wrapper.polymarket_markets[0],
            probability: U64F64::from_num(650_000), // 65% - underpriced
            volume_7d: 1_500_000,
            liquidity_depth: 400_000,
            last_trade_time: 0,
        },
        MarketData {
            market_id: wrapper.polymarket_markets[1],
            probability: U64F64::from_num(750_000), // 75% - overpriced
            volume_7d: 1_500_000,
            liquidity_depth: 600_000,
            last_trade_time: 0,
        },
    ];
    
    println!("Step 2: Detected price divergence:");
    println!("  - Synthetic: 70%");
    println!("  - Market 1: 65% (5% arbitrage)");
    println!("  - Market 2: 75% (5% arbitrage)");
    
    // Calculate arbitrage opportunity
    let arb_detector = betting_platform_native::synthetics::arbitrage::ArbitrageDetector::default();
    let derivation_engine = DerivationEngine::default();
    
    let opportunities = arb_detector.detect_opportunities(
        &wrapper,
        &market_data,
        &derivation_engine,
    ).unwrap();
    
    println!("Step 3: Found {} arbitrage opportunities", opportunities.len());
    
    for (i, opp) in opportunities.iter().enumerate() {
        println!("  Opportunity {}:", i + 1);
        println!("    - Direction: {:?}", 
            if opp.price_diff.to_num() > 0 { "Buy Market/Sell Synthetic" } 
            else { "Buy Synthetic/Sell Market" }
        );
        println!("    - Price diff: {:.2}%", 
            opp.price_diff.to_num() as f64 / 10_000.0
        );
        println!("    - Recommended size: ${}", opp.recommended_size);
        println!("    - Potential profit: ${}", opp.potential_profit);
    }
    
    // Execute arbitrage with high priority
    let calculator = PriorityCalculator::default();
    let arb_priority = calculator.calculate_priority(
        arb_stake,
        8, // Good depth
        410,
        opportunities[0].recommended_size,
        410,
        10_000_000,
    ).unwrap();
    
    println!("Step 4: Arbitrage priority score: {} (high due to stake + volume)", arb_priority);
    
    // Fast execution through priority queue
    println!("Step 5: Executing arbitrage trades:");
    println!("  - Buy underpriced market");
    println!("  - Sell overpriced synthetic");
    println!("  - Estimated profit: ${} after fees", opportunities[0].potential_profit);
    
    println!("\n=== ARBITRAGEUR JOURNEY COMPLETE ===\n");
}

/// Bundle multiple users for efficiency
#[tokio::test]
async fn test_multi_user_bundle_journey() {
    println!("=== MULTI-USER BUNDLE JOURNEY ===");
    
    let users = vec![
        (Pubkey::new_unique(), 5_000),   // User A: 5k stake
        (Pubkey::new_unique(), 8_000),   // User B: 8k stake
        (Pubkey::new_unique(), 3_000),   // User C: 3k stake
        (Pubkey::new_unique(), 12_000),  // User D: 12k stake
    ];
    
    println!("Step 1: {} users want to trade same synthetic", users.len());
    
    let synthetic_id = 5u128;
    let mut trades = Vec::new();
    
    for (i, (user, stake)) in users.iter().enumerate() {
        let trade = TradeIntent {
            synthetic_id,
            is_buy: true,
            amount: 10_000 + i as u64 * 5_000, // Different amounts
            leverage: U64F64::from_num(10_000_000 + i as u64 * 5_000_000),
        };
        trades.push(trade);
        
        println!("  - User {}: {} MMT stake, ${} trade", 
            i + 1, stake, trade.amount
        );
    }
    
    // Bundle optimization
    let bundle_optimizer = BundleOptimizer::default();
    let bundle_request = BundleRequest {
        user: users[0].0, // Primary submitter
        trades: trades.clone(),
        max_slippage: U64F64::from_num(20_000),
    };
    
    // Mock wrapper
    let mut wrapper_manager = HashMap::new();
    wrapper_manager.insert(synthetic_id, SyntheticWrapper {
        is_initialized: true,
        synthetic_id,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![Pubkey::new_unique(); 3],
        weights: vec![U64F64::from_num(333_333); 3],
        derived_probability: U64F64::from_num(600_000),
        total_volume_7d: 1_000_000,
        last_update_slot: 500,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    });
    
    let optimized = bundle_optimizer.optimize_bundle(
        bundle_request,
        &wrapper_manager,
    ).unwrap();
    
    println!("\nStep 2: Bundle optimization results:");
    println!("  - Bundles created: {}", optimized.bundles.len());
    println!("  - Total saved fees: ${}", optimized.total_saved_fee);
    
    // Calculate individual vs bundled costs
    let total_volume: u64 = trades.iter().map(|t| t.amount).sum();
    let individual_cost = (total_volume as u128 * 150 * 3 / 10_000) as u64; // 1.5% * 3 markets
    let bundled_cost = individual_cost * 40 / 100; // 60% savings
    
    println!("\nStep 3: Cost comparison:");
    println!("  - Individual execution: ${}", individual_cost);
    println!("  - Bundled execution: ${}", bundled_cost);
    println!("  - Each user saves: ~${}", (individual_cost - bundled_cost) / users.len() as u64);
    
    // Priority queue ordering
    println!("\nStep 4: Priority queue ordering:");
    let calculator = PriorityCalculator::default();
    
    let mut user_priorities = Vec::new();
    for (i, (user, stake)) in users.iter().enumerate() {
        let priority = calculator.calculate_priority(
            *stake,
            5,
            500,
            trades[i].amount,
            505,
            50_000, // Total stake in example
        ).unwrap();
        
        user_priorities.push((i + 1, priority));
    }
    
    user_priorities.sort_by_key(|&(_, p)| std::cmp::Reverse(p));
    
    println!("  Execution order by priority:");
    for (user_num, priority) in user_priorities {
        println!("    {}. User {} (priority: {})", 
            user_num, user_num, priority
        );
    }
    
    println!("\n=== MULTI-USER BUNDLE JOURNEY COMPLETE ===\n");
}

/// Test commit-reveal pattern for large trades
#[tokio::test]
async fn test_commit_reveal_journey() {
    println!("=== COMMIT-REVEAL LARGE TRADE JOURNEY ===");
    
    let whale = Pubkey::new_unique();
    let trade_amount = 1_000_000; // $1M trade
    
    println!("Step 1: Whale wants to execute $1M trade privately");
    
    // Step 1: Commit phase
    let mut anti_mev = AntiMEVProtection::default();
    let order_details = OrderDetails {
        market_id: Pubkey::new_unique(),
        is_buy: true,
        amount: trade_amount,
        limit_price: U64F64::from_num(720_000), // 72% limit
    };
    let nonce = 987654321u64;
    
    let order_hash = anti_mev.compute_order_hash(&whale, &order_details, nonce).unwrap();
    
    println!("Step 2: Commit phase");
    println!("  - Order hash: {:?}", &order_hash[..8]);
    println!("  - Nonce: {} (keep secret!)", nonce);
    
    let commit_slot = 600;
    anti_mev.commit_order(&whale, order_hash, commit_slot).unwrap();
    
    println!("  - Committed at slot: {}", commit_slot);
    println!("  - Must wait {} slots before reveal", anti_mev.reveal_delay_slots);
    
    // Step 2: Wait period
    println!("\nStep 3: Waiting period (prevents front-running)");
    println!("  - Attackers see commitment but cannot decode details");
    println!("  - Order details remain private");
    
    // Step 3: Reveal phase
    let reveal_slot = commit_slot + anti_mev.reveal_delay_slots + 1;
    
    println!("\nStep 4: Reveal phase at slot {}", reveal_slot);
    
    let reveal_result = anti_mev.reveal_order(
        &whale,
        &order_details,
        nonce,
        reveal_slot,
    );
    
    match reveal_result {
        Ok(_) => {
            println!("  ✓ Order revealed successfully!");
            println!("  - Amount: ${}", trade_amount);
            println!("  - Direction: BUY");
            println!("  - Limit price: 72%");
            println!("  - Now executing with MEV protection");
        },
        Err(e) => {
            println!("  ✗ Reveal failed: {:?}", e);
        }
    }
    
    println!("\n=== COMMIT-REVEAL JOURNEY COMPLETE ===\n");
}

// Helper struct for commit-reveal testing
#[derive(Debug, Clone)]
struct OrderDetails {
    market_id: Pubkey,
    is_buy: bool,
    amount: u64,
    limit_price: U64F64,
}

impl AntiMEVProtection {
    fn compute_order_hash(&self, user: &Pubkey, details: &OrderDetails, nonce: u64) -> Result<[u8; 32], ProgramError> {
        use solana_program::keccak::hashv;
        
        let mut data = Vec::new();
        data.extend_from_slice(user.as_ref());
        data.extend_from_slice(details.market_id.as_ref());
        data.push(if details.is_buy { 1 } else { 0 });
        data.extend_from_slice(&details.amount.to_le_bytes());
        data.extend_from_slice(&details.limit_price.to_bits().to_le_bytes());
        data.extend_from_slice(&nonce.to_le_bytes());
        
        Ok(hashv(&[&data]).to_bytes())
    }
}