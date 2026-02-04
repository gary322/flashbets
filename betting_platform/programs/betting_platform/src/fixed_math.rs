use anchor_lang::prelude::*;

pub const PRECISION: u128 = 1_000_000_000_000_000_000; // 18 decimals

// FixedPoint struct for precise calculations
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, Ord)]
pub struct FixedPoint {
    pub value: u128,
}

impl FixedPoint {
    pub fn from_u64(n: u64) -> Self {
        Self {
            value: (n as u128) * PRECISION,
        }
    }

    pub fn from_i64(n: i64) -> Self {
        if n >= 0 {
            Self::from_u64(n as u64)
        } else {
            Self {
                value: 0, // Handle negative values appropriately
            }
        }
    }

    pub fn from_float(f: f64) -> Self {
        Self {
            value: (f * PRECISION as f64) as u128,
        }
    }

    pub fn to_u64_truncate(&self) -> u64 {
        (self.value / PRECISION) as u64
    }

    pub fn to_float(&self) -> f64 {
        self.value as f64 / PRECISION as f64
    }

    pub fn add(&self, other: &Self) -> Result<Self> {
        Ok(Self {
            value: add(self.value, other.value)?,
        })
    }

    pub fn sub(&self, other: &Self) -> Result<Self> {
        Ok(Self {
            value: sub(self.value, other.value)?,
        })
    }

    pub fn mul(&self, other: &Self) -> Result<Self> {
        Ok(Self {
            value: mul(self.value, other.value)?,
        })
    }

    pub fn div(&self, other: &Self) -> Result<Self> {
        Ok(Self {
            value: div(self.value, other.value)?,
        })
    }

    pub fn exp(&self) -> Result<Self> {
        Ok(Self {
            value: exp(self.value)?,
        })
    }

    pub fn log(&self) -> Result<Self> {
        Ok(Self {
            value: log(self.value)?,
        })
    }

    pub fn pow(&self, exp: u32) -> Result<Self> {
        Ok(Self {
            value: pow(self.value, exp)?,
        })
    }

    pub fn sqrt(&self) -> Result<Self> {
        Ok(Self {
            value: sqrt(self.value)?,
        })
    }
    
    pub fn ln(&self) -> Result<Self> {
        Ok(Self {
            value: log(self.value)?,
        })
    }
    
    pub fn abs(&self) -> Result<Self> {
        Ok(*self)
    }
    
    pub fn neg(&self) -> Result<Self> {
        // For a positive-only implementation, negation returns zero
        Ok(Self { value: 0 })
    }
    
    pub fn min(&self, other: &Self) -> Result<Self> {
        Ok(if self.value < other.value { *self } else { *other })
    }
    
    pub fn max(&self, other: &Self) -> Result<Self> {
        Ok(if self.value > other.value { *self } else { *other })
    }
    
    pub fn zero() -> Self {
        Self { value: 0 }
    }
    
    pub fn from_raw(value: u64) -> Self {
        Self { value: value as u128 }
    }
    
    pub fn to_raw(&self) -> u64 {
        self.value as u64
    }
}

impl std::ops::Add for FixedPoint {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            value: self.value.saturating_add(other.value),
        }
    }
}

impl std::ops::Div for FixedPoint {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        Self {
            value: self.value.saturating_mul(PRECISION) / other.value,
        }
    }
}

impl PartialOrd<Self> for FixedPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

// Helper function for integer square root
pub fn integer_sqrt(n: u128) -> Result<u128> {
    if n == 0 {
        return Ok(0);
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    Ok(x)
}

pub fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b)
        .and_then(|result| result.checked_div(PRECISION))
        .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))
}

pub fn div(a: u128, b: u128) -> Result<u128> {
    require!(b != 0, crate::errors::ErrorCode::DivisionByZero);
    a.checked_mul(PRECISION)
        .and_then(|result| result.checked_div(b))
        .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))
}

pub fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b)
        .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))
}

pub fn sub(a: u128, b: u128) -> Result<u128> {
    a.checked_sub(b)
        .ok_or(error!(crate::errors::ErrorCode::ArithmeticUnderflow))
}

pub fn exp(x: u128) -> Result<u128> {
    // Taylor series approximation
    let mut result = PRECISION;
    let mut term = PRECISION;
    
    for i in 1..20 {
        term = mul(term, x)?;
        term = term.checked_div(i as u128)
            .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))?;
        result = add(result, term)?;
        
        // Break early if term becomes negligible
        if term < 1000 {
            break;
        }
    }
    
    Ok(result)
}

pub fn sqrt(x: u128) -> Result<u128> {
    // For sqrt(x) where x is in fixed-point format (scaled by PRECISION),
    // we want: sqrt(x * PRECISION) / sqrt(PRECISION) = sqrt(x)
    // But since x is already scaled by PRECISION, we have:
    // sqrt(x_scaled) * sqrt(PRECISION) / PRECISION
    
    if x == 0 { 
        return Ok(0); 
    }
    
    // First compute the integer square root of x
    let x_sqrt = integer_sqrt(x)?;
    
    // Then scale it properly: multiply by sqrt(PRECISION)
    // Since PRECISION = 10^18, sqrt(PRECISION) = 10^9
    let scale_factor = 1_000_000_000u128; // sqrt(10^18) = 10^9
    
    x_sqrt.checked_mul(scale_factor)
        .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))
}

pub fn log(x: u128) -> Result<u128> {
    // Natural logarithm using series expansion
    // ln(x) = ln(1 + y) where y = x - 1
    require!(x > 0, crate::errors::ErrorCode::InvalidInput);
    
    if x == PRECISION {
        return Ok(0); // ln(1) = 0
    }
    
    // For x > 2, use: ln(x) = ln(x/2) + ln(2)
    if x > 2 * PRECISION {
        let half_x = x.checked_div(2)
            .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))?;
        let ln_half_x = log(half_x)?;
        let ln_2 = 693147180559945309u128; // ln(2) * PRECISION
        return add(ln_half_x, ln_2);
    }
    
    // For x < 0.5, use: ln(x) = ln(2x) - ln(2)
    if x < PRECISION / 2 {
        let double_x = x.checked_mul(2)
            .ok_or(error!(crate::errors::ErrorCode::ArithmeticOverflow))?;
        let ln_double_x = log(double_x)?;
        let ln_2 = 693147180559945309u128; // ln(2) * PRECISION
        return sub(ln_double_x, ln_2);
    }
    
    // Taylor series for ln(1 + y) where y = (x - PRECISION) / PRECISION
    let y = sub(x, PRECISION)?;
    let mut result = 0u128;
    let mut term = y;
    let mut sign = true;
    
    for i in 1..50 {
        if sign {
            result = add(result, div(term, i as u128 * PRECISION)?)?;
        } else {
            result = sub(result, div(term, i as u128 * PRECISION)?)?;
        }
        
        term = mul(term, y)?;
        sign = !sign;
        
        // Break early if term becomes negligible
        if term < 1000 {
            break;
        }
    }
    
    Ok(result)
}

pub fn pow(base: u128, exponent: u32) -> Result<u128> {
    // Simple exponentiation for integer powers
    let mut result = PRECISION;
    let mut current_base = base;
    let mut exp = exponent;
    
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul(result, current_base)?;
        }
        current_base = mul(current_base, current_base)?;
        exp >>= 1;
    }
    
    Ok(result)
}

// Error function approximation using Abramowitz and Stegun approximation
pub fn erf_approximation(x: f64) -> Result<f64> {
    // erf(x) = 1 - (a1*t + a2*t^2 + a3*t^3 + a4*t^4 + a5*t^5) * exp(-x^2)
    // where t = 1 / (1 + p*x)
    
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let x = x.abs();
    
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    
    Ok(sign * y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_point_math() {
        let a = 2 * PRECISION;
        let b = 3 * PRECISION;

        assert_eq!(mul(a, b).unwrap(), 6 * PRECISION);
        assert_eq!(div(a, b).unwrap(), 666_666_666_666_666_666);
        assert_eq!(add(a, b).unwrap(), 5 * PRECISION);
        assert_eq!(sub(b, a).unwrap(), PRECISION);
    }

    #[test]
    fn test_sqrt() {
        let x = 4 * PRECISION;
        let result = sqrt(x).unwrap();
        let expected = 2 * PRECISION;
        let diff = if result > expected { result - expected } else { expected - result };
        // Increase tolerance for the approximation
        assert!(diff < PRECISION / 1000, "sqrt(4) should be approximately 2, got {} expected {}", result, expected);
    }
    
    #[test]
    fn test_exp() {
        // Test e^0 = 1
        let result = exp(0).unwrap();
        assert_eq!(result, PRECISION);
        
        // Test e^1 ≈ 2.71828...
        let result = exp(PRECISION).unwrap();
        let expected = 2718281828459045235u128; // e * PRECISION
        let diff = if result > expected { result - expected } else { expected - result };
        assert!(diff < PRECISION / 1000, "e^1 should be approximately 2.71828...");
    }
    
    #[test]
    fn test_log() {
        // Test ln(1) = 0
        let result = log(PRECISION).unwrap();
        assert_eq!(result, 0);
        
        // Test ln(e) = 1
        let e = 2718281828459045235u128; // e * PRECISION
        let result = log(e).unwrap();
        let diff = if result > PRECISION { result - PRECISION } else { PRECISION - result };
        assert!(diff < PRECISION / 100, "ln(e) should be approximately 1");
    }
}