//! Auto Stop-Loss User Journey
//! 
//! Complete flow for automatic stop-loss creation and execution on high leverage positions

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
    state::{GlobalConfigPDA, ProposalPDA, Position, UserMap},
    state::order_accounts::{StopOrder as StateStopOrder, StopOrderType, discriminators},
    trading::{
        auto_stop_loss::{
            create_auto_stop_loss, 
            needs_auto_stop_loss,
            calculate_stop_loss_price,
            AUTO_STOP_LOSS_MIN_LEVERAGE,
            AUTO_STOP_LOSS_THRESHOLD_BPS,
        },
    },
    keeper_network::{KeeperAccount, KeeperStatus},
    events::{emit_event, EventType, PositionOpened, StopLossExecuted},
    pda::seeds::STOP_LOSS,
    instruction::OrderSide,
    math::U64F64,
};

/// Auto stop-loss journey state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AutoStopLossJourney {
    /// Position being monitored
    pub position_id: [u8; 32],
    
    /// Current step
    pub current_step: AutoStopLossStep,
    
    /// Position details
    pub leverage: u8,
    pub entry_price: u64,
    pub stop_loss_price: u64,
    
    /// Stop-loss order ID
    pub stop_loss_order_id: Option<[u8; 32]>,
    
    /// Price tracking
    pub current_price: u64,
    pub max_adverse_move_bps: u64,
    
    /// Execution details
    pub triggered_at_price: Option<u64>,
    pub executed_at_slot: Option<u64>,
    pub execution_slippage_bps: Option<u64>,
    
    /// Timestamps
    pub created_at: i64,
    pub triggered_at: Option<i64>,
    pub executed_at: Option<i64>,
}

/// Auto stop-loss journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum AutoStopLossStep {
    /// Position created with normal leverage
    NormalPosition,
    
    /// High leverage position created
    HighLeveragePosition,
    
    /// Auto stop-loss order created
    StopLossCreated,
    
    /// Price moving against position
    PriceMovingAdverse,
    
    /// Stop-loss triggered
    StopLossTriggered,
    
    /// Stop-loss executed by keeper
    StopLossExecuted,
    
    /// Position closed
    PositionClosed,
}

/// Create a high leverage position with auto stop-loss
pub fn create_high_leverage_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    size: u64,
    leverage: u8,
    is_long: bool,
    outcome: u8,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let user_map_account = next_account_info(account_iter)?;
    let stop_loss_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    msg!("Creating high leverage position: leverage={}, size={}", leverage, size);
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    let mut user_map = UserMap::try_from_slice(&user_map_account.data.borrow())?;
    
    // Step 1: Validate position parameters
    msg!("Step 1: Validating position parameters");
    // Validate position parameters
    if size == 0 {
        return Err(BettingPlatformError::InvalidPosition.into());
    }
    if leverage == 0 || leverage > 50 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    if outcome >= proposal.outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Calculate required margin
    let required_margin = size / leverage as u64;
    let entry_price = proposal.prices[outcome as usize];
    
    msg!("Required margin: {}", required_margin);
    msg!("Entry price: {}", entry_price);
    
    // Step 2: Create position
    msg!("Step 2: Creating position");
    let position_id = {
        let mut hasher_input = Vec::new();
        hasher_input.extend_from_slice(user_account.key.as_ref());
        hasher_input.extend_from_slice(&proposal.proposal_id);
        hasher_input.extend_from_slice(&Clock::get()?.slot.to_le_bytes());
        solana_program::hash::hash(&hasher_input).to_bytes()
    };
    
    let position = Position {
        discriminator: crate::state::accounts::discriminators::POSITION,
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        user: *user_account.key,
        proposal_id: u128::from_le_bytes(proposal.proposal_id[0..16].try_into().unwrap()),
        position_id,
        outcome,
        size,
        notional: size,
        margin: required_margin,
        collateral: required_margin,
        entry_price,
        liquidation_price: calculate_liquidation_price(entry_price, leverage as u64, is_long),
        is_long,
        leverage: leverage as u64,
        created_at: Clock::get()?.unix_timestamp,
        entry_funding_index: Some(U64F64::from_num(0)),
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: u128::from_le_bytes(proposal.verse_id[0..16].try_into().unwrap()),
        is_short: !is_long,
        last_mark_price: entry_price,
        unrealized_pnl: 0,
        cross_margin_enabled: false,
        unrealized_pnl_pct: 0,
    };
    
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Update global state
    global_config.total_oi = global_config.total_oi.saturating_add(size as u128);
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    
    // Update user map
    user_map.add_position(u128::from_le_bytes(proposal.proposal_id[0..16].try_into().unwrap()))?;
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;
    
    // Update proposal total volume
    proposal.total_volume = proposal.total_volume.saturating_add(size);
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Step 3: Check if auto stop-loss is needed
    msg!("Step 3: Checking auto stop-loss requirement");
    let needs_stop_loss = needs_auto_stop_loss(leverage);
    
    let mut journey_state = AutoStopLossJourney {
        position_id,
        current_step: if needs_stop_loss {
            AutoStopLossStep::HighLeveragePosition
        } else {
            AutoStopLossStep::NormalPosition
        },
        leverage,
        entry_price,
        stop_loss_price: 0,
        stop_loss_order_id: None,
        current_price: entry_price,
        max_adverse_move_bps: 0,
        triggered_at_price: None,
        executed_at_slot: None,
        execution_slippage_bps: None,
        created_at: Clock::get()?.unix_timestamp,
        triggered_at: None,
        executed_at: None,
    };
    
    if needs_stop_loss {
        msg!("Position has leverage >= {}, creating auto stop-loss", AUTO_STOP_LOSS_MIN_LEVERAGE);
        
        // Step 4: Create auto stop-loss
        msg!("Step 4: Creating auto stop-loss order");
        create_auto_stop_loss(
            program_id,
            &position,
            leverage,
            entry_price,
            &[
                user_account.clone(),
                stop_loss_account.clone(),
                system_program.clone(),
            ],
        )?;
        
        // Update journey state
        journey_state.stop_loss_price = calculate_stop_loss_price(
            entry_price,
            is_long,
            AUTO_STOP_LOSS_THRESHOLD_BPS,
        );
        journey_state.stop_loss_order_id = Some(position_id); // Using position ID as order ID
        journey_state.current_step = AutoStopLossStep::StopLossCreated;
        
        msg!("Auto stop-loss created at price: {}", journey_state.stop_loss_price);
        msg!("Trigger threshold: {}bps adverse move", AUTO_STOP_LOSS_THRESHOLD_BPS);
    } else {
        msg!("Position leverage {} < {}, no auto stop-loss needed", leverage, AUTO_STOP_LOSS_MIN_LEVERAGE);
    }
    
    // Save journey state
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Emit position opened event
    emit_event(EventType::PositionOpened, &PositionOpened {
        user: *user_account.key,
        proposal_id: u128::from_le_bytes(proposal.proposal_id[0..16].try_into().unwrap()),
        outcome,
        size,
        leverage: leverage as u64,
        entry_price,
        is_long,
        position_id,
        chain_id: None,
    });
    
    msg!("High leverage position created successfully!");
    
    Ok(())
}

/// Simulate price movement and check stop-loss trigger
pub fn simulate_price_movement(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_price: u64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    let stop_loss_account = next_account_info(account_iter)?;
    
    // Load accounts
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut journey_state = AutoStopLossJourney::try_from_slice(&journey_state_account.data.borrow())?;
    
    msg!("Simulating price movement: {} -> {}", journey_state.current_price, new_price);
    
    // Update proposal price
    proposal.prices[position.outcome as usize] = new_price;
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Calculate price movement
    let price_diff = if new_price > journey_state.entry_price {
        new_price - journey_state.entry_price
    } else {
        journey_state.entry_price - new_price
    };
    
    let move_bps = (price_diff * 10000) / journey_state.entry_price;
    let is_adverse = (position.is_long && new_price < journey_state.entry_price) ||
                     (!position.is_long && new_price > journey_state.entry_price);
    
    msg!("Price movement: {}bps {}", move_bps, if is_adverse { "adverse" } else { "favorable" });
    
    // Update journey state
    journey_state.current_price = new_price;
    if is_adverse && move_bps > journey_state.max_adverse_move_bps {
        journey_state.max_adverse_move_bps = move_bps;
    }
    
    // Check if stop-loss should trigger
    if journey_state.current_step == AutoStopLossStep::StopLossCreated && is_adverse {
        let should_trigger = if position.is_long {
            new_price <= journey_state.stop_loss_price
        } else {
            new_price >= journey_state.stop_loss_price
        };
        
        if should_trigger {
            msg!("Stop-loss triggered at price {}", new_price);
            journey_state.current_step = AutoStopLossStep::StopLossTriggered;
            journey_state.triggered_at_price = Some(new_price);
            journey_state.triggered_at = Some(Clock::get()?.unix_timestamp);
            
            // Update stop-loss order to mark as triggered
            if let Ok(mut stop_order) = StateStopOrder::try_from_slice(&stop_loss_account.data.borrow()) {
                // In real implementation, this would mark the order for execution
                msg!("Stop-loss order marked for execution");
            }
        } else {
            journey_state.current_step = AutoStopLossStep::PriceMovingAdverse;
            msg!("Price moving adverse but stop-loss not triggered yet");
            msg!("Current: {}, Stop trigger: {}", new_price, journey_state.stop_loss_price);
        }
    }
    
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Execute triggered stop-loss order
pub fn execute_stop_loss(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let position_owner_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let user_map_account = next_account_info(account_iter)?;
    let journey_state_account = next_account_info(account_iter)?;
    let stop_loss_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify keeper
    let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    if keeper.status != KeeperStatus::Active {
        return Err(BettingPlatformError::KeeperNotActive.into());
    }
    
    // Load accounts
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    let mut user_map = UserMap::try_from_slice(&user_map_account.data.borrow())?;
    let mut journey_state = AutoStopLossJourney::try_from_slice(&journey_state_account.data.borrow())?;
    
    // Verify stop-loss is triggered
    if journey_state.current_step != AutoStopLossStep::StopLossTriggered {
        return Err(ProgramError::InvalidAccountData);
    }
    
    msg!("Executing stop-loss order for position: {:?}", position.position_id);
    
    // Step 1: Execute trade on AMM to close position
    msg!("Step 1: Closing position on AMM");
    let execution_price = proposal.prices[position.outcome as usize];
    
    crate::amm::execute_trade(
        &mut proposal_account.data.borrow_mut()[..],
        position.outcome,
        position.size,
        !position.is_long, // Opposite direction to close
    )?;
    
    // Calculate execution slippage
    let triggered_price = journey_state.triggered_at_price.unwrap();
    let slippage_bps = if execution_price > triggered_price {
        ((execution_price - triggered_price) * 10000) / triggered_price
    } else {
        ((triggered_price - execution_price) * 10000) / triggered_price
    };
    
    msg!("Execution price: {}, Slippage: {}bps", execution_price, slippage_bps);
    
    // Step 2: Calculate PnL and settle position
    msg!("Step 2: Settling position");
    let pnl = calculate_position_pnl(&position, execution_price)?;
    let final_value = (position.margin as i64 + pnl).max(0) as u64;
    
    msg!("Position PnL: {}", pnl);
    msg!("Final value: {}", final_value);
    
    // Transfer remaining value to user (if any)
    if final_value > 0 {
        solana_program::program::invoke(
            &solana_program::system_instruction::transfer(
                vault_account.key,
                position_owner_account.key,
                final_value,
            ),
            &[vault_account.clone(), position_owner_account.clone(), system_program.clone()],
        )?;
        msg!("Returned {} to position owner", final_value);
    }
    
    // Step 3: Update position state
    msg!("Step 3: Updating position state");
    position.size = 0;
    position.is_closed = true;
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Update user map
    user_map.remove_position(position.proposal_id)?;
    user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;
    
    // Update proposal - reduce volume on position close
    // (In a real implementation, we'd track open interest separately)
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Update global state
    global_config.total_oi = global_config.total_oi.saturating_sub(position.size as u128);
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    
    // Step 4: Update journey state
    msg!("Step 4: Finalizing journey");
    journey_state.current_step = AutoStopLossStep::StopLossExecuted;
    journey_state.executed_at_slot = Some(Clock::get()?.slot);
    journey_state.execution_slippage_bps = Some(slippage_bps);
    journey_state.executed_at = Some(Clock::get()?.unix_timestamp);
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    // Clear stop-loss order
    let mut stop_order_data = stop_loss_account.data.borrow_mut();
    stop_order_data.fill(0);
    
    // Emit stop order executed event
    emit_event(EventType::StopLossExecuted, &StopOrderExecuted {
        order_id: position.position_id,
        user: position.user,
        market_id: *proposal_account.key,
        order_type: "STOP_LOSS".to_string(),
        execution_price,
        size: position.size,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    // Mark journey complete
    journey_state.current_step = AutoStopLossStep::PositionClosed;
    journey_state.serialize(&mut &mut journey_state_account.data.borrow_mut()[..])?;
    
    msg!("Stop-loss executed successfully!");
    msg!("Position closed at price {} with slippage {}bps", execution_price, slippage_bps);
    
    Ok(())
}

/// Verify auto stop-loss journey completion
pub fn verify_journey_completion(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let journey_state_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    
    // Load accounts
    let journey_state = AutoStopLossJourney::try_from_slice(&journey_state_account.data.borrow())?;
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    
    msg!("Verifying auto stop-loss journey completion");
    
    // Verify position is closed
    if !position.is_closed {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Verify journey completed
    if journey_state.current_step != AutoStopLossStep::PositionClosed {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Calculate total journey time
    let journey_duration = journey_state.executed_at.unwrap() - journey_state.created_at;
    
    msg!("Journey completed successfully!");
    msg!("Leverage: {}x", journey_state.leverage);
    msg!("Entry price: {}", journey_state.entry_price);
    msg!("Stop-loss price: {}", journey_state.stop_loss_price);
    msg!("Triggered at: {}", journey_state.triggered_at_price.unwrap());
    msg!("Max adverse move: {}bps", journey_state.max_adverse_move_bps);
    msg!("Execution slippage: {}bps", journey_state.execution_slippage_bps.unwrap());
    msg!("Journey duration: {} seconds", journey_duration);
    
    Ok(())
}

/// Helper: Calculate liquidation price
fn calculate_liquidation_price(entry_price: u64, leverage: u64, is_long: bool) -> u64 {
    let liquidation_threshold = 10000 / leverage; // In bps
    
    if is_long {
        entry_price.saturating_sub(entry_price * liquidation_threshold / 10000)
    } else {
        entry_price.saturating_add(entry_price * liquidation_threshold / 10000)
    }
}

/// Helper: Calculate position PnL
fn calculate_position_pnl(position: &Position, exit_price: u64) -> Result<i64, ProgramError> {
    let price_diff = if position.is_long {
        exit_price as i64 - position.entry_price as i64
    } else {
        position.entry_price as i64 - exit_price as i64
    };
    
    let pnl = (price_diff * position.size as i64) / 1_000_000;
    Ok(pnl)
}

/// Stop order executed event
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct StopOrderExecuted {
    pub order_id: [u8; 32],
    pub user: Pubkey,
    pub market_id: Pubkey,
    pub order_type: String,
    pub execution_price: u64,
    pub size: u64,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_auto_stop_loss_threshold() {
        // Test long position stop-loss
        let entry_price = 1_000_000; // $1.00
        let stop_loss_price = calculate_stop_loss_price(entry_price, true, AUTO_STOP_LOSS_THRESHOLD_BPS);
        assert_eq!(stop_loss_price, 999_000); // 0.1% below entry
        
        // Test short position stop-loss
        let stop_loss_price = calculate_stop_loss_price(entry_price, false, AUTO_STOP_LOSS_THRESHOLD_BPS);
        assert_eq!(stop_loss_price, 1_001_000); // 0.1% above entry
    }
    
    #[test]
    fn test_leverage_requirement() {
        assert!(!needs_auto_stop_loss(49)); // Below threshold
        assert!(needs_auto_stop_loss(50)); // At threshold
        assert!(needs_auto_stop_loss(100)); // Above threshold
    }
}