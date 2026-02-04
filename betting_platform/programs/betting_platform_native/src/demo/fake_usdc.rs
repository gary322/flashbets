//! Fake USDC management for demo mode
//!
//! Manages fake USDC balances and transfers for paper trading

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
    state::accounts::discriminators,
    events::{emit_event, EventType},
    demo::demo_mode::{DemoAccount, DemoPosition, DEMO_MAX_LEVERAGE},
};

/// Mint fake USDC to demo account
pub fn process_mint_demo_usdc(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let demo_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate PDA
    let (demo_account_key, _) = Pubkey::find_program_address(
        &[b"demo", user.key.as_ref()],
        program_id,
    );
    
    if demo_account_key != *demo_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load demo account
    let mut demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    
    // Verify ownership
    if demo_data.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if account is active
    if !demo_data.is_active {
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Limit minting to prevent abuse (max 100k USDC total)
    const MAX_DEMO_BALANCE: u64 = 100_000_000_000; // 100k USDC
    if demo_data.demo_balance + amount > MAX_DEMO_BALANCE {
        msg!("Demo balance would exceed maximum of 100k USDC");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Mint fake USDC
    let old_balance = demo_data.demo_balance;
    demo_data.demo_balance += amount;
    
    // Save
    demo_data.serialize(&mut &mut demo_account.data.borrow_mut()[..])?;
    
    msg!("Minted {} fake USDC to demo account", amount / 1_000_000);
    msg!("New balance: {} USDC", demo_data.demo_balance / 1_000_000);
    
    // Emit event
    DemoUsdcMinted {
        user: *user.key,
        amount,
        old_balance,
        new_balance: demo_data.demo_balance,
        timestamp: Clock::get()?.unix_timestamp,
    }.emit();
    
    Ok(())
}

/// Transfer fake USDC between demo accounts
pub fn process_transfer_demo_usdc(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let sender = next_account_info(account_info_iter)?;
    let sender_demo_account = next_account_info(account_info_iter)?;
    let receiver_demo_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !sender.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate sender PDA
    let (sender_demo_key, _) = Pubkey::find_program_address(
        &[b"demo", sender.key.as_ref()],
        program_id,
    );
    
    if sender_demo_key != *sender_demo_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load accounts
    let mut sender_data = DemoAccount::try_from_slice(&sender_demo_account.data.borrow())?;
    let mut receiver_data = DemoAccount::try_from_slice(&receiver_demo_account.data.borrow())?;
    
    // Verify ownership
    if sender_data.user != *sender.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check accounts are active
    if !sender_data.is_active || !receiver_data.is_active {
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Check balance
    if sender_data.demo_balance < amount {
        msg!("Insufficient demo balance: {} < {}", sender_data.demo_balance, amount);
        return Err(BettingPlatformError::InsufficientFunds.into());
    }
    
    // Transfer
    sender_data.demo_balance -= amount;
    receiver_data.demo_balance += amount;
    
    // Save accounts
    sender_data.serialize(&mut &mut sender_demo_account.data.borrow_mut()[..])?;
    receiver_data.serialize(&mut &mut receiver_demo_account.data.borrow_mut()[..])?;
    
    msg!("Transferred {} fake USDC", amount / 1_000_000);
    msg!("Sender new balance: {} USDC", sender_data.demo_balance / 1_000_000);
    msg!("Receiver new balance: {} USDC", receiver_data.demo_balance / 1_000_000);
    
    // Emit event
    DemoUsdcTransferred {
        sender: sender_data.user,
        receiver: receiver_data.user,
        amount,
        sender_balance: sender_data.demo_balance,
        receiver_balance: receiver_data.demo_balance,
        timestamp: Clock::get()?.unix_timestamp,
    }.emit();
    
    Ok(())
}

/// Get available demo balance for margin
pub fn get_available_demo_margin(
    demo_account: &DemoAccount,
) -> Result<u64, ProgramError> {
    // Calculate margin used by open positions
    let mut used_margin = 0u64;
    
    for position in &demo_account.demo_positions {
        // Margin = size / leverage
        let position_margin = position.size / position.leverage as u64;
        used_margin += position_margin;
    }
    
    // Available = total balance - used margin
    let available = demo_account.demo_balance.saturating_sub(used_margin);
    
    Ok(available)
}

/// Check if demo account can open position with given parameters
pub fn validate_demo_position_margin(
    demo_account: &DemoAccount,
    size: u64,
    leverage: u8,
) -> Result<(), ProgramError> {
    // Check leverage limit
    if leverage > DEMO_MAX_LEVERAGE {
        msg!("Leverage {} exceeds demo limit of {}", leverage, DEMO_MAX_LEVERAGE);
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    // Calculate required margin
    let required_margin = size / leverage as u64;
    
    // Get available margin
    let available_margin = get_available_demo_margin(demo_account)?;
    
    if required_margin > available_margin {
        msg!("Insufficient margin: required {} > available {}", 
            required_margin / 1_000_000, 
            available_margin / 1_000_000
        );
        return Err(BettingPlatformError::InsufficientFunds.into());
    }
    
    Ok(())
}

/// Calculate PnL for demo position
pub fn calculate_demo_pnl(
    position: &DemoPosition,
    current_price: u64,
) -> i64 {
    let price_diff = if position.is_long {
        current_price as i64 - position.entry_price as i64
    } else {
        position.entry_price as i64 - current_price as i64
    };
    
    // PnL = (price_diff / entry_price) * size * leverage
    let pnl = (price_diff * position.size as i64 * position.leverage as i64) / position.entry_price as i64;
    
    pnl
}

/// Update demo account balance after position close
pub fn update_demo_balance_on_close(
    demo_account: &mut DemoAccount,
    position: &DemoPosition,
    current_price: u64,
) -> Result<i64, ProgramError> {
    // Calculate PnL
    let pnl = calculate_demo_pnl(position, current_price);
    
    // Return margin
    let margin = position.size / position.leverage as u64;
    
    // Update balance
    if pnl >= 0 {
        demo_account.demo_balance += margin + pnl as u64;
    } else {
        let loss = (-pnl) as u64;
        if loss > margin {
            // Position liquidated - lose entire margin
            demo_account.demo_balance = demo_account.demo_balance;
        } else {
            demo_account.demo_balance += margin - loss;
        }
    }
    
    // Update stats
    demo_account.update_stats(pnl, pnl > 0);
    demo_account.total_volume += position.size;
    
    Ok(pnl)
}

/// Demo USDC minted event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoUsdcMinted {
    pub user: Pubkey,
    pub amount: u64,
    pub old_balance: u64,
    pub new_balance: u64,
    pub timestamp: i64,
}

impl DemoUsdcMinted {
    pub fn emit(&self) {
        emit_event(EventType::DemoUsdcMinted, self);
    }
}

/// Demo USDC transferred event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoUsdcTransferred {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub amount: u64,
    pub sender_balance: u64,
    pub receiver_balance: u64,
    pub timestamp: i64,
}

impl DemoUsdcTransferred {
    pub fn emit(&self) {
        emit_event(EventType::DemoUsdcTransferred, self);
    }
}