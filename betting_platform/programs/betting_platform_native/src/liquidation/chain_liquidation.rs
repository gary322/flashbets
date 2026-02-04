//! Chain position liquidation with proper unwinding order
//!
//! Implements liquidation of chained positions in the correct order:
//! stake → liquidate → borrow

use solana_program::{
    account_info::AccountInfo,
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
    events::Event,
    keeper_liquidation::{LiquidationKeeper, KEEPER_REWARD_BPS},
    math::U64F64,
    state::{Position, accounts::discriminators},
    state::chain_accounts::{ChainState, ChainPosition, PositionStatus as ChainPositionStatus, ChainStatus},
};

/// Chain liquidation types based on the specification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChainStepType {
    Stake,      // Stake position (must unwind first)
    Liquidate,  // Liquidation position (unwind second)
    Borrow,     // Borrow position (unwind last)
}

/// Chain liquidation processor
pub struct ChainLiquidationProcessor;

impl ChainLiquidationProcessor {
    /// Liquidate a chain of positions with proper unwinding order
    pub fn liquidate_chain(
        chain_state: &mut ChainState,
        chain_positions: &mut [ChainPosition],
        keeper_account: &AccountInfo,
        current_prices: &[(u128, u64)], // (proposal_id, price)
    ) -> Result<ChainLiquidationResult, ProgramError> {
        // Validate chain can be liquidated
        Self::validate_chain_liquidation(chain_state, chain_positions, current_prices)?;
        
        // Sort positions by liquidation priority (stake → liq → borrow)
        let sorted_positions = Self::sort_by_unwind_order(chain_positions);
        
        let mut total_liquidated = 0u64;
        let mut keeper_rewards = 0u64;
        let mut positions_liquidated = 0u32;
        
        // Process liquidations in correct order
        for position in sorted_positions.iter_mut() {
            if position.status != ChainPositionStatus::Open {
                continue;
            }
            
            // Find current price for this position
            let current_price = current_prices
                .iter()
                .find(|(id, _)| *id == position.proposal_id)
                .map(|(_, price)| *price)
                .ok_or(BettingPlatformError::PriceNotFound)?;
            
            // Calculate liquidation amount
            let liquidation_amount = Self::calculate_chain_position_liquidation(
                position,
                current_price,
                chain_state.current_balance,
            )?;
            
            if liquidation_amount > 0 {
                // Update position
                position.size = position.size.saturating_sub(liquidation_amount);
                if position.size == 0 {
                    position.status = ChainPositionStatus::Liquidated;
                    position.closed_at = Some(Clock::get()?.unix_timestamp);
                }
                
                // Calculate keeper reward
                let reward = (liquidation_amount as u128 * KEEPER_REWARD_BPS as u128 / 10000) as u64;
                keeper_rewards += reward;
                
                total_liquidated += liquidation_amount;
                positions_liquidated += 1;
                
                msg!(
                    "Liquidated chain position {} (type: {:?}), amount: {}",
                    position.position_id,
                    Self::get_position_type(position.step_index),
                    liquidation_amount
                );
            }
        }
        
        // Update chain state
        chain_state.current_balance = chain_state.current_balance
            .saturating_sub(total_liquidated)
            .saturating_sub(keeper_rewards);
        
        if chain_state.current_balance == 0 {
            chain_state.status = ChainStatus::Liquidated;
        }
        
        // Emit event
        ChainLiquidationExecuted {
            chain_id: chain_state.chain_id,
            keeper: *keeper_account.key,
            total_liquidated,
            keeper_rewards,
            positions_liquidated,
            slot: Clock::get()?.slot,
        }.emit();
        
        Ok(ChainLiquidationResult {
            total_liquidated,
            keeper_rewards,
            positions_liquidated,
            chain_terminated: chain_state.status == ChainStatus::Liquidated,
        })
    }
    
    /// Validate chain can be liquidated
    fn validate_chain_liquidation(
        chain_state: &ChainState,
        positions: &[ChainPosition],
        current_prices: &[(u128, u64)],
    ) -> Result<(), ProgramError> {
        // Chain must be active
        if chain_state.status != ChainStatus::Active {
            return Err(BettingPlatformError::ChainNotActive.into());
        }
        
        // Must have open positions
        let has_open_positions = positions.iter().any(|p| p.status == ChainPositionStatus::Open);
        if !has_open_positions {
            return Err(BettingPlatformError::NoOpenPositions.into());
        }
        
        // Check if any position is liquidatable
        let mut is_liquidatable = false;
        for position in positions {
            if position.status != ChainPositionStatus::Open {
                continue;
            }
            
            if let Some((_, price)) = current_prices.iter().find(|(id, _)| *id == position.proposal_id) {
                if Self::is_position_liquidatable(position, *price)? {
                    is_liquidatable = true;
                    break;
                }
            }
        }
        
        if !is_liquidatable {
            return Err(BettingPlatformError::NoLiquidatablePositions.into());
        }
        
        Ok(())
    }
    
    /// Sort positions by unwinding order: stake → liquidate → borrow
    fn sort_by_unwind_order(positions: &mut [ChainPosition]) -> &mut [ChainPosition] {
        positions.sort_by_key(|p| match Self::get_position_type(p.step_index) {
            ChainStepType::Stake => 0,      // First priority
            ChainStepType::Liquidate => 1,  // Second priority
            ChainStepType::Borrow => 2,      // Last priority
        });
        positions
    }
    
    /// Determine position type based on step index
    fn get_position_type(step_index: u8) -> ChainStepType {
        // In a real implementation, this would map step_index to the actual step type
        // For now, we'll use a simple modulo mapping
        match step_index % 3 {
            0 => ChainStepType::Stake,
            1 => ChainStepType::Liquidate,
            _ => ChainStepType::Borrow,
        }
    }
    
    /// Check if position should be liquidated
    fn is_position_liquidatable(position: &ChainPosition, current_price: u64) -> Result<bool, ProgramError> {
        // Use the standard liquidation check
        use crate::trading::helpers::should_liquidate;
        
        // Create a temporary Position for the check
        let temp_position = Position {
            discriminator: discriminators::POSITION,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user: Pubkey::default(),
            proposal_id: position.proposal_id,
            position_id: [0u8; 32],
            outcome: position.outcome,
            size: position.size,
            notional: position.size,
            leverage: position.leverage,
            entry_price: position.entry_price,
            liquidation_price: Self::calculate_liquidation_price(
                position.entry_price,
                position.leverage,
                position.is_long,
            )?,
            is_long: position.is_long,
            created_at: position.created_at,
            entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 0, // Chain position doesn't have verse_id  
            margin: position.size / position.leverage,
            collateral: 0,
            is_short: !position.is_long,
            last_mark_price: current_price,
            unrealized_pnl: 0, // Will be calculated
            unrealized_pnl_pct: 0, // Will be calculated
            cross_margin_enabled: false, // Chain positions use isolated margin
        };
        
        Ok(should_liquidate(&temp_position, current_price))
    }
    
    /// Calculate liquidation price for chain position
    fn calculate_liquidation_price(
        entry_price: u64,
        leverage: u64,
        is_long: bool,
    ) -> Result<u64, ProgramError> {
        use crate::trading::helpers::calculate_liquidation_price;
        calculate_liquidation_price(entry_price, leverage, is_long)
    }
    
    /// Calculate liquidation amount for chain position
    fn calculate_chain_position_liquidation(
        position: &ChainPosition,
        current_price: u64,
        available_balance: u64,
    ) -> Result<u64, ProgramError> {
        if !Self::is_position_liquidatable(position, current_price)? {
            return Ok(0);
        }
        
        // Calculate partial liquidation amount (50% of position)
        let partial_amount = position.size / 2;
        
        // Ensure we don't liquidate more than available balance
        let max_liquidation = available_balance.min(position.size);
        
        Ok(partial_amount.min(max_liquidation))
    }
}

/// Chain liquidation result
#[derive(Debug)]
pub struct ChainLiquidationResult {
    pub total_liquidated: u64,
    pub keeper_rewards: u64,
    pub positions_liquidated: u32,
    pub chain_terminated: bool,
}
/// Chain liquidation event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ChainLiquidationExecuted {
    pub chain_id: u128,
    pub keeper: Pubkey,
    pub total_liquidated: u64,
    pub keeper_rewards: u64,
    pub positions_liquidated: u32,
    pub slot: u64,
}

impl Event for ChainLiquidationExecuted {
    fn event_type() -> crate::events::EventType {
        crate::events::EventType::LiquidationExecuted
    }
    
    fn emit(&self) {
        msg!("BETTING_PLATFORM_EVENT");
        msg!("TYPE:{:?}", Self::event_type());
        
        // Serialize and log event data
        if let Ok(data) = self.try_to_vec() {
            msg!("DATA:{}", bs58::encode(&data).into_string());
        }
        
        // Also log human-readable format
        msg!(
            "ChainLiquidationExecuted: chain_id={}, keeper={}, liquidated={}, rewards={}, positions={}",
            self.chain_id,
            self.keeper,
            self.total_liquidated,
            self.keeper_rewards,
            self.positions_liquidated
        );
    }
}