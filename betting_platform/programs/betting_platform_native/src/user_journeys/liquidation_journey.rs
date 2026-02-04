//! Liquidation User Journey
//! 
//! Complete flow for position liquidation and keeper interactions

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
    liquidation::{
        calculate_liquidation_amount,
        calculate_keeper_reward,
        should_liquidate_coverage_based,
    },
    keeper_network::{KeeperAccount, KeeperStatus},
    events::{emit_event, EventType, PositionLiquidated, PositionOpened},
    math::U64F64,
};

/// Liquidation journey state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LiquidationUserJourney {
    /// Position being monitored
    pub position_id: [u8; 32],
    
    /// Current step
    pub current_step: LiquidationStep,
    
    /// Risk metrics
    pub margin_ratio: u64,
    pub coverage_ratio: u64,
    
    /// Liquidation details
    pub liquidation_type: Option<LiquidationType>,
    pub liquidation_amount: Option<u64>,
    pub keeper_reward: Option<u64>,
    
    /// Timestamps
    pub at_risk_timestamp: Option<i64>,
    pub liquidation_timestamp: Option<i64>,
}

/// Liquidation journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum LiquidationStep {
    /// Position healthy
    Healthy,
    
    /// Position at risk
    AtRisk,
    
    /// Liquidation triggered
    LiquidationTriggered,
    
    /// Keeper assigned
    KeeperAssigned,
    
    /// Partial liquidation executed
    PartialLiquidationExecuted,
    
    /// Full liquidation executed
    FullLiquidationExecuted,
    
    /// Position saved
    PositionSaved,
}

/// Liquidation types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum LiquidationType {
    /// Partial liquidation to restore health
    Partial(u16), // Percentage in basis points
    
    /// Full liquidation
    Full,
    
    /// Emergency liquidation (cascade risk)
    Emergency,
}

/// Monitor position health and trigger liquidation if needed
pub fn monitor_position_health(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let liquidation_state_account = next_account_info(account_iter)?;
    
    // Verify keeper is authorized
    let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    if keeper.status != KeeperStatus::Active {
        return Err(BettingPlatformError::KeeperNotActive.into());
    }
    
    msg!("Monitoring position health: {:?}", position_id);
    
    // Load accounts
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Step 1: Calculate current margin ratio
    msg!("Step 1: Calculating margin ratio");
    let current_price = proposal.prices[position.outcome as usize];
    let margin_ratio = calculate_margin_ratio(&position, current_price)?;
    msg!("Current margin ratio: {}%", margin_ratio / 100);
    
    // Step 2: Check coverage-based liquidation criteria
    msg!("Step 2: Checking coverage-based liquidation");
    let coverage_ratio = (global_config.vault * 10000) / global_config.total_oi;
    msg!("Platform coverage ratio: {}bps", coverage_ratio);
    
    let should_liquidate = should_liquidate_coverage_based(
        &position,
        current_price,
        U64F64::from_num(coverage_ratio as u64) / U64F64::from_num(10000), // Convert bps to ratio
    )?;
    
    // Step 3: Determine liquidation type if needed
    if should_liquidate {
        msg!("Step 3: Position requires liquidation");
        
        let liquidation_type = determine_liquidation_type(
            margin_ratio,
            coverage_ratio as u16,
            &position,
        )?;
        
        // Step 4: Calculate liquidation parameters
        msg!("Step 4: Calculating liquidation parameters");
        let (liquidation_amount, remaining_position) = match liquidation_type {
            LiquidationType::Partial(percentage) => {
                let amount = (position.size * percentage as u64) / 10000;
                (amount, position.size - amount)
            }
            LiquidationType::Full => (position.size, 0),
            LiquidationType::Emergency => (position.size, 0),
        };
        
        msg!("Liquidation type: {:?}", liquidation_type);
        msg!("Liquidation amount: {}", liquidation_amount);
        msg!("Remaining position: {}", remaining_position);
        
        // Step 5: Calculate keeper reward
        let base_reward_bps = 50; // 0.5% base reward
        let keeper_reward = calculate_keeper_reward(
            liquidation_amount,
            base_reward_bps,
        )?;
        msg!("Keeper reward: {}", keeper_reward);
        
        // Save liquidation state
        let liquidation_state = LiquidationUserJourney {
            position_id,
            current_step: LiquidationStep::LiquidationTriggered,
            margin_ratio,
            coverage_ratio: coverage_ratio as u64,
            liquidation_type: Some(liquidation_type),
            liquidation_amount: Some(liquidation_amount),
            keeper_reward: Some(keeper_reward),
            at_risk_timestamp: Some(Clock::get()?.unix_timestamp),
            liquidation_timestamp: None,
        };
        
        liquidation_state.serialize(&mut &mut liquidation_state_account.data.borrow_mut()[..])?;
        
        // Emit liquidation event
        emit_event(EventType::PositionLiquidated, &PositionLiquidated {
            position_id,
            liquidator: *keeper_account.key,
            liquidation_price: position.entry_price, // Using entry price as liquidation price
            amount_liquidated: position.size,
            remaining_position: 0, // Full liquidation
        });
        
        Ok(())
    } else {
        msg!("Position is healthy - no liquidation needed");
        msg!("Margin ratio: {}%", margin_ratio / 100);
        msg!("Minimum required: {}%", calculate_min_margin_ratio(position.leverage) / 100);
        
        Ok(())
    }
}

/// Execute liquidation
pub fn execute_liquidation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let position_owner_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let liquidation_state_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let keeper_rewards_account = next_account_info(account_iter)?;
    let user_map_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Verify keeper
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    if keeper.status != KeeperStatus::Active {
        return Err(BettingPlatformError::KeeperNotActive.into());
    }
    
    // Load liquidation state
    let mut liquidation_state = LiquidationUserJourney::try_from_slice(
        &liquidation_state_account.data.borrow()
    )?;
    
    if liquidation_state.current_step != LiquidationStep::LiquidationTriggered {
        return Err(ProgramError::InvalidAccountData);
    }
    
    msg!("Executing liquidation for position: {:?}", position_id);
    
    // Load accounts
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Verify position matches
    if position.position_id != position_id {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let liquidation_type = liquidation_state.liquidation_type.unwrap();
    let liquidation_amount = liquidation_state.liquidation_amount.unwrap();
    let keeper_reward = liquidation_state.keeper_reward.unwrap();
    
    // Step 1: Update keeper state
    msg!("Step 1: Assigning keeper");
    keeper.total_operations += 1;
    keeper.successful_operations += 1;
    keeper.total_rewards_earned += keeper_reward;
    keeper.last_operation_slot = Clock::get()?.slot;
    keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    
    liquidation_state.current_step = LiquidationStep::KeeperAssigned;
    
    // Step 2: Execute liquidation on AMM
    msg!("Step 2: Executing liquidation on AMM");
    let liquidation_price = proposal.prices[position.outcome as usize];
    
    // Close liquidated portion
    crate::amm::execute_trade(
        &mut proposal_account.data.borrow_mut()[..],
        position.outcome,
        liquidation_amount,
        !position.is_long, // Opposite direction to close
    )?;
    
    // Step 3: Update position
    msg!("Step 3: Updating position");
    match liquidation_type {
        LiquidationType::Partial(percentage) => {
            // Reduce position size
            position.size = position.size.saturating_sub(liquidation_amount);
            position.notional = position.notional.saturating_sub(liquidation_amount);
            position.margin = (position.margin * (10000 - percentage as u64)) / 10000;
            position.partial_liq_accumulator += liquidation_amount;
            
            msg!("Partial liquidation completed");
            msg!("Remaining position size: {}", position.size);
            msg!("Remaining margin: {}", position.margin);
            
            liquidation_state.current_step = LiquidationStep::PartialLiquidationExecuted;
        }
        LiquidationType::Full | LiquidationType::Emergency => {
            // Close entire position
            position.size = 0;
            position.is_closed = true;
            
            msg!("Full liquidation completed");
            
            liquidation_state.current_step = LiquidationStep::FullLiquidationExecuted;
            
            // Update user map
            let mut user_map = UserMap::try_from_slice(&user_map_account.data.borrow())?;
            user_map.remove_position(position.proposal_id)?;
            user_map.serialize(&mut &mut user_map_account.data.borrow_mut()[..])?;
        }
    }
    
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Step 4: Process liquidation proceeds
    msg!("Step 4: Processing liquidation proceeds");
    let liquidation_value = (liquidation_amount * liquidation_price) / 1_000_000;
    let proceeds_after_keeper = liquidation_value.saturating_sub(keeper_reward);
    
    // Transfer keeper reward
    solana_program::program::invoke(
        &solana_program::system_instruction::transfer(
            vault_account.key,
            keeper_rewards_account.key,
            keeper_reward,
        ),
        &[vault_account.clone(), keeper_rewards_account.clone(), system_program.clone()],
    )?;
    
    // Return remaining proceeds to position owner (if any)
    if proceeds_after_keeper > 0 && position.margin > 0 {
        let owner_payout = proceeds_after_keeper.min(position.margin);
        
        solana_program::program::invoke(
            &solana_program::system_instruction::transfer(
                vault_account.key,
                position_owner_account.key,
                owner_payout,
            ),
            &[vault_account.clone(), position_owner_account.clone(), system_program.clone()],
        )?;
        
        msg!("Returned {} to position owner", owner_payout);
    }
    
    // Step 5: Update global state
    msg!("Step 5: Updating global state");
    global_config.total_oi = global_config.total_oi.saturating_sub(liquidation_amount as u128);
    global_config.serialize(&mut &mut global_config_account.data.borrow_mut()[..])?;
    
    // Save proposal
    proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
    
    // Update liquidation state
    liquidation_state.liquidation_timestamp = Some(Clock::get()?.unix_timestamp);
    liquidation_state.serialize(&mut &mut liquidation_state_account.data.borrow_mut()[..])?;
    
    // Emit liquidation executed event
    emit_event(EventType::PositionLiquidated, &PositionLiquidated {
        position_id,
        liquidator: *keeper_account.key,
        liquidation_price,
        amount_liquidated: liquidation_amount,
        remaining_position: position.size,
    });
    
    msg!("Liquidation executed successfully!");
    
    Ok(())
}

/// Post-liquidation recovery check
pub fn check_position_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let liquidation_state_account = next_account_info(account_iter)?;
    
    // Load accounts
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut liquidation_state = LiquidationUserJourney::try_from_slice(
        &liquidation_state_account.data.borrow()
    )?;
    
    // Only check partially liquidated positions
    if liquidation_state.current_step != LiquidationStep::PartialLiquidationExecuted {
        return Ok(());
    }
    
    // Calculate current health
    let current_price = proposal.prices[position.outcome as usize];
    let margin_ratio = calculate_margin_ratio(&position, current_price)?;
    let min_healthy_ratio = calculate_min_margin_ratio(position.leverage) * 2; // 2x safety
    
    msg!("Checking position recovery");
    msg!("Current margin ratio: {}%", margin_ratio / 100);
    msg!("Minimum healthy ratio: {}%", min_healthy_ratio / 100);
    
    if margin_ratio >= min_healthy_ratio {
        msg!("Position has recovered to healthy state!");
        liquidation_state.current_step = LiquidationStep::PositionSaved;
        liquidation_state.serialize(&mut &mut liquidation_state_account.data.borrow_mut()[..])?;
        
        // Use PositionOpened event to indicate position is healthy/recovered
        emit_event(EventType::PositionOpened, &PositionOpened {
            user: position.user,
            proposal_id: position.proposal_id,
            outcome: position.outcome,
            size: position.size,
            leverage: position.leverage,
            entry_price: position.entry_price,
            is_long: position.is_long,
            position_id,
            chain_id: None,
        });
    } else {
        msg!("Position still at risk - margin ratio below healthy threshold");
    }
    
    Ok(())
}

/// Calculate margin ratio
fn calculate_margin_ratio(position: &Position, current_price: u64) -> Result<u64, ProgramError> {
    // Calculate current value
    let current_value = if position.is_long {
        (position.size * current_price) / 1_000_000
    } else {
        let price_diff = position.entry_price.saturating_sub(current_price);
        position.notional + (position.size * price_diff) / 1_000_000
    };
    
    // Margin ratio = (current_value - debt) / debt * 10000
    let debt = position.notional.saturating_sub(position.margin);
    if debt == 0 {
        return Ok(10000); // 100%
    }
    
    let equity = current_value.saturating_sub(debt);
    Ok((equity * 10000) / debt)
}

/// Calculate minimum margin ratio for leverage
fn calculate_min_margin_ratio(leverage: u64) -> u64 {
    // Min margin = 1 / leverage * 10000
    10000 / leverage
}

/// Determine liquidation type based on risk metrics
fn determine_liquidation_type(
    margin_ratio: u64,
    coverage_ratio: u16,
    position: &Position,
) -> Result<LiquidationType, ProgramError> {
    // Emergency liquidation if margin < 5%
    if margin_ratio < 500 {
        return Ok(LiquidationType::Emergency);
    }
    
    // Full liquidation if coverage < 50% and margin < 10%
    if coverage_ratio < 5000 && margin_ratio < 1000 {
        return Ok(LiquidationType::Full);
    }
    
    // Partial liquidation to restore health
    // Target: bring margin ratio back to 20%
    let target_margin = 2000; // 20%
    let current_margin = margin_ratio;
    
    if current_margin >= target_margin {
        return Ok(LiquidationType::Partial(0));
    }
    
    // Calculate percentage to liquidate
    let liquidation_percentage = ((target_margin - current_margin) * 10000) / target_margin;
    let capped_percentage = liquidation_percentage.min(5000); // Max 50% partial
    
    Ok(LiquidationType::Partial(capped_percentage as u16))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_margin_ratio_calculation() {
        let position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            size: 10000,
            notional: 10000,
            margin: 1000,
            collateral: 0,
            entry_price: 500_000,
            is_long: true,
            leverage: 10,
            // ... other fields
            user: Pubkey::default(),
            proposal_id: 0,
            position_id: [0; 32],
            outcome: 0,
            liquidation_price: 450_000,
            created_at: 0,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 0,
            is_short: false,
            last_mark_price: 500_000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        // Price at entry - should have 10% margin
        let margin_ratio = calculate_margin_ratio(&position, 500_000).unwrap();
        assert_eq!(margin_ratio, 1111); // ~11.11%
        
        // Price drops to liquidation threshold
        let margin_ratio = calculate_margin_ratio(&position, 450_000).unwrap();
        assert!(margin_ratio < 500); // Less than 5%
    }
    
    #[test]
    fn test_liquidation_type_determination() {
        let position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            size: 10000,
            leverage: 10,
            // ... other fields
            user: Pubkey::default(),
            proposal_id: 0,
            position_id: [0; 32],
            outcome: 0,
            notional: 10000,
            margin: 1000,
            collateral: 0,
            entry_price: 500_000,
            liquidation_price: 450_000,
            is_long: true,
            created_at: 0,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 0,
            is_short: false,
            last_mark_price: 500_000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        // Emergency liquidation
        let liq_type = determine_liquidation_type(400, 5000, &position).unwrap();
        assert!(matches!(liq_type, LiquidationType::Emergency));
        
        // Full liquidation
        let liq_type = determine_liquidation_type(800, 4000, &position).unwrap();
        assert!(matches!(liq_type, LiquidationType::Full));
        
        // Partial liquidation
        let liq_type = determine_liquidation_type(1500, 6000, &position).unwrap();
        match liq_type {
            LiquidationType::Partial(pct) => assert!(pct > 0 && pct <= 5000),
            _ => panic!("Expected partial liquidation"),
        }
    }
}