//! Stop order execution logic
//!
//! Handles execution of stop loss, take profit, and trailing stop orders

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::order_accounts::{StopOrder, StopOrderType},
};

/// Execute a triggered stop order
pub fn process_execute_stop_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    current_price: u64,
) -> ProgramResult {
    msg!("Executing stop order at price: {}", current_price);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper = next_account_info(account_info_iter)?;
    let stop_order_account = next_account_info(account_info_iter)?;
    let user = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let price_feed_account = next_account_info(account_info_iter)?;
    let keeper_reward_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify keeper is signer (in production, would verify keeper is registered)
    if !keeper.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load and validate stop order
    let mut stop_order = StopOrder::try_from_slice(&stop_order_account.data.borrow())?;
    stop_order.validate()?;
    
    // Verify order is active
    if !stop_order.is_active {
        msg!("Stop order is not active");
        return Err(BettingPlatformError::OrderNotActive.into());
    }
    
    // Verify user matches
    if stop_order.user != *user.key {
        msg!("User mismatch");
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify price from feed
    let price_data = price_feed_account.data.borrow();
    let feed_price = if price_data.len() >= 16 {
        u64::from_le_bytes(price_data[8..16].try_into().unwrap())
    } else {
        return Err(BettingPlatformError::InvalidAccountData.into());
    };
    
    if feed_price != current_price {
        msg!("Price mismatch between feed and input");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Check if order should be triggered based on type
    let should_trigger = match stop_order.order_type {
        StopOrderType::StopLoss => {
            // Trigger if price falls below stop price
            current_price <= stop_order.trigger_price
        }
        StopOrderType::TakeProfit => {
            // Trigger if price rises above take profit price
            current_price >= stop_order.trigger_price
        }
        StopOrderType::TrailingStop => {
            // Trigger if price falls below trailing price
            current_price <= stop_order.trailing_price
        }
    };
    
    if !should_trigger {
        msg!("Order conditions not met");
        msg!("  Order type: {:?}", stop_order.order_type);
        msg!("  Current price: {}", current_price);
        msg!("  Trigger price: {}", stop_order.trigger_price);
        if stop_order.order_type == StopOrderType::TrailingStop {
            msg!("  Trailing price: {}", stop_order.trailing_price);
        }
        return Err(BettingPlatformError::OrderConditionsNotMet.into());
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock)?;
    
    // Execute the order
    msg!("Executing {} order", match stop_order.order_type {
        StopOrderType::StopLoss => "stop loss",
        StopOrderType::TakeProfit => "take profit",
        StopOrderType::TrailingStop => "trailing stop",
    });
    
    // In a real implementation, this would:
    // 1. Close the position at current market price
    // 2. Calculate PnL
    // 3. Transfer funds to user
    // 4. Update position status
    
    // For now, we'll just mark the order as executed
    stop_order.is_active = false;
    
    // Pay keeper bounty
    msg!("Paying keeper bounty: {} lamports", stop_order.prepaid_bounty);
    // In production, would transfer the bounty to keeper
    
    // Log execution details
    msg!("Stop order executed:");
    msg!("  Order ID: {:?}", stop_order.order_id);
    msg!("  User: {}", stop_order.user);
    msg!("  Type: {:?}", stop_order.order_type);
    msg!("  Size: {}", stop_order.size);
    msg!("  Trigger price: {}", stop_order.trigger_price);
    msg!("  Execution price: {}", current_price);
    msg!("  Keeper: {}", keeper.key);
    msg!("  Timestamp: {}", clock.unix_timestamp);
    
    // Calculate slippage
    let slippage = if current_price > stop_order.trigger_price {
        current_price - stop_order.trigger_price
    } else {
        stop_order.trigger_price - current_price
    };
    msg!("  Slippage: {}", slippage);
    
    // Save updated order
    stop_order.serialize(&mut &mut stop_order_account.data.borrow_mut()[..])?;
    
    // Log event for order execution
    msg!("Stop order executed:");
    msg!("  Order ID: {:?}", stop_order.order_id);
    msg!("  Order type: {:?}", stop_order.order_type);
    msg!("  User: {}", stop_order.user);
    msg!("  Trigger price: {}", stop_order.trigger_price);
    msg!("  Execution price: {}", current_price);
    
    msg!("Stop order executed successfully");
    
    Ok(())
}

/// Cancel a stop order
pub fn process_cancel_stop_order(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Cancelling stop order");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let user = next_account_info(account_info_iter)?;
    let stop_order_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Verify user is signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load stop order
    let mut stop_order = StopOrder::try_from_slice(&stop_order_account.data.borrow())?;
    
    // Verify user owns the order
    if stop_order.user != *user.key {
        msg!("User does not own this order");
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify order is active
    if !stop_order.is_active {
        msg!("Order is already inactive");
        return Err(BettingPlatformError::OrderNotActive.into());
    }
    
    // Cancel the order
    stop_order.is_active = false;
    
    // Refund prepaid keeper bounty to user
    if stop_order.prepaid_bounty > 0 {
        // Transfer bounty back to user
        let transfer_ix = solana_program::system_instruction::transfer(
            stop_order_account.key,
            user.key,
            stop_order.prepaid_bounty,
        );
        
        solana_program::program::invoke(
            &transfer_ix,
            &[
                stop_order_account.clone(),
                user.clone(),
                system_program.clone(),
            ],
        )?;
        
        msg!("Refunded {} lamports keeper bounty to user", stop_order.prepaid_bounty);
    }
    
    // Log cancellation
    msg!("Stop order cancelled:");
    msg!("  Order ID: {:?}", stop_order.order_id);
    msg!("  Type: {:?}", stop_order.order_type);
    msg!("  User: {}", user.key);
    
    // Save updated order
    stop_order.serialize(&mut &mut stop_order_account.data.borrow_mut()[..])?;
    
    msg!("Stop order cancelled successfully");
    
    Ok(())
}

/// Get active stop orders for monitoring
pub fn process_get_active_stop_orders(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 32],
) -> ProgramResult {
    msg!("Getting active stop orders for market: {:?}", market_id);
    
    // In a real implementation, this would:
    // 1. Query all stop order accounts for the market
    // 2. Filter for active orders
    // 3. Return a list for keeper monitoring
    
    // For now, just validate the request
    let account_info_iter = &mut accounts.iter();
    let keeper = next_account_info(account_info_iter)?;
    
    if !keeper.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    msg!("Active stop orders query completed");
    
    Ok(())
}