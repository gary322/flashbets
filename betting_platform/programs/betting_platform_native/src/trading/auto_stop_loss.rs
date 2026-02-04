use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    program::invoke_signed,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::Position,
    state::order_accounts::{StopOrder as StateStopOrder, StopOrderType, discriminators},
    pda::seeds::STOP_LOSS,
    instruction::OrderSide,
    math::U64F64,
};

/// Auto stop-loss threshold for high leverage positions (0.1% = 10 basis points)
pub const AUTO_STOP_LOSS_THRESHOLD_BPS: u64 = 10;

/// Minimum leverage that triggers auto stop-loss
pub const AUTO_STOP_LOSS_MIN_LEVERAGE: u8 = 50;

/// Create auto stop-loss for high leverage positions
pub fn create_auto_stop_loss(
    program_id: &Pubkey,
    position: &Position,
    leverage: u8,
    entry_price: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    // Only create auto stop-loss for high leverage positions
    if leverage < AUTO_STOP_LOSS_MIN_LEVERAGE {
        return Ok(());
    }

    // Get accounts
    let user = &accounts[0];
    let stop_loss_account = &accounts[1];
    let system_program = &accounts[2];
    
    // Calculate stop-loss trigger price (0.1% below entry for longs)
    let stop_loss_price = if position.is_long {
        entry_price.saturating_sub(entry_price * AUTO_STOP_LOSS_THRESHOLD_BPS / 10000)
    } else {
        entry_price.saturating_add(entry_price * AUTO_STOP_LOSS_THRESHOLD_BPS / 10000)
    };
    
    msg!("Creating auto stop-loss for high leverage position: leverage={}, stop_price={}", 
         leverage, stop_loss_price);
    
    // Create stop-loss order PDA
    let (stop_loss_pda, bump) = Pubkey::find_program_address(
        &[
            STOP_LOSS,
            user.key.as_ref(),
            &position.proposal_id.to_le_bytes(),
            &position.outcome.to_le_bytes(),
            b"auto",
        ],
        program_id,
    );
    
    // Verify PDA matches
    if stop_loss_pda != *stop_loss_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Create stop-loss account
    let rent = Rent::get()?;
    let space = std::mem::size_of::<StateStopOrder>();
    let lamports = rent.minimum_balance(space);
    
    invoke_signed(
        &system_instruction::create_account(
            user.key,
            stop_loss_account.key,
            lamports,
            space as u64,
            program_id,
        ),
        &[
            user.clone(),
            stop_loss_account.clone(),
            system_program.clone(),
        ],
        &[&[
            STOP_LOSS,
            user.key.as_ref(),
            &position.proposal_id.to_le_bytes(),
            &position.outcome.to_le_bytes(),
            b"auto",
            &[bump],
        ]],
    )?;
    
    // Initialize stop-loss order
    let stop_loss_order = StateStopOrder {
        discriminator: discriminators::STOP_ORDER,
        order_id: {
            let mut hasher_input = Vec::new();
            hasher_input.extend_from_slice(user.key.as_ref());
            hasher_input.extend_from_slice(&position.proposal_id.to_le_bytes());
            hasher_input.extend_from_slice(&position.outcome.to_le_bytes());
            hasher_input.extend_from_slice(b"auto");
            solana_program::hash::hash(&hasher_input).to_bytes()
        },
        market_id: {
            let mut market_id = [0u8; 32];
            market_id[..16].copy_from_slice(&position.proposal_id.to_le_bytes());
            market_id
        },
        user: *user.key,
        order_type: StopOrderType::StopLoss,
        side: OrderSide::Sell, // Stop loss is always a sell for longs, buy for shorts
        size: position.size,
        trigger_price: stop_loss_price,
        is_active: true,
        prepaid_bounty: 0, // No bounty for auto stop-loss, protocol pays keeper
        position_entry_price: entry_price,
        trailing_distance: 0,
        trailing_price: 0,
    };
    
    stop_loss_order.serialize(&mut &mut stop_loss_account.data.borrow_mut()[..])?;
    
    // Emit event  
    msg!("Auto stop-loss created: user={}, trigger_price={}, leverage={}", 
        user.key, stop_loss_price, leverage);
    
    msg!("Auto stop-loss created successfully at price {}", stop_loss_price);
    Ok(())
}

/// Check if position needs auto stop-loss
pub fn needs_auto_stop_loss(leverage: u8) -> bool {
    leverage >= AUTO_STOP_LOSS_MIN_LEVERAGE
}

/// Calculate stop-loss price for position
pub fn calculate_stop_loss_price(
    entry_price: u64,
    is_long: bool,
    threshold_bps: u64,
) -> u64 {
    if is_long {
        entry_price.saturating_sub(entry_price * threshold_bps / 10000)
    } else {
        entry_price.saturating_add(entry_price * threshold_bps / 10000)
    }
}