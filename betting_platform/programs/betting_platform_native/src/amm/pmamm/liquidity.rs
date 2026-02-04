//! PM-AMM liquidity management
//!
//! Handles adding and removing liquidity from constant-product pools

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    error::BettingPlatformError,
    events::{Event, LiquidityAdded, LiquidityRemoved},
    pda::{PmammPoolPDA, LpPositionPDA},
    state::amm_accounts::{PMAMMMarket as PMAMMPool, MarketState as PoolState, LPPosition},
};

use super::math::{calculate_lp_tokens_to_mint, calculate_liquidity_amounts};

/// Add liquidity to PM-AMM pool
pub fn process_add_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pool_id: u128,
    amounts: Vec<u64>,
    min_lp_tokens: Option<u64>,
) -> ProgramResult {
    msg!("Adding liquidity to PM-AMM pool");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let provider = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let lp_position_account = next_account_info(account_info_iter)?;
    let lp_mint = next_account_info(account_info_iter)?;
    let lp_token_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(provider)?;
    validate_writable(pool_account)?;
    validate_writable(lp_position_account)?;
    validate_writable(lp_mint)?;
    validate_writable(lp_token_account)?;

    // Load pool
    let mut pool = PMAMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool PDA
    let (pool_pda, pool_bump) = PmammPoolPDA::derive(program_id, pool.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate pool state
    if pool.state != PoolState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Validate amounts
    if amounts.len() != pool.num_outcomes as usize {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    for &amount in &amounts {
        if amount == 0 {
            return Err(BettingPlatformError::InvalidInput.into());
        }
    }

    // Calculate LP tokens to mint
    let lp_tokens = calculate_lp_tokens_to_mint(&pool, &amounts)?;

    // Check slippage
    if let Some(min_tokens) = min_lp_tokens {
        if lp_tokens < min_tokens {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Calculate total deposit
    let total_deposit: u64 = amounts.iter().sum();

    // Check provider has sufficient balance
    if **provider.lamports.borrow() < total_deposit {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Transfer tokens from provider to pool
    **provider.lamports.borrow_mut() = provider
        .lamports()
        .checked_sub(total_deposit)
        .ok_or(BettingPlatformError::Overflow)?;

    **pool_account.lamports.borrow_mut() = pool_account
        .lamports()
        .checked_add(total_deposit)
        .ok_or(BettingPlatformError::Overflow)?;

    // Update pool reserves
    for (i, &amount) in amounts.iter().enumerate() {
        pool.reserves[i] = pool.reserves[i]
            .checked_add(amount)
            .ok_or(BettingPlatformError::Overflow)?;
    }

    // Update total LP supply
    pool.total_lp_supply = pool.total_lp_supply
        .checked_add(lp_tokens)
        .ok_or(BettingPlatformError::Overflow)?;

    // Update or create LP position
    update_lp_position(
        program_id,
        lp_position_account,
        provider.key,
        pool_id,
        lp_tokens,
        total_deposit,
        true,
    )?;

    // Mint LP tokens
    mint_lp_tokens(
        lp_mint,
        lp_token_account,
        pool_account,
        token_program,
        lp_tokens,
        &[b"pmamm_pool", &pool_id.to_le_bytes(), &[pool_bump]],
    )?;

    // Update pool metadata
    let clock = Clock::get()?;
    pool.last_update = clock.unix_timestamp;

    // Save pool
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Emit event
    LiquidityAdded {
        pool_id,
        provider: *provider.key,
        amounts,
        lp_tokens_minted: lp_tokens,
        new_reserves: pool.reserves.clone(),
        new_lp_supply: pool.total_lp_supply,
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!("Added liquidity: {} LP tokens minted", lp_tokens);
    Ok(())
}

/// Remove liquidity from PM-AMM pool
pub fn process_remove_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pool_id: u128,
    lp_tokens: u64,
    min_amounts: Option<Vec<u64>>,
) -> ProgramResult {
    msg!("Removing liquidity from PM-AMM pool");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let provider = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let lp_position_account = next_account_info(account_info_iter)?;
    let lp_mint = next_account_info(account_info_iter)?;
    let lp_token_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(provider)?;
    validate_writable(pool_account)?;
    validate_writable(lp_position_account)?;
    validate_writable(lp_mint)?;
    validate_writable(lp_token_account)?;

    // Load pool
    let mut pool = PMAMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool PDA
    let (pool_pda, pool_bump) = PmammPoolPDA::derive(program_id, pool.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Load LP position
    let lp_position = LPPosition::try_from_slice(&lp_position_account.data.borrow())?;
    
    // Verify LP position ownership
    if lp_position.provider != *provider.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }

    // Check LP token balance
    if lp_position.lp_tokens < lp_tokens {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Calculate amounts to return
    let amounts = calculate_liquidity_amounts(&pool, lp_tokens)?;

    // Check slippage
    if let Some(min_amounts) = min_amounts {
        if min_amounts.len() != amounts.len() {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        for (i, &amount) in amounts.iter().enumerate() {
            if amount < min_amounts[i] {
                return Err(BettingPlatformError::SlippageExceeded.into());
            }
        }
    }

    // Calculate total withdrawal
    let total_withdrawal: u64 = amounts.iter().sum();

    // Check pool has sufficient balance
    if **pool_account.lamports.borrow() < total_withdrawal {
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }

    // Burn LP tokens
    burn_lp_tokens(
        lp_mint,
        lp_token_account,
        pool_account,
        token_program,
        lp_tokens,
        &[b"pmamm_pool", &pool_id.to_le_bytes(), &[pool_bump]],
    )?;

    // Update pool reserves
    for (i, &amount) in amounts.iter().enumerate() {
        pool.reserves[i] = pool.reserves[i]
            .checked_sub(amount)
            .ok_or(BettingPlatformError::Overflow)?;
    }

    // Update total LP supply
    pool.total_lp_supply = pool.total_lp_supply
        .checked_sub(lp_tokens)
        .ok_or(BettingPlatformError::Overflow)?;

    // Transfer tokens from pool to provider
    **pool_account.lamports.borrow_mut() = pool_account
        .lamports()
        .checked_sub(total_withdrawal)
        .ok_or(BettingPlatformError::Overflow)?;

    **provider.lamports.borrow_mut() = provider
        .lamports()
        .checked_add(total_withdrawal)
        .ok_or(BettingPlatformError::Overflow)?;

    // Update LP position
    update_lp_position(
        program_id,
        lp_position_account,
        provider.key,
        pool_id,
        lp_tokens,
        total_withdrawal,
        false,
    )?;

    // Update pool metadata
    let clock = Clock::get()?;
    pool.last_update = clock.unix_timestamp;

    // Save pool
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Emit event
    LiquidityRemoved {
        pool_id,
        provider: *provider.key,
        lp_tokens_burned: lp_tokens,
        amounts_withdrawn: amounts,
        new_reserves: pool.reserves.clone(),
        new_lp_supply: pool.total_lp_supply,
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!("Removed liquidity: {} LP tokens burned", lp_tokens);
    Ok(())
}

/// Update LP position tracking
fn update_lp_position(
    program_id: &Pubkey,
    lp_position_account: &AccountInfo,
    provider: &Pubkey,
    pool_id: u128,
    lp_tokens: u64,
    value: u64,
    is_add: bool,
) -> ProgramResult {
    // Derive LP position PDA
    let (lp_position_pda, _) = LpPositionPDA::derive(program_id, provider, pool_id);
    
    if lp_position_account.key != &lp_position_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut position = if lp_position_account.data_len() > 0 {
        LPPosition::try_from_slice(&lp_position_account.data.borrow())?
    } else {
        LPPosition {
            discriminator: *b"LP_POSIT",
            provider: *provider,
            pool_id,
            lp_tokens: 0,
            initial_investment: 0,
            withdrawn_amount: 0,
            last_update: Clock::get()?.unix_timestamp,
        }
    };

    if is_add {
        position.lp_tokens = position.lp_tokens
            .checked_add(lp_tokens)
            .ok_or(BettingPlatformError::Overflow)?;
        
        position.initial_investment = position.initial_investment
            .checked_add(value)
            .ok_or(BettingPlatformError::Overflow)?;
    } else {
        position.lp_tokens = position.lp_tokens
            .checked_sub(lp_tokens)
            .ok_or(BettingPlatformError::Overflow)?;
        
        position.withdrawn_amount = position.withdrawn_amount
            .checked_add(value)
            .ok_or(BettingPlatformError::Overflow)?;
    }

    position.last_update = Clock::get()?.unix_timestamp;
    position.serialize(&mut &mut lp_position_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Mint LP tokens using CPI to SPL Token program
fn mint_lp_tokens<'a>(
    mint: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    use crate::cpi::spl_token;
    
    msg!("Minting {} LP tokens", amount);
    
    // Use the CPI helper to mint tokens
    spl_token::mint_to(
        mint,
        token_account,
        authority,
        amount,
        token_program,
        &[signer_seeds],
    )
}

/// Burn LP tokens using CPI to SPL Token program
fn burn_lp_tokens<'a>(
    mint: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    use crate::cpi::spl_token;
    
    msg!("Burning {} LP tokens", amount);
    
    // Use the CPI helper to burn tokens
    spl_token::burn(
        token_account,
        mint,
        authority,
        amount,
        token_program,
        &[signer_seeds],
    )
}