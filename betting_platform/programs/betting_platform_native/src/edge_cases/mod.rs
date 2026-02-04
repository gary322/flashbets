//! Edge Case Testing Module
//! 
//! Tests for extreme scenarios and boundary conditions

pub mod market_halt_test;
pub mod oracle_spread_test;
pub mod rate_limit_test;
pub mod max_leverage_test;
pub mod cascade_liquidation_test;

// Re-exports
pub use market_halt_test::*;
pub use oracle_spread_test::*;
pub use rate_limit_test::*;
pub use max_leverage_test::*;
pub use cascade_liquidation_test::*;