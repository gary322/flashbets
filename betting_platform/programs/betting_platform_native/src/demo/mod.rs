//! Demo mode and paper trading functionality
//!
//! Provides a safe environment for users to practice trading with fake USDC

pub mod demo_mode;
pub mod fake_usdc;
pub mod demo_positions;
pub mod loss_simulation;

pub use demo_mode::*;
pub use fake_usdc::*;
pub use demo_positions::*;
pub use loss_simulation::*;