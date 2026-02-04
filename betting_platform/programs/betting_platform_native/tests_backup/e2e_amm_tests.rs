//! E2E Tests for Different AMM Types
//! 
//! Comprehensive tests for LMSR, PM-AMM, and L2-AMM functionality

use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
};
use betting_platform_native::*;
use borsh::BorshSerialize;

#[tokio::test]
async fn test_lmsr_binary_market_full_lifecycle() {
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    );
    
    let (mut banks_client, payer, _) = program_test.start().await;
    
    // Create binary market
    let market = create_binary_market(&mut banks_client, &payer, "Trump wins 2024?").await;
    
    // Test initial state
    let market_data = get_market_data(&mut banks_client, &market).await;
    assert_eq!(market_data.outcome_count, 2);
    assert_eq!(market_data.amm_type, AMMType::LMSR);
    assert_eq!(market_data.yes_price, 5000); // 50% initial
    
    // Create traders
    let bull_trader = create_funded_trader(&mut banks_client, &payer, 100_000_000_000).await;
    let bear_trader = create_funded_trader(&mut banks_client, &payer, 100_000_000_000).await;
    
    // Bull buys YES
    execute_lmsr_trade(
        &mut banks_client,
        &bull_trader,
        &market,
        10_000_000_000, // 10k USDC
        true, // YES
    ).await.expect("Bull trade failed");
    
    // Check price moved up
    let market_data = get_market_data(&mut banks_client, &market).await;
    assert!(market_data.yes_price > 5000, "Price should increase after YES buy");
    let price_after_bull = market_data.yes_price;
    
    // Bear buys NO
    execute_lmsr_trade(
        &mut banks_client,
        &bear_trader,
        &market,
        5_000_000_000, // 5k USDC
        false, // NO
    ).await.expect("Bear trade failed");
    
    // Check price moved down
    let market_data = get_market_data(&mut banks_client, &market).await;
    assert!(market_data.yes_price < price_after_bull, "Price should decrease after NO buy");
    
    // Test large trade impact
    let whale = create_funded_trader(&mut banks_client, &payer, 1_000_000_000_000).await;
    
    execute_lmsr_trade(
        &mut banks_client,
        &whale,
        &market,
        100_000_000_000, // 100k USDC - large trade
        true, // YES
    ).await.expect("Whale trade failed");
    
    let market_data = get_market_data(&mut banks_client, &market).await;
    assert!(market_data.yes_price > 8000, "Large trade should move price significantly");
    
    // Resolve market
    resolve_market(&mut banks_client, &payer, &market, 1).await; // YES wins
    
    // Claim winnings
    let bull_winnings = claim_winnings(&mut banks_client, &bull_trader, &market).await;
    let bear_winnings = claim_winnings(&mut banks_client, &bear_trader, &market).await;
    let whale_winnings = claim_winnings(&mut banks_client, &whale, &market).await;
    
    assert!(bull_winnings > 0, "Bull should have winnings");
    assert_eq!(bear_winnings, 0, "Bear should have no winnings");
    assert!(whale_winnings > 100_000_000_000, "Whale should profit");
    
    println!("LMSR Binary Market Test Complete:");
    println!("- Bull winnings: ${}", bull_winnings / 1_000_000);
    println!("- Bear winnings: ${}", bear_winnings / 1_000_000);
    println!("- Whale winnings: ${}", whale_winnings / 1_000_000);
}

#[tokio::test]
async fn test_pmamm_multi_outcome_market() {
    let mut program_test = create_program_test();
    let (mut banks_client, payer, _) = program_test.start().await;
    
    // Create 5-outcome market
    let outcomes = vec![
        "Team A wins",
        "Team B wins", 
        "Team C wins",
        "Team D wins",
        "Draw",
    ];
    
    let market = create_pmamm_market_with_outcomes(
        &mut banks_client,
        &payer,
        "World Cup Winner",
        outcomes.clone(),
    ).await;
    
    // Check initial probabilities (should sum to 1)
    let market_data = get_market_data(&mut banks_client, &market).await;
    let prob_sum: u64 = market_data.outcome_probabilities.iter().sum();
    assert_eq!(prob_sum, 10000, "Initial probabilities should sum to 100%");
    
    // Create traders with different beliefs
    let traders: Vec<(Keypair, usize, u64)> = vec![
        (create_funded_trader(&mut banks_client, &payer, 50_000_000_000).await, 0, 20_000_000_000), // Team A supporter
        (create_funded_trader(&mut banks_client, &payer, 50_000_000_000).await, 1, 15_000_000_000), // Team B supporter
        (create_funded_trader(&mut banks_client, &payer, 50_000_000_000).await, 2, 10_000_000_000), // Team C supporter
        (create_funded_trader(&mut banks_client, &payer, 50_000_000_000).await, 3, 5_000_000_000),  // Team D supporter
        (create_funded_trader(&mut banks_client, &payer, 50_000_000_000).await, 4, 8_000_000_000),  // Draw believer
    ];
    
    // Execute trades
    for (trader, outcome, amount) in &traders {
        execute_pmamm_trade(
            &mut banks_client,
            trader,
            &market,
            *amount,
            *outcome as u8,
        ).await.expect("PM-AMM trade failed");
    }
    
    // Check probabilities changed based on trades
    let market_data = get_market_data(&mut banks_client, &market).await;
    
    // Team A should have highest probability after largest bet
    let max_prob_index = market_data.outcome_probabilities
        .iter()
        .enumerate()
        .max_by_key(|(_, &prob)| prob)
        .map(|(idx, _)| idx)
        .unwrap();
    
    assert_eq!(max_prob_index, 0, "Team A should have highest probability");
    
    // Test Newton-Raphson convergence by checking price consistency
    let test_trader = create_funded_trader(&mut banks_client, &payer, 10_000_000_000).await;
    
    // Small trade shouldn't fail due to convergence issues
    execute_pmamm_trade(
        &mut banks_client,
        &test_trader,
        &market,
        100_000, // Very small trade
        0,
    ).await.expect("Small trade should succeed");
    
    // Large trade should also succeed
    execute_pmamm_trade(
        &mut banks_client,
        &test_trader,
        &market,
        5_000_000_000, // Large trade
        1,
    ).await.expect("Large trade should succeed");
    
    // Resolve market (Team B wins)
    resolve_market(&mut banks_client, &payer, &market, 1).await;
    
    // Only Team B supporters should have winnings
    for (i, (trader, outcome, _)) in traders.iter().enumerate() {
        let winnings = claim_winnings(&mut banks_client, trader, &market).await;
        if *outcome == 1 {
            assert!(winnings > 0, "Team B supporter should have winnings");
        } else {
            assert_eq!(winnings, 0, "Non-winners should have no winnings");
        }
        println!("Trader {} (bet on outcome {}): ${} winnings", i, outcome, winnings / 1_000_000);
    }
}

#[tokio::test]
async fn test_l2amm_continuous_distribution() {
    let mut program_test = create_program_test();
    let (mut banks_client, payer, _) = program_test.start().await;
    
    // Create continuous distribution market
    let market = create_l2amm_market(
        &mut banks_client,
        &payer,
        "ETH price in 30 days",
        2000.0, // Mean $2000
        100.0,  // Std dev $100
    ).await;
    
    // Create traders with different views
    let bull = create_funded_trader(&mut banks_client, &payer, 100_000_000_000).await;
    let bear = create_funded_trader(&mut banks_client, &payer, 100_000_000_000).await;
    let neutral = create_funded_trader(&mut banks_client, &payer, 100_000_000_000).await;
    
    // Bull expects price > $2100
    execute_l2amm_range_trade(
        &mut banks_client,
        &bull,
        &market,
        20_000_000_000, // 20k USDC
        2100.0, // Lower bound
        f64::INFINITY, // Upper bound
    ).await.expect("Bull range trade failed");
    
    // Bear expects price < $1900
    execute_l2amm_range_trade(
        &mut banks_client,
        &bear,
        &market,
        20_000_000_000, // 20k USDC
        0.0, // Lower bound
        1900.0, // Upper bound
    ).await.expect("Bear range trade failed");
    
    // Neutral expects price between $1950-$2050
    execute_l2amm_range_trade(
        &mut banks_client,
        &neutral,
        &market,
        30_000_000_000, // 30k USDC
        1950.0, // Lower bound
        2050.0, // Upper bound
    ).await.expect("Neutral range trade failed");
    
    // Check distribution parameters updated
    let market_data = get_l2amm_market_data(&mut banks_client, &market).await;
    println!("Distribution after trades:");
    println!("- Mean: ${:.2}", market_data.mean);
    println!("- Std Dev: ${:.2}", market_data.std_dev);
    println!("- Skewness: {:.4}", market_data.skewness);
    
    // Test Simpson's integration for probability calculations
    let prob_above_2100 = calculate_probability_above(
        &mut banks_client,
        &market,
        2100.0,
    ).await;
    
    let prob_below_1900 = calculate_probability_below(
        &mut banks_client,
        &market,
        1900.0,
    ).await;
    
    let prob_between = calculate_probability_between(
        &mut banks_client,
        &market,
        1950.0,
        2050.0,
    ).await;
    
    println!("\nProbabilities:");
    println!("- P(price > $2100): {:.2}%", prob_above_2100 * 100.0);
    println!("- P(price < $1900): {:.2}%", prob_below_1900 * 100.0);
    println!("- P($1950 < price < $2050): {:.2}%", prob_between * 100.0);
    
    // Resolve market at $2025
    resolve_continuous_market(&mut banks_client, &payer, &market, 2025.0).await;
    
    // Calculate payouts
    let bull_payout = claim_winnings(&mut banks_client, &bull, &market).await;
    let bear_payout = claim_winnings(&mut banks_client, &bear, &market).await;
    let neutral_payout = claim_winnings(&mut banks_client, &neutral, &market).await;
    
    println!("\nPayouts (market settled at $2025):");
    println!("- Bull (bet > $2100): ${}", bull_payout / 1_000_000);
    println!("- Bear (bet < $1900): ${}", bear_payout / 1_000_000);
    println!("- Neutral (bet $1950-$2050): ${}", neutral_payout / 1_000_000);
    
    assert_eq!(bull_payout, 0, "Bull should lose (price not > $2100)");
    assert_eq!(bear_payout, 0, "Bear should lose (price not < $1900)");
    assert!(neutral_payout > 30_000_000_000, "Neutral should win and profit");
}

#[tokio::test]
async fn test_amm_auto_selection() {
    let mut program_test = create_program_test();
    let (mut banks_client, payer, _) = program_test.start().await;
    
    // Test N=1 -> LMSR (though this is technically invalid, test error handling)
    let result = create_market_with_outcomes(
        &mut banks_client,
        &payer,
        "Invalid single outcome",
        1,
    ).await;
    assert!(result.is_err(), "Single outcome market should fail");
    
    // Test N=2 -> LMSR
    let binary_market = create_market_with_outcomes(
        &mut banks_client,
        &payer,
        "Binary market",
        2,
    ).await.unwrap();
    
    let market_data = get_market_data(&mut banks_client, &binary_market).await;
    assert_eq!(market_data.amm_type, AMMType::LMSR, "Binary should use LMSR");
    
    // Test N=3-20 -> PM-AMM
    for n in 3..=20 {
        let market = create_market_with_outcomes(
            &mut banks_client,
            &payer,
            &format!("{}-outcome market", n),
            n,
        ).await.unwrap();
        
        let market_data = get_market_data(&mut banks_client, &market).await;
        assert_eq!(market_data.amm_type, AMMType::PMAMM, "{}-outcome should use PM-AMM", n);
    }
    
    // Test continuous -> L2-AMM
    let continuous_market = create_continuous_market(
        &mut banks_client,
        &payer,
        "Continuous market",
    ).await.unwrap();
    
    let market_data = get_market_data(&mut banks_client, &continuous_market).await;
    assert_eq!(market_data.amm_type, AMMType::L2AMM, "Continuous should use L2-AMM");
    
    println!("AMM auto-selection test passed!");
}

// Helper function implementations would go here...
async fn create_binary_market(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    title: &str,
) -> Pubkey {
    // Implementation
    Keypair::new().pubkey()
}

async fn create_funded_trader(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    amount: u64,
) -> Keypair {
    let trader = Keypair::new();
    // Fund the trader account
    trader
}

async fn execute_lmsr_trade(
    banks_client: &mut BanksClient,
    trader: &Keypair,
    market: &Pubkey,
    amount: u64,
    is_yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Implementation
    Ok(())
}

async fn get_market_data(
    banks_client: &mut BanksClient,
    market: &Pubkey,
) -> MarketData {
    // Implementation - would fetch and deserialize market account
    MarketData {
        outcome_count: 2,
        amm_type: AMMType::LMSR,
        yes_price: 5000,
        outcome_probabilities: vec![5000, 5000],
        ..Default::default()
    }
}

async fn resolve_market(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    market: &Pubkey,
    winning_outcome: u8,
) {
    // Implementation
}

async fn claim_winnings(
    banks_client: &mut BanksClient,
    trader: &Keypair,
    market: &Pubkey,
) -> u64 {
    // Implementation
    0
}

fn create_program_test() -> ProgramTest {
    ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    )
}

// Additional helper stubs for compilation
async fn create_pmamm_market_with_outcomes(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    title: &str,
    outcomes: Vec<&str>,
) -> Pubkey {
    Keypair::new().pubkey()
}

async fn execute_pmamm_trade(
    banks_client: &mut BanksClient,
    trader: &Keypair,
    market: &Pubkey,
    amount: u64,
    outcome: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

async fn create_l2amm_market(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    title: &str,
    mean: f64,
    std_dev: f64,
) -> Pubkey {
    Keypair::new().pubkey()
}

async fn execute_l2amm_range_trade(
    banks_client: &mut BanksClient,
    trader: &Keypair,
    market: &Pubkey,
    amount: u64,
    lower: f64,
    upper: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[derive(Default)]
struct L2AMMMarketData {
    mean: f64,
    std_dev: f64,
    skewness: f64,
}

async fn get_l2amm_market_data(
    banks_client: &mut BanksClient,
    market: &Pubkey,
) -> L2AMMMarketData {
    L2AMMMarketData::default()
}

async fn calculate_probability_above(
    banks_client: &mut BanksClient,
    market: &Pubkey,
    value: f64,
) -> f64 {
    0.0
}

async fn calculate_probability_below(
    banks_client: &mut BanksClient,
    market: &Pubkey,
    value: f64,
) -> f64 {
    0.0
}

async fn calculate_probability_between(
    banks_client: &mut BanksClient,
    market: &Pubkey,
    lower: f64,
    upper: f64,
) -> f64 {
    0.0
}

async fn resolve_continuous_market(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    market: &Pubkey,
    value: f64,
) {
    // Implementation
}

async fn create_market_with_outcomes(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    title: &str,
    outcomes: u8,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    Ok(Keypair::new().pubkey())
}

async fn create_continuous_market(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    title: &str,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    Ok(Keypair::new().pubkey())
}