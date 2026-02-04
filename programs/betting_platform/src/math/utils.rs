// Utility functions for fixed-point math
// Native Solana implementation - NO ANCHOR

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::math::fixed_point::{U64F64, U128F128, U256, MathError, ONE};

pub struct MathUtils;

impl MathUtils {
    /// Convert between precision levels - U64F64 to U128F128
    pub fn u64f64_to_u128f128(x: U64F64) -> U128F128 {
        // Extend to 256 bits, shifting left by 64 to maintain fixed point position
        let extended = U256 {
            low: x.0 << 64,
            high: x.0 >> 64,
        };
        U128F128(extended)
    }
    
    /// Convert between precision levels - U128F128 to U64F64
    pub fn u128f128_to_u64f64(x: U128F128) -> Result<U64F64, MathError> {
        // Extract the middle 128 bits (bits 64-191)
        let result = x.0.low >> 64 | (x.0.high << 64);
        
        // Check for overflow
        if x.0.high >> 64 != 0 {
            return Err(MathError::Overflow);
        }
        
        Ok(U64F64::from_raw(result))
    }
    
    /// Calculate percentage with basis points (1 bp = 0.01%)
    pub fn calculate_percentage_bps(value: U64F64, bps: u16) -> Result<U64F64, MathError> {
        let bps_fixed = U64F64::from_num(bps as u64);
        let ten_thousand = U64F64::from_num(10_000);
        
        value.checked_mul(bps_fixed)
            .ok_or(MathError::Overflow)?
            .checked_div(ten_thousand)
            .ok_or(MathError::DivisionByZero)
    }
    
    /// Safe division with rounding to nearest
    pub fn safe_div_round(numerator: U64F64, denominator: U64F64) -> Result<U64F64, MathError> {
        if denominator == U64F64::ZERO {
            return Err(MathError::DivisionByZero);
        }
        
        // Add half denominator for rounding
        let half_denom = U64F64(denominator.0 >> 1);
        let adjusted_num = numerator.checked_add(half_denom).ok_or(MathError::Overflow)?;
        
        adjusted_num.checked_div(denominator).ok_or(MathError::DivisionByZero)
    }
    
    /// Clamp value to range [min, max]
    pub fn clamp(value: U64F64, min: U64F64, max: U64F64) -> U64F64 {
        value.max(min).min(max)
    }
    
    /// Linear interpolation: lerp(a, b, t) = a + t * (b - a)
    pub fn lerp(a: U64F64, b: U64F64, t: U64F64) -> Result<U64F64, MathError> {
        // Clamp t to [0, 1]
        let t_clamped = Self::clamp(t, U64F64::ZERO, U64F64::ONE);
        
        if a <= b {
            let diff = b.checked_sub(a).ok_or(MathError::Underflow)?;
            let scaled = t_clamped.checked_mul(diff).ok_or(MathError::Overflow)?;
            a.checked_add(scaled).ok_or(MathError::Overflow)
        } else {
            let diff = a.checked_sub(b).ok_or(MathError::Underflow)?;
            let scaled = t_clamped.checked_mul(diff).ok_or(MathError::Overflow)?;
            a.checked_sub(scaled).ok_or(MathError::Underflow)
        }
    }
    
    /// Calculate average of multiple values
    pub fn average(values: &[U64F64]) -> Result<U64F64, MathError> {
        if values.is_empty() {
            return Err(MathError::InvalidInput);
        }
        
        let mut sum = U64F64::ZERO;
        for &value in values {
            sum = sum.checked_add(value).ok_or(MathError::Overflow)?;
        }
        
        sum.checked_div(U64F64::from_num(values.len() as u64))
            .ok_or(MathError::DivisionByZero)
    }
    
    /// Calculate weighted average
    pub fn weighted_average(values_weights: &[(U64F64, U64F64)]) -> Result<U64F64, MathError> {
        if values_weights.is_empty() {
            return Err(MathError::InvalidInput);
        }
        
        let mut weighted_sum = U64F64::ZERO;
        let mut total_weight = U64F64::ZERO;
        
        for &(value, weight) in values_weights {
            let weighted_value = value.checked_mul(weight).ok_or(MathError::Overflow)?;
            weighted_sum = weighted_sum.checked_add(weighted_value).ok_or(MathError::Overflow)?;
            total_weight = total_weight.checked_add(weight).ok_or(MathError::Overflow)?;
        }
        
        if total_weight == U64F64::ZERO {
            return Err(MathError::DivisionByZero);
        }
        
        weighted_sum.checked_div(total_weight).ok_or(MathError::DivisionByZero)
    }
    
    /// Convert from Polymarket probability [0,1] to fixed-point percentage [0,100]
    pub fn from_polymarket_prob(prob_float: f64) -> Result<U64F64, MathError> {
        if prob_float < 0.0 || prob_float > 1.0 {
            return Err(MathError::InvalidInput);
        }
        
        // Convert to percentage (0-100) in fixed point
        let percentage = (prob_float * 100.0 * (ONE as f64)) as u128;
        Ok(U64F64::from_raw(percentage))
    }
    
    /// Convert fixed-point percentage to float for display
    pub fn to_float_percentage(percentage: U64F64) -> f64 {
        (percentage.0 as f64) / (ONE as f64)
    }
    
    /// Convert price with decimals to fixed-point
    pub fn price_from_decimals(amount: u64, decimals: u8) -> Result<U64F64, MathError> {
        if decimals > 18 {
            return Err(MathError::InvalidInput);
        }
        
        let scale = 10u64.pow(decimals as u32);
        let amount_fixed = U64F64::from_num(amount);
        let scale_fixed = U64F64::from_num(scale);
        
        amount_fixed.checked_div(scale_fixed).ok_or(MathError::DivisionByZero)
    }
    
    /// Convert fixed-point to price with decimals
    pub fn to_price_decimals(value: U64F64, decimals: u8) -> Result<u64, MathError> {
        if decimals > 18 {
            return Err(MathError::InvalidInput);
        }
        
        let scale = 10u64.pow(decimals as u32);
        let scale_fixed = U64F64::from_num(scale);
        
        let scaled = value.checked_mul(scale_fixed).ok_or(MathError::Overflow)?;
        Ok(scaled.to_num())
    }
    
    /// Calculate compound interest: A = P(1 + r)^n
    pub fn compound_interest(
        principal: U64F64,
        rate_per_period: U64F64,
        periods: u32,
    ) -> Result<U64F64, MathError> {
        let one_plus_rate = U64F64::ONE.checked_add(rate_per_period)
            .ok_or(MathError::Overflow)?;
        
        let mut result = principal;
        for _ in 0..periods {
            result = result.checked_mul(one_plus_rate).ok_or(MathError::Overflow)?;
        }
        
        Ok(result)
    }
    
    /// Calculate the absolute value of a signed fixed-point number
    pub fn abs_i64f64(x: i128) -> U64F64 {
        if x < 0 {
            U64F64::from_raw((-x) as u128)
        } else {
            U64F64::from_raw(x as u128)
        }
    }
    
    /// Convert unsigned fixed-point to signed
    pub fn u64f64_to_i64f64(x: U64F64) -> Result<i128, MathError> {
        if x.0 > i128::MAX as u128 {
            Err(MathError::Overflow)
        } else {
            Ok(x.0 as i128)
        }
    }
    
    /// Convert signed fixed-point to unsigned
    pub fn i64f64_to_u64f64(x: i128) -> Result<U64F64, MathError> {
        if x < 0 {
            Err(MathError::InvalidInput)
        } else {
            Ok(U64F64::from_raw(x as u128))
        }
    }
}

/// Helper functions for leverage calculations
pub struct LeverageUtils;

impl LeverageUtils {
    /// Calculate maximum leverage according to formula:
    /// lev_max = min(100 × (1 + 0.1 × depth), coverage × 100/√N, tier_cap(N))
    pub fn calculate_max_leverage(
        depth: u32,
        coverage: U64F64,
        n_outcomes: u32,
    ) -> Result<U64F64, MathError> {
        // Component 1: 100 × (1 + 0.1 × depth)
        let depth_factor = U64F64::ONE
            .checked_add(
                U64F64::from_num(depth as u64)
                    .checked_div(U64F64::from_num(10))
                    .ok_or(MathError::DivisionByZero)?
            )
            .ok_or(MathError::Overflow)?;
        let depth_leverage = U64F64::from_num(100)
            .checked_mul(depth_factor)
            .ok_or(MathError::Overflow)?;
        
        // Component 2: coverage × 100/√N
        let sqrt_n = crate::math::functions::MathFunctions::sqrt(U64F64::from_num(n_outcomes as u64))?;
        let coverage_leverage = coverage
            .checked_mul(U64F64::from_num(100))
            .ok_or(MathError::Overflow)?
            .checked_div(sqrt_n)
            .ok_or(MathError::DivisionByZero)?;
        
        // Component 3: tier_cap(N)
        let tier_cap = Self::get_tier_cap(n_outcomes);
        
        // Return minimum of all three
        Ok(depth_leverage.min(coverage_leverage).min(tier_cap))
    }
    
    /// Get tier cap based on number of outcomes
    fn get_tier_cap(n: u32) -> U64F64 {
        match n {
            1 => U64F64::from_num(100),      // Binary: 100x
            2 => U64F64::from_num(70),       // 70x
            3..=4 => U64F64::from_num(25),   // 25x
            5..=8 => U64F64::from_num(15),   // 15x
            9..=16 => U64F64::from_num(12),  // 12x
            17..=64 => U64F64::from_num(10), // 10x
            _ => U64F64::from_num(5),        // 5x for N>64
        }
    }
    
    /// Calculate effective leverage with chaining
    /// lev_eff = lev_base × ∏(1 + r_i)
    pub fn calculate_effective_leverage(
        base_leverage: U64F64,
        chain_returns: &[U64F64],
    ) -> Result<U64F64, MathError> {
        let mut effective = base_leverage;
        
        for &r in chain_returns {
            let multiplier = U64F64::ONE.checked_add(r).ok_or(MathError::Overflow)?;
            effective = effective.checked_mul(multiplier).ok_or(MathError::Overflow)?;
            
            // Safety check: cap at 500x
            if effective > U64F64::from_num(500) {
                effective = U64F64::from_num(500);
                break;
            }
        }
        
        Ok(effective)
    }
}

/// Helper functions for fee calculations
pub struct FeeUtils;

impl FeeUtils {
    /// Calculate elastic fee: fee = 3bp + 25bp × exp(-3 × coverage)
    pub fn calculate_elastic_fee(coverage: U64F64) -> Result<U64F64, MathError> {
        let three_bp = MathUtils::calculate_percentage_bps(U64F64::ONE, 3)?;      // 0.03%
        let twenty_five_bp = MathUtils::calculate_percentage_bps(U64F64::ONE, 25)?; // 0.25%
        
        // Calculate -3 × coverage
        let three = U64F64::from_num(3);
        let neg_three_coverage = U64F64::ZERO
            .checked_sub(coverage.checked_mul(three).ok_or(MathError::Overflow)?)
            .ok_or(MathError::Underflow)?;
        
        // Calculate exp(-3 × coverage)
        let exp_term = crate::math::functions::MathFunctions::exp(neg_three_coverage)?;
        
        // Calculate fee = 3bp + 25bp × exp_term
        let variable_fee = twenty_five_bp.checked_mul(exp_term).ok_or(MathError::Overflow)?;
        let total_fee = three_bp.checked_add(variable_fee).ok_or(MathError::Overflow)?;
        
        // Cap at 28bp maximum
        let max_fee = MathUtils::calculate_percentage_bps(U64F64::ONE, 28)?;
        Ok(total_fee.min(max_fee))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_percentage_calculation() {
        let value = U64F64::from_num(1000);
        let result = MathUtils::calculate_percentage_bps(value, 250).unwrap(); // 2.5%
        assert_eq!(result.to_num::<u64>(), 25);
    }
    
    #[test]
    fn test_polymarket_conversion() {
        let prob = 0.75; // 75% probability
        let fixed = MathUtils::from_polymarket_prob(prob).unwrap();
        let percentage = MathUtils::to_float_percentage(fixed);
        assert!((percentage - 75.0).abs() < 0.001);
    }
    
    #[test]
    fn test_leverage_calculation() {
        let depth = 5;
        let coverage = U64F64::from_num(2);
        let n_outcomes = 4;
        
        let max_lev = LeverageUtils::calculate_max_leverage(depth, coverage, n_outcomes).unwrap();
        
        // Should be limited by tier cap of 25x for 4 outcomes
        assert_eq!(max_lev.to_num::<u64>(), 25);
    }
    
    #[test]
    fn test_elastic_fee() {
        // High coverage should give low fee (close to 3bp)
        let high_coverage = U64F64::from_num(10);
        let low_fee = FeeUtils::calculate_elastic_fee(high_coverage).unwrap();
        let three_bp = MathUtils::calculate_percentage_bps(U64F64::ONE, 3).unwrap();
        assert!(low_fee.abs_diff(three_bp).0 < (ONE >> 20));
        
        // Low coverage should give higher fee (but capped at 28bp)
        let low_coverage = U64F64::from_raw(ONE / 10); // 0.1
        let high_fee = FeeUtils::calculate_elastic_fee(low_coverage).unwrap();
        let twenty_eight_bp = MathUtils::calculate_percentage_bps(U64F64::ONE, 28).unwrap();
        assert_eq!(high_fee, twenty_eight_bp);
    }
}