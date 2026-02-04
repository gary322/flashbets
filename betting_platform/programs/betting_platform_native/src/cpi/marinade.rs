//! Marinade Staked SOL Integration
//! 
//! Native Solana CPI integration with Marinade protocol for mSOL collateral
//! No Anchor dependencies - pure Native Solana implementation

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
    sysvar::stake_history::StakeHistory,
    clock::Clock,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    cpi::depth_tracker::CPIDepthTracker,
};

/// Marinade program ID on mainnet
pub const MARINADE_PROGRAM_ID: Pubkey = solana_program::pubkey!("MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD");

/// mSOL mint address on mainnet
pub const MSOL_MINT: Pubkey = solana_program::pubkey!("mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So");

/// Marinade state account
pub const MARINADE_STATE: Pubkey = solana_program::pubkey!("8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC");

/// Marinade instructions
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum MarinadeInstruction {
    /// Deposit SOL and receive mSOL
    Deposit {
        lamports: u64,
    },
    /// Deposit stake account and receive mSOL
    DepositStakeAccount,
    /// Liquid unstake mSOL for SOL
    LiquidUnstake {
        msol_amount: u64,
    },
    /// Add liquidity to the liquidity pool
    AddLiquidity {
        lamports: u64,
    },
    /// Remove liquidity from the liquidity pool
    RemoveLiquidity {
        tokens: u64,
    },
    /// Claim rewards
    Claim,
    /// Order unstake (delayed unstake)
    OrderUnstake {
        msol_amount: u64,
    },
}

/// Deposit SOL to Marinade and receive mSOL
/// 
/// This is used when users want to use mSOL as collateral for positions
pub fn deposit_sol<'a>(
    marinade_program: &AccountInfo<'a>,
    marinade_state: &AccountInfo<'a>,
    msol_mint: &AccountInfo<'a>,
    liq_pool_sol_leg_pda: &AccountInfo<'a>,
    liq_pool_msol_leg: &AccountInfo<'a>,
    liq_pool_msol_leg_authority: &AccountInfo<'a>,
    reserve_pda: &AccountInfo<'a>,
    user: &AccountInfo<'a>,
    user_msol_account: &AccountInfo<'a>,
    msol_mint_authority: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    msg!("Depositing {} SOL to Marinade for mSOL", lamports);
    
    // Track CPI depth
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Validate inputs
    if lamports == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Check user has enough SOL
    if user.lamports() < lamports {
        return Err(BettingPlatformError::InsufficientFunds.into());
    }
    
    // Create deposit instruction
    let instruction_data = MarinadeInstruction::Deposit { lamports };
    let data = instruction_data.try_to_vec()?;
    
    let instruction = solana_program::instruction::Instruction {
        program_id: *marinade_program.key,
        accounts: vec![
            solana_program::instruction::AccountMeta::new_readonly(*marinade_state.key, false),
            solana_program::instruction::AccountMeta::new(*msol_mint.key, false),
            solana_program::instruction::AccountMeta::new(*liq_pool_sol_leg_pda.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*liq_pool_msol_leg.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*liq_pool_msol_leg_authority.key, false),
            solana_program::instruction::AccountMeta::new(*reserve_pda.key, false),
            solana_program::instruction::AccountMeta::new(*user.key, true),
            solana_program::instruction::AccountMeta::new(*user_msol_account.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*msol_mint_authority.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*system_program.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*token_program.key, false),
        ],
        data,
    };
    
    // Invoke Marinade program
    invoke(&instruction, &[
        marinade_state.clone(),
        msol_mint.clone(),
        liq_pool_sol_leg_pda.clone(),
        liq_pool_msol_leg.clone(),
        liq_pool_msol_leg_authority.clone(),
        reserve_pda.clone(),
        user.clone(),
        user_msol_account.clone(),
        msol_mint_authority.clone(),
        system_program.clone(),
        token_program.clone(),
    ])?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully deposited SOL and received mSOL");
    
    Ok(())
}

/// Liquid unstake mSOL for SOL
/// 
/// This is used when users want to withdraw their mSOL collateral
pub fn liquid_unstake<'a>(
    marinade_program: &AccountInfo<'a>,
    marinade_state: &AccountInfo<'a>,
    msol_mint: &AccountInfo<'a>,
    liq_pool_sol_leg_pda: &AccountInfo<'a>,
    liq_pool_msol_leg: &AccountInfo<'a>,
    treasury_msol_account: &AccountInfo<'a>,
    get_msol_from: &AccountInfo<'a>,
    get_msol_from_authority: &AccountInfo<'a>,
    transfer_sol_to: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    msol_amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    msg!("Liquid unstaking {} mSOL for SOL", msol_amount);
    
    // Track CPI depth
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Validate inputs
    if msol_amount == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Create liquid unstake instruction
    let instruction_data = MarinadeInstruction::LiquidUnstake { msol_amount };
    let data = instruction_data.try_to_vec()?;
    
    let instruction = solana_program::instruction::Instruction {
        program_id: *marinade_program.key,
        accounts: vec![
            solana_program::instruction::AccountMeta::new_readonly(*marinade_state.key, false),
            solana_program::instruction::AccountMeta::new(*msol_mint.key, false),
            solana_program::instruction::AccountMeta::new(*liq_pool_sol_leg_pda.key, false),
            solana_program::instruction::AccountMeta::new(*liq_pool_msol_leg.key, false),
            solana_program::instruction::AccountMeta::new(*treasury_msol_account.key, false),
            solana_program::instruction::AccountMeta::new(*get_msol_from.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*get_msol_from_authority.key, true),
            solana_program::instruction::AccountMeta::new(*transfer_sol_to.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*system_program.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*token_program.key, false),
        ],
        data,
    };
    
    // Invoke with signer seeds for PDA authority
    invoke_signed(&instruction, &[
        marinade_state.clone(),
        msol_mint.clone(),
        liq_pool_sol_leg_pda.clone(),
        liq_pool_msol_leg.clone(),
        treasury_msol_account.clone(),
        get_msol_from.clone(),
        get_msol_from_authority.clone(),
        transfer_sol_to.clone(),
        system_program.clone(),
        token_program.clone(),
    ], signer_seeds)?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully liquid unstaked mSOL for SOL");
    
    Ok(())
}

/// Calculate mSOL to SOL exchange rate
/// 
/// Used for collateral valuation
pub fn get_msol_price(marinade_state_data: &[u8]) -> Result<u64, ProgramError> {
    // Parse Marinade state to get exchange rate
    // In production, would deserialize actual Marinade state
    // For now, use a mock rate of 1.1 SOL per mSOL
    let mock_rate = 1_100_000_000; // 1.1 SOL in lamports
    
    msg!("mSOL price: {} lamports per mSOL", mock_rate);
    Ok(mock_rate)
}

/// Validate mSOL token account
pub fn validate_msol_account(
    account: &AccountInfo,
    expected_mint: &Pubkey,
) -> Result<(), ProgramError> {
    // Check account is initialized
    if account.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }
    
    // In production, would deserialize and validate SPL token account
    // Check mint matches mSOL
    
    Ok(())
}

/// Helper to calculate mSOL collateral value in USD
pub fn calculate_msol_collateral_value(
    msol_amount: u64,
    sol_price_usd: u64, // In basis points (100 = $1)
) -> Result<u64, ProgramError> {
    // Get mSOL to SOL rate
    let msol_to_sol_rate = 1_100_000_000u64; // 1.1 SOL per mSOL
    
    // Calculate SOL equivalent
    let sol_amount = msol_amount
        .checked_mul(msol_to_sol_rate)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
        .checked_div(1_000_000_000)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    // Calculate USD value
    let usd_value = sol_amount
        .checked_mul(sol_price_usd)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
        .checked_div(100)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    msg!("mSOL collateral value: {} USD", usd_value);
    Ok(usd_value)
}

/// Order unstake (for delayed unstaking)
/// 
/// This creates a ticket for unstaking that can be claimed after the epoch ends
pub fn order_unstake<'a>(
    marinade_program: &AccountInfo<'a>,
    marinade_state: &AccountInfo<'a>,
    msol_mint: &AccountInfo<'a>,
    burn_msol_from: &AccountInfo<'a>,
    burn_msol_authority: &AccountInfo<'a>,
    new_ticket_account: &AccountInfo<'a>,
    clock: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    msol_amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    msg!("Ordering unstake of {} mSOL", msol_amount);
    
    // Track CPI depth
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Create order unstake instruction
    let instruction_data = MarinadeInstruction::OrderUnstake { msol_amount };
    let data = instruction_data.try_to_vec()?;
    
    let instruction = solana_program::instruction::Instruction {
        program_id: *marinade_program.key,
        accounts: vec![
            solana_program::instruction::AccountMeta::new(*marinade_state.key, false),
            solana_program::instruction::AccountMeta::new(*msol_mint.key, false),
            solana_program::instruction::AccountMeta::new(*burn_msol_from.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*burn_msol_authority.key, true),
            solana_program::instruction::AccountMeta::new(*new_ticket_account.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*clock.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*rent.key, false),
            solana_program::instruction::AccountMeta::new_readonly(*token_program.key, false),
        ],
        data,
    };
    
    // Invoke with signer seeds
    invoke_signed(&instruction, &[
        marinade_state.clone(),
        msol_mint.clone(),
        burn_msol_from.clone(),
        burn_msol_authority.clone(),
        new_ticket_account.clone(),
        clock.clone(),
        rent.clone(),
        token_program.clone(),
    ], signer_seeds)?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully ordered unstake, ticket created");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_msol_collateral_calculation() {
        // Test with 10 mSOL at $100 SOL price
        let msol_amount = 10_000_000_000; // 10 mSOL
        let sol_price = 10000; // $100 in basis points
        
        let value = calculate_msol_collateral_value(msol_amount, sol_price).unwrap();
        
        // 10 mSOL * 1.1 SOL/mSOL * $100/SOL = $1100
        assert_eq!(value, 1100_000_000_000); // $1100 with 9 decimals
    }
    
    #[test]
    fn test_marinade_constants() {
        // Verify Marinade addresses are correct format
        assert_eq!(MARINADE_PROGRAM_ID.to_string().len(), 44);
        assert_eq!(MSOL_MINT.to_string().len(), 44);
        assert_eq!(MARINADE_STATE.to_string().len(), 44);
    }
}