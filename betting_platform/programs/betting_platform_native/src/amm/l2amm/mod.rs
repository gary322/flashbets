//! L2-AMM (L2 Norm AMM) implementation
//!
//! Implements L2 norm-based AMM for continuous outcome distributions

pub mod initialize;
pub mod trade;
pub mod distribution;
pub mod math;
pub mod optimized_math;
pub mod simpson;
pub mod types;

pub use initialize::process_initialize_l2amm;
pub use trade::process_l2amm_trade;
pub use distribution::{process_update_distribution, process_resolve_continuous};
pub use optimized_math::*;
pub use simpson::{SimpsonIntegrator, IntegrationResult, SimpsonConfig};
pub use types::{L2AMMContext, L2TradeParams};