use anchor_lang::prelude::*;

#[account]
pub struct Verse {
    pub verse_id: [u8; 32],
    pub authority: Pubkey,
    pub name: String,
    pub status: VerseStatus,
    pub created_at: i64,
    pub total_volume: u64,
    pub total_oi: u64,
}

impl Verse {
    pub const LEN: usize = 8 + // discriminator
        32 + // verse_id
        32 + // authority
        4 + 32 + // name (string with max 32 chars)
        1 + // status
        8 + // created_at
        8 + // total_volume
        8; // total_oi
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum VerseStatus {
    Active,
    Inactive,
    Halted,
    Resolved,
}