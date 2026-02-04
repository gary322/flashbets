//! LMSR trade execution
//!
//! Handles buy and sell operations for LMSR markets

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
    events::{Event, TradeExecuted},
    instruction::TradeParams,
    pda::{LmsrMarketPDA, PositionPDA, UserMapPDA},
    state::{
        accounts::{Position, UserMap},
        amm_accounts::{LSMRMarket, MarketState},
    },
};

use super::math::{
    calculate_buy_cost, calculate_price, calculate_probabilities, calculate_sell_payout,
    calculate_shares_to_buy,
};

/// Process LMSR trade (buy or sell shares)
pub fn process_lmsr_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: TradeParams,
) -> ProgramResult {
    msg!("Processing LMSR trade");

    // Validate trade parameters
    validate_trade_params(&params)?;

    // Get accounts
    let account_info_iter = &mut accounts.iter();

    let trader = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let user_map_account = next_account_info(account_info_iter)?;
    let fee_collector = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(trader)?;
    validate_writable(market_account)?;
    validate_writable(position_account)?;
    validate_writable(user_map_account)?;
    validate_writable(fee_collector)?;

    // Load and validate market
    let mut market = LSMRMarket::try_from_slice(&market_account.data.borrow())?;
    
    // Verify market PDA
    let (market_pda, _) = LmsrMarketPDA::derive(program_id, market.market_id);
    if market_account.key != &market_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check market state
    if market.state != MarketState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Verify outcome is valid
    if params.outcome >= market.num_outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }

    // Get current timestamp
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Execute trade based on direction
    let (shares_traded, total_cost, fee_amount) = match params.is_buy {
        true => execute_buy(&mut market, &params, trader, market_account)?,
        false => execute_sell(&mut market, &params, trader, market_account)?,
    };

    // Update or create position
    update_position(
        program_id,
        position_account,
        trader.key,
        market.market_id,
        params.outcome,
        shares_traded,
        params.is_buy,
        total_cost,
    )?;

    // Update user map
    update_user_map(program_id, user_map_account, trader.key, market.market_id)?;

    // Transfer fees
    if fee_amount > 0 {
        **fee_collector.lamports.borrow_mut() += fee_amount;
    }

    // Update market state
    market.total_volume = market.total_volume.saturating_add(total_cost);
    market.last_update = clock.unix_timestamp;

    // Save updated market
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;

    // Calculate new probabilities
    let probabilities = calculate_probabilities(&market)?;

    // Emit trade event
    TradeExecuted {
        market_id: market.market_id,
        trader: *trader.key,
        outcome: params.outcome,
        is_buy: params.is_buy,
        shares: shares_traded,
        cost: total_cost,
        fee: fee_amount,
        new_probabilities: probabilities,
        timestamp: clock.unix_timestamp,
    }
    .emit();

    msg!(
        "LMSR trade completed: {} {} shares of outcome {} for {} lamports",
        if params.is_buy { "bought" } else { "sold" },
        shares_traded,
        params.outcome,
        total_cost
    );

    Ok(())
}

/// Execute buy operation
fn execute_buy(
    market: &mut LSMRMarket,
    params: &TradeParams,
    trader: &AccountInfo,
    market_account: &AccountInfo,
) -> Result<(u64, u64, u64), ProgramError> {
    // Calculate shares to buy based on max cost
    let shares_to_buy = if let Some(shares) = params.shares {
        // Verify cost doesn't exceed max
        let cost = calculate_buy_cost(market, params.outcome, shares)?;
        if let Some(max_cost) = params.max_cost {
            if cost > max_cost {
                return Err(BettingPlatformError::SlippageExceeded.into());
            }
        }
        shares
    } else if let Some(max_cost) = params.max_cost {
        calculate_shares_to_buy(market, params.outcome, max_cost)?
    } else {
        return Err(BettingPlatformError::InvalidInput.into());
    };

    if shares_to_buy == 0 {
        return Err(BettingPlatformError::InvalidTradeAmount.into());
    }

    // Calculate actual cost
    let cost_before_fees = calculate_buy_cost(market, params.outcome, shares_to_buy)?;

    // Calculate fees
    let fee_amount = cost_before_fees
        .saturating_mul(market.fee_bps as u64)
        .saturating_div(10_000);
    
    let total_cost = cost_before_fees.saturating_add(fee_amount);

    // Check trader has sufficient balance
    if **trader.lamports.borrow() < total_cost {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Check slippage
    if let Some(max_slippage_bps) = params.max_slippage_bps {
        let initial_price = calculate_price(&market.shares, params.outcome, market.b_parameter)?;
        
        // Update shares temporarily to calculate new price
        market.shares[params.outcome as usize] = 
            market.shares[params.outcome as usize].saturating_add(shares_to_buy);
        
        let new_price = calculate_price(&market.shares, params.outcome, market.b_parameter)?;
        
        // Revert temporary update
        market.shares[params.outcome as usize] = 
            market.shares[params.outcome as usize].saturating_sub(shares_to_buy);
        
        let price_impact = if new_price > initial_price {
            ((new_price - initial_price) * 10_000) / initial_price
        } else {
            0
        };
        
        if price_impact > max_slippage_bps as u64 {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Transfer payment from trader
    **trader.lamports.borrow_mut() = trader
        .lamports()
        .checked_sub(total_cost)
        .ok_or(BettingPlatformError::Overflow)?;
    
    **market_account.lamports.borrow_mut() = market_account
        .lamports()
        .checked_add(cost_before_fees)
        .ok_or(BettingPlatformError::Overflow)?;

    // Update market shares
    market.shares[params.outcome as usize] = 
        market.shares[params.outcome as usize].saturating_add(shares_to_buy);
    
    // Update cost basis
    market.cost_basis = market.cost_basis.saturating_add(cost_before_fees);

    Ok((shares_to_buy, total_cost, fee_amount))
}

/// Execute sell operation
fn execute_sell(
    market: &mut LSMRMarket,
    params: &TradeParams,
    trader: &AccountInfo,
    market_account: &AccountInfo,
) -> Result<(u64, u64, u64), ProgramError> {
    let shares_to_sell = params.shares
        .ok_or(BettingPlatformError::InvalidInput)?;

    if shares_to_sell == 0 {
        return Err(BettingPlatformError::InvalidTradeAmount.into());
    }

    // Verify market has enough shares
    if market.shares[params.outcome as usize] < shares_to_sell {
        return Err(BettingPlatformError::InsufficientShares.into());
    }

    // Calculate payout
    let payout_before_fees = calculate_sell_payout(market, params.outcome, shares_to_sell)?;

    // Calculate fees
    let fee_amount = payout_before_fees
        .saturating_mul(market.fee_bps as u64)
        .saturating_div(10_000);
    
    let net_payout = payout_before_fees.saturating_sub(fee_amount);

    // Check minimum payout
    if let Some(min_payout) = params.min_payout {
        if net_payout < min_payout {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Check market has sufficient balance
    if **market_account.lamports.borrow() < payout_before_fees {
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }

    // Check slippage
    if let Some(max_slippage_bps) = params.max_slippage_bps {
        let initial_price = calculate_price(&market.shares, params.outcome, market.b_parameter)?;
        
        // Update shares temporarily to calculate new price
        market.shares[params.outcome as usize] = 
            market.shares[params.outcome as usize].saturating_sub(shares_to_sell);
        
        let new_price = calculate_price(&market.shares, params.outcome, market.b_parameter)?;
        
        // Revert temporary update
        market.shares[params.outcome as usize] = 
            market.shares[params.outcome as usize].saturating_add(shares_to_sell);
        
        let price_impact = if initial_price > new_price {
            ((initial_price - new_price) * 10_000) / initial_price
        } else {
            0
        };
        
        if price_impact > max_slippage_bps as u64 {
            return Err(BettingPlatformError::SlippageExceeded.into());
        }
    }

    // Transfer payout to trader
    **market_account.lamports.borrow_mut() = market_account
        .lamports()
        .checked_sub(payout_before_fees)
        .ok_or(BettingPlatformError::Overflow)?;
    
    **trader.lamports.borrow_mut() = trader
        .lamports()
        .checked_add(net_payout)
        .ok_or(BettingPlatformError::Overflow)?;

    // Update market shares
    market.shares[params.outcome as usize] = 
        market.shares[params.outcome as usize].saturating_sub(shares_to_sell);
    
    // Update cost basis
    market.cost_basis = market.cost_basis.saturating_sub(payout_before_fees);

    Ok((shares_to_sell, payout_before_fees, fee_amount))
}

/// Update or create position
fn update_position(
    program_id: &Pubkey,
    position_account: &AccountInfo,
    trader: &Pubkey,
    market_id: u128,
    outcome: u8,
    shares_traded: u64,
    is_buy: bool,
    cost: u64,
) -> ProgramResult {
    // Derive position PDA
    let position_index = 0u8; // For simplicity, using index 0 for AMM positions
    let (position_pda, _) = PositionPDA::derive(program_id, trader, market_id, position_index);
    
    if position_account.key != &position_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Load or initialize position
    let mut position = if position_account.data_len() > 0 {
        Position::try_from_slice(&position_account.data.borrow())?
    } else {
        // Create new position account if needed
        Position::new(
            *trader,
            market_id,  // Using market_id as proposal_id for AMM
            0,  // verse_id - 0 for AMM trades
            outcome,
            shares_traded,
            1,  // No leverage for AMM trades
            cost / shares_traded.max(1),  // Average price
            is_buy,
            Clock::get()?.unix_timestamp,
        )
    };

    // Update position
    if is_buy {
        // Calculate new average entry price
        let total_cost = position.size
            .saturating_mul(position.entry_price)
            .saturating_add(cost);
        
        position.size = position.size.saturating_add(shares_traded);
        
        if position.size > 0 {
            position.entry_price = total_cost / position.size;
        }
    } else {
        // For sells, reduce position size
        position.size = position.size.saturating_sub(shares_traded);
    }

    position.created_at = Clock::get()?.unix_timestamp;

    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Update user map to track active positions
fn update_user_map(
    program_id: &Pubkey,
    user_map_account: &AccountInfo,
    trader: &Pubkey,
    market_id: u128,
) -> ProgramResult {
    // Derive user map PDA
    let (user_map_pda, _) = UserMapPDA::derive(program_id, trader);
    
    if user_map_account.key != &user_map_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Load or initialize user map
    let mut user_map = if user_map_account.data_len() > 0 {
        UserMap::try_from_slice(&user_map_account.data.borrow())?
    } else {
        UserMap::new(*trader)
    };

    // Add market to active positions if not already present
    user_map.add_position(market_id)?;

    // Save user map
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Validate trade parameters
fn validate_trade_params(params: &TradeParams) -> ProgramResult {
    // Must specify either shares or max_cost for buys
    if params.is_buy && params.shares.is_none() && params.max_cost.is_none() {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Must specify shares for sells
    if !params.is_buy && params.shares.is_none() {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Validate slippage bounds
    if let Some(slippage) = params.max_slippage_bps {
        if slippage > MAX_SLIPPAGE_BPS {
            return Err(BettingPlatformError::InvalidInput.into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_trade_params() {
        // Valid buy with max cost
        let params = TradeParams {
            market_id: 1,
            outcome: 0,
            is_buy: true,
            amount: 1000, // Amount to spend
            shares: None,
            max_cost: Some(1000),
            min_shares: Some(500), // Min shares acceptable
            min_payout: None,
            max_slippage_bps: Some(100),
        };
        assert!(validate_trade_params(&params).is_ok());

        // Invalid buy without shares or max cost
        let params = TradeParams {
            market_id: 1,
            outcome: 0,
            is_buy: true,
            amount: 0, // Invalid: no amount specified
            shares: None,
            max_cost: None,
            min_shares: None,
            min_payout: None,
            max_slippage_bps: None,
        };
        assert!(validate_trade_params(&params).is_err());

        // Valid sell with shares
        let params = TradeParams {
            market_id: 1,
            outcome: 0,
            is_buy: false,
            amount: 100, // Amount of shares to sell
            shares: Some(100),
            max_cost: None,
            min_shares: None,
            min_payout: Some(50),
            max_slippage_bps: Some(200),
        };
        assert!(validate_trade_params(&params).is_ok());
    }
}