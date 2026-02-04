use anchor_lang::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct ContentionMetrics {
    pub avg_write_time_ms: f64,
    pub peak_write_time_ms: f64,
    pub transaction_count: u64,
    pub hot_markets: Vec<Pubkey>,
}

impl Default for ContentionMetrics {
    fn default() -> Self {
        Self {
            avg_write_time_ms: 0.0,
            peak_write_time_ms: 0.0,
            transaction_count: 0,
            hot_markets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub struct RebalanceExecution {
    pub proposal_id: [u8; 32],
    pub moves: Vec<(Pubkey, u8, u8)>, // (market, from_shard, to_shard)
    pub execution_slot: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub enum MigrationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct MarketSnapshot {
    pub market_id: Pubkey,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub amm_state: AmmState,
    pub snapshot_slot: u64,
}

// Placeholder structs - these should match your actual trading engine types
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct Position {
    pub owner: Pubkey,
    pub size: i64,
    pub entry_price: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct Order {
    pub owner: Pubkey,
    pub side: OrderSide,
    pub size: u64,
    pub price: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct AmmState {
    pub liquidity: u64,
    pub fees_collected: u64,
}