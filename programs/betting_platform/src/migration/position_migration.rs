// Position Migration System
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
    clock::Clock,
    sysvar::Sysvar,
    program::{invoke_signed, invoke},
    system_instruction,
    program_pack::Pack,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::{U64F64, MathError};
use crate::migration::core::{
    MigrationState, PositionSnapshot, ChainSnapshot, ChainStepType,
    PositionSide, MigrationStatus, verify_migration_active,
    emit_position_migrated, POSITION_SNAPSHOT_DISCRIMINATOR,
};

pub struct PositionMigrator;

impl PositionMigrator {
    /// Main position migration function - "close old, open new"
    pub fn migrate_position(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        position_snapshot: PositionSnapshot,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        // Accounts expected:
        // 0. Migration state
        // 1. Old position account
        // 2. New position account (to be created)
        // 3. User (owner)
        // 4. Old program
        // 5. New program
        // 6. Price feed
        // 7. MMT mint
        // 8. User MMT token account
        // 9. System program
        // 10. Market account
        // 11. Vault account
        // 12+ Verse accounts for chain positions
        
        let migration_state_account = next_account_info(account_info_iter)?;
        let old_position_account = next_account_info(account_info_iter)?;
        let new_position_account = next_account_info(account_info_iter)?;
        let user_account = next_account_info(account_info_iter)?;
        let old_program_account = next_account_info(account_info_iter)?;
        let new_program_account = next_account_info(account_info_iter)?;
        let price_feed_account = next_account_info(account_info_iter)?;
        let mmt_mint_account = next_account_info(account_info_iter)?;
        let user_mmt_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let market_account = next_account_info(account_info_iter)?;
        let vault_account = next_account_info(account_info_iter)?;
        
        // Load and verify migration state
        let mut migration_state = MigrationState::unpack(&migration_state_account.data.borrow())?;
        verify_migration_active(&migration_state)?;
        
        // Verify snapshot matches current position
        Self::verify_snapshot(&position_snapshot, old_position_account)?;
        
        // Verify user ownership
        if &position_snapshot.owner != user_account.key {
            msg!("User does not own position");
            return Err(ProgramError::InvalidAccountOwner);
        }
        
        if !user_account.is_signer {
            msg!("User must sign for migration");
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        let clock = Clock::get()?;
        
        // Calculate final P&L and funding (simplified for now)
        let final_pnl = Self::calculate_pnl_at_price(
            &position_snapshot,
            price_feed_account,
        )?;
        let pending_funding = position_snapshot.funding_paid;
        
        // Calculate close amount
        let close_amount = position_snapshot.margin
            .checked_add(final_pnl.max(0) as u64).ok_or(ProgramError::InvalidAccountData)?
            .checked_add(pending_funding.max(0) as u64).ok_or(ProgramError::InvalidAccountData)?;
        
        // Close old position by zeroing lamports
        let old_position_lamports = old_position_account.lamports();
        **old_position_account.lamports.borrow_mut() = 0;
        **user_account.lamports.borrow_mut() = user_account.lamports()
            .checked_add(old_position_lamports)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        // Create new position account with same parameters
        let new_position_seeds: &[&[u8]] = &[
            b"position",
            user_account.key.as_ref(),
            &position_snapshot.market_id,
            &[clock.slot as u8], // Add uniqueness
        ];
        
        let (new_position_pda, bump) = Pubkey::find_program_address(
            new_position_seeds,
            new_program_account.key,
        );
        
        if new_position_pda != *new_position_account.key {
            msg!("Invalid new position PDA");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Create the new position account
        let rent_lamports = solana_program::rent::Rent::default().minimum_balance(
            PositionSnapshot::BASE_LEN + position_snapshot.chain_positions.len() * 64
        );
        
        invoke_signed(
            &system_instruction::create_account(
                user_account.key,
                new_position_account.key,
                rent_lamports,
                (PositionSnapshot::BASE_LEN + position_snapshot.chain_positions.len() * 64) as u64,
                new_program_account.key,
            ),
            &[
                user_account.clone(),
                new_position_account.clone(),
                system_program.clone(),
            ],
            &[&[
                b"position",
                user_account.key.as_ref(),
                &position_snapshot.market_id,
                &[clock.slot as u8],
                &[bump],
            ]],
        )?;
        
        // Initialize new position data
        let new_position_data = PositionSnapshot {
            discriminator: POSITION_SNAPSHOT_DISCRIMINATOR,
            position_id: position_snapshot.position_id,
            owner: position_snapshot.owner,
            market_id: position_snapshot.market_id,
            notional: position_snapshot.notional,
            margin: close_amount, // Use the close amount as new margin
            entry_price: position_snapshot.entry_price,
            leverage: position_snapshot.leverage,
            side: position_snapshot.side,
            unrealized_pnl: 0, // Reset P&L
            funding_paid: 0,   // Reset funding
            chain_positions: position_snapshot.chain_positions.clone(),
            snapshot_slot: clock.slot,
            signature: [0u8; 64], // Will be signed later
        };
        
        new_position_data.pack(&mut new_position_account.data.borrow_mut())?;
        
        // Migrate chain positions if any
        if !position_snapshot.chain_positions.is_empty() {
            Self::migrate_chain_positions(
                accounts,
                &position_snapshot.chain_positions,
                new_program_account.key,
            )?;
        }
        
        // Calculate and apply migration incentive
        let incentive_amount = Self::calculate_migration_incentive(
            &position_snapshot,
            U64F64::from_raw(migration_state.incentive_multiplier),
        )?;
        
        // Mint MMT rewards (simplified - in production would use proper SPL token mint)
        Self::mint_mmt_reward(
            mmt_mint_account,
            user_mmt_account,
            incentive_amount,
        )?;
        
        // Update migration state
        migration_state.accounts_migrated = migration_state.accounts_migrated
            .checked_add(1)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        migration_state.pack_into_slice(&mut migration_state_account.data.borrow_mut());
        
        // Emit event
        emit_position_migrated(
            &position_snapshot.position_id,
            old_program_account.key,
            new_program_account.key,
            user_account.key,
            position_snapshot.notional,
            incentive_amount,
            clock.slot,
        );
        
        Ok(())
    }
    
    /// Verify snapshot matches current position
    fn verify_snapshot(
        snapshot: &PositionSnapshot,
        position_account: &AccountInfo,
    ) -> Result<(), ProgramError> {
        // In production, would deserialize actual position data and compare
        // For now, verify basic properties
        
        if snapshot.discriminator != POSITION_SNAPSHOT_DISCRIMINATOR {
            msg!("Invalid snapshot discriminator");
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Verify signature (simplified - in production would use ed25519)
        // let snapshot_data = snapshot.to_signing_bytes()?;
        // ed25519_verify(&snapshot_data, &snapshot.signature, &snapshot.owner)?;
        
        Ok(())
    }
    
    /// Calculate P&L at current price
    fn calculate_pnl_at_price(
        snapshot: &PositionSnapshot,
        price_feed: &AccountInfo,
    ) -> Result<i64, ProgramError> {
        // Read price from feed (simplified)
        let current_price = U64F64::from_num(100); // Placeholder
        let entry_price = U64F64::from_raw(snapshot.entry_price);
        
        let price_diff = if snapshot.side == PositionSide::Long {
            current_price.saturating_sub(entry_price)
        } else {
            entry_price.saturating_sub(current_price)
        };
        
        let pnl_fixed = price_diff.saturating_mul(U64F64::from_num(snapshot.notional));
        Ok(pnl_fixed.to_num::<i64>())
    }
    
    /// Migrate chain positions
    fn migrate_chain_positions(
        accounts: &[AccountInfo],
        chain_snapshots: &[ChainSnapshot],
        new_program: &Pubkey,
    ) -> Result<(), ProgramError> {
        // In production, would iterate through verse accounts and recreate chain positions
        for (i, snapshot) in chain_snapshots.iter().enumerate() {
            msg!(
                "Migrating chain position {}: type={:?}, amount={}, verse={:?}",
                i,
                snapshot.step_type,
                snapshot.amount,
                snapshot.verse_id
            );
            // Would invoke new program to create chain position
        }
        Ok(())
    }
    
    /// Calculate migration incentive
    fn calculate_migration_incentive(
        snapshot: &PositionSnapshot,
        multiplier: U64F64,
    ) -> Result<u64, ProgramError> {
        // Base incentive = 0.1% of notional * multiplier
        let base_incentive_bps = 10u64; // 0.1% = 10 basis points
        let base_incentive = snapshot.notional
            .checked_mul(base_incentive_bps)
            .ok_or(ProgramError::InvalidAccountData)?
            .checked_div(10_000)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        let incentive_fixed = U64F64::from_num(base_incentive)
            .saturating_mul(multiplier);
        
        Ok(incentive_fixed.to_num())
    }
    
    /// Mint MMT reward tokens
    fn mint_mmt_reward(
        mint_account: &AccountInfo,
        user_token_account: &AccountInfo,
        amount: u64,
    ) -> Result<(), ProgramError> {
        // In production, would use SPL token program to mint
        msg!("Minting {} MMT tokens as migration incentive", amount);
        Ok(())
    }
}

/// Create position snapshot for migration
pub fn create_position_snapshot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_info_iter)?;
    let snapshot_account = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Verify user owns the position
    if !user_account.is_signer {
        msg!("User must sign to create snapshot");
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = Clock::get()?;
    
    // Read position data (simplified)
    // In production would deserialize actual position structure
    
    // Create snapshot
    let snapshot = PositionSnapshot {
        discriminator: POSITION_SNAPSHOT_DISCRIMINATOR,
        position_id: [0u8; 32], // Would read from position
        owner: *user_account.key,
        market_id: [0u8; 32], // Would read from position
        notional: 1000,       // Placeholder
        margin: 100,          // Placeholder
        entry_price: U64F64::from_num(100).0,
        leverage: U64F64::from_num(10).0,
        side: PositionSide::Long,
        unrealized_pnl: 0,
        funding_paid: 0,
        chain_positions: vec![],
        snapshot_slot: clock.slot,
        signature: [0u8; 64], // Would be signed
    };
    
    // Create snapshot account
    let snapshot_size = PositionSnapshot::BASE_LEN;
    let rent_lamports = solana_program::rent::Rent::default().minimum_balance(snapshot_size);
    
    invoke(
        &system_instruction::create_account(
            user_account.key,
            snapshot_account.key,
            rent_lamports,
            snapshot_size as u64,
            program_id,
        ),
        &[
            user_account.clone(),
            snapshot_account.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Store snapshot
    snapshot.pack(&mut snapshot_account.data.borrow_mut())?;
    
    msg!("Position snapshot created");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_incentive_calculation() {
        let snapshot = PositionSnapshot {
            discriminator: POSITION_SNAPSHOT_DISCRIMINATOR,
            position_id: [0u8; 32],
            owner: Pubkey::new_unique(),
            market_id: [0u8; 32],
            notional: 100_000,
            margin: 10_000,
            entry_price: U64F64::from_num(100).0,
            leverage: U64F64::from_num(10).0,
            side: PositionSide::Long,
            unrealized_pnl: 0,
            funding_paid: 0,
            chain_positions: vec![],
            snapshot_slot: 0,
            signature: [0u8; 64],
        };
        
        let multiplier = U64F64::from_num(2); // 2x incentive
        let incentive = PositionMigrator::calculate_migration_incentive(&snapshot, multiplier).unwrap();
        
        // 0.1% of 100,000 = 100, times 2 = 200
        assert_eq!(incentive, 200);
    }
}