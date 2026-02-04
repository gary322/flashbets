//! AMM constants and configuration

/// Maximum number of outcomes for any market
pub const MAX_OUTCOMES: u8 = 64;

/// Minimum liquidity for market creation
pub const MIN_LIQUIDITY: u64 = 1_000_000; // 1 USDC

// MAX_PRICE moved to global constants

/// Minimum price (basis points)
pub const MIN_PRICE: u64 = 1; // 0.01%

/// Default fee (basis points)
pub const DEFAULT_FEE_BPS: u16 = 30; // 0.3%

// PRICE_PRECISION moved to global constants

/// Maximum slippage allowed (basis points)
pub const MAX_SLIPPAGE_BPS: u16 = 500; // 5%

/// Minimum trade size
pub const MIN_TRADE_SIZE: u64 = 1_000; // 0.001 USDC

/// LVR (Loss-Versus-Rebalancing) protection (basis points)
pub const LVR_PROTECTION_BPS: u16 = 500; // 5%

// PRICE_CLAMP_PER_SLOT_BPS moved to global constants