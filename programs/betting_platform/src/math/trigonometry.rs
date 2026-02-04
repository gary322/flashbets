// Trigonometric and statistical functions for PM-AMM
// Native Solana implementation - NO ANCHOR

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::math::fixed_point::{U64F64, MathError, ONE, HALF, PI, SQRT2};
use crate::math::functions::MathFunctions;

pub struct TrigFunctions;

impl TrigFunctions {
    /// Error function approximation for normal CDF calculation
    /// Using Abramowitz and Stegun approximation
    pub fn erf(x: U64F64) -> Result<U64F64, MathError> {
        // erf(x) ≈ sign(x) * (1 - exp(-x² * (a₁ + a₂x² + a₃x⁴) / (1 + b₁x² + b₂x⁴)))
        // This approximation is accurate to ~5×10⁻⁴
        
        // Handle negative x
        let is_negative = x.0 & (1u128 << 127) != 0;
        let abs_x = if is_negative {
            U64F64::ZERO.checked_sub(x).ok_or(MathError::Underflow)?
        } else {
            x
        };
        
        // Constants for approximation (scaled to fixed point)
        let a1 = U64F64::from_raw(23405591988520038);  // 1.26551223
        let a2 = U64F64::from_raw(19580075507265331);  // 1.00002368
        let a3 = U64F64::from_raw(6884282046387404);   // 0.37409196
        let b1 = U64F64::from_raw(4553649124011119);   // 0.24714957
        let b2 = U64F64::from_raw(2628361000280841);   // 0.14253654
        
        // Calculate x²
        let x_squared = abs_x.checked_mul(abs_x).ok_or(MathError::Overflow)?;
        let x_fourth = x_squared.checked_mul(x_squared).ok_or(MathError::Overflow)?;
        
        // Calculate numerator: a₁ + a₂x² + a₃x⁴
        let num_term2 = a2.checked_mul(x_squared).ok_or(MathError::Overflow)?;
        let num_term3 = a3.checked_mul(x_fourth).ok_or(MathError::Overflow)?;
        let numerator = a1.checked_add(num_term2)
            .ok_or(MathError::Overflow)?
            .checked_add(num_term3)
            .ok_or(MathError::Overflow)?;
        
        // Calculate denominator: 1 + b₁x² + b₂x⁴
        let den_term2 = b1.checked_mul(x_squared).ok_or(MathError::Overflow)?;
        let den_term3 = b2.checked_mul(x_fourth).ok_or(MathError::Overflow)?;
        let denominator = U64F64::ONE.checked_add(den_term2)
            .ok_or(MathError::Overflow)?
            .checked_add(den_term3)
            .ok_or(MathError::Overflow)?;
        
        // Calculate the ratio
        let ratio = numerator.checked_div(denominator).ok_or(MathError::DivisionByZero)?;
        
        // Calculate -x² * ratio
        let neg_x_squared_ratio = x_squared.checked_mul(ratio).ok_or(MathError::Overflow)?;
        
        // exp(-x² * ratio)
        let exp_term = MathFunctions::exp(U64F64::ZERO.checked_sub(neg_x_squared_ratio)
            .ok_or(MathError::Underflow)?)?;
        
        // 1 - exp(...)
        let result = U64F64::ONE.checked_sub(exp_term).ok_or(MathError::Underflow)?;
        
        // Apply sign
        if is_negative {
            U64F64::ZERO.checked_sub(result).ok_or(MathError::Underflow)
        } else {
            Ok(result)
        }
    }
    
    /// Hyperbolic tangent: tanh(x) = (e^(2x) - 1) / (e^(2x) + 1)
    pub fn tanh(x: U64F64) -> Result<U64F64, MathError> {
        // For large |x|, tanh(x) ≈ ±1
        if x > U64F64::from_num(10) {
            return Ok(U64F64::ONE);
        }
        
        let neg_ten = U64F64::ZERO.checked_sub(U64F64::from_num(10))
            .ok_or(MathError::Underflow)?;
        if x < neg_ten {
            return Ok(U64F64::ZERO.checked_sub(U64F64::ONE).ok_or(MathError::Underflow)?);
        }
        
        // Calculate 2x
        let two_x = x.checked_mul(U64F64::from_num(2)).ok_or(MathError::Overflow)?;
        
        // Calculate e^(2x)
        let exp_2x = MathFunctions::exp(two_x)?;
        
        // Calculate (e^(2x) - 1) / (e^(2x) + 1)
        let numerator = exp_2x.checked_sub(U64F64::ONE).ok_or(MathError::Underflow)?;
        let denominator = exp_2x.checked_add(U64F64::ONE).ok_or(MathError::Overflow)?;
        
        numerator.checked_div(denominator).ok_or(MathError::DivisionByZero)
    }
    
    /// Normal Cumulative Distribution Function (CDF)
    /// Φ(x) = 0.5 * (1 + erf(x / √2))
    pub fn normal_cdf(x: U64F64) -> Result<U64F64, MathError> {
        // Scale by 1/√2
        let sqrt2_inv = U64F64::from_raw(13043817825332782);  // 1/√2 ≈ 0.7071
        let x_scaled = x.checked_mul(sqrt2_inv).ok_or(MathError::Overflow)?;
        
        // Calculate erf(x/√2)
        let erf_result = Self::erf(x_scaled)?;
        
        // Calculate 1 + erf(x/√2)
        let one_plus_erf = U64F64::ONE.checked_add(erf_result).ok_or(MathError::Overflow)?;
        
        // Multiply by 0.5
        let half = U64F64::from_raw(HALF);
        half.checked_mul(one_plus_erf).ok_or(MathError::Overflow)
    }
    
    /// Normal Probability Density Function (PDF)
    /// φ(x) = (1/√(2π)) * exp(-x²/2)
    pub fn normal_pdf(x: U64F64) -> Result<U64F64, MathError> {
        // Calculate 1/√(2π)
        let two_pi = U64F64::from_raw(PI << 1);  // 2π
        let sqrt_2pi = MathFunctions::sqrt(two_pi)?;
        let one_over_sqrt_2pi = U64F64::ONE.checked_div(sqrt_2pi)
            .ok_or(MathError::DivisionByZero)?;
        
        // Calculate -x²/2
        let x_squared = x.checked_mul(x).ok_or(MathError::Overflow)?;
        let half = U64F64::from_raw(HALF);
        let half_x_squared = x_squared.checked_mul(half).ok_or(MathError::Overflow)?;
        let neg_half_x_squared = U64F64::ZERO.checked_sub(half_x_squared)
            .ok_or(MathError::Underflow)?;
        
        // Calculate exp(-x²/2)
        let exp_term = MathFunctions::exp(neg_half_x_squared)?;
        
        // Final result
        one_over_sqrt_2pi.checked_mul(exp_term).ok_or(MathError::Overflow)
    }
    
    /// Inverse Normal CDF (Quantile function) approximation
    /// Uses Beasley-Springer-Moro algorithm
    pub fn normal_cdf_inv(p: U64F64) -> Result<U64F64, MathError> {
        // p must be in (0, 1)
        if p <= U64F64::ZERO || p >= U64F64::ONE {
            return Err(MathError::InvalidInput);
        }
        
        // Constants for the approximation
        let a0 = U64F64::from_raw(46132903510846914);   // 2.50662823884
        let a1 = U64F64::from_raw(-338844859260526140); // -18.61500062529
        let a2 = U64F64::from_raw(749709499293654322);  // 41.39119773534
        let a3 = U64F64::from_raw(-466963222289118823); // -25.44106049637
        
        let b0 = U64F64::from_raw(-147279186961024729); // -8.04716736160
        let b1 = U64F64::from_raw(440444224352120038);  // 23.93876650760
        let b2 = U64F64::from_raw(-385156242196124019); // -21.06224101826
        let b3 = U64F64::from_raw(63829337846191607);   // 3.46186899655
        
        // Calculate q = p - 0.5
        let half = U64F64::from_raw(HALF);
        let q = p.checked_sub(half).ok_or(MathError::Underflow)?;
        
        // For central region |q| <= 0.425
        let threshold = U64F64::from_raw(7853884307451903);  // 0.425
        if q.abs_diff(U64F64::ZERO) <= threshold {
            let r = q.checked_mul(q).ok_or(MathError::Overflow)?;
            
            // Calculate numerator
            let num = a0.checked_add(
                a1.checked_mul(r)?.checked_add(
                    a2.checked_mul(r)?.checked_mul(r)?.checked_add(
                        a3.checked_mul(r)?.checked_mul(r)?.checked_mul(r)?
                    )?
                )?
            )?;
            
            // Calculate denominator
            let den = U64F64::ONE.checked_add(
                b0.checked_mul(r)?.checked_add(
                    b1.checked_mul(r)?.checked_mul(r)?.checked_add(
                        b2.checked_mul(r)?.checked_mul(r)?.checked_mul(r)?.checked_add(
                            b3.checked_mul(r)?.checked_mul(r)?.checked_mul(r)?.checked_mul(r)?
                        )?
                    )?
                )?
            )?;
            
            return q.checked_mul(num)?.checked_div(den).ok_or(MathError::DivisionByZero);
        }
        
        // For tail regions, use different approximation
        // This is simplified - a full implementation would have more terms
        let r = if q < U64F64::ZERO {
            MathFunctions::sqrt(MathFunctions::ln(p)?)?
        } else {
            MathFunctions::sqrt(MathFunctions::ln(U64F64::ONE.checked_sub(p)?)?)?
        };
        
        // Simplified tail approximation
        let sign = if q < U64F64::ZERO { -1i32 } else { 1i32 };
        let result = r.checked_mul(U64F64::from_num(sign.abs() as u64))?;
        
        if sign < 0 {
            U64F64::ZERO.checked_sub(result).ok_or(MathError::Underflow)
        } else {
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normal_cdf() {
        // Test Φ(0) = 0.5
        let x = U64F64::ZERO;
        let result = TrigFunctions::normal_cdf(x).unwrap();
        let expected = U64F64::from_raw(HALF);
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 20)); // Very small difference
        
        // Test that Φ(x) + Φ(-x) = 1
        let x = U64F64::from_num(1);
        let neg_x = U64F64::ZERO.saturating_sub(x);
        let cdf_x = TrigFunctions::normal_cdf(x).unwrap();
        let cdf_neg_x = TrigFunctions::normal_cdf(neg_x).unwrap();
        let sum = cdf_x.saturating_add(cdf_neg_x);
        let diff = sum.abs_diff(U64F64::ONE);
        assert!(diff.0 < (ONE >> 10)); // Reasonable tolerance
    }
    
    #[test]
    fn test_normal_pdf() {
        // Test φ(0) ≈ 0.3989 (1/√(2π))
        let x = U64F64::ZERO;
        let result = TrigFunctions::normal_pdf(x).unwrap();
        // 1/√(2π) ≈ 0.3989422804
        let expected = U64F64::from_raw(7365300113010534);  // Approximately 0.3989
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 10)); // Reasonable tolerance
        
        // Test symmetry: φ(x) = φ(-x)
        let x = U64F64::from_num(1);
        let neg_x = U64F64::ZERO.saturating_sub(x);
        let pdf_x = TrigFunctions::normal_pdf(x).unwrap();
        let pdf_neg_x = TrigFunctions::normal_pdf(neg_x).unwrap();
        assert_eq!(pdf_x, pdf_neg_x);
    }
    
    #[test]
    fn test_tanh() {
        // Test tanh(0) = 0
        let x = U64F64::ZERO;
        let result = TrigFunctions::tanh(x).unwrap();
        assert_eq!(result, U64F64::ZERO);
        
        // Test tanh(∞) ≈ 1
        let x = U64F64::from_num(20);
        let result = TrigFunctions::tanh(x).unwrap();
        let diff = result.abs_diff(U64F64::ONE);
        assert!(diff.0 < (ONE >> 20));
    }
}