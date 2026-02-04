use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use std::collections::HashMap;

use crate::sharding::types::*;
use crate::sharding::errors::ShardingError;

pub const SHARD_COUNT_DEFAULT: u8 = 4;
pub const MAX_CONTENTION_MS: f64 = 1.5;
pub const REBALANCE_INTERVAL: u64 = 1000; // slots

#[derive(Clone, Debug, PartialEq, Eq, Hash, AnchorSerialize, AnchorDeserialize)]
pub struct RebalanceProposal {
    pub id: [u8; 32],
    pub overloaded_shards: Vec<(u8, ContentionMetrics)>,
    pub underloaded_shards: Vec<(u8, ContentionMetrics)>,
    pub markets_to_move: Vec<(Pubkey, u8, u8)>, // (market, from, to)
    pub estimated_improvement: f64,
    pub votes_for: u64,
    pub votes_against: u64,
    pub voting_ends_slot: u64,
}

pub struct ShardManager {
    pub shard_assignments: HashMap<Pubkey, u8>,
    pub contention_metrics: HashMap<u8, ContentionMetrics>,
    pub rebalance_votes: HashMap<RebalanceProposal, u32>,
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

    pub fn assign_shard(&self, market_id: &Pubkey) -> u8 {
        // Deterministic hash-based assignment
        let hash = keccak::hash(&market_id.to_bytes());
        hash.0[0] % SHARD_COUNT_DEFAULT
    }

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
            id: [0u8; 32], // Will be set by generate_proposal_id
            overloaded_shards: overloaded_shards.clone(),
            underloaded_shards,
            markets_to_move: self.select_markets_to_move(&overloaded_shards),
            estimated_improvement: self.estimate_improvement(),
            votes_for: 0,
            votes_against: 0,
            voting_ends_slot: 0,
        })
    }

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

    pub fn find_best_target_shard(&self, exclude_shard: u8) -> Option<u8> {
        let mut best_shard = None;
        let mut min_contention = f64::MAX;

        for shard_id in 0..SHARD_COUNT_DEFAULT {
            if shard_id == exclude_shard {
                continue;
            }

            if let Some(metrics) = self.contention_metrics.get(&shard_id) {
                if metrics.avg_write_time_ms < min_contention {
                    min_contention = metrics.avg_write_time_ms;
                    best_shard = Some(shard_id);
                }
            } else {
                // Shard with no metrics is assumed to have no contention
                return Some(shard_id);
            }
        }

        // Only return a shard if it has acceptable contention
        if min_contention < MAX_CONTENTION_MS * 0.7 {
            best_shard
        } else {
            None
        }
    }

    pub fn estimate_improvement(&self) -> f64 {
        let mut total_contention = 0.0;
        let mut count = 0;

        for metrics in self.contention_metrics.values() {
            if metrics.avg_write_time_ms > MAX_CONTENTION_MS {
                total_contention += metrics.avg_write_time_ms - MAX_CONTENTION_MS;
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        // Estimate 30-50% improvement from rebalancing
        let avg_excess = total_contention / count as f64;
        avg_excess * 0.4 // 40% estimated improvement
    }
}