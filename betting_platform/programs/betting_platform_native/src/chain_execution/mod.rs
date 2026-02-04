//! Chain execution module
//!
//! Handles automated chain trading strategies

pub mod auto_chain;
pub mod unwind;
pub mod cycle_detector;
pub mod timing_safety;
pub mod cross_verse_validator;

#[cfg(test)]
mod test_formulas;

// Re-export chain types from state
pub use crate::state::chain_accounts::{
    ChainPosition,
    ChainState,
    ChainStatus,
    PositionStatus,
    ChainType,
    PositionInfo,
};

// Define ChainLeg as an alias for now (it seems to be missing)
pub type ChainLeg = ChainPosition;