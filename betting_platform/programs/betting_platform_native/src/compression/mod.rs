//! State compression module for 10x reduction
//!
//! Implements ZK-based compression for efficient on-chain storage

pub mod zk_state_compression;
pub mod cu_tracker;

pub use zk_state_compression::*;
pub use cu_tracker::{
    CompressionCUTracker,
    HotDataCache,
    CacheStats,
    BatchOptimizer,
};