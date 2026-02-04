//! Instruction definitions for the betting platform
//! 
//! Complete enumeration of all 49 instruction handlers with their parameters

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use crate::math::tables::TableValues;
use crate::error_handling::atomic_rollback::ChainOperation;

/// Main instruction enum containing all 49 operations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum BettingPlatformInstruction {
    // === Core Instructions (4) ===
    
    /// Initialize the global configuration
    Initialize {
        seed: u128,
    },
    
    /// Initialize genesis parameters
    InitializeGenesis,
    
    /// Initialize MMT token
    InitializeMmt,
    
    /// Atomic genesis initialization
    GenesisAtomic,
    
    /// Emergency halt (only within 100 slots of genesis)
    EmergencyHalt,
    
    // === Trading Instructions (2) ===
    
    /// Open a new position
    OpenPosition {
        params: OpenPositionParams,
    },
    
    /// Close an existing position
    ClosePosition {
        position_index: u8,
    },
    
    /// Create a new prediction market
    CreateMarket {
        params: CreateMarketParams,
    },
    
    /// Create a new verse
    CreateVerse {
        params: CreateVerseParams,
    },
    
    // === Fee & Liquidation Instructions (2) ===
    
    /// Distribute fees
    DistributeFees {
        fee_amount: u64,
    },
    
    /// Partial liquidation of a position
    PartialLiquidate {
        position_index: u8,
    },
    
    // === Chain Instructions (2) ===
    
    /// Execute auto-chain
    AutoChain {
        verse_id: u128,
        deposit: u64,
        steps: Vec<ChainStepType>,
    },
    
    /// Unwind a chain
    UnwindChain {
        chain_id: u128,
    },
    
    // === Safety Instructions (2) ===
    
    /// Check circuit breakers
    CheckCircuitBreakers {
        price_movement: i64,
    },
    
    /// Monitor position health
    MonitorPositionHealth,
    
    // === AMM Instructions (8) ===
    
    /// Initialize LMSR market
    InitializeLmsrMarket {
        market_id: u128,
        b_parameter: u64,
        num_outcomes: u8,
    },
    
    /// Execute LMSR trade
    ExecuteLmsrTrade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
    
    /// Initialize PM-AMM market
    InitializePmammMarket {
        market_id: u128,
        l_parameter: u64,
        expiry_time: i64,
        initial_price: u64,
    },
    
    /// Execute PM-AMM trade
    ExecutePmammTrade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
    
    /// Initialize L2 AMM market
    InitializeL2AmmMarket {
        market_id: u128,
        k_parameter: u64,
        b_bound: u64,
        distribution_type: DistributionType,
        discretization_points: u16,
        range_min: u64,
        range_max: u64,
    },
    
    /// Execute L2 trade
    ExecuteL2Trade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
    
    /// Update L2 distribution weights
    UpdateDistribution {
        distribution_bins: Vec<(u8, u64)>, // (outcome, weight)
    },
    
    /// Resolve continuous L2 market
    ResolveContinuous {
        winning_value: u64,
        oracle_signature: [u8; 64],
    },
    
    /// Claim winnings from continuous L2 market
    ClaimContinuous {
        position_id: [u8; 32],
    },
    
    /// Initialize hybrid AMM
    InitializeHybridAmm {
        market_id: u128,
        amm_type: AMMType,
        num_outcomes: u8,
        expiry_time: i64,
        is_continuous: bool,
        amm_specific_data: Vec<u8>,
    },
    
    /// Execute hybrid trade
    ExecuteHybridTrade {
        outcome: u8,
        amount: u64,
        is_buy: bool,
    },
    
    // === Advanced Trading Instructions (6) ===
    
    /// Place iceberg order
    PlaceIcebergOrder {
        market_id: u128,
        outcome: u8,
        visible_size: u64,
        total_size: u64,
        side: OrderSide,
    },
    
    /// Execute iceberg fill
    ExecuteIcebergFill {
        fill_size: u64,
    },
    
    /// Place TWAP order
    PlaceTwapOrder {
        market_id: u128,
        outcome: u8,
        total_size: u64,
        duration: u64,
        intervals: u8,
        side: OrderSide,
    },
    
    /// Execute TWAP interval
    ExecuteTwapInterval,
    
    /// Initialize dark pool
    InitializeDarkPool {
        market_id: u128,
        minimum_size: u64,
        price_improvement_bps: u16,
    },
    
    /// Place dark order
    PlaceDarkOrder {
        side: OrderSide,
        outcome: u8,
        size: u64,
        min_price: Option<u64>,
        max_price: Option<u64>,
        time_in_force: TimeInForce,
    },
    
    // === Security Instructions (8) ===
    
    /// Initialize attack detector
    InitializeAttackDetector,
    
    /// Process trade for security
    ProcessTradeSecurity {
        market_id: [u8; 32],
        size: u64,
        price: u64,
        leverage: u64,
        is_buy: bool,
    },
    
    /// Update volume baseline
    UpdateVolumeBaseline {
        new_avg_volume: u64,
        new_std_dev: u64,
    },
    
    /// Reset attack detector
    ResetAttackDetector,
    
    /// Initialize circuit breaker
    InitializeCircuitBreaker,
    
    /// Check advanced breakers
    CheckAdvancedBreakers {
        coverage: u64,
        liquidation_count: u64,
        liquidation_volume: u64,
        total_oi: u64,
        failed_tx: u64,
        oi_rate_per_slot: u64,
    },
    
    /// Emergency shutdown
    EmergencyShutdown,
    
    /// Update breaker config
    UpdateBreakerConfig {
        new_cooldown_period: Option<u64>,
        new_coverage_halt_duration: Option<u64>,
        new_price_halt_duration: Option<u64>,
        new_volume_halt_duration: Option<u64>,
        new_liquidation_halt_duration: Option<u64>,
        new_congestion_halt_duration: Option<u64>,
        new_oi_rate_halt_duration: Option<u64>,
    },
    
    // === Liquidation Queue Instructions (4) ===
    
    /// Initialize liquidation queue
    InitializeLiquidationQueue,
    
    /// Update at-risk position
    UpdateAtRiskPosition {
        mark_price: u64,
    },
    
    /// Process priority liquidation
    ProcessPriorityLiquidation {
        max_liquidations: u64,
    },
    
    /// Claim keeper rewards
    ClaimKeeperRewards,
    
    // === Keeper & Resolution Instructions (8) ===
    
    /// Initialize price cache
    InitializePriceCache {
        verse_id: u128,
    },
    
    /// Update price cache
    UpdatePriceCache {
        verse_id: u128,
        new_price: u64,
    },
    
    /// Process resolution
    ProcessResolution {
        verse_id: u128,
        market_id: String,
        resolution_outcome: String,
    },
    
    /// Initiate dispute
    InitiateDispute {
        verse_id: u128,
        market_id: String,
    },
    
    /// Resolve dispute
    ResolveDispute {
        verse_id: u128,
        market_id: String,
        final_resolution: String,
    },
    
    /// Mirror dispute
    MirrorDispute {
        market_id: String,
        disputed: bool,
    },
    
    /// Initialize keeper health
    InitializeKeeperHealth,
    
    /// Report keeper metrics
    ReportKeeperMetrics {
        markets_processed: u64,
        errors: u64,
        avg_latency: u64,
    },
    
    /// Initialize performance metrics
    InitializePerformanceMetrics,
    
    /// Update performance metrics
    UpdatePerformanceMetrics {
        request_count: u64,
        success_count: u64,
        fail_count: u64,
        latencies: Vec<u64>,
    },
    
    // === MMT Token Instructions ===
    
    /// Initialize the MMT token system
    InitializeMMTToken,
    
    /// Lock the reserved vault permanently
    LockReservedVault,
    
    /// Initialize the staking pool
    InitializeStakingPool,
    
    /// Stake MMT tokens
    StakeMMT {
        amount: u64,
        lock_period_slots: Option<u64>,
    },
    
    /// Unstake MMT tokens
    UnstakeMMT {
        amount: u64,
    },
    
    /// Distribute trading fees to stakers
    DistributeTradingFees {
        total_fees: u64,
    },
    
    /// Initialize a maker account
    InitializeMakerAccount,
    
    /// Record a maker trade and calculate rewards
    RecordMakerTrade {
        notional: u64,
        spread_improvement_bp: u16,
    },
    
    /// Claim accumulated maker rewards
    ClaimMakerRewards,
    
    /// Distribute MMT tokens from treasury
    DistributeEmission {
        distribution_type: u8, // Will be converted to DistributionType
        amount: u64,
        distribution_id: u64,
    },
    
    /// Transition to the next season
    TransitionSeason,
    
    /// Initialize early trader registry for a season
    InitializeEarlyTraderRegistry {
        season: u8,
    },
    
    /// Register a trader as an early trader
    RegisterEarlyTrader {
        season: u8,
    },
    
    /// Update treasury balance
    UpdateTreasuryBalance,
    
    /// Initialize all MMT PDAs
    InitializeMMTPDAs,
    
    /// Create vesting schedule for 90M reserved tokens
    CreateVestingSchedule {
        schedule_type: VestingScheduleType,
        beneficiary: Pubkey,
        allocation: u64,
    },
    
    /// Claim vested MMT tokens
    ClaimVested,
    
    // === CDF/PDF Table Instructions ===
    
    /// Initialize normal distribution tables
    InitializeNormalTables,
    
    /// Populate normal distribution tables chunk
    PopulateTablesChunk {
        start_index: usize,
        values: Vec<TableValues>,
    },
    
    // === Polymarket Sole Oracle Instructions ===
    
    /// Initialize Polymarket as sole oracle
    InitializePolymarketSoleOracle {
        authority: Pubkey,
    },
    
    /// Update price from Polymarket
    UpdatePolymarketPrice {
        market_id: [u8; 16],
        yes_price: u64,
        no_price: u64,
        volume_24h: u64,
        liquidity: u64,
        timestamp: i64,
        slot: u64,
        signature: [u8; 64],
    },
    
    /// Check and handle price spread
    CheckPriceSpread {
        market_id: [u8; 16],
    },
    
    /// Reset oracle halt status
    ResetOracleHalt {
        market_id: [u8; 16],
    },
    
    // === Bootstrap Phase Instructions ===
    
    /// Initialize enhanced bootstrap coordinator
    InitializeBootstrapPhase {
        mmt_allocation: u64,
    },
    
    /// Process bootstrap deposit with MMT rewards
    ProcessBootstrapDeposit {
        amount: u64,
    },
    
    /// Process bootstrap withdrawal with vampire check
    ProcessBootstrapWithdrawal {
        amount: u64,
    },
    
    /// Update bootstrap coverage ratio
    UpdateBootstrapCoverage,
    
    /// Complete bootstrap phase
    CompleteBootstrap,
    
    /// Check vampire attack conditions
    CheckVampireAttack {
        withdrawal_amount: u64,
    },
    
    /// Halt market due to excessive spread
    HaltMarketDueToSpread {
        market_id: [u8; 16],
    },
    
    /// Unhalt market after resolution
    UnhaltMarket {
        market_id: [u8; 16],
    },
    
    // === Migration Instructions ===
    
    /// Plan a migration to a new version
    PlanMigration {
        target_version: u32,
    },
    
    /// Migrate a batch of accounts
    MigrateBatch {
        batch_accounts: Vec<Pubkey>,
    },
    
    /// Verify migration completion
    VerifyMigration,
    
    /// Pause migration (emergency)
    PauseMigration,
    
    // === Extended Migration Instructions (60-day Parallel Deployment) ===
    
    /// Initialize parallel migration for 60-day transition
    InitializeParallelMigration {
        new_program_id: Pubkey,
    },
    
    /// Migrate a position with double MMT incentives
    MigratePositionWithIncentives {
        position_id: [u8; 32],
    },
    
    /// Complete migration after 60-day period
    CompleteMigration,
    
    /// Pause extended migration
    PauseExtendedMigration {
        reason: String,
    },
    
    /// Resume extended migration
    ResumeExtendedMigration,
    
    /// Get migration status
    GetMigrationStatus,
    
    // === Liquidation Halt Instructions ===
    
    /// Initialize liquidation halt state
    InitializeLiquidationHaltState {
        override_authority: Pubkey,
    },
    
    /// Override liquidation halt
    OverrideLiquidationHalt {
        force_resume: bool,
    },
    
    // === Funding Rate Instructions ===
    
    /// Update funding rate for a market
    UpdateFundingRate {
        market_id: [u8; 32],
    },
    
    /// Settle funding payment for a position
    SettlePositionFunding {
        position_id: [u8; 32],
    },
    
    /// Halt market and set funding rate to +1.25%/hour
    HaltMarketWithFunding {
        market_id: [u8; 32],
        reason: String,
    },
    
    /// Resume market from halt
    ResumeMarketFromHalt {
        market_id: [u8; 32],
    },
    
    // === Demo Mode Instructions ===
    
    /// Initialize demo account for a user
    InitializeDemoAccount,
    
    /// Reset demo account balance and stats
    ResetDemoAccount,
    
    /// Mint fake USDC to demo account
    MintDemoUsdc {
        amount: u64,
    },
    
    /// Transfer fake USDC between demo accounts
    TransferDemoUsdc {
        amount: u64,
    },
    
    /// Open a demo position
    OpenDemoPosition {
        size: u64,
        leverage: u8,
        is_long: bool,
    },
    
    /// Close a demo position
    CloseDemoPosition {
        position_id: u128,
    },
    
    /// Update demo position prices (keeper operation)
    UpdateDemoPositions,
    
    // === Risk Quiz Instructions ===
    
    /// Initialize risk quiz state for a user
    InitializeRiskQuiz,
    
    /// Submit quiz answers
    SubmitRiskQuizAnswers {
        answers: Vec<u8>,
    },
    
    /// Acknowledge risk disclosure
    AcknowledgeRiskDisclosure {
        risk_hash: [u8; 32],
    },
    
    // === Error Handling & Recovery Instructions ===
    
    /// Begin atomic chain transaction
    BeginChainTransaction {
        chain_id: u128,
        operations: Vec<ChainOperation>,
    },
    
    /// Execute next operation in chain transaction
    ExecuteChainOperation {
        transaction_id: [u8; 32],
    },
    
    /// Rollback failed chain transaction
    RollbackChainTransaction {
        transaction_id: [u8; 32],
    },
    
    /// Submit transaction with undo window
    SubmitWithUndoWindow {
        transaction_type: TransactionType,
        transaction_data: Vec<u8>,
    },
    
    /// Cancel pending transaction
    CancelPendingTransaction {
        transaction_id: [u8; 32],
    },
    
    /// Execute pending transaction after window
    ExecutePendingTransaction {
        transaction_id: [u8; 32],
    },
    
    /// Record revertible action
    RecordRevertibleAction {
        action: RevertibleAction,
        state_snapshot: Vec<u8>,
    },
    
    /// Revert action within same slot
    RevertAction {
        action_id: [u8; 32],
    },
    
    /// Initiate recovery operation
    InitiateRecovery {
        recovery_type: RecoveryType,
        related_id: [u8; 32],
    },
    
    /// Execute recovery operation
    ExecuteRecovery {
        operation_id: [u8; 32],
    },
    
    // === Pre-launch Airdrop Instructions ===
    
    /// Initialize pre-launch airdrop system for influencers (0.1% MMT allocation)
    InitializePreLaunchAirdrop {
        claim_start_slot: u64,
        claim_end_slot: u64,
    },
    
    /// Register an influencer for the airdrop
    RegisterInfluencer {
        social_handle: String,
        platform: u8,  // 1=Twitter, 2=YouTube, 3=TikTok
        follower_count: u64,
    },
    
    /// Claim pre-launch airdrop allocation
    ClaimPreLaunchAirdrop,
    
    /// End pre-launch airdrop period (admin only)
    EndPreLaunchAirdrop,
}

// === Supporting Types ===

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct OpenPositionParams {
    pub proposal_id: u128,
    pub outcome: u8,
    pub leverage: u8,
    pub size: u64,
    pub max_loss: u64,
    pub chain_id: Option<u128>,
}

/// Parameters for creating a new market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CreateMarketParams {
    pub market_id: u128,
    pub verse_id: u128,
    pub title: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub amm_type: AMMType,
    pub settle_time: i64,
    pub oracle_authority: Option<Pubkey>,
    pub initial_liquidity: u64,
    pub b_parameter: Option<u64>,  // For LMSR
    pub l_parameter: Option<u64>,  // For PM-AMM
}

/// Parameters for creating a new verse
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CreateVerseParams {
    pub verse_id: u128,
    pub parent_id: Option<u128>,
    pub title: String,
    pub description: String,
    pub verse_type: u8,  // 0=Root, 1=Category, 2=SubCategory, 3=Market
    pub risk_tier: u8,   // 0-4 risk levels
    pub fee_multiplier: u64,
}

/// Parameters for trade instructions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TradeParams {
    pub market_id: u128,
    pub outcome: u8,
    pub is_buy: bool,
    pub amount: u64,
    pub shares: Option<u64>,
    pub max_cost: Option<u64>,
    pub min_shares: Option<u64>,
    pub min_payout: Option<u64>,
    pub max_slippage_bps: Option<u16>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ChainStepType {
    Long { outcome: u8, leverage: u8 },
    Short { outcome: u8, leverage: u8 },
    Lend { amount: u64 },
    Borrow { amount: u64 },
    Liquidity { amount: u64 },
    Stake { amount: u64 },
    ClosePosition,
    TakeProfit { threshold: u64 },
    StopLoss { threshold: u64 },
}

impl ChainStepType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(ChainStepType::Long { outcome: 0, leverage: 1 }),
            1 => Some(ChainStepType::Short { outcome: 0, leverage: 1 }),
            2 => Some(ChainStepType::Lend { amount: 0 }),
            3 => Some(ChainStepType::Borrow { amount: 0 }),
            4 => Some(ChainStepType::Liquidity { amount: 0 }),
            5 => Some(ChainStepType::Stake { amount: 0 }),
            6 => Some(ChainStepType::ClosePosition),
            7 => Some(ChainStepType::TakeProfit { threshold: 0 }),
            8 => Some(ChainStepType::StopLoss { threshold: 0 }),
            _ => None,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum DistributionType {
    Normal,
    LogNormal,
    Custom,
}

// Import types from state module
use crate::state::accounts::AMMType;
use crate::mmt::vesting::VestingScheduleType;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum TimeInForce {
    ImmediateOrCancel,
    FillOrKill,
    Session,
}

// === Error Handling Types ===

use crate::error_handling::{
    TransactionType,
    RevertibleAction,
    RecoveryType,
};