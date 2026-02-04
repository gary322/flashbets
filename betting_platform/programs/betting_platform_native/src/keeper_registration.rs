//! Keeper registration and slashing system
//!
//! Manages keeper registration with MMT staking and slashing for failures

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    events::{Event, KeeperRegistered, KeeperSlashed, KeeperDeactivated},
    pda,
    state::{KeeperRegistry, KeeperAccount, KeeperStatus, KeeperType, KeeperSpecialization},
};

/// Minimum MMT stake required (100 MMT)
pub const MIN_KEEPER_STAKE: u64 = 100_000_000_000; // 100 MMT with 9 decimals

/// Slashing percentage (1% of stake)
pub const SLASH_PERCENTAGE: u64 = 100; // 1%

/// Slashing evidence types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum SlashingEvidence {
    MissedLiquidation { position_id: [u8; 32], slot: u64 },
    FalseExecution { order_id: [u8; 32], execution_price: u64 },
    Downtime { start_slot: u64, end_slot: u64 },
    MaliciousBehavior { description: [u8; 256] },
}

/// Keeper registration implementation
pub struct KeeperRegistration;

impl KeeperRegistration {
    /// Register a new keeper with MMT stake
    pub fn register_keeper(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        keeper_type: KeeperType,
        mmt_stake: u64,
        specializations: Vec<KeeperSpecialization>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let authority = next_account_info(account_info_iter)?;
        let keeper_account = next_account_info(account_info_iter)?;
        let registry_account = next_account_info(account_info_iter)?;
        let mmt_source = next_account_info(account_info_iter)?;
        let mmt_stake_vault = next_account_info(account_info_iter)?;
        let token_program = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent_sysvar = next_account_info(account_info_iter)?;
        
        // Verify authority is signer
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Validate minimum stake
        if mmt_stake < MIN_KEEPER_STAKE {
            msg!("Insufficient stake: {} < {}", mmt_stake, MIN_KEEPER_STAKE);
            return Err(BettingPlatformError::InsufficientCollateral.into());
        }
        
        // Verify keeper account PDA
        let keeper_id = Self::generate_keeper_id(authority.key, &Clock::get()?.slot);
        let (expected_keeper_pda, bump) = pda::KeeperAccountPDA::derive(program_id, &keeper_id);
        
        if keeper_account.key != &expected_keeper_pda {
            return Err(ProgramError::InvalidSeeds);
        }
        
        // Transfer MMT to stake vault
        let transfer_ix = spl_token::instruction::transfer(
            token_program.key,
            mmt_source.key,
            mmt_stake_vault.key,
            authority.key,
            &[authority.key],
            mmt_stake,
        )?;
        
        invoke(
            &transfer_ix,
            &[
                mmt_source.clone(),
                mmt_stake_vault.clone(),
                authority.clone(),
                token_program.clone(),
            ],
        )?;
        
        // Initialize keeper account
        let rent = Rent::from_account_info(rent_sysvar)?;
        let keeper_size = KeeperAccount::space(specializations.len());
        let keeper_lamports = rent.minimum_balance(keeper_size);
        
        // Create keeper account
        invoke(
            &solana_program::system_instruction::create_account(
                authority.key,
                keeper_account.key,
                keeper_lamports,
                keeper_size as u64,
                program_id,
            ),
            &[
                authority.clone(),
                keeper_account.clone(),
                system_program.clone(),
            ],
        )?;
        
        // Initialize keeper data
        let mut keeper = KeeperAccount::new(keeper_id, *authority.key, keeper_type);
        keeper.mmt_stake = mmt_stake;
        keeper.specializations = specializations;
        keeper.registration_slot = Clock::get()?.slot;
        
        keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
        
        // Update registry
        let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
        registry.total_keepers = registry.total_keepers
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        match keeper_type {
            KeeperType::Liquidation => {
                registry.active_liquidation_keepers = registry.active_liquidation_keepers
                    .checked_add(1)
                    .ok_or(BettingPlatformError::Overflow)?;
            }
            KeeperType::Order => {
                registry.active_order_keepers = registry.active_order_keepers
                    .checked_add(1)
                    .ok_or(BettingPlatformError::Overflow)?;
            }
            KeeperType::Ingestor => {
                registry.active_ingestor_keepers = registry.active_ingestor_keepers
                    .checked_add(1)
                    .ok_or(BettingPlatformError::Overflow)?;
            }
            KeeperType::General => {
                // General keepers don't have a specific counter
            }
        }
        
        registry.total_mmt_staked = registry.total_mmt_staked
            .checked_add(mmt_stake)
            .ok_or(BettingPlatformError::Overflow)?;
            
        registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
        
        // Emit registration event
        KeeperRegistered {
            keeper_id,
            authority: *authority.key,
            keeper_type: keeper_type as u8,
            mmt_stake,
            specializations: keeper.specializations.iter().map(|s| *s as u8).collect(),
        }.emit();
        
        msg!("Registered {} keeper {} with {} MMT stake",
            format!("{:?}", keeper_type),
            bs58::encode(&keeper_id[..8]).into_string(),
            mmt_stake / 1_000_000_000
        );
        
        Ok(())
    }
    
    /// Slash keeper for failures
    pub fn slash_keeper(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        evidence: SlashingEvidence,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let slasher = next_account_info(account_info_iter)?;
        let keeper_account = next_account_info(account_info_iter)?;
        let registry_account = next_account_info(account_info_iter)?;
        let mmt_stake_vault = next_account_info(account_info_iter)?;
        let burn_account = next_account_info(account_info_iter)?;
        let token_program = next_account_info(account_info_iter)?;
        
        // Load keeper
        let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
        let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
        
        // Validate evidence
        match &evidence {
            SlashingEvidence::MissedLiquidation { position_id, slot } => {
                // Verify the position was liquidatable at the given slot
                // In production, would load position and verify
                msg!("Slashing for missed liquidation of position {}",
                    bs58::encode(&position_id[..8]).into_string());
            }
            SlashingEvidence::FalseExecution { order_id, execution_price } => {
                // Verify the order was executed at wrong price
                msg!("Slashing for false execution of order {} at price {}",
                    bs58::encode(&order_id[..8]).into_string(), execution_price);
            }
            SlashingEvidence::Downtime { start_slot, end_slot } => {
                // Verify keeper was offline for > 1 hour
                let downtime_slots = end_slot.saturating_sub(*start_slot);
                if downtime_slots <= 9_000 { // ~1 hour at 0.4s/slot
                    return Err(BettingPlatformError::InsufficientPoints.into());
                }
                msg!("Slashing for downtime of {} slots", downtime_slots);
            }
            SlashingEvidence::MaliciousBehavior { description } => {
                msg!("Slashing for malicious behavior: {:?}", description);
            }
        }
        
        // Calculate slashing amount (1% of stake)
        let slash_amount = keeper.mmt_stake
            .checked_mul(SLASH_PERCENTAGE)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        // Update keeper stake
        keeper.mmt_stake = keeper.mmt_stake
            .checked_sub(slash_amount)
            .ok_or(BettingPlatformError::Underflow)?;
            
        keeper.slashing_count = keeper.slashing_count
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
        
        // If stake below minimum, deactivate keeper
        if keeper.mmt_stake < MIN_KEEPER_STAKE {
            keeper.status = KeeperStatus::Deactivated;
            
            // Update registry counts
            match keeper.keeper_type {
                KeeperType::Liquidation => {
                    registry.active_liquidation_keepers = registry.active_liquidation_keepers
                        .saturating_sub(1);
                }
                KeeperType::Order => {
                    registry.active_order_keepers = registry.active_order_keepers
                        .saturating_sub(1);
                }
                KeeperType::Ingestor => {
                    registry.active_ingestor_keepers = registry.active_ingestor_keepers
                        .saturating_sub(1);
                }
                KeeperType::General => {
                    // General keepers don't have a specific counter
                }
            }
            
            KeeperDeactivated {
                keeper_id: keeper.keeper_id,
                reason_code: 0, // 0 = Below minimum stake after slashing
            }.emit();
        }
        
        // Burn slashed MMT
        let burn_ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mmt_stake_vault.key,
            &pda::CollateralVaultPDA::derive(program_id).0,
            &[],
            slash_amount,
        )?;
        
        invoke(
            &burn_ix,
            &[
                burn_account.clone(),
                mmt_stake_vault.clone(),
                token_program.clone(),
            ],
        )?;
        
        // Update registry
        registry.slashing_events = registry.slashing_events
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        registry.total_mmt_staked = registry.total_mmt_staked
            .checked_sub(slash_amount)
            .ok_or(BettingPlatformError::Underflow)?;
        
        // Save updates
        keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
        registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
        
        // Emit slashing event
        KeeperSlashed {
            keeper_id: keeper.keeper_id,
            slash_amount,
            evidence_type: match evidence {
                SlashingEvidence::MissedLiquidation { .. } => 0,
                SlashingEvidence::FalseExecution { .. } => 1,
                SlashingEvidence::Downtime { .. } => 2,
                SlashingEvidence::MaliciousBehavior { .. } => 3,
            },
            remaining_stake: keeper.mmt_stake,
        }.emit();
        
        msg!("Slashed keeper {} for {} MMT, remaining stake: {} MMT",
            bs58::encode(&keeper.keeper_id[..8]).into_string(),
            slash_amount / 1_000_000_000,
            keeper.mmt_stake / 1_000_000_000
        );
        
        Ok(())
    }
    
    /// Generate unique keeper ID
    fn generate_keeper_id(authority: &Pubkey, slot: &u64) -> [u8; 32] {
        use solana_program::keccak;
        let mut data = Vec::new();
        data.extend_from_slice(authority.as_ref());
        data.extend_from_slice(&slot.to_le_bytes());
        data.extend_from_slice(b"keeper");
        keccak::hash(&data).to_bytes()
    }
}

// KeeperAccount methods are implemented in state/keeper_accounts.rs

// Hex encoding utility
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_minimum_stake_requirement() {
        let stake = 50_000_000_000; // 50 MMT
        assert!(stake < MIN_KEEPER_STAKE);
        
        let valid_stake = 150_000_000_000; // 150 MMT
        assert!(valid_stake >= MIN_KEEPER_STAKE);
    }
    
    #[test]
    fn test_slash_calculation() {
        let stake = 200_000_000_000; // 200 MMT
        let expected_slash = 2_000_000_000; // 2 MMT (1%)
        
        let slash_amount = stake * SLASH_PERCENTAGE / 10000;
        assert_eq!(slash_amount, expected_slash);
    }
    
    #[test]
    fn test_keeper_priority() {
        use crate::state::keeper_accounts::discriminators;
        
        let keeper = KeeperAccount {
            discriminator: discriminators::KEEPER_ACCOUNT,
            keeper_id: [1u8; 32],
            authority: Pubkey::default(),
            keeper_type: KeeperType::Liquidation,
            mmt_stake: 1_000_000_000_000, // 1000 MMT
            performance_score: 9500, // 95%
            total_operations: 100,
            successful_operations: 95,
            total_rewards_earned: 50_000_000,
            last_operation_slot: 1000,
            status: KeeperStatus::Active,
            specializations: vec![KeeperSpecialization::Liquidations],
            slashing_count: 0,
            registration_slot: 0,
            average_response_time: 0,
            priority_score: 0,
        };
        
        let priority = keeper.calculate_priority();
        assert_eq!(priority, 950_000_000_000); // 1000 MMT * 0.95
    }
}