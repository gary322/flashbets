//! Fee module for elastic fee calculation and distribution
//!
//! Implements coverage-based elastic fees (3-28bp) with maker/taker distinction
//! and proper fee distribution (70% vault, 20% MMT, 10% burn)

pub mod elastic_fee;
pub mod distribution;
pub mod maker_taker;
pub mod polymarket_fee_integration;

pub use elastic_fee::*;
pub use distribution::*;
pub use maker_taker::*;
pub use polymarket_fee_integration::*;

// Fee constants based on Part 7 specifications
pub const FEE_BASE_BPS: u16 = 3; // 3 basis points minimum
pub const FEE_MAX_BPS: u16 = 28; // 28 basis points maximum
pub const FEE_SLOPE: f64 = 25.0; // Slope for exponential fee curve

// Fee distribution ratios (must sum to 100%)
pub const FEE_TO_VAULT_BPS: u16 = 7000; // 70%
pub const FEE_TO_MMT_BPS: u16 = 2000; // 20%
pub const FEE_TO_BURN_BPS: u16 = 1000; // 10%

// Maker/taker specific constants
pub const MAKER_REBATE_BPS: u16 = 3; // 3bp rebate for makers who improve spread
pub const SPREAD_IMPROVEMENT_THRESHOLD_BPS: u16 = 1; // 1bp minimum spread improvement