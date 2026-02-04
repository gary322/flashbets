use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use super::{QuantumState, CollapseTrigger};

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct QuantumFlash {
    pub position_id: u128,           // Unique position ID (16 bytes)
    pub verse_id: u128,              // Associated flash verse (16 bytes)
    pub owner: Pubkey,               // Position owner (32 bytes)
    pub states: Vec<QuantumState>,  // Quantum superposition states (variable)
    pub leverage: u8,                // Leverage multiplier (1 byte)
    pub base_amount: u64,            // Base bet amount (8 bytes)
    pub total_exposure: u64,         // Total leveraged exposure (8 bytes)
    pub is_collapsed: bool,          // Collapse status (1 byte)
    pub collapsed_outcome: Option<u8>, // Collapsed to which outcome (2 bytes)
    pub payout: u64,                 // Final payout amount (8 bytes)
    pub collapse_trigger: CollapseTrigger, // Trigger mechanism (variable)
}

impl QuantumFlash {
    pub const BASE_SIZE: usize = 8 + // discriminator
        16 + // position_id
        16 + // verse_id
        32 + // owner
        4 + // states vec prefix
        1 + // leverage
        8 + // base_amount
        8 + // total_exposure
        1 + // is_collapsed
        1 + 1 + // collapsed_outcome option
        8 + // payout
        1 + 16; // collapse_trigger (enum + data)
    
    pub const STATE_SIZE: usize = 4 + 32 + // outcome string
        8 + // probability
        8 + // amplitude
        8; // phase
    
    pub fn space(max_states: usize) -> usize {
        Self::BASE_SIZE + (Self::STATE_SIZE * max_states)
    }
}

impl Default for QuantumFlash {
    fn default() -> Self {
        Self {
            position_id: 0,
            verse_id: 0,
            owner: Pubkey::default(),
            states: Vec::new(),
            leverage: 1,
            base_amount: 0,
            total_exposure: 0,
            is_collapsed: false,
            collapsed_outcome: None,
            payout: 0,
            collapse_trigger: CollapseTrigger::TimeExpiry { slot: 0 },
        }
    }
}