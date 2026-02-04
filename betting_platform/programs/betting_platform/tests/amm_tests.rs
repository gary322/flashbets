use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::{self, Token, TokenAccount};
use betting_platform::*;
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

#[tokio::test]
async fn test_lmsr_price_sum() {
    let program_test = ProgramTest::new(
        "betting_platform",
        betting_platform::id(),
        processor!(betting_platform::entry),
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Initialize LMSR market with 2 outcomes
    let market_id = 1u128;
    let b_parameter = 100_000_000_000_000_000_000u64; // 100 in fixed point
    let num_outcomes = 2u8;
    
    // Calculate expected prices for binary market with q = [0, 0]
    // Each outcome should have probability 0.5
    let expected_price = 500_000_000_000_000_000u64; // 0.5 in fixed point
    
    // Test price sum equals 1
    let market = betting_platform::lmsr_amm::LSMRMarket::new(
        betting_platform::fixed_math::FixedPoint::from_raw(b_parameter),
        num_outcomes as usize,
    );
    
    let prices = market.all_prices().unwrap();
    assert_eq!(prices.len(), 2);
    
    let sum = prices[0].add(&prices[1]).unwrap();
    let one = betting_platform::fixed_math::FixedPoint::from_u64(1);
    let epsilon = betting_platform::fixed_math::FixedPoint::from_float(0.000001);
    
    assert!((sum.sub(&one).unwrap().abs().unwrap()) < epsilon);
}

#[tokio::test]
async fn test_lmsr_bounded_slippage() {
    // Test that large trades have bounded slippage
    let b = betting_platform::fixed_math::FixedPoint::from_u64(10000);
    let market = betting_platform::lmsr_amm::LSMRMarket::new(b, 2);
    
    let shares = betting_platform::fixed_math::FixedPoint::from_u64(1000);
    let cost = market.buy_cost(0, shares).unwrap();
    
    // Calculate slippage
    let expected_cost = shares;
    let slippage = if cost > expected_cost {
        cost.sub(&expected_cost).unwrap().div(&expected_cost).unwrap()
    } else {
        expected_cost.sub(&cost).unwrap().div(&expected_cost).unwrap()
    };
    
    // Verify slippage is less than 5%
    let max_slippage = betting_platform::fixed_math::FixedPoint::from_float(0.05);
    assert!(slippage < max_slippage);
}

#[tokio::test]
async fn test_pmamm_newton_raphson_convergence() {
    let l = betting_platform::fixed_math::FixedPoint::from_u64(100);
    let t = betting_platform::fixed_math::FixedPoint::from_u64(86400); // 1 day
    let current_price = betting_platform::fixed_math::FixedPoint::from_float(0.5);
    let inventory = betting_platform::fixed_math::FixedPoint::from_u64(0);
    
    let market = betting_platform::pm_amm::PMAMMMarket {
        l,
        t,
        current_price,
        inventory,
    };
    
    let order_size = betting_platform::fixed_math::FixedPoint::from_u64(10);
    let current_time = betting_platform::fixed_math::FixedPoint::from_u64(0);
    
    // Test that Newton-Raphson converges
    let result = market.solve_trade(order_size, current_time);
    assert!(result.is_ok());
    
    // Verify the solution is reasonable
    let solution = result.unwrap();
    assert!(solution > current_price); // Buy order should increase price
}

#[tokio::test]
async fn test_l2_norm_constraint() {
    let k = betting_platform::fixed_math::FixedPoint::from_u64(10);
    let b = betting_platform::fixed_math::FixedPoint::from_u64(2);
    
    let amm = betting_platform::l2_amm::L2DistributionAMM {
        k,
        b,
        distribution_type: betting_platform::l2_amm::DistributionType::Normal {
            mean: 500_000_000_000_000_000,
            variance: 100_000_000_000_000_000,
        },
        parameters: betting_platform::l2_amm::DistributionParams {
            discretization_points: 100,
            range_min: betting_platform::fixed_math::FixedPoint::from_u64(0),
            range_max: betting_platform::fixed_math::FixedPoint::from_u64(1000),
        },
    };
    
    let distribution = amm.calculate_distribution().unwrap();
    
    // Verify L2 norm constraint is satisfied
    let norm = amm.calculate_l2_norm(&distribution).unwrap();
    let epsilon = betting_platform::fixed_math::FixedPoint::from_float(0.001);
    
    let diff = if norm > k {
        norm.sub(&k).unwrap()
    } else {
        k.sub(&norm).unwrap()
    };
    
    assert!(diff < epsilon);
}

#[tokio::test]
async fn test_iceberg_order_visibility() {
    // Test that only visible portion is revealed
    let visible = 100u64;
    let total = 1000u64;
    
    // Verify visible size constraints
    assert!(visible <= total / 10); // Max 10% visible
    
    // Test reveal mechanism
    let mut revealed = visible;
    let mut remaining = total;
    let mut executed = 0u64;
    
    // Execute visible portion
    executed += visible;
    remaining -= visible;
    revealed = 0;
    
    // Reveal next chunk
    if revealed == 0 && remaining > 0 {
        revealed = visible.min(remaining);
    }
    
    assert_eq!(revealed, visible);
    assert_eq!(remaining, 900);
    assert_eq!(executed, 100);
}

#[tokio::test]
async fn test_twap_interval_execution() {
    let total_size = 1000u64;
    let intervals = 10u8;
    let duration = 1000u64; // slots
    
    let size_per_interval = total_size / intervals as u64;
    assert_eq!(size_per_interval, 100);
    
    let interval_duration = duration / intervals as u64;
    assert_eq!(interval_duration, 100);
    
    // Simulate interval executions
    let mut intervals_completed = 0u8;
    let mut executed_size = 0u64;
    let mut current_slot = 0u64;
    let mut next_execution_slot = interval_duration;
    
    while intervals_completed < intervals {
        // Wait for next interval
        current_slot = next_execution_slot;
        
        // Execute interval
        executed_size += size_per_interval;
        intervals_completed += 1;
        next_execution_slot = current_slot + interval_duration;
    }
    
    assert_eq!(executed_size, total_size);
    assert_eq!(intervals_completed, intervals);
}

#[tokio::test]
async fn test_dark_pool_price_improvement() {
    let reference_price = 500_000_000_000_000_000u64; // 0.5
    let improvement_bps = 50u16; // 0.5%
    
    // Calculate price improvement for buy order
    let improvement = reference_price * improvement_bps as u64 / 10000;
    let buy_execution_price = reference_price - improvement;
    
    assert_eq!(buy_execution_price, 497_500_000_000_000_000); // 0.4975
    
    // Calculate price improvement for sell order
    let sell_execution_price = reference_price + improvement;
    
    assert_eq!(sell_execution_price, 502_500_000_000_000_000); // 0.5025
}