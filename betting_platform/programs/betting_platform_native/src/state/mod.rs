//! State module containing all account structures
//!
//! Complete migration of 31 account types from Anchor to native Solana

pub mod accounts;
pub mod amm_accounts;
pub mod chain_accounts;
pub mod keeper_accounts;
pub mod order_accounts;
pub mod security_accounts;
pub mod resolution_accounts;
pub mod pda_size_validation;
pub mod validation;
pub mod quantum_accounts;
pub mod l2_distribution_state;
pub mod rollback_protection;
pub mod versioned_accounts;
pub mod migration_framework;
pub mod fused_migration;

// Re-export all account types
pub use accounts::*;
pub use amm_accounts::*;
pub use chain_accounts::*;
pub use keeper_accounts::*;
pub use order_accounts::*;
pub use security_accounts::*;
// Removed resolution_accounts::* to avoid duplicate discriminators
pub use resolution_accounts::{ResolutionState, DisputeState};
pub use pda_size_validation::*;
pub use validation::*;
pub use quantum_accounts::*;
pub use l2_distribution_state::L2DistributionState;
pub use versioned_accounts::{VersionedGlobalConfigPDA, VersionedVersePDA, VersionedProposalPDA, VersionedPosition, Versioned, CURRENT_VERSION};
pub use migration_framework::{MigrationManager, MigrationManagerState, MigrationStrategy};
pub use fused_migration::{FusedMigrationFlags, MigrationStats, FUSED_MIGRATION};

use borsh::{BorshDeserialize, BorshSerialize};

// Type aliases for backwards compatibility
pub type GlobalState = GlobalConfigPDA;
pub type VerseState = VersePDA;
pub type PositionPDA = Position;

/// Collateral vault for storing USDC deposits
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CollateralVault {
    pub total_deposits: u64,
    pub total_borrowed: u64,
    pub depositor_count: u32,
    pub last_update: i64,
}

/// Funding state for tracking rewards and incentives
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Default)]
pub struct FundingState {
    pub total_funding: u64,
    pub funding_rate: u64,
    pub last_funding_time: i64,
    pub cumulative_funding: u64,
}

impl CollateralVault {
    pub const SIZE: usize = 8 + 8 + 4 + 8; // u64 + u64 + u32 + i64
}

// Constants for account sizes
pub mod sizes {
    pub const DISCRIMINATOR: usize = 8;
    pub const PUBKEY: usize = 32;
    pub const U64: usize = 8;
    pub const U128: usize = 16;
    pub const I64: usize = 8;
    pub const U32: usize = 4;
    pub const U16: usize = 2;
    pub const U8: usize = 1;
    pub const BOOL: usize = 1;
    
    // Account-specific sizes
    pub const GLOBAL_CONFIG: usize = DISCRIMINATOR + 
        U64 +       // epoch
        U64 +       // season
        U128 +      // vault
        U128 +      // total_oi
        U128 +      // coverage
        U32 +       // fee_base
        U32 +       // fee_slope
        BOOL +      // halt_flag
        U64 +       // genesis_slot
        U64 +       // season_start_slot
        U64 +       // season_end_slot
        U64 +       // mmt_total_supply
        U64 +       // mmt_current_season
        U64 +       // mmt_emission_rate
        (7 * (U32 + U8)) + // leverage_tiers (7 tiers)
        32;         // padding
    
    pub const VERSE_PDA: usize = DISCRIMINATOR +
        16 +        // verse_id (u128)
        17 +        // parent_id (Option<u128>)
        32 +        // children_root
        U16 +       // child_count
        U32 +       // total_descendants
        U8 +        // status
        U8 +        // depth
        U64 +       // last_update_slot
        U64 +       // total_oi
        U64 +       // derived_prob (U64F64)
        U64 +       // correlation_factor (U64F64)
        U8;         // bump
    
    pub const PROPOSAL_PDA: usize = DISCRIMINATOR +
        32 +        // proposal_id
        32 +        // verse_id
        32 +        // market_id
        U8 +        // amm_type
        U8 +        // outcomes count
        (64 * 8) +  // prices (max 64 outcomes)
        (64 * 8) +  // volumes
        U64 +       // liquidity_depth
        U8 +        // state
        U64 +       // settle_slot
        17 +        // resolution (Option<Resolution>)
        U64 +       // partial_liq_accumulator
        4 + (10 * 64) + // chain_positions Vec (10 positions max, 64 bytes each)
        64;         // padding
    
    pub const POSITION: usize = DISCRIMINATOR +
        U128 +      // proposal_id
        U8 +        // outcome
        U64 +       // size
        U64 +       // leverage
        U64 +       // entry_price
        U64 +       // liquidation_price
        BOOL +      // is_long
        I64;        // created_at
    
    pub const USER_MAP: usize = DISCRIMINATOR +
        PUBKEY +    // user
        U32 +       // position_count
        (32 * U128); // position_ids (max 32)
}