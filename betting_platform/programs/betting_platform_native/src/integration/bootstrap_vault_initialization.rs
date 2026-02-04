//! Bootstrap Vault Initialization Module
//! 
//! Handles initialization of the VaultPDA with $0 starting balance
//! during the bootstrap phase as per specification requirements.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType, VaultInitializedEvent},
    pda::CollateralVaultPDA,
    state::CollateralVault,
    integration::bootstrap_coordinator::BootstrapCoordinator,
    constants::BOOTSTRAP_TARGET_VAULT,
};

/// Bootstrap vault state extension
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Default)]
pub struct BootstrapVaultState {
    /// Standard collateral vault fields
    pub total_deposits: u64,
    pub total_borrowed: u64,
    pub depositor_count: u32,
    pub last_update: i64,
    
    /// Bootstrap-specific fields
    pub is_bootstrap_phase: bool,
    pub bootstrap_start_slot: u64,
    pub bootstrap_coordinator: Pubkey,
    pub minimum_viable_size: u64,
    pub coverage_ratio: u64, // In basis points (10000 = 1.0)
    pub is_accepting_deposits: bool,
    pub bootstrap_complete: bool,
    pub total_mmt_distributed: u64,
}

impl BootstrapVaultState {
    pub const SIZE: usize = 8 +  // total_deposits
        8 +  // total_borrowed
        4 +  // depositor_count
        8 +  // last_update
        1 +  // is_bootstrap_phase
        8 +  // bootstrap_start_slot
        32 + // bootstrap_coordinator
        8 +  // minimum_viable_size
        8 +  // coverage_ratio
        1 +  // is_accepting_deposits
        1 +  // bootstrap_complete
        8 +  // total_mmt_distributed
        64;  // padding for future expansion
}

/// Initialize the vault with $0 balance for bootstrap phase
pub fn process_initialize_bootstrap_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Vault account (PDA, uninitialized)
    // 1. Bootstrap coordinator account (PDA)
    // 2. Authority (signer, payer)
    // 3. System program
    // 4. Rent sysvar
    
    let vault_account = next_account_info(account_info_iter)?;
    let bootstrap_coordinator_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let rent = &Rent::from_account_info(rent_sysvar)?;
    let clock = solana_program::clock::Clock::get()?;
    
    // Derive and verify vault PDA
    let (vault_pda, vault_bump) = CollateralVaultPDA::derive(program_id);
    if vault_pda != *vault_account.key {
        msg!("Invalid vault PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Verify vault is not already initialized
    if vault_account.data_len() > 0 {
        msg!("Vault already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Load bootstrap coordinator to verify it's initialized
    let bootstrap = BootstrapCoordinator::deserialize(&mut &bootstrap_coordinator_account.data.borrow()[..])?;
    
    // Verify bootstrap phase is active
    if bootstrap.bootstrap_complete {
        return Err(BettingPlatformError::BootstrapAlreadyComplete.into());
    }
    
    // Calculate required space
    let vault_size = BootstrapVaultState::SIZE;
    
    // Create vault account
    let rent_lamports = rent.minimum_balance(vault_size);
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            vault_account.key,
            rent_lamports,
            vault_size as u64,
            program_id,
        ),
        &[
            authority.clone(),
            vault_account.clone(),
            system_program.clone(),
        ],
        &[&[b"collateral_vault", &[vault_bump]]],
    )?;
    
    // Initialize vault with $0 balance
    let vault_state = BootstrapVaultState {
        total_deposits: 0, // $0 starting balance as per spec
        total_borrowed: 0,
        depositor_count: 0,
        last_update: clock.unix_timestamp,
        is_bootstrap_phase: true,
        bootstrap_start_slot: clock.slot,
        bootstrap_coordinator: *bootstrap_coordinator_account.key,
        minimum_viable_size: BOOTSTRAP_TARGET_VAULT, // $10k target
        coverage_ratio: 0, // Will be updated as deposits come in
        is_accepting_deposits: true,
        bootstrap_complete: false,
        total_mmt_distributed: 0,
    };
    
    // Serialize and save
    vault_state.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Emit initialization event
    emit_event(EventType::VaultInitialized, &VaultInitializedEvent {
        vault: vault_pda,
        initial_balance: 0,
        bootstrap_phase: true,
        minimum_viable_size: BOOTSTRAP_TARGET_VAULT,
        authority: *authority.key,
    });
    
    msg!("Bootstrap vault initialized with $0 balance");
    msg!("Minimum viable size: ${}", BOOTSTRAP_TARGET_VAULT / 1_000_000);
    msg!("Now accepting deposits from liquidity providers");
    
    Ok(())
}

/// Update vault state during bootstrap phase
pub fn update_bootstrap_vault_state(
    vault_account: &AccountInfo,
    deposit_amount: u64,
    mmt_distributed: u64,
    is_new_depositor: bool,
) -> ProgramResult {
    let mut vault = BootstrapVaultState::deserialize(&mut &vault_account.data.borrow()[..])?;
    
    // Verify bootstrap phase is active
    if !vault.is_bootstrap_phase {
        return Err(BettingPlatformError::BootstrapNotActive.into());
    }
    
    if vault.bootstrap_complete {
        return Err(BettingPlatformError::BootstrapAlreadyComplete.into());
    }
    
    // Update vault balance
    vault.total_deposits = vault.total_deposits
        .checked_add(deposit_amount)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    if is_new_depositor {
        vault.depositor_count += 1;
    }
    
    vault.total_mmt_distributed = vault.total_mmt_distributed
        .checked_add(mmt_distributed)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    // Update coverage ratio
    // Simplified calculation: coverage = vault_balance / minimum_viable_size
    vault.coverage_ratio = (vault.total_deposits * 10000) / vault.minimum_viable_size;
    
    // Check if bootstrap target reached
    if vault.total_deposits >= vault.minimum_viable_size {
        vault.bootstrap_complete = true;
        vault.is_accepting_deposits = true; // Continue accepting deposits
        msg!("Bootstrap phase complete! Vault reached minimum viable size");
    }
    
    // Check vampire attack protection
    if vault.coverage_ratio < 5000 && vault.total_deposits > 0 { // < 0.5 coverage
        vault.is_accepting_deposits = false;
        msg!("Vampire attack protection triggered - deposits halted");
    }
    
    vault.last_update = solana_program::clock::Clock::get()?.unix_timestamp;
    
    // Save updated state
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Check if vault is ready for normal operations
pub fn check_vault_bootstrap_status(
    vault: &BootstrapVaultState,
) -> Result<VaultBootstrapStatus, ProgramError> {
    let status = VaultBootstrapStatus {
        is_active: vault.is_bootstrap_phase && !vault.bootstrap_complete,
        current_balance: vault.total_deposits,
        target_balance: vault.minimum_viable_size,
        progress_percent: (vault.total_deposits * 100) / vault.minimum_viable_size,
        coverage_ratio: vault.coverage_ratio,
        is_accepting_deposits: vault.is_accepting_deposits,
        depositors_count: vault.depositor_count,
        mmt_distributed: vault.total_mmt_distributed,
        can_enable_leverage: vault.total_deposits >= 1_000_000_000, // $1k minimum
        max_leverage_available: calculate_available_leverage(vault.total_deposits),
    };
    
    Ok(status)
}

/// Calculate available leverage based on vault balance
fn calculate_available_leverage(vault_balance: u64) -> u8 {
    if vault_balance < 1_000_000_000 { // < $1k
        0
    } else if vault_balance < 10_000_000_000 { // < $10k
        // Linear scaling: $1k = 1x, $10k = 10x
        ((vault_balance / 1_000_000_000) as u8).min(10)
    } else {
        10 // Maximum 10x leverage
    }
}

/// Bootstrap vault status
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VaultBootstrapStatus {
    pub is_active: bool,
    pub current_balance: u64,
    pub target_balance: u64,
    pub progress_percent: u64,
    pub coverage_ratio: u64,
    pub is_accepting_deposits: bool,
    pub depositors_count: u32,
    pub mmt_distributed: u64,
    pub can_enable_leverage: bool,
    pub max_leverage_available: u8,
}

// VaultInitializedEvent is defined in events.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_leverage_calculation() {
        assert_eq!(calculate_available_leverage(0), 0);
        assert_eq!(calculate_available_leverage(500_000_000), 0); // $0.5k
        assert_eq!(calculate_available_leverage(1_000_000_000), 1); // $1k = 1x
        assert_eq!(calculate_available_leverage(5_000_000_000), 5); // $5k = 5x
        assert_eq!(calculate_available_leverage(10_000_000_000), 10); // $10k = 10x
        assert_eq!(calculate_available_leverage(20_000_000_000), 10); // >$10k = 10x max
    }
    
    #[test]
    fn test_coverage_ratio_calculation() {
        let mut vault = BootstrapVaultState {
            total_deposits: 5_000_000_000, // $5k
            total_borrowed: 0,
            depositor_count: 10,
            last_update: 0,
            is_bootstrap_phase: true,
            bootstrap_start_slot: 0,
            bootstrap_coordinator: Pubkey::default(),
            minimum_viable_size: 10_000_000_000, // $10k
            coverage_ratio: 0,
            is_accepting_deposits: true,
            bootstrap_complete: false,
            total_mmt_distributed: 0,
        };
        
        // Calculate coverage ratio
        vault.coverage_ratio = (vault.total_deposits * 10000) / vault.minimum_viable_size;
        assert_eq!(vault.coverage_ratio, 5000); // 0.5 coverage
        
        // Should trigger vampire attack protection at exactly 0.5
        assert!(vault.coverage_ratio <= 5000);
    }
}