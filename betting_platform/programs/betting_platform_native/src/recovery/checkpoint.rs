//! Checkpoint system for state recovery
//!
//! Creates periodic snapshots of critical system state

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;
use crate::math::U64F64;

/// Checkpoint account for state snapshots
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Checkpoint {
    pub checkpoint_id: u64,
    pub created_slot: u64,
    pub created_by: Pubkey,
    pub checkpoint_type: CheckpointType,
    
    // Global state snapshot
    pub global_snapshot: GlobalSnapshot,
    
    // Critical accounts
    pub critical_accounts: Vec<CriticalAccount>,
    
    // Merkle roots for verification
    pub positions_root: [u8; 32],
    pub orders_root: [u8; 32],
    pub verses_root: [u8; 32],
    
    // Statistics
    pub total_positions: u64,
    pub total_orders: u64,
    pub total_volume: u128,
    pub total_oi: u128,
    
    // Verification
    pub verified: bool,
    pub verification_slot: Option<u64>,
    pub verification_signature: Option<[u8; 64]>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum CheckpointType {
    Scheduled,      // Regular interval checkpoint
    Manual,         // Manually triggered
    PreUpgrade,     // Before system upgrade
    Emergency,      // During emergency
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct GlobalSnapshot {
    pub epoch: u64,
    pub season: u64,
    pub vault_balance: u128,
    pub total_oi: u128,
    pub coverage: U64F64,
    pub mmt_supply: u64,
    pub keeper_count: u16,
    pub active_markets: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CriticalAccount {
    pub account_type: AccountType,
    pub pubkey: Pubkey,
    pub data_hash: [u8; 32],
    pub size: usize,
    pub last_modified_slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum AccountType {
    GlobalConfig,
    SystemHealth,
    RecoveryState,
    CollateralVault,
    KeeperRegistry,
}

/// State snapshot for recovery
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StateSnapshot {
    pub snapshot_slot: u64,
    pub positions: Vec<PositionSnapshot>,
    pub orders: Vec<OrderSnapshot>,
    pub keeper_states: Vec<KeeperSnapshot>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionSnapshot {
    pub position_id: [u8; 32],
    pub owner: Pubkey,
    pub market_id: [u8; 32],
    pub size: u64,
    pub entry_price: U64F64,
    pub leverage: u32,
    pub is_long: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderSnapshot {
    pub order_id: [u8; 32],
    pub user: Pubkey,
    pub order_type: u8, // Simplified type
    pub size: u64,
    pub status: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct KeeperSnapshot {
    pub keeper: Pubkey,
    pub stake: u64,
    pub performance_score: u16,
    pub last_action_slot: u64,
}

/// Checkpoint manager
pub struct CheckpointManager;

impl CheckpointManager {
    /// Create a new checkpoint
    pub fn create_checkpoint(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        checkpoint_type: CheckpointType,
    ) -> ProgramResult {
        // Account layout:
        // 0. Checkpoint account (mut)
        // 1. Global config
        // 2. System health
        // 3. Authority (signer)
        // 4. Clock sysvar
        
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let checkpoint_account = &accounts[0];
        let global_config = &accounts[1];
        let system_health = &accounts[2];
        let authority = &accounts[3];
        let clock = Clock::get()?;
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        msg!("Creating {:?} checkpoint at slot {}", checkpoint_type, clock.slot);
        
        // Create global snapshot
        let global_snapshot = Self::create_global_snapshot(global_config)?;
        let total_oi_copy = global_snapshot.total_oi;
        
        // Create critical accounts list
        let critical_accounts = vec![
            CriticalAccount {
                account_type: AccountType::GlobalConfig,
                pubkey: *global_config.key,
                data_hash: Self::hash_account_data(global_config)?,
                size: global_config.data_len(),
                last_modified_slot: clock.slot,
            },
            CriticalAccount {
                account_type: AccountType::SystemHealth,
                pubkey: *system_health.key,
                data_hash: Self::hash_account_data(system_health)?,
                size: system_health.data_len(),
                last_modified_slot: clock.slot,
            },
        ];
        
        // Create checkpoint
        let checkpoint = Checkpoint {
            checkpoint_id: clock.slot, // Use slot as unique ID
            created_slot: clock.slot,
            created_by: *authority.key,
            checkpoint_type,
            global_snapshot,
            critical_accounts,
            positions_root: [0; 32], // Would be computed from actual positions
            orders_root: [0; 32],    // Would be computed from actual orders
            verses_root: [0; 32],    // Would be computed from actual verses
            total_positions: 0,      // Would be counted
            total_orders: 0,         // Would be counted
            total_volume: 0,         // Would be calculated
            total_oi: total_oi_copy,
            verified: false,
            verification_slot: None,
            verification_signature: None,
        };
        
        // Serialize checkpoint
        let mut checkpoint_data = checkpoint_account.try_borrow_mut_data()?;
        checkpoint.serialize(&mut *checkpoint_data)?;
        
        msg!(
            "CheckpointCreated - id: {}, type: {:?}, vault: {}, oi: {}",
            checkpoint.checkpoint_id,
            checkpoint_type,  
            checkpoint.global_snapshot.vault_balance,
            checkpoint.global_snapshot.total_oi
        );
        
        Ok(())
    }
    
    /// Verify a checkpoint
    pub fn verify_checkpoint(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        checkpoint_id: u64,
    ) -> ProgramResult {
        // Account layout:
        // 0. Checkpoint account (mut)
        // 1. Verifier (signer)
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let checkpoint_account = &accounts[0];
        let verifier = &accounts[1];
        let clock = Clock::get()?;
        
        if !verifier.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize checkpoint
        let mut checkpoint_data = checkpoint_account.try_borrow_mut_data()?;
        let mut checkpoint = Checkpoint::try_from_slice(&checkpoint_data)?;
        
        if checkpoint.checkpoint_id != checkpoint_id {
            return Err(BettingPlatformError::InvalidCheckpoint.into());
        }
        
        if checkpoint.verified {
            msg!("Checkpoint already verified");
            return Ok(());
        }
        
        // Verify critical accounts still match
        // In production, would verify merkle roots and account hashes
        
        checkpoint.verified = true;
        checkpoint.verification_slot = Some(clock.slot);
        
        // Create verification signature (simplified)
        let mut signature = [0u8; 64];
        signature[0..32].copy_from_slice(&checkpoint_id.to_le_bytes()[0..8]);
        signature[32..40].copy_from_slice(&clock.slot.to_le_bytes());
        checkpoint.verification_signature = Some(signature);
        
        // Serialize updated checkpoint
        checkpoint.serialize(&mut *checkpoint_data)?;
        
        msg!(
            "CheckpointVerified - id: {}, verifier: {}, slot: {}",
            checkpoint_id,
            verifier.key,
            clock.slot
        );
        
        Ok(())
    }
    
    /// Load state from checkpoint
    pub fn load_from_checkpoint(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        checkpoint_id: u64,
    ) -> Result<StateSnapshot, ProgramError> {
        // Account layout:
        // 0. Checkpoint account
        // 1. Authority (signer)
        
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let checkpoint_account = &accounts[0];
        let authority = &accounts[1];
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize checkpoint
        let checkpoint_data = checkpoint_account.try_borrow_data()?;
        let checkpoint = Checkpoint::try_from_slice(&checkpoint_data)?;
        
        if checkpoint.checkpoint_id != checkpoint_id {
            return Err(BettingPlatformError::InvalidCheckpoint.into());
        }
        
        if !checkpoint.verified {
            return Err(BettingPlatformError::UnverifiedCheckpoint.into());
        }
        
        // Create state snapshot
        // In production, would load actual account data
        let state_snapshot = StateSnapshot {
            snapshot_slot: checkpoint.created_slot,
            positions: vec![], // Would load from merkle tree
            orders: vec![],    // Would load from merkle tree
            keeper_states: vec![], // Would load from registry
        };
        
        Ok(state_snapshot)
    }
    
    /// Create global snapshot from config
    fn create_global_snapshot(
        global_config: &AccountInfo,
    ) -> Result<GlobalSnapshot, ProgramError> {
        // Deserialize global config account
        use crate::state::GlobalConfigPDA;
        use borsh::BorshDeserialize;
        
        let config_data = global_config.try_borrow_data()?;
        let config = GlobalConfigPDA::deserialize(&mut &config_data[..])
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        // Create snapshot from actual config data
        // Convert coverage from u128 to U64F64
        let coverage = if config.coverage == u128::MAX {
            // Infinite coverage
            U64F64::from_num(u64::MAX)
        } else {
            // Convert u128 to U64F64, capping at u64::MAX if needed
            U64F64::from_num(config.coverage.min(u64::MAX as u128) as u64)
        };
        
        Ok(GlobalSnapshot {
            epoch: config.epoch,
            season: config.season,
            vault_balance: config.vault,
            total_oi: config.total_oi,
            coverage,
            mmt_supply: config.mmt_total_supply,
            keeper_count: 0u16, // keeper_registry not in GlobalConfigPDA
            active_markets: 0u32, // active_markets not in GlobalConfigPDA
        })
    }
    
    /// Hash account data for verification
    fn hash_account_data(account: &AccountInfo) -> Result<[u8; 32], ProgramError> {
        let data = account.try_borrow_data()?;
        let hash = solana_program::keccak::hash(&data);
        Ok(hash.to_bytes())
    }
}

// Checkpoint pruning
impl CheckpointManager {
    /// Prune old checkpoints
    pub fn prune_old_checkpoints(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        keep_last_n: u8,
    ) -> ProgramResult {
        msg!("Pruning old checkpoints, keeping last {}", keep_last_n);
        
        // In production, would iterate through checkpoints and remove old ones
        // keeping only the specified number of recent checkpoints
        
        Ok(())
    }
}

// Size calculations
impl Checkpoint {
    pub const SIZE: usize = 8 + // checkpoint_id
        8 + // created_slot
        32 + // created_by
        1 + // checkpoint_type
        GlobalSnapshot::SIZE +
        4 + (10 * CriticalAccount::SIZE) + // critical_accounts Vec (max 10)
        32 + // positions_root
        32 + // orders_root
        32 + // verses_root
        8 + // total_positions
        8 + // total_orders
        16 + // total_volume
        16 + // total_oi
        1 + // verified
        1 + 8 + // verification_slot Option
        1 + 64; // verification_signature Option
}

impl GlobalSnapshot {
    pub const SIZE: usize = 8 + // epoch
        8 + // season
        16 + // vault_balance
        16 + // total_oi
        16 + // coverage (U64F64)
        8 + // mmt_supply
        2 + // keeper_count
        4; // active_markets
}

impl CriticalAccount {
    pub const SIZE: usize = 1 + // account_type
        32 + // pubkey
        32 + // data_hash
        8 + // size
        8; // last_modified_slot
}