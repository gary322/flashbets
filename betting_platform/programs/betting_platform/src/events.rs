use anchor_lang::prelude::*;

#[event]
pub struct TradeEvent {
    pub user: Pubkey,
    pub verse_id: u128,
    pub amount: u64,
    pub leverage: u64,
    pub is_long: bool,
    pub timestamp: i64,
}

#[event]
pub struct GenesisEvent {
    pub slot: u64,
    pub epoch: u64,
    pub season: u64,
}

#[event]
pub struct EmergencyHaltEvent {
    pub slot: u64,
    pub reason: String,
}

#[event]
pub struct ProposalCreatedEvent {
    pub proposal_id: u128,
    pub verse_id: u128,
    pub proposer: Pubkey,
    pub created_slot: u64,
    pub expiry_slot: u64,
}

#[event]
pub struct ProposalResolvedEvent {
    pub proposal_id: u128,
    pub winning_outcome: String,
    pub resolver: Pubkey,
    pub resolution_slot: u64,
}


#[event]
pub struct PositionClosedEvent {
    pub user: Pubkey,
    pub verse_id: u128,
    pub amount: u64,
    pub exit_price: u64,
    pub pnl: i64,
}

#[event]
pub struct FeeCollectedEvent {
    pub verse_id: u128,
    pub amount: u64,
    pub fee_type: String,
}

#[event]
pub struct VaultUpdateEvent {
    pub old_vault: u64,
    pub new_vault: u64,
    pub old_oi: u64,
    pub new_oi: u64,
    pub coverage: u128,
}

#[event]
pub struct SeasonEndEvent {
    pub season: u64,
    pub end_slot: u64,
    pub total_fees_collected: u64,
    pub mmt_distributed: u64,
}

#[event]
pub struct LeverageUpdateEvent {
    pub verse_id: u128,
    pub old_leverage: u64,
    pub new_leverage: u64,
    pub coverage: u128,
}

#[event]
pub struct PriceUpdateEvent {
    pub verse_id: u128,
    pub price: u64,
    pub slot: u64,
}

#[event]
pub struct DisputeEvent {
    pub verse_id: u128,
    pub market_id: String,
    pub disputed: bool,
    pub slot: u64,
}

#[event]
pub struct KeeperHealthEvent {
    pub keeper: Pubkey,
    pub is_healthy: bool,
    pub metrics: KeeperMetrics,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct KeeperMetrics {
    pub markets_processed: u64,
    pub errors: u64,
    pub avg_latency: u64,
}

#[event]
pub struct PositionOpenedEvent {
    pub user: Pubkey,
    pub verse_id: u128,
    pub proposal_id: u128,
    pub position: Position,
    pub collateral: u64,
}

#[event]
pub struct FeeDistributionEvent {
    pub total_fee: u64,
    pub vault_portion: u64,
    pub mmt_portion: u64,
    pub burn_portion: u64,
    pub new_vault_balance: u64,
}

#[event]
pub struct LiquidationEvent {
    pub user: Pubkey,
    pub keeper: Pubkey,
    pub position_index: u8,
    pub liquidation_price: u64,
    pub pnl: i64,
    pub keeper_reward: u64,
    pub insurance_fund_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct CircuitBreakerEvent {
    pub reason: String,
    pub total_movement: i64,
    pub halt_until: u64,
    pub coverage: u128,
}

#[event]
pub struct HealthWarningEvent {
    pub user: Pubkey,
    pub old_health: u64,
    pub new_health: u64,
    pub at_risk_positions: u8,
}

// Phase 3.5 Chaining Events

#[event]
pub struct ChainCreatedEvent {
    pub chain_id: u128,
    pub user: Pubkey,
    pub verse_id: u128,
    pub initial_deposit: u64,
    pub final_value: u64,
    pub effective_leverage: FixedPoint,
    pub steps: u8,
}

#[event]
pub struct ChainUnwoundEvent {
    pub chain_id: u128,
    pub user: Pubkey,
    pub initial_deposit: u64,
    pub recovered_amount: u64,
    pub loss_amount: u64,
}

// Import needed structs
use crate::account_structs::{Position};
use crate::fixed_math::FixedPoint;