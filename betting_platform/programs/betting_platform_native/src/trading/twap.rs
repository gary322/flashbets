//! TWAP (Time-Weighted Average Price) order implementation
//!
//! Executes orders over time to minimize market impact

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::trading::advanced_orders::{
    AdvancedOrder, OrderType, OrderStatus, PolymarketOrder, PolymarketOrderType, PriceFeed,
};

pub const DEFAULT_TWAP_DURATION: u64 = 10; // 10 slots as per CLAUDE.md
pub const MIN_SLICE_SIZE_BPS: u64 = 100;   // 1% minimum slice

pub struct TWAPEngine;

impl TWAPEngine {
    /// Calculate TWAP slice size and timing
    pub fn calculate_twap_slice(
        total_size: u64,
        duration_slots: u64,
        slice_count: u16,
        current_slot: u64,
        start_slot: u64,
        executions_so_far: u16,
    ) -> Result<(u64, u64), ProgramError> { // (slice_size, next_execution_slot)
        if slice_count == 0 {
            return Err(BettingPlatformError::InvalidSliceCount.into());
        }
        if executions_so_far >= slice_count {
            return Err(BettingPlatformError::TWAPComplete.into());
        }
        
        // Calculate time per slice
        let slots_per_slice = duration_slots
            .saturating_div(slice_count as u64)
            .max(1);
        
        // Calculate next execution slot
        let next_slot = start_slot
            .saturating_add(slots_per_slice.saturating_mul((executions_so_far + 1) as u64));
        
        // Calculate slice size (weighted by remaining time)
        let remaining_slices = slice_count.saturating_sub(executions_so_far);
        let remaining_size = total_size
            .saturating_sub(
                total_size
                    .saturating_mul(executions_so_far as u64)
                    .saturating_div(slice_count as u64)
            );
        
        let slice_size = if remaining_slices == 1 {
            remaining_size // Last slice takes all remaining
        } else {
            remaining_size.saturating_div(remaining_slices as u64)
        };
        
        Ok((slice_size, next_slot))
    }
    
    /// Execute TWAP slice
    pub fn execute_twap_slice(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order_id: [u8; 32],
    ) -> ProgramResult {
        // Account layout:
        // 0. Order account (mut)
        // 1. User account (signer)
        // 2. Price feed
        // 3. Keeper signer
        // 4. Polymarket interface
        // 5. System program
        // 6. Clock sysvar
        
        if accounts.len() < 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let order_account = &accounts[0];
        let user_account = &accounts[1];
        let price_feed_account = &accounts[2];
        let keeper_account = &accounts[3];
        let polymarket_interface = &accounts[4];
        let _system_program = &accounts[5];
        let clock = Clock::get()?;
        
        // Deserialize order
        let mut order_data = order_account.try_borrow_mut_data()?;
        let mut order = AdvancedOrder::deserialize(&mut &order_data[..])?;
        
        // Deserialize price feed
        let price_feed_data = price_feed_account.try_borrow_data()?;
        let price_feed = PriceFeed::deserialize(&mut &price_feed_data[..])?;
        
        match &order.order_type {
            OrderType::TWAP { duration_slots, slice_count, min_slice_size } => {
                // Check if it's time for next slice
                let (slice_size, next_slot) = Self::calculate_twap_slice(
                    order.remaining_amount + order.filled_amount,
                    *duration_slots,
                    *slice_count,
                    clock.slot,
                    order.created_slot,
                    order.executions_count,
                )?;
                
                if clock.slot < next_slot && order.executions_count > 0 {
                    return Err(BettingPlatformError::TWAPTooEarly.into());
                }
                
                if slice_size < *min_slice_size {
                    return Err(BettingPlatformError::SliceTooSmall.into());
                }
                
                // Route to Polymarket with time priority
                let polymarket_order = PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Market,
                    time_priority: true, // TWAP gets priority
                    dark_pool: false,
                };
                
                msg!("Routing TWAP slice to Polymarket: {} units with priority", slice_size);
                
                // Update state
                order.filled_amount = order.filled_amount.saturating_add(slice_size);
                order.remaining_amount = order.remaining_amount.saturating_sub(slice_size);
                order.executions_count += 1;
                order.last_execution_slot = clock.slot;
                
                // Update average price (weighted)
                let execution_price = price_feed.get_latest_price()?;
                let old_value = U64F64::from_num(order.average_price) * U64F64::from_num(order.filled_amount.saturating_sub(slice_size));
                let new_value = execution_price * U64F64::from_num(slice_size);
                order.average_price = ((old_value + new_value) / U64F64::from_num(order.filled_amount)).to_num();
                
                if order.remaining_amount == 0 {
                    order.status = OrderStatus::Filled;
                    
                    msg!(
                        "TWAPCompleted - order_id: {:?}, average_price: {}, total_slices: {}, duration: {}",
                        order_id,
                        order.average_price,
                        order.executions_count,
                        clock.slot.saturating_sub(order.created_slot)
                    );
                }
                
                // Serialize updated order back
                order.serialize(&mut *order_data)?;
                
                Ok(())
            }
            _ => Err(BettingPlatformError::InvalidOrderType.into()),
        }
    }
}

// Helper functions for TWAP management
impl TWAPEngine {
    /// Check if TWAP order should execute
    pub fn should_execute_slice(
        order: &AdvancedOrder,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        match &order.order_type {
            OrderType::TWAP { duration_slots, slice_count, .. } => {
                let (_, next_slot) = Self::calculate_twap_slice(
                    order.remaining_amount + order.filled_amount,
                    *duration_slots,
                    *slice_count,
                    current_slot,
                    order.created_slot,
                    order.executions_count,
                )?;
                
                Ok(current_slot >= next_slot || order.executions_count == 0)
            }
            _ => Ok(false),
        }
    }
    
    /// Get progress percentage
    pub fn get_progress_percentage(order: &AdvancedOrder) -> u8 {
        if order.filled_amount == 0 {
            return 0;
        }
        
        let total = order.filled_amount + order.remaining_amount;
        if total == 0 {
            return 100;
        }
        
        ((order.filled_amount * 100) / total) as u8
    }
}