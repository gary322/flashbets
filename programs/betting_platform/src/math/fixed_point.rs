// Native Solana fixed-point math implementation
// NO ANCHOR - pure Solana

use solana_program::{
    program_error::ProgramError,
    msg,
};
use std::convert::TryFrom;

// 64.64 fixed point for standard precision
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct U64F64(pub u128);

// Constants
pub const ONE: u128 = 1 << 64;
pub const HALF: u128 = 1 << 63;
pub const E: u128 = 50143449209799256682;      // e ≈ 2.71828 in 64.64 fixed point
pub const PI: u128 = 57952155664616982739;     // π ≈ 3.14159 in 64.64 fixed point
pub const SQRT2: u128 = 26087635650665564424;  // √2 ≈ 1.41421 in 64.64 fixed point
pub const LN2: u128 = 12786308645202655660;    // ln(2) ≈ 0.693147 in 64.64 fixed point

// 256-bit type for high precision intermediate calculations
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct U256 {
    pub low: u128,
    pub high: u128,
}

// 128.128 fixed point for high precision calculations
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct U128F128(pub U256);

impl U64F64 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(ONE);
    pub const MAX: Self = Self(u128::MAX);
    pub const MIN: Self = Self(0);
    
    // Constructors
    pub fn from_num(n: u64) -> Self {
        Self((n as u128) << 64)
    }
    
    pub fn from_raw(raw: u128) -> Self {
        Self(raw)
    }
    
    pub fn from_bits(bits: u128) -> Self {
        Self(bits)
    }
    
    // Conversions
    pub fn to_num<T: From<u64>>(&self) -> T {
        T::from((self.0 >> 64) as u64)
    }
    
    pub fn to_bits(&self) -> u128 {
        self.0
    }
    
    pub fn frac(&self) -> u64 {
        (self.0 & ((1u128 << 64) - 1)) as u64
    }
    
    pub fn floor(&self) -> Self {
        Self((self.0 >> 64) << 64)
    }
    
    pub fn ceil(&self) -> Self {
        let has_frac = self.frac() > 0;
        let floor = self.floor();
        if has_frac {
            floor.saturating_add(Self::ONE)
        } else {
            floor
        }
    }
    
    // Saturating arithmetic operations
    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
    
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
    
    pub fn saturating_mul(self, other: Self) -> Self {
        // Split into high and low parts for multiplication
        let a_hi = self.0 >> 64;
        let a_lo = self.0 & ((1u128 << 64) - 1);
        let b_hi = other.0 >> 64;
        let b_lo = other.0 & ((1u128 << 64) - 1);
        
        // Calculate partial products
        let hi_hi = a_hi.saturating_mul(b_hi);
        let hi_lo = a_hi.saturating_mul(b_lo);
        let lo_hi = a_lo.saturating_mul(b_hi);
        let lo_lo = a_lo.saturating_mul(b_lo);
        
        // Combine results, shifting to maintain fixed point
        let result = hi_hi.saturating_mul(ONE)
            .saturating_add(hi_lo)
            .saturating_add(lo_hi)
            .saturating_add(lo_lo >> 64);
            
        Self(result)
    }
    
    pub fn saturating_div(self, other: Self) -> Self {
        if other.0 == 0 {
            return Self::MAX;
        }
        
        // Extended precision division
        // (a << 64) / b to maintain fixed point
        let a_extended = U256 {
            low: self.0 << 64,
            high: self.0 >> 64,
        };
        
        let result = u256_div_u128(a_extended, other.0);
        Self(result.low.min(u128::MAX))
    }
    
    // Checked arithmetic operations
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
    
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        // Full 256-bit multiplication
        let a_hi = self.0 >> 64;
        let a_lo = self.0 & ((1u128 << 64) - 1);
        let b_hi = other.0 >> 64;
        let b_lo = other.0 & ((1u128 << 64) - 1);
        
        // Check for definite overflow
        if a_hi != 0 && b_hi != 0 {
            let hi_hi = a_hi.checked_mul(b_hi)?;
            if hi_hi > 1 {
                return None;
            }
        }
        
        // Calculate all partial products
        let hi_lo = a_hi.checked_mul(b_lo)?;
        let lo_hi = a_lo.checked_mul(b_hi)?;
        let lo_lo = a_lo.checked_mul(b_lo)?;
        
        // Combine with overflow checking
        let result = hi_lo
            .checked_add(lo_hi)?
            .checked_add(lo_lo >> 64)?;
            
        Some(Self(result))
    }
    
    pub fn checked_div(self, other: Self) -> Option<Self> {
        if other.0 == 0 {
            return None;
        }
        
        // Extended precision division
        let a_extended = U256 {
            low: self.0 << 64,
            high: self.0 >> 64,
        };
        
        let result = u256_div_u128(a_extended, other.0);
        
        // Check if result fits in u128
        if result.high == 0 {
            Some(Self(result.low))
        } else {
            None
        }
    }
    
    // Comparison helpers
    pub fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }
    
    pub fn max(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }
    
    pub fn abs_diff(self, other: Self) -> Self {
        if self.0 >= other.0 {
            Self(self.0 - other.0)
        } else {
            Self(other.0 - self.0)
        }
    }
}

// Helper function for 256-bit division by 128-bit
fn u256_div_u128(dividend: U256, divisor: u128) -> U256 {
    if divisor == 0 {
        return U256 { low: u128::MAX, high: u128::MAX };
    }
    
    // Simple long division algorithm
    let mut quotient = U256 { low: 0, high: 0 };
    let mut remainder = dividend;
    
    // Divide high part first
    if remainder.high >= divisor {
        quotient.high = remainder.high / divisor;
        remainder.high = remainder.high % divisor;
    }
    
    // Combine remainder with low part and divide
    // This is simplified - in production would need full 256-bit arithmetic
    let combined = remainder.high.saturating_mul(1u128 << 64).saturating_add(remainder.low >> 64);
    quotient.low = combined / divisor;
    
    quotient
}

// Implement standard ops traits for convenience
impl std::ops::Add for U64F64 {
    type Output = Self;
    
    fn add(self, other: Self) -> Self {
        self.saturating_add(other)
    }
}

impl std::ops::Sub for U64F64 {
    type Output = Self;
    
    fn sub(self, other: Self) -> Self {
        self.saturating_sub(other)
    }
}

impl std::ops::Mul for U64F64 {
    type Output = Self;
    
    fn mul(self, other: Self) -> Self {
        self.saturating_mul(other)
    }
}

impl std::ops::Div for U64F64 {
    type Output = Self;
    
    fn div(self, other: Self) -> Self {
        self.saturating_div(other)
    }
}

// Conversion from various numeric types
impl From<u8> for U64F64 {
    fn from(n: u8) -> Self {
        Self::from_num(n as u64)
    }
}

impl From<u16> for U64F64 {
    fn from(n: u16) -> Self {
        Self::from_num(n as u64)
    }
}

impl From<u32> for U64F64 {
    fn from(n: u32) -> Self {
        Self::from_num(n as u64)
    }
}

impl From<u64> for U64F64 {
    fn from(n: u64) -> Self {
        Self::from_num(n)
    }
}

// Error type for math operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathError {
    Overflow,
    Underflow,
    DivisionByZero,
    InvalidInput,
    ConvergenceFailure,
}

impl From<MathError> for ProgramError {
    fn from(e: MathError) -> Self {
        msg!("Math error: {:?}", e);
        ProgramError::Custom(match e {
            MathError::Overflow => 1001,
            MathError::Underflow => 1002,
            MathError::DivisionByZero => 1003,
            MathError::InvalidInput => 1004,
            MathError::ConvergenceFailure => 1005,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_arithmetic() {
        let a = U64F64::from_num(10);
        let b = U64F64::from_num(3);
        
        let sum = a + b;
        assert_eq!(sum.to_num::<u64>(), 13);
        
        let diff = a - b;
        assert_eq!(diff.to_num::<u64>(), 7);
        
        let product = a * b;
        assert_eq!(product.to_num::<u64>(), 30);
        
        let quotient = a / b;
        assert_eq!(quotient.to_num::<u64>(), 3);
    }
    
    #[test]
    fn test_fractional_arithmetic() {
        // 2.5 * 1.5 = 3.75
        let a = U64F64::from_raw((2u128 << 64) + (1u128 << 63)); // 2.5
        let b = U64F64::from_raw((1u128 << 64) + (1u128 << 63)); // 1.5
        
        let product = a * b;
        let expected = U64F64::from_raw((3u128 << 64) + (3u128 << 62)); // 3.75
        
        // Allow small rounding error
        assert!((product.0 as i128 - expected.0 as i128).abs() < 100);
    }
    
    #[test]
    fn test_overflow_protection() {
        let max = U64F64::MAX;
        let one = U64F64::ONE;
        
        // Saturating add should not panic
        let result = max.saturating_add(one);
        assert_eq!(result, U64F64::MAX);
        
        // Checked add should return None
        assert_eq!(max.checked_add(one), None);
    }
}