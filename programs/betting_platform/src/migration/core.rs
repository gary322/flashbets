// Migration Core Infrastructure
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    msg,
    program_pack::{Pack, Sealed},
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::U64F64;

// Account discriminators for different types
pub const MIGRATION_STATE_DISCRIMINATOR: [u8; 8] = [0x4D, 0x49, 0x47, 0x52, 0x41, 0x54, 0x45, 0x00];
pub const POSITION_SNAPSHOT_DISCRIMINATOR: [u8; 8] = [0x50, 0x4F, 0x53, 0x53, 0x4E, 0x41, 0x50, 0x00];
pub const VERSE_SNAPSHOT_DISCRIMINATOR: [u8; 8] = [0x56, 0x45, 0x52, 0x53, 0x4E, 0x41, 0x50, 0x00];

// Migration types
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MigrationType {
    CriticalBugFix,
    FeatureUpgrade,
    SolanaCompatibility,
    EmergencyMigration,
}

// Migration status
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MigrationStatus {
    Announced,      // Users notified
    Active,         // Migration in progress
    Finalizing,     // No new migrations, wrapping up
    Completed,      // All done
    Cancelled,      // If critical issue found
}

// Position side enum
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

// Chain step type for position chains
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum ChainStepType {
    Multiply,
    Add,
    Conditional,
}

/// Main migration state account
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MigrationState {
    pub discriminator: [u8; 8],
    pub old_program_id: Pubkey,
    pub new_program_id: Pubkey,
    pub migration_authority: Pubkey,
    pub start_slot: u64,
    pub end_slot: u64,
    pub total_accounts_to_migrate: u64,
    pub accounts_migrated: u64,
    pub migration_type: MigrationType,
    pub incentive_multiplier: u64, // Fixed point value stored as u64
    pub status: MigrationStatus,
    pub merkle_root: [u8; 32],
}

impl Sealed for MigrationState {}

impl Pack for MigrationState {
    const LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 1 + 8 + 1 + 32;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let mut offset = 0;
        
        // Discriminator
        dst[offset..offset + 8].copy_from_slice(&self.discriminator);
        offset += 8;
        
        // Program IDs
        dst[offset..offset + 32].copy_from_slice(self.old_program_id.as_ref());
        offset += 32;
        dst[offset..offset + 32].copy_from_slice(self.new_program_id.as_ref());
        offset += 32;
        
        // Authority
        dst[offset..offset + 32].copy_from_slice(self.migration_authority.as_ref());
        offset += 32;
        
        // Slots
        dst[offset..offset + 8].copy_from_slice(&self.start_slot.to_le_bytes());
        offset += 8;
        dst[offset..offset + 8].copy_from_slice(&self.end_slot.to_le_bytes());
        offset += 8;
        
        // Account counts
        dst[offset..offset + 8].copy_from_slice(&self.total_accounts_to_migrate.to_le_bytes());
        offset += 8;
        dst[offset..offset + 8].copy_from_slice(&self.accounts_migrated.to_le_bytes());
        offset += 8;
        
        // Migration type (1 byte)
        dst[offset] = match self.migration_type {
            MigrationType::CriticalBugFix => 0,
            MigrationType::FeatureUpgrade => 1,
            MigrationType::SolanaCompatibility => 2,
            MigrationType::EmergencyMigration => 3,
        };
        offset += 1;
        
        // Incentive multiplier
        dst[offset..offset + 8].copy_from_slice(&self.incentive_multiplier.to_le_bytes());
        offset += 8;
        
        // Status (1 byte)
        dst[offset] = match self.status {
            MigrationStatus::Announced => 0,
            MigrationStatus::Active => 1,
            MigrationStatus::Finalizing => 2,
            MigrationStatus::Completed => 3,
            MigrationStatus::Cancelled => 4,
        };
        offset += 1;
        
        // Merkle root
        dst[offset..offset + 32].copy_from_slice(&self.merkle_root);
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        if src.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let mut offset = 0;
        
        // Discriminator
        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&src[offset..offset + 8]);
        offset += 8;
        
        if discriminator != MIGRATION_STATE_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Program IDs
        let old_program_id = Pubkey::new_from_array(
            src[offset..offset + 32].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 32;
        
        let new_program_id = Pubkey::new_from_array(
            src[offset..offset + 32].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 32;
        
        // Authority
        let migration_authority = Pubkey::new_from_array(
            src[offset..offset + 32].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 32;
        
        // Slots
        let start_slot = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;
        
        let end_slot = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;
        
        // Account counts
        let total_accounts_to_migrate = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;
        
        let accounts_migrated = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;
        
        // Migration type
        let migration_type = match src[offset] {
            0 => MigrationType::CriticalBugFix,
            1 => MigrationType::FeatureUpgrade,
            2 => MigrationType::SolanaCompatibility,
            3 => MigrationType::EmergencyMigration,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        offset += 1;
        
        // Incentive multiplier
        let incentive_multiplier = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;
        
        // Status
        let status = match src[offset] {
            0 => MigrationStatus::Announced,
            1 => MigrationStatus::Active,
            2 => MigrationStatus::Finalizing,
            3 => MigrationStatus::Completed,
            4 => MigrationStatus::Cancelled,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        offset += 1;
        
        // Merkle root
        let mut merkle_root = [0u8; 32];
        merkle_root.copy_from_slice(&src[offset..offset + 32]);
        
        Ok(Self {
            discriminator,
            old_program_id,
            new_program_id,
            migration_authority,
            start_slot,
            end_slot,
            total_accounts_to_migrate,
            accounts_migrated,
            migration_type,
            incentive_multiplier,
            status,
            merkle_root,
        })
    }
}

/// Chain snapshot for position migration
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ChainSnapshot {
    pub step_type: ChainStepType,
    pub amount: u64,
    pub multiplier: u64, // Fixed point value
    pub verse_id: [u8; 32],
}

/// Position snapshot for migration
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct PositionSnapshot {
    pub discriminator: [u8; 8],
    pub position_id: [u8; 32],
    pub owner: Pubkey,
    pub market_id: [u8; 32],
    pub notional: u64,
    pub margin: u64,
    pub entry_price: u64, // Fixed point value
    pub leverage: u64,    // Fixed point value
    pub side: PositionSide,
    pub unrealized_pnl: i64,
    pub funding_paid: i64,
    pub chain_positions: Vec<ChainSnapshot>,
    pub snapshot_slot: u64,
    pub signature: [u8; 64],
}

impl PositionSnapshot {
    pub const BASE_LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 4 + 8 + 64; // Without chain positions
    
    pub fn pack(&self, dst: &mut [u8]) -> Result<(), ProgramError> {
        let serialized = self.try_to_vec().map_err(|_| ProgramError::InvalidAccountData)?;
        if serialized.len() > dst.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        dst[..serialized.len()].copy_from_slice(&serialized);
        Ok(())
    }
    
    pub fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }
    
    pub fn to_signing_bytes(&self) -> Result<Vec<u8>, ProgramError> {
        // Create a copy without the signature for signing
        let mut signing_data = self.clone();
        signing_data.signature = [0u8; 64];
        signing_data.try_to_vec().map_err(|_| ProgramError::InvalidAccountData)
    }
}

/// Verse snapshot for migration
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VerseSnapshot {
    pub discriminator: [u8; 8],
    pub verse_id: [u8; 32],
    pub parent_id: Option<[u8; 32]>,
    pub depth: u8,
    pub children: Vec<[u8; 32]>,
    pub proposals: Vec<[u8; 32]>,
    pub derived_prob: u64,        // Fixed point value
    pub correlation_factor: u64,  // Fixed point value
    pub total_oi: u64,
}

impl VerseSnapshot {
    pub fn pack(&self, dst: &mut [u8]) -> Result<(), ProgramError> {
        let serialized = self.try_to_vec().map_err(|_| ProgramError::InvalidAccountData)?;
        if serialized.len() > dst.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        dst[..serialized.len()].copy_from_slice(&serialized);
        Ok(())
    }
    
    pub fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }
}

/// Migration progress tracking
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MigrationProgress {
    pub accounts_migrated: u64,
    pub accounts_remaining: u64,
    pub percentage_complete: u64,
    pub estimated_completion_slot: u64,
    pub current_slot: u64,
    pub time_remaining: u64,
}

// Constants
pub const MIGRATION_NOTICE_PERIOD: u64 = 21_600;  // ~2 hours notice
pub const MIGRATION_DURATION: u64 = 1_296_000;    // ~6 days

// Helper functions
pub fn verify_migration_authority(
    authority: &AccountInfo,
    migration_state: &MigrationState,
) -> Result<(), ProgramError> {
    if authority.key != &migration_state.migration_authority {
        msg!("Invalid migration authority");
        return Err(ProgramError::InvalidAccountOwner);
    }
    
    if !authority.is_signer {
        msg!("Migration authority must sign");
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    Ok(())
}

pub fn verify_migration_active(migration_state: &MigrationState) -> Result<(), ProgramError> {
    if migration_state.status != MigrationStatus::Active {
        msg!("Migration not active");
        return Err(ProgramError::InvalidAccountData);
    }
    
    let clock = Clock::get()?;
    if clock.slot > migration_state.end_slot {
        msg!("Migration expired");
        return Err(ProgramError::InvalidAccountData);
    }
    
    Ok(())
}

// Events (logged via msg!)
pub fn emit_migration_announced(
    old_program: &Pubkey,
    new_program: &Pubkey,
    migration_type: MigrationType,
    start_slot: u64,
    end_slot: u64,
    incentive_multiplier: U64F64,
    total_accounts: u64,
) {
    msg!(
        "MigrationAnnounced: old={}, new={}, type={:?}, start={}, end={}, incentive={}, total={}",
        old_program,
        new_program,
        migration_type,
        start_slot,
        end_slot,
        incentive_multiplier.to_num::<u64>(),
        total_accounts
    );
}

pub fn emit_position_migrated(
    position_id: &[u8; 32],
    old_program: &Pubkey,
    new_program: &Pubkey,
    user: &Pubkey,
    notional: u64,
    incentive_mmt: u64,
    slot: u64,
) {
    msg!(
        "PositionMigrated: id={:?}, old={}, new={}, user={}, notional={}, incentive={}, slot={}",
        position_id,
        old_program,
        new_program,
        user,
        notional,
        incentive_mmt,
        slot
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_migration_state_pack_unpack() {
        let state = MigrationState {
            discriminator: MIGRATION_STATE_DISCRIMINATOR,
            old_program_id: Pubkey::new_unique(),
            new_program_id: Pubkey::new_unique(),
            migration_authority: Pubkey::new_unique(),
            start_slot: 100,
            end_slot: 200,
            total_accounts_to_migrate: 1000,
            accounts_migrated: 50,
            migration_type: MigrationType::FeatureUpgrade,
            incentive_multiplier: U64F64::from_num(2).0,
            status: MigrationStatus::Active,
            merkle_root: [0u8; 32],
        };
        
        let mut packed = vec![0u8; MigrationState::LEN];
        state.pack_into_slice(&mut packed);
        
        let unpacked = MigrationState::unpack_from_slice(&packed).unwrap();
        assert_eq!(state.old_program_id, unpacked.old_program_id);
        assert_eq!(state.migration_type, unpacked.migration_type);
        assert_eq!(state.status, unpacked.status);
    }
}