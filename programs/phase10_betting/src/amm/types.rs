use anchor_lang::prelude::*;
use crate::types::{U64F64, I64F64};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum AMMType {
    /// LMSR for binary markets (N=1)
    LMSR,
    
    /// PM-AMM for multi-outcome markets (2 <= N <= 64)
    PMAMM,
    
    /// L2 norm for continuous distributions
    L2Distribution,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MarketType {
    /// Simple yes/no binary
    Binary,
    
    /// Multiple discrete outcomes
    MultiOutcome { count: u8 },
    
    /// Continuous range (e.g., price, date)
    Continuous {
        min: I64F64,
        max: I64F64,
        precision: u8,
    },
    
    /// Hierarchical verse with children
    Verse { depth: u8 },
    
    /// Quantum with collapse mechanism
    Quantum { proposals: u8 },
}

#[account]
pub struct HybridAMMSelector {
    /// Market identifier
    pub market_id: [u8; 32],
    
    /// Selected AMM type
    pub amm_type: AMMType,
    
    /// Market type classification
    pub market_type: MarketType,
    
    /// Time to expiry in slots
    pub time_to_expiry: u64,
    
    /// Current slot
    pub current_slot: u64,
    
    /// Override flags for special conditions
    pub override_flags: AMMOverrideFlags,
    
    /// Performance metrics for adaptive selection
    pub performance_metrics: AMMPerformanceMetrics,
    
    /// Transition state if AMM type changes
    pub transition_state: Option<AMMTransition>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct AMMOverrideFlags {
    /// Force PM-AMM for time decay optimization
    pub force_time_decay: bool,
    
    /// Force L2 for complex distributions
    pub force_distribution: bool,
    
    /// Disable automatic switching
    pub lock_amm_type: bool,
    
    /// High volatility mode
    pub high_volatility: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct AMMPerformanceMetrics {
    /// Total volume through this AMM
    pub total_volume: u64,
    
    /// Average slippage in bps
    pub avg_slippage_bps: u16,
    
    /// Number of trades
    pub trade_count: u64,
    
    /// LVR accumulated
    pub total_lvr: U64F64,
    
    /// Efficiency score (0-100)
    pub efficiency_score: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct AMMTransition {
    /// Previous AMM type
    pub from_amm: AMMType,
    
    /// New AMM type
    pub to_amm: AMMType,
    
    /// Slot when transition started
    pub start_slot: u64,
    
    /// Expected completion slot
    pub end_slot: u64,
    
    /// Migration progress (0-100%)
    pub progress: u8,
}

impl HybridAMMSelector {
    pub const LEN: usize = 8 + // discriminator
        32 + // market_id
        1 + // amm_type
        17 + // market_type (worst case)
        8 + // time_to_expiry
        8 + // current_slot
        4 + // override_flags
        33 + // performance_metrics
        1 + 26; // optional transition_state
}