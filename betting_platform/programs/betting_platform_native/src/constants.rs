//! Global constants for the betting platform
//!
//! Central location for all platform-wide constants

/// Price precision (10000 = 100%)
pub const PRICE_PRECISION: u64 = 10_000;

/// Maximum price value
pub const MAX_PRICE: u64 = 10_000_000_000; // $10k with 6 decimals

/// Minimum B value for AMMs
pub const MIN_B_VALUE: u64 = 1_000; // 0.001 with 6 decimals

/// Maximum B value for AMMs
pub const MAX_B_VALUE: u64 = 1_000_000_000; // 1000 with 6 decimals

/// Maximum liquidation penalty in basis points
pub const MAX_LIQUIDATION_PENALTY_BPS: u16 = 1000; // 10%

/// Slot duration in milliseconds
pub const SLOT_DURATION: u64 = 400;

/// Price clamp per slot in basis points
pub const PRICE_CLAMP_PER_SLOT_BPS: u64 = 200; // 2%

/// Maximum oracle staleness in seconds
pub const MAX_ORACLE_STALENESS: i64 = 300; // 5 minutes

/// Bootstrap phase constants
pub const BOOTSTRAP_TARGET_VAULT: u64 = 100_000_000_000; // $100k with 6 decimals
pub const BOOTSTRAP_FEE_BPS: u16 = 28; // 0.28%
pub const BOOTSTRAP_MMT_MULTIPLIER: u64 = 2; // 2x rewards

/// MMT decimals
pub const MMT_DECIMALS: u8 = 9;

/// Staking constants
pub const STAKING_REBATE_BASIS_POINTS: u16 = 1500; // 15%
pub const LOCK_MULTIPLIER_30_DAYS: u16 = 12500; // 1.25x
pub const LOCK_MULTIPLIER_90_DAYS: u16 = 15000; // 1.5x

/// Position discriminator
pub const POSITION_DISCRIMINATOR: [u8; 8] = [189, 45, 122, 98, 201, 167, 43, 90];

/// Collateral decimals (USDC = 6)
pub const COLLATERAL_DECIMALS: u8 = 6;

/// Leverage constants
pub const MAX_LEVERAGE: u16 = 500; // 500x maximum leverage
pub const MAX_LEVERAGE_NO_QUIZ: u8 = 10; // Maximum leverage without quiz

/// ===== FUSED LEVERAGE SYSTEM CONSTANTS =====
/// Oracle-Vault-Fused Perpetual CDP with Cascade-Resilient Leverage

/// Core leverage parameters
pub const BASE_LEVERAGE: u64 = 100; // Start at 100x base leverage
pub const MAX_FUSED_LEVERAGE: u64 = 100; // Initially 100x, will scale to 1000x later
pub const LEVERAGE_CAP_HARD: u64 = 1000; // Absolute maximum leverage cap

/// Scalar calculation parameters
pub const CAP_FUSED: f64 = 20.0; // Fused scalar cap
pub const CAP_VAULT: f64 = 30.0; // Vault premium cap
pub const BASE_RISK: f64 = 0.25; // Base risk for vault calculations

/// Volatility and risk parameters
pub const VOL_SPIKE_THRESHOLD: f64 = 0.5; // Volatility spike threshold for liquidations
pub const DEV_THRESHOLD: f64 = 0.1; // Price deviation threshold for cascade protection
pub const BUFF_CAP: f64 = 1.5; // Buffer cap multiplier for over-collateralization

/// Funding rate parameters
pub const FUND_SLOPE: f64 = 500.0; // Funding rate slope for buffer adjustments
pub const FUNDING_CAP: f64 = 0.5; // Fixed funding cap for perpetual positions
pub const FUNDING_BASE_RATE: f64 = 50.0; // Base funding rate in bp/8h

/// CDP parameters
pub const COLL_CAP: f64 = 2.0; // Fixed optimal collateral cap
pub const CDP_LIQUIDATION_THRESHOLD: f64 = 1.5; // 150% collateralization ratio

/// Oracle parameters
pub const MIN_ORACLE_CONFIDENCE: f64 = 0.95; // Minimum confidence for oracle data
pub const MAX_ORACLE_STALENESS_SLOTS: u64 = 32; // Maximum staleness for oracle updates
pub const TWAP_DEVIATION_MAX: f64 = 0.02; // Maximum TWAP deviation (2%)

/// Liquidation parameters
pub const LIQ_CAP_MIN: f64 = 0.02; // Minimum liquidation cap (2% of OI)
pub const LIQ_CAP_MAX: f64 = 0.08; // Maximum liquidation cap (8% of OI)
pub const SIGMA_FACTOR: f64 = 0.1; // Sigma factor for liquidation calculations
pub const PARTIAL_LIQ_MAX_PERCENT: f64 = 0.2; // Maximum 20% partial liquidation per epoch

/// Vault parameters
pub const VAULT_MAX_UTILIZATION: f64 = 0.8; // Maximum vault utilization (80%)
pub const VAULT_WITHDRAWAL_CAP: f64 = 0.1; // Maximum 10% withdrawal per slot
pub const VAULT_APR_BASE: f64 = 0.04; // Base APR (4%)
pub const VAULT_APR_SLOPE: f64 = 0.16; // APR slope based on risk (16%)

/// Probability bounds
pub const PROB_MIN_CLAMP: f64 = 0.01; // Minimum probability clamp
pub const PROB_MAX_CLAMP: f64 = 0.99; // Maximum probability clamp
pub const PROB_JUMP_MAX: f64 = 0.2; // Maximum probability jump for early resolution

/// Governance parameters
pub const GOVERNANCE_TIMELOCK: u64 = 604800; // 7 days in seconds
pub const PARAMETER_UPDATE_COOLDOWN: u64 = 7776000; // 90 days (quarterly updates)

/// Migration parameters
pub const MIGRATION_GRACE_PERIOD: u64 = 2592000; // 30 days for migration
pub const LEGACY_MODE_ENABLED: bool = true; // Run legacy in parallel initially

/// Flash betting specific
pub const FLASH_UNWRAP_MAX_SLOTS: u64 = 1; // Maximum slots for instant unwrap
pub const SPORTS_EVENT_MAX_DURATION: u64 = 14400; // 4 hours maximum for sports events

/// Risk limits
pub const MAX_POSITION_SIZE_PERCENT: f64 = 0.05; // Maximum 5% of OI/vault per position
pub const MIN_POSITION_SIZE: u64 = 100_000; // Minimum $0.10 with 6 decimals

/// Circuit breaker thresholds
pub const HALT_PRICE_MOVE_PERCENT: f64 = 0.05; // Halt on 5% price move
pub const HALT_PRICE_MOVE_SLOTS: u64 = 4; // Within 4 slots (1.6 seconds)
pub const HALT_COVERAGE_MIN: f64 = 0.5; // Halt if coverage drops below 50%
pub const HALT_DURATION_SLOTS: u64 = 9000; // Halt for 1 hour (1h = 9000 slots)
pub const MAX_CHAIN_LEVERAGE: u16 = 500; // Maximum effective leverage for chains

/// Liquidation constants
pub const PARTIAL_LIQUIDATION_BPS: u16 = 800; // 8% per slot
pub const MAX_DRAWDOWN_BPS: i32 = -29700; // -297% maximum drawdown handling

/// Fee constants
pub const BASE_FEE_BPS: u16 = 28; // Fixed 28 basis points base fee
pub const POLYMARKET_FEE_BPS: u16 = 150; // 1.5% Polymarket fee

/// Risk metrics constants
pub const TARGET_WIN_RATE_BPS: u16 = 7800; // 78% target win rate
pub const HIGH_RISK_THRESHOLD: u8 = 70; // Risk score threshold for warnings
pub const WIN_LOSS_RATIO_TARGET: u64 = 150; // 1.5:1 win/loss ratio target (scaled by 100)

/// Other constants
pub const BASIS_POINTS_DIVISOR: u64 = 10_000;
pub const LEVERAGE_PRECISION: u64 = 100; // 100 = 1x leverage