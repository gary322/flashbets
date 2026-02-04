//! Vault Module
//!
//! Core vault system for deposits, yield generation, and zero-loss guarantees

pub mod state;
pub mod deposits;
pub mod withdrawals;
pub mod yield_generation;
pub mod insurance;
pub mod strategies;
pub mod accounting;
pub mod instructions;

pub use state::*;
pub use deposits::*;
pub use withdrawals::*;
pub use yield_generation::*;
pub use insurance::*;
pub use strategies::*;
pub use accounting::*;
pub use instructions::*;