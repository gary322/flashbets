//! Oracle Module
//!
//! Advanced oracle aggregation and price feeds

pub mod advanced_aggregator;
// pub mod handlers; // Temporarily disabled - depends on integration module
pub mod polymarket;
pub mod polymarket_mirror;
pub mod pyth_client;
pub mod validation;
pub mod sigma;
pub mod state;
pub mod fallback;

#[cfg(test)]
pub mod tests;

pub use advanced_aggregator::{
    AdvancedOracleAggregator,
    OracleSource,
    OracleType,
    AggregationMethod,
    AggregationResult,
};

pub use polymarket::OraclePrice;
pub use polymarket_mirror::{
    PolymarketMirror,
    MarketResolution,
    MirrorStatus,
    sync_polymarket_market,
    sync_polymarket_resolution,
    get_mirrored_market,
};

// Export new oracle functionality for fused leverage
pub use pyth_client::{
    PythClient,
    ProbabilityFeed,
    FeedStatus,
    MAX_PROB_LATENCY_SLOTS,
    MAX_SIGMA_LATENCY_SLOTS,
};

pub use validation::{
    OracleValidator,
    PriceHistory,
    ValidationResult,
    TWAP_WINDOW_SLOTS,
    MIN_ORACLE_SOURCES,
    MAX_SOURCE_DEVIATION,
    MAX_TWAP_DEVIATION,
    EWMA_ALPHA,
};

pub use sigma::{
    SigmaCalculator,
    BatchSigmaCalculator,
    SIGMA_EWMA_ALPHA,
    MIN_SIGMA,
    MAX_SIGMA,
    COMPRESSED_HISTORY_SIZE,
};

pub use state::{
    OraclePDA,
    OracleHistoryPDA,
    derive_oracle_pda,
    derive_oracle_history_pda,
    initialize_oracle_pda,
    initialize_oracle_history_pda,
    ORACLE_PDA_SEED,
    ORACLE_HISTORY_SEED,
};

pub use fallback::{
    FallbackHandler,
    FallbackReason,
    FallbackEvent,
    AutoFallbackConfig,
};