use anchor_lang::prelude::*;
use fixed::types::U64F64;

#[account]
pub struct GlobalState {
    /// Authority that can update the state
    pub authority: Pubkey,
    
    /// Total open interest across all markets
    pub total_open_interest: u64,
    
    /// Bootstrap phase active flag
    pub bootstrap_active: bool,
    
    /// Bootstrap start slot
    pub bootstrap_start_slot: u64,
    
    /// Protocol fee receiver
    pub fee_receiver: Pubkey,
    
    /// Vault pubkey
    pub vault: Pubkey,
    
    /// MMT mint pubkey
    pub mmt_mint: Pubkey,
    
    /// MMT treasury
    pub mmt_treasury: Pubkey,
    
    /// Total protocol revenue
    pub total_revenue: u64,
    
    /// Total MMT distributed
    pub total_mmt_distributed: u64,
    
    /// Current season
    pub current_season: u64,
    
    /// Season MMT allocation (10M per season)
    pub season_mmt_allocation: u64,
    
    /// Coverage ratio target
    pub coverage_target: U64F64,
    
    /// Emergency pause
    pub emergency_pause: bool,
    
    /// Padding for future upgrades
    pub _padding: [u8; 256],
}

impl GlobalState {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        8 + // total_open_interest
        1 + // bootstrap_active
        8 + // bootstrap_start_slot
        32 + // fee_receiver
        32 + // vault
        32 + // mmt_mint
        32 + // mmt_treasury
        8 + // total_revenue
        8 + // total_mmt_distributed
        8 + // current_season
        8 + // season_mmt_allocation
        8 + // coverage_target
        1 + // emergency_pause
        256; // padding
}