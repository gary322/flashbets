//! Error Handling and Recovery Module
//! 
//! Implements advanced error handling mechanisms:
//! - Atomic transaction rollback for failed chains
//! - Client-side undo window (5 seconds)
//! - On-chain revert capability (1 slot for non-liquidation)

pub mod atomic_rollback;
pub mod undo_window;
pub mod on_chain_revert;
pub mod recovery_manager;

pub use atomic_rollback::*;
pub use undo_window::*;
pub use on_chain_revert::*;
pub use recovery_manager::*;

// Re-export specific types needed by processor
pub use on_chain_revert::StateBeforeAction;