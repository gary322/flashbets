// Math utilities bridging to new fixed-point implementation
// This file maintains backward compatibility while using new math module

use anchor_lang::prelude::*;
use crate::ErrorCode;

// Import our new fixed-point math module
pub mod fixed_point;
pub mod functions;
pub mod trigonometry;
pub mod lookup_tables;
pub mod utils;

// Re-export for convenience
pub use self::fixed_point::{U64F64, MathError};
pub use self::functions::MathFunctions;
pub use self::utils::{MathUtils, FeeUtils, LeverageUtils};

// Legacy functions for backward compatibility
pub fn calculate_fee(amount: u64, fee_bps: u16) -> Result<u64> {
    let amount_fixed = U64F64::from_num(amount);
    let fee_fixed = MathUtils::calculate_percentage_bps(amount_fixed, fee_bps)
        .map_err(|_| ErrorCode::MathOverflow)?;
    Ok(fee_fixed.to_num())
}

pub fn calculate_rebate(fee: u64, rebate_bps: u16) -> Result<u64> {
    let fee_fixed = U64F64::from_num(fee);
    let rebate_fixed = MathUtils::calculate_percentage_bps(fee_fixed, rebate_bps)
        .map_err(|_| ErrorCode::MathOverflow)?;
    Ok(rebate_fixed.to_num())
}

pub fn safe_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(ErrorCode::MathOverflow.into())
}

pub fn safe_sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or(ErrorCode::MathOverflow.into())
}

pub fn safe_mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or(ErrorCode::MathOverflow.into())
}

pub fn safe_div(a: u64, b: u64) -> Result<u64> {
    if b == 0 {
        return Err(ErrorCode::MathOverflow.into());
    }
    a.checked_div(b).ok_or(ErrorCode::MathOverflow.into())
}

pub fn calculate_coverage_ratio(
    vault_balance: u64,
    total_open_interest: u64,
    tail_loss: U64F64,
) -> U64F64 {
    if total_open_interest == 0 {
        return U64F64::from_num(0);
    }
    
    let vault_fp = U64F64::from_num(vault_balance);
    let oi_fp = U64F64::from_num(total_open_interest);
    let required = tail_loss * oi_fp;
    
    if required > U64F64::from_num(0) {
        vault_fp / required
    } else {
        U64F64::from_num(0)
    }
}

pub fn calculate_weighted_average(
    values: &[(u64, u64)], // (value, weight)
) -> Result<u64> {
    let mut weighted_sum: u128 = 0;
    let mut total_weight: u128 = 0;
    
    for (value, weight) in values {
        weighted_sum = weighted_sum
            .checked_add((*value as u128).checked_mul(*weight as u128).ok_or(ErrorCode::MathOverflow)?)
            .ok_or(ErrorCode::MathOverflow)?;
        total_weight = total_weight
            .checked_add(*weight as u128)
            .ok_or(ErrorCode::MathOverflow)?;
    }
    
    if total_weight == 0 {
        return Ok(0);
    }
    
    Ok((weighted_sum / total_weight) as u64)
}