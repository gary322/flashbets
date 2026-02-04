//! L2-AMM distribution management
//!
//! Handles distribution updates and continuous market resolution

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
    events::{Event, DistributionUpdated, ContinuousMarketResolved},
    pda::L2ammPoolPDA,
    state::amm_accounts::{L2AMMPool, PoolState},
};

use super::math::{
    calculate_confidence_interval, calculate_expected_value, 
    calculate_variance, normalize_distribution,
};

/// Update distribution shape (admin only)
pub fn process_update_distribution(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pool_id: u128,
    new_weights: Vec<u64>,
) -> ProgramResult {
    msg!("Updating L2-AMM distribution");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let admin = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(admin)?;
    validate_writable(pool_account)?;

    // Load pool
    let mut pool = L2AMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool PDA
    let (pool_pda, _) = L2ammPoolPDA::derive(program_id, pool.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate admin authority (in production, check against stored admin key)
    // For now, any signer can update

    // Validate new weights
    if new_weights.len() != pool.discretization_points as usize {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Ensure at least one non-zero weight
    if new_weights.iter().all(|&w| w == 0) {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Update distribution weights
    for (i, &weight) in new_weights.iter().enumerate() {
        pool.distribution[i].weight = weight;
    }

    // Normalize distribution
    pool.distribution = normalize_distribution(&pool.distribution)?;

    // Update metadata
    let clock = Clock::get()?;
    pool.last_update = clock.unix_timestamp;
    pool.total_shares = pool.distribution.iter().map(|b| b.weight).sum();

    // Save pool
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Calculate new statistics
    let expected_value = calculate_expected_value(&pool)?;
    let variance = calculate_variance(&pool)?;
    let (ci_lower, ci_upper) = calculate_confidence_interval(&pool)?;

    // Emit event
    DistributionUpdated {
        pool_id,
        new_expected_value: expected_value,
        new_variance: variance,
        confidence_interval: (ci_lower, ci_upper),
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!("Distribution updated successfully");
    Ok(())
}

/// Resolve continuous market with actual outcome value
pub fn process_resolve_continuous(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pool_id: u128,
    outcome_value: u64,
) -> ProgramResult {
    msg!("Resolving continuous market");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let oracle = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(oracle)?;
    validate_writable(pool_account)?;

    // Load pool
    let mut pool = L2AMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool PDA
    let (pool_pda, _) = L2ammPoolPDA::derive(program_id, pool.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify oracle
    if oracle.key != &pool.oracle {
        return Err(BettingPlatformError::InvalidOracle.into());
    }

    // Check pool is active
    if pool.state != PoolState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Validate outcome value is within bounds
    if outcome_value < pool.min_value || outcome_value > pool.max_value {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }

    // Find winning bin
    let mut winning_bin_index = None;
    for (i, bin) in pool.distribution.iter().enumerate() {
        if outcome_value >= bin.lower_bound && outcome_value < bin.upper_bound {
            winning_bin_index = Some(i);
            break;
        }
    }

    let winning_bin = winning_bin_index
        .ok_or(BettingPlatformError::InvalidOutcome)?;

    // Calculate payouts based on shares in winning bin
    let total_pool_value = **pool_account.lamports.borrow() - pool.liquidity_parameter;
    let winning_shares = pool.distribution[winning_bin].weight;
    let total_shares = pool.total_shares;

    let payout_per_share = if winning_shares > 0 {
        total_pool_value / winning_shares
    } else {
        0
    };

    // Update pool state
    pool.state = PoolState::Resolved;
    let clock = Clock::get()?;
    pool.last_update = clock.unix_timestamp;

    // Save resolution data (store in pool for now)
    // In production, create separate resolution account
    
    // Save pool
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Emit event
    ContinuousMarketResolved {
        pool_id,
        outcome_value,
        winning_bin: winning_bin as u8,
        winning_range: (
            pool.distribution[winning_bin].lower_bound,
            pool.distribution[winning_bin].upper_bound,
        ),
        payout_per_share,
        total_pool_value,
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!(
        "Continuous market resolved: outcome {} in bin {} with payout {} per share",
        outcome_value,
        winning_bin,
        payout_per_share
    );

    Ok(())
}

/// Claim winnings from resolved continuous market
pub fn process_claim_continuous(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pool_id: u128,
    position_id: u128,
) -> ProgramResult {
    msg!("Claiming continuous market winnings");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let claimer = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(claimer)?;
    validate_writable(pool_account)?;
    validate_writable(position_account)?;

    // Load pool
    let pool = L2AMMPool::try_from_slice(&pool_account.data.borrow())?;
    
    // Verify pool is resolved
    if pool.state != PoolState::Resolved {
        return Err(BettingPlatformError::MarketNotResolved.into());
    }

    // Load position
    let mut position = L2Position::try_from_slice(&position_account.data.borrow())?;
    
    // Verify position ownership
    if position.trader != *claimer.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }

    // Verify position matches pool
    if position.pool_id != pool_id {
        return Err(ProgramError::InvalidAccountData);
    }

    // Calculate payout based on position range and outcome
    // This is simplified - in production, store resolution data separately
    let payout = calculate_position_payout(&pool, &position)?;

    if payout == 0 {
        return Err(BettingPlatformError::NoClaimableAmount.into());
    }

    // Transfer payout
    **pool_account.lamports.borrow_mut() = pool_account
        .lamports()
        .checked_sub(payout)
        .ok_or(BettingPlatformError::Overflow)?;
    
    **claimer.lamports.borrow_mut() = claimer
        .lamports()
        .checked_add(payout)
        .ok_or(BettingPlatformError::Overflow)?;

    // Mark position as claimed
    position.shares = 0; // Reset shares to indicate claimed

    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;

    msg!("Claimed {} lamports from continuous market", payout);
    Ok(())
}

/// Calculate payout for a position in resolved market
fn calculate_position_payout(
    pool: &L2AMMPool,
    position: &L2Position,
) -> Result<u64, ProgramError> {
    // This is a simplified calculation
    // In production, need to check if position range contains winning outcome
    
    // For now, assume positions in any range get proportional payout
    // based on their share of total shares
    
    if position.shares == 0 {
        return Ok(0);
    }

    // Simple proportional payout
    let total_pool_value = pool.liquidity_parameter; // Simplified
    let payout = (position.shares as u128 * total_pool_value as u128 / pool.total_shares as u128) as u64;

    Ok(payout)
}

// Import L2Position for claiming
use crate::state::amm_accounts::L2Position;