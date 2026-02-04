#[cfg(test)]
mod shard_tests {
    use anchor_lang::prelude::*;
    use crate::sharding::*;

    #[test]
    fn test_deterministic_shard_assignment() {
        let manager = ShardManager::new();
        let market_id = Pubkey::new_unique();

        // Test deterministic assignment
        let shard1 = manager.assign_shard(&market_id);
        let shard2 = manager.assign_shard(&market_id);
        assert_eq!(shard1, shard2, "Shard assignment should be deterministic");

        // Test shard range
        assert!(shard1 < SHARD_COUNT_DEFAULT, "Shard ID should be within valid range");
    }

    #[test]
    fn test_shard_distribution() {
        let manager = ShardManager::new();
        let mut shard_counts = vec![0u32; SHARD_COUNT_DEFAULT as usize];
        
        // Test distribution across shards
        for _ in 0..10000 {
            let market = Pubkey::new_unique();
            let shard = manager.assign_shard(&market);
            shard_counts[shard as usize] += 1;
        }

        // Check relatively even distribution (within 20% of expected)
        let expected = 10000 / SHARD_COUNT_DEFAULT as u32;
        for count in shard_counts {
            let deviation = ((count as f64 - expected as f64) / expected as f64).abs();
            assert!(deviation < 0.2, "Shards should be evenly distributed. Count: {}, Expected: {}", count, expected);
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

        // Check metrics
        let metrics = manager.contention_metrics.get(&0).unwrap();
        assert!(metrics.avg_write_time_ms > MAX_CONTENTION_MS, "Should detect high contention");
        assert_eq!(metrics.transaction_count, 100);
        assert!(metrics.hot_markets.len() > 0, "Should track hot markets");

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
        
        // Register keepers with stakes
        let keeper1 = Pubkey::new_unique();
        let keeper2 = Pubkey::new_unique();
        let keeper3 = Pubkey::new_unique();
        
        voter.register_keeper(keeper1, 100);
        voter.register_keeper(keeper2, 100);
        voter.register_keeper(keeper3, 100);

        // Create proposal
        let proposal = RebalanceProposal {
            id: [1u8; 32],
            overloaded_shards: vec![(0, ContentionMetrics::default())],
            underloaded_shards: vec![(1, ContentionMetrics::default())],
            markets_to_move: vec![(Pubkey::new_unique(), 0, 1)],
            estimated_improvement: 0.5,
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 0,
        };

        voter.submit_proposal(proposal, 100).unwrap();

        // Vote
        voter.vote(&keeper1, &[1u8; 32], true).unwrap();
        voter.vote(&keeper2, &[1u8; 32], true).unwrap();
        voter.vote(&keeper3, &[1u8; 32], false).unwrap();

        // Check votes
        let status = voter.get_proposal_status(&[1u8; 32]).unwrap();
        assert_eq!(status.votes_for, 200);
        assert_eq!(status.votes_against, 100);
        assert!(status.approval_ratio > VOTE_THRESHOLD);

        // Execute approved proposals
        let executions = voter.execute_approved_proposals(200);
        assert_eq!(executions.len(), 1);
    }

    #[test]
    fn test_shard_migration() {
        let mut migrator = ShardMigrator::new();
        let market_id = Pubkey::new_unique();

        // Start migration
        migrator.migrate_market(market_id, 0, 1, 100).unwrap();

        // Check paused
        assert!(migrator.is_market_paused(&market_id));

        // Check progress
        let progress = migrator.get_migration_progress(&market_id).unwrap();
        assert_eq!(progress.status, MigrationStatus::Pending);
        assert_eq!(progress.from_shard, 0);
        assert_eq!(progress.to_shard, 1);

        // Complete migration
        migrator.complete_migration(&market_id).unwrap();

        // Check no longer paused
        assert!(!migrator.is_market_paused(&market_id));
    }

    #[test]
    fn test_migration_timeout() {
        let mut migrator = ShardMigrator::new();
        let market_id = Pubkey::new_unique();

        // Start migration
        migrator.migrate_market(market_id, 0, 1, 100).unwrap();

        // Check timeout
        let timed_out = migrator.check_migration_timeouts(100 + MIGRATION_TIMEOUT_SLOTS + 1);
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], market_id);

        // Should no longer be paused
        assert!(!migrator.is_market_paused(&market_id));
    }

    #[test]
    fn test_rebalance_similarity_check() {
        let mut voter = RebalanceVoter::new();
        let market1 = Pubkey::new_unique();
        let market2 = Pubkey::new_unique();

        // First proposal
        let proposal1 = RebalanceProposal {
            id: [1u8; 32],
            overloaded_shards: vec![(0, ContentionMetrics::default())],
            underloaded_shards: vec![(1, ContentionMetrics::default())],
            markets_to_move: vec![(market1, 0, 1), (market2, 0, 1)],
            estimated_improvement: 0.5,
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 0,
        };

        voter.submit_proposal(proposal1, 100).unwrap();

        // Similar proposal (same markets)
        let proposal2 = RebalanceProposal {
            id: [2u8; 32],
            overloaded_shards: vec![(0, ContentionMetrics::default())],
            underloaded_shards: vec![(1, ContentionMetrics::default())],
            markets_to_move: vec![(market1, 0, 2), (market2, 0, 2)], // Different target
            estimated_improvement: 0.5,
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 0,
        };

        // Should not add similar proposal
        let result = voter.submit_proposal(proposal2, 100);
        assert!(result.is_ok()); // Silently ignores similar proposals
        assert_eq!(voter.active_proposals.len(), 1); // Only one proposal
    }
}