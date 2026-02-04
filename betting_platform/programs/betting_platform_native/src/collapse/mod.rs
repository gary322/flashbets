//! Market collapse implementation
//!
//! Handles various collapse conditions including max probability and time-based

pub mod max_probability_collapse;

pub use max_probability_collapse::{
    process_settle_slot_collapse,
    process_emergency_collapse,
    check_flash_loan_halt,
    CollapseType,
};