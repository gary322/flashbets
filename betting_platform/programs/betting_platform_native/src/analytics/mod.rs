//! Analytics module for tracking user metrics and platform performance
//!
//! This module provides comprehensive analytics capabilities including:
//! - User lifetime value (LTV) tracking
//! - Performance metrics and win/loss rates
//! - Risk metrics display and dashboards
//! - Real-time dashboards and reporting

pub mod user_ltv;
pub mod performance_metrics;
pub mod risk_metrics_display;
pub mod backtest_display;

pub use user_ltv::*;
// Don't use wildcard for performance_metrics to avoid conflicts

// Re-export key types
pub use user_ltv::{
    UserLTVMetrics,
    UserSegment,
    LTVIncentives,
    TARGET_LTV_USD,
    process_update_user_ltv,
};

pub use risk_metrics_display::{
    RiskMetricsDisplay,
    display_user_risk_metrics,
};

pub use backtest_display::{
    BacktestResults,
    BacktestScenario,
    process_display_backtest,
    process_compare_strategies,
};