#[cfg(test)]
mod optimization_tests {
    use betting_platform::performance::*;
    use anchor_lang::prelude::*;
    use std::time::Instant;

    #[test]
    fn test_cu_optimization_leverage() {
        let mut optimizer = CUOptimizer::new();
        let params = LeverageParams {
            depth: 5,
            coverage: 150_000, // 1.5 in fixed point (assuming 100k scale)
            n: 4,
        };
        
        let result = optimizer.optimize_leverage_calculation(&params).unwrap();
        
        // Should hit tier cap of 300 for n=4
        assert_eq!(result, 150); // min(150, coverage_factor, 300)
    }

    #[test]
    fn test_precomputed_tables() {
        let tables = PrecomputedTables::new();
        
        // Test sqrt lookup
        assert_eq!(tables.get_sqrt(4), Some(2_000));
        assert_eq!(tables.get_sqrt(9), Some(3_000));
        
        // Test tier caps
        assert_eq!(tables.get_tier_cap(2), 100);
        assert_eq!(tables.get_tier_cap(4), 300);
        assert_eq!(tables.get_tier_cap(100), 1000); // Default for unknown
    }

    #[test]
    fn test_cache_manager() {
        let mut cache = CacheManager::new(10);
        
        let result = AMMResult {
            price: 1000,
            iterations: 3,
        };
        
        // Test set and get
        cache.set(12345, result.clone());
        assert_eq!(cache.get(&12345).unwrap().price, 1000);
        
        // Test cache miss
        assert!(cache.get(&99999).is_none());
    }

    #[test]
    fn test_newton_raphson_convergence() {
        let mut optimizer = CUOptimizer::new();
        
        let params = AMMParams {
            liquidity_parameter: 1000,
            outcome_quantity: 100,
            initial_guess: 500,
            distribution_mean: 0,
            distribution_variance: 1000,
        };
        
        let result = optimizer.optimize_pm_amm(&params).unwrap();
        
        // Should converge within 5 iterations
        assert!(result.iterations <= 5);
        assert!(result.price > 0);
    }

    #[test]
    fn test_batch_processor() {
        let processor = BatchProcessor::new(10, 100_000);
        
        assert!(processor.should_batch(15));
        assert!(!processor.should_batch(5));
        
        let cu = processor.calculate_batch_cu(20, 5_000);
        assert_eq!(cu, 100_000); // Capped at max
    }

    #[test]
    fn test_performance_profiler() {
        let mut profiler = PerformanceProfiler::new();
        
        let operation = || -> Result<u64> {
            // Simulate some work
            let mut sum = 0u64;
            for i in 0..100 {
                sum = sum.saturating_add(i);
            }
            Ok(sum)
        };
        
        let (result, metrics) = profiler.profile_transaction("test_operation", operation).unwrap();
        
        assert_eq!(result, 4950); // Sum of 0..100
        assert_eq!(metrics.operation, "test_operation");
        assert!(metrics.latency_ms >= 0.0);
    }

    #[test]
    fn test_bottleneck_detection() {
        let detector = BottleneckDetector::new();
        
        let cu_breakdown = vec![
            ("amm_calculation".to_string(), 18_000),
            ("state_update".to_string(), 5_000),
            ("validation".to_string(), 2_000),
        ];
        
        let bottlenecks = detector.detect_bottlenecks(&cu_breakdown, 25_000);
        
        assert!(!bottlenecks.is_empty());
        assert_eq!(bottlenecks[0].component, "amm_calculation");
        assert_eq!(bottlenecks[0].severity, BottleneckSeverity::High);
    }

    #[test]
    fn test_stress_test_framework() {
        let mut framework = StressTestFramework::new();
        
        // Test scenario creation
        let test_result = framework.test_concurrent_users(10).unwrap();
        
        assert_eq!(test_result.successful_operations, 10);
        assert_eq!(test_result.failed_operations, 0);
        assert!(test_result.peak_tps > 0.0);
    }

    #[test]
    fn test_load_generator() {
        let generator = LoadGenerator::new(
            1000.0,
            std::time::Duration::from_secs(60),
            100,
        );
        
        let pattern = generator.generate_load_pattern();
        
        assert_eq!(pattern.phases.len(), 4); // Rampup, Steady, Spike, Cooldown
        assert_eq!(pattern.concurrent_users, 100);
    }

    #[test]
    fn test_optimization_techniques() {
        let mut optimizer = OptimizationTechniques::new();
        
        let operations = vec![
            Operation {
                id: 1,
                operation_type: OperationType::Trade,
                data: vec![1, 2, 3],
            },
            Operation {
                id: 2,
                operation_type: OperationType::Trade,
                data: vec![4, 5, 6],
            },
            Operation {
                id: 3,
                operation_type: OperationType::PriceUpdate,
                data: vec![7, 8, 9],
            },
        ];
        
        let batched = optimizer.optimize_batch_operations(operations).unwrap();
        
        assert_eq!(batched.total_operations(), 3);
        assert!(batched.estimated_total_cu() > 0);
    }

    #[test]
    fn test_state_compression() {
        let mut optimizer = OptimizationTechniques::new();
        
        let state = MarketState {
            markets: vec![
                Market {
                    id: 1,
                    total_volume: 1000,
                    market_type: MarketType::Binary,
                },
                Market {
                    id: 2,
                    total_volume: 2000,
                    market_type: MarketType::Binary,
                },
            ],
            positions: vec![
                Position {
                    id: 1,
                    size: 100,
                    leverage: 10,
                },
                Position {
                    id: 2,
                    size: 200,
                    leverage: 20,
                },
            ],
        };
        
        let compressed = optimizer.optimize_state_compression(&state).unwrap();
        
        assert!(compressed.compression_ratio > 1.0);
        assert!(!compressed.markets.is_empty());
        assert!(!compressed.positions.is_empty());
    }

    #[test]
    fn test_memory_pool() {
        let mut pool = MemoryPool::new(10);
        
        // Allocate buffer
        let buffer1 = pool.allocate(1024);
        assert_eq!(buffer1.len(), 1024);
        
        // Return to pool
        pool.deallocate(buffer1);
        
        // Should reuse from pool
        let buffer2 = pool.allocate(1024);
        assert_eq!(buffer2.len(), 1024);
    }

    #[test]
    fn test_parallel_processor() {
        let processor = ParallelProcessor::new(10, 100_000);
        
        let operations: Vec<Operation> = (0..25).map(|i| Operation {
            id: i,
            operation_type: OperationType::Trade,
            data: vec![],
        }).collect();
        
        let chunks = processor.split_for_parallel_execution(operations);
        
        assert_eq!(chunks.len(), 3); // 25 / 10 = 3 chunks
        assert_eq!(chunks[0].len(), 10);
        assert_eq!(chunks[2].len(), 5); // Last chunk has remainder
    }

    #[cfg(test)]
    mod performance_benchmarks {
        use super::*;

        #[test]
        fn benchmark_leverage_calculation() {
            let optimizer = CUOptimizer::new();
            let params = LeverageParams {
                depth: 10,
                coverage: 200_000,
                n: 8,
            };
            
            let start = Instant::now();
            
            for _ in 0..10_000 {
                let _ = optimizer.optimize_leverage_calculation(&params);
            }
            
            let duration = start.elapsed();
            let avg_time = duration.as_micros() / 10_000;
            
            println!("Average leverage calculation time: {} μs", avg_time);
            assert!(avg_time < 100); // Should be < 100 microseconds
        }

        #[test]
        fn benchmark_amm_optimization() {
            let mut optimizer = CUOptimizer::new();
            
            let params = AMMParams {
                liquidity_parameter: 1000,
                outcome_quantity: 500,
                initial_guess: 750,
                distribution_mean: 0,
                distribution_variance: 1000,
            };
            
            let start = Instant::now();
            
            // First call will cache
            let _ = optimizer.optimize_pm_amm(&params);
            
            // Benchmark cached calls
            for _ in 0..10_000 {
                let _ = optimizer.optimize_pm_amm(&params);
            }
            
            let duration = start.elapsed();
            let avg_time = duration.as_micros() / 10_000;
            
            println!("Average cached AMM calculation time: {} μs", avg_time);
            assert!(avg_time < 10); // Cached calls should be very fast
        }

        #[test]
        fn test_stress_5k_tps() {
            let mut framework = StressTestFramework::new();
            
            // Simulate 5000 operations
            let operations = vec![Operation {
                id: 1,
                operation_type: OperationType::Trade,
                data: vec![],
            }; 5000];
            
            let start = Instant::now();
            
            let mut optimizer = OptimizationTechniques::new();
            let batched = optimizer.optimize_batch_operations(operations).unwrap();
            
            let duration = start.elapsed();
            let tps = 5000.0 / duration.as_secs_f64();
            
            println!("Achieved TPS: {}", tps);
            assert!(tps >= 5000.0, "System should handle 5k+ TPS");
            
            // Check CU usage
            let avg_cu = batched.estimated_total_cu() / 5000;
            println!("Average CU per operation: {}", avg_cu);
            assert!(avg_cu < 20_000, "Average CU should be <20k");
        }

        #[test]
        fn test_state_compression_ratio() {
            let mut optimizer = OptimizationTechniques::new();
            
            // Create large state with 21k markets
            let markets: Vec<Market> = (0..21_000).map(|i| Market {
                id: i,
                total_volume: (i * 1000) as u64,
                market_type: if i % 3 == 0 { MarketType::Binary } 
                            else if i % 3 == 1 { MarketType::Categorical }
                            else { MarketType::Scalar },
            }).collect();
            
            let positions: Vec<Position> = (0..10_000).map(|i| Position {
                id: i,
                size: (i * 100) as u64,
                leverage: ((i % 100) + 1) as u64,
            }).collect();
            
            let state = MarketState { markets, positions };
            
            let compressed = optimizer.optimize_state_compression(&state).unwrap();
            
            println!("Compression ratio: {}", compressed.compression_ratio);
            assert!(
                compressed.compression_ratio >= 10.0,
                "Should achieve 10x compression"
            );
        }
    }
}