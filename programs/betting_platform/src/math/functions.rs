// Advanced mathematical functions for fixed-point arithmetic
// Native Solana implementation - NO ANCHOR

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::math::fixed_point::{U64F64, MathError, ONE, HALF, E, LN2};

pub struct MathFunctions;

impl MathFunctions {
    /// Square root using Newton-Raphson method
    /// For leverage calculations: lev_max = coverage × 100/√N
    pub fn sqrt(x: U64F64) -> Result<U64F64, MathError> {
        if x == U64F64::ZERO {
            return Ok(U64F64::ZERO);
        }
        
        // Initial guess: x/2 for x > 1, otherwise use a better guess
        let mut guess = if x > U64F64::ONE {
            U64F64(x.0 >> 1)
        } else {
            // For x < 1, use sqrt(x) ≈ x for initial guess
            x
        };
        
        let two = U64F64::from_num(2);
        
        // Newton-Raphson: x_{n+1} = (x_n + a/x_n) / 2
        for i in 0..20 {
            let x_over_guess = x.checked_div(guess).ok_or(MathError::DivisionByZero)?;
            let next = guess.checked_add(x_over_guess)
                .ok_or(MathError::Overflow)?
                .checked_div(two)
                .ok_or(MathError::DivisionByZero)?;
            
            // Check convergence
            let diff = guess.abs_diff(next);
            if diff.0 < 100 {  // Converged to within tiny epsilon
                return Ok(next);
            }
            
            // Prevent infinite loops
            if next >= guess && i > 0 {
                return Ok(guess);
            }
            
            guess = next;
        }
        
        Ok(guess)
    }
    
    /// Exponential function using Taylor series
    /// For fee calculations: fee = 3bp + 25bp × exp(-3 × coverage)
    pub fn exp(x: U64F64) -> Result<U64F64, MathError> {
        // Handle special cases
        if x == U64F64::ZERO {
            return Ok(U64F64::ONE);
        }
        
        // For large positive x, use exp(x) = exp(x/2)^2 to avoid overflow
        if x > U64F64::from_num(10) {
            let half_x = U64F64(x.0 >> 1);
            let half_exp = Self::exp(half_x)?;
            return half_exp.checked_mul(half_exp).ok_or(MathError::Overflow);
        }
        
        // For large negative x, result approaches 0
        if x.0 > (20u128 << 64) && x.0 & (1u128 << 127) != 0 {  // Very negative
            return Ok(U64F64::ZERO);
        }
        
        // Taylor series: e^x = 1 + x + x²/2! + x³/3! + ...
        let mut result = U64F64::ONE;
        let mut term = U64F64::ONE;
        let mut n = 1u64;
        
        // Calculate up to 20 terms for good precision
        while n <= 20 {
            // term = term * x / n
            term = term.checked_mul(x)
                .ok_or(MathError::Overflow)?
                .checked_div(U64F64::from_num(n))
                .ok_or(MathError::DivisionByZero)?;
            
            let new_result = result.checked_add(term);
            
            match new_result {
                Some(r) => {
                    // Check if term is negligible
                    if term.0 < (ONE >> 50) {  // Less than 2^-50 in relative terms
                        break;
                    }
                    result = r;
                }
                None => {
                    // Overflow - return saturated value
                    return Ok(U64F64::MAX);
                }
            }
            
            n += 1;
        }
        
        Ok(result)
    }
    
    /// Natural logarithm using series expansion
    /// For LMSR cost function: C(q) = b × log(Σ exp(q_i/b))
    pub fn ln(x: U64F64) -> Result<U64F64, MathError> {
        if x == U64F64::ZERO {
            return Err(MathError::InvalidInput);  // ln(0) is undefined
        }
        
        if x == U64F64::ONE {
            return Ok(U64F64::ZERO);  // ln(1) = 0
        }
        
        // Use the identity: ln(x) = ln(x/2^k) + k*ln(2)
        // Scale x to the range [0.5, 1.5] for better convergence
        let mut scaled = x;
        let mut k = 0i32;
        
        // Scale down if x > 1.5
        let one_point_five = U64F64::from_raw((3u128 << 64) >> 1);  // 1.5
        while scaled > one_point_five {
            scaled = U64F64(scaled.0 >> 1);  // Divide by 2
            k += 1;
        }
        
        // Scale up if x < 0.5
        let half = U64F64::from_raw(HALF);
        while scaled < half {
            scaled = U64F64(scaled.0 << 1);  // Multiply by 2
            k -= 1;
        }
        
        // Now scaled is in [0.5, 1.5], use series: ln(1+y) = y - y²/2 + y³/3 - ...
        // where y = scaled - 1
        let one = U64F64::ONE;
        let y = scaled.checked_sub(one).ok_or(MathError::Underflow)?;
        
        let mut result = U64F64::ZERO;
        let mut term = y;
        let mut sign = 1i32;
        
        for n in 1..=30 {
            let divisor = U64F64::from_num(n as u64);
            let contribution = term.checked_div(divisor).ok_or(MathError::DivisionByZero)?;
            
            if sign > 0 {
                result = result.checked_add(contribution).ok_or(MathError::Overflow)?;
            } else {
                result = result.checked_sub(contribution).ok_or(MathError::Underflow)?;
            }
            
            // Check convergence
            if contribution.0 < (ONE >> 50) {
                break;
            }
            
            // Update term: term = term * y
            term = term.checked_mul(y).ok_or(MathError::Overflow)?;
            sign = -sign;
        }
        
        // Add back the scaling factor: result + k * ln(2)
        let ln2_fixed = U64F64::from_raw(LN2);
        let k_fixed = if k >= 0 {
            U64F64::from_num(k.abs() as u64)
        } else {
            // Negative k means we need to subtract
            return result.checked_sub(
                ln2_fixed.checked_mul(U64F64::from_num(k.abs() as u64))
                    .ok_or(MathError::Overflow)?
            ).ok_or(MathError::Underflow);
        };
        
        let k_ln2 = ln2_fixed.checked_mul(k_fixed).ok_or(MathError::Overflow)?;
        result.checked_add(k_ln2).ok_or(MathError::Overflow)
    }
    
    /// Power function: a^b = exp(b * ln(a))
    pub fn pow(base: U64F64, exponent: U64F64) -> Result<U64F64, MathError> {
        // Special cases
        if exponent == U64F64::ZERO {
            return Ok(U64F64::ONE);  // a^0 = 1
        }
        
        if base == U64F64::ZERO {
            return Ok(U64F64::ZERO);  // 0^b = 0 (for b > 0)
        }
        
        if base == U64F64::ONE {
            return Ok(U64F64::ONE);  // 1^b = 1
        }
        
        // General case: a^b = exp(b * ln(a))
        let ln_base = Self::ln(base)?;
        let b_ln_a = exponent.checked_mul(ln_base).ok_or(MathError::Overflow)?;
        Self::exp(b_ln_a)
    }
    
    /// Integer power for more efficient calculation when exponent is integer
    pub fn powi(base: U64F64, n: u32) -> Result<U64F64, MathError> {
        if n == 0 {
            return Ok(U64F64::ONE);
        }
        
        if n == 1 {
            return Ok(base);
        }
        
        // Use exponentiation by squaring
        let mut result = U64F64::ONE;
        let mut base_power = base;
        let mut exp = n;
        
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.checked_mul(base_power).ok_or(MathError::Overflow)?;
            }
            base_power = base_power.checked_mul(base_power).ok_or(MathError::Overflow)?;
            exp >>= 1;
        }
        
        Ok(result)
    }
    
    /// Calculate percentage with basis points
    pub fn calculate_percentage_bps(value: U64F64, bps: u16) -> Result<U64F64, MathError> {
        let bps_fixed = U64F64::from_num(bps as u64);
        let ten_thousand = U64F64::from_num(10_000);
        
        value.checked_mul(bps_fixed)
            .ok_or(MathError::Overflow)?
            .checked_div(ten_thousand)
            .ok_or(MathError::DivisionByZero)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sqrt() {
        // Test sqrt(4) = 2
        let x = U64F64::from_num(4);
        let result = MathFunctions::sqrt(x).unwrap();
        assert_eq!(result.to_num::<u64>(), 2);
        
        // Test sqrt(9) = 3
        let x = U64F64::from_num(9);
        let result = MathFunctions::sqrt(x).unwrap();
        assert_eq!(result.to_num::<u64>(), 3);
        
        // Test sqrt(2) ≈ 1.414
        let x = U64F64::from_num(2);
        let result = MathFunctions::sqrt(x).unwrap();
        let expected = U64F64::from_raw(SQRT2);
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 20)); // Very small difference
    }
    
    #[test]
    fn test_exp() {
        // Test exp(0) = 1
        let x = U64F64::ZERO;
        let result = MathFunctions::exp(x).unwrap();
        assert_eq!(result, U64F64::ONE);
        
        // Test exp(1) ≈ e
        let x = U64F64::ONE;
        let result = MathFunctions::exp(x).unwrap();
        let expected = U64F64::from_raw(E);
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 10)); // Reasonably small difference
    }
    
    #[test]
    fn test_ln() {
        // Test ln(1) = 0
        let x = U64F64::ONE;
        let result = MathFunctions::ln(x).unwrap();
        assert_eq!(result, U64F64::ZERO);
        
        // Test ln(e) ≈ 1
        let x = U64F64::from_raw(E);
        let result = MathFunctions::ln(x).unwrap();
        let diff = result.abs_diff(U64F64::ONE);
        assert!(diff.0 < (ONE >> 10));
    }
    
    #[test]
    fn test_pow() {
        // Test 2^3 = 8
        let base = U64F64::from_num(2);
        let exp = U64F64::from_num(3);
        let result = MathFunctions::pow(base, exp).unwrap();
        assert_eq!(result.to_num::<u64>(), 8);
    }
}