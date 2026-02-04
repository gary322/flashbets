//! Trading validation functions

use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::{
    error::BettingPlatformError,
    state::{Position, Proposal, UserMap},
};

/// Validate position can be opened
pub fn validate_position_open(
    proposal: &Proposal,
    user_map: &UserMap,
    size: u64,
    leverage: u64,
) -> ProgramResult {
    // Check market is active
    if !proposal.is_active() {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Check leverage limits
    if leverage == 0 || leverage > 100 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }

    // Check size limits
    if size == 0 {
        return Err(BettingPlatformError::InvalidSize.into());
    }

    // Check user hasn't exceeded position limit
    if user_map.active_positions() >= 50 {
        return Err(BettingPlatformError::TooManyPositions.into());
    }

    Ok(())
}

/// Validate order parameters
pub fn validate_order_parameters(
    total_size: u64,
    limit_price: u64,
    min_order_size: u64,
    max_order_size: u64,
) -> ProgramResult {
    // Check size limits
    if total_size < min_order_size {
        return Err(BettingPlatformError::BelowMinimumSize.into());
    }
    
    if total_size > max_order_size {
        return Err(BettingPlatformError::OrderSizeTooLarge.into());
    }
    
    // Check price validity (0 is allowed for market orders)
    if limit_price > 0 && limit_price > 1_000_000 {
        return Err(BettingPlatformError::InvalidPrice.into());
    }
    
    Ok(())
}

/// Validate position can be closed
pub fn validate_position_close(
    position: &Position,
    user: &Pubkey,
) -> ProgramResult {
    // Check ownership
    if position.user != *user {
        return Err(BettingPlatformError::Unauthorized.into());
    }

    // Check position is open
    if position.is_closed {
        return Err(BettingPlatformError::PositionAlreadyClosed.into());
    }

    Ok(())
}

/// Validate margin requirements
pub fn validate_margin_requirements(
    position_size: u64,
    leverage: u64,
    user_balance: u64,
) -> ProgramResult {
    let required_margin = position_size / leverage;
    
    if user_balance < required_margin {
        return Err(BettingPlatformError::InsufficientMargin.into());
    }

    Ok(())
}