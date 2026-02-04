//! Settlement Module for Perpetual Positions
//!
//! Handles settlement, expiry, and final PnL calculations

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    cdp::CDPAccount,
};

use super::{
    state::{PerpetualPosition, PerpetualMarket, PositionStatus},
    funding::calculate_funding_payment,
};

/// Settlement type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum SettlementType {
    /// Cash settlement
    Cash,
    /// Physical delivery
    Physical,
    /// Auto-roll to next contract
    AutoRoll,
    /// Forced liquidation
    ForcedLiquidation,
}

/// Settlement configuration
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SettlementConfig {
    /// Settlement type
    pub settlement_type: SettlementType,
    
    /// Settlement price source
    pub price_source: PriceSource,
    
    /// Grace period after expiry (slots)
    pub grace_period: u64,
    
    /// Settlement fee
    pub settlement_fee: f64,
    
    /// Auto-roll enabled by default
    pub auto_roll_default: bool,
}

/// Price source for settlement
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum PriceSource {
    /// Use oracle TWAP
    OracleTWAP,
    /// Use mark price at expiry
    MarkPrice,
    /// Use index price
    IndexPrice,
    /// Volume-weighted average
    VWAP,
}

/// Settlement result
#[derive(Debug)]
pub struct SettlementResult {
    /// Position ID
    pub position_id: u128,
    
    /// Settlement price
    pub settlement_price: f64,
    
    /// Final PnL
    pub final_pnl: i128,
    
    /// Settlement fee paid
    pub settlement_fee: u128,
    
    /// Collateral returned
    pub collateral_returned: u128,
    
    /// Settlement type used
    pub settlement_type: SettlementType,
    
    /// Timestamp of settlement
    pub settled_at: i64,
}

/// Execute settlement for expired position
pub fn settle_expired_position(
    position: &mut PerpetualPosition,
    market: &PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
    config: &SettlementConfig,
) -> Result<SettlementResult, ProgramError> {
    // Check if position is expired
    let current_time = Clock::get()?.unix_timestamp;
    
    if let Some(expiry) = position.expiry {
        if current_time < expiry {
            return Err(BettingPlatformError::PositionNotExpired.into());
        }
    } else {
        // Perpetual position, no expiry
        return Err(BettingPlatformError::InvalidExpiryTime.into());
    }
    
    // Check grace period
    let grace_end = position.expiry.unwrap() + config.grace_period as i64;
    let in_grace_period = current_time <= grace_end;
    
    // Determine settlement type
    let settlement_type = if position.auto_roll_enabled && in_grace_period {
        SettlementType::AutoRoll
    } else if position.is_liquidatable() {
        SettlementType::ForcedLiquidation
    } else {
        config.settlement_type.clone()
    };
    
    // Get settlement price
    let settlement_price = get_settlement_price(oracle, market, &config.price_source);
    
    // Update position to settlement price
    position.mark_price = settlement_price;
    position.calculate_unrealized_pnl();
    
    // Calculate final funding
    let final_funding = calculate_funding_payment(
        position,
        market.funding_rate,
        current_time,
    );
    position.apply_funding(final_funding);
    
    // Calculate settlement fee
    let settlement_fee = calculate_settlement_fee(
        position.size,
        settlement_price,
        config.settlement_fee,
    );
    
    // Calculate final PnL
    let gross_pnl = position.unrealized_pnl + position.realized_pnl + position.accumulated_funding;
    let final_pnl = gross_pnl - settlement_fee as i128;
    
    // Calculate collateral to return
    let collateral_returned = if final_pnl >= 0 {
        position.collateral + final_pnl as u128
    } else {
        position.collateral.saturating_sub(final_pnl.abs() as u128)
    };
    
    // Update CDP
    if collateral_returned > 0 {
        cdp.withdraw_collateral(collateral_returned, settlement_price)?;
    }
    
    // Clear CDP debt if any
    if cdp.debt_amount > 0 {
        let repay_amount = cdp.debt_amount.min(collateral_returned);
        cdp.debt_amount = cdp.debt_amount.saturating_sub(repay_amount);
    }
    
    // Mark position as settled
    position.status = match settlement_type {
        SettlementType::AutoRoll => PositionStatus::RollingOver,
        SettlementType::ForcedLiquidation => PositionStatus::Liquidated,
        _ => PositionStatus::Expired,
    };
    
    let result = SettlementResult {
        position_id: position.position_id,
        settlement_price,
        final_pnl,
        settlement_fee,
        collateral_returned,
        settlement_type,
        settled_at: current_time,
    };
    
    msg!("Settled position {} at price {:.2} with PnL {}", 
         position.position_id, settlement_price, final_pnl);
    
    Ok(result)
}

/// Batch settle multiple positions
pub fn batch_settle_positions(
    positions: &mut [PerpetualPosition],
    market: &PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
    config: &SettlementConfig,
) -> Result<Vec<SettlementResult>, ProgramError> {
    let mut results = Vec::new();
    let current_time = Clock::get()?.unix_timestamp;
    
    for position in positions.iter_mut() {
        // Skip if not expired
        if let Some(expiry) = position.expiry {
            if current_time < expiry {
                continue;
            }
        } else {
            continue;
        }
        
        // Skip if already settled
        if position.status == PositionStatus::Expired ||
           position.status == PositionStatus::Closed ||
           position.status == PositionStatus::Liquidated {
            continue;
        }
        
        // Attempt settlement
        match settle_expired_position(position, market, cdp, oracle, config) {
            Ok(result) => {
                results.push(result);
            },
            Err(e) => {
                msg!("Failed to settle position {}: {:?}", position.position_id, e);
            }
        }
    }
    
    msg!("Batch settled {} positions", results.len());
    Ok(results)
}

/// Get settlement price based on source
fn get_settlement_price(
    oracle: &OraclePDA,
    market: &PerpetualMarket,
    source: &PriceSource,
) -> f64 {
    match source {
        PriceSource::OracleTWAP => oracle.twap_prob,
        PriceSource::MarkPrice => market.mark_price,
        PriceSource::IndexPrice => oracle.current_prob,
        PriceSource::VWAP => {
            // Simple average for now
            (market.mark_price + oracle.current_prob) / 2.0
        }
    }
}

/// Calculate settlement fee
fn calculate_settlement_fee(
    size: u128,
    price: f64,
    fee_rate: f64,
) -> u128 {
    ((size as f64) * price * fee_rate) as u128
}

/// Force settle all positions (emergency)
pub fn force_settle_all(
    positions: &mut [PerpetualPosition],
    market: &PerpetualMarket,
    oracle: &OraclePDA,
    settlement_price: f64,
) -> Result<u32, ProgramError> {
    let mut settled_count = 0;
    
    for position in positions.iter_mut() {
        if position.status != PositionStatus::Active {
            continue;
        }
        
        // Force update to settlement price
        position.mark_price = settlement_price;
        position.calculate_unrealized_pnl();
        
        // Close position
        position.status = PositionStatus::Expired;
        settled_count += 1;
        
        msg!("Force settled position {} at {}", 
             position.position_id, settlement_price);
    }
    
    msg!("Force settled {} positions", settled_count);
    Ok(settled_count)
}

/// Calculate settlement profit/loss
pub fn calculate_settlement_pnl(
    position: &PerpetualPosition,
    settlement_price: f64,
) -> i128 {
    let price_diff = settlement_price - position.entry_price;
    
    let pnl = match position.position_type {
        super::state::PositionType::Long => {
            (position.size as f64) * price_diff / position.entry_price
        },
        super::state::PositionType::Short => {
            -(position.size as f64) * price_diff / position.entry_price
        }
    };
    
    pnl as i128 + position.accumulated_funding + position.realized_pnl
}

/// Check if market should enter settlement-only mode
pub fn should_enter_settlement_mode(
    market: &PerpetualMarket,
    oracle: &OraclePDA,
) -> bool {
    // Enter settlement mode if:
    // 1. Oracle is stale
    let oracle_stale = Clock::get()
        .map(|c| c.slot.saturating_sub(oracle.last_update_slot) > 10)
        .unwrap_or(false);
    
    // 2. Extreme price deviation
    let price_deviation = ((market.mark_price - oracle.current_prob).abs() / oracle.current_prob) > 0.1;
    
    // 3. Liquidity crisis (large imbalance)
    let total_oi = market.open_interest_long + market.open_interest_short;
    let imbalance = if total_oi > 0 {
        let diff = (market.open_interest_long as f64 - market.open_interest_short as f64).abs();
        diff / total_oi as f64 > 0.8
    } else {
        false
    };
    
    oracle_stale || price_deviation || imbalance
}

impl Default for SettlementConfig {
    fn default() -> Self {
        Self {
            settlement_type: SettlementType::Cash,
            price_source: PriceSource::OracleTWAP,
            grace_period: 432000, // ~3 days
            settlement_fee: 0.001, // 0.1%
            auto_roll_default: true,
        }
    }
}

impl PerpetualPosition {
    fn calculate_unrealized_pnl(&mut self) {
        let price_diff = self.mark_price - self.entry_price;
        
        let pnl = match self.position_type {
            super::state::PositionType::Long => {
                (self.size as f64) * price_diff / self.entry_price
            },
            super::state::PositionType::Short => {
                -(self.size as f64) * price_diff / self.entry_price
            }
        };
        
        self.unrealized_pnl = pnl as i128;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_settlement_fee_calculation() {
        let fee = calculate_settlement_fee(10000, 100.0, 0.001);
        assert_eq!(fee, 100); // 0.1% of 1M
    }
    
    #[test]
    fn test_settlement_pnl() {
        let position = PerpetualPosition::new(
            1,
            Pubkey::new_unique(),
            1,
            Pubkey::new_unique(),
            super::super::state::PositionType::Long,
            100.0,
            10000,
            10,
            1000,
        );
        
        // Price increases to 110
        let pnl = calculate_settlement_pnl(&position, 110.0);
        assert!(pnl > 0);
        
        // Price decreases to 90
        let pnl = calculate_settlement_pnl(&position, 90.0);
        assert!(pnl < 0);
    }
}