//! PM-AMM trade execution
//!
//! Handles swaps between different outcomes in constant-product pools

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
    amm::constants::*,
    error::BettingPlatformError,
    events::{Event, SwapExecuted},
    pda::{PmammPoolPDA, PositionPDA, UserMapPDA},
    state::{
        accounts::{Position, UserMap},
        amm_accounts::{PMAMMMarket as PMAMMPool, MarketState as PoolState},
    },
};

use super::math::{
    calculate_invariant, calculate_price_impact, calculate_probabilities,
    calculate_swap_input, calculate_swap_output, calculate_lvr_adjustment,
    calculate_swap_output_with_uniform_lvr,
};

/// PM-AMM swap parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SwapParams {
    pub pool_id: u128,
    pub outcome_in: u8,
    pub outcome_out: u8,
    pub amount_in: Option<u64>,
    pub amount_out: Option<u64>,
    pub max_slippage_bps: Option<u16>,
}

/// Process PM-AMM swap
pub fn process_pmamm_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: SwapParams,
) -> ProgramResult {
    msg!("Processing PM-AMM swap");

    // Validate parameters
    validate_swap_params(&params)?;

    // Get accounts
    let account_info_iter = &mut accounts.iter();

    let trader = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let position_in_account = next_account_info(account_info_iter)?;
    let position_out_account = next_account_info(account_info_iter)?;
    let user_map_account = next_account_info(account_info_iter)?;
    let fee_collector = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(trader)?;
    validate_writable(pool_account)?;
    validate_writable(position_in_account)?;
    validate_writable(position_out_account)?;
    validate_writable(user_map_account)?;
    validate_writable(fee_collector)?;

    // Load and validate pool
    let mut pool = PMAMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool PDA
    let (pool_pda, _) = PmammPoolPDA::derive(program_id, pool.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check pool state
    if pool.state != PoolState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Calculate swap amounts
    let (amount_in, amount_out, fee_amount) = calculate_swap_amounts(&pool, &params)?;
    
    // Apply LVR protection
    let lvr_adjustment = calculate_lvr_adjustment(&pool, params.outcome_in, params.outcome_out, amount_in)?;
    let total_cost = amount_in.saturating_add(lvr_adjustment);

    // Check slippage
    if let Some(max_slippage_bps) = params.max_slippage_bps {
        let price_impact = calculate_price_impact(
            &pool,
            params.outcome_in,
            params.outcome_out,
            amount_in,
        )?;

        if price_impact > max_slippage_bps {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Verify invariant before swap
    let k_before = calculate_invariant(&pool.reserves)?;

    // Update reserves
    pool.reserves[params.outcome_in as usize] = 
        pool.reserves[params.outcome_in as usize].saturating_add(amount_in);
    
    pool.reserves[params.outcome_out as usize] = 
        pool.reserves[params.outcome_out as usize].saturating_sub(amount_out);

    // Verify invariant maintained (with small tolerance for rounding)
    let k_after = calculate_invariant(&pool.reserves)?;
    let tolerance = k_before / 10000; // 0.01% tolerance
    
    if k_after < k_before.saturating_sub(tolerance) {
        return Err(BettingPlatformError::InvalidMarketState.into());
    }

    // Transfer tokens (including LVR adjustment)
    execute_token_transfers(
        trader,
        pool_account,
        fee_collector,
        total_cost,
        amount_out,
        fee_amount.saturating_add(lvr_adjustment),
    )?;

    // Update positions
    update_swap_positions(
        program_id,
        position_in_account,
        position_out_account,
        trader.key,
        pool.pool_id,
        params.outcome_in,
        params.outcome_out,
        amount_in,
        amount_out,
    )?;

    // Update user map
    update_user_map(program_id, user_map_account, trader.key, pool.pool_id)?;

    // Update pool metadata
    let clock = Clock::get()?;
    pool.total_volume = pool.total_volume.saturating_add(amount_in);
    pool.last_update = clock.unix_timestamp;

    // Save pool
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Calculate new probabilities
    let probabilities = calculate_probabilities(&pool)?;

    // Emit event
    SwapExecuted {
        pool_id: pool.pool_id,
        trader: *trader.key,
        outcome_in: params.outcome_in,
        outcome_out: params.outcome_out,
        amount_in,
        amount_out,
        fee: fee_amount,
        new_reserves: pool.reserves.clone(),
        new_probabilities: probabilities,
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!(
        "Swap completed: {} of outcome {} for {} of outcome {}",
        amount_in,
        params.outcome_in,
        amount_out,
        params.outcome_out
    );

    Ok(())
}

/// Calculate swap amounts based on input parameters
fn calculate_swap_amounts(
    pool: &PMAMMPool,
    params: &SwapParams,
) -> Result<(u64, u64, u64), ProgramError> {
    match (params.amount_in, params.amount_out) {
        (Some(amount_in), None) => {
            // Fixed input amount
            let (amount_out, fee) = if pool.use_uniform_lvr {
                calculate_swap_output_with_uniform_lvr(
                    pool,
                    params.outcome_in,
                    params.outcome_out,
                    amount_in,
                )?
            } else {
                calculate_swap_output(
                    pool,
                    params.outcome_in,
                    params.outcome_out,
                    amount_in,
                )?
            };
            Ok((amount_in, amount_out, fee))
        }
        (None, Some(amount_out)) => {
            // Fixed output amount
            let (amount_in, fee) = calculate_swap_input(
                pool,
                params.outcome_in,
                params.outcome_out,
                amount_out,
            )?;
            Ok((amount_in, amount_out, fee))
        }
        _ => Err(BettingPlatformError::InvalidInput.into()),
    }
}

/// Execute token transfers for the swap
fn execute_token_transfers(
    trader: &AccountInfo,
    pool: &AccountInfo,
    fee_collector: &AccountInfo,
    amount_in: u64,
    amount_out: u64,
    fee_amount: u64,
) -> ProgramResult {
    // Check trader has sufficient balance
    if **trader.lamports.borrow() < amount_in {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Transfer input from trader to pool
    **trader.lamports.borrow_mut() = trader
        .lamports()
        .checked_sub(amount_in)
        .ok_or(BettingPlatformError::Overflow)?;

    let amount_to_pool = amount_in.saturating_sub(fee_amount);
    **pool.lamports.borrow_mut() = pool
        .lamports()
        .checked_add(amount_to_pool)
        .ok_or(BettingPlatformError::Overflow)?;

    // Transfer fee
    if fee_amount > 0 {
        **fee_collector.lamports.borrow_mut() = fee_collector
            .lamports()
            .checked_add(fee_amount)
            .ok_or(BettingPlatformError::Overflow)?;
    }

    // Transfer output from pool to trader
    **pool.lamports.borrow_mut() = pool
        .lamports()
        .checked_sub(amount_out)
        .ok_or(BettingPlatformError::Overflow)?;

    **trader.lamports.borrow_mut() = trader
        .lamports()
        .checked_add(amount_out)
        .ok_or(BettingPlatformError::Overflow)?;

    Ok(())
}

/// Update positions for both outcomes involved in swap
fn update_swap_positions(
    program_id: &Pubkey,
    position_in_account: &AccountInfo,
    position_out_account: &AccountInfo,
    trader: &Pubkey,
    pool_id: u128,
    outcome_in: u8,
    outcome_out: u8,
    amount_in: u64,
    amount_out: u64,
) -> ProgramResult {
    // Update position for outcome_in (selling this outcome)
    let position_index = 0u8; // For simplicity, using index 0 for AMM positions
    let (position_in_pda, _) = PositionPDA::derive(program_id, trader, pool_id, position_index);
    
    if position_in_account.key != &position_in_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut position_in = if position_in_account.data_len() > 0 {
        Position::try_from_slice(&position_in_account.data.borrow())?
    } else {
        Position::new(
            *trader,
            pool_id,  // Using pool_id as proposal_id for AMM
            0,  // verse_id: 0 for AMM positions
            outcome_in,
            amount_in,
            1,  // No leverage for AMM trades
            10000,  // 100% (selling)
            false,  // Selling
            Clock::get()?.unix_timestamp,
        )
    };

    // Track the sale (reducing position in outcome_in)
    if position_in.size >= amount_in {
        position_in.size = position_in.size.saturating_sub(amount_in);
    } else {
        // Going short
        position_in.size = 0;
    }

    position_in.serialize(&mut &mut position_in_account.data.borrow_mut()[..])?;

    // Update position for outcome_out (buying this outcome)
    let position_out_index = 1u8; // Using index 1 for the second outcome
    let (position_out_pda, _) = PositionPDA::derive(program_id, trader, pool_id, position_out_index);
    
    if position_out_account.key != &position_out_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut position_out = if position_out_account.data_len() > 0 {
        Position::try_from_slice(&position_out_account.data.borrow())?
    } else {
        Position::new(
            *trader,
            pool_id,  // Using pool_id as proposal_id for AMM
            0,  // verse_id: 0 for AMM positions
            outcome_out,
            0,
            1,  // No leverage for AMM trades
            0,
            true,  // Buying
            Clock::get()?.unix_timestamp,
        )
    };

    // Update average entry price
    let total_cost = position_out.size
        .saturating_mul(position_out.entry_price)
        .saturating_add(amount_in); // Cost basis is amount paid

    position_out.size = position_out.size.saturating_add(amount_out);

    if position_out.size > 0 {
        position_out.entry_price = total_cost / position_out.size;
    }

    position_out.serialize(&mut &mut position_out_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Update user map
fn update_user_map(
    program_id: &Pubkey,
    user_map_account: &AccountInfo,
    trader: &Pubkey,
    pool_id: u128,
) -> ProgramResult {
    let (user_map_pda, _) = UserMapPDA::derive(program_id, trader);
    
    if user_map_account.key != &user_map_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut user_map = if user_map_account.data_len() > 0 {
        UserMap::try_from_slice(&user_map_account.data.borrow())?
    } else {
        UserMap::new(*trader)
    };

    user_map.add_position(pool_id)?;
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Validate swap parameters
fn validate_swap_params(params: &SwapParams) -> ProgramResult {
    // Must specify exactly one of amount_in or amount_out
    match (params.amount_in, params.amount_out) {
        (Some(_), None) | (None, Some(_)) => {}
        _ => return Err(BettingPlatformError::InvalidInput.into()),
    }

    // Outcomes must be different
    if params.outcome_in == params.outcome_out {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Validate amounts
    if let Some(amount) = params.amount_in {
        if amount < MIN_TRADE_SIZE {
            return Err(BettingPlatformError::InvalidTradeAmount.into());
        }
    }

    if let Some(amount) = params.amount_out {
        if amount < MIN_TRADE_SIZE {
            return Err(BettingPlatformError::InvalidTradeAmount.into());
        }
    }

    // Validate slippage
    if let Some(slippage) = params.max_slippage_bps {
        if slippage > MAX_SLIPPAGE_BPS {
            return Err(BettingPlatformError::InvalidInput.into());
        }
    }

    Ok(())
}