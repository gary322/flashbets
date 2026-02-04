use anchor_lang::prelude::*;
use fixed::types::U64F64;

#[account]
pub struct UserPosition {
    /// User who owns this position
    pub owner: Pubkey,
    
    /// Market this position is in
    pub market: Pubkey,
    
    /// Position size (positive for long, negative for short)
    pub size: i64,
    
    /// Entry price
    pub entry_price: U64F64,
    
    /// Collateral deposited
    pub collateral: u64,
    
    /// Leverage used
    pub leverage: U64F64,
    
    /// Unrealized PnL
    pub unrealized_pnl: i64,
    
    /// Realized PnL
    pub realized_pnl: i64,
    
    /// Fees paid
    pub fees_paid: u64,
    
    /// MMT rewards earned
    pub mmt_rewards: u64,
    
    /// Open timestamp
    pub open_timestamp: i64,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Is liquidated
    pub is_liquidated: bool,
    
    /// Padding for future upgrades
    pub _padding: [u8; 128],
}

impl UserPosition {
    pub const LEN: usize = 8 + // discriminator
        32 + // owner
        32 + // market
        8 + // size
        8 + // entry_price
        8 + // collateral
        8 + // leverage
        8 + // unrealized_pnl
        8 + // realized_pnl
        8 + // fees_paid
        8 + // mmt_rewards
        8 + // open_timestamp
        8 + // last_update
        1 + // is_liquidated
        128; // padding
}