//! PM-AMM (Prediction Market AMM) implementation
//!
//! Implements constant-product AMM for multi-outcome markets

pub mod initialize;
pub mod trade;
pub mod liquidity;
pub mod math;
pub mod table_integration;
pub mod newton_raphson;
pub mod price_discovery;

#[cfg(test)]
pub mod test_uniform_lvr;

pub use initialize::process_initialize_pmamm;
pub use trade::process_pmamm_trade;
pub use liquidity::{process_add_liquidity, process_remove_liquidity};
pub use table_integration::{
    calculate_pmamm_delta_with_tables,
    batch_calculate_pmamm,
    process_pmamm_trade_with_tables,
};
pub use newton_raphson::{NewtonRaphsonSolver, SolverResult};
pub use price_discovery::{PriceDiscoveryEngine, PriceDiscoveryResult, ExecutionPath, PMAMMContext};