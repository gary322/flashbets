use borsh::{BorshDeserialize, BorshSerialize};
use super::Outcome;

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FlashVerse {
    pub id: u128,                    // Unique identifier (16 bytes)
    pub parent_id: u128,             // Link to main verse (16 bytes)
    pub title: String,               // Market title (32 bytes max)
    pub sport_type: u8,              // 1=Soccer, 2=Basketball, etc (1 byte)
    pub tau: f64,                    // Micro-tau value (8 bytes)
    pub time_left: u64,              // Seconds to resolution (8 bytes)
    pub settle_slot: u64,            // Deadline slot (8 bytes)
    pub outcomes: Vec<Outcome>,      // Dynamic outcomes (variable)
    pub total_volume: u64,           // Total bet volume (8 bytes)
    pub leverage_mult: u16,          // Leverage multiplier (2 bytes) - changed from u8
    pub max_leverage: u16,           // Max leverage for duration (2 bytes)
    pub is_resolved: bool,           // Resolution status (1 byte)
    pub winning_outcome: Option<u8>, // Winning outcome index (2 bytes)
    pub proof_hash: [u8; 32],       // ZK proof hash (32 bytes)
    pub leverage_system: Option<super::LeverageSystem>, // Which leverage system was used
}

impl FlashVerse {
    pub const BASE_SIZE: usize = 8 + // discriminator
        16 + // id
        16 + // parent_id
        4 + 32 + // title (string prefix + max content)
        1 + // sport_type
        8 + // tau
        8 + // time_left
        8 + // settle_slot
        4 + // outcomes vec prefix
        8 + // total_volume
        2 + // leverage_mult (changed to u16)
        2 + // max_leverage
        1 + // is_resolved
        1 + 1 + // winning_outcome option
        32 + // proof_hash
        1 + 1; // leverage_system option
    
    pub const OUTCOME_SIZE: usize = 4 + 32 + // name string
        8 + // probability
        8 + // volume
        8; // odds
    
    pub fn space(max_outcomes: usize) -> usize {
        Self::BASE_SIZE + (Self::OUTCOME_SIZE * max_outcomes)
    }
}

impl Default for FlashVerse {
    fn default() -> Self {
        Self {
            id: 0,
            parent_id: 0,
            title: String::new(),
            sport_type: 0,
            tau: 0.0,
            time_left: 0,
            settle_slot: 0,
            outcomes: Vec::new(),
            total_volume: 0,
            leverage_mult: 1,
            max_leverage: 75,
            is_resolved: false,
            winning_outcome: None,
            proof_hash: [0u8; 32],
            leverage_system: None,
        }
    }
}