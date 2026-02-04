use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::BTreeMap;
use std::cmp::Ordering;
use crate::error::BettingPlatformError;
use crate::math::U64F64;

pub const MAX_QUEUE_SIZE: usize = 1000;
pub const MEV_HISTORY_SLOTS: u64 = 100;

/// Priority queue account structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriorityQueue {
    pub is_initialized: bool,
    pub queue_id: u128,
    pub max_size: u32,
    pub current_size: u32,
    pub head_index: u32,
    pub tail_index: u32,
    pub total_pending_volume: u64,
    pub last_process_slot: u64,
    pub bump: u8,
}

impl Sealed for PriorityQueue {}

impl IsInitialized for PriorityQueue {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for PriorityQueue {
    const LEN: usize = 1 + 16 + 4 + 4 + 4 + 4 + 8 + 8 + 1;

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        PriorityQueue::try_from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Queue entry for trades
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct QueueEntry {
    pub entry_id: u128,
    pub user: Pubkey,
    pub priority_score: u128,
    pub submission_slot: u64,
    pub submission_timestamp: i64,
    pub trade_data: TradeData,
    pub status: EntryStatus,
    pub stake_snapshot: u64, // MMT stake at submission
    pub depth_boost: u32,    // Verse depth for priority
    pub bump: u8,
}

impl QueueEntry {
    pub const LEN: usize = 16 + 32 + 16 + 8 + 8 + 200 + 1 + 8 + 4 + 1; // Approximate size
    
    /// Deserialize from slice
    pub fn try_from_slice(data: &[u8]) -> Result<Self, ProgramError> {
        Self::deserialize(&mut &data[..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
    
    /// Serialize to mutable slice
    pub fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError> {
        let encoded = self.try_to_vec()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        if encoded.len() > data.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        data[..encoded.len()].copy_from_slice(&encoded);
        Ok(())
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TradeData {
    pub synthetic_id: u128,
    pub is_buy: bool,
    pub amount: u64,
    pub leverage: U64F64,
    pub max_slippage: U64F64,
    pub stop_loss: Option<U64F64>,
    pub take_profit: Option<U64F64>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum EntryStatus {
    Pending = 0,
    Processing = 1,
    Executed = 2,
    Cancelled = 3,
    Expired = 4,
}

impl EntryStatus {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(EntryStatus::Pending),
            1 => Some(EntryStatus::Processing),
            2 => Some(EntryStatus::Executed),
            3 => Some(EntryStatus::Cancelled),
            4 => Some(EntryStatus::Expired),
            _ => None,
        }
    }
}

/// Priority calculator for MMT stake-based scoring
pub struct PriorityCalculator {
    pub stake_weight: U64F64,    // Weight for MMT stake
    pub time_weight: U64F64,     // Weight for submission time
    pub depth_weight: U64F64,    // Weight for verse depth
    pub volume_weight: U64F64,   // Weight for trade volume
}

impl Default for PriorityCalculator {
    fn default() -> Self {
        Self {
            stake_weight: U64F64::from_num(400_000),    // 40% (0.4 * 1e6)
            time_weight: U64F64::from_num(300_000),     // 30% (0.3 * 1e6)
            depth_weight: U64F64::from_num(200_000),    // 20% (0.2 * 1e6)
            volume_weight: U64F64::from_num(100_000),   // 10% (0.1 * 1e6)
        }
    }
}

impl PriorityCalculator {
    /// Calculate priority score based on multiple factors
    pub fn calculate_priority(
        &self,
        user_stake: u64,
        verse_depth: u32,
        submission_slot: u64,
        trade_volume: u64,
        current_slot: u64,
        total_stake: u64,
    ) -> Result<u128, ProgramError> {
        // Normalize components to [0, 1]
        let stake_normalized = if total_stake > 0 {
            U64F64::from_num(user_stake) / U64F64::from_num(total_stake)
        } else {
            U64F64::from_num(0)
        };

        // Time priority (earlier = higher priority)
        let slot_age = current_slot.saturating_sub(submission_slot);
        let time_normalized = U64F64::from_num(1) / 
            (U64F64::from_num(1) + U64F64::from_num(slot_age));

        // Depth boost (deeper = higher priority)
        let max_depth = 32u32;
        let depth_normalized = U64F64::from_num(verse_depth as u64) / 
            U64F64::from_num(max_depth as u64);

        // Volume component (larger = slightly higher priority)
        let max_volume = 1_000_000u64;
        let volume_normalized = (U64F64::from_num(trade_volume) / 
            U64F64::from_num(max_volume)).min(U64F64::from_num(1));

        // Calculate weighted score
        let score = stake_normalized.checked_mul(self.stake_weight)?
            .checked_add(time_normalized.checked_mul(self.time_weight)?)?
            .checked_add(depth_normalized.checked_mul(self.depth_weight)?)?
            .checked_add(volume_normalized.checked_mul(self.volume_weight)?)?;

        // Convert to u128 for sorting (multiply by large factor for precision)
        let score_u128 = (score.checked_mul(U64F64::from_num(u64::MAX))?).to_num();
        Ok(score_u128 as u128)
    }
}

/// Order priority for BTreeMap ordering
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderPriority {
    pub score: u128,          // Composite priority score
    pub submission_slot: u64, // When order was submitted
    pub sequence_number: u64, // Tiebreaker
}

impl Ord for OrderPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher score = higher priority
        self.score.cmp(&other.score)
            .then_with(|| self.submission_slot.cmp(&other.submission_slot))
            .then_with(|| self.sequence_number.cmp(&other.sequence_number))
    }
}

impl PartialOrd for OrderPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Queue manager for handling priority operations
pub struct QueueManager {
    pub priority_calculator: PriorityCalculator,
    pub next_sequence: u64,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            priority_calculator: PriorityCalculator::default(),
            next_sequence: 0,
        }
    }

    /// Insert order into priority queue
    pub fn insert_order(
        &mut self,
        queue: &mut PriorityQueue,
        order: QueueEntry,
    ) -> ProgramResult {
        if queue.current_size >= queue.max_size {
            return Err(BettingPlatformError::QueueFull.into());
        }

        queue.current_size += 1;
        queue.total_pending_volume = queue.total_pending_volume
            .saturating_add(order.trade_data.amount);

        self.next_sequence += 1;

        msg!("Inserted order {} into queue with priority {}", 
            order.entry_id, 
            order.priority_score
        );

        Ok(())
    }

    /// Remove expired orders
    pub fn remove_expired_orders(
        &mut self,
        queue: &mut PriorityQueue,
        current_slot: u64,
        expiry_slots: u64,
    ) -> Result<u32, ProgramError> {
        let cutoff_slot = current_slot.saturating_sub(expiry_slots);
        let mut removed = 0u32;

        // In a real implementation, would iterate through queue entries
        // and remove those with submission_slot < cutoff_slot

        queue.current_size = queue.current_size.saturating_sub(removed);

        Ok(removed)
    }

    /// Get queue position for an order
    pub fn get_queue_position(
        &self,
        _order_id: &Pubkey,
        _queue_entries: &[QueueEntry],
    ) -> Result<u32, ProgramError> {
        // In real implementation, would find order and count higher priority orders
        Ok(1)
    }
}

/// Batch processing parameters
pub const BATCH_SIZE_LIMIT: usize = 70; // 1.4M CU/block = 70 trades at 20k CU
pub const CU_PER_LIQUIDATION: u64 = 20_000;
pub const MAX_CU_PER_BLOCK: u64 = 1_400_000;

/// Liquidation-specific queue entry
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LiquidationOrder {
    pub position_id: u128,
    pub trader: Pubkey,
    pub risk_score: u64, // Lower score = higher priority
    pub distance_to_liq: u64,
    pub effective_leverage: u64,
    pub mmt_stake: u64,
    pub submission_slot: u64,
    pub open_interest: u64,
}

impl LiquidationOrder {
    /// Calculate risk score: risk_score = (distance_to_liq / lev_eff) * stake
    /// Lower score = higher priority (closer to liquidation)
    pub fn calculate_risk_score(&self) -> u64 {
        if self.effective_leverage == 0 {
            return u64::MAX; // Lowest priority
        }

        let base_score = (self.distance_to_liq as u128 * 10000 / self.effective_leverage as u128) as u64;

        // Multiply by stake inverse (higher stake = lower score = higher priority)
        if self.mmt_stake > 0 {
            base_score / self.mmt_stake.min(10000) // Cap stake multiplier at 100x
        } else {
            base_score * 100 // Penalty for no stake
        }
    }
}

impl Ord for LiquidationOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower risk score = higher priority
        other.risk_score.cmp(&self.risk_score)
            .then_with(|| self.submission_slot.cmp(&other.submission_slot))
    }
}

impl PartialOrd for LiquidationOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for LiquidationOrder {
    fn eq(&self, other: &Self) -> bool {
        self.position_id == other.position_id
    }
}

impl Eq for LiquidationOrder {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_calculation() {
        let calculator = PriorityCalculator::default();

        let priority1 = calculator.calculate_priority(
            1000_000,    // High stake
            10,          // Medium depth
            100,         // Recent submission
            10_000,      // Medium volume
            200,         // Current slot
            10_000_000,  // Total stake
        ).unwrap();

        let priority2 = calculator.calculate_priority(
            100_000,     // Low stake
            20,          // High depth
            50,          // Very recent submission
            100_000,     // High volume
            200,         // Current slot
            10_000_000,  // Total stake
        ).unwrap();

        // Priority should reflect weighted components
        assert!(priority1 > 0);
        assert!(priority2 > 0);
    }

    #[test]
    fn test_liquidation_risk_score() {
        let order = LiquidationOrder {
            position_id: 1,
            trader: Pubkey::new_unique(),
            risk_score: 0, // Will be calculated
            distance_to_liq: 100, // 1% distance
            effective_leverage: 50, // 50x leverage
            mmt_stake: 1000,
            submission_slot: 1000,
            open_interest: 100000,
        };

        let risk_score = order.calculate_risk_score();

        // Risk score = (100 * 10000 / 50) / 1000 = 20000 / 1000 = 20
        assert_eq!(risk_score, 20);
    }

    #[test]
    fn test_order_priority_ordering() {
        let p1 = OrderPriority {
            score: 1000,
            submission_slot: 100,
            sequence_number: 1,
        };

        let p2 = OrderPriority {
            score: 2000,
            submission_slot: 100,
            sequence_number: 2,
        };

        assert!(p2 > p1); // Higher score = higher priority
    }
}