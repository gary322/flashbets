//! Advanced order types system
//!
//! Implements iceberg, TWAP, peg orders, and dark pools as specified in CLAUDE.md
//! All orders route through Polymarket while adding our advanced features on top.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

use crate::math::U64F64;

// Constants for advanced orders
pub const ADVANCED_ORDER_SEED: &[u8] = b"advanced_order";
pub const ADVANCED_ORDER_DISCRIMINATOR: [u8; 8] = [65, 68, 86, 79, 82, 68, 69, 82]; // "ADVORDER"

// Order side
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

// Alias for compatibility
pub use Side as OrderSide;

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum OrderType {
    Market,
    Limit {
        price: U64F64,
    },
    Stop {
        trigger_price: U64F64,
    },
    Iceberg {
        display_size: u64,      // 10% chunks as per CLAUDE.md
        total_size: u64,
        randomization: u8,      // 0-10% randomization
    },
    TWAP {
        duration_slots: u64,    // 10 slots as per CLAUDE.md
        slice_count: u16,
        min_slice_size: u64,
    },
    Peg {
        reference: PegReference,
        offset: i64,            // Positive or negative offset
        limit_price: Option<U64F64>,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum PegReference {
    BestBid,           // Track best bid price
    BestAsk,           // Track best ask price  
    MidPrice,          // Track mid-market price (bid+ask)/2
    PolymarketPrice,   // Track Polymarket price + offset
    VerseDerivedPrice, // Track verse weighted average as per CLAUDE.md
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AdvancedOrder {
    pub discriminator: [u8; 8],
    pub order_id: [u8; 32],
    pub user: Pubkey,
    pub market_id: [u8; 32],
    pub side: Side,
    pub order_type: OrderType,
    pub limit_price: u64,
    pub status: OrderStatus,
    pub created_at: i64,
    pub created_slot: u64,
    pub filled_amount: u64,
    pub remaining_amount: u64,
    pub average_price: u64,
    pub last_execution_slot: u64,
    pub executions_count: u16,
    pub time_priority: bool,
    pub expires_at: Option<i64>,
    pub bump: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum OrderStatus {
    Active,
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

// Size constants for Borsh serialization
impl AdvancedOrder {
    pub const LEN: usize = 8 + // discriminator
        32 + // order_id
        32 + // user
        32 + // market_id
        1 + // side
        1 + 32 + // order_type (enum discriminant + max variant size)
        8 + // limit_price
        1 + // status
        8 + // created_at
        8 + // created_slot
        8 + // filled_amount
        8 + // remaining_amount
        8 + // average_price
        8 + // last_execution_slot
        2 + // executions_count
        1 + // time_priority
        1 + 8 + // expires_at Option
        1; // bump
}

// Polymarket order structure for routing
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketOrder {
    pub market_id: [u8; 32],
    pub side: Side,
    pub size: u64,
    pub order_type: PolymarketOrderType,
    pub time_priority: bool,
    pub dark_pool: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum PolymarketOrderType {
    Market,
    Limit { price: U64F64 },
}

// Polymarket order update for peg orders
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketOrderUpdate {
    pub order_id: [u8; 32],
    pub new_price: U64F64,
    pub maintain_priority: bool,
}

// Price feed for peg orders
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceFeed {
    pub best_bid: U64F64,
    pub best_ask: U64F64,
    pub polymarket_price: U64F64,
    pub last_update_slot: u64,
}

impl PriceFeed {
    pub fn mid_price(&self) -> U64F64 {
        // (bid + ask) / 2
        (self.best_bid + self.best_ask) / U64F64::from_num(2)
    }
    
    pub fn get_latest_price(&self) -> Result<U64F64, solana_program::program_error::ProgramError> {
        Ok(self.polymarket_price)
    }
}

// Dark order status
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum DarkOrderStatus {
    Pending,
    Matched,
    Expired,
    Cancelled,
}