//! Liquidation keeper system
//!
//! Implements permissionless keepers with 5bp liquidation rewards

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    events::{Event, LiquidationExecuted},
    liquidation::calculate_risk_score_with_price,
    math::U64F64,
    state::{Position, KeeperAccount, KeeperStatus},
};

/// Keeper reward basis points (5bp = 0.05%)
pub const KEEPER_REWARD_BPS: u64 = 5;

/// Maximum liquidation percent per slot (8%)
pub const MAX_LIQUIDATION_PERCENT: u64 = 800;

/// Liquidation threshold (90% risk score)
pub const LIQUIDATION_THRESHOLD: u8 = 90;

/// Monitoring threshold (80% risk score)
pub const MONITORING_THRESHOLD: u8 = 80;

/// Sigma factor for volatility-based liquidation calculations
pub const SIGMA_FACTOR: u64 = 150; // 1.5 in basis points

/// Minimum liquidation cap percentage (2%)
pub const LIQ_CAP_MIN: u64 = 200; // 2% in basis points

/// Maximum liquidation cap percentage (8%)
pub const LIQ_CAP_MAX: u64 = 800; // 8% in basis points

/// At-risk position information
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AtRiskPosition {
    pub position_id: [u8; 32],
    pub account: Pubkey,
    pub risk_score: u8,
    pub distance_to_liquidation: U64F64,
    pub notional: u64,
    pub leverage: u64,
}

/// Liquidation keeper implementation
pub struct LiquidationKeeper;

impl LiquidationKeeper {
    /// Execute liquidation with 5bp bounty
    pub fn execute_liquidation(
        position: &mut Position,
        keeper: &mut KeeperAccount,
        vault: &AccountInfo,
        current_price: U64F64,
    ) -> ProgramResult {
        // Verify position is at risk
        let risk_score = calculate_risk_score_with_price(position, current_price)?;
        
        if risk_score < LIQUIDATION_THRESHOLD {
            return Err(BettingPlatformError::PositionNotAtRisk.into());
        }
        
        // Verify keeper is active
        if keeper.status != KeeperStatus::Active {
            return Err(BettingPlatformError::NoActiveKeepers.into());
        }
        
        // Calculate liquidation amount (max 8% per slot)
        let max_liquidation = position.notional
            .checked_mul(MAX_LIQUIDATION_PERCENT)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        let margin_at_risk = Self::calculate_margin_at_risk(position, current_price)?;
        let liquidation_amount = max_liquidation.min(margin_at_risk);
        
        // Execute partial liquidation
        position.execute_partial_liquidation(liquidation_amount, current_price)?;
        
        // Calculate keeper reward (5bp of liquidated amount)
        let keeper_reward = liquidation_amount
            .checked_mul(KEEPER_REWARD_BPS)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        // Transfer reward from vault to keeper
        // Note: In production, this would use CPI to transfer SOL
        
        // Update keeper stats
        keeper.successful_operations = keeper
            .successful_operations
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.total_operations = keeper
            .total_operations
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.total_rewards_earned = keeper
            .total_rewards_earned
            .checked_add(keeper_reward)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.last_operation_slot = Clock::get()?.slot;
        
        // Update performance score
        keeper.performance_score = keeper.successful_operations
            .checked_mul(10000)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(keeper.total_operations)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        // Emit liquidation event
        LiquidationExecuted {
            position_id: position.position_id,
            keeper_id: keeper.keeper_id,
            amount_liquidated: liquidation_amount,
            keeper_reward,
            risk_score,
            slot: Clock::get()?.slot,
        }.emit();
        
        msg!("Liquidated {} from position {}, keeper reward: {}",
            liquidation_amount,
            bs58::encode(&position.position_id[..8]).into_string(),
            keeper_reward
        );
        
        Ok(())
    }
    
    /// Scan for at-risk positions
    pub fn scan_at_risk_positions(
        position_accounts: &[AccountInfo],
        current_price: U64F64,
        batch_size: u8,
    ) -> Result<Vec<AtRiskPosition>, ProgramError> {
        let mut at_risk = Vec::new();
        
        for account in position_accounts.iter() {
            if at_risk.len() >= batch_size as usize {
                break;
            }
            
            match Position::try_from_slice(&account.data.borrow()) {
                Ok(position) => {
                    let risk_score = calculate_risk_score_with_price(&position, current_price)?;
                    
                    if risk_score >= MONITORING_THRESHOLD {
                        let distance_to_liq = Self::calculate_distance_to_liquidation(
                            &position,
                            current_price
                        )?;
                        
                        at_risk.push(AtRiskPosition {
                            position_id: position.position_id,
                            account: *account.key,
                            risk_score,
                            distance_to_liquidation: distance_to_liq,
                            notional: position.notional,
                            leverage: position.leverage,
                        });
                    }
                }
                Err(_) => continue,
            }
        }
        
        // Sort by risk score (highest first)
        at_risk.sort_by(|a, b| b.risk_score.cmp(&a.risk_score));
        
        Ok(at_risk)
    }
    
    /// Calculate dynamic liquidation cap based on volatility
    /// Returns liquidation cap as a percentage (in basis points)
    /// Formula: clamp(LIQ_CAP_MIN, SIGMA_FACTOR*σ, LIQ_CAP_MAX)*OI
    pub fn calculate_dynamic_liquidation_cap(
        volatility_sigma: U64F64,
        open_interest: u64,
    ) -> Result<u64, ProgramError> {
        // Calculate SIGMA_FACTOR * σ
        let sigma_factor_fp = U64F64::from_num(SIGMA_FACTOR) / U64F64::from_num(10000);
        let volatility_component = sigma_factor_fp
            .checked_mul(volatility_sigma)?;
        
        // Convert volatility component to basis points
        let volatility_bps = (volatility_component * U64F64::from_num(10000)).to_num();
        
        // Clamp between LIQ_CAP_MIN and LIQ_CAP_MAX
        let clamped_cap = volatility_bps.clamp(LIQ_CAP_MIN, LIQ_CAP_MAX);
        
        // Apply to open interest: (cap * OI) / 10000
        let liquidation_amount = (clamped_cap as u128)
            .checked_mul(open_interest as u128)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        Ok(liquidation_amount as u64)
    }
    
    /// Calculate margin at risk for position
    fn calculate_margin_at_risk(
        position: &Position,
        current_price: U64F64,
    ) -> Result<u64, ProgramError> {
        // Calculate current position value
        let position_value = if position.is_long {
            // Long: value = size * current_price
            let price_ratio = current_price
                .checked_div(U64F64::from_num(position.entry_price))?;
            
            U64F64::from_num(position.size)
                .checked_mul(price_ratio)?
                .to_num()
        } else {
            // Short: value = size * (2 - current_price/entry_price)
            let price_ratio = current_price
                .checked_div(U64F64::from_num(position.entry_price))?;
            
            let factor = U64F64::from_num(2u64)
                .checked_sub(price_ratio)?;
            
            U64F64::from_num(position.size)
                .checked_mul(factor)?
                .to_num()
        };
        
        // Margin at risk = initial margin - unrealized P&L
        let initial_margin = position.size
            .checked_div(position.leverage)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        let pnl = if position_value > position.size {
            position_value.saturating_sub(position.size)
        } else {
            0
        };
        
        Ok(initial_margin.saturating_sub(pnl))
    }
    
    /// Calculate distance to liquidation price
    fn calculate_distance_to_liquidation(
        position: &Position,
        current_price: U64F64,
    ) -> Result<U64F64, ProgramError> {
        let liq_price = U64F64::from_num(position.liquidation_price);
        
        if position.is_long {
            // Long: distance = (current - liq) / current
            current_price
                .checked_sub(liq_price)?
                .checked_div(current_price)
        } else {
            // Short: distance = (liq - current) / current
            liq_price
                .checked_sub(current_price)?
                .checked_div(current_price)
        }
    }
}

impl Position {
    /// Execute partial liquidation on position
    pub fn execute_partial_liquidation(
        &mut self,
        amount: u64,
        current_price: U64F64,
    ) -> Result<(), ProgramError> {
        // Reduce position size
        self.size = self.size
            .checked_sub(amount)
            .ok_or(BettingPlatformError::Underflow)?;
        
        // Update notional
        self.notional = self.notional
            .checked_sub(amount)
            .ok_or(BettingPlatformError::Underflow)?;
        
        // If position fully liquidated, mark as closed
        if self.size == 0 {
            self.is_closed = true;
        }
        
        // Update partial liquidation accumulator
        self.partial_liq_accumulator = self
            .partial_liq_accumulator
            .checked_add(amount)
            .ok_or(BettingPlatformError::Overflow)?;
        
        Ok(())
    }
}

// KeeperAccount implementation is in state/keeper_accounts.rs

// Hex encoding utility
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_liquidation_reward_calculation() {
        let liquidation_amount = 100_000_000; // $100
        let expected_reward = 50_000; // 5bp = $0.05
        
        let reward = liquidation_amount * KEEPER_REWARD_BPS / 10000;
        assert_eq!(reward, expected_reward);
    }
    
    #[test]
    fn test_max_liquidation_limit() {
        let notional = 1_000_000_000; // $1000
        let expected_max = 80_000_000; // 8% = $80
        
        let max_liq = notional * MAX_LIQUIDATION_PERCENT / 10000;
        assert_eq!(max_liq, expected_max);
    }
    
    #[test]
    fn test_risk_score_thresholds() {
        assert!(LIQUIDATION_THRESHOLD > MONITORING_THRESHOLD);
        assert_eq!(LIQUIDATION_THRESHOLD, 90);
        assert_eq!(MONITORING_THRESHOLD, 80);
    }
}