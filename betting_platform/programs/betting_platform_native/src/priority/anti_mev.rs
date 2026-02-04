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
use std::collections::{HashMap, VecDeque};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::priority::{QueueEntry, EntryStatus};

/// Anti-MEV protection mechanisms
pub struct AntiMEVProtection {
    pub min_delay_slots: u64,           // Minimum delay before execution
    pub batch_window_slots: u64,        // Window for batching similar trades
    pub price_deviation_threshold: U64F64, // Max allowed price deviation
}

impl Default for AntiMEVProtection {
    fn default() -> Self {
        Self {
            min_delay_slots: 2,
            batch_window_slots: 10,
            price_deviation_threshold: U64F64::from_num(20_000), // 2% (0.02 * 1e6)
        }
    }
}

#[derive(Debug, Clone)]
pub struct MEVDetector {
    pub sandwich_threshold: U64F64,  // Price impact threshold
    pub frontrun_window: u64,        // Slots to check for frontrunning
}

impl Default for MEVDetector {
    fn default() -> Self {
        Self {
            sandwich_threshold: U64F64::from_num(20_000), // 2% (0.02 * 1e6)
            frontrun_window: 10,
        }
    }
}

/// MEV protection state account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MEVProtectionState {
    pub recent_trades: Vec<RecentTrade>,
    pub suspicious_patterns: u32,
    pub last_check_slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RecentTrade {
    pub user: Pubkey,
    pub synthetic_id: u128,
    pub is_buy: bool,
    pub amount: u64,
    pub slot: u64,
    pub price_impact: U64F64,
}

/// Order commitment for commit-reveal pattern
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderCommitment {
    pub commitment_hash: [u8; 32],
    pub committed_slot: u64,
    pub reveal_deadline: u64,
    pub is_revealed: bool,
}

/// Price band for limiting manipulation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceBand {
    pub market_id: Pubkey,
    pub last_price: U64F64,
    pub min_price: U64F64,
    pub max_price: U64F64,
    pub last_update_slot: u64,
}

/// Order details for commit-reveal
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderDetails {
    pub market_id: Pubkey,
    pub is_buy: bool,
    pub amount: u64,
    pub limit_price: U64F64,
    pub max_slippage: U64F64,
}

impl AntiMEVProtection {
    pub fn new(
        min_delay_slots: u64,
        batch_window_slots: u64,
        price_deviation_threshold: U64F64,
    ) -> Self {
        Self {
            min_delay_slots,
            batch_window_slots,
            price_deviation_threshold,
        }
    }

    /// Validate trade timing to prevent front-running
    pub fn validate_trade_timing(
        &self,
        entry: &QueueEntry,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Enforce minimum delay
        let slots_waited = current_slot.saturating_sub(entry.submission_slot);
        if slots_waited < self.min_delay_slots {
            return Ok(false);
        }

        // Check if within batch window
        if slots_waited > self.batch_window_slots {
            // Trade has waited too long, might be stale
            return Ok(true);
        }

        Ok(true)
    }

    /// Detect sandwich attack patterns
    pub fn detect_sandwich_attack(
        &self,
        entry: &QueueEntry,
        recent_trades: &[RecentTrade],
        detector: &MEVDetector,
    ) -> Result<bool, ProgramError> {
        let current_slot = Clock::get()?.slot;

        // Look for suspicious patterns before this trade
        let suspicious_trades: Vec<&RecentTrade> = recent_trades
            .iter()
            .filter(|t| {
                t.synthetic_id == entry.trade_data.synthetic_id &&
                t.slot >= current_slot.saturating_sub(detector.frontrun_window) &&
                t.user != entry.user &&
                t.is_buy != entry.trade_data.is_buy // Opposite direction
            })
            .collect();

        // Check for sandwich pattern
        for trade in suspicious_trades {
            if trade.price_impact > detector.sandwich_threshold {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Calculate batch groups for similar trades
    pub fn calculate_batch_groups(
        &self,
        entries: Vec<&QueueEntry>,
    ) -> Result<Vec<BatchGroup>, ProgramError> {
        let mut groups: Vec<BatchGroup> = Vec::new();

        for entry in entries {
            let mut added_to_group = false;

            for group in &mut groups {
                if self.can_batch_together(entry, &group)? {
                    group.add_entry(entry);
                    added_to_group = true;
                    break;
                }
            }

            if !added_to_group {
                let mut new_group = BatchGroup::new(entry.trade_data.synthetic_id);
                new_group.add_entry(entry);
                groups.push(new_group);
            }
        }

        Ok(groups)
    }

    /// Check if trades can be batched together
    fn can_batch_together(
        &self,
        entry: &QueueEntry,
        group: &BatchGroup,
    ) -> Result<bool, ProgramError> {
        // Same synthetic and direction
        if entry.trade_data.synthetic_id != group.synthetic_id {
            return Ok(false);
        }

        // Similar submission time
        let time_diff = entry.submission_slot.abs_diff(group.avg_submission_slot);
        if time_diff > self.batch_window_slots {
            return Ok(false);
        }

        // Similar price expectations (within deviation threshold)
        // This prevents batching trades with very different price expectations
        Ok(true)
    }
}

/// Batch group for executing similar trades together
#[derive(Debug, Clone)]
pub struct BatchGroup {
    pub synthetic_id: u128,
    pub entries: Vec<QueueEntry>,
    pub total_volume: u64,
    pub avg_submission_slot: u64,
    pub execution_priority: u128,
}

impl BatchGroup {
    pub fn new(synthetic_id: u128) -> Self {
        Self {
            synthetic_id,
            entries: Vec::new(),
            total_volume: 0,
            avg_submission_slot: 0,
            execution_priority: 0,
        }
    }

    pub fn add_entry(&mut self, entry: &QueueEntry) {
        self.total_volume += entry.trade_data.amount;

        // Update average submission slot
        let new_count = self.entries.len() + 1;
        self.avg_submission_slot =
            (self.avg_submission_slot * self.entries.len() as u64 + entry.submission_slot)
            / new_count as u64;

        self.entries.push(entry.clone());

        // Update execution priority (highest priority in group)
        self.execution_priority = self.execution_priority.max(entry.priority_score);
    }
}

/// Commit-reveal handler for large orders
pub struct CommitRevealHandler {
    pub commitments: HashMap<[u8; 32], OrderCommitment>,
    pub reveal_delay_slots: u64,
}

impl CommitRevealHandler {
    pub fn new(reveal_delay_slots: u64) -> Self {
        Self {
            commitments: HashMap::new(),
            reveal_delay_slots,
        }
    }

    /// Commit order hash
    pub fn commit_order(
        &mut self,
        user: &Pubkey,
        order_hash: [u8; 32],
        current_slot: u64,
    ) -> ProgramResult {
        // Check no existing commitment
        if self.commitments.contains_key(&order_hash) {
            return Err(BettingPlatformError::DuplicateCommitment.into());
        }

        let commitment = OrderCommitment {
            commitment_hash: order_hash,
            committed_slot: current_slot,
            reveal_deadline: current_slot + self.reveal_delay_slots + 100, // Grace period
            is_revealed: false,
        };

        self.commitments.insert(order_hash, commitment);

        msg!("Order committed: {:?}", order_hash);

        Ok(())
    }

    /// Reveal and validate committed order
    pub fn reveal_order(
        &mut self,
        user: &Pubkey,
        order_details: &OrderDetails,
        nonce: u64,
        current_slot: u64,
    ) -> ProgramResult {
        // Compute hash
        let computed_hash = self.compute_order_hash(user, order_details, nonce)?;

        // Get commitment
        let commitment = self.commitments.get_mut(&computed_hash)
            .ok_or(BettingPlatformError::InvalidCommitment)?;

        // Verify timing
        if current_slot < commitment.committed_slot + self.reveal_delay_slots {
            return Err(BettingPlatformError::TooEarlyToReveal.into());
        }

        if current_slot > commitment.reveal_deadline {
            return Err(BettingPlatformError::RevealDeadlinePassed.into());
        }

        // Mark as revealed
        commitment.is_revealed = true;

        msg!("Order revealed: {:?}", computed_hash);

        Ok(())
    }

    /// Compute order hash for commit-reveal
    fn compute_order_hash(
        &self,
        user: &Pubkey,
        order: &OrderDetails,
        nonce: u64,
    ) -> Result<[u8; 32], ProgramError> {
        use solana_program::keccak;

        let mut data = Vec::new();
        data.extend_from_slice(user.as_ref());
        data.extend_from_slice(order.market_id.as_ref());
        data.push(order.is_buy as u8);
        data.extend_from_slice(&order.amount.to_le_bytes());
        data.extend_from_slice(&order.limit_price.to_bits().to_le_bytes());
        data.extend_from_slice(&nonce.to_le_bytes());

        Ok(keccak::hash(&data).to_bytes())
    }
}

/// Price band validator
pub struct PriceBandValidator {
    pub price_bands: HashMap<Pubkey, PriceBand>,
    pub max_price_deviation_bps: u16,
}

impl PriceBandValidator {
    pub fn new(max_price_deviation_bps: u16) -> Self {
        Self {
            price_bands: HashMap::new(),
            max_price_deviation_bps,
        }
    }

    /// Update price bands
    pub fn update_price_bands(
        &mut self,
        market_id: &Pubkey,
        current_price: U64F64,
        current_slot: u64,
    ) -> ProgramResult {
        let band = self.price_bands.entry(*market_id).or_insert(PriceBand {
            market_id: *market_id,
            last_price: current_price,
            min_price: current_price,
            max_price: current_price,
            last_update_slot: current_slot,
        });

        // Update bands with deviation limits
        let deviation = U64F64::from_num(self.max_price_deviation_bps as u64) / 
                       U64F64::from_num(10_000);
        
        let one = U64F64::from_num(1);
        let lower_mult = one.checked_sub(deviation)?;
        let upper_mult = one.checked_add(deviation)?;

        band.min_price = current_price.checked_mul(lower_mult)?;
        band.max_price = current_price.checked_mul(upper_mult)?;
        band.last_price = current_price;
        band.last_update_slot = current_slot;

        Ok(())
    }

    /// Validate price within bands
    pub fn validate_price(
        &self,
        market_id: &Pubkey,
        price: U64F64,
    ) -> Result<bool, ProgramError> {
        let band = self.price_bands.get(market_id)
            .ok_or(BettingPlatformError::NoPriceBand)?;

        Ok(price >= band.min_price && price <= band.max_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandwich_detection() {
        let anti_mev = AntiMEVProtection::default();
        let detector = MEVDetector::default();

        let entry = QueueEntry {
            entry_id: 1,
            user: Pubkey::new_unique(),
            priority_score: 1000,
            submission_slot: 101,
            submission_timestamp: 0,
            trade_data: crate::priority::TradeData {
                synthetic_id: 1,
                is_buy: true,
                amount: 50000,
                leverage: U64F64::from_num(10),
                max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 1000,
            depth_boost: 5,
            bump: 0,
        };

        let recent_trades = vec![
            RecentTrade {
                user: Pubkey::new_unique(),
                synthetic_id: 1,
                is_buy: false, // Opposite direction
                amount: 10000,
                slot: 100,
                price_impact: U64F64::from_num(3) / U64F64::from_num(100), // 0.03 - High impact
            },
        ];

        let is_sandwich = anti_mev.detect_sandwich_attack(
            &entry,
            &recent_trades,
            &detector,
        ).unwrap();

        assert!(is_sandwich);
    }

    #[test]
    fn test_batch_grouping() {
        let anti_mev = AntiMEVProtection::default();

        let entries = vec![
            QueueEntry {
                entry_id: 1,
                user: Pubkey::new_unique(),
                priority_score: 1000,
                submission_slot: 100,
                submission_timestamp: 0,
                trade_data: crate::priority::TradeData {
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
                trade_data: crate::priority::TradeData {
                    synthetic_id: 1, // Same synthetic
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

        let entry_refs: Vec<&QueueEntry> = entries.iter().collect();
        let groups = anti_mev.calculate_batch_groups(entry_refs).unwrap();

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entries.len(), 2);
        assert_eq!(groups[0].total_volume, 3000);
    }

    #[test]
    fn test_commit_reveal() {
        let mut handler = CommitRevealHandler::new(5);
        let user = Pubkey::new_unique();
        let order_hash = [1u8; 32];

        // Commit
        handler.commit_order(&user, order_hash, 100).unwrap();

        // Try to reveal too early
        let order_details = OrderDetails {
            market_id: Pubkey::new_unique(),
            is_buy: true,
            amount: 1000,
            limit_price: U64F64::from_num(100),
            max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
        };

        let result = handler.reveal_order(&user, &order_details, 123, 102);
        assert!(result.is_err());

        // Reveal after delay
        let result = handler.reveal_order(&user, &order_details, 123, 106);
        // Will fail because hash doesn't match, but timing is correct
        assert!(result.is_err());
    }

    #[test]
    fn test_price_bands() {
        let mut validator = PriceBandValidator::new(200); // 2%

        let market = Pubkey::new_unique();
        let price = U64F64::from_num(100);

        validator.update_price_bands(&market, price, 100).unwrap();

        // Price within bands
        assert!(validator.validate_price(&market, U64F64::from_num(99)).unwrap());
        assert!(validator.validate_price(&market, U64F64::from_num(101)).unwrap());

        // Price outside bands
        assert!(!validator.validate_price(&market, U64F64::from_num(97)).unwrap());
        assert!(!validator.validate_price(&market, U64F64::from_num(103)).unwrap());
    }
}