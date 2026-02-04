//! Graduated Liquidation System
//!
//! Implements partial liquidations at 10%, 25%, 50%, and 100% thresholds
//! to minimize market impact and protect users

use solana_program::{
    account_info::{next_account_info, AccountInfo},
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
    math::U64F64,
    state::{Position, ProposalPDA},
    events::{Event, PartialLiquidationExecuted},
};

/// Liquidation thresholds and percentages
pub const LIQUIDATION_LEVELS: [(u16, u16); 4] = [
    (9500, 1000),  // At 95% of liquidation price, liquidate 10%
    (9750, 2500),  // At 97.5% of liquidation price, liquidate 25%
    (9900, 5000),  // At 99% of liquidation price, liquidate 50%
    (10000, 10000), // At 100% of liquidation price, liquidate 100%
];

/// Grace period slots before next liquidation level
pub const LIQUIDATION_GRACE_PERIOD: u64 = 10;

/// Graduated liquidation state for a position
#[derive(Debug, Clone, Copy)]
pub struct GraduatedLiquidationState {
    /// Current liquidation level (0-3)
    pub current_level: u8,
    
    /// Total percentage liquidated (basis points)
    pub total_liquidated_bps: u16,
    
    /// Last liquidation slot
    pub last_liquidation_slot: u64,
    
    /// Grace period active
    pub in_grace_period: bool,
}

impl GraduatedLiquidationState {
    /// Create new liquidation state
    pub fn new() -> Self {
        Self {
            current_level: 0,
            total_liquidated_bps: 0,
            last_liquidation_slot: 0,
            in_grace_period: false,
        }
    }
    
    /// Check if position needs liquidation and determine amount
    pub fn check_liquidation_needed(
        &self,
        position: &Position,
        current_price: u64,
        current_slot: u64,
    ) -> Result<LiquidationDecision, ProgramError> {
        // Skip if position already closed
        if position.is_closed {
            return Ok(LiquidationDecision::None);
        }
        
        // Check if in grace period
        if self.in_grace_period && 
           current_slot < self.last_liquidation_slot + LIQUIDATION_GRACE_PERIOD {
            return Ok(LiquidationDecision::GracePeriod);
        }
        
        // Calculate position health
        let health_ratio = calculate_health_ratio(position, current_price)?;
        
        // Find appropriate liquidation level
        for (level_idx, &(threshold_bps, liquidate_bps)) in LIQUIDATION_LEVELS.iter().enumerate() {
            // Skip levels we've already processed
            if level_idx < self.current_level as usize {
                continue;
            }
            
            // Check if we've crossed this threshold
            if health_ratio <= threshold_bps {
                // Calculate amount to liquidate
                let remaining_position = position.size
                    .saturating_sub(position.partial_liq_accumulator);
                
                if remaining_position == 0 {
                    return Ok(LiquidationDecision::None);
                }
                
                // Determine liquidation amount based on level
                let base_liquidation = (remaining_position as u128 * liquidate_bps as u128 / 10000) as u64;
                
                // For the final level, liquidate everything
                let liquidation_amount = if level_idx == LIQUIDATION_LEVELS.len() - 1 {
                    remaining_position
                } else {
                    base_liquidation.min(remaining_position)
                };
                
                return Ok(LiquidationDecision::Liquidate {
                    level: level_idx as u8,
                    amount: liquidation_amount,
                    health_ratio,
                    is_full: liquidation_amount == remaining_position,
                });
            }
        }
        
        Ok(LiquidationDecision::None)
    }
    
    /// Update state after liquidation
    pub fn update_after_liquidation(
        &mut self,
        level: u8,
        amount_liquidated: u64,
        position_size: u64,
        current_slot: u64,
    ) {
        self.current_level = level;
        self.total_liquidated_bps = ((amount_liquidated as u128 * 10000 / position_size as u128) as u16)
            .min(10000);
        self.last_liquidation_slot = current_slot;
        self.in_grace_period = level < LIQUIDATION_LEVELS.len() as u8 - 1;
    }
}

/// Calculate position health ratio in basis points
fn calculate_health_ratio(position: &Position, current_price: u64) -> Result<u16, ProgramError> {
    if position.liquidation_price == 0 {
        return Ok(10000); // 100% healthy
    }
    
    let health = if position.is_long {
        // Long position: health decreases as price drops
        if current_price >= position.entry_price {
            10000 // 100% healthy
        } else if current_price <= position.liquidation_price {
            0 // 0% healthy
        } else {
            // Linear interpolation
            let price_range = position.entry_price - position.liquidation_price;
            let current_range = current_price - position.liquidation_price;
            ((current_range as u128 * 10000 / price_range as u128) as u16).min(10000)
        }
    } else {
        // Short position: health decreases as price rises
        if current_price <= position.entry_price {
            10000 // 100% healthy
        } else if current_price >= position.liquidation_price {
            0 // 0% healthy
        } else {
            // Linear interpolation
            let price_range = position.liquidation_price - position.entry_price;
            let current_range = position.liquidation_price - current_price;
            ((current_range as u128 * 10000 / price_range as u128) as u16).min(10000)
        }
    };
    
    Ok(health)
}

/// Liquidation decision
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidationDecision {
    /// No liquidation needed
    None,
    
    /// In grace period, wait
    GracePeriod,
    
    /// Liquidate with details
    Liquidate {
        level: u8,
        amount: u64,
        health_ratio: u16,
        is_full: bool,
    },
}

/// Process graduated liquidation
pub fn process_graduated_liquidation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    msg!("Processing graduated liquidation for position");
    
    let account_info_iter = &mut accounts.iter();
    let liquidator = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    
    // Load position
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    position.validate()?;
    
    // Load proposal for current price
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    proposal.validate()?;
    
    // Get current price
    let current_price = proposal.prices[position.outcome as usize];
    
    // Initialize or load liquidation state
    let mut liq_state = if position.partial_liq_accumulator > 0 {
        // Reconstruct from position data
        let total_liquidated = position.partial_liq_accumulator;
        let total_bps = (total_liquidated as u128 * 10000 / position.size as u128) as u16;
        
        let mut level = 0u8;
        for (idx, &(_, level_bps)) in LIQUIDATION_LEVELS.iter().enumerate() {
            if total_bps >= level_bps {
                level = idx as u8;
            }
        }
        
        GraduatedLiquidationState {
            current_level: level,
            total_liquidated_bps: total_bps,
            last_liquidation_slot: 0, // Will check grace period
            in_grace_period: false,
        }
    } else {
        GraduatedLiquidationState::new()
    };
    
    // Check liquidation decision
    let clock = Clock::get()?;
    let decision = liq_state.check_liquidation_needed(
        &position,
        current_price,
        clock.slot,
    )?;
    
    match decision {
        LiquidationDecision::None => {
            msg!("Position healthy, no liquidation needed");
            return Err(BettingPlatformError::PositionHealthy.into());
        }
        LiquidationDecision::GracePeriod => {
            msg!("Position in grace period, waiting");
            return Err(BettingPlatformError::InGracePeriod.into());
        }
        LiquidationDecision::Liquidate { level, amount, health_ratio, is_full } => {
            msg!("Liquidating {} at level {} (health: {}%)", 
                amount, level, health_ratio / 100);
            
            // Calculate liquidation value and fees
            let liquidation_value = (amount as u128 * current_price as u128 / 1_000_000) as u64;
            let liquidation_fee = liquidation_value / 100; // 1% fee
            let keeper_reward = liquidation_fee / 2; // 50% to keeper
            let insurance_fund = liquidation_fee - keeper_reward; // 50% to insurance
            
            // Update position
            position.partial_liq_accumulator = position.partial_liq_accumulator
                .saturating_add(amount);
            position.size = position.size.saturating_sub(amount);
            
            if is_full || position.size == 0 {
                position.is_closed = true;
            }
            
            // Update liquidation state
            liq_state.update_after_liquidation(
                level,
                amount,
                position.size + amount,
                clock.slot,
            );
            
            // Transfer funds
            // User receives liquidation value minus fees
            **user_account.lamports.borrow_mut() = user_account
                .lamports()
                .checked_add(liquidation_value - liquidation_fee)
                .ok_or(ProgramError::InvalidAccountData)?;
            
            // Keeper receives reward
            **liquidator.lamports.borrow_mut() = liquidator
                .lamports()
                .checked_add(keeper_reward)
                .ok_or(ProgramError::InvalidAccountData)?;
            
            // Vault receives insurance fund portion
            **vault_account.lamports.borrow_mut() = vault_account
                .lamports()
                .checked_add(insurance_fund)
                .ok_or(ProgramError::InvalidAccountData)?;
            
            // Deduct from position account
            **position_account.lamports.borrow_mut() = position_account
                .lamports()
                .checked_sub(liquidation_value)
                .ok_or(ProgramError::InsufficientFunds)?;
            
            // Save position
            position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
            
            // Emit event
            PartialLiquidationExecuted {
                position_id,
                keeper_id: *liquidator.key,
                amount_liquidated: amount,
                keeper_reward,
                risk_score: (100 - health_ratio / 100) as u8,
                slot: clock.slot,
            }.emit();
            
            msg!("Graduated liquidation complete: {} liquidated at level {}", 
                amount, level);
        }
    }
    
    Ok(())
}

/// Calculate maximum safe leverage based on volatility
pub fn calculate_safe_leverage(
    volatility_bps: u16,
    user_experience_score: u8,
) -> u8 {
    // Base max leverage
    let mut max_leverage = 100u8;
    
    // Reduce based on volatility (high volatility = lower leverage)
    if volatility_bps > 500 { // > 5% volatility
        max_leverage = max_leverage.saturating_sub(30);
    } else if volatility_bps > 300 { // > 3% volatility
        max_leverage = max_leverage.saturating_sub(20);
    } else if volatility_bps > 100 { // > 1% volatility
        max_leverage = max_leverage.saturating_sub(10);
    }
    
    // Adjust based on user experience
    if user_experience_score < 50 {
        max_leverage = max_leverage.min(20); // New users capped at 20x
    } else if user_experience_score < 80 {
        max_leverage = max_leverage.min(50); // Intermediate users capped at 50x
    }
    
    max_leverage.max(2) // Minimum 2x leverage
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_graduated_liquidation_levels() {
        let position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user: Pubkey::new_unique(),
            proposal_id: 1,
            position_id: [0; 32],
            outcome: 0,
            size: 10000,
            notional: 10000,
            leverage: 10,
            entry_price: 50000,
            liquidation_price: 45000,
            is_long: true,
            created_at: 0,
            entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 1,
            margin: 1000,
            collateral: 0,
            is_short: false,
            last_mark_price: 50000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        let mut liq_state = GraduatedLiquidationState::new();
        
        // Test level 1 (95% threshold, 10% liquidation)
        let decision = liq_state.check_liquidation_needed(
            &position,
            45250, // Just below 95% threshold
            100,
        ).unwrap();
        
        match decision {
            LiquidationDecision::Liquidate { level, amount, .. } => {
                assert_eq!(level, 0);
                assert_eq!(amount, 1000); // 10% of 10000
            }
            _ => panic!("Expected liquidation"),
        }
    }
    
    #[test]
    fn test_safe_leverage_calculation() {
        // High volatility, new user
        assert_eq!(calculate_safe_leverage(600, 30), 20);
        
        // Low volatility, experienced user
        assert_eq!(calculate_safe_leverage(50, 90), 90);
        
        // Medium volatility, intermediate user
        assert_eq!(calculate_safe_leverage(200, 60), 50);
    }
}