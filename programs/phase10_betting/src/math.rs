use anchor_lang::prelude::*;
use crate::types::{U64F64, I64F64};

pub fn calculate_fee(amount: u64, fee_bps: u16) -> Result<u64> {
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(crate::errors::ErrorCode::MathOverflow)?
        .checked_div(10_000)
        .ok_or(crate::errors::ErrorCode::MathOverflow)?;
    
    Ok(fee as u64)
}

pub fn calculate_rebate(fee: u64, rebate_bps: u16) -> Result<u64> {
    let rebate = (fee as u128)
        .checked_mul(rebate_bps as u128)
        .ok_or(crate::errors::ErrorCode::MathOverflow)?
        .checked_div(10_000)
        .ok_or(crate::errors::ErrorCode::MathOverflow)?;
    
    Ok(rebate as u64)
}

pub fn safe_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(crate::errors::ErrorCode::MathOverflow.into())
}

pub fn safe_sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or(crate::errors::ErrorCode::MathOverflow.into())
}

pub fn safe_mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or(crate::errors::ErrorCode::MathOverflow.into())
}

pub fn safe_div(a: u64, b: u64) -> Result<u64> {
    if b == 0 {
        return Err(crate::errors::ErrorCode::MathOverflow.into());
    }
    a.checked_div(b).ok_or(crate::errors::ErrorCode::MathOverflow.into())
}

pub fn calculate_coverage_ratio(
    vault_balance: u64,
    total_open_interest: u64,
    tail_loss: U64F64,
) -> U64F64 {
    if total_open_interest == 0 {
        return U64F64::zero();
    }
    
    let vault_fp = U64F64::from_num(vault_balance);
    let oi_fp = U64F64::from_num(total_open_interest);
    let required = tail_loss * oi_fp;
    
    if required > U64F64::zero() {
        vault_fp / required
    } else {
        U64F64::zero()
    }
}

pub fn calculate_weighted_average(
    values: &[(u64, u64)], // (value, weight)
) -> Result<u64> {
    let mut weighted_sum: u128 = 0;
    let mut total_weight: u128 = 0;
    
    for (value, weight) in values {
        weighted_sum = weighted_sum
            .checked_add((*value as u128).checked_mul(*weight as u128).ok_or(crate::errors::ErrorCode::MathOverflow)?)
            .ok_or(crate::errors::ErrorCode::MathOverflow)?;
        total_weight = total_weight
            .checked_add(*weight as u128)
            .ok_or(crate::errors::ErrorCode::MathOverflow)?;
    }
    
    if total_weight == 0 {
        return Ok(0);
    }
    
    Ok((weighted_sum / total_weight) as u64)
}

