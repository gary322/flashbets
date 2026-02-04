//! Queue Entry Storage Management
//!
//! Handles loading and saving queue entries for the priority queue system

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::error::BettingPlatformError;
use super::{QueueEntry, EntryStatus, MAX_QUEUE_SIZE};

/// Queue entries storage account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct QueueEntriesStorage {
    pub queue_id: u128,
    pub entries: Vec<QueueEntry>,
    pub total_entries: u32,
    pub last_update_slot: u64,
}

impl QueueEntriesStorage {
    /// Create new storage
    pub fn new(queue_id: u128, current_slot: u64) -> Self {
        Self {
            queue_id,
            entries: Vec::with_capacity(MAX_QUEUE_SIZE),
            total_entries: 0,
            last_update_slot: current_slot,
        }
    }
    
    /// Load entries from account
    pub fn load_entries(account: &AccountInfo) -> Result<Vec<QueueEntry>, ProgramError> {
        if account.data_len() == 0 {
            return Ok(Vec::new());
        }
        
        let storage = Self::try_from_slice(&account.data.borrow())
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        Ok(storage.entries)
    }
    
    /// Save entries to account
    pub fn save_entries(
        account: &AccountInfo,
        entries: &[QueueEntry],
        queue_id: u128,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        let mut storage = Self {
            queue_id,
            entries: entries.to_vec(),
            total_entries: entries.len() as u32,
            last_update_slot: current_slot,
        };
        
        // Ensure we don't exceed max size
        if storage.entries.len() > MAX_QUEUE_SIZE {
            storage.entries.truncate(MAX_QUEUE_SIZE);
            storage.total_entries = MAX_QUEUE_SIZE as u32;
        }
        
        let data = storage.try_to_vec()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if data.len() > account.data_len() {
            msg!("Storage data too large: {} > {}", data.len(), account.data_len());
            return Err(ProgramError::AccountDataTooSmall);
        }
        
        let mut account_data = account.data.borrow_mut();
        account_data[..data.len()].copy_from_slice(&data);
        
        Ok(())
    }
    
    /// Add entry to storage
    pub fn add_entry(&mut self, entry: QueueEntry) -> Result<(), ProgramError> {
        if self.entries.len() >= MAX_QUEUE_SIZE {
            return Err(BettingPlatformError::QueueFull.into());
        }
        
        self.entries.push(entry);
        self.total_entries += 1;
        
        Ok(())
    }
    
    /// Remove entry by ID
    pub fn remove_entry(&mut self, entry_id: u128) -> Result<(), ProgramError> {
        self.entries.retain(|e| e.entry_id != entry_id);
        self.total_entries = self.entries.len() as u32;
        Ok(())
    }
    
    /// Clean expired entries
    pub fn clean_expired_entries(&mut self, current_slot: u64, max_age: u64) -> u32 {
        let initial_count = self.entries.len();
        
        self.entries.retain(|entry| {
            // Keep if not expired
            match entry.status {
                EntryStatus::Expired => false,
                EntryStatus::Executed => false,
                EntryStatus::Cancelled => false,
                EntryStatus::Processing => false, // Processing entries are kept
                EntryStatus::Pending => {
                    // Check if too old
                    current_slot.saturating_sub(entry.submission_slot) <= max_age
                }
            }
        });
        
        self.total_entries = self.entries.len() as u32;
        (initial_count - self.entries.len()) as u32
    }
    
    /// Get entries sorted by priority
    pub fn get_sorted_entries(&self) -> Vec<QueueEntry> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| {
            // Higher priority first
            b.priority_score.cmp(&a.priority_score)
                .then_with(|| {
                    // Earlier submission first for same priority
                    a.submission_slot.cmp(&b.submission_slot)
                })
        });
        sorted
    }
    
    /// Update entry status
    pub fn update_entry_status(
        &mut self,
        entry_id: u128,
        new_status: EntryStatus,
    ) -> Result<(), ProgramError> {
        let entry = self.entries.iter_mut()
            .find(|e| e.entry_id == entry_id)
            .ok_or(BettingPlatformError::EntryNotFound)?;
        
        entry.status = new_status;
        Ok(())
    }
}

/// Load all queue entries from the entries storage account
pub fn load_all_queue_entries(
    entries_storage_account: &AccountInfo,
) -> Result<Vec<QueueEntry>, ProgramError> {
    QueueEntriesStorage::load_entries(entries_storage_account)
}

/// Save updated entries to the entries storage account
pub fn save_updated_entries(
    entries_storage_account: &AccountInfo,
    entries: &[QueueEntry],
    queue_id: u128,
    current_slot: u64,
) -> Result<(), ProgramError> {
    QueueEntriesStorage::save_entries(
        entries_storage_account,
        entries,
        queue_id,
        current_slot,
    )
}

/// Load and clean expired entries
pub fn load_and_clean_expired_entries(
    entries_storage_account: &AccountInfo,
    queue_id: u128,
    current_slot: u64,
    max_age: u64,
) -> Result<(Vec<QueueEntry>, u32), ProgramError> {
    let mut storage = if entries_storage_account.data_len() > 0 {
        QueueEntriesStorage::try_from_slice(&entries_storage_account.data.borrow())
            .map_err(|_| ProgramError::InvalidAccountData)?
    } else {
        QueueEntriesStorage::new(queue_id, current_slot)
    };
    
    // Clean expired entries
    let removed_count = storage.clean_expired_entries(current_slot, max_age);
    
    // Save back if any were removed
    if removed_count > 0 {
        storage.last_update_slot = current_slot;
        let data = storage.try_to_vec()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if data.len() <= entries_storage_account.data_len() {
            let mut account_data = entries_storage_account.data.borrow_mut();
            account_data[..data.len()].copy_from_slice(&data);
        }
    }
    
    Ok((storage.entries, removed_count))
}

/// Load ordering state from account
pub fn load_ordering_state(
    ordering_state_account: &AccountInfo,
) -> Result<super::OrderingState, ProgramError> {
    use solana_program::program_pack::Pack;
    
    if ordering_state_account.data_len() > 0 {
        super::OrderingState::unpack(&ordering_state_account.data.borrow())
    } else {
        Ok(super::OrderingState::new())
    }
}

/// Save reordered entries after fair ordering
pub fn save_reordered_entries(
    entries_storage_account: &AccountInfo,
    entries: &[QueueEntry],
    queue_id: u128,
    current_slot: u64,
) -> Result<(), ProgramError> {
    save_updated_entries(entries_storage_account, entries, queue_id, current_slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::priority::TradeData;
    use crate::math::fixed_point::U64F64;
    
    #[test]
    fn test_queue_storage() {
        let queue_id = 1u128;
        let current_slot = 1000u64;
        
        let mut storage = QueueEntriesStorage::new(queue_id, current_slot);
        
        // Add test entry
        let entry = QueueEntry {
            entry_id: 1,
            user: Pubkey::default(),
            priority_score: 100,
            submission_slot: 950,
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: true,
                amount: 1000,
                leverage: U64F64::from_num(2),
                max_slippage: U64F64::from_num(1),
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 1000,
            depth_boost: 0,
            bump: 0,
        };
        
        storage.add_entry(entry.clone()).unwrap();
        assert_eq!(storage.entries.len(), 1);
        assert_eq!(storage.total_entries, 1);
        
        // Test sorting
        let mut entry2 = entry.clone();
        entry2.entry_id = 2;
        entry2.priority_score = 200;
        storage.add_entry(entry2).unwrap();
        
        let sorted = storage.get_sorted_entries();
        assert_eq!(sorted[0].entry_id, 2); // Higher priority first
        assert_eq!(sorted[1].entry_id, 1);
    }
    
    #[test]
    fn test_expired_entry_cleanup() {
        let mut storage = QueueEntriesStorage::new(1, 1000);
        
        // Add old entry
        let old_entry = QueueEntry {
            entry_id: 1,
            user: Pubkey::default(),
            priority_score: 100,
            submission_slot: 100, // Very old
            submission_timestamp: 0,
            trade_data: TradeData {
                synthetic_id: 1,
                is_buy: true,
                amount: 1000,
                leverage: U64F64::from_num(2),
                max_slippage: U64F64::from_num(1),
                stop_loss: None,
                take_profit: None,
            },
            status: EntryStatus::Pending,
            stake_snapshot: 1000,
            depth_boost: 0,
            bump: 0,
        };
        
        storage.add_entry(old_entry).unwrap();
        
        // Clean with max age of 500 slots
        let removed = storage.clean_expired_entries(1000, 500);
        assert_eq!(removed, 1);
        assert_eq!(storage.entries.len(), 0);
    }
}