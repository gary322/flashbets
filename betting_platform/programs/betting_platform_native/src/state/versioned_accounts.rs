//! Versioned Account Structures
//!
//! Adds version fields to all PDAs for future upgradability
//! This ensures smooth migrations and backward compatibility

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::{
    account_validation::DISCRIMINATOR_SIZE,
    error::BettingPlatformError,
    state::accounts::discriminators,
};

/// Current version for all account structures
pub const CURRENT_VERSION: u32 = 1;

/// Version info trait for all PDAs
pub trait Versioned {
    fn get_version(&self) -> u32;
    fn set_version(&mut self, version: u32);
    fn is_current_version(&self) -> bool {
        self.get_version() == CURRENT_VERSION
    }
    fn needs_migration(&self) -> bool {
        self.get_version() < CURRENT_VERSION
    }
}

/// Versioned Global Configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct VersionedGlobalConfigPDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Migration state
    pub migration_state: MigrationState,
    
    /// Current epoch number
    pub epoch: u64,
    
    /// Current season number
    pub season: u64,
    
    /// Total vault balance
    pub vault: u128,
    
    /// Total open interest
    pub total_oi: u128,
    
    /// Coverage ratio
    pub coverage: u128,
    
    /// Base fee in basis points
    pub fee_base: u32,
    
    /// Fee slope for dynamic pricing
    pub fee_slope: u32,
    
    /// Global halt flag
    pub halt_flag: bool,
    
    /// Genesis slot
    pub genesis_slot: u64,
    
    /// Season start slot
    pub season_start_slot: u64,
    
    /// Season end slot
    pub season_end_slot: u64,
    
    /// MMT total supply
    pub mmt_total_supply: u64,
    
    /// MMT current season distribution
    pub mmt_current_season: u64,
    
    /// MMT emission rate
    pub mmt_emission_rate: u64,
    
    /// Leverage tiers (max 7)
    pub leverage_tiers: Vec<(u32, u8)>,
    
    /// Update authority
    pub update_authority: Pubkey,
    
    /// Reserved for future use
    pub reserved: [u8; 32],
}

impl Versioned for VersionedGlobalConfigPDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// Versioned Verse PDA
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct VersionedVersePDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Unique verse identifier
    pub verse_id: u128,
    
    /// Parent verse ID (None for root verses)
    pub parent_id: Option<u128>,
    
    /// Merkle root of child verses
    pub children_root: [u8; 32],
    
    /// Number of direct children
    pub child_count: u16,
    
    /// Total descendants count
    pub total_descendants: u32,
    
    /// Verse status
    pub status: VerseStatus,
    
    /// Depth in hierarchy (0 for root)
    pub depth: u8,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Total open interest in this verse
    pub total_oi: u64,
    
    /// Derived probability
    pub derived_prob: u64,
    
    /// Correlation factor
    pub correlation_factor: u64,
    
    /// Bump seed for PDA
    pub bump: u8,
    
    /// Reserved for future use
    pub reserved: [u8; 16],
}

impl Versioned for VersionedVersePDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// Versioned Proposal PDA
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct VersionedProposalPDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Unique proposal identifier
    pub proposal_id: [u8; 32],
    
    /// Verse this proposal belongs to
    pub verse_id: [u8; 32],
    
    /// Market identifier
    pub market_id: [u8; 32],
    
    /// AMM type
    pub amm_type: u8,
    
    /// Number of outcomes
    pub outcomes: u8,
    
    /// Current prices for each outcome
    pub prices: Vec<u64>,
    
    /// Volume for each outcome
    pub volumes: Vec<u64>,
    
    /// Liquidity depth
    pub liquidity_depth: u64,
    
    /// Proposal state
    pub state: u8,
    
    /// Settlement slot
    pub settle_slot: u64,
    
    /// Resolution data
    pub resolution: Option<Resolution>,
    
    /// Partial liquidation accumulator
    pub partial_liq_accumulator: u64,
    
    /// Chain positions
    pub chain_positions: Vec<ChainPositionData>,
    
    /// Reserved for future use
    pub reserved: [u8; 64],
}

impl Versioned for VersionedProposalPDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// Versioned Position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct VersionedPosition {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Position owner
    pub user: Pubkey,
    
    /// Proposal ID
    pub proposal_id: u128,
    
    /// Outcome index
    pub outcome: u8,
    
    /// Position size
    pub size: u64,
    
    /// Notional value
    pub notional: u64,
    
    /// Leverage used
    pub leverage: u64,
    
    /// Entry price
    pub entry_price: u64,
    
    /// Liquidation price
    pub liquidation_price: u64,
    
    /// Is long position
    pub is_long: bool,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Position state
    pub state: PositionState,
    
    /// Partial liquidation count
    pub partial_liq_count: u8,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Reserved for future use
    pub reserved: [u8; 32],
}

impl Versioned for VersionedPosition {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// Migration state for accounts
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MigrationState {
    /// Account is at current version
    Current,
    /// Migration in progress
    Migrating,
    /// Migration completed, waiting for verification
    PendingVerification,
    /// Migration failed, needs manual intervention
    Failed,
}

/// Verse status enum
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum VerseStatus {
    Active,
    Paused,
    Archived,
    Migrating,
}

/// Position state enum
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum PositionState {
    Open,
    PartiallyLiquidated,
    FullyLiquidated,
    Closed,
    Migrating,
}

/// Resolution data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct Resolution {
    pub winning_outcome: u8,
    pub resolved_at: i64,
    pub resolver: Pubkey,
    pub verification_hash: [u8; 32],
}

/// Chain position data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ChainPositionData {
    pub chain_id: u128,
    pub position_id: u128,
    pub allocation_bps: u16,
}

/// Migration functions
impl VersionedGlobalConfigPDA {
    pub fn migrate_from_v0(&mut self, old_data: &[u8]) -> Result<(), ProgramError> {
        // Deserialize old format
        // Apply transformations
        // Update version
        self.version = CURRENT_VERSION;
        self.migration_state = MigrationState::Current;
        Ok(())
    }
    
    pub fn validate_migration(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::GLOBAL_CONFIG {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.version > CURRENT_VERSION {
            return Err(BettingPlatformError::InvalidStateVersion.into());
        }
        
        Ok(())
    }
}

/// Helper to check if account needs migration
pub fn check_account_version(data: &[u8]) -> Result<u32, ProgramError> {
    if data.len() < DISCRIMINATOR_SIZE + 4 {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Version is always after discriminator
    let version_bytes = &data[DISCRIMINATOR_SIZE..DISCRIMINATOR_SIZE + 4];
    let version = u32::from_le_bytes([
        version_bytes[0],
        version_bytes[1],
        version_bytes[2],
        version_bytes[3],
    ]);
    
    Ok(version)
}

/// Batch migration support
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MigrationBatch {
    pub accounts: Vec<Pubkey>,
    pub from_version: u32,
    pub to_version: u32,
    pub started_at: i64,
    pub completed_count: u32,
    pub failed_count: u32,
    pub status: MigrationBatchStatus,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MigrationBatchStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_traits() {
        let mut config = VersionedGlobalConfigPDA {
            discriminator: discriminators::GLOBAL_CONFIG,
            version: 0,
            migration_state: MigrationState::Current,
            // ... other fields with defaults
            epoch: 0,
            season: 0,
            vault: 0,
            total_oi: 0,
            coverage: 0,
            fee_base: 0,
            fee_slope: 0,
            halt_flag: false,
            genesis_slot: 0,
            season_start_slot: 0,
            season_end_slot: 0,
            mmt_total_supply: 0,
            mmt_current_season: 0,
            mmt_emission_rate: 0,
            leverage_tiers: vec![],
            update_authority: Pubkey::default(),
            reserved: [0; 32],
        };
        
        assert!(!config.is_current_version());
        assert!(config.needs_migration());
        
        config.set_version(CURRENT_VERSION);
        assert!(config.is_current_version());
        assert!(!config.needs_migration());
    }
    
    #[test]
    fn test_version_check() {
        let mut data = vec![0u8; DISCRIMINATOR_SIZE + 4];
        
        // Set version to 1
        data[DISCRIMINATOR_SIZE] = 1;
        data[DISCRIMINATOR_SIZE + 1] = 0;
        data[DISCRIMINATOR_SIZE + 2] = 0;
        data[DISCRIMINATOR_SIZE + 3] = 0;
        
        let version = check_account_version(&data).unwrap();
        assert_eq!(version, 1);
    }
}