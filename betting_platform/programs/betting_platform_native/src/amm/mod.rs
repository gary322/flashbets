//! AMM (Automated Market Maker) implementations
//!
//! Contains LMSR, PM-AMM, L2-AMM, and Hybrid AMM modules

pub mod lmsr;
pub mod pmamm;
pub mod l2amm;
pub mod hybrid;
pub mod helpers;
pub mod constants;
pub mod auto_selector;
pub mod enforced_selector;

pub use constants::*;
pub use auto_selector::{select_amm_type, should_use_l2_norm, validate_amm_selection};
pub use helpers::{calculate_price_impact, execute_trade};
pub use hybrid::calculate_hybrid_price;

// Re-export newton_raphson_solver module
pub mod newton_raphson_solver {
    pub use crate::amm::pmamm::newton_raphson::*;
}

// Production implementations
pub mod newton_raphson_production;
pub mod simpson_integration_production;