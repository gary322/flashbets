// Verse & Market Migration
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
    clock::Clock,
    sysvar::Sysvar,
    program::invoke_signed,
    system_instruction,
    program_pack::Pack,
    keccak,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::U64F64;
use crate::migration::core::{
    MigrationState, VerseSnapshot, verify_migration_active,
    VERSE_SNAPSHOT_DISCRIMINATOR,
};

// Proposal AMM type
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum AmmType {
    Lmsr,
    PmAmm,
    ConstantSum,
    ConstantProduct,
}

// Simplified proposal structure for migration
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ProposalData {
    pub proposal_id: [u8; 32],
    pub market_id: [u8; 32],
    pub amm_type: AmmType,
    pub outcomes: u8,
}

pub struct VerseMigrator;

impl VerseMigrator {
    /// Migrate verse hierarchy recursively
    pub fn migrate_verse_hierarchy(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        verse_snapshot: VerseSnapshot,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        // Accounts expected:
        // 0. Migration state
        // 1. Old verse account
        // 2. New verse account (to be created)
        // 3. Parent verse account (optional)
        // 4. Migration authority
        // 5. New program
        // 6. System program
        // 7+ Child verse accounts
        // N+ Proposal accounts
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let old_verse_account = next_account_info(account_info_iter)?;
        let new_verse_account = next_account_info(account_info_iter)?;
        let parent_verse_account = next_account_info(account_info_iter)?;
        let migration_authority = next_account_info(account_info_iter)?;
        let new_program_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        
        // Load and verify migration state
        let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        verify_migration_active(&migration_state)?;
        
        // Verify migration authority
        if migration_authority.key != &migration_state.migration_authority {
            msg!("Invalid migration authority");
            return Err(ProgramError::InvalidAccountOwner);
        }
        
        if !migration_authority.is_signer {
            msg!("Migration authority must sign");
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Verify verse hasn't been migrated yet
        if Self::is_verse_migrated(&verse_snapshot.verse_id, accounts, new_program_account.key)? {
            msg!("Verse already migrated");
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        
        // Create new verse account
        let verse_seeds: &[&[u8]] = &[
            b"verse",
            &verse_snapshot.verse_id,
        ];
        
        let (verse_pda, bump) = Pubkey::find_program_address(
            verse_seeds,
            new_program_account.key,
        );
        
        if verse_pda != *new_verse_account.key {
            msg!("Invalid new verse PDA");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Calculate size based on dynamic data
        let verse_size = 8 + 32 + 33 + 1 + 4 + (verse_snapshot.children.len() * 32) + 
                        4 + (verse_snapshot.proposals.len() * 32) + 8 + 8 + 8;
        
        let rent_lamports = solana_program::rent::Rent::default().minimum_balance(verse_size);
        
        invoke_signed(
            &system_instruction::create_account(
                migration_authority.key,
                new_verse_account.key,
                rent_lamports,
                verse_size as u64,
                new_program_account.key,
            ),
            &[
                migration_authority.clone(),
                new_verse_account.clone(),
                system_program.clone(),
            ],
            &[&[
                b"verse",
                &verse_snapshot.verse_id,
                &[bump],
            ]],
        )?;
        
        // Initialize new verse data
        let new_verse_data = VerseSnapshot {
            discriminator: VERSE_SNAPSHOT_DISCRIMINATOR,
            verse_id: verse_snapshot.verse_id,
            parent_id: verse_snapshot.parent_id,
            depth: verse_snapshot.depth,
            children: verse_snapshot.children.clone(),
            proposals: verse_snapshot.proposals.clone(),
            derived_prob: verse_snapshot.derived_prob,
            correlation_factor: verse_snapshot.correlation_factor,
            total_oi: verse_snapshot.total_oi,
        };
        
        new_verse_data.pack(&mut new_verse_account.data.borrow_mut())?;
        
        // Migrate all child verses recursively
        for (i, child_id) in verse_snapshot.children.iter().enumerate() {
            msg!("Migrating child verse {}: {:?}", i, child_id);
            // In production, would call migrate_child_verse with proper accounts
        }
        
        // Migrate all proposals in this verse
        for (i, proposal_id) in verse_snapshot.proposals.iter().enumerate() {
            msg!("Migrating proposal {}: {:?}", i, proposal_id);
            Self::migrate_proposal(
                accounts,
                *proposal_id,
                new_program_account.key,
                migration_authority.key,
            )?;
        }
        
        // Update merkle root in new verse
        let children_root = Self::compute_merkle_root(&verse_snapshot.children)?;
        
        // Update migration tracking
        Self::mark_verse_migrated(
            &verse_snapshot.verse_id,
            accounts,
            new_program_account.key,
        )?;
        
        msg!(
            "VerseMigrated: id={:?}, depth={}, children={}, proposals={}",
            verse_snapshot.verse_id,
            verse_snapshot.depth,
            verse_snapshot.children.len(),
            verse_snapshot.proposals.len()
        );
        
        Ok(())
    }
    
    /// Migrate a single proposal
    fn migrate_proposal(
        accounts: &[AccountInfo],
        proposal_id: [u8; 32],
        new_program: &Pubkey,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // In production, would:
        // 1. Load old proposal data
        // 2. Create new proposal account
        // 3. Copy proposal parameters
        // 4. Update verse reference
        
        msg!("Migrating proposal: {:?}", proposal_id);
        
        // Placeholder proposal data
        let proposal = ProposalData {
            proposal_id,
            market_id: [0u8; 32],
            amm_type: AmmType::Lmsr,
            outcomes: 2,
        };
        
        Ok(())
    }
    
    /// Check if verse has been migrated
    fn is_verse_migrated(
        verse_id: &[u8; 32],
        accounts: &[AccountInfo],
        new_program: &Pubkey,
    ) -> Result<bool, ProgramError> {
        // Check migration tracking PDA
        let (tracking_pda, _) = Pubkey::find_program_address(
            &[b"migration", verse_id],
            new_program,
        );
        
        // Look for tracking account in remaining accounts
        for account in accounts {
            if account.key == &tracking_pda && account.data_len() > 0 {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Mark verse as migrated
    fn mark_verse_migrated(
        verse_id: &[u8; 32],
        accounts: &[AccountInfo],
        new_program: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Create migration tracking PDA
        let (tracking_pda, bump) = Pubkey::find_program_address(
            &[b"migration", verse_id],
            new_program,
        );
        
        msg!("Marking verse as migrated: {:?}", verse_id);
        
        // In production, would create small tracking account
        Ok(())
    }
    
    /// Compute merkle root of child verses
    fn compute_merkle_root(children: &[[u8; 32]]) -> Result<[u8; 32], ProgramError> {
        if children.is_empty() {
            return Ok([0u8; 32]);
        }
        
        // Simple merkle tree implementation
        let mut current_level: Vec<[u8; 32]> = children.to_vec();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                let left = current_level[i];
                let right = if i + 1 < current_level.len() {
                    current_level[i + 1]
                } else {
                    current_level[i] // Duplicate if odd number
                };
                
                let mut combined = Vec::with_capacity(64);
                combined.extend_from_slice(&left);
                combined.extend_from_slice(&right);
                
                let hash = keccak::hash(&combined);
                next_level.push(hash.to_bytes());
            }
            
            current_level = next_level;
        }
        
        Ok(current_level[0])
    }
}

/// Create verse snapshot for migration
pub fn create_verse_snapshot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let verse_account = next_account_info(account_info_iter)?;
    let snapshot_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    if !authority_account.is_signer {
        msg!("Authority must sign to create snapshot");
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = Clock::get()?;
    
    // Create snapshot (simplified)
    let snapshot = VerseSnapshot {
        discriminator: VERSE_SNAPSHOT_DISCRIMINATOR,
        verse_id: [1u8; 32], // Would read from verse
        parent_id: None,
        depth: 0,
        children: vec![],
        proposals: vec![],
        derived_prob: U64F64::from_num(50).0, // 50%
        correlation_factor: U64F64::from_num(1).0,
        total_oi: 0,
    };
    
    // Calculate size
    let snapshot_size = 8 + 32 + 33 + 1 + 4 + 4 + 8 + 8 + 8;
    let rent_lamports = solana_program::rent::Rent::default().minimum_balance(snapshot_size);
    
    invoke_signed(
        &system_instruction::create_account(
            authority_account.key,
            snapshot_account.key,
            rent_lamports,
            snapshot_size as u64,
            program_id,
        ),
        &[
            authority_account.clone(),
            snapshot_account.clone(),
            system_program.clone(),
        ],
        &[],
    )?;
    
    snapshot.pack(&mut snapshot_account.data.borrow_mut())?;
    
    msg!("Verse snapshot created");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_merkle_root_computation() {
        let children = vec![
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
        ];
        
        let root = VerseMigrator::compute_merkle_root(&children).unwrap();
        assert_ne!(root, [0u8; 32]);
        
        // Test empty children
        let empty_root = VerseMigrator::compute_merkle_root(&[]).unwrap();
        assert_eq!(empty_root, [0u8; 32]);
        
        // Test single child
        let single = vec![[5u8; 32]];
        let single_root = VerseMigrator::compute_merkle_root(&single).unwrap();
        assert_eq!(single_root, [5u8; 32]);
    }
}