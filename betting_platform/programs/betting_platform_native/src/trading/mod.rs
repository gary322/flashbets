//! Trading module for position management

pub mod open_position;
pub mod close_position;
pub mod validation;
pub mod helpers;
pub mod collateral;
pub mod multi_collateral;
pub mod auto_stop_loss;
pub mod funding_rate;
pub mod leverage_validation;

// Advanced order types
pub mod advanced_orders;
pub mod iceberg;
pub mod twap;
pub mod peg;
pub mod dark_pool;
pub mod block_trading;
pub mod polymarket_interface;

// Backtesting
pub mod backtesting;

// Instruction handlers
pub mod instructions;

pub use open_position::process_open_position;
pub use close_position::process_close_position;
pub use collateral::{process_deposit_collateral, process_withdraw_collateral};
pub use multi_collateral::{
    process_deposit_multi_collateral, 
    process_withdraw_multi_collateral, 
    CollateralType
};

// Re-export helper functions
pub use helpers::{
    calculate_margin_requirement,
    calculate_liquidation_price,
    validate_leverage,
};

// Fixed point type for trading calculations
pub type FixedPoint = crate::math::U64F64;