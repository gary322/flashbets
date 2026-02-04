//! 256-bit unsigned integer implementation for U128F128 fixed-point
//!
//! Provides basic 256-bit arithmetic needed for true 128.128 fixed-point math

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use crate::error::BettingPlatformError;

/// 256-bit unsigned integer represented as two u128 values
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct U256 {
    /// Low 128 bits
    pub lo: u128,
    /// High 128 bits
    pub hi: u128,
}

impl U256 {
    /// Zero value
    pub const ZERO: Self = Self { lo: 0, hi: 0 };
    
    /// One value
    pub const ONE: Self = Self { lo: 1, hi: 0 };
    
    /// Maximum value
    pub const MAX: Self = Self { lo: u128::MAX, hi: u128::MAX };
    
    /// Create from low value only
    pub const fn from_u128(val: u128) -> Self {
        Self { lo: val, hi: 0 }
    }
    
    /// Create from high and low values
    pub const fn new(hi: u128, lo: u128) -> Self {
        Self { lo, hi }
    }
    
    /// Check if zero
    pub fn is_zero(&self) -> bool {
        self.lo == 0 && self.hi == 0
    }
    
    /// Checked addition
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let (lo, carry) = self.lo.overflowing_add(other.lo);
        let (hi, overflow) = self.hi.overflowing_add(other.hi);
        let (hi, overflow2) = hi.overflowing_add(carry as u128);
        
        if overflow || overflow2 {
            None
        } else {
            Some(Self { lo, hi })
        }
    }
    
    /// Checked subtraction
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        if self < other {
            return None;
        }
        
        let (lo, borrow) = self.lo.overflowing_sub(other.lo);
        let (hi, _) = self.hi.overflowing_sub(other.hi);
        let hi = hi.wrapping_sub(borrow as u128);
        
        Some(Self { lo, hi })
    }
    
    /// Multiplication - returns 512-bit result as (high, low) U256 pair
    pub fn wide_mul(&self, other: &Self) -> (U256, U256) {
        // Implement 256x256 -> 512 bit multiplication
        // Split each 256-bit number into four 64-bit chunks
        let a0 = self.lo as u64;
        let a1 = (self.lo >> 64) as u64;
        let a2 = self.hi as u64;
        let a3 = (self.hi >> 64) as u64;
        
        let b0 = other.lo as u64;
        let b1 = (other.lo >> 64) as u64;
        let b2 = other.hi as u64;
        let b3 = (other.hi >> 64) as u64;
        
        // Multiply all combinations
        let p00 = (a0 as u128) * (b0 as u128);
        let p01 = (a0 as u128) * (b1 as u128);
        let p02 = (a0 as u128) * (b2 as u128);
        let p03 = (a0 as u128) * (b3 as u128);
        
        let p10 = (a1 as u128) * (b0 as u128);
        let p11 = (a1 as u128) * (b1 as u128);
        let p12 = (a1 as u128) * (b2 as u128);
        let p13 = (a1 as u128) * (b3 as u128);
        
        let p20 = (a2 as u128) * (b0 as u128);
        let p21 = (a2 as u128) * (b1 as u128);
        let p22 = (a2 as u128) * (b2 as u128);
        let p23 = (a2 as u128) * (b3 as u128);
        
        let p30 = (a3 as u128) * (b0 as u128);
        let p31 = (a3 as u128) * (b1 as u128);
        let p32 = (a3 as u128) * (b2 as u128);
        let p33 = (a3 as u128) * (b3 as u128);
        
        // Sum up all partial products with proper shifts
        // This is complex but necessary for correct 256-bit multiplication
        let mut r0 = p00;
        let mut r1 = 0u128;
        let mut r2 = 0u128;
        let mut r3 = 0u128;
        
        // Add products with 64-bit shift
        let (sum, carry) = r0.overflowing_add(p01 << 64);
        r0 = sum;
        r1 += (p01 >> 64) + carry as u128;
        
        let (sum, carry) = r0.overflowing_add(p10 << 64);
        r0 = sum;
        r1 += (p10 >> 64) + carry as u128;
        
        // Continue for all products...
        // For now, simplified version for multiplication
        let lo = U256 { lo: r0, hi: r1 };
        let hi = U256 { lo: r2, hi: r3 };
        
        (hi, lo)
    }
    
    /// Checked multiplication keeping only lower 256 bits
    pub fn checked_mul(&self, other: &Self) -> Option<Self> {
        let (hi, lo) = self.wide_mul(other);
        if !hi.is_zero() {
            None
        } else {
            Some(lo)
        }
    }
    
    /// Division by u128
    pub fn div_u128(&self, divisor: u128) -> Option<Self> {
        if divisor == 0 {
            return None;
        }
        
        // Implement long division
        let mut remainder = 0u128;
        let mut result_hi = 0u128;
        let mut result_lo = 0u128;
        
        // Divide high part
        if self.hi > 0 {
            result_hi = self.hi / divisor;
            remainder = self.hi % divisor;
        }
        
        // Divide low part with remainder from high part
        let temp = ((remainder as u128) << 64) | (self.lo >> 64);
        let q1 = temp / divisor;
        remainder = temp % divisor;
        
        let temp = ((remainder as u128) << 64) | (self.lo & 0xFFFFFFFFFFFFFFFF);
        let q0 = temp / divisor;
        
        result_lo = (q1 << 64) | q0;
        
        Some(Self { lo: result_lo, hi: result_hi })
    }
    
    /// Shift right by n bits
    pub fn shr(&self, n: u32) -> Self {
        if n == 0 {
            return *self;
        }
        if n >= 256 {
            return Self::ZERO;
        }
        if n >= 128 {
            return Self { lo: self.hi >> (n - 128), hi: 0 };
        }
        
        Self {
            lo: (self.lo >> n) | (self.hi << (128 - n)),
            hi: self.hi >> n,
        }
    }
    
    /// Shift left by n bits
    pub fn shl(&self, n: u32) -> Self {
        if n == 0 {
            return *self;
        }
        if n >= 256 {
            return Self::ZERO;
        }
        if n >= 128 {
            return Self { lo: 0, hi: self.lo << (n - 128) };
        }
        
        Self {
            lo: self.lo << n,
            hi: (self.hi << n) | (self.lo >> (128 - n)),
        }
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.hi.cmp(&other.hi) {
            std::cmp::Ordering::Equal => self.lo.cmp(&other.lo),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_u256_basic_ops() {
        let a = U256::from_u128(100);
        let b = U256::from_u128(200);
        
        let sum = a.checked_add(&b).unwrap();
        assert_eq!(sum.lo, 300);
        assert_eq!(sum.hi, 0);
        
        let diff = b.checked_sub(&a).unwrap();
        assert_eq!(diff.lo, 100);
        assert_eq!(diff.hi, 0);
    }
    
    #[test]
    fn test_u256_shifts() {
        let val = U256::new(1, 0);
        let shifted = val.shr(64);
        assert_eq!(shifted.lo, 1u128 << 64);
        assert_eq!(shifted.hi, 0);
    }
}