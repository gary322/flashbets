use anchor_lang::prelude::*;
use crate::errors::ErrorCode;

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum OrderType {
    Market,
    Limit { price: u64 },
    Stop { trigger_price: u64 },
    StopLimit { trigger_price: u64, limit_price: u64 },
    Iceberg { visible_size: u64, total_size: u64 },
    TWAP { duration: u64, intervals: u8 },
    Peg { offset: i64, peg_type: PegType },
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum PegType {
    Midpoint,
    Primary,
    Market,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum OrderStatus {
    Active,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

#[account]
pub struct AdvancedOrderPDA {
    pub order_id: u128,
    pub user: Pubkey,
    pub market_id: u128,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub outcome: u8,
    pub remaining_size: u64,
    pub executed_size: u64,
    pub average_price: u64,
    pub status: OrderStatus,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub execution_metadata: OrderExecutionMetadata,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct OrderExecutionMetadata {
    pub last_execution_slot: u64,
    pub num_fills: u32,
    pub twap_progress: Option<TWAPProgress>,
    pub iceberg_revealed: u64,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct TWAPProgress {
    pub intervals_completed: u8,
    pub next_execution_slot: u64,
    pub size_per_interval: u64,
}

// Helper functions
pub fn generate_order_id() -> u128 {
    // In production, this would use a combination of slot, user pubkey, and counter
    let clock = Clock::get().unwrap();
    let slot = clock.slot;
    let timestamp = clock.unix_timestamp;
    
    // Simple hash combination
    ((slot as u128) << 64) | (timestamp as u128)
}

// Placeholder for orderbook integration
pub fn add_to_orderbook<'info>(
    _accounts: &impl AsRef<[AccountInfo<'info>]>,
    _order_id: u128,
    _size: u64,
) -> Result<()> {
    // This would integrate with the orderbook module
    // For now, just log the action
    msg!("Adding order {} with size {} to orderbook", _order_id, _size);
    Ok(())
}

// Placeholder for market order execution
pub fn execute_market_order<'info>(
    _accounts: &impl AsRef<[AccountInfo<'info>]>,
    _market_id: u128,
    _outcome: u8,
    _size: u64,
    _side: OrderSide,
) -> Result<ExecutionResult> {
    // This would execute against the appropriate AMM
    // For now, return a mock result
    Ok(ExecutionResult {
        executed_size: _size,
        average_price: 500_000_000_000_000_000, // 0.5 in fixed point
    })
}

pub struct ExecutionResult {
    pub executed_size: u64,
    pub average_price: u64,
}

// Events
#[event]
pub struct OrderPlacedEvent {
    pub order_id: u128,
    pub user: Pubkey,
    pub market_id: u128,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub outcome: u8,
    pub size: u64,
}

#[event]
pub struct OrderFilledEvent {
    pub order_id: u128,
    pub executed_size: u64,
    pub average_price: u64,
}

#[event]
pub struct OrderCancelledEvent {
    pub order_id: u128,
    pub remaining_size: u64,
}