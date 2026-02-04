//! Trading User Journey
//! 
//! Complete flow for users executing trades on the platform

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{GlobalConfigPDA, ProposalPDA, Position, UserMap, UserStatsPDA, VersePDA},
    amm::{calculate_price_impact, execute_trade},
    oracle::polymarket::{PolymarketOracle, get_market_prices},
    trading::{calculate_margin_requirement, calculate_liquidation_price, validate_leverage},
    events::{emit_event, EventType, PositionClosed, CloseReason, PositionOpened},
    math::U64F64,
    pda,
};

/// Trading journey state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TradingUserJourney {
    /// User public key
    pub user: Pubkey,
    
    /// Current step
    pub current_step: TradingStep,
    
    /// Selected market
    pub selected_market: Option<[u8; 32]>,
    
    /// Selected outcome
    pub selected_outcome: Option<u8>,
    
    /// Position size
    pub position_size: u64,
    
    /// Leverage
    pub leverage: u64,
    
    /// Entry price
    pub entry_price: Option<u64>,
    
    /// Position ID
    pub position_id: Option<[u8; 32]>,
    
    /// Journey timestamp
    pub timestamp: i64,
}

/// Trading journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum TradingStep {
    /// Initial state
    NotStarted,
    
    /// Browsing markets
    BrowsingMarkets,
    
    /// Market selected
    MarketSelected,
    
    /// Analyzing prices
    AnalyzingPrices,
    
    /// Position configured
    PositionConfigured,
    
    /// Trade executed
    TradeExecuted,
    
    /// Monitoring position
    MonitoringPosition,
    
    /// Position closed
    PositionClosed,
}

/// Execute complete trading journey
pub fn execute_trading_journey(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: [u8; 32],
    outcome: u8,
    size: u64,
    leverage: u64,
    is_long: bool,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let verse_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let user_map_account = next_account_info(account_iter)?;
    let user_stats_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    msg!("Starting trading journey for user {}", user_account.key);
    
    // Step 1: Load and validate market
    msg!("Step 1: Loading market data");
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let verse = VersePDA::try_from_slice(&verse_account.data.borrow())?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Verify market is active
    if !proposal.is_active() {
        return Err(BettingPlatformError::MarketHalted.into());
    }
    
    // Verify outcome is valid
    if outcome >= proposal.outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Step 2: Get current prices from oracle
    msg!("Step 2: Fetching current prices from oracle");
    let oracle_prices = get_market_prices(&oracle_account)?;
    
    // Verify oracle spread is acceptable
    let spread = calculate_spread(&oracle_prices)?;
    if spread > 1000 { // 10% max spread
        msg!("Oracle spread too high: {}bps", spread);
        return Err(BettingPlatformError::OracleSpreadTooHigh.into());
    }
    
    // Step 3: Calculate position parameters
    msg!("Step 3: Calculating position parameters");
    
    // Get max leverage from tiers based on outcome count
    let max_leverage = get_max_leverage_from_tiers(&global_config, outcome as u32)?;
    
    // Validate leverage
    validate_leverage(leverage, max_leverage)?;
    
    // Calculate margin requirement
    let margin_required = calculate_margin_requirement(size, leverage)?;
    msg!("Margin required: {} lamports", margin_required);
    
    // Calculate entry price with impact
    let base_price = proposal.prices[outcome as usize];
    let price_impact = calculate_price_impact(&proposal_account.data.borrow(), outcome, size, is_long)?;
    let entry_price = if is_long {
        base_price + price_impact
    } else {
        base_price.saturating_sub(price_impact)
    };
    msg!("Entry price after impact: {}", entry_price);
    
    // Calculate liquidation price
    let liquidation_price = calculate_liquidation_price(
        entry_price,
        leverage,
        is_long,
    )?;
    msg!("Liquidation price: {}", liquidation_price);
    
    // Step 4: Transfer margin to vault
    msg!("Step 4: Transferring margin to vault");
    solana_program::program::invoke(
        &solana_program::system_instruction::transfer(
            user_account.key,
            vault_account.key,
            margin_required,
        ),
        &[user_account.clone(), vault_account.clone(), system_program.clone()],
    )?;
    
    // Step 5: Create position
    msg!("Step 5: Creating position");
    let position = Position::new(
        *user_account.key,
        u128::from_le_bytes(proposal.proposal_id[..16].try_into().unwrap()),
        u128::from_le_bytes(verse.verse_id.to_le_bytes()[..16].try_into().unwrap()),
        outcome,
        size,
        leverage,
        entry_price,
        is_long,
        Clock::get()?.unix_timestamp,
    );
    
    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Step 6: Update user map
    msg!("Step 6: Updating user position map");
    let mut user_map = if user_map_account.data_len() == 0 {
        UserMap::new(*user_account.key)
    } else {
        UserMap::try_from_slice(&user_map_account.data.borrow())?
    };
    
    user_map.add_position(position.proposal_id)?;
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;
    
    // Step 7: Execute trade on AMM
    msg!("Step 7: Executing trade on AMM");
    let entry_price = execute_trade(
        &mut proposal_account.data.borrow_mut()[..],
        outcome,
        size,
        is_long,
    )?;
    
    // Update proposal volumes
    proposal.volumes[outcome as usize] += size;
    
    // Step 8: Update global state
    msg!("Step 8: Updating global state");
    global_config.total_oi += size as u128;
    
    // Step 9: Update user stats
    msg!("Step 9: Updating user statistics");
    let mut user_stats = if user_stats_account.data_len() == 0 {
        UserStatsPDA::new(*user_account.key)
    } else {
        UserStatsPDA::try_from_slice(&user_stats_account.data.borrow())?
    };
    
    user_stats.total_positions += 1;
    user_stats.total_volume += size;
    user_stats.total_fees += calculate_fees(size, &global_config)?;
    user_stats.last_activity = Clock::get()?.unix_timestamp;
    
    // Save all state
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    user_stats.serialize(&mut &mut user_stats_account.data.borrow_mut()[..])?;
    
    // Step 10: Emit position opened event
    let proposal_id = u128::from_le_bytes(market_id[0..16].try_into().unwrap());
    emit_event(EventType::PositionOpened, &PositionOpened {
        user: *user_account.key,
        proposal_id,
        outcome,
        size,
        leverage,
        entry_price,
        is_long,
        position_id: position.position_id,
        chain_id: None,
    });
    
    msg!("Trading journey completed successfully!");
    msg!("Position ID: {:?}", position.position_id);
    msg!("Entry price: {}", entry_price);
    msg!("Liquidation price: {}", liquidation_price);
    msg!("Margin: {}", margin_required);
    
    Ok(())
}

/// Monitor position and calculate PnL
pub fn monitor_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;
    
    // Load position
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    
    // Verify ownership
    if position.user != *user_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load proposal for current prices
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Get current price
    let current_price = proposal.prices[position.outcome as usize];
    
    // Calculate PnL
    let pnl = calculate_pnl(&position, current_price)?;
    let pnl_percentage = (pnl as i64 * 10000) / position.margin as i64;
    
    // Check liquidation
    let should_liquidate = if position.is_long {
        current_price <= position.liquidation_price
    } else {
        current_price >= position.liquidation_price
    };
    
    msg!("Position monitoring report:");
    msg!("Position ID: {:?}", position_id);
    msg!("Entry price: {}", position.entry_price);
    msg!("Current price: {}", current_price);
    msg!("PnL: {} ({} bps)", pnl, pnl_percentage);
    msg!("Liquidation price: {}", position.liquidation_price);
    msg!("Should liquidate: {}", should_liquidate);
    
    // Emit monitoring event as a position opened event (for tracking)
    let position_id_u128 = u128::from_le_bytes(position_id[0..16].try_into().unwrap());
    emit_event(EventType::PositionOpened, &PositionOpened {
        user: position.user,
        proposal_id: position.proposal_id,
        outcome: position.outcome,
        size: position.size,
        leverage: position.leverage,
        entry_price: current_price, // Using current price to indicate monitoring
        is_long: position.is_long,
        position_id: position_id,
        chain_id: None,
    });
    
    Ok(())
}

/// Close position
pub fn close_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let user_map_account = next_account_info(account_iter)?;
    let user_stats_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load position
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    
    // Verify ownership
    if position.user != *user_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Verify not already closed
    if position.is_closed {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load proposal
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Get exit price
    let exit_price = proposal.prices[position.outcome as usize];
    
    // Calculate final PnL
    let pnl = calculate_pnl(&position, exit_price)?;
    let payout = (position.margin as i64 + pnl).max(0) as u64;
    
    msg!("Closing position with PnL: {}", pnl);
    msg!("Payout: {}", payout);
    
    // Execute close trade on AMM
    let exit_price = execute_trade(
        &mut proposal_account.data.borrow_mut()[..],
        position.outcome,
        position.size,
        !position.is_long, // Opposite direction to close
    )?;
    
    // Transfer payout to user
    if payout > 0 {
        solana_program::program::invoke(
            &solana_program::system_instruction::transfer(
                vault_account.key,
                user_account.key,
                payout,
            ),
            &[vault_account.clone(), user_account.clone(), system_program.clone()],
        )?;
    }
    
    // Update position state
    position.is_closed = true;
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Update user map
    let mut user_map = UserMap::try_from_slice(&user_map_account.data.borrow())?;
    user_map.remove_position(position.proposal_id)?;
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;
    
    // Update global OI
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    global_config.total_oi = global_config.total_oi.saturating_sub(position.size as u128);
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    
    // Update user stats
    let mut user_stats = UserStatsPDA::try_from_slice(&user_stats_account.data.borrow())?;
    if pnl > 0 {
        user_stats.win_rate_bps = calculate_new_win_rate(
            user_stats.win_rate_bps,
            user_stats.total_positions,
            true,
        );
    } else {
        user_stats.win_rate_bps = calculate_new_win_rate(
            user_stats.win_rate_bps,
            user_stats.total_positions,
            false,
        );
    }
    user_stats.serialize(&mut &mut user_stats_account.data.borrow_mut()[..])?;
    
    // Save proposal
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Emit close event
    emit_event(EventType::PositionClosed, &PositionClosed {
        user: *user_account.key,
        position_id,
        exit_price,
        pnl,
        close_reason: CloseReason::UserInitiated,
    });
    
    msg!("Position closed successfully!");
    
    Ok(())
}

/// Calculate PnL for a position
fn calculate_pnl(position: &Position, current_price: u64) -> Result<i64, ProgramError> {
    let price_diff = if position.is_long {
        current_price as i64 - position.entry_price as i64
    } else {
        position.entry_price as i64 - current_price as i64
    };
    
    // PnL = price_diff * size * leverage / entry_price
    let pnl = (price_diff * position.size as i64 * position.leverage as i64) / position.entry_price as i64;
    
    Ok(pnl)
}

/// Calculate spread from oracle prices
fn calculate_spread(prices: &[u64]) -> Result<u16, ProgramError> {
    if prices.len() < 2 {
        return Ok(0);
    }
    
    let max_price = prices.iter().max().unwrap();
    let min_price = prices.iter().min().unwrap();
    
    if *max_price == 0 {
        return Ok(0);
    }
    
    let spread_bps = ((max_price - min_price) * 10000) / max_price;
    Ok(spread_bps as u16)
}

/// Calculate trading fees
fn calculate_fees(size: u64, config: &GlobalConfigPDA) -> Result<u64, ProgramError> {
    let base_fee_bps = config.fee_base as u64;
    let fee = (size * base_fee_bps) / 10000;
    Ok(fee)
}

/// Calculate new win rate
fn calculate_new_win_rate(current_win_rate_bps: u16, total_positions: u64, won: bool) -> u16 {
    let current_wins = (current_win_rate_bps as u64 * total_positions) / 10000;
    let new_wins = if won { current_wins + 1 } else { current_wins };
    let new_total = total_positions + 1;
    
    ((new_wins * 10000) / new_total) as u16
}

/// Get max leverage from tiers based on outcome count
fn get_max_leverage_from_tiers(
    config: &GlobalConfigPDA,
    outcome_count: u32,
) -> Result<u64, ProgramError> {
    // Find the appropriate tier based on outcome count
    for tier in &config.leverage_tiers {
        if outcome_count <= tier.n {
            return Ok(tier.max as u64);
        }
    }
    
    // Default to lowest tier if no match
    Ok(5)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pnl_calculation() {
        // Long position
        let mut position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user: Pubkey::default(),
            proposal_id: 0,
            position_id: [0; 32],
            outcome: 0,
            size: 1000,
            notional: 1000,
            leverage: 10,
            entry_price: 500_000, // 0.5
            liquidation_price: 450_000, // 0.45
            is_long: true,
            created_at: 0,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 0,
            margin: 100,
            collateral: 0,
            is_short: false,
            last_mark_price: 500_000, // Same as entry price
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        // Price moves to 0.6 (+20%)
        let pnl = calculate_pnl(&position, 600_000).unwrap();
        assert_eq!(pnl, 200); // 20% of 1000 with 10x leverage = 200
        
        // Short position
        position.is_long = false;
        position.is_short = true;
        
        // Price moves to 0.4 (-20%)
        let pnl = calculate_pnl(&position, 400_000).unwrap();
        assert_eq!(pnl, 200); // 20% of 1000 with 10x leverage = 200
    }
    
    #[test]
    fn test_spread_calculation() {
        let prices = vec![500_000, 510_000]; // 0.5 and 0.51
        let spread = calculate_spread(&prices).unwrap();
        assert_eq!(spread, 200); // 2%
    }
}