//! CDP Instructions
//!
//! Entry points for CDP operations

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    synthetics::MintAuthority,
};

use super::{
    state::{CDPAccount, CDPState, CollateralType, derive_cdp_account_pda},
    vault::{CDPVault, execute_vault_deposit, execute_vault_withdraw},
    borrowing::{BorrowRequest, execute_borrow, execute_repay},
    liquidation::{LiquidationEngine, execute_liquidation},
    oracle_feed::{validate_oracle_price, get_collateral_value},
};

/// Create a new CDP
pub fn create_cdp(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    collateral_type: CollateralType,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cdp_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let collateral_mint = next_account_info(account_iter)?;
    let synthetic_mint = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    let rent_sysvar = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Generate CDP ID
    let cdp_id = market_id * 10000 + 1; // Simple ID generation
    
    // Derive PDA
    let (pda, bump) = derive_cdp_account_pda(program_id, owner.key, cdp_id);
    
    if pda != *cdp_account.key {
        msg!("Invalid CDP account PDA");
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create CDP
    let mut cdp = CDPAccount::new(
        *owner.key,
        cdp_id,
        market_id,
        collateral_type,
        *collateral_mint.key,
        *synthetic_mint.key,
        *oracle_account.key,
    );
    
    cdp.created_at = solana_program::clock::Clock::get()?.unix_timestamp;
    
    // Serialize and save
    let mut data = cdp_account.try_borrow_mut_data()?;
    cdp.serialize(&mut &mut data[..])?;
    
    msg!("Created CDP {} for owner {}", cdp_id, owner.key);
    
    Ok(())
}

/// Deposit collateral into CDP
pub fn deposit_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cdp_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let collateral_source = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load CDP
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    
    // Verify owner
    if cdp.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Deposit to CDP
    cdp.deposit_collateral(amount)?;
    
    // Transfer collateral to vault
    // In production, would use SPL token transfer here
    
    // Save CDP
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Deposited {} collateral to CDP {}", amount, cdp.cdp_id);
    
    Ok(())
}

/// Borrow synthetic tokens from CDP
pub fn borrow_synthetic(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    borrow_request: BorrowRequest,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cdp_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let synthetic_destination = next_account_info(account_iter)?;
    let mint_authority_account = next_account_info(account_iter)?;
    let synthetic_mint = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle_pda = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    let mut mint_authority = MintAuthority::deserialize(&mut &mint_authority_account.data.borrow()[..])?;
    
    // Verify owner
    if cdp.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Execute borrow
    let amount_borrowed = execute_borrow(
        program_id,
        &mut cdp,
        &borrow_request,
        &oracle_pda,
        &mut mint_authority,
        solana_program::clock::Clock::get()?.unix_timestamp,
    )?;
    
    // Mint synthetic tokens
    let signer_seeds = &[
        b"mint_authority",
        &cdp.market_id.to_le_bytes(),
        &[mint_authority.bump],
    ];
    
    // In production, would mint tokens here using SPL token program
    msg!("Would mint {} synthetic tokens", amount_borrowed);
    
    // Save accounts
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    mint_authority.serialize(&mut &mut mint_authority_account.data.borrow_mut()[..])?;
    
    msg!("Borrowed {} synthetic tokens from CDP {}", amount_borrowed, cdp.cdp_id);
    
    Ok(())
}

/// Repay debt to CDP
pub fn repay_debt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cdp_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let synthetic_source = next_account_info(account_iter)?;
    let mint_authority_account = next_account_info(account_iter)?;
    let synthetic_mint = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let mut mint_authority = MintAuthority::deserialize(&mut &mint_authority_account.data.borrow()[..])?;
    
    // Verify owner
    if cdp.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Execute repay
    let amount_repaid = execute_repay(
        program_id,
        &mut cdp,
        amount,
        &mut mint_authority,
        solana_program::clock::Clock::get()?.unix_timestamp,
    )?;
    
    // Burn synthetic tokens
    let signer_seeds = &[
        b"mint_authority",
        &cdp.market_id.to_le_bytes(),
        &[mint_authority.bump],
    ];
    
    // In production, would burn tokens here using SPL token program
    msg!("Would burn {} synthetic tokens", amount_repaid);
    
    // Save accounts
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    mint_authority.serialize(&mut &mut mint_authority_account.data.borrow_mut()[..])?;
    
    msg!("Repaid {} to CDP {}", amount_repaid, cdp.cdp_id);
    
    Ok(())
}

/// Withdraw collateral from CDP
pub fn withdraw_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cdp_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let collateral_destination = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle_pda = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify owner
    if cdp.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Withdraw from CDP
    cdp.withdraw_collateral(amount, oracle_pda.current_prob)?;
    
    // Transfer collateral from vault
    // In production, would use SPL token transfer here
    
    // Save CDP
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Withdrew {} collateral from CDP {}", amount, cdp.cdp_id);
    
    Ok(())
}

/// Liquidate an under-collateralized CDP
pub fn liquidate_cdp(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    repay_amount: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cdp_account = next_account_info(account_iter)?;
    let liquidator = next_account_info(account_iter)?;
    let collateral_destination = next_account_info(account_iter)?;
    let synthetic_source = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let liquidation_engine_account = next_account_info(account_iter)?;
    let token_program = next_account_info(account_iter)?;
    
    // Verify signer
    if !liquidator.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle_pda = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    let liquidation_engine = LiquidationEngine::deserialize(&mut &liquidation_engine_account.data.borrow()[..])?;
    
    // Execute liquidation
    let (collateral_seized, debt_repaid) = execute_liquidation(
        program_id,
        &mut cdp,
        liquidator.key,
        repay_amount,
        &oracle_pda,
        &liquidation_engine.params,
    )?;
    
    // Transfer collateral to liquidator
    // Burn debt tokens
    // In production, would handle token transfers here
    
    // Save CDP
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Liquidated CDP {}: {} collateral for {} debt", 
         cdp.cdp_id, collateral_seized, debt_repaid);
    
    Ok(())
}

/// Update oracle price for CDPs
pub fn update_oracle_price(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let oracle_account = next_account_info(account_iter)?;
    let cdp_state_account = next_account_info(account_iter)?;
    
    // Load oracle
    let oracle_pda = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Load CDP state
    let mut cdp_state = CDPState::deserialize(&mut &cdp_state_account.data.borrow()[..])?;
    
    // Update state with new oracle price
    cdp_state.last_update_slot = solana_program::clock::Clock::get()?.slot;
    cdp_state.update();
    
    // Save state
    cdp_state.serialize(&mut &mut cdp_state_account.data.borrow_mut()[..])?;
    
    msg!("Updated oracle price for CDP system");
    
    Ok(())
}

/// Emergency shutdown of CDP system
pub fn emergency_shutdown(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let authority = next_account_info(account_iter)?;
    let cdp_state_account = next_account_info(account_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load CDP state
    let mut cdp_state = CDPState::deserialize(&mut &cdp_state_account.data.borrow()[..])?;
    
    // Activate emergency shutdown
    cdp_state.emergency_shutdown = true;
    
    // Save state
    cdp_state.serialize(&mut &mut cdp_state_account.data.borrow_mut()[..])?;
    
    msg!("Emergency shutdown activated for CDP system");
    
    Ok(())
}