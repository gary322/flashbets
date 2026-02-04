use solana_program::program_error::ProgramError;

/// Fixed point representation using u64
/// Represents values as integer * 10^6 (6 decimal places)
pub const FIXED_POINT_SCALE: u64 = 1_000_000;

pub struct U64F64;

impl U64F64 {
    pub const ZERO: u64 = 0;
    pub const ONE: u64 = FIXED_POINT_SCALE;
    pub const MAX: u64 = u64::MAX;
    
    pub fn from_num(value: u64) -> u64 {
        value.saturating_mul(FIXED_POINT_SCALE)
    }
    
    pub fn from_scaled(value: u64) -> u64 {
        value
    }
    
    pub fn to_num(value: u64) -> u64 {
        value / FIXED_POINT_SCALE
    }
    
    pub fn checked_add(a: u64, b: u64) -> Result<u64, ProgramError> {
        a.checked_add(b).ok_or(ProgramError::ArithmeticOverflow)
    }
    
    pub fn checked_sub(a: u64, b: u64) -> Result<u64, ProgramError> {
        a.checked_sub(b).ok_or(ProgramError::ArithmeticOverflow)
    }
    
    pub fn checked_mul(a: u64, b: u64) -> Result<u64, ProgramError> {
        let result = (a as u128).saturating_mul(b as u128) / FIXED_POINT_SCALE as u128;
        if result > u64::MAX as u128 {
            Err(ProgramError::ArithmeticOverflow)
        } else {
            Ok(result as u64)
        }
    }
    
    pub fn checked_div(a: u64, b: u64) -> Result<u64, ProgramError> {
        if b == 0 {
            return Err(ProgramError::DivideByZero);
        }
        let result = (a as u128).saturating_mul(FIXED_POINT_SCALE as u128) / b as u128;
        if result > u64::MAX as u128 {
            Err(ProgramError::ArithmeticOverflow)
        } else {
            Ok(result as u64)
        }
    }
}