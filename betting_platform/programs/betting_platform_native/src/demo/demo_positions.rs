//! Demo position trading functionality
//!
//! Handles opening, closing, and managing demo positions

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
    state::{
        accounts::{ProposalPDA as Market, discriminators},
        Position,
    },
    events::{emit_event, EventType},
    demo::{
        demo_mode::{DemoAccount, DemoPosition, DEMO_MAX_POSITIONS},
        fake_usdc::{
            validate_demo_position_margin, 
            update_demo_balance_on_close,
            calculate_demo_pnl,
        },
    },
};

/// Open a demo position
pub fn process_open_demo_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    size: u64,
    leverage: u8,
    is_long: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let demo_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate demo account PDA
    let (demo_account_key, _) = Pubkey::find_program_address(
        &[b"demo", user.key.as_ref()],
        program_id,
    );
    
    if demo_account_key != *demo_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load accounts
    let mut demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    let market = Market::try_from_slice(&market_account.data.borrow())?;
    
    // Verify ownership
    if demo_data.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if can open position
    if !demo_data.can_open_position() {
        msg!("Cannot open position: max positions reached or account inactive");
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Validate margin requirements
    validate_demo_position_margin(&demo_data, size, leverage)?;
    
    // Get current price from market
    // Get current price from the first outcome (demo uses binary markets)
    let current_price = if market.prices.is_empty() { 0 } else { market.prices[0] };
    if current_price == 0 {
        msg!("Market price not available");
        return Err(BettingPlatformError::InvalidPrice.into());
    }
    
    // Create position ID
    let position_id = Clock::get()?.unix_timestamp as u128;
    
    // Create demo position
    let demo_position = DemoPosition {
        position_id,
        market: *market_account.key,
        size,
        entry_price: current_price,
        leverage,
        is_long,
        opened_at: Clock::get()?.unix_timestamp,
        unrealized_pnl: 0,
    };
    
    // Add to positions
    demo_data.demo_positions.push(demo_position.clone());
    demo_data.positions_opened += 1;
    demo_data.total_volume += size;
    
    // Save
    demo_data.serialize(&mut &mut demo_account.data.borrow_mut()[..])?;
    
    msg!("Demo position opened:");
    msg!("  Position ID: {}", position_id);
    msg!("  Market: {}", market_account.key);
    msg!("  Size: {} USDC", size / 1_000_000);
    msg!("  Leverage: {}x", leverage);
    msg!("  Direction: {}", if is_long { "LONG" } else { "SHORT" });
    msg!("  Entry price: {}", current_price);
    
    // Emit event
    DemoPositionOpened {
        user: *user.key,
        position_id,
        market: *market_account.key,
        size,
        leverage,
        is_long,
        entry_price: current_price,
        timestamp: demo_position.opened_at,
    }.emit();
    
    Ok(())
}

/// Close a demo position
pub fn process_close_demo_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: u128,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let demo_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate demo account PDA
    let (demo_account_key, _) = Pubkey::find_program_address(
        &[b"demo", user.key.as_ref()],
        program_id,
    );
    
    if demo_account_key != *demo_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load accounts
    let mut demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    let market = Market::try_from_slice(&market_account.data.borrow())?;
    
    // Verify ownership
    if demo_data.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Find position
    let position_index = demo_data.demo_positions
        .iter()
        .position(|p| p.position_id == position_id)
        .ok_or(BettingPlatformError::PositionNotFound)?;
    
    let position = demo_data.demo_positions[position_index].clone();
    
    // Verify market matches
    if position.market != *market_account.key {
        return Err(BettingPlatformError::InvalidMarket.into());
    }
    
    // Get current price
    // Get current price from the first outcome (demo uses binary markets)
    let current_price = if market.prices.is_empty() { 0 } else { market.prices[0] };
    if current_price == 0 {
        msg!("Market price not available");
        return Err(BettingPlatformError::InvalidPrice.into());
    }
    
    // Calculate and update balance
    let pnl = update_demo_balance_on_close(&mut demo_data, &position, current_price)?;
    
    // Remove position
    demo_data.demo_positions.remove(position_index);
    
    // Save
    demo_data.serialize(&mut &mut demo_account.data.borrow_mut()[..])?;
    
    msg!("Demo position closed:");
    msg!("  Position ID: {}", position_id);
    msg!("  Exit price: {}", current_price);
    msg!("  PnL: {} USDC", pnl / 1_000_000);
    msg!("  New balance: {} USDC", demo_data.demo_balance / 1_000_000);
    
    // Emit event
    DemoPositionClosed {
        user: *user.key,
        position_id,
        market: position.market,
        entry_price: position.entry_price,
        exit_price: current_price,
        pnl,
        timestamp: Clock::get()?.unix_timestamp,
    }.emit();
    
    Ok(())
}

/// Update demo position prices (called by keeper)
pub fn process_update_demo_positions(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let keeper = next_account_info(account_info_iter)?;
    let demo_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    
    // In production, verify keeper authority
    // For demo purposes, allow any signer
    if !keeper.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load accounts
    let mut demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    let market = Market::try_from_slice(&market_account.data.borrow())?;
    
    // Get current price
    // Get current price from the first outcome (demo uses binary markets)
    let current_price = if market.prices.is_empty() { 0 } else { market.prices[0] };
    if current_price == 0 {
        return Ok(()); // Skip if no price
    }
    
    // Update all positions for this market
    let mut liquidations = Vec::new();
    
    for position in &mut demo_data.demo_positions {
        if position.market != *market_account.key {
            continue;
        }
        
        // Calculate PnL
        let pnl = calculate_demo_pnl(position, current_price);
        position.unrealized_pnl = pnl;
        
        // Check for liquidation
        let margin = position.size / position.leverage as u64;
        if pnl < 0 && (-pnl) as u64 >= margin * 90 / 100 {
            // 90% loss triggers liquidation
            liquidations.push(position.position_id);
        }
    }
    
    // Process liquidations
    for position_id in liquidations {
        msg!("Liquidating demo position {}", position_id);
        
        if let Some(index) = demo_data.demo_positions
            .iter()
            .position(|p| p.position_id == position_id) {
            
            // Clone position data before mutating demo_data
            let position_data = demo_data.demo_positions[index].clone();
            let margin = position_data.size / position_data.leverage as u64;
            
            // Lose entire margin on liquidation
            demo_data.total_pnl -= margin as i64;
            demo_data.update_stats(-(margin as i64), false);
            
            // Emit liquidation event
            DemoPositionLiquidated {
                user: demo_data.user,
                position_id,
                market: position_data.market,
                loss: margin,
                timestamp: Clock::get()?.unix_timestamp,
            }.emit();
            
            // Remove position
            demo_data.demo_positions.remove(index);
        }
    }
    
    // Save
    demo_data.serialize(&mut &mut demo_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Get demo positions for user
pub fn get_demo_positions(
    demo_account: &AccountInfo,
) -> Result<Vec<DemoPositionInfo>, ProgramError> {
    let demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    
    let positions: Vec<DemoPositionInfo> = demo_data.demo_positions
        .iter()
        .map(|p| DemoPositionInfo {
            position_id: p.position_id,
            market: p.market,
            size: p.size,
            entry_price: p.entry_price,
            leverage: p.leverage,
            is_long: p.is_long,
            opened_at: p.opened_at,
            unrealized_pnl: p.unrealized_pnl,
            margin: p.size / p.leverage as u64,
        })
        .collect();
    
    Ok(positions)
}

/// Demo position info for UI
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoPositionInfo {
    pub position_id: u128,
    pub market: Pubkey,
    pub size: u64,
    pub entry_price: u64,
    pub leverage: u8,
    pub is_long: bool,
    pub opened_at: i64,
    pub unrealized_pnl: i64,
    pub margin: u64,
}

/// Demo position opened event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoPositionOpened {
    pub user: Pubkey,
    pub position_id: u128,
    pub market: Pubkey,
    pub size: u64,
    pub leverage: u8,
    pub is_long: bool,
    pub entry_price: u64,
    pub timestamp: i64,
}

impl DemoPositionOpened {
    pub fn emit(&self) {
        emit_event(EventType::DemoPositionOpened, self);
    }
}

/// Demo position closed event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoPositionClosed {
    pub user: Pubkey,
    pub position_id: u128,
    pub market: Pubkey,
    pub entry_price: u64,
    pub exit_price: u64,
    pub pnl: i64,
    pub timestamp: i64,
}

impl DemoPositionClosed {
    pub fn emit(&self) {
        emit_event(EventType::DemoPositionClosed, self);
    }
}

/// Demo position liquidated event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoPositionLiquidated {
    pub user: Pubkey,
    pub position_id: u128,
    pub market: Pubkey,
    pub loss: u64,
    pub timestamp: i64,
}

impl DemoPositionLiquidated {
    pub fn emit(&self) {
        emit_event(EventType::DemoPositionLiquidated, self);
    }
}