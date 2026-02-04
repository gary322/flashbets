//! Vault Instructions
//!
//! Entry points for vault operations

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
};

use super::{
    state::{Vault, VaultType, UserDeposit, derive_vault_pda, derive_user_deposit_pda},
    deposits::{process_deposit, mint_deposit_receipt},
    withdrawals::{process_withdrawal, process_emergency_withdrawal},
    yield_generation::{generate_yield, MarketConditions},
    insurance::{fund_insurance_pool, process_insurance_claim, InsuranceFund},
    strategies::execute_strategy,
    accounting::{update_vault_accounting, AccountingOperation},
};

/// Create a new vault
pub fn create_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    vault_id: u128,
    name: [u8; 32],
    vault_type: VaultType,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let admin = next_account_info(account_iter)?;
    let deposit_token = next_account_info(account_iter)?;
    let synthetic_mint = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify admin signature
    if !admin.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Derive and verify PDA
    let (pda, _bump) = derive_vault_pda(program_id, vault_id);
    if pda != *vault_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create vault
    let mut vault = Vault::new(
        vault_id,
        name,
        vault_type,
        *deposit_token.key,
        *synthetic_mint.key,
        *oracle_account.key,
        *admin.key,
    );
    
    vault.created_at = Clock::get()?.unix_timestamp;
    vault.last_update = vault.created_at;
    
    // Serialize and save
    let mut data = vault_account.try_borrow_mut_data()?;
    vault.serialize(&mut &mut data[..])?;
    
    msg!("Created vault {} with type {:?}", vault_id, vault_type);
    
    Ok(())
}

/// Deposit into vault
pub fn deposit_to_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u128,
    lock_period: Option<i64>,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let user = next_account_info(account_iter)?;
    let user_deposit_account = next_account_info(account_iter)?;
    let deposit_token_source = next_account_info(account_iter)?;
    let vault_token_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user signature
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut vault = Vault::deserialize(&mut &vault_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Process deposit
    let user_deposit = process_deposit(
        program_id,
        &mut vault,
        user.key,
        amount,
        &oracle,
        lock_period,
    )?;
    
    // Mint receipt tokens
    mint_deposit_receipt(program_id, &vault, user.key, user_deposit.shares)?;
    
    // Update accounting
    update_vault_accounting(&mut vault, AccountingOperation::Deposit(amount))?;
    
    // Save vault and user deposit
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    let mut user_data = user_deposit_account.try_borrow_mut_data()?;
    user_deposit.serialize(&mut &mut user_data[..])?;
    
    msg!("Deposited {} into vault {}", amount, vault.vault_id);
    
    Ok(())
}

/// Withdraw from vault
pub fn withdraw_from_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    shares: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let user = next_account_info(account_iter)?;
    let user_deposit_account = next_account_info(account_iter)?;
    let withdrawal_destination = next_account_info(account_iter)?;
    let vault_token_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify user signature
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut vault = Vault::deserialize(&mut &vault_account.data.borrow()[..])?;
    let mut user_deposit = UserDeposit::deserialize(&mut &user_deposit_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify ownership
    if user_deposit.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Process withdrawal
    let withdrawal_amount = process_withdrawal(
        program_id,
        &mut vault,
        &mut user_deposit,
        shares,
        &oracle,
    )?;
    
    // Update accounting
    update_vault_accounting(&mut vault, AccountingOperation::Withdrawal(withdrawal_amount))?;
    
    // Transfer tokens to user
    // In production, would execute SPL token transfer
    
    // Save updated accounts
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    user_deposit.serialize(&mut &mut user_deposit_account.data.borrow_mut()[..])?;
    
    msg!("Withdrew {} shares ({} tokens) from vault {}", 
         shares, withdrawal_amount, vault.vault_id);
    
    Ok(())
}

/// Generate yield for vault
pub fn generate_vault_yield(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let keeper = next_account_info(account_iter)?;
    
    // Verify keeper authority
    if !keeper.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut vault = Vault::deserialize(&mut &vault_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Generate yield
    let market_conditions = MarketConditions::default();
    let yield_amount = generate_yield(&mut vault, &oracle, &market_conditions)?;
    
    // Update accounting
    update_vault_accounting(&mut vault, AccountingOperation::YieldGenerated(yield_amount))?;
    
    // Save vault
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    msg!("Generated {} yield for vault {}", yield_amount, vault.vault_id);
    
    Ok(())
}

/// Claim insurance
pub fn claim_vault_insurance(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    claim_amount: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let user = next_account_info(account_iter)?;
    let user_deposit_account = next_account_info(account_iter)?;
    let insurance_payout_account = next_account_info(account_iter)?;
    
    // Verify user signature
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut vault = Vault::deserialize(&mut &vault_account.data.borrow()[..])?;
    let mut user_deposit = UserDeposit::deserialize(&mut &user_deposit_account.data.borrow()[..])?;
    
    // Verify ownership
    if user_deposit.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Create insurance fund tracker
    let mut insurance_fund = InsuranceFund {
        total_value: vault.insurance_fund,
        reserved_amount: 0,
        available_amount: vault.insurance_fund,
        total_claims_paid: 0,
        claim_count: 0,
        coverage_ratio: vault.insurance_fund as f64 / vault.total_value_locked as f64,
        target_coverage_ratio: 0.1,
        premium_rate: 100,
    };
    
    // Process claim
    let payout = process_insurance_claim(
        &mut user_deposit,
        &mut vault,
        &mut insurance_fund,
        claim_amount,
    )?;
    
    // Update vault insurance fund
    vault.insurance_fund = insurance_fund.total_value;
    
    // Transfer payout
    // In production, would execute token transfer
    
    // Save accounts
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    user_deposit.serialize(&mut &mut user_deposit_account.data.borrow_mut()[..])?;
    
    msg!("Processed insurance claim: {} paid out", payout);
    
    Ok(())
}

/// Emergency withdraw
pub fn emergency_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let user = next_account_info(account_iter)?;
    let user_deposit_account = next_account_info(account_iter)?;
    let withdrawal_destination = next_account_info(account_iter)?;
    
    // Verify user signature
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut vault = Vault::deserialize(&mut &vault_account.data.borrow()[..])?;
    let mut user_deposit = UserDeposit::deserialize(&mut &user_deposit_account.data.borrow()[..])?;
    
    // Verify ownership
    if user_deposit.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Process emergency withdrawal
    let amount = process_emergency_withdrawal(&mut vault, &mut user_deposit)?;
    
    // Transfer tokens
    // In production, would execute token transfer
    
    // Save accounts
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    user_deposit.serialize(&mut &mut user_deposit_account.data.borrow_mut()[..])?;
    
    msg!("Emergency withdrawal: {} returned to user", amount);
    
    Ok(())
}

/// Update vault strategy
pub fn update_vault_strategy(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_strategy: super::state::VaultStrategy,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_iter)?;
    let admin = next_account_info(account_iter)?;
    
    // Verify admin
    if !admin.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load vault
    let mut vault = Vault::deserialize(&mut &vault_account.data.borrow()[..])?;
    
    // Verify admin authority
    if vault.admin != *admin.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Update strategy
    vault.strategy = new_strategy;
    vault.last_update = Clock::get()?.unix_timestamp;
    
    // Save vault
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    msg!("Updated vault {} strategy", vault.vault_id);
    
    Ok(())
}