//! LMSR market validation
//!
//! Validation functions for LMSR market operations

use solana_program::{
    clock::Clock,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    amm::constants::*,
    error::BettingPlatformError,
    state::amm_accounts::{LSMRMarket, MarketState},
};

/// Validate market can accept trades
pub fn validate_market_tradeable(market: &LSMRMarket) -> ProgramResult {
    // Check market is active
    if market.state != MarketState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Check market has sufficient liquidity
    if market.b_parameter < MIN_LIQUIDITY {
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }

    // Validate shares array
    if market.shares.len() != market.num_outcomes as usize {
        return Err(BettingPlatformError::InvalidMarketState.into());
    }

    Ok(())
}

/// Validate outcome index
pub fn validate_outcome(outcome: u8, num_outcomes: u8) -> ProgramResult {
    if outcome >= num_outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    Ok(())
}

/// Validate trade amount
pub fn validate_trade_amount(amount: u64, is_buy: bool) -> ProgramResult {
    if amount < MIN_TRADE_SIZE {
        return Err(BettingPlatformError::InvalidTradeAmount.into());
    }

    // Additional validation for large trades
    if is_buy && amount > MAX_TRADE_SIZE {
        return Err(BettingPlatformError::TradeTooLarge.into());
    }

    Ok(())
}

/// Validate market resolution
pub fn validate_resolution(
    market: &LSMRMarket,
    winning_outcome: u8,
    oracle: &Pubkey,
) -> ProgramResult {
    // Check market state
    if market.state == MarketState::Resolved {
        return Err(BettingPlatformError::MarketAlreadyResolved.into());
    }

    // Validate oracle
    if oracle != &market.oracle {
        return Err(BettingPlatformError::InvalidOracle.into());
    }

    // Validate winning outcome
    validate_outcome(winning_outcome, market.num_outcomes)?;

    Ok(())
}

/// Validate market pause/unpause
pub fn validate_pause_authority(
    market: &LSMRMarket,
    authority: &Pubkey,
    operation: PauseOperation,
) -> ProgramResult {
    match operation {
        PauseOperation::Pause => {
            if market.state != MarketState::Active {
                return Err(BettingPlatformError::InvalidMarketState.into());
            }
        }
        PauseOperation::Unpause => {
            if market.state != MarketState::Paused {
                return Err(BettingPlatformError::InvalidMarketState.into());
            }
        }
    }

    // In production, validate authority is admin or emergency pause key
    // For now, accept any signer
    Ok(())
}

/// Validate fee update
pub fn validate_fee_update(
    current_fee: u16,
    new_fee: u16,
    last_update: i64,
) -> ProgramResult {
    // Check fee bounds
    if new_fee > MAX_FEE_BPS {
        return Err(BettingPlatformError::FeeTooHigh.into());
    }

    // Prevent frequent fee changes (24 hour cooldown)
    let clock = Clock::get()?;
    let time_since_update = clock.unix_timestamp - last_update;
    
    if time_since_update < FEE_UPDATE_COOLDOWN {
        return Err(BettingPlatformError::UpdateTooFrequent.into());
    }

    // Limit fee increase per update (max 50% increase)
    if new_fee > current_fee {
        let increase = new_fee - current_fee;
        let max_increase = current_fee / 2;
        
        if increase > max_increase {
            return Err(BettingPlatformError::FeeIncreaseTooLarge.into());
        }
    }

    Ok(())
}

/// Validate liquidity adjustment
pub fn validate_liquidity_adjustment(
    market: &LSMRMarket,
    adjustment: i64,
) -> ProgramResult {
    // Calculate new liquidity
    let new_liquidity = if adjustment >= 0 {
        market.b_parameter.saturating_add(adjustment as u64)
    } else {
        market.b_parameter.saturating_sub(adjustment.abs() as u64)
    };

    // Check minimum liquidity maintained
    if new_liquidity < MIN_LIQUIDITY {
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }

    // Check maximum liquidity
    if new_liquidity > MAX_LIQUIDITY {
        return Err(BettingPlatformError::LiquidityTooHigh.into());
    }

    // Prevent drastic liquidity changes (max 20% per adjustment)
    let change_percent = if adjustment >= 0 {
        (adjustment as u64 * 100) / market.b_parameter
    } else {
        (adjustment.abs() as u64 * 100) / market.b_parameter
    };

    if change_percent > 20 {
        return Err(BettingPlatformError::LiquidityChangeTooLarge.into());
    }

    Ok(())
}

/// Validate share balance consistency
pub fn validate_share_balances(market: &LSMRMarket) -> ProgramResult {
    // Sum of shares should match tracked positions
    let total_shares: u64 = market.shares.iter().sum();
    
    // Basic sanity check - total shares shouldn't exceed reasonable bounds
    let max_expected_shares = market.b_parameter.saturating_mul(1000);
    
    if total_shares > max_expected_shares {
        return Err(BettingPlatformError::InvalidMarketState.into());
    }

    Ok(())
}

/// Validate market can be closed
pub fn validate_market_closure(market: &LSMRMarket) -> ProgramResult {
    // Market must be resolved
    if market.state != MarketState::Resolved {
        return Err(BettingPlatformError::MarketNotResolved.into());
    }

    // Check settlement period has passed (7 days)
    let clock = Clock::get()?;
    let time_since_resolution = clock.unix_timestamp - market.last_update;
    
    if time_since_resolution < SETTLEMENT_PERIOD {
        return Err(BettingPlatformError::SettlementPeriodNotComplete.into());
    }

    Ok(())
}

/// Pause operation type
pub enum PauseOperation {
    Pause,
    Unpause,
}

// Additional constants
pub const MAX_TRADE_SIZE: u64 = 1_000_000_000_000; // 1M USDC
pub const MAX_FEE_BPS: u16 = 1000; // 10%
pub const FEE_UPDATE_COOLDOWN: i64 = 86400; // 24 hours
pub const MAX_LIQUIDITY: u64 = 10_000_000_000_000; // 10M USDC
pub const SETTLEMENT_PERIOD: i64 = 604800; // 7 days

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_outcome() {
        assert!(validate_outcome(0, 2).is_ok());
        assert!(validate_outcome(1, 2).is_ok());
        assert!(validate_outcome(2, 2).is_err());
    }

    #[test]
    fn test_validate_trade_amount() {
        assert!(validate_trade_amount(MIN_TRADE_SIZE, true).is_ok());
        assert!(validate_trade_amount(MIN_TRADE_SIZE - 1, true).is_err());
        assert!(validate_trade_amount(MAX_TRADE_SIZE + 1, true).is_err());
    }

    #[test]
    fn test_validate_fee_update() {
        let current_fee = 30; // 0.3%
        let last_update = 0;

        // Valid fee update
        assert!(validate_fee_update(current_fee, 40, last_update).is_ok());

        // Fee too high
        assert!(validate_fee_update(current_fee, MAX_FEE_BPS + 1, last_update).is_err());

        // Increase too large (>50%)
        assert!(validate_fee_update(current_fee, 50, last_update).is_err());
    }
}