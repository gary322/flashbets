use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use std::collections::HashMap;

use crate::sharding::{
    ContentionMetrics, RebalanceProposal, SHARD_COUNT_DEFAULT, 
    MAX_CONTENTION_MS, REBALANCE_INTERVAL
};

pub struct ShardManager {
    pub shard_assignments: HashMap<Pubkey, u8>,
    pub contention_metrics: HashMap<u8, ContentionMetrics>,
    pub rebalance_votes: HashMap<[u8; 32], u32>,
    pub last_rebalance_slot: u64,
}

impl ShardManager {
    pub fn new() -> Self {
        Self {
            shard_assignments: HashMap::new(),
            contention_metrics: HashMap::new(),
            rebalance_votes: HashMap::new(),
            last_rebalance_slot: 0,
        }
    }

    /// Deterministic shard assignment using keccak hash
    pub fn assign_shard(&self, market_id: &Pubkey) -> u8 {
        // Deterministic hash-based assignment
        let hash = keccak::hash(&market_id.to_bytes());
        hash.0[0] % SHARD_COUNT_DEFAULT
    }

    /// Assign shard with rebalance check
    pub fn assign_shard_with_rebalance(
        &self,
        market_id: &Pubkey,
        _current_slot: u64,
    ) -> u8 {
        // Check if market has been reassigned
        if let Some(&reassigned_shard) = self.shard_assignments.get(market_id) {
            return reassigned_shard;
        }

        // Default deterministic assignment
        self.assign_shard(market_id)
    }

    /// Measure contention for a shard
    pub fn measure_contention(
        &mut self,
        shard_id: u8,
        write_time_ms: f64,
        market_id: Pubkey,
    ) {
        let metrics = self.contention_metrics.entry(shard_id)
            .or_insert_with(ContentionMetrics::default);

        // Update rolling average
        metrics.transaction_count += 1;
        metrics.avg_write_time_ms =
            (metrics.avg_write_time_ms * (metrics.transaction_count - 1) as f64
             + write_time_ms) / metrics.transaction_count as f64;

        // Track peak
        if write_time_ms > metrics.peak_write_time_ms {
            metrics.peak_write_time_ms = write_time_ms;
        }

        // Track hot markets
        if write_time_ms > MAX_CONTENTION_MS {
            if !metrics.hot_markets.contains(&market_id) {
                metrics.hot_markets.push(market_id);
            }
        }
    }

    /// Check if rebalancing is needed
    pub fn check_rebalance_needed(&self) -> Option<RebalanceProposal> {
        let mut overloaded_shards = vec![];
        let mut underloaded_shards = vec![];

        for (shard_id, metrics) in &self.contention_metrics {
            if metrics.avg_write_time_ms > MAX_CONTENTION_MS {
                overloaded_shards.push((*shard_id, metrics.clone()));
            } else if metrics.avg_write_time_ms < MAX_CONTENTION_MS * 0.5 {
                underloaded_shards.push((*shard_id, metrics.clone()));
            }
        }

        if overloaded_shards.is_empty() {
            return None;
        }

        // Create rebalance proposal
        Some(RebalanceProposal {
            id: self.generate_proposal_id(),
            overloaded_shards: overloaded_shards.clone(),
            underloaded_shards,
            markets_to_move: self.select_markets_to_move(&overloaded_shards),
            estimated_improvement: self.estimate_improvement(&overloaded_shards),
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 0, // Set by voter
        })
    }

    /// Select markets to move from overloaded shards
    fn select_markets_to_move(
        &self,
        overloaded: &[(u8, ContentionMetrics)],
    ) -> Vec<(Pubkey, u8, u8)> {
        let mut moves = vec![];

        for (shard_id, metrics) in overloaded {
            // Move hottest markets from overloaded shards
            for market in &metrics.hot_markets {
                if let Some(target_shard) = self.find_best_target_shard(*shard_id) {
                    moves.push((*market, *shard_id, target_shard));
                }
            }
        }

        moves
    }

    /// Find best target shard for migration
    fn find_best_target_shard(&self, exclude_shard: u8) -> Option<u8> {
        let mut best_shard = None;
        let mut lowest_contention = f64::MAX;

        for shard_id in 0..SHARD_COUNT_DEFAULT {
            if shard_id == exclude_shard {
                continue;
            }

            if let Some(metrics) = self.contention_metrics.get(&shard_id) {
                if metrics.avg_write_time_ms < lowest_contention {
                    lowest_contention = metrics.avg_write_time_ms;
                    best_shard = Some(shard_id);
                }
            } else {
                // Empty shard, perfect target
                return Some(shard_id);
            }
        }

        best_shard
    }

    /// Estimate improvement from rebalancing
    fn estimate_improvement(&self, overloaded: &[(u8, ContentionMetrics)]) -> f64 {
        let total_excess: f64 = overloaded.iter()
            .map(|(_, metrics)| (metrics.avg_write_time_ms - MAX_CONTENTION_MS).max(0.0))
            .sum();

        // Estimate 50-80% improvement based on distribution
        total_excess * 0.65
    }

    /// Generate unique proposal ID
    fn generate_proposal_id(&self) -> [u8; 32] {
        let mut id = [0u8; 32];
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        id[..8].copy_from_slice(&timestamp.to_le_bytes());
        id[8..16].copy_from_slice(&self.contention_metrics.len().to_le_bytes());
        
        id
    }

    /// Apply rebalance by updating shard assignments
    pub fn apply_rebalance(&mut self, moves: &[(Pubkey, u8, u8)]) -> Result<()> {
        for (market_id, _from_shard, to_shard) in moves {
            self.shard_assignments.insert(*market_id, *to_shard);
        }
        Ok(())
    }

    /// Reset contention metrics after rebalance
    pub fn reset_contention_metrics(&mut self) {
        for metrics in self.contention_metrics.values_mut() {
            metrics.hot_markets.clear();
            // Keep some history but reduce weight
            metrics.avg_write_time_ms *= 0.5;
            metrics.peak_write_time_ms *= 0.5;
        }
    }

    /// Check if rebalance interval has passed
    pub fn should_check_rebalance(&self, current_slot: u64) -> bool {
        current_slot >= self.last_rebalance_slot + REBALANCE_INTERVAL
    }
}