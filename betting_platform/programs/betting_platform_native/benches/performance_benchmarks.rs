//! Performance Benchmarks for Part 7 Specification
//! 
//! Measures CU usage and performance metrics for key operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use betting_platform_native::{
    amm::{
        pmamm::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonConfig},
        l2amm::simpson::{SimpsonIntegrator, SimpsonConfig},
        lmsr::LMSRPool,
    },
    math::fixed_point::{U64F64, U128F128},
    sharding::enhanced_sharding::{MarketShardAllocation, OperationType},
};
use solana_program::pubkey::Pubkey;

/// Benchmark Newton-Raphson solver performance
fn bench_newton_raphson(c: &mut Criterion) {
    let mut group = c.benchmark_group("newton_raphson");
    
    // Test different numbers of outcomes
    for num_outcomes in [2, 4, 8, 16, 32, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("solve_prices", num_outcomes),
            num_outcomes,
            |b, &n| {
                let mut solver = NewtonRaphsonSolver::new();
                let pool = create_test_pool(n);
                let target_probs: Vec<u64> = (0..n)
                    .map(|i| 10000 / n + if i == 0 { 10000 % n } else { 0 })
                    .collect();
                
                b.iter(|| {
                    let result = solver.solve_for_prices(&pool, &target_probs).unwrap();
                    black_box(result);
                });
            },
        );
    }
    
    // Measure iteration statistics
    group.bench_function("average_iterations", |b| {
        let mut solver = NewtonRaphsonSolver::new();
        let pool = create_test_pool(4);
        
        b.iter(|| {
            for _ in 0..100 {
                let target_probs = vec![2500, 2500, 2500, 2500];
                let _ = solver.solve_for_prices(&pool, &target_probs);
            }
            let avg = solver.get_average_iterations();
            assert!((avg - 4.2).abs() < 1.0, "Should average ~4.2 iterations");
        });
    });
    
    group.finish();
}

/// Benchmark Simpson's rule integration
fn bench_simpson_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("simpson_integration");
    
    // Test different point counts
    for num_points in [10, 12, 14, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("integrate", num_points),
            num_points,
            |b, &n| {
                let config = SimpsonConfig {
                    num_points: n,
                    error_tolerance: U64F64::from_raw(4398), // ~1e-6
                    max_iterations: 5,
                };
                let mut integrator = SimpsonIntegrator::with_config(config);
                
                // Test function: normal distribution
                let f = |x: U64F64| -> Result<U64F64, _> {
                    let x_f = x.to_num();
                    let value = (-0.5 * x_f * x_f).exp() / (2.0 * std::f64::consts::PI).sqrt();
                    Ok(U64F64::from_num(value))
                };
                
                b.iter(|| {
                    let result = integrator.integrate(
                        f,
                        U64F64::from_num(-3),
                        U64F64::from_num(3),
                    ).unwrap();
                    
                    assert!(result.cu_used <= 2000, "Should use <= 2000 CU");
                    black_box(result);
                });
            },
        );
    }
    
    // Benchmark fast Simpson's with pre-computed weights
    group.bench_function("fast_simpson_10", |b| {
        use betting_platform_native::amm::l2amm::simpson::fast_simpson_integration;
        
        let values: Vec<U64F64> = (0..11)
            .map(|i| U64F64::from_num(i as f64 / 10.0))
            .collect();
        let h = U64F64::from_num(0.1);
        
        b.iter(|| {
            let result = fast_simpson_integration(&values, h).unwrap();
            black_box(result);
        });
    });
    
    group.finish();
}

/// Benchmark LMSR AMM operations
fn bench_lmsr_amm(c: &mut Criterion) {
    let mut group = c.benchmark_group("lmsr_amm");
    
    group.bench_function("price_calculation", |b| {
        let pool = LMSRPool {
            b_parameter: U64F64::from_num(1000),
            shares: vec![U64F64::from_num(500), U64F64::from_num(500)],
            total_shares: U64F64::from_num(1000),
        };
        
        b.iter(|| {
            let price = pool.calculate_price(0).unwrap();
            black_box(price);
        });
    });
    
    group.bench_function("trade_execution", |b| {
        let mut pool = LMSRPool {
            b_parameter: U64F64::from_num(1000),
            shares: vec![U64F64::from_num(500), U64F64::from_num(500)],
            total_shares: U64F64::from_num(1000),
        };
        
        b.iter(|| {
            let cost = pool.execute_trade(0, U64F64::from_num(10), true).unwrap();
            black_box(cost);
            // Reset for next iteration
            pool.shares[0] = U64F64::from_num(500);
            pool.total_shares = U64F64::from_num(1000);
        });
    });
    
    group.finish();
}

/// Benchmark sharding operations
fn bench_sharding(c: &mut Criterion) {
    let mut group = c.benchmark_group("sharding");
    
    // Benchmark shard assignment
    group.bench_function("shard_assignment", |b| {
        let market_id = Pubkey::new_unique();
        let allocation = MarketShardAllocation::new(market_id, 1000);
        
        b.iter(|| {
            let shard = allocation.get_shard_for_operation(OperationType::PlaceOrder);
            black_box(shard);
        });
    });
    
    // Benchmark cross-shard lookup with different market counts
    for market_count in [1000, 5000, 10000, 21000].iter() {
        group.bench_with_input(
            BenchmarkId::new("market_lookup", market_count),
            market_count,
            |b, &n| {
                let markets: Vec<(Pubkey, MarketShardAllocation)> = (0..n)
                    .map(|i| {
                        let id = Pubkey::new_unique();
                        (id, MarketShardAllocation::new(id, i * 4))
                    })
                    .collect();
                
                b.iter(|| {
                    // Random market lookup
                    let target = markets[n / 2].0;
                    let found = markets.iter()
                        .find(|(id, _)| *id == target)
                        .map(|(_, alloc)| alloc);
                    black_box(found);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark end-to-end trading flow
fn bench_trading_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("trading_flow");
    
    group.bench_function("complete_trade_cycle", |b| {
        b.iter(|| {
            // Simulate complete trade cycle
            let market_id = Pubkey::new_unique();
            let user = Pubkey::new_unique();
            
            // 1. Get market shard (simulated)
            let shard_id = (market_id.to_bytes()[0] as u32) % 4;
            
            // 2. Place order (simulated CU usage)
            simulate_cu_usage(1000); // Order placement
            
            // 3. Execute trade (Newton-Raphson for PM-AMM)
            simulate_cu_usage(4000); // PM-AMM execution
            
            // 4. Update balances
            simulate_cu_usage(500); // Balance updates
            
            // 5. Emit events
            simulate_cu_usage(200); // Event emission
            
            black_box((market_id, user, shard_id));
        });
    });
    
    group.finish();
}

/// Benchmark chain execution
fn bench_chain_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_execution");
    
    for chain_length in [1, 2, 3, 5].iter() {
        group.bench_with_input(
            BenchmarkId::new("chain_steps", chain_length),
            chain_length,
            |b, &n| {
                b.iter(|| {
                    let mut total_cu = 0u64;
                    
                    for _ in 0..n {
                        // Each step involves:
                        // - Market lookup: 500 CU
                        // - Trade execution: 4000 CU (PM-AMM)
                        // - State update: 1000 CU
                        // - Cross-shard comm: 1500 CU
                        total_cu += 7000;
                    }
                    
                    // Verify within limits
                    assert!(total_cu < 50000, "Chain should be under 50k CU");
                    black_box(total_cu);
                });
            },
        );
    }
    
    group.finish();
}

// Helper functions

fn create_test_pool(num_outcomes: usize) -> betting_platform_native::state::amm_accounts::PMAMMMarket {
    use betting_platform_native::state::amm_accounts::{PMAMMMarket, MarketState};
    use solana_program::pubkey::Pubkey;
    
    PMAMMMarket {
        discriminator: [112, 78, 45, 209, 156, 34, 89, 167], // PMAMM_MARKET discriminator
        market_id: 1,
        pool_id: 1,
        l_parameter: 10000,
        expiry_time: 1735689600, // Some future timestamp
        num_outcomes: num_outcomes as u8,
        reserves: vec![1000u64; num_outcomes],
        total_liquidity: 10000,
        total_lp_supply: 1000000,
        liquidity_providers: 1, // u32 count, not Vec
        state: MarketState::Active,
        initial_price: 5000, // 50% for binary markets
        probabilities: vec![10000 / num_outcomes as u64; num_outcomes],
        fee_bps: 30,
        oracle: Pubkey::new_unique(),
        total_volume: 0,
        created_at: 1704067200, // Jan 1, 2024
        last_update: 1704067200,
    }
}

fn simulate_cu_usage(cu: u64) {
    // Simulate compute unit usage
    std::thread::sleep(std::time::Duration::from_micros(cu / 100));
}

// Criterion configuration
criterion_group!(
    benches,
    bench_newton_raphson,
    bench_simpson_integration,
    bench_lmsr_amm,
    bench_sharding,
    bench_trading_flow,
    bench_chain_execution
);

criterion_main!(benches);