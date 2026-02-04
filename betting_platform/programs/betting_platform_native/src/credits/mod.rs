//! Credits System Implementation
//!
//! Implements the quantum credits system where deposits provide credits
//! across all proposals within a verse

pub mod credits_manager;
pub mod credit_locking;
pub mod refund_processor;

pub use credits_manager::*;
pub use credit_locking::*;
pub use refund_processor::*;