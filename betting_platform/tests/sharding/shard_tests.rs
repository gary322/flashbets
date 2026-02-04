#[cfg(test)]
mod shard_tests {
    use super::*;
    use betting_platform::sharding::*;
    use anchor_lang::prelude::*;

    #[test]
    fn test_deterministic_shard_assignment() {
        let manager = ShardManager::new();
        let market_id = Pubkey::new_unique();

        // Test deterministic assignment
        let shard1 = manager.assign_shard(&market_id);
        let shard2 = manager.assign_shard(&market_id);
        assert_eq!(shard1, shard2, "Shard assignment should be deterministic");

        // Test distribution across shards
        let mut shard_counts = vec![0u32; SHARD_COUNT_DEFAULT as usize];
        for _ in 0..10000 {
            let market = Pubkey::new_unique();
            let shard = manager.assign_shard(&market);
            shard_counts[shard as usize] += 1;
        }

        // Check relatively even distribution
        for count in shard_counts {
            assert!((count as f64 - 2500.0).abs() < 500.0, "Shards should be evenly distributed");
        }
    }

    #[test]
    fn test_contention_detection() {
        let mut manager = ShardManager::new();

        // Simulate high contention on shard 0
        for i in 0..100 {
            manager.measure_contention(
                0,
                2.0, // > 1.5ms threshold
                Pubkey::new_unique(),
            );
        }

        // Check rebalance needed
        let proposal = manager.check_rebalance_needed();
        assert!(proposal.is_some(), "Should detect rebalance needed");

        let proposal = proposal.unwrap();
        assert!(!proposal.overloaded_shards.is_empty());
        assert!(proposal.estimated_improvement > 0.0);
    }

    #[test]
    fn test_rebalance_voting() {
        let mut voter = RebalanceVoter::new();
        let keeper1 = Pubkey::new_unique();
        let keeper2 = Pubkey::new_unique();
        let keeper3 = Pubkey::new_unique();

        // Add keepers with stakes
        voter.add_keeper_stake(keeper1, 1000);
        voter.add_keeper_stake(keeper2, 2000);
        voter.add_keeper_stake(keeper3, 1500);

        // Create a proposal
        let proposal = RebalanceProposal {
            id: [1u8; 32],
            overloaded_shards: vec![(0, ContentionMetrics::default())],
            underloaded_shards: vec![(1, ContentionMetrics::default())],
            markets_to_move: vec![(Pubkey::new_unique(), 0, 1)],
            estimated_improvement: 0.5,
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 1000,
        };

        voter.submit_proposal(proposal, 0).unwrap();

        // Vote on proposal
        voter.vote(&keeper1, &[1u8; 32], true).unwrap();
        voter.vote(&keeper2, &[1u8; 32], true).unwrap();
        voter.vote(&keeper3, &[1u8; 32], false).unwrap();

        // Execute proposals (should pass with 3000 for vs 1500 against)
        let executions = voter.execute_approved_proposals(1001);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].proposal_id, [1u8; 32]);
    }

    #[test]
    fn test_shard_migration() {
        let mut migrator = ShardMigrator::new();
        let market_id = Pubkey::new_unique();

        // Start migration
        migrator.migrate_market(market_id, 0, 1, 100).unwrap();

        // Check market is paused
        assert!(migrator.is_market_paused(&market_id));

        // Complete migration
        migrator.complete_migration(&market_id).unwrap();

        // Check market is no longer paused
        assert!(!migrator.is_market_paused(&market_id));
    }

    #[test]
    fn test_migration_timeout() {
        let mut migrator = ShardMigrator::new();
        let market_id = Pubkey::new_unique();

        // Start migration
        migrator.migrate_market(market_id, 0, 1, 100).unwrap();

        // Check timeout after migration period
        let timed_out = migrator.check_migration_timeouts(100 + MIGRATION_TIMEOUT_SLOTS + 1);
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], market_id);
    }

    #[test]
    fn test_find_best_target_shard() {
        let mut manager = ShardManager::new();

        // Set up shard contention metrics
        manager.measure_contention(0, 2.0, Pubkey::new_unique()); // Overloaded
        manager.measure_contention(1, 0.5, Pubkey::new_unique()); // Low load
        manager.measure_contention(2, 1.0, Pubkey::new_unique()); // Medium load
        manager.measure_contention(3, 1.8, Pubkey::new_unique()); // Near threshold

        // Find best target (should be shard 1)
        let best = manager.find_best_target_shard(0);
        assert_eq!(best, Some(1));
    }

    #[test]
    fn test_contention_metrics_rolling_average() {
        let mut manager = ShardManager::new();

        // Add multiple measurements
        manager.measure_contention(0, 1.0, Pubkey::new_unique());
        manager.measure_contention(0, 2.0, Pubkey::new_unique());
        manager.measure_contention(0, 3.0, Pubkey::new_unique());

        let metrics = &manager.contention_metrics[&0];
        assert_eq!(metrics.avg_write_time_ms, 2.0); // (1+2+3)/3
        assert_eq!(metrics.peak_write_time_ms, 3.0);
        assert_eq!(metrics.transaction_count, 3);
    }

    #[test]
    fn test_hot_market_tracking() {
        let mut manager = ShardManager::new();
        let hot_market = Pubkey::new_unique();

        // Add measurements for hot market
        manager.measure_contention(0, 2.0, hot_market); // Above threshold
        manager.measure_contention(0, 2.5, hot_market); // Above threshold again

        let metrics = &manager.contention_metrics[&0];
        assert_eq!(metrics.hot_markets.len(), 1);
        assert_eq!(metrics.hot_markets[0], hot_market);
    }

    #[test]
    fn test_insufficient_improvement_rejection() {
        let mut voter = RebalanceVoter::new();

        let proposal = RebalanceProposal {
            id: [0u8; 32],
            overloaded_shards: vec![],
            underloaded_shards: vec![],
            markets_to_move: vec![(Pubkey::new_unique(), 0, 1)],
            estimated_improvement: 0.05, // Below 0.1 threshold
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 0,
        };

        let result = voter.submit_proposal(proposal, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_unauthorized_keeper_vote() {
        let mut voter = RebalanceVoter::new();
        let unauthorized = Pubkey::new_unique();

        // Try to vote without being registered
        let result = voter.vote(&unauthorized, &[0u8; 32], true);
        assert!(result.is_err());
    }

    #[test]
    fn test_migration_cancel() {
        let mut migrator = ShardMigrator::new();
        let market_id = Pubkey::new_unique();

        // Start migration
        migrator.migrate_market(market_id, 0, 1, 100).unwrap();

        // Cancel migration (should succeed for pending status)
        migrator.cancel_migration(&market_id).unwrap();

        // Market should no longer be paused
        assert!(!migrator.is_market_paused(&market_id));
    }
}