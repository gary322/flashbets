//! Cross-Module Integration Tests
//! 
//! Tests that verify correct interaction between multiple modules

pub mod amm_oracle_trading_test;
pub mod liquidation_keeper_mmt_test;
pub mod state_compression_pda_test;
pub mod stress_tests;
pub mod attack_detection_test;
pub mod dark_pool_integration_test;
pub mod security_test;

// Re-exports
pub use amm_oracle_trading_test::*;
pub use liquidation_keeper_mmt_test::*;
pub use state_compression_pda_test::*;
pub use stress_tests::*;
pub use security_test::*;