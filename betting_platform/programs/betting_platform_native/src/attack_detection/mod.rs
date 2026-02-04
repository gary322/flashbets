//! Attack detection module
//!
//! Detects and prevents malicious trading patterns

pub mod initialize;
pub mod process;
pub mod update;
pub mod reset;
pub mod flash_loan_fee;

pub use flash_loan_fee::{
    FLASH_LOAN_FEE_BPS,
    apply_flash_loan_fee,
    calculate_flash_loan_total,
    verify_flash_loan_repayment,
};