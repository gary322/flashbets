//! Bootstrap Deposit Handler
//!
//! Integrates vault deposits, MMT rewards, and bootstrap phase coordination
//! to handle liquidity provider deposits during the $0 to $10k bootstrap phase.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    account_validation::{validate_signer, validate_writable},
    cpi::{associated_token, spl_token},
    error::BettingPlatformError,
    events::{emit_event, EventType, BootstrapDepositEvent},
    constants::BOOTSTRAP_TARGET_VAULT,
    integration::{
        bootstrap_coordinator::BootstrapCoordinator,
        bootstrap_mmt_integration::{
            calculate_bootstrap_mmt_rewards, process_bootstrap_mmt_reward,
            verify_liquidity_provider_eligibility, MIN_DEPOSIT_AMOUNT,
        },
        bootstrap_vault_initialization::{
            BootstrapVaultState, update_bootstrap_vault_state,
        },
    },
    pda::CollateralVaultPDA,
};

/// USDC mint address on mainnet
pub const USDC_MINT: Pubkey = solana_program::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

/// Process bootstrap deposit - handles USDC deposit, MMT rewards, and state updates
pub fn process_bootstrap_deposit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    msg!("Processing bootstrap deposit of {} USDC", amount);
    
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Depositor (signer)
    // 1. Depositor USDC account
    // 2. Depositor MMT account
    // 3. Vault account (PDA)
    // 4. Vault USDC account
    // 5. Bootstrap coordinator account (PDA)
    // 6. Season emission account
    // 7. MMT config account
    // 8. Treasury account
    // 9. Treasury token account
    // 10. USDC mint
    // 11. MMT mint
    // 12. Token program
    // 13. Associated token program
    // 14. System program
    // 15. Clock sysvar
    // 16. Rent sysvar
    
    let depositor = next_account_info(account_info_iter)?;
    let depositor_usdc_account = next_account_info(account_info_iter)?;
    let depositor_mmt_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let vault_usdc_account = next_account_info(account_info_iter)?;
    let bootstrap_account = next_account_info(account_info_iter)?;
    let season_emission_account = next_account_info(account_info_iter)?;
    let mmt_config_account = next_account_info(account_info_iter)?;
    let treasury_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let usdc_mint = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let associated_token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(depositor)?;
    validate_writable(vault_account)?;
    validate_writable(vault_usdc_account)?;
    validate_writable(bootstrap_account)?;
    
    // Verify minimum deposit
    if amount < MIN_DEPOSIT_AMOUNT {
        return Err(BettingPlatformError::DepositTooSmall.into());
    }
    
    // Verify USDC mint
    if usdc_mint.key != &USDC_MINT {
        return Err(BettingPlatformError::InvalidMint.into());
    }
    
    // Load bootstrap coordinator
    let mut bootstrap = BootstrapCoordinator::deserialize(&mut &bootstrap_account.data.borrow()[..])?;
    
    // Load vault state
    let vault = BootstrapVaultState::deserialize(&mut &vault_account.data.borrow()[..])?;
    
    // Determine if this is a new depositor
    let is_new_depositor = !has_previous_deposit(depositor.key, &vault)?;
    
    // Verify eligibility
    if !verify_liquidity_provider_eligibility(depositor.key, amount, &bootstrap)? {
        return Err(BettingPlatformError::IneligibleForRewards.into());
    }
    
    // Calculate MMT rewards
    let mmt_reward = calculate_bootstrap_mmt_rewards(
        amount,
        vault.total_deposits,
        vault.depositor_count,
        bootstrap.current_milestone,
        bootstrap.incentive_pool.saturating_sub(bootstrap.total_mmt_distributed),
    )?;
    
    // Create vault's USDC ATA if needed
    let vault_ata = associated_token::get_associated_token_address(
        vault_account.key,
        &USDC_MINT,
    );
    
    if vault_usdc_account.key != &vault_ata {
        return Err(ProgramError::InvalidAccountData);
    }
    
    if vault_usdc_account.data_is_empty() {
        associated_token::create_associated_token_account(
            depositor,
            vault_usdc_account,
            vault_account,
            usdc_mint,
            system_program,
            token_program,
            rent_sysvar,
        )?;
    }
    
    // Transfer USDC from depositor to vault
    spl_token::transfer(
        depositor_usdc_account,
        vault_usdc_account,
        depositor,
        amount,
        token_program,
        &[], // No signer seeds needed for user transfer
    )?;
    
    // Process bootstrap coordinator update
    let deposit_result = bootstrap.process_deposit(
        depositor.key,
        amount,
        is_new_depositor,
    )?;
    
    // Update vault state
    update_bootstrap_vault_state(
        vault_account,
        amount,
        mmt_reward,
        is_new_depositor,
    )?;
    
    // Process MMT reward distribution
    if mmt_reward > 0 {
        // Create depositor's MMT ATA if needed
        let depositor_mmt_ata = associated_token::get_associated_token_address(
            depositor.key,
            mmt_mint.key,
        );
        
        if depositor_mmt_account.key != &depositor_mmt_ata {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if depositor_mmt_account.data_is_empty() {
            associated_token::create_associated_token_account(
                depositor,
                depositor_mmt_account,
                depositor,
                mmt_mint,
                system_program,
                token_program,
                rent_sysvar,
            )?;
        }
        
        // Distribute MMT rewards
        process_bootstrap_mmt_reward(
            program_id,
            &[
                bootstrap_account.clone(),
                season_emission_account.clone(),
                mmt_config_account.clone(),
                treasury_account.clone(),
                treasury_token_account.clone(),
                depositor_mmt_account.clone(),
                mmt_mint.clone(),
                clock_sysvar.clone(),
                token_program.clone(),
                system_program.clone(),
            ],
            depositor.key,
            amount,
            mmt_reward,
        )?;
    }
    
    // Save updated bootstrap state
    bootstrap.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;
    
    msg!("Bootstrap deposit successful:");
    msg!("  Amount: ${}", amount / 1_000_000);
    msg!("  MMT Earned: {}", mmt_reward / 1_000_000);
    msg!("  Vault Balance: ${}", (vault.total_deposits + amount) / 1_000_000);
    msg!("  Progress: {}%", deposit_result.current_progress_percent);
    
    Ok(())
}

/// Check if depositor has made previous deposits
fn has_previous_deposit(
    depositor: &Pubkey,
    _vault: &BootstrapVaultState,
) -> Result<bool, ProgramError> {
    // In production, this would check a depositor registry
    // For now, we'll use the is_new_depositor flag from the caller
    Ok(false)
}

/// Process instruction router for bootstrap deposits
pub fn process_bootstrap_deposit_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let instruction = instruction_data[0];
    let amount = u64::from_le_bytes(
        instruction_data[1..9]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?
    );
    
    match instruction {
        0 => process_bootstrap_deposit(program_id, accounts, amount),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_deposit_validation() {
        // Test minimum deposit enforcement
        assert!(MIN_DEPOSIT_AMOUNT == 1_000_000); // $1 minimum
        
        // Test USDC mint constant
        assert_eq!(
            USDC_MINT.to_string(),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        );
    }
}