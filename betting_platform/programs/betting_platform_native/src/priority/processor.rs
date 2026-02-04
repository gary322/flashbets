use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::{HashMap, HashSet};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::priority::{
    PriorityQueue, QueueEntry, EntryStatus, AntiMEVProtection, 
    MEVProtectionState, MEVDetector, RecentTrade, BatchGroup
};
use crate::synthetics::router::{RouteRequest, RoutingEngine};

pub const ESTIMATED_GAS_PER_TRADE: u64 = 200_000;
pub const MEV_HISTORY_SLOTS: u64 = 100;

/// Queue processor for handling priority execution
pub struct QueueProcessor {
    pub max_batch_size: usize,
    pub max_gas_per_batch: u64,
    pub anti_mev: AntiMEVProtection,
}

impl Default for QueueProcessor {
    fn default() -> Self {
        Self {
            max_batch_size: 50,
            max_gas_per_batch: 10_000_000,
            anti_mev: AntiMEVProtection::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub processed_count: u32,
    pub failed_count: u32,
    pub total_volume: u64,
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct BatchExecutionResult {
    pub success_count: u32,
    pub fail_count: u32,
    pub volume: u64,
    pub gas_used: u64,
    pub price_impact: U64F64,
    pub executed_entries: Vec<QueueEntry>,
}

impl QueueProcessor {
    pub fn new(
        max_batch_size: usize,
        max_gas_per_batch: u64,
        anti_mev: AntiMEVProtection,
    ) -> Self {
        Self {
            max_batch_size,
            max_gas_per_batch,
            anti_mev,
        }
    }

    /// Process queue with MEV protection
    pub fn process_queue(
        &self,
        queue: &mut PriorityQueue,
        entries: &mut Vec<QueueEntry>,
        mev_state: &mut MEVProtectionState,
    ) -> Result<ProcessResult, ProgramError> {
        let current_slot = Clock::get()?.slot;

        // Filter entries ready for processing
        let ready_entries: Vec<&mut QueueEntry> = entries
            .iter_mut()
            .filter(|e| {
                e.status == EntryStatus::Pending &&
                self.anti_mev.validate_trade_timing(e, current_slot).unwrap_or(false)
            })
            .collect();

        // Sort by priority (highest first)
        let mut sorted_entries = ready_entries;
        sorted_entries.sort_by(|a, b| b.priority_score.cmp(&a.priority_score));

        // Take top entries up to batch size
        let batch_entries: Vec<&mut QueueEntry> = sorted_entries
            .into_iter()
            .take(self.max_batch_size)
            .collect();

        // Check for MEV attacks
        let mut safe_entries = Vec::new();
        for entry in batch_entries {
            let is_sandwich = self.anti_mev.detect_sandwich_attack(
                entry,
                &mev_state.recent_trades,
                &MEVDetector::default(),
            )?;

            if !is_sandwich {
                safe_entries.push(entry);
            } else {
                entry.status = EntryStatus::Cancelled;
                msg!("MEV attack detected for user {}", entry.user);
            }
        }

        // Group into batches for execution
        let batch_groups = self.anti_mev.calculate_batch_groups(
            safe_entries.iter().map(|e| &**e).collect()
        )?;

        let mut result = ProcessResult {
            processed_count: 0,
            failed_count: 0,
            total_volume: 0,
            gas_used: 0,
        };

        // Execute batches
        for group in batch_groups {
            if result.gas_used + ESTIMATED_GAS_PER_TRADE * group.entries.len() as u64
                > self.max_gas_per_batch {
                break;
            }

            let group_size = group.entries.len();
            match self.execute_batch(group) {
                Ok(batch_result) => {
                    result.processed_count += batch_result.success_count;
                    result.failed_count += batch_result.fail_count;
                    result.total_volume += batch_result.volume;
                    result.gas_used += batch_result.gas_used;

                    // Update MEV state
                    for entry in batch_result.executed_entries {
                        mev_state.recent_trades.push(RecentTrade {
                            user: entry.user,
                            synthetic_id: entry.trade_data.synthetic_id,
                            is_buy: entry.trade_data.is_buy,
                            amount: entry.trade_data.amount,
                            slot: current_slot,
                            price_impact: batch_result.price_impact,
                        });
                    }
                }
                Err(e) => {
                    msg!("Batch execution failed: {:?}", e);
                    result.failed_count += group_size as u32;
                }
            }
        }

        // Clean up old entries from MEV state
        mev_state.recent_trades.retain(|t| {
            current_slot.saturating_sub(t.slot) < MEV_HISTORY_SLOTS
        });

        // Update queue state
        queue.current_size = entries.iter()
            .filter(|e| e.status == EntryStatus::Pending)
            .count() as u32;
        queue.last_process_slot = current_slot;

        Ok(result)
    }

    /// Execute a batch of similar trades
    fn execute_batch(
        &self,
        group: BatchGroup,
    ) -> Result<BatchExecutionResult, ProgramError> {
        // Combine orders for efficiency
        let combined_amount: u64 = group.entries.iter()
            .map(|e| e.trade_data.amount)
            .sum();

        let avg_leverage = group.entries.iter()
            .map(|e| e.trade_data.leverage)
            .fold(U64F64::from_num(0), |acc, lev| acc + lev)
            / U64F64::from_num(group.entries.len() as u64);

        // Simulate execution
        let mut batch_result = BatchExecutionResult {
            success_count: 0,
            fail_count: 0,
            volume: 0,
            gas_used: ESTIMATED_GAS_PER_TRADE * group.entries.len() as u64,
            price_impact: U64F64::from_num(1_000), // 0.1% simulated impact (0.001 * 1e6)
            executed_entries: Vec::new(),
        };

        for mut entry in group.entries {
            // Simulate proportional fill
            let fill_ratio = U64F64::from_num(950_000); // 95% fill rate (0.95 * 1e6)
            let entry_fill = (U64F64::from_num(entry.trade_data.amount) * fill_ratio).to_num();

            if entry_fill > 0 {
                entry.status = EntryStatus::Executed;
                batch_result.success_count += 1;
                batch_result.volume += entry_fill;
                batch_result.executed_entries.push(entry);
            } else {
                entry.status = EntryStatus::Cancelled;
                batch_result.fail_count += 1;
            }
        }

        Ok(batch_result)
    }
}

/// Congestion manager for high-traffic periods
pub struct CongestionManager {
    pub congestion_threshold: f64,    // 80% of max TPS
    pub priority_boost_factor: U64F64,
    pub batch_size: u32,
    pub fairness_window: u64,
}

impl Default for CongestionManager {
    fn default() -> Self {
        Self {
            congestion_threshold: 0.8,
            priority_boost_factor: U64F64::from_num(1_500_000), // 1.5x (1.5 * 1e6)
            batch_size: 30,
            fairness_window: 10,
        }
    }
}

impl CongestionManager {
    /// Process orders fairly during high congestion
    pub fn process_congested_batch(
        &mut self,
        queue: &mut PriorityQueue,
        entries: &mut Vec<QueueEntry>,
        max_batch_size: u32,
        current_slot: u64,
    ) -> Result<Vec<QueueEntry>, ProgramError> {
        let mut selected_orders = Vec::new();
        let mut users_in_batch = HashSet::new();

        // First pass: High priority orders (with stake)
        let high_priority_count = (max_batch_size as f64 * 0.7) as u32;

        // Sort entries by priority
        entries.sort_by(|a, b| b.priority_score.cmp(&a.priority_score));

        for entry in entries.iter_mut() {
            if selected_orders.len() >= high_priority_count as usize {
                break;
            }

            if entry.status != EntryStatus::Pending {
                continue;
            }

            // Skip if user already in batch (fairness)
            if users_in_batch.contains(&entry.user) {
                continue;
            }

            // Validate order still valid
            if entry.submission_slot + 1000 < current_slot { // Expire after 1000 slots
                entry.status = EntryStatus::Expired;
                continue;
            }

            selected_orders.push(entry.clone());
            users_in_batch.insert(entry.user);
        }

        // Second pass: Fill remaining with FIFO (fairness for low stake)
        let remaining_slots = max_batch_size as usize - selected_orders.len();
        let mut fifo_count = 0;

        for entry in entries.iter() {
            if fifo_count >= remaining_slots {
                break;
            }

            if entry.status != EntryStatus::Pending {
                continue;
            }

            if !users_in_batch.contains(&entry.user) {
                selected_orders.push(entry.clone());
                users_in_batch.insert(entry.user);
                fifo_count += 1;
            }
        }

        // Update queue size
        queue.current_size = entries.iter()
            .filter(|e| e.status == EntryStatus::Pending)
            .count() as u32;

        msg!(
            "Congestion batch processed: {} orders, {} unique users",
            selected_orders.len(),
            users_in_batch.len()
        );

        Ok(selected_orders)
    }

    /// Check if system is congested
    pub fn is_congested(&self, current_tps: f64, max_tps: f64) -> bool {
        current_tps / max_tps >= self.congestion_threshold
    }

    /// Adjust priority during congestion
    pub fn adjust_priority_for_congestion(
        &self,
        base_priority: u128,
        is_high_stake: bool,
    ) -> u128 {
        if is_high_stake {
            let boosted = U64F64::from_num(base_priority as u64) * self.priority_boost_factor;
            boosted.to_num() as u128
        } else {
            base_priority
        }
    }
}

/// Batch execution optimizer
pub struct BatchOptimizer {
    pub min_batch_size: usize,
    pub max_batch_size: usize,
    pub gas_per_trade: u64,
}

impl Default for BatchOptimizer {
    fn default() -> Self {
        Self {
            min_batch_size: 5,
            max_batch_size: 50,
            gas_per_trade: ESTIMATED_GAS_PER_TRADE,
        }
    }
}

impl BatchOptimizer {
    /// Optimize batch size based on gas and similarity
    pub fn optimize_batch_size(
        &self,
        entries: &[QueueEntry],
        max_gas: u64,
    ) -> usize {
        let max_by_gas = (max_gas / self.gas_per_trade) as usize;
        let optimal_size = entries.len().min(max_by_gas).min(self.max_batch_size);

        if optimal_size < self.min_batch_size {
            0 // Don't process if too small
        } else {
            optimal_size
        }
    }

    /// Group entries by similarity for optimal execution
    pub fn group_by_similarity(
        &self,
        entries: &[QueueEntry],
    ) -> Vec<Vec<QueueEntry>> {
        let mut groups: HashMap<(u128, bool), Vec<QueueEntry>> = HashMap::new();

        for entry in entries {
            let key = (entry.trade_data.synthetic_id, entry.trade_data.is_buy);
            groups.entry(key).or_insert_with(Vec::new).push(entry.clone());
        }

        groups.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::priority::{TradeData, QueueManager};

    #[test]
    fn test_queue_processing() {
        let processor = QueueProcessor::default();
        let mut queue = PriorityQueue {
            is_initialized: true,
            queue_id: 1,
            max_size: 1000,
            current_size: 2,
            head_index: 0,
            tail_index: 2,
            total_pending_volume: 3000,
            last_process_slot: 0,
            bump: 0,
        };

        let mut entries = vec![
            QueueEntry {
                entry_id: 1,
                user: Pubkey::new_unique(),
                priority_score: 1000,
                submission_slot: 100,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 1000,
                    leverage: U64F64::from_num(10),
                    max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: 1000,
                depth_boost: 5,
                bump: 0,
            },
            QueueEntry {
                entry_id: 2,
                user: Pubkey::new_unique(),
                priority_score: 900,
                submission_slot: 102,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 2000,
                    leverage: U64F64::from_num(20),
                    max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: 500,
                depth_boost: 5,
                bump: 0,
            },
        ];

        let mut mev_state = MEVProtectionState {
            recent_trades: Vec::new(),
            suspicious_patterns: 0,
            last_check_slot: 0,
        };

        let result = processor.process_queue(&mut queue, &mut entries, &mut mev_state).unwrap();

        assert!(result.processed_count > 0);
        assert_eq!(result.total_volume, 2850); // 95% of 3000
    }

    #[test]
    fn test_congestion_management() {
        let mut manager = CongestionManager::default();
        let mut queue = PriorityQueue {
            is_initialized: true,
            queue_id: 1,
            max_size: 1000,
            current_size: 100,
            head_index: 0,
            tail_index: 100,
            total_pending_volume: 100_000,
            last_process_slot: 0,
            bump: 0,
        };

        let mut entries = Vec::new();
        for i in 0..10 {
            entries.push(QueueEntry {
                entry_id: i as u128,
                user: Pubkey::new_unique(),
                priority_score: (1000 - i * 100) as u128,
                submission_slot: 100 + i,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 1000,
                    leverage: U64F64::from_num(10),
                    max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: (1000 - i * 100) as u64,
                depth_boost: 5,
                bump: 0,
            });
        }

        let selected = manager.process_congested_batch(
            &mut queue,
            &mut entries,
            5,
            200,
        ).unwrap();

        assert_eq!(selected.len(), 5);
        // Should prioritize high stake users
        assert!(selected[0].priority_score >= selected[4].priority_score);
    }

    #[test]
    fn test_batch_optimization() {
        let optimizer = BatchOptimizer::default();

        let entries = vec![
            QueueEntry {
                entry_id: 1,
                user: Pubkey::new_unique(),
                priority_score: 1000,
                submission_slot: 100,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 1000,
                    leverage: U64F64::from_num(10),
                    max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: 1000,
                depth_boost: 5,
                bump: 0,
            },
            QueueEntry {
                entry_id: 2,
                user: Pubkey::new_unique(),
                priority_score: 900,
                submission_slot: 102,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: false, // Different direction
                    amount: 2000,
                    leverage: U64F64::from_num(20),
                    max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: 500,
                depth_boost: 5,
                bump: 0,
            },
        ];

        let groups = optimizer.group_by_similarity(&entries);
        assert_eq!(groups.len(), 2); // Separated by direction
    }
}