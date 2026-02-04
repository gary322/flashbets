//! Trailing stop order implementation

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::order_accounts::{StopOrder, StopOrderType, discriminators},
    instruction::OrderSide,
};

pub fn process_place_trailing_stop(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    trailing_distance: u64,
) -> ProgramResult {
    msg!("Placing trailing stop order with distance: {}", trailing_distance);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let user = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let stop_order_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let price_feed_account = next_account_info(account_info_iter)?; // For current price
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify user is signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Extract position data to validate
    let position_data = position_account.data.borrow();
    if position_data.len() < 40 { // Basic size check
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    
    // Extract market ID from market account
    let market_data = market_account.data.borrow();
    if market_data.len() < 40 { // 8 byte discriminator + 32 byte market_id
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    let mut market_id_bytes = [0u8; 32];
    market_id_bytes.copy_from_slice(&market_data[8..40]);
    
    // Extract current price from price feed
    let price_data = price_feed_account.data.borrow();
    let current_price = if price_data.len() >= 16 {
        u64::from_le_bytes(price_data[8..16].try_into().unwrap())
    } else {
        return Err(BettingPlatformError::InvalidAccountData.into());
    };
    
    // Generate order ID (using user pubkey and current time)
    let clock = Clock::from_account_info(clock)?;
    let order_id_seed = [user.key.as_ref(), &clock.unix_timestamp.to_le_bytes(), b"ts"].concat();
    let order_id = solana_program::hash::hash(&order_id_seed).to_bytes();
    
    // Derive stop order PDA
    let (stop_order_pda, bump_seed) = Pubkey::find_program_address(
        &[b"stop_order", user.key.as_ref(), &order_id],
        program_id,
    );
    
    // Verify PDA matches
    if stop_order_pda != *stop_order_account.key {
        msg!("Invalid stop order PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if order already exists
    if stop_order_account.data_len() > 0 {
        msg!("Trailing stop order already exists");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate required space
    let stop_order_size = std::mem::size_of::<StopOrder>();
    
    // Create account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(stop_order_size);
    
    invoke_signed(
        &system_instruction::create_account(
            user.key,
            stop_order_account.key,
            rent_lamports,
            stop_order_size as u64,
            program_id,
        ),
        &[
            user.clone(),
            stop_order_account.clone(),
            system_program.clone(),
        ],
        &[&[b"stop_order", user.key.as_ref(), &order_id, &[bump_seed]]],
    )?;
    
    // Extract position size from position account
    let position_size = if position_data.len() >= 48 {
        u64::from_le_bytes(position_data[40..48].try_into().unwrap())
    } else {
        return Err(BettingPlatformError::InvalidAccountData.into());
    };
    
    // Extract position entry price
    let entry_price = if position_data.len() >= 56 {
        u64::from_le_bytes(position_data[48..56].try_into().unwrap())
    } else {
        0
    };
    
    // Validate trailing distance
    if trailing_distance == 0 {
        msg!("Invalid trailing distance");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate initial trailing price (current price - trailing distance)
    let initial_trailing_price = current_price.saturating_sub(trailing_distance);
    
    // Create trailing stop order
    let stop_order = StopOrder {
        discriminator: discriminators::STOP_ORDER,
        order_id,
        market_id: market_id_bytes,
        user: *user.key,
        order_type: StopOrderType::TrailingStop,
        side: OrderSide::Sell, // Trailing stop sells to protect profits
        size: position_size,
        trigger_price: initial_trailing_price,
        is_active: true,
        prepaid_bounty: 150_000, // 0.00015 SOL - higher bounty for trailing stops due to updates
        position_entry_price: entry_price,
        trailing_distance,
        trailing_price: initial_trailing_price,
    };
    
    // Log order details
    msg!("Trailing stop order created:");
    msg!("  Order ID: {:?}", order_id);
    msg!("  User: {}", user.key);
    msg!("  Market ID: {:?}", market_id_bytes);
    msg!("  Size: {}", position_size);
    msg!("  Current price: {}", current_price);
    msg!("  Trailing distance: {}", trailing_distance);
    msg!("  Initial trailing price: {}", initial_trailing_price);
    msg!("  Keeper bounty: {}", stop_order.prepaid_bounty);
    
    // Serialize and save
    stop_order.serialize(&mut &mut stop_order_account.data.borrow_mut()[..])?;
    
    msg!("Trailing stop order placed successfully");
    
    // Add to keeper monitoring queue for price updates
    // Log event for keeper network to monitor
    msg!("Trailing stop order created:");
    msg!("  Order ID: {:?}", stop_order.order_id);
    msg!("  User: {}", user.key);
    msg!("  Position size: {}", position_size);
    msg!("  Initial trigger price: {}", stop_order.trigger_price);
    msg!("  Trail distance: {}", stop_order.trailing_distance);
    
    Ok(())
}

/// Update trailing stop price when market moves favorably
pub fn process_update_trailing_stop(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_market_price: u64,
) -> ProgramResult {
    msg!("Updating trailing stop with new market price: {}", new_market_price);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper = next_account_info(account_info_iter)?;
    let stop_order_account = next_account_info(account_info_iter)?;
    let price_feed_account = next_account_info(account_info_iter)?;
    
    // Verify keeper is authorized (in production, would check keeper registry)
    if !keeper.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load stop order
    let mut stop_order = StopOrder::try_from_slice(&stop_order_account.data.borrow())?;
    
    // Verify it's a trailing stop
    if stop_order.order_type != StopOrderType::TrailingStop {
        msg!("Not a trailing stop order");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Verify order is active
    if !stop_order.is_active {
        msg!("Order is not active");
        return Err(BettingPlatformError::OrderNotActive.into());
    }
    
    // Verify price from feed matches
    let price_data = price_feed_account.data.borrow();
    let feed_price = if price_data.len() >= 16 {
        u64::from_le_bytes(price_data[8..16].try_into().unwrap())
    } else {
        return Err(BettingPlatformError::InvalidAccountData.into());
    };
    
    if feed_price != new_market_price {
        msg!("Price mismatch between feed and input");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate new trailing price
    let new_trailing_price = new_market_price.saturating_sub(stop_order.trailing_distance);
    
    // Only update if new trailing price is higher (price moved up)
    if new_trailing_price > stop_order.trailing_price {
        let old_price = stop_order.trailing_price;
        stop_order.trailing_price = new_trailing_price;
        stop_order.trigger_price = new_trailing_price;
        
        msg!("Trailing stop updated:");
        msg!("  Old trailing price: {}", old_price);
        msg!("  New trailing price: {}", new_trailing_price);
        msg!("  Market price: {}", new_market_price);
        msg!("  Trailing distance: {}", stop_order.trailing_distance);
        
        // Save updated order
        stop_order.serialize(&mut &mut stop_order_account.data.borrow_mut()[..])?;
    } else {
        msg!("Price did not move favorably, no update needed");
    }
    
    Ok(())
}