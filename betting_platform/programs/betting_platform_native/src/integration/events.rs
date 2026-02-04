//! Event structures for Phase 20 Integration module

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use crate::events::{EventType, emit_event as base_emit_event};

// Helper function to emit events properly
pub fn emit_event<T: BorshSerialize>(event_type: EventType, event_data: T) -> Result<(), solana_program::program_error::ProgramError> {
    base_emit_event(event_type, &event_data);
    Ok(())
}

// Event structures for Phase 20

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CoordinatorInitializedEvent {
    pub admin: Pubkey,
    pub components: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BootstrapProgressEvent {
    pub vault_balance: u64,
    pub target: u64,
    pub progress_pct: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BootstrapStartedEvent {
    pub target_vault: u64,
    pub incentive_pool: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BootstrapDepositEvent {
    pub depositor: Pubkey,
    pub amount: u64,
    pub vault_balance: u64,
    pub mmt_earned: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BootstrapCompleteEvent {
    pub coverage: u64,
    pub max_leverage: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BootstrapCompleteDetailedEvent {
    pub final_vault: u64,
    pub total_depositors: u32,
    pub duration_slots: u64,
    pub mmt_distributed: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SystemHealthCheckEvent {
    pub status: u8,
    pub components_healthy: u8,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ComponentHealthUpdateEvent {
    pub component_name: [u8; 32],
    pub status: u8,
    pub latency_ms: u64,
    pub throughput: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct EmergencyShutdownEvent {
    pub reason: String,
    pub admin: Pubkey,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MarketBatchProcessedEvent {
    pub batch_size: u32,
    pub total_markets: u64,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VaultBalanceUpdatedEvent {
    pub old_balance: u64,
    pub new_balance: u64,
    pub change: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct AutoRecoveryTriggeredEvent {
    pub component: [u8; 32],
    pub recovery_action: u8,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MilestoneReachedEvent {
    pub milestone: u32,
    pub vault_balance: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ReferralRewardEvent {
    pub referrer: Pubkey,
    pub referred: Pubkey,
    pub reward: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct HealthCheckCompleteEvent {
    pub status: u8,
    pub components_healthy: u8,
    pub slot: u64,
}