use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VerseMetadata {
    pub is_initialized: bool,
    pub verse_id: [u8; 16],  // u128 as byte array
    pub title: String,
    pub normalized_title: String,
    pub keywords: Vec<String>,
    pub category: String,
    pub parent_verse: Option<[u8; 16]>,
    pub child_verses: Vec<[u8; 16]>,
    pub market_count: u32,
    pub total_volume: u64,
    pub average_probability: u64,  // Fixed point representation
    pub last_updated: i64,
    pub bump: u8,
}

impl VerseMetadata {
    pub const BASE_LEN: usize = 1 + 16 + 4 + 4 + 4 + 4 + 1 + 16 + 4 + 4 + 8 + 8 + 8 + 1;
    
    pub fn calculate_len(
        title_len: usize,
        normalized_title_len: usize,
        keywords: &[String],
        category_len: usize,
        child_count: usize,
    ) -> usize {
        Self::BASE_LEN 
            + title_len 
            + normalized_title_len
            + keywords.iter().map(|k| 4 + k.len()).sum::<usize>()
            + category_len
            + (child_count * 16)
    }
    
    pub fn new(
        verse_id: [u8; 16],
        title: String,
        normalized_title: String,
        keywords: Vec<String>,
        category: String,
        bump: u8,
    ) -> Self {
        Self {
            is_initialized: true,
            verse_id,
            title,
            normalized_title,
            keywords,
            category,
            parent_verse: None,
            child_verses: Vec::new(),
            market_count: 0,
            total_volume: 0,
            average_probability: 0,
            last_updated: 0,
            bump,
        }
    }
    
    pub fn add_child(&mut self, child_id: [u8; 16]) -> Result<(), ProgramError> {
        if self.child_verses.contains(&child_id) {
            return Err(ProgramError::InvalidArgument);
        }
        self.child_verses.push(child_id);
        Ok(())
    }
    
    pub fn set_parent(&mut self, parent_id: [u8; 16]) {
        self.parent_verse = Some(parent_id);
    }
    
    pub fn update_stats(&mut self, volume: u64, probability: u64, timestamp: i64) {
        self.market_count += 1;
        self.total_volume = self.total_volume.saturating_add(volume);
        
        // Update average probability
        let new_avg = if self.market_count == 1 {
            probability
        } else {
            let total = self.average_probability
                .saturating_mul(self.market_count as u64 - 1)
                .saturating_add(probability);
            total / self.market_count as u64
        };
        
        self.average_probability = new_avg;
        self.last_updated = timestamp;
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum MarketStatus {
    Active,
    Resolved,
    Cancelled,
    Paused,
}