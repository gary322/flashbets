//! Special Mathematical Functions using Precomputed Tables
//! 
//! Implements Black-Scholes, VaR, and other financial calculations
//! Uses CDF/PDF tables for efficient on-chain computation

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::math::{
    U64F64,
    tables::NormalDistributionTables,
    table_lookup::{lookup_cdf, lookup_pdf, lookup_inverse_cdf},
};
use crate::BettingPlatformError;

/// Black-Scholes call option pricing using precomputed tables
pub fn black_scholes_call(
    tables: &NormalDistributionTables,
    spot: U64F64,
    strike: U64F64,
    time_to_expiry: U64F64,
    volatility: U64F64,
    risk_free_rate: U64F64,
) -> Result<U64F64, ProgramError> {
    // Handle expired option
    if time_to_expiry.raw <= 0 {
        // Intrinsic value: max(S - K, 0)
        if spot.raw > strike.raw {
            return spot.checked_sub(strike);
        } else {
            return Ok(U64F64::from_num(0));
        }
    }
    
    // Calculate volatility * sqrt(T)
    let vol_sqrt_t = volatility.checked_mul(time_to_expiry.sqrt()?)?;
    
    // Prevent division by zero
    if vol_sqrt_t.is_zero() {
        msg!("Zero volatility or time");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Calculate d1 and d2
    // d1 = (ln(S/K) + (r + σ²/2)T) / (σ√T)
    
    // First, calculate ln(S/K) using approximation
    let s_over_k = spot.checked_div(strike)?;
    let ln_s_over_k = approximate_ln(s_over_k)?;
    
    // Calculate (r + σ²/2)T
    let vol_squared = volatility.checked_mul(volatility)?;
    let vol_squared_half = vol_squared.checked_div(U64F64::from_num(2))?;
    let drift = risk_free_rate.checked_add(vol_squared_half)?;
    let drift_t = drift.checked_mul(time_to_expiry)?;
    
    // d1 = (ln(S/K) + drift*T) / vol_sqrt_t
    let numerator = ln_s_over_k.checked_add(drift_t)?;
    let d1 = numerator.checked_div(vol_sqrt_t)?;
    
    // d2 = d1 - σ√T
    let d2 = d1.checked_sub(vol_sqrt_t)?;
    
    // Look up N(d1) and N(d2) from tables
    let n_d1 = lookup_cdf(tables, d1)?;
    let n_d2 = lookup_cdf(tables, d2)?;
    
    // Calculate discount factor: e^(-rT)
    let neg_rt = U64F64::from_num(0).checked_sub(risk_free_rate.checked_mul(time_to_expiry)?)?;
    let discount = approximate_exp(neg_rt)?;
    
    // Call price: C = S*N(d1) - K*e^(-rT)*N(d2)
    let first_term = spot.checked_mul(n_d1)?;
    let second_term = strike.checked_mul(discount)?.checked_mul(n_d2)?;
    
    if first_term.raw > second_term.raw {
        first_term.checked_sub(second_term)
    } else {
        Ok(U64F64::from_num(0)) // Option has no value
    }
}

/// Black-Scholes put option pricing
pub fn black_scholes_put(
    tables: &NormalDistributionTables,
    spot: U64F64,
    strike: U64F64,
    time_to_expiry: U64F64,
    volatility: U64F64,
    risk_free_rate: U64F64,
) -> Result<U64F64, ProgramError> {
    // Put-Call parity: P = C - S + K*e^(-rT)
    let call_price = black_scholes_call(tables, spot, strike, time_to_expiry, volatility, risk_free_rate)?;
    
    // Calculate K*e^(-rT)
    let neg_rt = U64F64::from_num(0).checked_sub(risk_free_rate.checked_mul(time_to_expiry)?)?;
    let discounted_strike = strike.checked_mul(approximate_exp(neg_rt)?)?;
    
    // P = C - S + K*e^(-rT)
    let put_value = call_price.checked_add(discounted_strike)?;
    
    if put_value.raw > spot.raw {
        put_value.checked_sub(spot)
    } else {
        Ok(U64F64::from_num(0))
    }
}

/// Calculate Value at Risk (VaR) using normal distribution
pub fn calculate_var(
    tables: &NormalDistributionTables,
    portfolio_value: U64F64,
    volatility: U64F64,
    confidence_level: U64F64,
    time_horizon: U64F64,
) -> Result<U64F64, ProgramError> {
    // VaR = portfolio_value * volatility * Φ^(-1)(1 - confidence_level) * √time_horizon
    
    // Calculate 1 - confidence_level (e.g., 0.05 for 95% confidence)
    let one = U64F64::from_num(1);
    let alpha = one.checked_sub(confidence_level)?;
    
    // Get quantile from inverse CDF
    let quantile = lookup_inverse_cdf(tables, alpha)?;
    
    // Since we want the loss (negative return), we need the absolute value
    let quantile_abs = if quantile.raw < U64F64::from_num(0).raw {
        U64F64::from_num(0).checked_sub(quantile)?
    } else {
        quantile
    };
    
    // Calculate VaR
    let sqrt_time = time_horizon.sqrt()?;
    let var = portfolio_value
        .checked_mul(volatility)?
        .checked_mul(quantile_abs)?
        .checked_mul(sqrt_time)?;
    
    Ok(var)
}

/// Calculate VaR with specific formula: -deposit * norm.ppf(0.05) * sigma * sqrt(time)
/// Specification: For deposit=100, sigma=0.2, time=1, result should be -32.9
pub fn calculate_var_specific(
    tables: &NormalDistributionTables,
    deposit: U64F64,
    sigma: U64F64,
    time: U64F64,
) -> Result<U64F64, ProgramError> {
    // Get quantile for 5% (0.05) probability
    let alpha = U64F64::from_num(500u64) / U64F64::from_num(10000u64); // 0.05
    let quantile = lookup_inverse_cdf(tables, alpha)?;
    
    // quantile will be negative (around -1.645 for 5%)
    // We want the result as a positive VaR value
    let quantile_abs = if quantile.raw < U64F64::from_num(0).raw {
        U64F64::from_num(0).checked_sub(quantile)?
    } else {
        quantile
    };
    
    // Calculate VaR = deposit * |quantile| * sigma * sqrt(time)
    let sqrt_time = time.sqrt()?;
    let var = deposit
        .checked_mul(quantile_abs)?
        .checked_mul(sigma)?
        .checked_mul(sqrt_time)?;
    
    // Verify with example: deposit=100, sigma=0.2, time=1
    // quantile for 0.05 ≈ -1.645, so |quantile| ≈ 1.645
    // VaR = 100 * 1.645 * 0.2 * 1 = 32.9
    
    Ok(var)
}

/// Calculate Greeks for options using tables
pub struct Greeks {
    pub delta: U64F64,
    pub gamma: U64F64,
    pub vega: U64F64,
    pub theta: U64F64,
    pub rho: U64F64,
}

/// Calculate option Greeks
pub fn calculate_greeks(
    tables: &NormalDistributionTables,
    spot: U64F64,
    strike: U64F64,
    time_to_expiry: U64F64,
    volatility: U64F64,
    risk_free_rate: U64F64,
    is_call: bool,
) -> Result<Greeks, ProgramError> {
    // Calculate common parameters
    let vol_sqrt_t = volatility.checked_mul(time_to_expiry.sqrt()?)?;
    
    if vol_sqrt_t.is_zero() {
        return Err(ProgramError::InvalidArgument);
    }
    
    // Calculate d1 and d2 (same as Black-Scholes)
    let s_over_k = spot.checked_div(strike)?;
    let ln_s_over_k = approximate_ln(s_over_k)?;
    
    let vol_squared = volatility.checked_mul(volatility)?;
    let vol_squared_half = vol_squared.checked_div(U64F64::from_num(2))?;
    let drift = risk_free_rate.checked_add(vol_squared_half)?;
    let drift_t = drift.checked_mul(time_to_expiry)?;
    
    let numerator = ln_s_over_k.checked_add(drift_t)?;
    let d1 = numerator.checked_div(vol_sqrt_t)?;
    let d2 = d1.checked_sub(vol_sqrt_t)?;
    
    // Look up values from tables
    let n_d1 = lookup_cdf(tables, d1)?;
    let n_d2 = lookup_cdf(tables, d2)?;
    let phi_d1 = lookup_pdf(tables, d1)?;
    
    // Calculate discount factor
    let neg_rt = U64F64::from_num(0).checked_sub(risk_free_rate.checked_mul(time_to_expiry)?)?;
    let discount = approximate_exp(neg_rt)?;
    
    // Delta: ∂C/∂S = N(d1) for call, N(d1) - 1 for put
    let delta = if is_call {
        n_d1
    } else {
        n_d1.checked_sub(U64F64::from_num(1))?
    };
    
    // Gamma: ∂²C/∂S² = φ(d1) / (S * σ * √T)
    let denominator = spot.checked_mul(vol_sqrt_t)?;
    let gamma = phi_d1.checked_div(denominator)?;
    
    // Vega: ∂C/∂σ = S * φ(d1) * √T
    let vega = spot.checked_mul(phi_d1)?.checked_mul(time_to_expiry.sqrt()?)?;
    
    // Theta: ∂C/∂T
    let theta_first = spot.checked_mul(phi_d1)?.checked_mul(volatility)?
        .checked_div(U64F64::from_num(2).checked_mul(time_to_expiry.sqrt()?)?)?;
    
    let theta_second = risk_free_rate.checked_mul(strike)?
        .checked_mul(discount)?.checked_mul(n_d2)?;
    
    let theta = if is_call {
        U64F64::from_num(0).checked_sub(theta_first.checked_add(theta_second)?)?
    } else {
        U64F64::from_num(0).checked_sub(theta_first.checked_sub(theta_second)?)?
    };
    
    // Rho: ∂C/∂r = K * T * e^(-rT) * N(d2) for call
    let rho = if is_call {
        strike.checked_mul(time_to_expiry)?.checked_mul(discount)?.checked_mul(n_d2)?
    } else {
        U64F64::from_num(0).checked_sub(
            strike.checked_mul(time_to_expiry)?.checked_mul(discount)?
                .checked_mul(U64F64::from_num(1).checked_sub(n_d2)?)?
        )?
    };
    
    Ok(Greeks {
        delta,
        gamma,
        vega,
        theta,
        rho,
    })
}

/// Probability of touch calculation for binary options
pub fn probability_of_touch(
    tables: &NormalDistributionTables,
    spot: U64F64,
    barrier: U64F64,
    time_to_expiry: U64F64,
    volatility: U64F64,
) -> Result<U64F64, ProgramError> {
    // For a barrier option, probability of touch = 2 * N(|ln(B/S)| / (σ√T))
    
    let b_over_s = barrier.checked_div(spot)?;
    let ln_b_over_s = approximate_ln(b_over_s)?;
    
    // Take absolute value
    let ln_abs = if ln_b_over_s.raw < U64F64::from_num(0).raw {
        U64F64::from_num(0).checked_sub(ln_b_over_s)?
    } else {
        ln_b_over_s
    };
    
    let vol_sqrt_t = volatility.checked_mul(time_to_expiry.sqrt()?)?;
    let z = ln_abs.checked_div(vol_sqrt_t)?;
    
    // Look up N(z)
    let n_z = lookup_cdf(tables, z)?;
    
    // Probability of touch = 2 * N(z) - 1
    let two = U64F64::from_num(2);
    let prob = two.checked_mul(n_z)?.checked_sub(U64F64::from_num(1))?;
    
    // Ensure probability is in [0, 1]
    if prob.raw < U64F64::from_num(0).raw {
        Ok(U64F64::from_num(0))
    } else if prob.raw > U64F64::from_num(1).raw {
        Ok(U64F64::from_num(1))
    } else {
        Ok(prob)
    }
}

/// Approximate natural logarithm for fixed-point
/// Uses Taylor series expansion around 1
pub fn approximate_ln(x: U64F64) -> Result<U64F64, ProgramError> {
    if x.raw <= 0 {
        msg!("Cannot take ln of non-positive number");
        return Err(ProgramError::InvalidArgument);
    }
    
    // For x close to 1, use Taylor series: ln(1+y) ≈ y - y²/2 + y³/3 - ...
    // Otherwise, use: ln(x) = ln(x/2^n) + n*ln(2)
    
    let one = U64F64::from_num(1);
    
    // If x is close to 1 (between 0.5 and 2), use Taylor series directly
    if x.raw >= U64F64::from_fraction(1, 2).map_err(|_| BettingPlatformError::ArithmeticOverflow)?.raw && x.raw <= U64F64::from_num(2).raw {
        let y = x.checked_sub(one)?;
        
        // Calculate terms
        let y2 = y.checked_mul(y)?;
        let y3 = y2.checked_mul(y)?;
        let y4 = y3.checked_mul(y)?;
        
        // ln(1+y) ≈ y - y²/2 + y³/3 - y⁴/4
        let term1 = y;
        let term2 = y2.checked_div(U64F64::from_num(2))?;
        let term3 = y3.checked_div(U64F64::from_num(3))?;
        let term4 = y4.checked_div(U64F64::from_num(4))?;
        
        let result = term1.checked_sub(term2)?
            .checked_add(term3)?
            .checked_sub(term4)?;
        
        return Ok(result);
    }
    
    // For larger values, normalize to [0.5, 1] range
    let mut normalized = x;
    let mut exponent = 0i32;
    
    // Divide by 2 until in range
    while normalized.raw > U64F64::from_num(2).raw {
        normalized = normalized.checked_div(U64F64::from_num(2))?;
        exponent += 1;
    }
    
    // Multiply by 2 until in range
    while normalized.raw < U64F64::from_fraction(1, 2).map_err(|_| BettingPlatformError::ArithmeticOverflow)?.raw {
        normalized = normalized.checked_mul(U64F64::from_num(2))?;
        exponent -= 1;
    }
    
    // Now use Taylor series on normalized value
    let y = normalized.checked_sub(one)?;
    let y2 = y.checked_mul(y)?;
    let y3 = y2.checked_mul(y)?;
    
    let ln_normalized = y.checked_sub(y2.checked_div(U64F64::from_num(2))?)?
        .checked_add(y3.checked_div(U64F64::from_num(3))?)?;
    
    // Add back the exponent part: result = ln(normalized) + exponent * ln(2)
    let ln2 = U64F64::from_raw(0xB17217F7D1CF79AB); // ln(2) ≈ 0.693147 in 64.64 format
    
    if exponent >= 0 {
        let exp_term = ln2.checked_mul(U64F64::from_num(exponent as u64))?;
        ln_normalized.checked_add(exp_term)
    } else {
        let exp_term = ln2.checked_mul(U64F64::from_num((-exponent) as u64))?;
        ln_normalized.checked_sub(exp_term)
    }
}

/// Approximate exponential function
/// Uses Taylor series: e^x = 1 + x + x²/2! + x³/3! + ...
pub fn approximate_exp(x: U64F64) -> Result<U64F64, ProgramError> {
    // Handle large values to prevent overflow
    if x.raw > U64F64::from_num(10).raw {
        msg!("Exponential argument too large");
        return Err(BettingPlatformError::ArithmeticOverflow.into());
    }
    
    // Since U64F64 is unsigned, we can't have negative values
    // This check is not needed for unsigned types
    
    let one = U64F64::from_num(1);
    let mut result = one;
    let mut term = one;
    let mut n = U64F64::from_num(1);
    
    // Calculate up to 10 terms for good accuracy
    for i in 1..=10 {
        // term = term * x / i
        term = term.checked_mul(x)?.checked_div(n)?;
        
        // Add term to result
        let new_result = result.checked_add(term)?;
        
        // Check for convergence
        if new_result.raw.saturating_sub(result.raw) < 100 {
            break;
        }
        
        result = new_result;
        n = U64F64::from_num(i + 1);
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approximate_ln() {
        // Test ln(1) = 0
        let one = U64F64::from_num(1);
        let ln_one = approximate_ln(one).unwrap();
        assert!(ln_one.raw < U64F64::from_fraction(1, 1000).unwrap().raw);
        
        // Test ln(e) ≈ 1
        let e = U64F64::from_fraction(2718, 1000).unwrap(); // e ≈ 2.718
        let ln_e = approximate_ln(e).unwrap();
        let diff = if ln_e.raw > one.raw {
            ln_e.checked_sub(one).unwrap()
        } else {
            one.checked_sub(ln_e).unwrap()
        };
        assert!(diff.raw < U64F64::from_fraction(1, 10).unwrap().raw);
    }

    #[test]
    fn test_approximate_exp() {
        // Test e^0 = 1
        let zero = U64F64::from_num(0);
        let exp_zero = approximate_exp(zero).unwrap();
        assert_eq!(exp_zero.to_num(), 1);
        
        // Test e^1 ≈ 2.718
        let one = U64F64::from_num(1);
        let exp_one = approximate_exp(one).unwrap();
        assert!(exp_one.to_num() >= 2);
        assert!(exp_one.to_num() <= 3);
    }

    #[test]
    fn test_black_scholes_edge_cases() {
        // Test expired option (would need initialized tables for full test)
        let tables = NormalDistributionTables {
            discriminator: NormalDistributionTables::DISCRIMINATOR,
            is_initialized: false,
            version: 1,
            min_x: -400,
            max_x: 400,
            step: 1,
            table_size: 801,
            cdf_table: vec![],
            pdf_table: vec![],
            erf_table: vec![],
        };
        
        let spot = U64F64::from_num(100);
        let strike = U64F64::from_num(90);
        let expired = U64F64::from_num(0);
        let vol = U64F64::from_fraction(20, 100).unwrap();
        let rate = U64F64::from_fraction(5, 100).unwrap();
        
        // Expired ITM call should return intrinsic value
        let call_value = black_scholes_call(&tables, spot, strike, expired, vol, rate).unwrap();
        assert_eq!(call_value.to_num(), 10);
    }
}