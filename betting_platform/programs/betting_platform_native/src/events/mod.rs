//! Event logging system for native Solana
//!
//! Comprehensive event system to replace Anchor's emit! macro

pub mod chain_events;
pub mod migration_events;
#[cfg(test)]
mod chain_events_test;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    msg,
    pubkey::Pubkey,
};

pub use chain_events::*;
pub use migration_events::*;

/// Event type discriminator
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum EventType {
    // Core events
    GenesisEvent = 1,
    EmergencyHaltEvent = 2,
    
    // Trading events
    PositionOpened = 10,
    PositionClosed = 11,
    PositionLiquidated = 12,
    
    // Fee events
    FeesDistributed = 20,
    
    // Chain events
    ChainCreated = 30,
    ChainStepExecuted = 31,
    ChainCompleted = 32,
    ChainFailed = 33,
    ChainUnwound = 34,
    
    // AMM events
    MarketCreated = 40,
    TradePlaced = 41,
    LiquidityAdded = 42,
    LiquidityRemoved = 43,
    TradeExecuted = 44,
    PoolCreated = 45,
    SwapExecuted = 46,
    L2PoolCreated = 47,
    L2TradeExecuted = 48,
    DistributionUpdated = 49,
    ContinuousMarketResolved = 110,
    AMMTypeConverted = 111,
    
    // Keeper events
    LiquidationExecuted = 50,
    StopLossExecuted = 51,
    WorkAssigned = 52,
    PartialLiquidationExecuted = 53,
    KeeperSuspended = 54,
    WorkReassigned = 55,
    KeeperRegistered = 56,
    KeeperSlashed = 57,
    KeeperDeactivated = 58,
    
    // Security events
    CircuitBreakerTriggered = 60,
    CircuitBreakerReset = 61,
    AttackDetected = 62,
    EmergencyShutdownSecurity = 63,
    
    // Resolution events
    MarketResolved = 70,
    DisputeInitiated = 71,
    DisputeResolved = 72,
    MarketCollapsed = 73,
    DisputeDetected = 74,
    
    // Order events
    IcebergOrderPlaced = 80,
    IcebergOrderFilled = 81,
    TWAPOrderPlaced = 82,
    TWAPIntervalExecuted = 83,
    DarkOrderPlaced = 84,
    DarkOrderMatched = 85,
    BlockTradeProposed = 86,
    BlockTradeExecuted = 87,
    
    // State events
    MarketArchived = 90,
    StateCompressed = 91,
    
    // Price events
    PriceUpdated = 100,
    WebSocketHealthAlert = 101,
    
    // Collateral events
    CollateralDeposited = 120,
    CollateralWithdrawn = 121,
    
    // Phase 20 Integration events
    CoordinatorInitialized = 130,
    BootstrapProgress = 131,
    BootstrapStarted = 132,
    BootstrapDeposit = 133,
    BootstrapComplete = 134,
    SystemHealthCheck = 135,
    ComponentHealthUpdate = 136,
    EmergencyShutdownIntegration = 137,
    MarketBatchProcessed = 138,
    VaultBalanceUpdated = 139,
    AutoRecoveryTriggered = 140,
    
    // Market ingestion events
    IngestionInitialized = 141,
    BatchProcessed = 142,
    PriceUpdateProcessed = 143,
    IngestionHalted = 144,
    MarketsIngested = 145,
    MarketDisputed = 146,
    IngestionResumed = 147,
    
    // MMT reward events
    MMTRewardDistributed = 148,
    MMTStake = 160,
    MMTRewardClaim = 161,
    MmtDistribution = 162,
    
    // Vault events
    VaultInitialized = 149,
    VaultViabilityChecked = 150,
    VaultViabilityReached = 151,
    VaultDegraded = 152,
    VaultRecovered = 153,
    VaultNearingViability = 154,
    
    // Vampire attack events
    VampireAttackDetected = 155,
    VampireProtectionReset = 156,
    
    // Credits system events
    RefundProcessed = 157,
    
    // Coverage events
    LeverageUpdated = 182,
    RecoveryModeActivated = 163,
    RecoveryModeDeactivated = 164,
    
    // Missing events
    OracleInitialized = 165,
    MarketHalted = 166,
    MarketResumed = 167,
    BootstrapInitialized = 168,
    BootstrapWithdrawal = 169,
    CoverageUpdated = 170,
    BootstrapCompleted = 171,
    PositionMonitored = 172,
    IntegrationTestCompleted = 173,
    OracleSpreadExceeded = 174,
    OracleSpreadNormalized = 175,
    OracleDivergenceDetected = 176,
    OracleStale = 177,
    CascadeLiquidationDetected = 178,
    CascadeRecovered = 179,
    SystemWideHalt = 180,
    LiquidationHalt = 181,
    
    // Migration events
    MigrationStarted = 190,
    PositionMigrated = 191,
    MigrationCompleted = 192,
    MigrationPaused = 193,
    MigrationResumed = 194,
    
    // Demo mode events
    DemoAccountCreated = 200,
    DemoAccountReset = 201,
    DemoUsdcMinted = 202,
    DemoUsdcTransferred = 203,
    DemoPositionOpened = 204,
    DemoPositionClosed = 205,
    DemoPositionLiquidated = 206,
    DemoSimulation = 207,
    
    // Risk quiz events
    RiskQuizInitialized = 210,
    RiskQuizSubmitted = 211,
    RiskAcknowledged = 212,
    
    // UX events
    HealthAlert = 213,
    
    // Error handling events (Phase 21)
    ChainTransactionBegun = 220,
    ChainTransactionCompleted = 221,
    ChainOperationFailed = 222,
    ChainTransactionRolledBack = 223,
    TransactionPending = 224,
    TransactionCancelled = 225,
    TransactionExecuted = 226,
    TransactionFailed = 227,
    ActionRecorded = 228,
    ActionReverted = 229,
    RecoveryInitiated = 230,
    RecoveryCompleted = 231,
    RecoveryFailed = 232,
    
    // Analytics events
    UserMetricsUpdate = 240,
    PerformanceSnapshot = 241,
    BacktestDisplayed = 242,
    
    Unknown,
}

/// Base event trait
pub trait Event: BorshSerialize {
    fn event_type() -> EventType;
    
    fn emit(&self) {
        msg!("BETTING_PLATFORM_EVENT");
        msg!("TYPE:{:?}", Self::event_type());
        
        // Serialize and log event data
        if let Ok(data) = self.try_to_vec() {
            msg!("DATA:{}", bs58::encode(&data).into_string());
        }
    }
}

/// Macro for easy event definition
#[macro_export]
macro_rules! define_event {
    // New syntax: define_event!(EventName { field: type, ... })
    ($name:ident { $($field:ident: $type:ty),* $(,)? }) => {
        #[derive(::borsh::BorshSerialize, ::borsh::BorshDeserialize, Debug, Clone)]
        pub struct $name {
            $(pub $field: $type,)*
        }
        
        impl $crate::events::Event for $name {
            fn event_type() -> $crate::events::EventType {
                $crate::events::EventType::$name
            }
        }
    };
    
    // Original syntax: define_event!(EventName, EventType::Variant, { field: type, ... })
    ($name:ident, $event_type:expr, { $($field:ident: $type:ty),* $(,)? }) => {
        #[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
        pub struct $name {
            $(pub $field: $type,)*
        }
        
        impl $crate::events::Event for $name {
            fn event_type() -> $crate::events::EventType {
                $event_type
            }
        }
    };
}

// === Core Events ===

define_event!(GenesisEvent, EventType::GenesisEvent, {
    slot: u64,
    epoch: u64,
    season: u64,
});

define_event!(EmergencyHaltEvent, EventType::EmergencyHaltEvent, {
    slot: u64,
    reason: String,
});

// === Trading Events ===

define_event!(PositionOpened, EventType::PositionOpened, {
    user: Pubkey,
    proposal_id: u128,
    outcome: u8,
    size: u64,
    leverage: u64,
    entry_price: u64,
    is_long: bool,
    position_id: [u8; 32],
    chain_id: Option<u128>,
});

define_event!(PositionClosed, EventType::PositionClosed, {
    user: Pubkey,
    position_id: [u8; 32],
    exit_price: u64,
    pnl: i64,
    close_reason: CloseReason,
});

define_event!(PositionLiquidated, EventType::PositionLiquidated, {
    position_id: [u8; 32],
    liquidator: Pubkey,
    liquidation_price: u64,
    amount_liquidated: u64,
    remaining_position: u64,
});

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum CloseReason {
    UserInitiated,
    StopLoss,
    TakeProfit,
    Liquidation,
    MarketResolved,
}

// === Fee Events ===

define_event!(FeesDistributed, EventType::FeesDistributed, {
    fee_amount: u64,
    to_vault: u64,
    to_mmt_stakers: u64,
    to_lps: u64,
    slot: u64,
});

// === Chain Events ===

define_event!(ChainCreated, EventType::ChainCreated, {
    chain_id: u128,
    user: Pubkey,
    verse_id: u128,
    initial_deposit: u64,
    steps: u8,
});

define_event!(ChainStepExecuted, EventType::ChainStepExecuted, {
    chain_id: u128,
    step_index: u8,
    step_type: String,
    position_created: Option<u128>,
    current_balance: u64,
});

define_event!(ChainCompleted, EventType::ChainCompleted, {
    chain_id: u128,
    final_balance: u64,
    total_pnl: i64,
    positions_created: u32,
});

// === AMM Events ===

define_event!(MarketCreated, EventType::MarketCreated, {
    market_id: u128,
    amm_type: String,
    num_outcomes: u8,
    initial_liquidity: u64,
    oracle: Pubkey,
});

define_event!(TradePlaced, EventType::TradePlaced, {
    market_id: u128,
    trader: Pubkey,
    outcome: u8,
    is_buy: bool,
    amount: u64,
    price: u64,
    fee: u64,
});

define_event!(TradeExecuted, EventType::TradeExecuted, {
    market_id: u128,
    trader: Pubkey,
    outcome: u8,
    is_buy: bool,
    shares: u64,
    cost: u64,
    fee: u64,
    new_probabilities: Vec<u64>,
    timestamp: i64,
});

define_event!(PoolCreated, EventType::PoolCreated, {
    pool_id: u128,
    amm_type: String,
    num_outcomes: u8,
    initial_reserves: Vec<u64>,
    initial_lp_supply: u64,
    fee_bps: u16,
});

define_event!(SwapExecuted, EventType::SwapExecuted, {
    pool_id: u128,
    trader: Pubkey,
    outcome_in: u8,
    outcome_out: u8,
    amount_in: u64,
    amount_out: u64,
    fee: u64,
    new_reserves: Vec<u64>,
    new_probabilities: Vec<u64>,
    timestamp: i64,
});

define_event!(LiquidityAdded, EventType::LiquidityAdded, {
    pool_id: u128,
    provider: Pubkey,
    amounts: Vec<u64>,
    lp_tokens_minted: u64,
    new_reserves: Vec<u64>,
    new_lp_supply: u64,
    timestamp: i64,
});

define_event!(LiquidityRemoved, EventType::LiquidityRemoved, {
    pool_id: u128,
    provider: Pubkey,
    lp_tokens_burned: u64,
    amounts_withdrawn: Vec<u64>,
    new_reserves: Vec<u64>,
    new_lp_supply: u64,
    timestamp: i64,
});

define_event!(L2PoolCreated, EventType::L2PoolCreated, {
    pool_id: u128,
    min_value: u64,
    max_value: u64,
    num_bins: u8,
    liquidity_parameter: u64,
    oracle: Pubkey,
});

define_event!(L2TradeExecuted, EventType::L2TradeExecuted, {
    pool_id: u128,
    trader: Pubkey,
    lower_bound: u64,
    upper_bound: u64,
    shares: u64,
    is_buy: bool,
    cost: u64,
    fee: u64,
    new_expected_value: u64,
    new_l2_norm: u64,
    timestamp: i64,
});

define_event!(DistributionUpdated, EventType::DistributionUpdated, {
    pool_id: u128,
    new_expected_value: u64,
    new_variance: u64,
    confidence_interval: (u64, u64),
    timestamp: i64,
});

define_event!(ContinuousMarketResolved, EventType::ContinuousMarketResolved, {
    pool_id: u128,
    outcome_value: u64,
    winning_bin: u8,
    winning_range: (u64, u64),
    payout_per_share: u64,
    total_pool_value: u64,
    timestamp: i64,
});

define_event!(AMMTypeConverted, EventType::AMMTypeConverted, {
    market_id: u128,
    from_type: u8,
    to_type: u8,
    timestamp: i64,
    liquidity_preserved: u64,
});

// === Keeper Events ===

define_event!(LiquidationExecuted, EventType::LiquidationExecuted, {
    position_id: [u8; 32],
    keeper_id: [u8; 32],
    amount_liquidated: u64,
    keeper_reward: u64,
    risk_score: u8,
    slot: u64,
});

define_event!(StopLossExecuted, EventType::StopLossExecuted, {
    order_id: [u8; 32],
    keeper_id: [u8; 32],
    trigger_price: u64,
    execution_price: u64,
    keeper_bounty: u64,
    order_type: u8,
});

define_event!(PartialLiquidationExecuted, EventType::PartialLiquidationExecuted, {
    position_id: [u8; 32],
    keeper_id: Pubkey,
    amount_liquidated: u64,
    keeper_reward: u64,
    risk_score: u8,
    slot: u64,
});

define_event!(LiquidationHaltEvent, EventType::LiquidationHalt, {
    reason: String,
    halt_start_slot: u64,
    halt_end_slot: u64,
    liquidation_count: u32,
    liquidation_value: u64,
    coverage_ratio: u64,
    timestamp: i64,
});

define_event!(WorkAssigned, EventType::WorkAssigned, {
    keeper_id: [u8; 32],
    work_type: u8,
    items_count: u32,
    priority: u64,
});

define_event!(KeeperSuspended, EventType::KeeperSuspended, {
    keeper_id: [u8; 32],
    performance_score: u64,
    total_failures: u64,
});

define_event!(KeeperSlashed, EventType::KeeperSlashed, {
    keeper_id: [u8; 32],
    slash_amount: u64,
    evidence_type: u8,
    remaining_stake: u64,
});

// === Security Events ===

define_event!(CircuitBreakerTriggered, EventType::CircuitBreakerTriggered, {
    breaker_type: u8,
    threshold_value: u64,
    actual_value: u64,
    halt_duration: u64,
    triggered_at: i64,
});

define_event!(AttackDetected, EventType::AttackDetected, {
    attack_type: u8,
    suspicious_address: Pubkey,
    market_id: [u8; 32],
    evidence_score: u32,
});

define_event!(CircuitBreakerResetEvent, EventType::CircuitBreakerReset, {
    breaker_type: u8,
    threshold_value: u64,
    actual_value: u64,
    halt_duration: u64,
    triggered_at: i64,
});

// === Resolution Events ===

define_event!(MarketResolved, EventType::MarketResolved, {
    market_id: u128,
    verse_id: u128,
    winning_outcome: u8,
    total_payout: u64,
    resolution_time: i64,
});

define_event!(DisputeInitiated, EventType::DisputeInitiated, {
    market_id: String,
    verse_id: u128,
    disputer: Pubkey,
    dispute_reason: String,
    stake_amount: u64,
});

define_event!(DisputeDetected, EventType::DisputeDetected, {
    market_id: [u8; 16],
    dispute_id: String,
    proposed_outcome: String,
});

define_event!(MarketCollapsed, EventType::MarketCollapsed, {
    proposal_id: [u8; 32],
    winning_outcome: u8,
    probability: u64,
    collapse_type: u8,
    timestamp: i64,
});

// === Order Events ===

define_event!(IcebergOrderPlaced, EventType::IcebergOrderPlaced, {
    order_id: u64,
    user: Pubkey,
    market_id: u128,
    visible_size: u64,
    total_size: u64,
    side: u8,
});

define_event!(TWAPOrderPlaced, EventType::TWAPOrderPlaced, {
    order_id: u64,
    user: Pubkey,
    market_id: u128,
    total_size: u64,
    intervals: u8,
    duration: u64,
});

// === State Events ===

define_event!(MarketArchived, EventType::MarketArchived, {
    proposal_id: [u8; 32],
    ipfs_hash: [u8; 32],
    slot: u64,
});

define_event!(PriceUpdateProcessed, EventType::PriceUpdated, {
    market_id: [u8; 32],
    keeper_id: [u8; 32],
    timestamp: i64,
});

define_event!(WebSocketHealthAlert, EventType::WebSocketHealthAlert, {
    health: u8,
    slots_since_update: u64,
    fallback_active: bool,
});

// === Collateral Events ===

define_event!(CollateralDeposited, EventType::CollateralDeposited, {
    depositor: Pubkey,
    amount: u64,
    total_deposits: u64,
    timestamp: i64,
});

define_event!(CollateralWithdrawn, EventType::CollateralWithdrawn, {
    withdrawer: Pubkey,
    amount: u64,
    total_deposits: u64,
    timestamp: i64,
});

define_event!(WorkReassigned, EventType::WorkReassigned, {
    original_keeper: [u8; 32],
    new_keeper: [u8; 32],
    work_item_id: [u8; 32],
});

define_event!(KeeperRegistered, EventType::KeeperRegistered, {
    keeper_id: [u8; 32],
    authority: Pubkey,
    keeper_type: u8,
    mmt_stake: u64,
    specializations: Vec<u8>,
});

define_event!(KeeperDeactivated, EventType::KeeperDeactivated, {
    keeper_id: [u8; 32],
    reason_code: u8,
});

// === Phase 20 Integration Events ===

define_event!(CoordinatorInitializedEvent, EventType::CoordinatorInitialized, {
    admin: Pubkey,
    components: u32,
});

define_event!(BootstrapProgressEvent, EventType::BootstrapProgress, {
    vault_balance: u64,
    target: u64,
    progress_pct: u64,
});

define_event!(BootstrapStartedEvent, EventType::BootstrapStarted, {
    target_vault: u64,
    incentive_pool: u64,
});

define_event!(BootstrapDepositEvent, EventType::BootstrapDeposit, {
    depositor: Pubkey,
    amount: u64,
    vault_balance: u64,
    mmt_earned: u64,
});

define_event!(BootstrapCompleteEvent, EventType::BootstrapComplete, {
    coverage: u64,
    max_leverage: u64,
});

define_event!(BootstrapCompleteDetailedEvent, EventType::BootstrapComplete, {
    final_vault: u64,
    total_depositors: u32,
    duration_slots: u64,
    mmt_distributed: u64,
});

define_event!(SystemHealthCheckEvent, EventType::SystemHealthCheck, {
    status: u8,
    components_healthy: u32,
    slot: u64,
});

define_event!(ComponentHealthUpdateEvent, EventType::ComponentHealthUpdate, {
    component: String,
    status: u8,
    latency_ms: u32,
});

define_event!(EmergencyShutdownEvent, EventType::EmergencyShutdownIntegration, {
    reason: String,
    admin: Pubkey,
    slot: u64,
});

define_event!(MarketBatchProcessedEvent, EventType::MarketBatchProcessed, {
    count: u32,
    slot: u64,
});

define_event!(VaultBalanceUpdatedEvent, EventType::VaultBalanceUpdated, {
    new_balance: u64,
    delta: i64,
});

define_event!(AutoRecoveryTriggeredEvent, EventType::AutoRecoveryTriggered, {
    components_reset: u32,
});

define_event!(MilestoneReachedEvent, EventType::BootstrapProgress, {
    milestone: u8,
    vault_balance: u64,
});

define_event!(ReferralRewardEvent, EventType::BootstrapProgress, {
    referrer: Pubkey,
    referred: Pubkey,
    reward: u64,
});

define_event!(HealthCheckCompleteEvent, EventType::SystemHealthCheck, {
    status: u8,
    components_healthy: u32,
    slot: u64,
});

define_event!(AutoRecoveryAttemptedEvent, EventType::AutoRecoveryTriggered, {
    components_reset: u32,
});

// === MMT Reward Events ===

define_event!(MMTRewardDistributedEvent, EventType::MMTRewardDistributed, {
    recipient: Pubkey,
    amount: u64,
    distribution_type: u8, // Maps to DistributionType enum
    deposit_amount: u64,
    vault_balance: u64,
});

define_event!(MMTStakeEvent, EventType::MMTStake, {
    staker: Pubkey,
    amount: u64,
    lock_period_days: u32,
    tier: u8, // StakingTier as u8
    timestamp: i64,
});

define_event!(MMTRewardClaimEvent, EventType::MMTRewardClaim, {
    staker: Pubkey,
    amount: u64,
    rewards_type: u8, // 0 = trading rebate, 1 = staking rewards
    timestamp: i64,
});

// === Vault Events ===

define_event!(VaultInitializedEvent, EventType::VaultInitialized, {
    vault: Pubkey,
    initial_balance: u64,
    bootstrap_phase: bool,
    minimum_viable_size: u64,
    authority: Pubkey,
});

define_event!(VaultViabilityCheckedEvent, EventType::VaultViabilityChecked, {
    state: u8,
    current_balance: u64,
    minimum_required: u64,
    enabled_features_bitmap: u8, // Simplified for event
    timestamp: i64,
});

define_event!(VaultViabilityReachedEvent, EventType::VaultViabilityReached, {
    balance: u64,
    timestamp: i64,
    bootstrap_duration_slots: u64,
});

define_event!(VaultDegradedEvent, EventType::VaultDegraded, {
    balance: u64,
    minimum_required: u64,
    degradation_count: u32,
    timestamp: i64,
});

define_event!(VaultRecoveredEvent, EventType::VaultRecovered, {
    balance: u64,
    timestamp: i64,
});

define_event!(VaultNearingViabilityEvent, EventType::VaultNearingViability, {
    balance: u64,
    target: u64,
    percent_complete: u64,
});

// === Vampire Attack Events ===

define_event!(VampireAttackDetectedEvent, EventType::VampireAttackDetected, {
    attacker: Pubkey,
    attack_type: u8,
    amount: u64,
    coverage_ratio: u64,
    slot: u64,
});

define_event!(VampireProtectionResetEvent, EventType::VampireProtectionReset, {
    admin: Pubkey,
    timestamp: i64,
});

// === Credits System Events ===

define_event!(RefundProcessed, EventType::RefundProcessed, {
    user: Pubkey,
    proposal_id: u128,
    verse_id: u128,
    refund_amount: u64,
    timestamp: i64,
    refund_type: u8,
});

// === Oracle Events ===

define_event!(OracleInitializedEvent, EventType::OracleInitialized, {
    oracle_type: String,
    admin: Pubkey,
    timestamp: i64,
});

define_event!(OracleSpreadExceededEvent, EventType::OracleSpreadExceeded, {
    market_id: u128,
    spread_bps: u16,
    max_allowed_bps: u16,
    timestamp: i64,
});

define_event!(OracleSpreadNormalizedEvent, EventType::OracleSpreadNormalized, {
    market_id: u128,
    new_spread_bps: u16,
    timestamp: i64,
});

define_event!(OracleDivergenceDetectedEvent, EventType::OracleDivergenceDetected, {
    market_id: u128,
    max_divergence_bps: u16,
    resolution: String,
    timestamp: i64,
});

define_event!(OracleStaleEvent, EventType::OracleStale, {
    market_id: u128,
    staleness_seconds: i64,
    max_allowed_seconds: i64,
    timestamp: i64,
});

define_event!(MarketHaltedEvent, EventType::MarketHalted, {
    market_id: u128,
    reason: String,
    timestamp: i64,
});

define_event!(MarketResumedEvent, EventType::MarketResumed, {
    market_id: u128,
    timestamp: i64,
});

// === Bootstrap Events ===

define_event!(BootstrapInitializedEvent, EventType::BootstrapInitialized, {
    market_id: u128,
    target_amount: u64,
    deadline: i64,
});

define_event!(BootstrapWithdrawalEvent, EventType::BootstrapWithdrawal, {
    market_id: u128,
    user: Pubkey,
    amount: u64,
    timestamp: i64,
});

define_event!(CoverageUpdatedEvent, EventType::CoverageUpdated, {
    market_id: u128,
    coverage_ratio: u64,
    timestamp: i64,
});

define_event!(RecoveryModeActivatedEvent, EventType::RecoveryModeActivated, {
    market_id: u128,
    coverage_before: u64,
    min_coverage: u64,
    timestamp: i64,
});

define_event!(RecoveryModeDeactivatedEvent, EventType::RecoveryModeDeactivated, {
    market_id: u128,
    coverage_after: u64,
    timestamp: i64,
});

define_event!(BootstrapCompletedEvent, EventType::BootstrapCompleted, {
    market_id: u128,
    total_raised: u64,
    timestamp: i64,
});

// === Position Events ===

define_event!(PositionMonitoredEvent, EventType::PositionMonitored, {
    position_id: [u8; 32],
    health_factor: u64,
    timestamp: i64,
});

// === Test Events ===

define_event!(IntegrationTestCompletedEvent, EventType::IntegrationTestCompleted, {
    test_name: String,
    modules: Vec<String>,
    success: bool,
    details: String,
    timestamp: i64,
});

// === Cascade Liquidation Events ===

define_event!(CascadeLiquidationDetectedEvent, EventType::CascadeLiquidationDetected, {
    market_id: [u8; 32],
    initial_liquidations: u32,
    cascade_liquidations: u32,
    total_liquidation_rate_bps: u16,
    circuit_breaker_activated: bool,
    timestamp: i64,
});

define_event!(CascadeRecoveredEvent, EventType::CascadeRecovered, {
    market_id: [u8; 32],
    recovery_price: u64,
    remaining_at_risk: u32,
    duration_slots: u64,
    timestamp: i64,
});

define_event!(SystemWideHaltEvent, EventType::SystemWideHalt, {
    trigger_market: String,
    initial_shock_bps: i16,
    affected_markets: u32,
    system_impact_bps: i16,
    timestamp: i64,
});

// === Block Trading Events ===

define_event!(BlockTradeProposedEvent, EventType::BlockTradeProposed, {
    trade_id: [u8; 32],
    initiator: Pubkey,
    counterparty: Pubkey,
    size: u64,
    initial_price: u64,
});

define_event!(BlockTradeExecutedEvent, EventType::BlockTradeExecuted, {
    trade_id: [u8; 32],
    buyer: Pubkey,
    seller: Pubkey,
    size: u64,
    price: u64,
});

/// Helper function to emit events without the trait
pub fn emit_event<T: BorshSerialize>(event_type: EventType, event_data: &T) {
    msg!("BETTING_PLATFORM_EVENT");
    msg!("TYPE:{:?}", event_type);
    
    if let Ok(data) = event_data.try_to_vec() {
        msg!("DATA:{}", bs58::encode(&data).into_string());
    }
}

/// Parse event from log
pub fn parse_event(log: &str) -> Option<(EventType, Vec<u8>)> {
    if !log.starts_with("Program log: BETTING_PLATFORM_EVENT") {
        return None;
    }
    
    // Parse event type and data from subsequent log entries
    // This would be implemented by an indexer or client
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_serialization() {
        let event = PositionOpened {
            user: Pubkey::new_unique(),
            proposal_id: 12345,
            outcome: 1,
            size: 1000,
            leverage: 5,
            entry_price: 50000,
            is_long: true,
            position_id: [67u8; 32],
            chain_id: Some(11111),
        };
        
        let serialized = event.try_to_vec().unwrap();
        assert!(!serialized.is_empty());
        
        let deserialized: PositionOpened = BorshDeserialize::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.proposal_id, event.proposal_id);
    }
}

// Type aliases for compatibility
pub type TradeExecutedEvent = TradeExecuted;
pub type LiquidationEvent = LiquidationExecuted;
pub type ChainPositionEvent = ChainStepExecuted;