//! MMT Token Constants
//! 
//! All constants related to the MMT token distribution and staking system
//! as specified in CLAUDE.md

/// Token decimals (standard SPL token)
pub const MMT_DECIMALS: u8 = 6;

/// Total supply: 100M MMT
pub const TOTAL_SUPPLY: u64 = 100_000_000 * 10u64.pow(MMT_DECIMALS as u32);

/// Current season allocation: 10M MMT (10% of total)
pub const SEASON_ALLOCATION: u64 = 10_000_000 * 10u64.pow(MMT_DECIMALS as u32);

/// Reserved/locked allocation: 90M MMT (90% of total)
pub const RESERVED_ALLOCATION: u64 = 90_000_000 * 10u64.pow(MMT_DECIMALS as u32);

/// Season duration in slots (~6 months at 0.4s/slot)
pub const SEASON_DURATION_SLOTS: u64 = 38_880_000;

/// Season duration in seconds (180 days)
pub const SEASON_DURATION_SECONDS: u64 = 15_552_000;

/// Staking rebate percentage in basis points (15% = 1500 bp)
pub const STAKING_REBATE_BASIS_POINTS: u16 = 1500;

/// Minimum spread improvement for maker rewards (1 basis point)
pub const MIN_SPREAD_IMPROVEMENT_BP: u16 = 1;

/// Early trader limit per season
pub const EARLY_TRADER_LIMIT: u32 = 100;

/// Early trader reward multiplier (2x)
pub const EARLY_TRADER_MULTIPLIER: u8 = 2;

/// Minimum stake amount (100 MMT)
pub const MIN_STAKE_AMOUNT: u64 = 100 * 10u64.pow(MMT_DECIMALS as u32);

/// Lock period options (in slots)
pub const LOCK_PERIOD_30_DAYS: u64 = 6_480_000; // 30 days at 0.4s/slot
pub const LOCK_PERIOD_90_DAYS: u64 = 19_440_000; // 90 days at 0.4s/slot

/// Lock period multipliers (fixed point with 4 decimals)
pub const LOCK_MULTIPLIER_30_DAYS: u16 = 12500; // 1.25x
pub const LOCK_MULTIPLIER_90_DAYS: u16 = 15000; // 1.5x

/// Anti-wash trading parameters
pub const MIN_TRADE_VOLUME_FOR_REWARDS: u64 = 100_000_000; // 100 USDC (6 decimals)
pub const MIN_SLOTS_BETWEEN_TRADES: u64 = 150; // ~1 minute at 0.4s/slot

/// Seeds for PDAs
pub const MMT_CONFIG_SEED: &[u8] = b"mmt_config";
pub const MMT_MINT_SEED: &[u8] = b"mmt_mint";
pub const MMT_TREASURY_SEED: &[u8] = b"mmt_treasury";
pub const MMT_RESERVED_VAULT_SEED: &[u8] = b"mmt_reserved_vault";
pub const STAKING_POOL_SEED: &[u8] = b"staking_pool";
pub const STAKE_VAULT_SEED: &[u8] = b"stake_vault";
pub const STAKE_ACCOUNT_SEED: &[u8] = b"stake";
pub const MAKER_ACCOUNT_SEED: &[u8] = b"maker";
pub const SEASON_EMISSION_SEED: &[u8] = b"season";
pub const DISTRIBUTION_RECORD_SEED: &[u8] = b"distribution";
pub const EARLY_TRADER_REGISTRY_SEED: &[u8] = b"early_traders";