//! Perpetual Instructions
//!
//! Entry points for perpetual trading operations

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    cdp::CDPAccount,
};

use super::{
    state::{
        PerpetualPosition, PerpetualMarket, PositionType, 
        derive_perpetual_position_pda, derive_perpetual_market_pda
    },
    position::{
        open_position, close_position, modify_position,
        add_stop_loss, add_take_profit, check_triggers,
    },
    rolling::{execute_auto_roll, configure_auto_roll, RollStrategy},
    funding::{process_market_funding, FundingConfig},
    settlement::{settle_expired_position, SettlementConfig},
};

/// Create a new perpetual market
pub fn create_perpetual_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    base_token: Pubkey,
    quote_token: Pubkey,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let market_account = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Derive and verify PDA
    let (pda, _bump) = derive_perpetual_market_pda(program_id, market_id);
    if pda != *market_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create market
    let market = PerpetualMarket::new(
        market_id,
        base_token,
        quote_token,
        *oracle_account.key,
    );
    
    // Serialize and save
    let mut data = market_account.try_borrow_mut_data()?;
    market.serialize(&mut &mut data[..])?;
    
    msg!("Created perpetual market {}", market_id);
    
    Ok(())
}

/// Open a perpetual position
pub fn open_perpetual_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_type: PositionType,
    size: u128,
    leverage: u16,
    collateral: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let market_account = next_account_info(account_iter)?;
    let cdp_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut market = PerpetualMarket::deserialize(&mut &market_account.data.borrow()[..])?;
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Open position
    let position = open_position(
        program_id,
        owner.key,
        &mut market,
        &mut cdp,
        &oracle,
        position_type,
        size,
        leverage,
        collateral,
    )?;
    
    // Save position
    let mut data = position_account.try_borrow_mut_data()?;
    position.serialize(&mut &mut data[..])?;
    
    // Save updated market and CDP
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Opened perpetual position {} with {}x leverage", 
         position.position_id, leverage);
    
    Ok(())
}

/// Close a perpetual position
pub fn close_perpetual_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let market_account = next_account_info(account_iter)?;
    let cdp_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    let mut market = PerpetualMarket::deserialize(&mut &market_account.data.borrow()[..])?;
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Close position
    let final_pnl = close_position(
        program_id,
        &mut position,
        &mut market,
        &mut cdp,
        &oracle,
    )?;
    
    // Save updated accounts
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Closed position {} with PnL: {}", position.position_id, final_pnl);
    
    Ok(())
}

/// Modify position parameters
pub fn modify_perpetual_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_size: Option<u128>,
    new_leverage: Option<u16>,
    add_collateral_amount: Option<u128>,
    remove_collateral_amount: Option<u128>,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let market_account = next_account_info(account_iter)?;
    let cdp_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    let mut market = PerpetualMarket::deserialize(&mut &market_account.data.borrow()[..])?;
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Modify position
    modify_position(
        &mut position,
        &mut market,
        &mut cdp,
        &oracle,
        new_size,
        new_leverage,
        add_collateral_amount,
        remove_collateral_amount,
    )?;
    
    // Save updated accounts
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Modified position {}", position.position_id);
    
    Ok(())
}

/// Set stop loss for position
pub fn set_stop_loss(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    stop_price: f64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load position
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Set stop loss
    add_stop_loss(&mut position, stop_price)?;
    
    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    msg!("Set stop loss at {} for position {}", stop_price, position.position_id);
    
    Ok(())
}

/// Set take profit for position
pub fn set_take_profit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    target_price: f64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load position
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Set take profit
    add_take_profit(&mut position, target_price)?;
    
    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    msg!("Set take profit at {} for position {}", target_price, position.position_id);
    
    Ok(())
}

/// Configure auto-roll for position
pub fn configure_position_auto_roll(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    enabled: bool,
    roll_params: Option<super::state::RollParameters>,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load position
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Configure auto-roll
    configure_auto_roll(&mut position, enabled, roll_params)?;
    
    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    msg!("Configured auto-roll for position {}: enabled={}", 
         position.position_id, enabled);
    
    Ok(())
}

/// Execute position roll
pub fn execute_position_roll(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    strategy: RollStrategy,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let market_account = next_account_info(account_iter)?;
    let cdp_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    let mut market = PerpetualMarket::deserialize(&mut &market_account.data.borrow()[..])?;
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Execute roll
    execute_auto_roll(
        program_id,
        &mut position,
        &mut market,
        &mut cdp,
        &oracle,
        strategy,
    )?;
    
    // Save updated accounts
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Rolled position {} (roll #{})", 
         position.position_id, position.roll_count);
    
    Ok(())
}

/// Process funding payments
pub fn process_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let market_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut market = PerpetualMarket::deserialize(&mut &market_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Process funding with default config
    let config = FundingConfig::default();
    
    // In production, would load all positions and process funding
    // For now, just update market funding rate
    let funding_rate = super::funding::calculate_funding_rate(&market, &oracle, &config);
    market.funding_rate = funding_rate;
    market.next_funding_time = Clock::get()?.unix_timestamp + config.interval as i64;
    
    // Save market
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    
    msg!("Processed funding: new rate={:.6}", funding_rate);
    
    Ok(())
}

/// Settle expired position
pub fn settle_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let position_account = next_account_info(account_iter)?;
    let owner = next_account_info(account_iter)?;
    let market_account = next_account_info(account_iter)?;
    let cdp_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    
    // Verify signer
    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut position = PerpetualPosition::deserialize(&mut &position_account.data.borrow()[..])?;
    let market = PerpetualMarket::deserialize(&mut &market_account.data.borrow()[..])?;
    let mut cdp = CDPAccount::deserialize(&mut &cdp_account.data.borrow()[..])?;
    let oracle = OraclePDA::try_from_slice(&oracle_account.data.borrow())?;
    
    // Verify owner
    if position.owner != *owner.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Settle with default config
    let config = SettlementConfig::default();
    let result = settle_expired_position(
        &mut position,
        &market,
        &mut cdp,
        &oracle,
        &config,
    )?;
    
    // Save updated accounts
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    cdp.serialize(&mut &mut cdp_account.data.borrow_mut()[..])?;
    
    msg!("Settled position {} with final PnL: {}", 
         result.position_id, result.final_pnl);
    
    Ok(())
}