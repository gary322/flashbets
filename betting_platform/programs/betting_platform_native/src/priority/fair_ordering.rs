use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;
use crate::error::BettingPlatformError;
use crate::priority::QueueEntry;

/// Fair ordering protocol for preventing manipulation
pub struct FairOrderingProtocol {
    pub randomness_delay: u64,      // Slots to wait for VRF
    pub batch_randomization: bool,  // Randomize within priority tiers
}

impl Default for FairOrderingProtocol {
    fn default() -> Self {
        Self {
            randomness_delay: 5,
            batch_randomization: true,
        }
    }
}

/// Ordering state for tracking randomness
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderingState {
    pub current_epoch: u64,
    pub randomness_seed: [u8; 32],
    pub last_vrf_slot: u64,
    pub pending_randomness: bool,
}

impl OrderingState {
    pub fn new() -> Self {
        Self {
            current_epoch: 0,
            randomness_seed: [0u8; 32],
            last_vrf_slot: 0,
            pending_randomness: false,
        }
    }
    
    /// Unpack from account data
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        Self::deserialize(&mut &data[..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
    
    /// Pack into account data
    pub fn pack(state: Self, data: &mut [u8]) -> Result<(), ProgramError> {
        let encoded = state.try_to_vec()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        if encoded.len() > data.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        data[..encoded.len()].copy_from_slice(&encoded);
        Ok(())
    }
}

impl FairOrderingProtocol {
    pub fn new(randomness_delay: u64, batch_randomization: bool) -> Self {
        Self {
            randomness_delay,
            batch_randomization,
        }
    }

    /// Apply fair ordering to queue entries
    pub fn apply_fair_ordering(
        &self,
        entries: &mut Vec<QueueEntry>,
        ordering_state: &OrderingState,
    ) -> ProgramResult {
        // Group by priority tiers using indices
        let mut tier_indices: Vec<Vec<usize>> = Vec::new();
        let tier_size = u128::MAX / 10; // 10 priority tiers

        for (idx, entry) in entries.iter().enumerate() {
            let tier_index = (entry.priority_score / tier_size) as usize;
            
            while tier_indices.len() <= tier_index {
                tier_indices.push(Vec::new());
            }
            
            tier_indices[tier_index].push(idx);
        }

        // Apply randomization within tiers if enabled
        if self.batch_randomization && ordering_state.randomness_seed != [0u8; 32] {
            let mut rng = XorShiftRng::from_seed(ordering_state.randomness_seed);
            
            for indices in &mut tier_indices {
                // Fisher-Yates shuffle on indices
                for i in (1..indices.len()).rev() {
                    let j = rng.gen_range(0..=i);
                    indices.swap(i, j);
                }
            }
        }

        // Create new ordered list
        let mut ordered_entries = Vec::new();
        
        // Process tiers from highest to lowest
        for indices in tier_indices.into_iter().rev() {
            for idx in indices {
                ordered_entries.push(entries[idx].clone());
            }
        }

        // Replace entries with ordered version
        *entries = ordered_entries;

        Ok(())
    }


    /// Request randomness for fair ordering
    pub fn request_randomness(
        &self,
        ordering_state: &mut OrderingState,
    ) -> ProgramResult {
        let current_slot = Clock::get()?.slot;

        if current_slot >= ordering_state.last_vrf_slot + self.randomness_delay {
            ordering_state.pending_randomness = true;
            ordering_state.last_vrf_slot = current_slot;

            msg!(
                "Randomness requested for epoch {} at slot {}",
                ordering_state.current_epoch,
                current_slot
            );
        }

        Ok(())
    }

    /// Update randomness from VRF
    pub fn update_randomness(
        &self,
        ordering_state: &mut OrderingState,
        vrf_output: [u8; 32],
    ) -> ProgramResult {
        if !ordering_state.pending_randomness {
            return Err(ProgramError::InvalidAccountData);
        }

        ordering_state.randomness_seed = vrf_output;
        ordering_state.pending_randomness = false;
        ordering_state.current_epoch += 1;

        msg!(
            "Randomness updated for epoch {}",
            ordering_state.current_epoch
        );

        Ok(())
    }
}

/// Simple XorShift RNG for deterministic randomization
pub struct XorShiftRng {
    state: u64,
}

impl XorShiftRng {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        // Convert first 8 bytes of seed to u64
        let mut state = u64::from_le_bytes(seed[0..8].try_into().unwrap());
        if state == 0 {
            state = 0xDEADBEEF; // Avoid zero state
        }
        Self { state }
    }

    pub fn gen_range(&mut self, range: std::ops::RangeInclusive<usize>) -> usize {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;

        let start = *range.start();
        let end = *range.end();
        start + (self.state as usize % (end - start + 1))
    }
}

/// Time-based ordering for additional fairness
pub struct TimeBasedOrdering {
    pub time_window_slots: u64,
    pub priority_decay_rate: f64,
}

impl Default for TimeBasedOrdering {
    fn default() -> Self {
        Self {
            time_window_slots: 100,
            priority_decay_rate: 0.01, // 1% per slot
        }
    }
}

impl TimeBasedOrdering {
    /// Adjust priority based on wait time
    pub fn adjust_priority_by_time(
        &self,
        base_priority: u128,
        submission_slot: u64,
        current_slot: u64,
    ) -> u128 {
        let slots_waited = current_slot.saturating_sub(submission_slot);
        
        // Increase priority for orders that have waited longer
        let time_boost = (slots_waited as f64 * self.priority_decay_rate).min(1.0);
        let boosted_priority = base_priority as f64 * (1.0 + time_boost);
        
        boosted_priority as u128
    }

    /// Check if order has waited too long
    pub fn is_stale(
        &self,
        submission_slot: u64,
        current_slot: u64,
    ) -> bool {
        current_slot.saturating_sub(submission_slot) > self.time_window_slots
    }
}

/// Fairness metrics for monitoring
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct FairnessMetrics {
    pub total_orders_processed: u64,
    pub unique_users_served: u64,
    pub avg_wait_time_slots: u64,
    pub max_wait_time_slots: u64,
    pub priority_distribution: Vec<u64>, // Count per tier
}

impl FairnessMetrics {
    pub fn new() -> Self {
        Self {
            total_orders_processed: 0,
            unique_users_served: 0,
            avg_wait_time_slots: 0,
            max_wait_time_slots: 0,
            priority_distribution: vec![0; 10], // 10 tiers
        }
    }

    /// Update metrics after processing batch
    pub fn update_batch_metrics(
        &mut self,
        processed_entries: &[QueueEntry],
        current_slot: u64,
    ) {
        let mut unique_users = std::collections::HashSet::new();
        let mut total_wait_time = 0u64;
        let tier_size = u128::MAX / 10;

        for entry in processed_entries {
            self.total_orders_processed += 1;
            unique_users.insert(entry.user);

            let wait_time = current_slot.saturating_sub(entry.submission_slot);
            total_wait_time += wait_time;
            self.max_wait_time_slots = self.max_wait_time_slots.max(wait_time);

            let tier_index = (entry.priority_score / tier_size) as usize;
            if tier_index < self.priority_distribution.len() {
                self.priority_distribution[tier_index] += 1;
            }
        }

        self.unique_users_served = unique_users.len() as u64;
        
        if !processed_entries.is_empty() {
            self.avg_wait_time_slots = total_wait_time / processed_entries.len() as u64;
        }
    }

    /// Calculate fairness score (0-100)
    pub fn calculate_fairness_score(&self) -> u8 {
        let mut score = 0u8;

        // User diversity (40 points)
        if self.total_orders_processed > 0 {
            let diversity_ratio = self.unique_users_served as f64 / self.total_orders_processed as f64;
            score += (diversity_ratio * 40.0) as u8;
        }

        // Wait time fairness (30 points)
        if self.avg_wait_time_slots < 50 {
            score += 30;
        } else if self.avg_wait_time_slots < 100 {
            score += 20;
        } else if self.avg_wait_time_slots < 200 {
            score += 10;
        }

        // Priority distribution fairness (30 points)
        let total_in_distribution: u64 = self.priority_distribution.iter().sum();
        if total_in_distribution > 0 {
            let mut distribution_score = 30;
            
            // Check if too concentrated in top tiers
            let top_tier_ratio = self.priority_distribution[8..].iter().sum::<u64>() as f64 
                / total_in_distribution as f64;
            
            if top_tier_ratio > 0.8 {
                distribution_score = 10; // Too concentrated
            } else if top_tier_ratio > 0.6 {
                distribution_score = 20;
            }
            
            score += distribution_score;
        }

        score.min(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::priority::{TradeData, EntryStatus};

    #[test]
    fn test_fair_ordering() {
        let protocol = FairOrderingProtocol::default();

        let mut entries = vec![];
        for i in 0..10 {
            entries.push(QueueEntry {
                entry_id: i as u128,
                user: Pubkey::new_unique(),
                priority_score: (i as u128) * 1000,
                submission_slot: 100 + i,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 1000,
                    leverage: crate::math::U64F64::from_num(10),
                    max_slippage: crate::math::U64F64::from_num(2) / crate::math::U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: 1000,
                depth_boost: 5,
                bump: 0,
            });
        }

        let ordering_state = OrderingState {
            current_epoch: 1,
            randomness_seed: [1u8; 32],
            last_vrf_slot: 0,
            pending_randomness: false,
        };

        protocol.apply_fair_ordering(&mut entries, &ordering_state).unwrap();

        // Verify high priority entries are first
        for i in 1..entries.len() {
            let tier_i = entries[i-1].priority_score / (u128::MAX / 10);
            let tier_j = entries[i].priority_score / (u128::MAX / 10);
            assert!(tier_i >= tier_j);
        }
    }

    #[test]
    fn test_xorshift_rng() {
        let mut rng = XorShiftRng::from_seed([42u8; 32]);
        
        let mut values = Vec::new();
        for _ in 0..10 {
            values.push(rng.gen_range(0..=9));
        }

        // Check that values are distributed
        let unique_values: std::collections::HashSet<_> = values.iter().collect();
        assert!(unique_values.len() > 3); // Should have some variety
    }

    #[test]
    fn test_time_based_ordering() {
        let time_ordering = TimeBasedOrdering::default();

        let base_priority = 1000u128;
        let submission_slot = 100u64;
        let current_slot = 150u64;

        let adjusted = time_ordering.adjust_priority_by_time(
            base_priority,
            submission_slot,
            current_slot,
        );

        // Should be boosted by 50 * 0.01 = 50%
        assert!(adjusted > base_priority);
        assert_eq!(adjusted, 1500);
    }

    #[test]
    fn test_fairness_metrics() {
        let mut metrics = FairnessMetrics::new();

        let entries = vec![
            QueueEntry {
                entry_id: 1,
                user: Pubkey::new_unique(),
                priority_score: 9000000000000000000000000000000000000,
                submission_slot: 100,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 1000,
                    leverage: crate::math::U64F64::from_num(10),
                    max_slippage: crate::math::U64F64::from_num(2) / crate::math::U64F64::from_num(100), // 0.02
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
                priority_score: 1000000000000000000000000000000000000,
                submission_slot: 110,
                submission_timestamp: 0,
                trade_data: TradeData {
                    synthetic_id: 1,
                    is_buy: true,
                    amount: 1000,
                    leverage: crate::math::U64F64::from_num(10),
                    max_slippage: crate::math::U64F64::from_num(2) / crate::math::U64F64::from_num(100), // 0.02
                    stop_loss: None,
                    take_profit: None,
                },
                status: EntryStatus::Pending,
                stake_snapshot: 100,
                depth_boost: 5,
                bump: 0,
            },
        ];

        metrics.update_batch_metrics(&entries, 150);

        assert_eq!(metrics.total_orders_processed, 2);
        assert_eq!(metrics.unique_users_served, 2);
        assert_eq!(metrics.avg_wait_time_slots, 45); // (50 + 40) / 2

        let fairness_score = metrics.calculate_fairness_score();
        assert!(fairness_score > 50); // Should have decent fairness
    }
}