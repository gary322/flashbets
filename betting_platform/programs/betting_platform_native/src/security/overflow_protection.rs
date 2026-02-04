//! Overflow Protection
//!
//! Production-grade arithmetic overflow protection and safe math operations

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

/// Safe math operations trait
pub trait SafeMath {
    /// Safe addition with overflow check
    fn safe_add(&self, other: Self) -> Result<Self, ProgramError>
    where
        Self: Sized;
    
    /// Safe subtraction with underflow check
    fn safe_sub(&self, other: Self) -> Result<Self, ProgramError>
    where
        Self: Sized;
    
    /// Safe multiplication with overflow check
    fn safe_mul(&self, other: Self) -> Result<Self, ProgramError>
    where
        Self: Sized;
    
    /// Safe division with zero check
    fn safe_div(&self, other: Self) -> Result<Self, ProgramError>
    where
        Self: Sized;
    
    /// Safe power with overflow check
    fn safe_pow(&self, exp: u32) -> Result<Self, ProgramError>
    where
        Self: Sized;
}

/// Implementation for u64
impl SafeMath for u64 {
    fn safe_add(&self, other: Self) -> Result<Self, ProgramError> {
        self.checked_add(other)
            .ok_or_else(|| {
                msg!("Addition overflow: {} + {}", self, other);
                BettingPlatformError::MathOverflow.into()
            })
    }
    
    fn safe_sub(&self, other: Self) -> Result<Self, ProgramError> {
        self.checked_sub(other)
            .ok_or_else(|| {
                msg!("Subtraction underflow: {} - {}", self, other);
                BettingPlatformError::MathUnderflow.into()
            })
    }
    
    fn safe_mul(&self, other: Self) -> Result<Self, ProgramError> {
        self.checked_mul(other)
            .ok_or_else(|| {
                msg!("Multiplication overflow: {} * {}", self, other);
                BettingPlatformError::MathOverflow.into()
            })
    }
    
    fn safe_div(&self, other: Self) -> Result<Self, ProgramError> {
        if other == 0 {
            msg!("Division by zero");
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        
        Ok(self / other)
    }
    
    fn safe_pow(&self, exp: u32) -> Result<Self, ProgramError> {
        self.checked_pow(exp)
            .ok_or_else(|| {
                msg!("Power overflow: {} ^ {}", self, exp);
                BettingPlatformError::MathOverflow.into()
            })
    }
}

/// Implementation for u128
impl SafeMath for u128 {
    fn safe_add(&self, other: Self) -> Result<Self, ProgramError> {
        self.checked_add(other)
            .ok_or_else(|| {
                msg!("Addition overflow: {} + {}", self, other);
                BettingPlatformError::MathOverflow.into()
            })
    }
    
    fn safe_sub(&self, other: Self) -> Result<Self, ProgramError> {
        self.checked_sub(other)
            .ok_or_else(|| {
                msg!("Subtraction underflow: {} - {}", self, other);
                BettingPlatformError::MathUnderflow.into()
            })
    }
    
    fn safe_mul(&self, other: Self) -> Result<Self, ProgramError> {
        self.checked_mul(other)
            .ok_or_else(|| {
                msg!("Multiplication overflow: {} * {}", self, other);
                BettingPlatformError::MathOverflow.into()
            })
    }
    
    fn safe_div(&self, other: Self) -> Result<Self, ProgramError> {
        if other == 0 {
            msg!("Division by zero");
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        
        Ok(self / other)
    }
    
    fn safe_pow(&self, exp: u32) -> Result<Self, ProgramError> {
        self.checked_pow(exp)
            .ok_or_else(|| {
                msg!("Power overflow: {} ^ {}", self, exp);
                BettingPlatformError::MathOverflow.into()
            })
    }
}

/// Safe percentage calculation
pub fn safe_percentage(value: u64, percentage_bps: u16) -> Result<u64, ProgramError> {
    if percentage_bps > 10_000 {
        msg!("Invalid percentage: {} bps", percentage_bps);
        return Err(BettingPlatformError::InvalidPercentage.into());
    }
    
    let value_u128 = value as u128;
    let percentage_u128 = percentage_bps as u128;
    
    let result = value_u128
        .safe_mul(percentage_u128)?
        .safe_div(10_000)?;
    
    if result > u64::MAX as u128 {
        msg!("Percentage calculation overflow");
        return Err(BettingPlatformError::MathOverflow.into());
    }
    
    Ok(result as u64)
}

/// Safe ratio calculation with precision
pub fn safe_ratio(numerator: u64, denominator: u64, precision: u64) -> Result<u64, ProgramError> {
    if denominator == 0 {
        msg!("Division by zero in ratio calculation");
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    let num_u128 = numerator as u128;
    let denom_u128 = denominator as u128;
    let precision_u128 = precision as u128;
    
    let result = num_u128
        .safe_mul(precision_u128)?
        .safe_div(denom_u128)?;
    
    if result > u64::MAX as u128 {
        msg!("Ratio calculation overflow");
        return Err(BettingPlatformError::MathOverflow.into());
    }
    
    Ok(result as u64)
}

/// Safe cast from u128 to u64
pub fn safe_cast_u64(value: u128) -> Result<u64, ProgramError> {
    if value > u64::MAX as u128 {
        msg!("Cast overflow: {} > u64::MAX", value);
        return Err(BettingPlatformError::CastOverflow.into());
    }
    
    Ok(value as u64)
}

/// Safe cast from i64 to u64
pub fn safe_cast_from_i64(value: i64) -> Result<u64, ProgramError> {
    if value < 0 {
        msg!("Cannot cast negative value to u64: {}", value);
        return Err(BettingPlatformError::InvalidCast.into());
    }
    
    Ok(value as u64)
}

/// Bounds checking for array access
pub fn safe_array_access<T>(array: &[T], index: usize) -> Result<&T, ProgramError> {
    array.get(index).ok_or_else(|| {
        msg!("Array index out of bounds: {} >= {}", index, array.len());
        BettingPlatformError::IndexOutOfBounds.into()
    })
}

/// Bounds checking for mutable array access
pub fn safe_array_access_mut<T>(array: &mut [T], index: usize) -> Result<&mut T, ProgramError> {
    let len = array.len();
    array.get_mut(index).ok_or_else(|| {
        msg!("Array index out of bounds: {} >= {}", index, len);
        BettingPlatformError::IndexOutOfBounds.into()
    })
}

/// Overflow detector for tracking potential issues
pub struct OverflowDetector {
    pub operations_count: u64,
    pub overflow_count: u64,
    pub underflow_count: u64,
    pub div_zero_count: u64,
    pub last_error_slot: u64,
}

impl OverflowDetector {
    pub fn new() -> Self {
        Self {
            operations_count: 0,
            overflow_count: 0,
            underflow_count: 0,
            div_zero_count: 0,
            last_error_slot: 0,
        }
    }
    
    /// Record an operation
    pub fn record_operation(&mut self) {
        self.operations_count = self.operations_count.saturating_add(1);
    }
    
    /// Record an overflow
    pub fn record_overflow(&mut self, slot: u64) {
        self.overflow_count = self.overflow_count.saturating_add(1);
        self.last_error_slot = slot;
        msg!("Overflow detected at slot {}", slot);
    }
    
    /// Record an underflow
    pub fn record_underflow(&mut self, slot: u64) {
        self.underflow_count = self.underflow_count.saturating_add(1);
        self.last_error_slot = slot;
        msg!("Underflow detected at slot {}", slot);
    }
    
    /// Record division by zero
    pub fn record_div_zero(&mut self, slot: u64) {
        self.div_zero_count = self.div_zero_count.saturating_add(1);
        self.last_error_slot = slot;
        msg!("Division by zero at slot {}", slot);
    }
    
    /// Get error rate
    pub fn error_rate(&self) -> f64 {
        if self.operations_count == 0 {
            return 0.0;
        }
        
        let total_errors = self.overflow_count + self.underflow_count + self.div_zero_count;
        total_errors as f64 / self.operations_count as f64
    }
    
    /// Check if error rate is acceptable
    pub fn is_healthy(&self) -> bool {
        self.error_rate() < 0.001 // Less than 0.1% error rate
    }
}

/// Validate numerical bounds
pub fn validate_bounds(value: u64, min: u64, max: u64) -> Result<(), ProgramError> {
    if value < min {
        msg!("Value {} below minimum {}", value, min);
        return Err(BettingPlatformError::BelowMinimum.into());
    }
    
    if value > max {
        msg!("Value {} above maximum {}", value, max);
        return Err(BettingPlatformError::AboveMaximum.into());
    }
    
    Ok(())
}

/// Safe increment with maximum check
pub fn safe_increment(value: &mut u64, max: u64) -> Result<(), ProgramError> {
    if *value >= max {
        msg!("Cannot increment: already at maximum {}", max);
        return Err(BettingPlatformError::AtMaximum.into());
    }
    
    *value = value.safe_add(1)?;
    Ok(())
}

/// Safe decrement with minimum check
pub fn safe_decrement(value: &mut u64, min: u64) -> Result<(), ProgramError> {
    if *value <= min {
        msg!("Cannot decrement: already at minimum {}", min);
        return Err(BettingPlatformError::AtMinimum.into());
    }
    
    *value = value.safe_sub(1)?;
    Ok(())
}

/// Batch overflow check for multiple operations
pub struct BatchOverflowChecker {
    results: Vec<Result<u64, ProgramError>>,
}

impl BatchOverflowChecker {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
    
    /// Add operation result
    pub fn add_result(&mut self, result: Result<u64, ProgramError>) {
        self.results.push(result);
    }
    
    /// Check all results
    pub fn check_all(&self) -> Result<Vec<u64>, ProgramError> {
        let mut values = Vec::with_capacity(self.results.len());
        
        for (i, result) in self.results.iter().enumerate() {
            match result {
                Ok(value) => values.push(*value),
                Err(e) => {
                    msg!("Batch operation {} failed", i);
                    return Err(e.clone());
                }
            }
        }
        
        Ok(values)
    }
    
    /// Get summary
    pub fn summary(&self) -> (usize, usize) {
        let successful = self.results.iter().filter(|r| r.is_ok()).count();
        let failed = self.results.len() - successful;
        (successful, failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_math_u64() {
        // Addition
        assert_eq!(10u64.safe_add(20).unwrap(), 30);
        assert!(u64::MAX.safe_add(1).is_err());
        
        // Subtraction
        assert_eq!(30u64.safe_sub(20).unwrap(), 10);
        assert!(10u64.safe_sub(20).is_err());
        
        // Multiplication
        assert_eq!(10u64.safe_mul(20).unwrap(), 200);
        assert!(u64::MAX.safe_mul(2).is_err());
        
        // Division
        assert_eq!(100u64.safe_div(20).unwrap(), 5);
        assert!(100u64.safe_div(0).is_err());
        
        // Power
        assert_eq!(2u64.safe_pow(10).unwrap(), 1024);
        assert!(10u64.safe_pow(20).is_err());
    }

    #[test]
    fn test_safe_percentage() {
        assert_eq!(safe_percentage(1000, 100).unwrap(), 10); // 1%
        assert_eq!(safe_percentage(1000, 5000).unwrap(), 500); // 50%
        assert_eq!(safe_percentage(1000, 10000).unwrap(), 1000); // 100%
        assert!(safe_percentage(1000, 10001).is_err()); // > 100%
    }

    #[test]
    fn test_safe_ratio() {
        assert_eq!(safe_ratio(1, 2, 1_000_000).unwrap(), 500_000);
        assert_eq!(safe_ratio(3, 4, 1_000_000).unwrap(), 750_000);
        assert!(safe_ratio(1, 0, 1_000_000).is_err());
    }

    #[test]
    fn test_overflow_detector() {
        let mut detector = OverflowDetector::new();
        
        for _ in 0..100 {
            detector.record_operation();
        }
        
        detector.record_overflow(100);
        detector.record_underflow(101);
        
        assert_eq!(detector.operations_count, 100);
        assert_eq!(detector.overflow_count, 1);
        assert_eq!(detector.underflow_count, 1);
        assert!(detector.is_healthy()); // 2/100 = 2% > 0.1% threshold
    }

    #[test]
    fn test_bounds_validation() {
        assert!(validate_bounds(50, 0, 100).is_ok());
        assert!(validate_bounds(0, 0, 100).is_ok());
        assert!(validate_bounds(100, 0, 100).is_ok());
        assert!(validate_bounds(101, 0, 100).is_err());
        assert!(validate_bounds(50, 60, 100).is_err());
    }
}