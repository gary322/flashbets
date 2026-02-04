use anchor_lang::prelude::*;

// Re-export structures from account_structs
pub use crate::account_structs::{
    VersePDA, VerseStatus, ProposalPDA, ProposalState, AMMType, 
    Outcome, Resolution, ChainPosition, MapEntryPDA, GlobalConfigPDA,
    LeverageTier, U64F64, U128F128
};

// Additional state structures specific to state module can go here

// Export Verse struct (different from VersePDA - this appears to be for verse creation)
#[account]
pub struct Verse {
    pub verse_id: [u8; 32],
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub created_at: i64,
    pub total_volume: u64,
    pub total_oi: u64,
}

impl Verse {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8;
}

#[account]
pub struct ResolutionPDA {
    pub verse_id: u128,
    pub market_id: String,
    pub resolution: String,
    pub resolved_at: i64,
    pub resolver: Pubkey,
    pub is_disputed: bool,
    pub dispute_deadline: i64,
}

impl ResolutionPDA {
    pub const LEN: usize = 8 + // discriminator
        16 + // verse_id
        4 + 64 + // market_id (string length + max string)
        4 + 32 + // resolution (string length + max string)
        8 + // resolved_at
        32 + // resolver
        1 + // is_disputed
        8; // dispute_deadline
}