//! Fee distribution logic
//!
//! Distributes collected fees according to Part 7 specification:
//! - 70% to vault (insurance)
//! - 20% to MMT holders (emission rewards)
//! - 10% burn

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use spl_token::instruction as token_instruction;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    fees::{FEE_TO_VAULT_BPS, FEE_TO_MMT_BPS, FEE_TO_BURN_BPS},
    state::CollateralVault,
    mmt::state::TreasuryAccount,
};

/// Fee distribution result
#[derive(Debug, Clone, Copy)]
pub struct FeeDistribution {
    pub total_fee: u64,
    pub vault_amount: u64,
    pub mmt_amount: u64,
    pub burn_amount: u64,
}

/// Calculate fee distribution amounts
pub fn calculate_fee_distribution(total_fee: u64) -> Result<FeeDistribution, ProgramError> {
    // Validate distribution ratios sum to 10000 (100%)
    let total_bps = FEE_TO_VAULT_BPS + FEE_TO_MMT_BPS + FEE_TO_BURN_BPS;
    if total_bps != 10000 {
        msg!("Invalid fee distribution ratios: {} != 10000", total_bps);
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate individual amounts
    let vault_amount = (total_fee * FEE_TO_VAULT_BPS as u64) / 10000;
    let mmt_amount = (total_fee * FEE_TO_MMT_BPS as u64) / 10000;
    let burn_amount = (total_fee * FEE_TO_BURN_BPS as u64) / 10000;
    
    // Handle rounding by giving remainder to vault
    let distributed = vault_amount + mmt_amount + burn_amount;
    let remainder = total_fee.saturating_sub(distributed);
    let final_vault_amount = vault_amount + remainder;
    
    Ok(FeeDistribution {
        total_fee,
        vault_amount: final_vault_amount,
        mmt_amount,
        burn_amount,
    })
}

/// Distribute fees according to the 70/20/10 split
pub fn distribute_fees<'a>(
    program_id: &Pubkey,
    fee_payer: &AccountInfo<'a>,
    fee_token_account: &AccountInfo<'a>,
    vault_account: &AccountInfo<'a>,
    vault_token_account: &AccountInfo<'a>,
    mmt_treasury: &AccountInfo<'a>,
    mmt_treasury_token: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    total_fee: u64,
) -> ProgramResult {
    let distribution = calculate_fee_distribution(total_fee)?;
    
    msg!("Distributing fees: total={}, vault={}, mmt={}, burn={}", 
         distribution.total_fee,
         distribution.vault_amount,
         distribution.mmt_amount,
         distribution.burn_amount);
    
    // 1. Transfer 70% to vault
    if distribution.vault_amount > 0 {
        invoke(
            &token_instruction::transfer(
                token_program.key,
                fee_token_account.key,
                vault_token_account.key,
                fee_payer.key,
                &[],
                distribution.vault_amount,
            )?,
            &[
                fee_token_account.clone(),
                vault_token_account.clone(),
                fee_payer.clone(),
                token_program.clone(),
            ],
        )?;
        
        // Update vault state
        let mut vault = CollateralVault::try_from_slice(&vault_account.data.borrow())?;
        vault.total_deposits = vault.total_deposits
            .checked_add(distribution.vault_amount)
            .ok_or(BettingPlatformError::MathOverflow)?;
        vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    }
    
    // 2. Transfer 20% to MMT treasury for emissions
    if distribution.mmt_amount > 0 {
        invoke(
            &token_instruction::transfer(
                token_program.key,
                fee_token_account.key,
                mmt_treasury_token.key,
                fee_payer.key,
                &[],
                distribution.mmt_amount,
            )?,
            &[
                fee_token_account.clone(),
                mmt_treasury_token.clone(),
                fee_payer.clone(),
                token_program.clone(),
            ],
        )?;
        
        // Update MMT treasury state
        let mut treasury = TreasuryAccount::try_from_slice(&mmt_treasury.data.borrow())?;
        treasury.balance = treasury.balance
            .checked_add(distribution.mmt_amount)
            .ok_or(BettingPlatformError::MathOverflow)?;
        treasury.serialize(&mut &mut mmt_treasury.data.borrow_mut()[..])?;
    }
    
    // 3. Burn 10%
    if distribution.burn_amount > 0 {
        invoke(
            &token_instruction::burn(
                token_program.key,
                fee_token_account.key,
                token_program.key, // Mint (USDC)
                fee_payer.key,
                &[],
                distribution.burn_amount,
            )?,
            &[
                fee_token_account.clone(),
                token_program.clone(),
                fee_payer.clone(),
            ],
        )?;
    }
    
    msg!("Fee distribution complete");
    Ok(())
}

/// Calculate and distribute maker rewards
/// 
/// Makers who improve the spread receive their portion of the 20% MMT allocation
pub fn distribute_maker_rewards<'a>(
    program_id: &Pubkey,
    mmt_treasury: &AccountInfo<'a>,
    mmt_treasury_token: &AccountInfo<'a>,
    maker_mmt_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    treasury_bump: u8,
    reward_amount: u64,
) -> ProgramResult {
    // Transfer MMT rewards from treasury to maker
    let treasury_seed = b"mmt_treasury";
    let seeds = &[
        treasury_seed.as_ref(),
        &[treasury_bump],
    ];
    
    invoke_signed(
        &token_instruction::transfer(
            token_program.key,
            mmt_treasury_token.key,
            maker_mmt_account.key,
            mmt_treasury.key,
            &[],
            reward_amount,
        )?,
        &[
            mmt_treasury_token.clone(),
            maker_mmt_account.clone(),
            mmt_treasury.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;
    
    msg!("Distributed {} MMT rewards to maker", reward_amount);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fee_distribution_calculation() {
        let total_fee = 10000; // $10 fee
        let distribution = calculate_fee_distribution(total_fee).unwrap();
        
        assert_eq!(distribution.vault_amount, 7000); // 70%
        assert_eq!(distribution.mmt_amount, 2000); // 20%
        assert_eq!(distribution.burn_amount, 1000); // 10%
        assert_eq!(
            distribution.vault_amount + distribution.mmt_amount + distribution.burn_amount,
            total_fee
        );
    }
    
    #[test]
    fn test_fee_distribution_with_remainder() {
        let total_fee = 10001; // Odd amount to test remainder handling
        let distribution = calculate_fee_distribution(total_fee).unwrap();
        
        // Remainder should go to vault
        assert_eq!(distribution.vault_amount, 7001); // 70% + remainder
        assert_eq!(distribution.mmt_amount, 2000); // 20%
        assert_eq!(distribution.burn_amount, 1000); // 10%
        assert_eq!(
            distribution.vault_amount + distribution.mmt_amount + distribution.burn_amount,
            total_fee
        );
    }
}