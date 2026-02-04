use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Priority queue for positions approaching liquidation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LiquidationQueue {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Is initialized
    pub is_initialized: bool,
    
    /// Authority
    pub authority: Pubkey,
    
    /// High priority positions (health ratio < 1.05)
    pub high_priority: Vec<LiquidationEntry>,
    
    /// Medium priority positions (health ratio < 1.1)
    pub medium_priority: Vec<LiquidationEntry>,
    
    /// Total positions in queue
    pub total_positions: u32,
    
    /// Last processed slot
    pub last_processed_slot: u64,
    
    /// Stats
    pub total_processed: u64,
    pub total_liquidated: u64,
}

/// Entry in liquidation queue
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LiquidationEntry {
    /// Position ID
    pub position_id: [u8; 32],
    
    /// Position health account
    pub position_health_account: Pubkey,
    
    /// Trader
    pub trader: Pubkey,
    
    /// Health ratio when added
    pub health_ratio: u64,
    
    /// Effective leverage
    pub effective_leverage: u64,
    
    /// Added to queue at slot
    pub added_slot: u64,
    
    /// Added timestamp
    pub added_timestamp: i64,
    
    /// Priority score (lower = higher priority)
    pub priority_score: u64,
}

impl LiquidationQueue {
    pub const DISCRIMINATOR: [u8; 8] = [76, 73, 81, 95, 81, 85, 69, 85]; // "LIQ_QUEU"
    
    pub const LEN: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // authority
        4 + (50 * 128) + // high_priority vec (max 50 entries)
        4 + (100 * 128) + // medium_priority vec (max 100 entries)
        4 + // total_positions
        8 + // last_processed_slot
        8 + // total_processed
        8 + // total_liquidated
        256; // padding
    
    pub const MAX_HIGH_PRIORITY: usize = 50;
    pub const MAX_MEDIUM_PRIORITY: usize = 100;
    
    /// Create new liquidation queue
    pub fn new(authority: Pubkey) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            authority,
            high_priority: Vec::new(),
            medium_priority: Vec::new(),
            total_positions: 0,
            last_processed_slot: 0,
            total_processed: 0,
            total_liquidated: 0,
        }
    }
    
    /// Add position to high priority queue
    pub fn add_high_priority(
        &mut self,
        position_id: [u8; 32],
        position_health_account: Pubkey,
        trader: Pubkey,
        health_ratio: u64,
        effective_leverage: u64,
        slot: u64,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        if self.high_priority.len() >= Self::MAX_HIGH_PRIORITY {
            // Remove lowest priority entry if full
            self.remove_lowest_priority_high();
        }
        
        let priority_score = Self::calculate_priority_score(health_ratio, effective_leverage);
        
        let entry = LiquidationEntry {
            position_id,
            position_health_account,
            trader,
            health_ratio,
            effective_leverage,
            added_slot: slot,
            added_timestamp: timestamp,
            priority_score,
        };
        
        // Insert sorted by priority
        let insert_pos = self.high_priority
            .iter()
            .position(|e| e.priority_score > priority_score)
            .unwrap_or(self.high_priority.len());
        
        self.high_priority.insert(insert_pos, entry);
        self.total_positions += 1;
        
        Ok(())
    }
    
    /// Add position to medium priority queue
    pub fn add_medium_priority(
        &mut self,
        position_id: [u8; 32],
        position_health_account: Pubkey,
        trader: Pubkey,
        health_ratio: u64,
        effective_leverage: u64,
        slot: u64,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        if self.medium_priority.len() >= Self::MAX_MEDIUM_PRIORITY {
            // Remove lowest priority entry if full
            self.remove_lowest_priority_medium();
        }
        
        let priority_score = Self::calculate_priority_score(health_ratio, effective_leverage);
        
        let entry = LiquidationEntry {
            position_id,
            position_health_account,
            trader,
            health_ratio,
            effective_leverage,
            added_slot: slot,
            added_timestamp: timestamp,
            priority_score,
        };
        
        // Insert sorted by priority
        let insert_pos = self.medium_priority
            .iter()
            .position(|e| e.priority_score > priority_score)
            .unwrap_or(self.medium_priority.len());
        
        self.medium_priority.insert(insert_pos, entry);
        self.total_positions += 1;
        
        Ok(())
    }
    
    /// Calculate priority score (lower = higher priority)
    pub fn calculate_priority_score(health_ratio: u64, effective_leverage: u64) -> u64 {
        // Priority = health_ratio / effective_leverage
        // Lower health ratio and higher leverage = higher priority
        if effective_leverage == 0 {
            u64::MAX
        } else {
            (health_ratio as u128 * 1_000_000 / effective_leverage as u128) as u64
        }
    }
    
    /// Get next position to process
    pub fn get_next_position(&mut self) -> Option<LiquidationEntry> {
        // Always process high priority first
        if !self.high_priority.is_empty() {
            self.total_positions = self.total_positions.saturating_sub(1);
            return Some(self.high_priority.remove(0));
        }
        
        // Then process medium priority
        if !self.medium_priority.is_empty() {
            self.total_positions = self.total_positions.saturating_sub(1);
            return Some(self.medium_priority.remove(0));
        }
        
        None
    }
    
    /// Remove position from queue
    pub fn remove_position(&mut self, position_id: &[u8; 32]) -> bool {
        // Check high priority
        if let Some(pos) = self.high_priority.iter().position(|e| &e.position_id == position_id) {
            self.high_priority.remove(pos);
            self.total_positions = self.total_positions.saturating_sub(1);
            return true;
        }
        
        // Check medium priority
        if let Some(pos) = self.medium_priority.iter().position(|e| &e.position_id == position_id) {
            self.medium_priority.remove(pos);
            self.total_positions = self.total_positions.saturating_sub(1);
            return true;
        }
        
        false
    }
    
    /// Check if position is in queue
    pub fn contains_position(&self, position_id: &[u8; 32]) -> bool {
        self.high_priority.iter().any(|e| &e.position_id == position_id) ||
        self.medium_priority.iter().any(|e| &e.position_id == position_id)
    }
    
    /// Remove lowest priority from high priority queue
    fn remove_lowest_priority_high(&mut self) {
        if !self.high_priority.is_empty() {
            self.high_priority.pop();
            self.total_positions = self.total_positions.saturating_sub(1);
        }
    }
    
    /// Remove lowest priority from medium priority queue
    fn remove_lowest_priority_medium(&mut self) {
        if !self.medium_priority.is_empty() {
            self.medium_priority.pop();
            self.total_positions = self.total_positions.saturating_sub(1);
        }
    }
    
    /// Update stats after processing
    pub fn update_stats(&mut self, slot: u64, was_liquidated: bool) {
        self.last_processed_slot = slot;
        self.total_processed += 1;
        
        if was_liquidated {
            self.total_liquidated += 1;
        }
    }
}