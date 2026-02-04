//! Bootstrap phase instruction handlers
//!
//! Handles all bootstrap-related instructions including initialization,
//! deposits with MMT rewards, withdrawals with vampire protection, and coverage updates.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    rent::Rent,
    program::{invoke, invoke_signed},
    system_instruction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use spl_token::instruction as token_instruction;

use crate::{
    error::BettingPlatformError,
    integration::bootstrap_enhanced::{
        EnhancedBootstrapCoordinator, BootstrapHaltReason, BootstrapStatus,
        MINIMUM_VIABLE_VAULT, BOOTSTRAP_MMT_ALLOCATION,
    },
    events::{emit_event, EventType, BootstrapDepositEvent, BootstrapCompleteEvent, 
             BootstrapWithdrawalEvent, CoverageUpdatedEvent, BootstrapCompletedEvent},
    integration::events::{BootstrapStartedEvent},
    cpi::system_program::create_pda_account,
    state::CollateralVault,
};

/// Initialize enhanced bootstrap phase
pub fn process_initialize_bootstrap_phase(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    mmt_allocation: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let bootstrap_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock_sysvar)?;
    let current_slot = clock.slot;
    
    // Derive PDA for bootstrap coordinator
    let (bootstrap_pda, bump) = Pubkey::find_program_address(
        &[b"bootstrap_coordinator"],
        program_id,
    );
    
    if bootstrap_account.key != &bootstrap_pda {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Create bootstrap account if needed
    if bootstrap_account.data_is_empty() {
        let space = EnhancedBootstrapCoordinator::SIZE;
        let rent = Rent::from_account_info(rent_sysvar)?;
        let lamports = rent.minimum_balance(space);
        
        create_pda_account(
            authority,
            bootstrap_account,
            space as u64,
            program_id,
            system_program,
            rent_sysvar,
            &[b"bootstrap_coordinator", &[bump]],
        )?;
    }
    
    // Initialize bootstrap coordinator
    let mut coordinator = EnhancedBootstrapCoordinator::default();
    coordinator.initialize(authority.key, current_slot)?;
    
    // Override MMT allocation if specified
    if mmt_allocation > 0 {
        coordinator.mmt_pool_remaining = mmt_allocation;
    }
    
    coordinator.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;
    
    // Initialize vault with $0
    if vault_account.data_is_empty() {
        // Create vault PDA
        let (vault_pda, vault_bump) = Pubkey::find_program_address(
            &[b"collateral_vault"],
            program_id,
        );
        
        let space = CollateralVault::SIZE;
        let rent = Rent::from_account_info(rent_sysvar)?;
        let lamports = rent.minimum_balance(space);
        
        create_pda_account(
            authority,
            vault_account,
            space as u64,
            program_id,
            system_program,
            rent_sysvar,
            &[b"collateral_vault", &[vault_bump]],
        )?;
        
        // Initialize with $0
        let vault = CollateralVault {
            total_deposits: 0,
            total_borrowed: 0,
            depositor_count: 0,
            last_update: clock.unix_timestamp,
        };
        vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    }
    
    msg!("Bootstrap phase initialized with $0 vault and {} MMT allocation", 
         coordinator.mmt_pool_remaining);
    
    // Emit event
    emit_event(EventType::BootstrapInitialized, &BootstrapStartedEvent {
        target_vault: MINIMUM_VIABLE_VAULT,
        incentive_pool: coordinator.mmt_pool_remaining,
    });
    
    Ok(())
}

/// Process bootstrap deposit with MMT rewards
pub fn process_bootstrap_deposit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let bootstrap_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let depositor = next_account_info(account_info_iter)?;
    let depositor_token_account = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let depositor_mmt_account = next_account_info(account_info_iter)?;
    let mmt_treasury = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify depositor signed
    if !depositor.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock_sysvar)?;
    let current_slot = clock.slot;
    
    // Load bootstrap coordinator
    let mut coordinator = EnhancedBootstrapCoordinator::try_from_slice(
        &bootstrap_account.data.borrow()
    )?;
    
    // Check if bootstrap is active
    if coordinator.bootstrap_complete {
        return Err(BettingPlatformError::BootstrapNotActive.into());
    }
    
    if coordinator.is_halted {
        return Err(BettingPlatformError::SystemHalted.into());
    }
    
    // Transfer USDC from depositor to vault
    invoke(
        &token_instruction::transfer(
            token_program.key,
            depositor_token_account.key,
            vault_token_account.key,
            depositor.key,
            &[],
            amount,
        )?,
        &[
            depositor_token_account.clone(),
            vault_token_account.clone(),
            depositor.clone(),
            token_program.clone(),
        ],
    )?;
    
    // Process deposit and calculate MMT reward
    let mmt_reward = coordinator.process_deposit(depositor.key, amount, current_slot)?;
    
    // Transfer MMT rewards if any
    if mmt_reward > 0 {
        // Transfer MMT from treasury to depositor
        let (treasury_pda, treasury_bump) = Pubkey::find_program_address(
            &[b"mmt_treasury"],
            program_id,
        );
        
        invoke_signed(
            &token_instruction::transfer(
                token_program.key,
                mmt_treasury.key,
                depositor_mmt_account.key,
                &treasury_pda,
                &[],
                mmt_reward,
            )?,
            &[
                mmt_treasury.clone(),
                depositor_mmt_account.clone(),
                mmt_treasury.clone(),
                token_program.clone(),
            ],
            &[&[b"mmt_treasury", &[treasury_bump]]],
        )?;
    }
    
    // Update vault balance
    let mut vault = CollateralVault::try_from_slice(&vault_account.data.borrow())?;
    vault.total_deposits = vault.total_deposits
        .checked_add(amount)
        .ok_or(BettingPlatformError::MathOverflow)?;
    vault.depositor_count += 1;
    vault.last_update = clock.unix_timestamp;
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Save coordinator state
    coordinator.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;
    
    msg!("Bootstrap deposit: {} USDC, {} MMT reward, vault: {}", 
         amount, mmt_reward, coordinator.vault);
    
    // Emit event
    emit_event(EventType::BootstrapDeposit, &BootstrapDepositEvent {
        depositor: *depositor.key,
        amount,
        vault_balance: coordinator.vault,
        mmt_earned: mmt_reward,
    });
    
    Ok(())
}

/// Process bootstrap withdrawal with vampire attack check
pub fn process_bootstrap_withdrawal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let bootstrap_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let withdrawer = next_account_info(account_info_iter)?;
    let withdrawer_token_account = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify withdrawer signed
    if !withdrawer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock_sysvar)?;
    let current_slot = clock.slot;
    
    // Load bootstrap coordinator
    let mut coordinator = EnhancedBootstrapCoordinator::try_from_slice(
        &bootstrap_account.data.borrow()
    )?;
    
    // Check vampire attack conditions
    let is_vampire_attack = coordinator.check_vampire_attack(amount, current_slot)?;
    if is_vampire_attack {
        return Err(BettingPlatformError::VampireAttackDetected.into());
    }
    
    // Process withdrawal
    coordinator.process_withdrawal(amount, current_slot)?;
    
    // Transfer USDC from vault to withdrawer
    let (vault_pda, vault_bump) = Pubkey::find_program_address(
        &[b"collateral_vault"],
        program_id,
    );
    
    invoke_signed(
        &token_instruction::transfer(
            token_program.key,
            vault_token_account.key,
            withdrawer_token_account.key,
            &vault_pda,
            &[],
            amount,
        )?,
        &[
            vault_token_account.clone(),
            withdrawer_token_account.clone(),
            vault_account.clone(),
            token_program.clone(),
        ],
        &[&[b"collateral_vault", &[vault_bump]]],
    )?;
    
    // Update vault
    let mut vault = CollateralVault::try_from_slice(&vault_account.data.borrow())?;
    vault.total_deposits = vault.total_deposits
        .checked_sub(amount)
        .ok_or(BettingPlatformError::InsufficientFunds)?;
    vault.last_update = clock.unix_timestamp;
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Save coordinator state
    coordinator.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;
    
    msg!("Bootstrap withdrawal: {} USDC, vault: {}, coverage: {}", 
         amount, coordinator.vault, coordinator.coverage_ratio);
    
    // Emit event
    emit_event(EventType::BootstrapWithdrawal, &BootstrapWithdrawalEvent {
        market_id: 0, // Bootstrap phase doesn't have a specific market ID
        user: *withdrawer.key,
        amount,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Update bootstrap coverage ratio
pub fn process_update_bootstrap_coverage(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let bootstrap_account = next_account_info(account_info_iter)?;
    let positions_account = next_account_info(account_info_iter)?; // Account tracking total OI
    
    // Load bootstrap coordinator
    let mut coordinator = EnhancedBootstrapCoordinator::try_from_slice(
        &bootstrap_account.data.borrow()
    )?;
    
    // Get total open interest from positions tracking
    // For now, we'll assume it's passed in the account data
    let total_oi = u64::from_le_bytes(
        positions_account.data.borrow()[0..8].try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?
    );
    
    // Update OI and recalculate coverage
    coordinator.total_oi = total_oi;
    coordinator.update_coverage_ratio()?;
    
    // Save state
    coordinator.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;
    
    msg!("Coverage ratio updated: {} bps (vault: {}, OI: {})", 
         coordinator.coverage_ratio, coordinator.vault, total_oi);
    
    // Emit event
    emit_event(EventType::CoverageUpdated, &CoverageUpdatedEvent {
        market_id: 0, // Bootstrap phase doesn't have a specific market ID
        coverage_ratio: coordinator.coverage_ratio,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Complete bootstrap phase
pub fn process_complete_bootstrap(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let bootstrap_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    // Load bootstrap coordinator
    let mut coordinator = EnhancedBootstrapCoordinator::try_from_slice(
        &bootstrap_account.data.borrow()
    )?;
    
    // Verify authority
    if !authority.is_signer || authority.key != &coordinator.authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if minimum viable vault reached
    if coordinator.vault < MINIMUM_VIABLE_VAULT {
        return Err(BettingPlatformError::InsufficientFunds.into());
    }
    
    // Mark bootstrap as complete
    coordinator.bootstrap_complete = true;
    
    // Save state
    coordinator.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;
    
    msg!("Bootstrap phase completed! Vault: {}, MMT distributed: {}", 
         coordinator.vault, coordinator.mmt_distributed);
    
    // Emit event
    emit_event(EventType::BootstrapCompleted, &BootstrapCompletedEvent {
        market_id: 0, // Bootstrap phase doesn't have a specific market ID
        total_raised: coordinator.vault,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}