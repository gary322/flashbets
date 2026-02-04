//! Parallel Operation Tests for Fused and Legacy Systems
//!
//! Tests that both leverage systems can run in parallel during migration

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{
        state::{FusedMigrationFlags, LeverageTier},
        constants::*,
    };
    use solana_program::{
        account_info::AccountInfo,
        pubkey::Pubkey,
        clock::Clock,
    };
    
    /// Test parallel operation with different percentages
    #[test]
    fn test_parallel_percentage_routing() {
        println!("Testing parallel percentage routing...");
        
        let mut flags = FusedMigrationFlags::new(Pubkey::default());
        flags.start_migration(1000, 10000);
        
        // Test with 10% fused
        flags.fused_percentage = 10;
        
        let mut fused_count = 0;
        let mut legacy_count = 0;
        
        // Simulate 100 orders
        for i in 0..100 {
            let seed = (i * 2 + 7) as u8; // Pseudo-random seed
            if flags.should_use_fused(seed) {
                fused_count += 1;
            } else {
                legacy_count += 1;
            }
        }
        
        println!("  With 10% setting: {} fused, {} legacy", fused_count, legacy_count);
        assert!(fused_count > 5 && fused_count < 20); // Allow some variance
        
        // Test with 50% fused
        flags.fused_percentage = 50;
        fused_count = 0;
        legacy_count = 0;
        
        for i in 0..100 {
            let seed = (i * 3 + 11) as u8;
            if flags.should_use_fused(seed) {
                fused_count += 1;
            } else {
                legacy_count += 1;
            }
        }
        
        println!("  With 50% setting: {} fused, {} legacy", fused_count, legacy_count);
        assert!(fused_count > 40 && fused_count < 60);
        
        // Test with 90% fused
        flags.fused_percentage = 90;
        fused_count = 0;
        legacy_count = 0;
        
        for i in 0..100 {
            let seed = (i * 5 + 13) as u8;
            if flags.should_use_fused(seed) {
                fused_count += 1;
            } else {
                legacy_count += 1;
            }
        }
        
        println!("  With 90% setting: {} fused, {} legacy", fused_count, legacy_count);
        assert!(fused_count > 80 && fused_count < 95);
        
        println!("  ✓ Percentage routing test passed");
    }
    
    /// Test fallback from fused to legacy
    #[test]
    fn test_fallback_mechanism() {
        println!("Testing fallback mechanism...");
        
        let mut flags = FusedMigrationFlags::new(Pubkey::default());
        flags.parallel_mode = true;
        flags.fused_enabled = true;
        flags.legacy_enabled = true;
        
        // Test fallback triggers
        assert_eq!(flags.fallback_count, 0);
        
        flags.trigger_fallback(2000);
        assert_eq!(flags.fallback_count, 1);
        assert_eq!(flags.last_fallback_slot, 2000);
        
        // Test emergency pause
        flags.emergency_pause();
        assert!(flags.fused_paused);
        assert!(flags.legacy_enabled);
        assert!(!flags.oracle_only);
        
        // Should not use fused when paused
        assert!(!flags.should_use_fused(128));
        
        // Resume and test
        flags.resume_fused();
        assert!(!flags.fused_paused);
        
        println!("  ✓ Fallback mechanism test passed");
    }
    
    /// Test leverage calculation in both systems
    #[test]
    fn test_parallel_leverage_calculation() {
        println!("Testing parallel leverage calculation...");
        
        // Setup
        let tiers = vec![
            LeverageTier { n: 1, max: 100 },
            LeverageTier { n: 2, max: 70 },
            LeverageTier { n: 4, max: 25 },
        ];
        
        // Test legacy calculation
        let legacy_leverage = fallback::FallbackHandler::calculate_legacy_leverage(
            1.2, // Good coverage
            &tiers,
            1, // 1 position
        ).unwrap();
        
        println!("  Legacy leverage: {}x", legacy_leverage);
        assert_eq!(legacy_leverage, 100);
        
        // Test fused calculation (simulated)
        let prob = 0.5;
        let sigma = 0.2;
        let risk = prob * (1.0 - prob);
        let unified_scalar = (1.0 / sigma) * CAP_FUSED;
        let premium_factor = (risk / BASE_RISK) * CAP_VAULT;
        let total_scalar = (unified_scalar * premium_factor).min(1000.0);
        let fused_leverage = (BASE_LEVERAGE as f64 * total_scalar / 100.0) as u16;
        
        println!("  Fused leverage: {}x (scalar: {})", fused_leverage, total_scalar);
        assert!(fused_leverage > legacy_leverage); // Fused should generally be higher
        
        println!("  ✓ Parallel leverage calculation test passed");
    }
    
    /// Test migration progression
    #[test]
    fn test_migration_progression() {
        println!("Testing migration progression...");
        
        let mut flags = FusedMigrationFlags::new(Pubkey::default());
        
        // Start migration
        flags.start_migration(1000, 10000);
        assert!(flags.parallel_mode);
        assert!(flags.fused_enabled);
        assert!(flags.legacy_enabled);
        assert_eq!(flags.fused_percentage, 10);
        
        // Increase percentage gradually
        for _ in 0..5 {
            flags.increase_fused_percentage(10);
        }
        assert_eq!(flags.fused_percentage, 60);
        
        // Complete migration
        flags.complete_migration();
        assert!(!flags.parallel_mode);
        assert!(flags.oracle_only);
        assert!(!flags.legacy_enabled);
        assert_eq!(flags.fused_percentage, 100);
        
        // Should always use fused after completion
        for seed in 0..=255 {
            assert!(flags.should_use_fused(seed));
        }
        
        println!("  ✓ Migration progression test passed");
    }
    
    /// Test concurrent operation statistics
    #[test]
    fn test_concurrent_statistics() {
        println!("Testing concurrent operation statistics...");
        
        use crate::state::MigrationStats;
        
        let mut stats = MigrationStats::new();
        
        // Record some fused orders
        for i in 1..=10 {
            stats.record_fused_order(100.0 * i as f64);
        }
        
        // Record some legacy orders
        for i in 1..=10 {
            stats.record_legacy_order(50.0 * i as f64);
        }
        
        println!("  Fused orders: {}", stats.fused_orders);
        println!("  Legacy orders: {}", stats.legacy_orders);
        println!("  Avg fused leverage: {}", stats.avg_fused_leverage);
        println!("  Avg legacy leverage: {}", stats.avg_legacy_leverage);
        
        assert_eq!(stats.fused_orders, 10);
        assert_eq!(stats.legacy_orders, 10);
        assert!(stats.avg_fused_leverage > 500.0); // Average of 100-1000
        assert!(stats.avg_legacy_leverage > 250.0); // Average of 50-500
        
        // Record errors
        stats.record_fused_error();
        stats.record_legacy_error();
        
        assert_eq!(stats.fused_errors, 1);
        assert_eq!(stats.legacy_errors, 1);
        
        println!("  ✓ Concurrent statistics test passed");
    }
    
    /// Test edge cases in parallel operation
    #[test]
    fn test_parallel_edge_cases() {
        println!("Testing parallel operation edge cases...");
        
        let mut flags = FusedMigrationFlags::new(Pubkey::default());
        
        // Test with both systems disabled (should fail validation)
        flags.fused_enabled = false;
        flags.legacy_enabled = false;
        assert!(flags.validate().is_err());
        
        // Test oracle-only with legacy enabled (should fail)
        flags.oracle_only = true;
        flags.legacy_enabled = true;
        flags.fused_enabled = true;
        assert!(flags.validate().is_err());
        
        // Test valid oracle-only
        flags.legacy_enabled = false;
        assert!(flags.validate().is_ok());
        
        // Test emergency pause during oracle-only
        flags.emergency_pause();
        assert!(flags.fused_paused);
        assert!(flags.legacy_enabled); // Should re-enable legacy
        assert!(!flags.oracle_only); // Should disable oracle-only
        
        println!("  ✓ Edge cases test passed");
    }
    
    /// Integration test for full parallel operation
    #[test]
    fn test_full_parallel_integration() {
        println!("Running full parallel operation integration test...");
        
        let mut flags = FusedMigrationFlags::new(Pubkey::default());
        let mut stats = crate::state::MigrationStats::new();
        
        // Simulate a full migration cycle
        println!("  Phase 1: Legacy only");
        assert!(!flags.should_use_fused(128));
        
        println!("  Phase 2: Start migration (10% fused)");
        flags.start_migration(1000, 100000);
        
        // Simulate 1000 orders during migration
        let mut phase2_fused = 0;
        let mut phase2_legacy = 0;
        
        for i in 0..1000 {
            let seed = ((i * 7 + 3) % 256) as u8;
            if flags.should_use_fused(seed) {
                phase2_fused += 1;
                stats.record_fused_order(150.0);
            } else {
                phase2_legacy += 1;
                stats.record_legacy_order(75.0);
            }
        }
        
        println!("    Processed {} fused, {} legacy", phase2_fused, phase2_legacy);
        assert!(phase2_fused > 50 && phase2_fused < 150); // ~10%
        
        println!("  Phase 3: Increase to 50% fused");
        flags.fused_percentage = 50;
        
        let mut phase3_fused = 0;
        let mut phase3_legacy = 0;
        
        for i in 0..1000 {
            let seed = ((i * 11 + 7) % 256) as u8;
            if flags.should_use_fused(seed) {
                phase3_fused += 1;
                stats.record_fused_order(200.0);
            } else {
                phase3_legacy += 1;
                stats.record_legacy_order(80.0);
            }
        }
        
        println!("    Processed {} fused, {} legacy", phase3_fused, phase3_legacy);
        assert!(phase3_fused > 450 && phase3_fused < 550); // ~50%
        
        println!("  Phase 4: Complete migration");
        flags.complete_migration();
        
        let mut phase4_fused = 0;
        
        for i in 0..100 {
            let seed = ((i * 13 + 11) % 256) as u8;
            assert!(flags.should_use_fused(seed));
            phase4_fused += 1;
        }
        
        println!("    Processed {} fused (100%)", phase4_fused);
        assert_eq!(phase4_fused, 100);
        
        println!("  Final statistics:");
        println!("    Total fused orders: {}", stats.fused_orders);
        println!("    Total legacy orders: {}", stats.legacy_orders);
        println!("    Avg fused leverage: {:.2}x", stats.avg_fused_leverage);
        println!("    Avg legacy leverage: {:.2}x", stats.avg_legacy_leverage);
        
        println!("  ✓ Full integration test passed");
    }
}

/// Run all parallel operation tests
pub fn run_parallel_tests() {
    println!("\n=== Running Parallel Operation Tests ===\n");
    
    tests::test_parallel_percentage_routing();
    tests::test_fallback_mechanism();
    tests::test_parallel_leverage_calculation();
    tests::test_migration_progression();
    tests::test_concurrent_statistics();
    tests::test_parallel_edge_cases();
    tests::test_full_parallel_integration();
    
    println!("\n=== All Parallel Operation Tests Passed ===\n");
}