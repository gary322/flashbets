//! Verse Module
//!
//! Handles market classification and hierarchy management

pub mod enhanced_classifier;
pub mod hierarchy_manager;
pub mod dynamic_rebalancer;
pub mod fee_discount;

pub use enhanced_classifier::*;
pub use hierarchy_manager::*;
pub use dynamic_rebalancer::*;
pub use fee_discount::*;

// Re-export verse types from state
pub use crate::state::accounts::VersePDA;

// Define VerseType enum with required variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum VerseType {
    Main,
    Quantum,
    Distribution,
    Root,
    Category,
    SubCategory,
    Market,
}

// Define VerseAccount structure for cross_verse compatibility
#[derive(Debug, Clone, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct VerseAccount {
    pub verse_id: u32,
    pub parent_verse: solana_program::pubkey::Pubkey,
    pub verse_type: VerseType,
    pub keywords: Vec<String>,
    pub total_markets: u32,
    pub active_markets: u32,
    pub total_volume: u64,
    pub created_at: i64,
    pub authority: solana_program::pubkey::Pubkey,
}