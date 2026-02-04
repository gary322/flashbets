//! Close position instruction handler
//!
//! Production-grade implementation of position closing logic

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    error::BettingPlatformError,
    events::{Event, PositionClosed, CloseReason},
    pda::{GlobalConfigPDA, ProposalPDA, PositionPDA, UserMapPDA},
    state::{ProposalPDA as Proposal, Position, UserMap, GlobalConfigPDA as GlobalConfig},
};

use solana_program::{
    clock::Clock,
    sysvar::Sysvar,
};

/// Process close position instruction
pub fn process_close_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_index: u8,
) -> ProgramResult {
    msg!("Processing close position");
    
    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let global_config_info = next_account_info(account_info_iter)?;
    let proposal_info = next_account_info(account_info_iter)?;
    let position_info = next_account_info(account_info_iter)?;
    let user_map_info = next_account_info(account_info_iter)?;
    let vault_info = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(position_info)?;
    validate_writable(user_map_info)?;
    validate_writable(vault_info)?;
    validate_writable(global_config_info)?;
    
    // Load position
    let position = Position::try_from_slice(&position_info.data.borrow())?;
    position.validate()?;
    
    // Validate PDAs
    let (position_pda, _) = PositionPDA::derive(
        program_id,
        user.key,
        position.proposal_id,
        position_index,
    );
    if position_info.key != &position_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let (global_config_pda, _) = GlobalConfigPDA::derive(program_id);
    if global_config_info.key != &global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let (proposal_pda, _) = ProposalPDA::derive(program_id, position.proposal_id);
    if proposal_info.key != &proposal_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let (user_map_pda, _) = UserMapPDA::derive(program_id, user.key);
    if user_map_info.key != &user_map_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load accounts
    let mut global_config = GlobalConfig::try_from_slice(&global_config_info.data.borrow())?;
    let proposal = Proposal::try_from_slice(&proposal_info.data.borrow())?;
    let mut user_map = UserMap::try_from_slice(&user_map_info.data.borrow())?;
    
    // Get current price
    let exit_price = proposal.prices[position.outcome as usize];
    
    // Calculate P&L
    let price_diff = if position.is_long {
        exit_price as i128 - position.entry_price as i128
    } else {
        position.entry_price as i128 - exit_price as i128
    };
    
    let pnl = (price_diff * position.size as i128 * position.leverage as i128) 
        / position.entry_price as i128;
    
    // Calculate payout
    let payout = if pnl >= 0 {
        position.size.saturating_add(pnl as u64)
    } else {
        position.size.saturating_sub((-pnl) as u64)
    };
    
    // Update global state
    let leveraged_size = position.size.saturating_mul(position.leverage);
    global_config.total_oi = global_config.total_oi
        .saturating_sub(leveraged_size as u128);
    
    if payout > 0 {
        // Check vault has enough funds
        if **vault_info.lamports.borrow() < payout {
            return Err(BettingPlatformError::InsufficientVaultBalance.into());
        }
        
        // Transfer payout from vault to user
        **vault_info.lamports.borrow_mut() -= payout;
        **user.lamports.borrow_mut() += payout;
        
        // Update vault balance
        if pnl >= 0 {
            global_config.vault = global_config.vault
                .saturating_sub(pnl as u128);
        } else {
            global_config.vault = global_config.vault
                .saturating_add((-pnl) as u128);
        }
    }
    
    global_config.serialize(&mut &mut global_config_info.data.borrow_mut()[..])?;
    
    // Update volume tracking
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    
    // Check if we need to reset 7-day volume (if last update was over 7 days ago)
    const SEVEN_DAYS_SECONDS: i64 = 7 * 24 * 60 * 60;
    if current_time - user_map.last_volume_update > SEVEN_DAYS_SECONDS {
        user_map.total_volume_7d = 0;
    }
    
    // Add this trade's volume to 7-day tracking
    let trade_volume = position.size.saturating_mul(position.leverage);
    user_map.total_volume_7d = user_map.total_volume_7d.saturating_add(trade_volume);
    user_map.last_volume_update = current_time;
    
    msg!("Updated user volume tracking: {} (7-day total)", user_map.total_volume_7d);
    
    // Remove position from user map
    user_map.remove_position(position.proposal_id)?;
    user_map.serialize(&mut &mut user_map_info.data.borrow_mut()[..])?;
    
    // Close position account and reclaim rent
    let position_lamports = position_info.lamports();
    **position_info.lamports.borrow_mut() = 0;
    **user.lamports.borrow_mut() += position_lamports;
    
    // Zero out position data
    position_info.data.borrow_mut().fill(0);
    
    // Emit event
    PositionClosed {
        user: *user.key,
        position_id: position.position_id,
        exit_price,
        pnl: pnl as i64,
        close_reason: CloseReason::UserInitiated,
    }.emit();
    
    msg!("Position closed successfully. PnL: {}", pnl);
    Ok(())
}