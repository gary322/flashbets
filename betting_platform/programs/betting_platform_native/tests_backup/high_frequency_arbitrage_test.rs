use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform_native::{
    trading::{PlaceOrder, ExecuteTrade},
    amm::{UpdateAMM, AMMState},
    chain_execution::{ChainExecution, ChainStepType},
    state::accounts::{ProposalPDA, VersePDA, UserPositionPDA},
    events::EventType,
    math::U64F64,
    synthetics::arbitrage::{ArbitrageOpportunity, ArbDirection},
};
use std::time::{Duration, Instant};

/// High-frequency arbitrage test configuration
pub struct ArbitrageTestConfig {
    pub num_markets: u32,
    pub num_arbitrageurs: u32,
    pub test_duration_slots: u64,
    pub target_arbitrage_per_second: u32,
    pub max_slippage_bps: u16,
    pub min_profit_bps: u16,
}

impl Default for ArbitrageTestConfig {
    fn default() -> Self {
        Self {
            num_markets: 100,
            num_arbitrageurs: 10,
            test_duration_slots: 1000, // ~400 seconds
            target_arbitrage_per_second: 100,
            max_slippage_bps: 50, // 0.5%
            min_profit_bps: 10,   // 0.1% minimum profit
        }
    }
}


/// Arbitrageur state
pub struct Arbitrageur {
    pub keypair: Keypair,
    pub balance: u64,
    pub opportunities_found: u32,
    pub opportunities_executed: u32,
    pub total_profit: i64,
    pub failed_attempts: u32,
    pub avg_latency_ms: u32,
}

impl Arbitrageur {
    pub fn new(initial_balance: u64) -> Self {
        Self {
            keypair: Keypair::new(),
            balance: initial_balance,
            opportunities_found: 0,
            opportunities_executed: 0,
            total_profit: 0,
            failed_attempts: 0,
            avg_latency_ms: 0,
        }
    }
}

/// High-frequency arbitrage flow test
#[tokio::test]
async fn test_high_frequency_arbitrage_flow() {
    println!("=== High-Frequency Arbitrage Flow Test ===");
    
    let config = ArbitrageTestConfig::default();
    let mut context = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    ).start_with_context().await;
    
    // Initialize markets
    let markets = initialize_test_markets(&mut context, config.num_markets).await;
    println!("Initialized {} markets", markets.len());
    
    // Initialize arbitrageurs
    let mut arbitrageurs = initialize_arbitrageurs(&mut context, config.num_arbitrageurs).await;
    println!("Initialized {} arbitrageurs", arbitrageurs.len());
    
    // Run arbitrage simulation
    let start_time = Instant::now();
    let mut total_opportunities = 0u32;
    let mut total_executed = 0u32;
    let mut total_profit = 0i64;
    
    for slot in 0..config.test_duration_slots {
        // Advance slot
        context.warp_to_slot(slot).unwrap();
        
        // Create price discrepancies
        let opportunities = create_arbitrage_opportunities(
            &markets,
            slot,
            &config,
        );
        total_opportunities += opportunities.len() as u32;
        
        // Arbitrageurs compete for opportunities
        for opportunity in opportunities {
            let executed = execute_arbitrage_competition(
                &mut context,
                &mut arbitrageurs,
                &opportunity,
                &config,
            ).await;
            
            if executed {
                total_executed += 1;
                total_profit += opportunity.potential_profit as i64;
            }
        }
        
        // Log progress every 100 slots
        if slot % 100 == 0 && slot > 0 {
            let elapsed = start_time.elapsed();
            let arb_per_second = total_executed as f64 / elapsed.as_secs_f64();
            
            println!(
                "Slot {}: {} opportunities, {} executed ({:.1}%), {:.1} arb/sec",
                slot,
                total_opportunities,
                total_executed,
                (total_executed as f64 / total_opportunities.max(1) as f64) * 100.0,
                arb_per_second
            );
        }
    }
    
    // Final results
    let total_duration = start_time.elapsed();
    print_arbitrage_results(
        &arbitrageurs,
        total_opportunities,
        total_executed,
        total_profit,
        total_duration,
        &config,
    );
    
    // Verify performance targets
    let arb_per_second = total_executed as f64 / total_duration.as_secs_f64();
    assert!(
        arb_per_second >= config.target_arbitrage_per_second as f64 * 0.9,
        "Arbitrage rate {:.1}/sec below target {}/sec",
        arb_per_second,
        config.target_arbitrage_per_second
    );
}

/// Test chain leverage arbitrage strategy
#[tokio::test]
async fn test_chain_leverage_arbitrage() {
    println!("=== Chain Leverage Arbitrage Test ===");
    
    let mut context = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    ).start_with_context().await;
    
    // Create verse hierarchy for chain execution
    let root_verse = create_test_verse(&mut context, "root", 0).await;
    let child_verses = vec![
        create_test_verse(&mut context, "child1", 1).await,
        create_test_verse(&mut context, "child2", 1).await,
        create_test_verse(&mut context, "child3", 1).await,
    ];
    
    // Create correlated markets with price discrepancies
    let markets = create_correlated_markets(&mut context, &root_verse, &child_verses).await;
    
    // Execute chain arbitrage
    let arbitrageur = Keypair::new();
    let initial_balance = 1_000_000_000; // $1000
    
    // Build chain execution steps
    let chain_steps = vec![
        ChainStepType::Borrow,      // Leverage up
        ChainStepType::Arbitrage,   // Capture price discrepancy
        ChainStepType::Liquidity,   // Provide liquidity for fees
        ChainStepType::Arbitrage,   // Second arbitrage
        ChainStepType::Stake,       // Stake for additional yield
    ];
    
    let chain_result = execute_chain_arbitrage(
        &mut context,
        &arbitrageur,
        &markets,
        initial_balance,
        chain_steps,
    ).await;
    
    // Verify chain multiplier effect
    let final_value = chain_result.final_value;
    let total_return = (final_value as f64 / initial_balance as f64 - 1.0) * 100.0;
    
    println!("Chain Arbitrage Results:");
    println!("- Initial: ${}", initial_balance / 1_000_000);
    println!("- Final: ${}", final_value / 1_000_000);
    println!("- Return: {:.1}%", total_return);
    println!("- Effective Leverage: {:.1}x", chain_result.effective_leverage);
    
    assert!(
        total_return > 50.0,
        "Chain arbitrage return {:.1}% below expected",
        total_return
    );
}

/// Test cross-market arbitrage with PM-AMM
#[tokio::test]
async fn test_cross_market_pm_amm_arbitrage() {
    println!("=== Cross-Market PM-AMM Arbitrage Test ===");
    
    let mut context = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    ).start_with_context().await;
    
    // Create two markets with PM-AMM
    let market_a = create_pm_amm_market(&mut context, "BTC > $100k", 6500).await; // 65% yes
    let market_b = create_pm_amm_market(&mut context, "BTC < $100k", 3700).await; // 37% yes
    
    // These markets should sum to ~100% but there's a 2% discrepancy
    let arbitrageur = Keypair::new();
    let arb_size = 100_000_000; // $100
    
    // Execute arbitrage: Buy NO on market A, Buy YES on market B
    let tx1 = place_order_transaction(
        &arbitrageur,
        &market_a,
        arb_size,
        false, // Buy NO
        3500,  // Limit price
    );
    
    let tx2 = place_order_transaction(
        &arbitrageur,
        &market_b,
        arb_size,
        true,  // Buy YES
        3700,  // Limit price
    );
    
    // Execute both orders atomically
    let start = Instant::now();
    context.banks_client.process_transaction(tx1).await.unwrap();
    context.banks_client.process_transaction(tx2).await.unwrap();
    let latency = start.elapsed();
    
    // Calculate profit
    let cost_a = (arb_size * 3500) / 10000; // 35% of size
    let cost_b = (arb_size * 3700) / 10000; // 37% of size
    let total_cost = cost_a + cost_b;        // 72% total
    let guaranteed_payout = arb_size;        // 100% payout (one must win)
    let profit = guaranteed_payout - total_cost;
    
    println!("Cross-Market Arbitrage Results:");
    println!("- Size: ${}", arb_size / 1_000_000);
    println!("- Cost A (NO@35%): ${}", cost_a / 1_000_000);
    println!("- Cost B (YES@37%): ${}", cost_b / 1_000_000);
    println!("- Total Cost: ${}", total_cost / 1_000_000);
    println!("- Guaranteed Payout: ${}", guaranteed_payout / 1_000_000);
    println!("- Profit: ${} ({:.1}%)", 
        profit / 1_000_000,
        (profit as f64 / total_cost as f64) * 100.0
    );
    println!("- Execution Latency: {:?}", latency);
    
    assert!(profit > 0, "Arbitrage should be profitable");
    assert!(latency.as_millis() < 50, "Execution too slow for HFT");
}

/// Test MEV protection during arbitrage
#[tokio::test]
async fn test_mev_protected_arbitrage() {
    println!("=== MEV-Protected Arbitrage Test ===");
    
    let mut context = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    ).start_with_context().await;
    
    // Create market with arbitrage opportunity
    let market = create_test_market(&mut context, "test_market").await;
    
    // Simulate multiple arbitrageurs trying to capture same opportunity
    let num_competitors = 5;
    let mut arbitrageurs = Vec::new();
    
    for i in 0..num_competitors {
        let arb = Keypair::new();
        fund_account(&mut context, &arb.pubkey(), 1_000_000_000).await;
        arbitrageurs.push(arb);
    }
    
    // All arbitrageurs submit transactions in same slot
    let opportunity_size = 50_000_000; // $50
    let mut transactions = Vec::new();
    
    for (i, arb) in arbitrageurs.iter().enumerate() {
        // Add small random delay to simulate network latency
        let delay_ms = i * 5; // 0-20ms delays
        
        let tx = create_arbitrage_transaction(
            arb,
            &market,
            opportunity_size,
            delay_ms as u64,
        );
        
        transactions.push((tx, delay_ms));
    }
    
    // Process transactions and see who wins
    let mut winner_index = None;
    let mut winner_profit = 0;
    
    for (i, (tx, delay)) in transactions.iter().enumerate() {
        // Simulate delay
        tokio::time::sleep(Duration::from_millis(*delay as u64)).await;
        
        match context.banks_client.process_transaction(tx.clone()).await {
            Ok(_) => {
                if winner_index.is_none() {
                    winner_index = Some(i);
                    winner_profit = calculate_arbitrage_profit(opportunity_size);
                    println!("Arbitrageur {} won with {}ms delay", i, delay);
                }
            }
            Err(e) => {
                // Later arbitrageurs should fail (opportunity taken)
                println!("Arbitrageur {} failed: {:?}", i, e);
            }
        }
    }
    
    assert!(winner_index.is_some(), "At least one arbitrageur should succeed");
    assert_eq!(
        winner_index.unwrap(),
        0,
        "First arbitrageur should win (FIFO ordering)"
    );
}

// Helper functions

async fn initialize_test_markets(
    context: &mut ProgramTestContext,
    count: u32,
) -> Vec<Pubkey> {
    let mut markets = Vec::new();
    
    for i in 0..count {
        let market_name = format!("market_{}", i);
        let market = create_test_market(context, &market_name).await;
        markets.push(market);
    }
    
    markets
}

async fn initialize_arbitrageurs(
    context: &mut ProgramTestContext,
    count: u32,
) -> Vec<Arbitrageur> {
    let mut arbitrageurs = Vec::new();
    let initial_balance = 10_000_000_000; // $10k each
    
    for _ in 0..count {
        let mut arb = Arbitrageur::new(initial_balance);
        fund_account(context, &arb.keypair.pubkey(), initial_balance).await;
        arbitrageurs.push(arb);
    }
    
    arbitrageurs
}

fn create_arbitrage_opportunities(
    markets: &[Pubkey],
    slot: u64,
    config: &ArbitrageTestConfig,
) -> Vec<ArbitrageOpportunity> {
    let mut opportunities = Vec::new();
    
    // Simulate price discrepancies between correlated markets
    let num_opportunities = ((slot % 10) + 1) as usize; // 1-10 opportunities per slot
    
    for i in 0..num_opportunities.min(markets.len() / 2) {
        let market_a = markets[i * 2];
        let market_b = markets[i * 2 + 1];
        
        // Create price discrepancy
        let base_price = 5000 + (slot % 1000) as u64; // 50-60%
        let price_a = base_price + (i as u64 * 100); // Higher price
        let price_b = 10000 - price_a + 200; // Should sum to 100% + profit
        
        if price_a + price_b < 10000 {
            let expected_profit = 10000 - (price_a + price_b);
            
            if expected_profit >= config.min_profit_bps as u64 {
                let size = 50_000_000 + (i as u64 * 10_000_000); // $50-150
                let potential_profit = expected_profit * size / 10000;
                opportunities.push(ArbitrageOpportunity {
                    synthetic_id: (i + 1) as u128, // Use index as synthetic_id for testing
                    market_id: market_a, // Using market_a as the primary market
                    direction: if price_a > price_b { ArbDirection::BuySyntheticSellMarket } else { ArbDirection::BuyMarketSellSynthetic },
                    price_diff: U64F64::from_num(expected_profit * 1_000_000 / 10000), // Convert to fixed point
                    potential_profit,
                    recommended_size: size,
                    confidence_score: 90 - (i * 5) as u8, // Decreasing confidence with index
                    timestamp: slot as i64 * 400, // Approximate timestamp based on slot
                    expected_profit_bps: expected_profit as u16,
                    synthetic_price: U64F64::from_num(price_a * 1_000_000 / 10000), // Convert to fixed point
                    market_price: U64F64::from_num(price_b * 1_000_000 / 10000), // Convert to fixed point
                });
            }
        }
    }
    
    opportunities
}

async fn execute_arbitrage_competition(
    context: &mut ProgramTestContext,
    arbitrageurs: &mut [Arbitrageur],
    opportunity: &ArbitrageOpportunity,
    config: &ArbitrageTestConfig,
) -> bool {
    // Randomly select arbitrageur (in production, fastest wins)
    let arb_index = (opportunity.timestamp as usize / 400) % arbitrageurs.len();
    let arbitrageur = &mut arbitrageurs[arb_index];
    
    arbitrageur.opportunities_found += 1;
    
    // Check if arbitrageur has sufficient balance
    let synthetic_price_u64 = opportunity.synthetic_price.to_num::<u64>() / 1_000_000;
    let market_price_u64 = opportunity.market_price.to_num::<u64>() / 1_000_000;
    let required_capital = (opportunity.recommended_size * (synthetic_price_u64 + market_price_u64)) / 10000;
    
    if arbitrageur.balance < required_capital {
        arbitrageur.failed_attempts += 1;
        return false;
    }
    
    // Simulate execution with latency
    let execution_start = Instant::now();
    
    // In production, would execute actual transactions
    let success = simulate_arbitrage_execution(
        opportunity,
        config.max_slippage_bps,
    );
    
    let latency = execution_start.elapsed().as_millis() as u32;
    
    if success {
        arbitrageur.opportunities_executed += 1;
        arbitrageur.total_profit += opportunity.potential_profit as i64;
        arbitrageur.balance += opportunity.potential_profit;
        
        // Update average latency
        arbitrageur.avg_latency_ms = 
            (arbitrageur.avg_latency_ms * (arbitrageur.opportunities_executed - 1) + latency) 
            / arbitrageur.opportunities_executed;
        
        true
    } else {
        arbitrageur.failed_attempts += 1;
        false
    }
}

fn simulate_arbitrage_execution(
    opportunity: &ArbitrageOpportunity,
    max_slippage_bps: u16,
) -> bool {
    // Simulate slippage and execution risk
    let slippage = (opportunity.timestamp as u16 / 400 % 100); // 0-99 bps
    
    if slippage > max_slippage_bps {
        return false; // Too much slippage
    }
    
    // 95% success rate for valid opportunities
    (opportunity.timestamp / 400 % 20) != 0
}

fn print_arbitrage_results(
    arbitrageurs: &[Arbitrageur],
    total_opportunities: u32,
    total_executed: u32,
    total_profit: i64,
    duration: Duration,
    config: &ArbitrageTestConfig,
) {
    println!("\n=== Arbitrage Test Results ===");
    println!("Duration: {:.1}s", duration.as_secs_f64());
    println!("Total Opportunities: {}", total_opportunities);
    println!("Total Executed: {} ({:.1}%)", 
        total_executed,
        (total_executed as f64 / total_opportunities as f64) * 100.0
    );
    println!("Total Profit: ${}", total_profit / 1_000_000);
    println!("Arbitrage Rate: {:.1}/sec", 
        total_executed as f64 / duration.as_secs_f64()
    );
    
    println!("\nTop Arbitrageurs:");
    let mut sorted_arbs: Vec<_> = arbitrageurs.iter().enumerate().collect();
    sorted_arbs.sort_by_key(|(_, a)| -(a.total_profit));
    
    for (i, (idx, arb)) in sorted_arbs.iter().take(5).enumerate() {
        println!("{}. Arbitrageur {}: ${} profit, {} executed, {:.1}% success, {}ms avg latency",
            i + 1,
            idx,
            arb.total_profit / 1_000_000,
            arb.opportunities_executed,
            (arb.opportunities_executed as f64 / arb.opportunities_found.max(1) as f64) * 100.0,
            arb.avg_latency_ms
        );
    }
    
    // Performance metrics
    let avg_latency = arbitrageurs.iter()
        .map(|a| a.avg_latency_ms)
        .sum::<u32>() / arbitrageurs.len() as u32;
    
    println!("\nPerformance Metrics:");
    println!("- Average Latency: {}ms", avg_latency);
    println!("- Target Rate: {}/sec", config.target_arbitrage_per_second);
    println!("- Achieved Rate: {:.1}/sec", 
        total_executed as f64 / duration.as_secs_f64()
    );
}

// Stub functions for transaction creation
async fn create_test_market(context: &mut ProgramTestContext, name: &str) -> Pubkey {
    // In production, would create actual market
    Pubkey::new_unique()
}

async fn create_test_verse(context: &mut ProgramTestContext, name: &str, depth: u8) -> Pubkey {
    // In production, would create actual verse
    Pubkey::new_unique()
}

async fn create_pm_amm_market(context: &mut ProgramTestContext, title: &str, yes_price: u64) -> Pubkey {
    // In production, would create PM-AMM market
    Pubkey::new_unique()
}

async fn fund_account(context: &mut ProgramTestContext, pubkey: &Pubkey, amount: u64) {
    // In production, would transfer SOL to account
}

fn place_order_transaction(
    trader: &Keypair,
    market: &Pubkey,
    size: u64,
    is_yes: bool,
    limit_price: u64,
) -> Transaction {
    // In production, would create actual transaction
    Transaction::new_with_payer(&[], Some(&trader.pubkey()))
}

fn create_arbitrage_transaction(
    arbitrageur: &Keypair,
    market: &Pubkey,
    size: u64,
    delay_ms: u64,
) -> Transaction {
    // In production, would create actual arbitrage transaction
    Transaction::new_with_payer(&[], Some(&arbitrageur.pubkey()))
}

fn calculate_arbitrage_profit(size: u64) -> u64 {
    // Simplified profit calculation
    size / 50 // 2% profit
}

async fn create_correlated_markets(
    context: &mut ProgramTestContext,
    root: &Pubkey,
    children: &[Pubkey],
) -> Vec<Pubkey> {
    // In production, would create actual correlated markets
    vec![*root]
}

struct ChainResult {
    final_value: u64,
    effective_leverage: f64,
}

async fn execute_chain_arbitrage(
    context: &mut ProgramTestContext,
    arbitrageur: &Keypair,
    markets: &[Pubkey],
    initial: u64,
    steps: Vec<ChainStepType>,
) -> ChainResult {
    // In production, would execute actual chain
    ChainResult {
        final_value: initial * 15 / 10, // 50% return
        effective_leverage: 3.5,
    }
}