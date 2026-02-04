//! Stop-loss order implementation

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

pub fn process_place_stop_loss(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    stop_price: u64,
) -> ProgramResult {
    msg!("Placing stop-loss order at price: {}", stop_price);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let user = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let stop_order_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify user is signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Extract position data to validate
    // In a real implementation, we would deserialize the position and validate ownership
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
    
    // Generate order ID (using user pubkey and current time)
    let clock = Clock::from_account_info(clock)?;
    let order_id_seed = [user.key.as_ref(), &clock.unix_timestamp.to_le_bytes()].concat();
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
        msg!("Stop order already exists");
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
    
    // Extract position size from position account (offset 40 for example)
    let position_size = if position_data.len() >= 48 {
        u64::from_le_bytes(position_data[40..48].try_into().unwrap())
    } else {
        return Err(BettingPlatformError::InvalidAccountData.into());
    };
    
    // Create stop order
    let stop_order = StopOrder {
        discriminator: discriminators::STOP_ORDER,
        order_id,
        market_id: market_id_bytes,
        user: *user.key,
        order_type: StopOrderType::StopLoss,
        side: OrderSide::Sell, // Stop loss always sells
        size: position_size,
        trigger_price: stop_price,
        is_active: true,
        prepaid_bounty: 100_000, // 0.0001 SOL keeper incentive
        position_entry_price: 0, // Would be extracted from position
        trailing_distance: 0, // Not used for basic stop loss
        trailing_price: 0,
    };
    
    // Validate stop order
    if stop_price == 0 {
        msg!("Invalid stop price");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Log order details
    msg!("Stop loss order created:");
    msg!("  Order ID: {:?}", order_id);
    msg!("  User: {}", user.key);
    msg!("  Market ID: {:?}", market_id_bytes);
    msg!("  Size: {}", position_size);
    msg!("  Trigger price: {}", stop_price);
    msg!("  Keeper bounty: {}", stop_order.prepaid_bounty);
    
    // Serialize and save
    stop_order.serialize(&mut &mut stop_order_account.data.borrow_mut()[..])?;
    
    msg!("Stop loss order placed successfully");
    
    // Add to keeper monitoring queue
    // Log event for keeper network to monitor
    msg!("Stop loss order created:");
    msg!("  Order ID: {:?}", stop_order.order_id);
    msg!("  User: {}", user.key);
    msg!("  Position size: {}", position_size);
    msg!("  Trigger price: {}", stop_order.trigger_price);
    
    Ok(())
}