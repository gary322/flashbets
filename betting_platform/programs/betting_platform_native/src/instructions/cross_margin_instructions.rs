//! Cross-Margin Integration Instructions
//!
//! Production-ready instructions for cross-margin functionality

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    error::BettingPlatformError,
    margin::cross_margin::{CrossMarginAccount, CrossMarginMode, CrossMarginCalculator},
            collateral: 0,    state::{Position, VersePDA, UserMap},
    account_validation::AccountValidator,
    pda,
};

/// Cross-margin instruction data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum CrossMarginInstruction {
    /// Enable cross-margin for user
    /// 
    /// Accounts:
    /// 0. [signer] User
    /// 1. [writable] Cross-margin account (PDA)
    /// 2. [] System program
    EnableCrossMargin {
        mode: CrossMarginMode,
    },
    
    /// Update cross-margin mode
    /// 
    /// Accounts:
    /// 0. [signer] User
    /// 1. [writable] Cross-margin account
    UpdateCrossMarginMode {
        new_mode: CrossMarginMode,
    },
    
    /// Recalculate cross-margin requirements
    /// 
    /// Accounts:
    /// 0. [signer] User
    /// 1. [writable] Cross-margin account
    /// 2. [] User map
    /// 3..N. [] User positions (variable)
    RecalculateCrossMargin,
    
    /// Apply cross-margin to new position
    /// 
    /// Accounts:
    /// 0. [signer] User
    /// 1. [writable] Cross-margin account
    /// 2. [writable] Position account
    /// 3. [] Verse account
    ApplyCrossMarginToPosition {
        position_index: u8,
    },
}

/// Enable cross-margin for a user
pub fn process_enable_cross_margin<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    mode: CrossMarginMode,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let user = AccountValidator::next_signer(account_iter)?;
    let cross_margin_account = AccountValidator::next_writable(account_iter)?;
    let system_program = AccountValidator::next_account(account_iter)?;
    
    // Derive cross-margin PDA
    let (expected_address, bump) = pda::derive_cross_margin_account(
        program_id,
        user.key,
    );
    
    if cross_margin_account.key != &expected_address {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create account if needed
    if cross_margin_account.data_is_empty() {
        let space = std::mem::size_of::<CrossMarginAccount>();
        let rent = solana_program::rent::Rent::get()?;
        let rent_lamports = rent.minimum_balance(space);
        
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::create_account(
                user.key,
                cross_margin_account.key,
                rent_lamports,
                space as u64,
                program_id,
            ),
            &[
                user.clone(),
                cross_margin_account.clone(),
                system_program.clone(),
            ],
            &[&[b"cross_margin", user.key.as_ref(), &[bump]]],
        )?;
    }
    
    // Initialize cross-margin account
    let mut cross_margin = CrossMarginAccount::new(*user.key, mode);
    cross_margin.serialize(&mut &mut cross_margin_account.data.borrow_mut()[..])?;
    
    msg!("Cross-margin enabled for user {} with mode {:?}", user.key, mode);
    
    Ok(())
}

/// Update cross-margin mode
pub fn process_update_cross_margin_mode<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    new_mode: CrossMarginMode,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let user = AccountValidator::next_signer(account_iter)?;
    let cross_margin_account = AccountValidator::next_writable(account_iter)?;
    
    // Validate PDA
    let (expected_address, _) = pda::derive_cross_margin_account(
        program_id,
        user.key,
    );
    
    if cross_margin_account.key != &expected_address {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load and update
    let mut cross_margin = CrossMarginAccount::try_from_slice(
        &cross_margin_account.data.borrow()
    )?;
    
    // Validate authority
    if cross_margin.authority != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Update mode
    let old_mode = cross_margin.mode;
    cross_margin.mode = new_mode;
    cross_margin.last_update = solana_program::clock::Clock::get()?.slot;
    
    cross_margin.serialize(&mut &mut cross_margin_account.data.borrow_mut()[..])?;
    
    msg!("Cross-margin mode updated from {:?} to {:?}", old_mode, new_mode);
    
    Ok(())
}

/// Recalculate cross-margin requirements based on all positions
pub fn process_recalculate_cross_margin<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let user = AccountValidator::next_signer(account_iter)?;
    let cross_margin_account = AccountValidator::next_writable(account_iter)?;
    let user_map = AccountValidator::next_account(account_iter)?;
    
    // Remaining accounts are positions
    let position_accounts: Vec<_> = account_iter.collect();
    
    // Validate cross-margin account
    let (expected_address, _) = pda::derive_cross_margin_account(
        program_id,
        user.key,
    );
    
    if cross_margin_account.key != &expected_address {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load accounts
    let mut cross_margin = CrossMarginAccount::try_from_slice(
        &cross_margin_account.data.borrow()
    )?;
    
    let user_map_data = UserMap::try_from_slice(&user_map.data.borrow())?;
    
    // Load all positions
    let mut positions = Vec::new();
    for (i, account) in position_accounts.iter().enumerate() {
        if i >= user_map_data.positions.len() {
            break;
        }
        
        let position = Position::try_from_slice(&account.data.borrow())?;
        positions.push(position);
    }
    
    // Calculate cross-margin requirements
    let calculator = CrossMarginCalculator::new();
    let (margin_required, efficiency_gain) = calculator.calculate_cross_margin_requirements(
        &positions,
        &cross_margin.mode,
    )?;
    
    // Update cross-margin account
    cross_margin.total_margin_required = margin_required;
    cross_margin.margin_efficiency_gain = efficiency_gain;
    cross_margin.position_count = positions.len() as u32;
    cross_margin.last_update = solana_program::clock::Clock::get()?.slot;
    
    // Calculate totals
    let total_notional: u64 = positions.iter().map(|p| p.size).sum();
    let total_isolated_margin: u64 = positions.iter().map(|p| p.margin).sum();
            collateral: 0,    
    cross_margin.total_notional = total_notional;
    
    // Serialize back
    cross_margin.serialize(&mut &mut cross_margin_account.data.borrow_mut()[..])?;
    
    msg!(
        "Cross-margin recalculated: {} positions, {} margin required ({}% efficiency)",
        positions.len(),
        margin_required,
        (efficiency_gain * 100) / total_isolated_margin
    );
    
    Ok(())
}

/// Apply cross-margin benefits to a specific position
pub fn process_apply_cross_margin_to_position<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    position_index: u8,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let user = AccountValidator::next_signer(account_iter)?;
    let cross_margin_account = AccountValidator::next_writable(account_iter)?;
    let position_account = AccountValidator::next_writable(account_iter)?;
    let verse_account = AccountValidator::next_account(account_iter)?;
    
    // Validate accounts
    let (expected_cm_address, _) = pda::derive_cross_margin_account(
        program_id,
        user.key,
    );
    
    if cross_margin_account.key != &expected_cm_address {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load accounts
    let cross_margin = CrossMarginAccount::try_from_slice(
        &cross_margin_account.data.borrow()
    )?;
    
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    let verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
    
    // Validate position ownership
    if position.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Apply cross-margin benefits if enabled
    if cross_margin.mode != CrossMarginMode::Isolated {
        let efficiency_ratio = cross_margin.margin_efficiency_gain as f64 / 
                              cross_margin.total_margin_required as f64;
        
        // Reduce margin requirement based on efficiency
        let reduced_margin = position.margin - 
            ((position.margin as f64 * efficiency_ratio * 0.5) as u64); // Cap at 50% reduction
        
        position.margin = reduced_margin.max(position.size / 40); // Minimum 2.5% margin
        position.cross_margin_enabled = true;
        
        // Serialize back
        position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
        
        msg!(
            "Cross-margin applied to position {}: margin reduced from {} to {}",
            position_index,
            position.margin + ((position.margin as f64 * efficiency_ratio * 0.5) as u64),
            position.margin
        );
    }
    
    Ok(())
}

// Helper functions

fn pda::derive_cross_margin_account(
    program_id: &Pubkey,
    user: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"cross_margin", user.as_ref()],
        program_id,
    )
}