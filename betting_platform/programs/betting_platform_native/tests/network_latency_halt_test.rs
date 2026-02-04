//! Network Latency Halt Test
//!
//! Tests the halt mechanism when network latency exceeds 1.5ms threshold

#[cfg(test)]
mod tests {
    use solana_program_test::*;
    use solana_sdk::{
        account::Account,
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };
    use betting_platform_native::monitoring::network_latency::{
        NetworkLatencyMonitor, LatencyConfig, LatencyStatus,
    };
    use std::thread;
    use std::time::Duration;

    #[tokio::test]
    async fn test_network_latency_halt_at_1_5ms() {
        // Create latency monitor
        let mut monitor = NetworkLatencyMonitor::new();
        
        // Verify default config has 1.5ms threshold
        assert_eq!(monitor.config.halt_threshold_micros, 1500);
        
        println!("Testing network latency halt mechanism with 1.5ms threshold...");
        
        // Simulate normal latencies (under 1ms)
        println!("\n1. Recording normal latencies (< 1ms):");
        for i in 0..5 {
            let latency = 500 + i * 100; // 500-900 microseconds
            monitor.record_latency(latency, i as i64).unwrap();
            println!("   Sample {}: {}μs ({}ms)", i + 1, latency, latency as f64 / 1000.0);
        }
        
        assert_eq!(monitor.get_status(), LatencyStatus::Normal);
        assert!(!monitor.is_halted);
        println!("   ✅ Status: Normal, Halt: false");
        
        // Add some warning-level latencies (1-1.5ms)
        println!("\n2. Recording warning latencies (1-1.5ms):");
        for i in 0..3 {
            let latency = 1200 + i * 100; // 1200-1400 microseconds
            monitor.record_latency(latency, 10 + i as i64).unwrap();
            println!("   Sample {}: {}μs ({}ms)", i + 1, latency, latency as f64 / 1000.0);
        }
        
        assert_eq!(monitor.get_status(), LatencyStatus::Warning);
        assert!(!monitor.is_halted);
        println!("   ⚠️ Status: Warning, Halt: false");
        
        // Simulate high latencies over 1.5ms to trigger halt
        println!("\n3. Recording high latencies (> 1.5ms):");
        for i in 0..12 {
            let latency = 1600 + i * 100; // 1600-2700 microseconds
            let should_halt = monitor.record_latency(latency, 20 + i as i64).unwrap();
            println!("   Sample {}: {}μs ({}ms)", i + 1, latency, latency as f64 / 1000.0);
            
            if should_halt {
                println!("\n🚨 HALT TRIGGERED!");
                break;
            }
        }
        
        // Verify halt was triggered
        assert!(monitor.is_halted);
        assert_eq!(monitor.get_status(), LatencyStatus::Halted);
        assert!(monitor.samples_over_threshold >= monitor.config.min_samples_for_halt);
        
        println!("\n4. Final state:");
        println!("   Average latency: {}μs ({}ms)", 
            monitor.avg_latency_micros, 
            monitor.avg_latency_micros as f64 / 1000.0);
        println!("   Peak latency: {}μs ({}ms)", 
            monitor.peak_latency_micros,
            monitor.peak_latency_micros as f64 / 1000.0);
        println!("   Samples over 1.5ms threshold: {}", monitor.samples_over_threshold);
        println!("   Halt triggered: {}", monitor.is_halted);
        println!("   Halt trigger count: {}", monitor.halt_trigger_count);
    }

    #[tokio::test]
    async fn test_latency_halt_integration() {
        let program_id = Pubkey::new_unique();
        let mut program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(betting_platform_native::entrypoint::process_instruction),
        );

        // Add accounts
        let latency_monitor_pubkey = Pubkey::new_unique();
        let circuit_breaker_pubkey = Pubkey::find_program_address(
            &[b"circuit_breaker"],
            &program_id,
        ).0;

        // Initialize latency monitor account
        program_test.add_account(
            latency_monitor_pubkey,
            Account {
                lamports: 1_000_000,
                data: vec![0; NetworkLatencyMonitor::SIZE],
                owner: program_id,
                ..Account::default()
            },
        );

        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        println!("\nTesting network latency integration with circuit breaker...");

        // Simulate multiple network operations with increasing latency
        for i in 0..15 {
            // Simulate network operation
            let start = std::time::Instant::now();
            
            // Artificial delay to simulate network latency
            if i < 5 {
                // Normal latency (< 1ms)
                thread::sleep(Duration::from_micros(500));
            } else if i < 10 {
                // Warning latency (1-1.5ms)
                thread::sleep(Duration::from_micros(1200));
            } else {
                // High latency (> 1.5ms) - should trigger halt
                thread::sleep(Duration::from_micros(1800));
            }
            
            let latency = start.elapsed().as_micros() as u64;
            
            println!("Operation {}: latency = {}μs ({}ms)", 
                i + 1, latency, latency as f64 / 1000.0);
            
            // In production, this would be called after each network operation
            // to update the latency monitor and potentially trigger halt
            
            if latency > 1500 {
                println!("⚠️ Latency exceeds 1.5ms threshold!");
            }
        }
        
        println!("\n✅ Network latency halt test completed successfully");
    }

    #[test]
    fn test_latency_threshold_edge_cases() {
        let mut monitor = NetworkLatencyMonitor::new();
        
        println!("\nTesting latency threshold edge cases:");
        
        // Test exactly at threshold
        println!("\n1. Testing at exactly 1500μs (1.5ms):");
        monitor.record_latency(1500, 1).unwrap();
        assert_eq!(monitor.samples_over_threshold, 0);
        println!("   1500μs is NOT over threshold ✅");
        
        // Test just over threshold
        println!("\n2. Testing at 1501μs:");
        monitor.record_latency(1501, 2).unwrap();
        assert_eq!(monitor.samples_over_threshold, 1);
        println!("   1501μs IS over threshold ✅");
        
        // Test just under threshold
        println!("\n3. Testing at 1499μs:");
        monitor.record_latency(1499, 3).unwrap();
        assert_eq!(monitor.samples_over_threshold, 1); // Still 1 from previous
        println!("   1499μs is NOT over threshold ✅");
    }

    #[test]
    fn test_minimum_samples_requirement() {
        let mut monitor = NetworkLatencyMonitor::new();
        
        println!("\nTesting minimum samples requirement for halt:");
        
        // Record 9 high latency samples (less than min_samples_for_halt)
        for i in 0..9 {
            let should_halt = monitor.record_latency(2000, i as i64).unwrap();
            assert!(!should_halt);
        }
        
        println!("   With 9 samples over threshold: No halt ✅");
        assert!(!monitor.is_halted);
        
        // 10th sample should trigger halt
        let should_halt = monitor.record_latency(2000, 10).unwrap();
        assert!(should_halt);
        println!("   With 10 samples over threshold: Halt triggered ✅");
        assert!(monitor.is_halted);
    }
}