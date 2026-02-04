//! Rollback Protection System
//!
//! Implements state hash chains and transaction ordering to prevent rollbacks

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    hash::hash,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
};

/// Discriminator for rollback protection state
pub const ROLLBACK_PROTECTION_DISCRIMINATOR: [u8; 8] = [82, 79, 76, 76, 66, 65, 67, 75];

/// Global rollback protection state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RollbackProtectionState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Current state version
    pub version: u64,
    
    /// Previous state hash (forms chain)
    pub previous_hash: [u8; 32],
    
    /// Current state hash
    pub current_hash: [u8; 32],
    
    /// Last transaction signature
    pub last_tx_signature: [u8; 64],
    
    /// Transaction counter (monotonic)
    pub tx_counter: u64,
    
    /// Last update slot
    pub last_slot: u64,
    
    /// Last update timestamp
    pub last_timestamp: i64,
    
    /// Authority that can reset in emergencies
    pub emergency_authority: Pubkey,
    
    /// State frozen for migration
    pub frozen: bool,
}

impl RollbackProtectionState {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 8 + 32 + 32 + 64 + 8 + 8 + 8 + 32 + 1;
    
    /// Initialize rollback protection
    pub fn initialize(emergency_authority: Pubkey) -> Self {
        Self {
            discriminator: ROLLBACK_PROTECTION_DISCRIMINATOR,
            version: 1,
            previous_hash: [0; 32],
            current_hash: [0; 32],
            last_tx_signature: [0; 64],
            tx_counter: 0,
            last_slot: 0,
            last_timestamp: 0,
            emergency_authority,
            frozen: false,
        }
    }
    
    /// Validate state consistency
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != ROLLBACK_PROTECTION_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Ensure version is valid
        if self.version == 0 {
            return Err(BettingPlatformError::InvalidStateVersion.into());
        }
        
        Ok(())
    }
    
    /// Update state with new transaction
    pub fn update_state(
        &mut self,
        tx_signature: &[u8; 64],
        state_data: &[u8],
        current_slot: u64,
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        // Check if frozen
        if self.frozen {
            return Err(BettingPlatformError::StateFrozen.into());
        }
        
        // Ensure monotonic slot progression
        if current_slot <= self.last_slot {
            return Err(BettingPlatformError::InvalidSlotProgression.into());
        }
        
        // Compute new state hash including previous hash (forms chain)
        let mut hash_data = Vec::new();
        hash_data.extend_from_slice(&self.current_hash);
        hash_data.extend_from_slice(tx_signature);
        hash_data.extend_from_slice(state_data);
        hash_data.extend_from_slice(&current_slot.to_le_bytes());
        
        let new_hash = hash(&hash_data);
        
        // Update state
        self.previous_hash = self.current_hash;
        self.current_hash = new_hash.to_bytes();
        self.last_tx_signature = *tx_signature;
        self.tx_counter += 1;
        self.last_slot = current_slot;
        self.last_timestamp = current_timestamp;
        
        Ok(())
    }
    
    /// Verify state hash chain integrity
    pub fn verify_chain_integrity(
        &self,
        expected_previous_hash: &[u8; 32],
    ) -> Result<(), ProgramError> {
        if &self.previous_hash != expected_previous_hash {
            return Err(BettingPlatformError::HashChainBroken.into());
        }
        Ok(())
    }
    
    /// Freeze state for migration
    pub fn freeze_for_migration(&mut self, authority: &Pubkey) -> Result<(), ProgramError> {
        if authority != &self.emergency_authority {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        self.frozen = true;
        msg!("State frozen for migration at version {}", self.version);
        
        Ok(())
    }
}

/// Transaction ordering validator
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TransactionOrdering {
    /// Expected next nonce
    pub next_nonce: u64,
    
    /// User nonces for replay protection
    pub user_nonces: Vec<(Pubkey, u64)>,
    
    /// Maximum nonce gap allowed
    pub max_nonce_gap: u64,
}

impl TransactionOrdering {
    pub const MAX_TRACKED_USERS: usize = 1000;
    
    /// Validate and update transaction nonce
    pub fn validate_nonce(
        &mut self,
        user: &Pubkey,
        nonce: u64,
    ) -> Result<(), ProgramError> {
        // Find user's last nonce
        let user_entry = self.user_nonces.iter_mut()
            .find(|(pubkey, _)| pubkey == user);
        
        match user_entry {
            Some((_, last_nonce)) => {
                // Ensure nonce is increasing
                if nonce <= *last_nonce {
                    return Err(BettingPlatformError::NonceReused.into());
                }
                
                // Check nonce gap
                if nonce > *last_nonce + self.max_nonce_gap {
                    return Err(BettingPlatformError::NonceTooHigh.into());
                }
                
                // Update nonce
                *last_nonce = nonce;
            }
            None => {
                // New user, add to tracking
                if self.user_nonces.len() >= Self::MAX_TRACKED_USERS {
                    // Remove oldest entry (FIFO)
                    self.user_nonces.remove(0);
                }
                
                self.user_nonces.push((*user, nonce));
            }
        }
        
        Ok(())
    }
}

/// Process state update with rollback protection
pub fn process_protected_state_update(
    accounts: &[AccountInfo],
    tx_signature: &[u8; 64],
    state_data: &[u8],
    user_nonce: u64,
) -> ProgramResult {
    let rollback_account = &accounts[0];
    let user_account = &accounts[1];
    let clock_sysvar = &accounts[2];
    
    // Load rollback protection state
    let mut rollback_data = rollback_account.try_borrow_mut_data()?;
    let mut rollback_state = RollbackProtectionState::try_from_slice(&rollback_data)?;
    rollback_state.validate()?;
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Update state with protection
    rollback_state.update_state(
        tx_signature,
        state_data,
        clock.slot,
        clock.unix_timestamp,
    )?;
    
    // Serialize back
    rollback_state.serialize(&mut *rollback_data)?;
    
    msg!("State updated with rollback protection: tx_counter={}, hash={:?}",
        rollback_state.tx_counter,
        rollback_state.current_hash
    );
    
    Ok(())
}

/// State version info for migrations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StateVersion {
    /// Current version number
    pub version: u32,
    
    /// Migration in progress
    pub migrating: bool,
    
    /// Target version for migration
    pub target_version: u32,
    
    /// Migration started at slot
    pub migration_slot: u64,
}

impl StateVersion {
    pub const CURRENT_VERSION: u32 = 1;
    
    /// Check if migration is needed
    pub fn needs_migration(&self, target: u32) -> bool {
        self.version < target && !self.migrating
    }
    
    /// Start migration
    pub fn start_migration(&mut self, target: u32, slot: u64) -> Result<(), ProgramError> {
        if self.migrating {
            return Err(BettingPlatformError::MigrationInProgress.into());
        }
        
        if target <= self.version {
            return Err(BettingPlatformError::InvalidMigrationTarget.into());
        }
        
        self.migrating = true;
        self.target_version = target;
        self.migration_slot = slot;
        
        Ok(())
    }
    
    /// Complete migration
    pub fn complete_migration(&mut self) -> Result<(), ProgramError> {
        if !self.migrating {
            return Err(BettingPlatformError::NoMigrationInProgress.into());
        }
        
        self.version = self.target_version;
        self.migrating = false;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_chain() {
        let mut state = RollbackProtectionState::initialize(Pubkey::new_unique());
        
        let tx_sig = [1u8; 64];
        let state_data = b"test state data";
        
        // First update
        state.update_state(&tx_sig, state_data, 100, 1000).unwrap();
        let first_hash = state.current_hash;
        
        // Second update forms chain
        let tx_sig2 = [2u8; 64];
        state.update_state(&tx_sig2, state_data, 101, 1001).unwrap();
        
        // Verify chain
        assert_eq!(state.previous_hash, first_hash);
        assert_ne!(state.current_hash, first_hash);
    }
    
    #[test]
    fn test_transaction_ordering() {
        let mut ordering = TransactionOrdering {
            next_nonce: 1,
            user_nonces: Vec::new(),
            max_nonce_gap: 100,
        };
        
        let user = Pubkey::new_unique();
        
        // First transaction
        ordering.validate_nonce(&user, 1).unwrap();
        
        // Replay should fail
        assert!(ordering.validate_nonce(&user, 1).is_err());
        
        // Next nonce should work
        ordering.validate_nonce(&user, 2).unwrap();
        
        // Too high nonce should fail
        assert!(ordering.validate_nonce(&user, 200).is_err());
    }
}