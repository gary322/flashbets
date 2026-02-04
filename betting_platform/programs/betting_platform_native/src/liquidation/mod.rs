//! Liquidation module
//!
//! Handles position liquidation and liquidation queue management

pub mod partial_liquidate;
pub mod queue;
pub mod risk_score;
pub mod high_performance_engine;
pub mod chain_liquidation;
pub mod unified;
pub mod formula_verification;
pub mod graduated_liquidation;
pub mod helpers;
pub mod halt_mechanism;
pub mod drawdown_handler;

pub use risk_score::{calculate_risk_score, calculate_risk_score_with_price};
pub use chain_liquidation::{ChainLiquidationProcessor, ChainLiquidationResult};
pub use unified::{process_liquidate, LiquidationType};

// Re-export commonly used types
pub use queue::{LiquidationQueue, LiquidationCandidate};
pub use partial_liquidate::PartialLiquidationResult;
pub use formula_verification::{
    calculate_liquidation_price_spec,
    calculate_margin_ratio_spec,
    calculate_effective_leverage,
    verify_liquidation_calculation,
    LiquidationVerification,
};
pub use graduated_liquidation::{
    process_graduated_liquidation,
    GraduatedLiquidationState,
    LiquidationDecision,
    calculate_safe_leverage,
};
pub use helpers::{
    calculate_liquidation_amount,
    calculate_keeper_reward,
    should_liquidate_coverage_based,
};
pub use halt_mechanism::{
    LiquidationHaltState,
    process_liquidation_with_halt_check,
    process_override_halt,
    process_initialize_halt_state,
    LIQUIDATION_HALT_DURATION,
};
pub use drawdown_handler::{
    DrawdownState,
    handle_extreme_drawdown,
    calculate_extreme_drawdown_liquidation,
    prevent_liquidation_cascade,
};