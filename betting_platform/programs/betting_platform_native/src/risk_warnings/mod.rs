//! Risk warnings and mandatory quiz for leverage trading
//!
//! Ensures users understand the risks before using high leverage

pub mod leverage_quiz;
pub mod risk_disclosure;
pub mod warning_modals;

pub use leverage_quiz::*;
pub use risk_disclosure::*;
pub use warning_modals::*;