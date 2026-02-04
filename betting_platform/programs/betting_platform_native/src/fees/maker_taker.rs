//! Maker/Taker fee distinction implementation
//!
//! Implements maker rebates for orders that improve the spread by at least 1bp

use solana_program::{
    msg,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::U64F64;

use crate::{
    error::BettingPlatformError,
    fees::{MAKER_REBATE_BPS, SPREAD_IMPROVEMENT_THRESHOLD_BPS},
};

/// Order type classification
#[derive(Debug, Clone, Copy, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum OrderType {
    Maker,
    Taker,
}

/// Fee calculation result including maker/taker status
#[derive(Debug, Clone, Copy)]
pub struct MakerTakerFee {
    pub order_type: OrderType,
    pub base_fee_bps: u16,
    pub final_fee_bps: i16, // Can be negative for maker rebates
    pub spread_improvement_bps: u16,
}

/// Calculate if an order improves the spread
pub fn calculate_spread_improvement(
    current_best_bid: u64,
    current_best_ask: u64,
    order_price: u64,
    is_buy: bool,
) -> Result<u16, ProgramError> {
    // Validate inputs
    if current_best_bid >= current_best_ask {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    let current_spread = current_best_ask - current_best_bid;
    
    // Calculate new spread based on order
    let new_spread = if is_buy {
        // Buy order - check if it improves bid
        if order_price > current_best_bid {
            current_best_ask - order_price
        } else {
            current_spread // No improvement
        }
    } else {
        // Sell order - check if it improves ask
        if order_price < current_best_ask {
            order_price - current_best_bid
        } else {
            current_spread // No improvement
        }
    };
    
    // Calculate improvement in basis points
    if new_spread < current_spread {
        let improvement = current_spread - new_spread;
        let improvement_bps = (improvement * 10000) / current_spread;
        Ok(improvement_bps as u16)
    } else {
        Ok(0)
    }
}

/// Determine maker/taker status and calculate final fee
pub fn calculate_maker_taker_fee(
    base_fee_bps: u16,
    spread_improvement_bps: u16,
) -> MakerTakerFee {
    let order_type = if spread_improvement_bps >= SPREAD_IMPROVEMENT_THRESHOLD_BPS {
        OrderType::Maker
    } else {
        OrderType::Taker
    };
    
    let final_fee_bps = match order_type {
        OrderType::Maker => {
            // Maker receives rebate
            -(MAKER_REBATE_BPS as i16)
        }
        OrderType::Taker => {
            // Taker pays full fee
            base_fee_bps as i16
        }
    };
    
    msg!("Order type: {:?}, spread improvement: {}bp, final fee: {}bp",
         order_type, spread_improvement_bps, final_fee_bps);
    
    MakerTakerFee {
        order_type,
        base_fee_bps,
        final_fee_bps,
        spread_improvement_bps,
    }
}

/// Apply maker/taker fee to an amount
pub fn apply_maker_taker_fee(
    amount: u64,
    fee: &MakerTakerFee,
) -> Result<(u64, u64), ProgramError> {
    if fee.final_fee_bps < 0 {
        // Maker rebate - add to amount
        let rebate_amount = (amount * (-fee.final_fee_bps) as u64) / 10000;
        let final_amount = amount
            .checked_add(rebate_amount)
            .ok_or(BettingPlatformError::MathOverflow)?;
        Ok((final_amount, rebate_amount))
    } else {
        // Taker fee - subtract from amount
        let fee_amount = (amount * fee.final_fee_bps as u64) / 10000;
        let final_amount = amount
            .checked_sub(fee_amount)
            .ok_or(BettingPlatformError::InsufficientFunds)?;
        Ok((final_amount, fee_amount))
    }
}

/// Track maker/taker statistics for a user
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct MakerTakerStats {
    pub total_maker_volume: u64,
    pub total_taker_volume: u64,
    pub maker_rebates_earned: u64,
    pub taker_fees_paid: u64,
    pub maker_order_count: u32,
    pub taker_order_count: u32,
    pub average_spread_improvement: u16,
}

impl MakerTakerStats {
    pub fn new() -> Self {
        Self {
            total_maker_volume: 0,
            total_taker_volume: 0,
            maker_rebates_earned: 0,
            taker_fees_paid: 0,
            maker_order_count: 0,
            taker_order_count: 0,
            average_spread_improvement: 0,
        }
    }
    
    pub fn update(&mut self, volume: u64, fee: &MakerTakerFee, fee_amount: u64) {
        match fee.order_type {
            OrderType::Maker => {
                self.total_maker_volume = self.total_maker_volume.saturating_add(volume);
                self.maker_rebates_earned = self.maker_rebates_earned.saturating_add(fee_amount);
                self.maker_order_count = self.maker_order_count.saturating_add(1);
            }
            OrderType::Taker => {
                self.total_taker_volume = self.total_taker_volume.saturating_add(volume);
                self.taker_fees_paid = self.taker_fees_paid.saturating_add(fee_amount);
                self.taker_order_count = self.taker_order_count.saturating_add(1);
            }
        }
        
        // Update average spread improvement
        let total_orders = self.maker_order_count + self.taker_order_count;
        if total_orders > 0 {
            let total_improvement = (self.average_spread_improvement as u64 * (total_orders - 1) as u64)
                + fee.spread_improvement_bps as u64;
            self.average_spread_improvement = (total_improvement / total_orders as u64) as u16;
        }
    }
    
    pub fn maker_ratio(&self) -> U64F64 {
        let total_volume = self.total_maker_volume + self.total_taker_volume;
        if total_volume == 0 {
            U64F64::from_num(0)
        } else {
            U64F64::from_num(self.total_maker_volume) / U64F64::from_num(total_volume)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spread_improvement_buy_order() {
        let best_bid = 4900; // 0.49
        let best_ask = 5100; // 0.51
        
        // Buy order that improves spread
        let improvement = calculate_spread_improvement(best_bid, best_ask, 4950, true).unwrap();
        assert_eq!(improvement, 2500); // 25% improvement (50/200 * 10000)
        
        // Buy order that doesn't improve spread
        let improvement = calculate_spread_improvement(best_bid, best_ask, 4800, true).unwrap();
        assert_eq!(improvement, 0);
    }
    
    #[test]
    fn test_spread_improvement_sell_order() {
        let best_bid = 4900; // 0.49
        let best_ask = 5100; // 0.51
        
        // Sell order that improves spread
        let improvement = calculate_spread_improvement(best_bid, best_ask, 5050, false).unwrap();
        assert_eq!(improvement, 2500); // 25% improvement
        
        // Sell order that doesn't improve spread
        let improvement = calculate_spread_improvement(best_bid, best_ask, 5200, false).unwrap();
        assert_eq!(improvement, 0);
    }
    
    #[test]
    fn test_maker_taker_classification() {
        // Maker (improves spread by >= 1bp)
        let fee = calculate_maker_taker_fee(10, 150); // 1.5% improvement
        assert_eq!(fee.order_type, OrderType::Maker);
        assert_eq!(fee.final_fee_bps, -3); // 3bp rebate
        
        // Taker (doesn't improve spread enough)
        let fee = calculate_maker_taker_fee(10, 50); // 0.5% improvement
        assert_eq!(fee.order_type, OrderType::Taker);
        assert_eq!(fee.final_fee_bps, 10); // Full fee
    }
    
    #[test]
    fn test_apply_maker_rebate() {
        let amount = 1_000_000; // $1
        let fee = MakerTakerFee {
            order_type: OrderType::Maker,
            base_fee_bps: 10,
            final_fee_bps: -3,
            spread_improvement_bps: 150,
        };
        
        let (final_amount, rebate) = apply_maker_taker_fee(amount, &fee).unwrap();
        assert_eq!(final_amount, 1_000_300); // $1 + $0.003 rebate
        assert_eq!(rebate, 300);
    }
    
    #[test]
    fn test_apply_taker_fee() {
        let amount = 1_000_000; // $1
        let fee = MakerTakerFee {
            order_type: OrderType::Taker,
            base_fee_bps: 10,
            final_fee_bps: 10,
            spread_improvement_bps: 0,
        };
        
        let (final_amount, fee_paid) = apply_maker_taker_fee(amount, &fee).unwrap();
        assert_eq!(final_amount, 999_000); // $1 - $0.001 fee
        assert_eq!(fee_paid, 1_000);
    }
}