use solana_program::program_error::ProgramError;
use crate::error::CorrelationError;

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
    
    pub fn from_f64(value: f64) -> Result<u64, ProgramError> {
        if value < 0.0 || value.is_nan() || value.is_infinite() {
            return Err(CorrelationError::ArithmeticOverflow.into());
        }
        let scaled = value * FIXED_POINT_SCALE as f64;
        if scaled > u64::MAX as f64 {
            return Err(CorrelationError::ArithmeticOverflow.into());
        }
        Ok(scaled as u64)
    }
    
    pub fn to_f64(value: u64) -> f64 {
        value as f64 / FIXED_POINT_SCALE as f64
    }
    
    pub fn checked_add(a: u64, b: u64) -> Result<u64, ProgramError> {
        a.checked_add(b).ok_or(CorrelationError::ArithmeticOverflow.into())
    }
    
    pub fn checked_sub(a: u64, b: u64) -> Result<u64, ProgramError> {
        a.checked_sub(b).ok_or(CorrelationError::ArithmeticOverflow.into())
    }
    
    pub fn checked_mul(a: u64, b: u64) -> Result<u64, ProgramError> {
        let result = (a as u128).saturating_mul(b as u128) / FIXED_POINT_SCALE as u128;
        if result > u64::MAX as u128 {
            Err(CorrelationError::ArithmeticOverflow.into())
        } else {
            Ok(result as u64)
        }
    }
    
    pub fn checked_div(a: u64, b: u64) -> Result<u64, ProgramError> {
        if b == 0 {
            return Err(CorrelationError::DivideByZero.into());
        }
        let result = (a as u128).saturating_mul(FIXED_POINT_SCALE as u128) / b as u128;
        if result > u64::MAX as u128 {
            Err(CorrelationError::ArithmeticOverflow.into())
        } else {
            Ok(result as u64)
        }
    }
    
    pub fn sqrt(value: u64) -> Result<u64, ProgramError> {
        // Square root for fixed point numbers
        // Input: value = x * 10^6
        // Output: sqrt(x) * 10^6
        
        if value == 0 {
            return Ok(0);
        }
        
        // Newton's method for integer square root
        let mut x = value;
        let mut y = (x + 1) / 2;
        
        while y < x {
            x = y;
            y = (x + value / x) / 2;
        }
        
        // x is now approximately sqrt(value)
        // Since value = real_value * 10^6, x = sqrt(real_value * 10^6) = sqrt(real_value) * 10^3
        // We need sqrt(real_value) * 10^6, so multiply by 10^3
        let result = (x as u128) * 1000;
        
        if result > u64::MAX as u128 {
            Err(CorrelationError::ArithmeticOverflow.into())
        } else {
            Ok(result as u64)
        }
    }
}

/// Convert between fixed point and percentage basis points
pub fn fixed_to_bps(value: u64) -> u64 {
    (value * 10_000) / FIXED_POINT_SCALE
}

pub fn bps_to_fixed(bps: u64) -> u64 {
    (bps * FIXED_POINT_SCALE) / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fixed_point_operations() {
        // Test from_num
        assert_eq!(U64F64::from_num(1), 1_000_000);
        assert_eq!(U64F64::from_num(100), 100_000_000);
        
        // Test addition
        let a = U64F64::from_num(1);
        let b = U64F64::from_num(2);
        assert_eq!(U64F64::checked_add(a, b).unwrap(), U64F64::from_num(3));
        
        // Test multiplication
        let result = U64F64::checked_mul(a, b).unwrap();
        assert_eq!(result, U64F64::from_num(2));
        
        // Test division
        let result = U64F64::checked_div(b, a).unwrap();
        assert_eq!(result, U64F64::from_num(2));
    }
    
    #[test]
    fn test_sqrt() {
        // Test sqrt(4) = 2
        let four = U64F64::from_num(4); // 4_000_000
        let two = U64F64::sqrt(four).unwrap();
        assert_eq!(two, U64F64::from_num(2)); // 2_000_000
        
        // Test sqrt(1) = 1
        let one = U64F64::ONE;
        let sqrt_one = U64F64::sqrt(one).unwrap();
        assert_eq!(sqrt_one, U64F64::ONE);
        
        // Test sqrt(0.25) = 0.5
        let quarter = U64F64::ONE / 4; // 250_000
        let half = U64F64::sqrt(quarter).unwrap();
        assert_eq!(half, U64F64::ONE / 2); // 500_000
    }
    
    #[test]
    fn test_bps_conversion() {
        // 50% = 5000 bps = 0.5 in fixed point
        let half = U64F64::from_num(1) / 2;
        assert_eq!(fixed_to_bps(half), 5000);
        assert_eq!(bps_to_fixed(5000), half);
        
        // 1% = 100 bps
        assert_eq!(bps_to_fixed(100), 10_000);
    }
}