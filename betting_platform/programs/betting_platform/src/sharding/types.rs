use anchor_lang::prelude::*;

pub const SHARD_COUNT_DEFAULT: u8 = 4;
pub const MAX_CONTENTION_MS: f64 = 1.5;
pub const REBALANCE_INTERVAL: u64 = 1000; // slots
pub const VOTING_PERIOD_SLOTS: u64 = 100; // ~40 seconds
pub const VOTE_THRESHOLD: f64 = 0.667; // 66.7% majority
pub const MIGRATION_TIMEOUT_SLOTS: u64 = 500;

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

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct RebalanceProposal {
    pub id: [u8; 32],
    pub overloaded_shards: Vec<(u8, ContentionMetrics)>,
    pub underloaded_shards: Vec<(u8, ContentionMetrics)>,
    pub markets_to_move: Vec<(Pubkey, u8, u8)>, // (market, from, to)
    pub estimated_improvement: f64,
    pub votes_for: u64,
    pub votes_against: u64,
    pub voting_ends_slot: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct RebalanceExecution {
    pub proposal_id: [u8; 32],
    pub moves: Vec<(Pubkey, u8, u8)>,
    pub execution_slot: u64,
}

#[derive(Clone, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub enum MigrationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct MigrationState {
    pub market_id: Pubkey,
    pub from_shard: u8,
    pub to_shard: u8,
    pub migration_started: u64,
    pub state_snapshot: MarketSnapshot,
    pub status: MigrationStatus,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct MarketSnapshot {
    pub market_id: Pubkey,
    pub positions: Vec<PositionSnapshot>,
    pub orders: Vec<OrderSnapshot>,
    pub amm_state: AMMStateSnapshot,
    pub snapshot_slot: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct PositionSnapshot {
    pub user: Pubkey,
    pub amount: u64,
    pub entry_price: u64,
    pub leverage: u32,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct OrderSnapshot {
    pub order_id: u128,
    pub user: Pubkey,
    pub side: bool, // true = buy, false = sell
    pub size: u64,
    pub price: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct AMMStateSnapshot {
    pub liquidity: u64,
    pub yes_shares: u64,
    pub no_shares: u64,
    pub k_constant: u128,
}