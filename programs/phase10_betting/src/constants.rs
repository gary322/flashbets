use anchor_lang::prelude::*;

// Seeds
pub const BOOTSTRAP_STATE_SEED: &[u8] = b"bootstrap_state";
pub const BOOTSTRAP_TRADER_SEED: &[u8] = b"bootstrap_trader";
pub const BOOTSTRAP_MILESTONE_SEED: &[u8] = b"bootstrap_milestone";
pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";
pub const AMM_SELECTOR_SEED: &[u8] = b"amm_selector";
pub const SYNTHETIC_ROUTER_SEED: &[u8] = b"synthetic_router";
pub const USER_POSITION_SEED: &[u8] = b"user_position";

// Bootstrap Constants
pub const BOOTSTRAP_MMT_ALLOCATION: u64 = 2_000_000 * 10u64.pow(6); // 2M MMT
pub const SEASON_MMT_ALLOCATION: u64 = 10_000_000 * 10u64.pow(6); // 10M MMT
pub const MAX_EARLY_TRADERS: u64 = 100;
pub const MIN_TRADE_SIZE: u64 = 10 * 10u64.pow(6); // $10 USDC
pub const BOOTSTRAP_MAX_FEE_BPS: u16 = 28; // 0.28%
pub const BOOTSTRAP_MIN_FEE_BPS: u16 = 3; // 0.03%
pub const BOOTSTRAP_DURATION_SLOTS: u64 = 38_880_000; // ~6 months
pub const EARLY_TRADER_MULTIPLIER: u64 = 2; // 2x rewards

// AMM Constants
pub const SLOTS_PER_DAY: u64 = 86_400;
pub const MAX_CHILD_MARKETS: usize = 50;
pub const DEFAULT_SLIPPAGE_TOLERANCE_BPS: u16 = 100; // 1%
pub const MAX_SLIPPAGE_BPS: u16 = 1000; // 10%

// Router Constants
pub const MIN_LIQUIDITY_DEPTH: u64 = 1000 * 10u64.pow(6); // $1000
pub const LIQUIDITY_WEIGHT_BPS: u16 = 7000; // 70%
pub const VOLUME_WEIGHT_BPS: u16 = 3000; // 30%
pub const ROUTER_UPDATE_INTERVAL: u64 = 3600; // 1 hour in slots

// Fee Constants
pub const POLYMARKET_FEE_BPS: u16 = 150; // 1.5%
pub const PROTOCOL_FEE_BPS: u16 = 15; // 0.15% average
pub const REFERRAL_RATE_BPS: u16 = 500; // 5% of rewards

// Coverage Constants
pub const TARGET_COVERAGE_RATIO: f64 = 1.0; // 100%
pub const BOOTSTRAP_TAIL_LOSS: f64 = 0.7; // 70% during bootstrap
pub const NORMAL_TAIL_LOSS: f64 = 0.5; // 50% after bootstrap

// Precision
pub const PRECISION: u64 = 10u64.pow(6); // 6 decimals
pub const BPS_PRECISION: u64 = 10_000; // Basis points