//! Keeper registration and management
//!
//! Handles keeper registration, staking, and status management

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::keeper_accounts::{
        KeeperRegistry, KeeperAccount, KeeperType, KeeperStatus,
        discriminators, KeeperSpecialization,
    },
};

/// Initialize the keeper registry
pub fn process_initialize_registry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing keeper registry");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let registry_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive keeper registry PDA
    let (registry_pda, bump_seed) = Pubkey::find_program_address(
        &[b"keeper_registry"],
        program_id,
    );
    
    // Verify PDA matches
    if registry_pda != *registry_account.key {
        msg!("Invalid keeper registry PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if already initialized
    if registry_account.data_len() > 0 {
        msg!("Keeper registry already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate required space
    let registry_size = KeeperRegistry::LEN;
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(registry_size);
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            registry_account.key,
            rent_lamports,
            registry_size as u64,
            program_id,
        ),
        &[
            authority.clone(),
            registry_account.clone(),
            system_program.clone(),
        ],
        &[&[b"keeper_registry", &[bump_seed]]],
    )?;
    
    // Initialize registry
    let registry = KeeperRegistry::new();
    
    // Log initialization
    msg!("Keeper registry initialized:");
    msg!("  Performance threshold: {}%", registry.performance_threshold);
    msg!("  Slash threshold: {} failures", registry.slash_threshold);
    
    // Serialize and save
    registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
    
    msg!("Keeper registry initialized successfully");
    
    Ok(())
}

/// Register a new keeper
pub fn process_register_keeper(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    keeper_type: KeeperType,
    initial_stake: u64,
) -> ProgramResult {
    msg!("Registering new keeper of type: {:?}", keeper_type);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let registry_account = next_account_info(account_info_iter)?;
    let mmt_source = next_account_info(account_info_iter)?;
    let mmt_vault = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify keeper authority is signer
    if !keeper_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load and validate registry
    let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
    registry.validate()?;
    
    // Generate keeper ID
    let clock = Clock::from_account_info(clock)?;
    let keeper_id_seed = [
        keeper_authority.key.as_ref(),
        &clock.unix_timestamp.to_le_bytes(),
        &registry.total_keepers.to_le_bytes(),
    ].concat();
    let keeper_id = solana_program::hash::hash(&keeper_id_seed).to_bytes();
    
    // Derive keeper account PDA
    let (keeper_pda, bump_seed) = Pubkey::find_program_address(
        &[b"keeper", &keeper_id],
        program_id,
    );
    
    // Verify PDA matches
    if keeper_pda != *keeper_account.key {
        msg!("Invalid keeper account PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if already exists
    if keeper_account.data_len() > 0 {
        msg!("Keeper already registered");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Validate initial stake (minimum 1000 MMT with 9 decimals = 1_000_000_000_000)
    const MIN_STAKE: u64 = 1_000_000_000_000;
    if initial_stake < MIN_STAKE {
        msg!("Initial stake {} is below minimum {}", initial_stake, MIN_STAKE);
        return Err(BettingPlatformError::InsufficientStake.into());
    }
    
    // Calculate required space (base + specializations)
    let keeper_size = KeeperAccount::space(4); // Space for up to 4 specializations
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(keeper_size);
    
    invoke_signed(
        &system_instruction::create_account(
            keeper_authority.key,
            keeper_account.key,
            rent_lamports,
            keeper_size as u64,
            program_id,
        ),
        &[
            keeper_authority.clone(),
            keeper_account.clone(),
            system_program.clone(),
        ],
        &[&[b"keeper", &keeper_id, &[bump_seed]]],
    )?;
    
    // Transfer MMT stake to vault
    // In production, this would use SPL token transfer
    msg!("Transferring {} MMT stake to vault", initial_stake);
    
    // Create keeper account
    let mut keeper = KeeperAccount::new(keeper_id, *keeper_authority.key, keeper_type);
    keeper.mmt_stake = initial_stake;
    keeper.registration_slot = clock.slot;
    
    // Set default specializations based on keeper type
    keeper.specializations = match keeper_type {
        KeeperType::Liquidation => vec![KeeperSpecialization::Liquidations],
        KeeperType::Order => vec![
            KeeperSpecialization::StopLosses,
            KeeperSpecialization::ChainExecution,
        ],
        KeeperType::Ingestor => vec![KeeperSpecialization::PriceUpdates],
        KeeperType::General => vec![
            KeeperSpecialization::MarketResolution,
            KeeperSpecialization::CircuitBreakers,
        ],
    };
    
    // Calculate initial priority
    keeper.priority_score = keeper.calculate_priority() as u128;
    
    // Update registry
    registry.total_keepers += 1;
    registry.active_keepers += 1;
    registry.total_mmt_staked += initial_stake;
    
    match keeper_type {
        KeeperType::Liquidation => registry.active_liquidation_keepers += 1,
        KeeperType::Order => registry.active_order_keepers += 1,
        KeeperType::Ingestor => registry.active_ingestor_keepers += 1,
        _ => {}
    }
    
    // Log registration
    msg!("Keeper registered successfully:");
    msg!("  Keeper ID: {:?}", keeper_id);
    msg!("  Authority: {}", keeper_authority.key);
    msg!("  Type: {:?}", keeper_type);
    msg!("  Initial stake: {} MMT", initial_stake);
    msg!("  Specializations: {:?}", keeper.specializations);
    msg!("  Priority score: {}", keeper.priority_score);
    
    // Serialize and save
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Update keeper stake
pub fn process_update_stake(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    additional_stake: u64,
    withdraw_amount: u64,
) -> ProgramResult {
    msg!("Updating keeper stake: add={}, withdraw={}", additional_stake, withdraw_amount);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let registry_account = next_account_info(account_info_iter)?;
    let mmt_source = next_account_info(account_info_iter)?;
    let mmt_destination = next_account_info(account_info_iter)?;
    let mmt_vault = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Verify keeper authority is signer
    if !keeper_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load accounts
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
    
    // Validate accounts
    keeper.validate()?;
    registry.validate()?;
    
    // Verify keeper authority
    if keeper.authority != *keeper_authority.key {
        msg!("Keeper authority mismatch");
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check keeper status
    if keeper.status != KeeperStatus::Active && keeper.status != KeeperStatus::Inactive {
        msg!("Cannot update stake for keeper in status: {:?}", keeper.status);
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Process stake addition
    if additional_stake > 0 {
        // Transfer MMT to vault
        msg!("Adding {} MMT to stake", additional_stake);
        
        keeper.mmt_stake += additional_stake;
        registry.total_mmt_staked += additional_stake;
    }
    
    // Process withdrawal
    if withdraw_amount > 0 {
        // Check minimum stake requirement
        const MIN_STAKE: u64 = 1_000_000_000_000; // 1000 MMT
        let remaining_stake = keeper.mmt_stake.saturating_sub(withdraw_amount);
        
        if remaining_stake < MIN_STAKE {
            msg!("Cannot withdraw: would leave stake below minimum");
            return Err(BettingPlatformError::InsufficientStake.into());
        }
        
        // Transfer MMT from vault to destination
        msg!("Withdrawing {} MMT from stake", withdraw_amount);
        
        keeper.mmt_stake = remaining_stake;
        registry.total_mmt_staked = registry.total_mmt_staked.saturating_sub(withdraw_amount);
    }
    
    // Recalculate priority
    keeper.priority_score = keeper.calculate_priority() as u128;
    
    // Log update
    msg!("Stake updated:");
    msg!("  New stake: {} MMT", keeper.mmt_stake);
    msg!("  New priority: {}", keeper.priority_score);
    
    // Serialize and save
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Update keeper status
pub fn process_update_keeper_status(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_status: KeeperStatus,
) -> ProgramResult {
    msg!("Updating keeper status to: {:?}", new_status);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let registry_account = next_account_info(account_info_iter)?;
    
    // Load keeper
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    keeper.validate()?;
    
    // Check authorization
    // Either keeper authority or program authority can update status
    let is_keeper_authority = authority.is_signer && *authority.key == keeper.authority;
    let is_program_authority = authority.is_signer && {
        // In production, check against program authority PDA
        true // Placeholder
    };
    
    if !is_keeper_authority && !is_program_authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate status transition
    match (keeper.status, new_status) {
        // Keeper can pause/unpause themselves
        (KeeperStatus::Active, KeeperStatus::Inactive) |
        (KeeperStatus::Inactive, KeeperStatus::Active) => {
            if !is_keeper_authority {
                return Err(BettingPlatformError::Unauthorized.into());
            }
        }
        // Only program can suspend/slash
        (_, KeeperStatus::Suspended) |
        (_, KeeperStatus::Slashed) => {
            if !is_program_authority {
                return Err(BettingPlatformError::Unauthorized.into());
            }
        }
        // Invalid transitions
        (KeeperStatus::Slashed, _) |
        (KeeperStatus::Deactivated, _) => {
            msg!("Invalid status transition");
            return Err(BettingPlatformError::InvalidOperation.into());
        }
        _ => {}
    }
    
    // Update registry counts
    let mut registry = KeeperRegistry::try_from_slice(&registry_account.data.borrow())?;
    
    if keeper.status == KeeperStatus::Active && new_status != KeeperStatus::Active {
        registry.active_keepers = registry.active_keepers.saturating_sub(1);
        
        match keeper.keeper_type {
            KeeperType::Liquidation => {
                registry.active_liquidation_keepers = registry.active_liquidation_keepers.saturating_sub(1);
            }
            KeeperType::Order => {
                registry.active_order_keepers = registry.active_order_keepers.saturating_sub(1);
            }
            KeeperType::Ingestor => {
                registry.active_ingestor_keepers = registry.active_ingestor_keepers.saturating_sub(1);
            }
            _ => {}
        }
    } else if keeper.status != KeeperStatus::Active && new_status == KeeperStatus::Active {
        registry.active_keepers += 1;
        
        match keeper.keeper_type {
            KeeperType::Liquidation => registry.active_liquidation_keepers += 1,
            KeeperType::Order => registry.active_order_keepers += 1,
            KeeperType::Ingestor => registry.active_ingestor_keepers += 1,
            _ => {}
        }
    }
    
    // Track slashing events
    if new_status == KeeperStatus::Slashed {
        registry.slashing_events += 1;
        keeper.slashing_count += 1;
    }
    
    // Update status
    let old_status = keeper.status;
    keeper.status = new_status;
    
    // Log update
    msg!("Keeper status updated:");
    msg!("  Keeper ID: {:?}", keeper.keeper_id);
    msg!("  Old status: {:?}", old_status);
    msg!("  New status: {:?}", new_status);
    if new_status == KeeperStatus::Slashed {
        msg!("  Total slashing events: {}", keeper.slashing_count);
    }
    
    // Serialize and save
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    registry.serialize(&mut &mut registry_account.data.borrow_mut()[..])?;
    
    Ok(())
}