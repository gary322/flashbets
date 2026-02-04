//! L2-AMM trade execution
//!
//! Handles buying and selling shares in continuous distribution markets

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
    events::{Event, L2TradeExecuted},
    pda::{L2ammPoolPDA, L2PositionPDA, UserMapPDA},
    state::{
        accounts::UserMap,
        amm_accounts::{L2AMMPool, L2Position, PoolState},
    },
};

use super::math::{
    calculate_expected_value, calculate_l2_norm,
    calculate_range_buy_cost, calculate_range_sell_payout,
};

/// L2-AMM trade parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct L2TradeParams {
    pub pool_id: u128,
    pub lower_bound: u64,
    pub upper_bound: u64,
    pub shares: u64,
    pub is_buy: bool,
    pub max_cost: Option<u64>,
    pub min_payout: Option<u64>,
}

/// Process L2-AMM trade
pub fn process_l2amm_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: L2TradeParams,
) -> ProgramResult {
    msg!("Processing L2-AMM trade");

    // Validate parameters
    validate_l2_trade_params(&params)?;

    // Get accounts
    let account_info_iter = &mut accounts.iter();

    let trader = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let user_map_account = next_account_info(account_info_iter)?;
    let fee_collector = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(trader)?;
    validate_writable(pool_account)?;
    validate_writable(position_account)?;
    validate_writable(user_map_account)?;
    validate_writable(fee_collector)?;

    // Load and validate pool
    let mut pool = L2AMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool PDA
    let (pool_pda, _) = L2ammPoolPDA::derive(program_id, pool.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check pool state
    if pool.state != PoolState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Validate range is within pool bounds
    if params.lower_bound < pool.min_value || params.upper_bound > pool.max_value {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    // Execute trade
    let (total_cost, fee_amount) = if params.is_buy {
        execute_range_buy(&mut pool, &params, trader, pool_account, fee_collector)?
    } else {
        execute_range_sell(&mut pool, &params, trader, pool_account, fee_collector)?
    };

    // Update position
    update_l2_position(
        program_id,
        position_account,
        trader.key,
        pool.pool_id,
        params.lower_bound,
        params.upper_bound,
        params.shares,
        params.is_buy,
        total_cost,
    )?;

    // Update user map
    update_user_map(program_id, user_map_account, trader.key, pool.pool_id)?;

    // Update pool metadata
    let clock = Clock::get()?;
    pool.total_volume = pool.total_volume.saturating_add(total_cost);
    pool.last_update = clock.unix_timestamp;

    // Recalculate total shares
    pool.total_shares = pool.distribution.iter().map(|b| b.weight).sum();

    // Save pool
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Calculate new statistics
    let expected_value = calculate_expected_value(&pool)?;
    let l2_norm = calculate_l2_norm(&pool.distribution)?;

    // Emit event
    L2TradeExecuted {
        pool_id: pool.pool_id,
        trader: *trader.key,
        lower_bound: params.lower_bound,
        upper_bound: params.upper_bound,
        shares: params.shares,
        is_buy: params.is_buy,
        cost: total_cost,
        fee: fee_amount,
        new_expected_value: expected_value,
        new_l2_norm: l2_norm.to_num(),
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!(
        "L2-AMM trade completed: {} {} shares in range [{}, {}] for {} lamports",
        if params.is_buy { "bought" } else { "sold" },
        params.shares,
        params.lower_bound,
        params.upper_bound,
        total_cost
    );

    Ok(())
}

/// Execute buy operation for range
fn execute_range_buy(
    pool: &mut L2AMMPool,
    params: &L2TradeParams,
    trader: &AccountInfo,
    pool_account: &AccountInfo,
    fee_collector: &AccountInfo,
) -> Result<(u64, u64), ProgramError> {
    // Calculate cost
    let cost_before_fees = calculate_range_buy_cost(
        pool,
        params.lower_bound,
        params.upper_bound,
        params.shares,
    )?;

    // Calculate fees
    let fee_amount = cost_before_fees
        .saturating_mul(pool.fee_bps as u64)
        .saturating_div(10_000);
    
    let total_cost = cost_before_fees.saturating_add(fee_amount);

    // Check max cost constraint
    if let Some(max_cost) = params.max_cost {
        if total_cost > max_cost {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Check trader has sufficient balance
    if **trader.lamports.borrow() < total_cost {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Transfer payment
    **trader.lamports.borrow_mut() = trader
        .lamports()
        .checked_sub(total_cost)
        .ok_or(BettingPlatformError::Overflow)?;
    
    **pool_account.lamports.borrow_mut() = pool_account
        .lamports()
        .checked_add(cost_before_fees)
        .ok_or(BettingPlatformError::Overflow)?;

    if fee_amount > 0 {
        **fee_collector.lamports.borrow_mut() = fee_collector
            .lamports()
            .checked_add(fee_amount)
            .ok_or(BettingPlatformError::Overflow)?;
    }

    // Update distribution
    update_distribution_weights(
        pool,
        params.lower_bound,
        params.upper_bound,
        params.shares,
        true,
    )?;

    Ok((total_cost, fee_amount))
}

/// Execute sell operation for range
fn execute_range_sell(
    pool: &mut L2AMMPool,
    params: &L2TradeParams,
    trader: &AccountInfo,
    pool_account: &AccountInfo,
    fee_collector: &AccountInfo,
) -> Result<(u64, u64), ProgramError> {
    // Calculate payout
    let payout_before_fees = calculate_range_sell_payout(
        pool,
        params.lower_bound,
        params.upper_bound,
        params.shares,
    )?;

    // Calculate fees
    let fee_amount = payout_before_fees
        .saturating_mul(pool.fee_bps as u64)
        .saturating_div(10_000);
    
    let net_payout = payout_before_fees.saturating_sub(fee_amount);

    // Check min payout constraint
    if let Some(min_payout) = params.min_payout {
        if net_payout < min_payout {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Check pool has sufficient balance
    if **pool_account.lamports.borrow() < payout_before_fees {
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }

    // Transfer payout
    **pool_account.lamports.borrow_mut() = pool_account
        .lamports()
        .checked_sub(payout_before_fees)
        .ok_or(BettingPlatformError::Overflow)?;
    
    **trader.lamports.borrow_mut() = trader
        .lamports()
        .checked_add(net_payout)
        .ok_or(BettingPlatformError::Overflow)?;

    if fee_amount > 0 {
        **fee_collector.lamports.borrow_mut() = fee_collector
            .lamports()
            .checked_add(fee_amount)
            .ok_or(BettingPlatformError::Overflow)?;
    }

    // Update distribution
    update_distribution_weights(
        pool,
        params.lower_bound,
        params.upper_bound,
        params.shares,
        false,
    )?;

    Ok((payout_before_fees, fee_amount))
}

/// Update distribution weights based on trade
fn update_distribution_weights(
    pool: &mut L2AMMPool,
    lower_bound: u64,
    upper_bound: u64,
    shares: u64,
    is_buy: bool,
) -> ProgramResult {
    use super::math::{find_overlapping_bins, calculate_overlap_fraction};

    // Find affected bins
    let affected_bins = find_overlapping_bins(&pool.distribution, lower_bound, upper_bound)?;
    
    if affected_bins.is_empty() {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    // Distribute shares proportionally across affected bins
    let mut total_overlap = 0.0;
    let mut overlaps = Vec::new();
    
    for &idx in &affected_bins {
        let overlap = calculate_overlap_fraction(
            &pool.distribution[idx],
            lower_bound,
            upper_bound,
        )?.to_num() as f64;
        overlaps.push(overlap);
        total_overlap += overlap;
    }

    // Update weights
    for (i, &idx) in affected_bins.iter().enumerate() {
        let share_fraction = overlaps[i] / total_overlap;
        let shares_for_bin = (shares as f64 * share_fraction) as u64;
        
        if is_buy {
            pool.distribution[idx].weight = pool.distribution[idx].weight
                .saturating_add(shares_for_bin);
        } else {
            pool.distribution[idx].weight = pool.distribution[idx].weight
                .saturating_sub(shares_for_bin);
        }
    }

    Ok(())
}

/// Update or create L2 position
fn update_l2_position(
    program_id: &Pubkey,
    position_account: &AccountInfo,
    trader: &Pubkey,
    pool_id: u128,
    lower_bound: u64,
    upper_bound: u64,
    shares: u64,
    is_buy: bool,
    cost: u64,
) -> ProgramResult {
    // Generate position ID based on trader, pool, and range
    let position_id = L2Position::generate_id(trader, pool_id, lower_bound, upper_bound);
    let (position_pda, _) = L2PositionPDA::derive(program_id, position_id);
    
    if position_account.key != &position_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Load or initialize position
    let mut position = if position_account.data_len() > 0 {
        L2Position::try_from_slice(&position_account.data.borrow())?
    } else {
        L2Position {
            discriminator: *b"L2_POSIT",
            position_id,
            trader: *trader,
            pool_id,
            lower_bound,
            upper_bound,
            shares: 0,
            entry_cost: 0,
            last_update: Clock::get()?.unix_timestamp,
            realized_pnl: 0,
            fees_paid: 0,
        }
    };

    // Update position
    if is_buy {
        position.shares = position.shares.saturating_add(shares);
        position.entry_cost = position.entry_cost.saturating_add(cost);
    } else {
        // Calculate realized PnL
        if position.shares > 0 {
            let avg_entry = position.entry_cost / position.shares;
            let sale_value_per_share = cost / shares;
            
            if sale_value_per_share > avg_entry {
                let profit = (sale_value_per_share - avg_entry) * shares;
                position.realized_pnl = position.realized_pnl.saturating_add(profit as i64);
            } else {
                let loss = (avg_entry - sale_value_per_share) * shares;
                position.realized_pnl = position.realized_pnl.saturating_sub(loss as i64);
            }
        }
        
        position.shares = position.shares.saturating_sub(shares);
    }

    position.last_update = Clock::get()?.unix_timestamp;

    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;

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

/// Validate L2 trade parameters
fn validate_l2_trade_params(params: &L2TradeParams) -> ProgramResult {
    // Validate range
    if params.lower_bound >= params.upper_bound {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    // Validate shares
    if params.shares == 0 {
        return Err(BettingPlatformError::InvalidTradeAmount.into());
    }

    // For sells, ensure min_payout is set
    if !params.is_buy && params.min_payout.is_none() {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    Ok(())
}