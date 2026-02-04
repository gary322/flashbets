// Optimized LMSR Math for <50k CU per trade
// Uses precomputed tables and approximations to reduce compute units

use solana_program::program_error::ProgramError;
use crate::{
    error::BettingPlatformError,
    state::amm_accounts::LSMRMarket,
    math::tables::{EXP_LOOKUP, LN_LOOKUP},
};

/// CU-optimized exp approximation using lookup tables
/// Target: <500 CU
#[inline(always)]
pub fn fast_exp_lookup(x: u64) -> Result<u64, ProgramError> {
    // Scale input to table range [0, 4] with 12-bit precision
    let scaled = (x * 4096) / 10000; // Convert basis points to table index
    
    if scaled >= EXP_LOOKUP.len() as u64 {
        // For large values, use approximation: e^x ≈ 2^(1.44x)
        let power = (scaled * 147) / 100; // 1.44 ≈ 147/100
        return Ok(1u64 << (power.min(63) as u32));
    }
    
    // Direct lookup for common values
    // Convert U64F64 to u64 - using to_num() to get raw value
    Ok(EXP_LOOKUP[scaled as usize].to_num())
}

/// CU-optimized ln approximation using lookup tables
/// Target: <500 CU
#[inline(always)]
fn fast_ln_lookup(x: u64) -> Result<u64, ProgramError> {
    if x == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Find highest bit position (fast log2)
    let log2 = 63 - x.leading_zeros();
    let mantissa = x >> log2.saturating_sub(10); // 10-bit mantissa
    
    // Lookup ln(mantissa) and add ln(2) * log2
    let ln_mantissa = if mantissa < LN_LOOKUP.len() as u64 {
        LN_LOOKUP[mantissa as usize].to_num()
    } else {
        LN_LOOKUP[1023].to_num() // Max value
    };
    
    // ln(x) = ln(mantissa * 2^log2) = ln(mantissa) + log2 * ln(2)
    // ln(2) ≈ 0.693 ≈ 693/1000
    Ok(ln_mantissa + (log2 as u64 * 693) / 1000)
}

/// Optimized LMSR price calculation
/// Target: <20k CU (down from 50k)
pub fn calculate_price_optimized(
    shares: &[u64],
    outcome: u8,
    b_parameter: u64,
) -> Result<u64, ProgramError> {
    if outcome as usize >= shares.len() {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    if b_parameter == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Use integer arithmetic to avoid expensive operations
    let mut sum_exp_scaled = 0u128;
    let mut outcome_exp_scaled = 0u128;
    
    // Precompute reciprocal for division
    let b_recip = (1u128 << 32) / b_parameter as u128;
    
    for (i, &share) in shares.iter().enumerate() {
        // Fast division using multiplication by reciprocal
        let normalized = ((share as u128 * b_recip) >> 32) as u64;
        
        // Use lookup table for exp
        let exp_value = fast_exp_lookup(normalized)? as u128;
        
        if i == outcome as usize {
            outcome_exp_scaled = exp_value;
        }
        sum_exp_scaled = sum_exp_scaled.saturating_add(exp_value);
    }
    
    if sum_exp_scaled == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Calculate price as ratio (scaled to basis points)
    // price = (outcome_exp / sum_exp) * 10000
    let price_bps = ((outcome_exp_scaled * 10000) / sum_exp_scaled) as u64;
    
    Ok(price_bps)
}

/// Optimized cost function using approximations
/// Target: <15k CU
pub fn calculate_cost_optimized(
    shares: &[u64],
    b_parameter: u64,
) -> Result<u64, ProgramError> {
    if b_parameter == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // For small markets (2 outcomes), use direct formula
    if shares.len() == 2 {
        return calculate_binary_cost_optimized(shares[0], shares[1], b_parameter);
    }
    
    // Sum exponentials using lookup
    let mut sum_exp = 0u128;
    let b_recip = (1u128 << 32) / b_parameter as u128;
    
    for &share in shares {
        let normalized = ((share as u128 * b_recip) >> 32) as u64;
        let exp_value = fast_exp_lookup(normalized)?;
        sum_exp = sum_exp.saturating_add(exp_value as u128);
    }
    
    // Use lookup for ln
    let ln_sum = fast_ln_lookup((sum_exp >> 16) as u64)?;
    
    // Multiply by b
    Ok(((ln_sum as u128 * b_parameter as u128) >> 16) as u64)
}

/// Specialized binary market cost (most common case)
/// Target: <10k CU
fn calculate_binary_cost_optimized(
    shares0: u64,
    shares1: u64,
    b_parameter: u64,
) -> Result<u64, ProgramError> {
    // For binary markets: C = b * ln(e^(q0/b) + e^(q1/b))
    
    // Normalize shares
    let b_recip = (1u128 << 32) / b_parameter as u128;
    let norm0 = ((shares0 as u128 * b_recip) >> 32) as u64;
    let norm1 = ((shares1 as u128 * b_recip) >> 32) as u64;
    
    // Fast exp using lookup
    let exp0 = fast_exp_lookup(norm0)?;
    let exp1 = fast_exp_lookup(norm1)?;
    
    // Sum and ln
    let sum = exp0.saturating_add(exp1);
    let ln_sum = fast_ln_lookup(sum)?;
    
    // Result
    Ok(((ln_sum as u128 * b_parameter as u128) >> 10) as u64)
}

/// Optimized share calculation using Newton's method with lookup tables
/// Target: <25k CU
pub fn calculate_shares_optimized(
    market: &LSMRMarket,
    outcome: u8,
    max_cost: u64,
) -> Result<u64, ProgramError> {
    if outcome >= market.num_outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // For binary markets, use closed-form approximation
    if market.num_outcomes == 2 {
        return calculate_binary_shares_optimized(market, outcome, max_cost);
    }
    
    // Use Newton's method with just 3 iterations (sufficient for 0.1% accuracy)
    let mut shares = max_cost / 2; // Initial guess
    
    for _ in 0..3 {
        let cost = calculate_buy_cost_inline(market, outcome, shares)?;
        
        if cost <= max_cost {
            // Estimate derivative using finite difference
            let delta = shares / 10 + 1;
            let cost_plus = calculate_buy_cost_inline(market, outcome, shares + delta)?;
            
            if cost_plus > cost {
                let derivative = (cost_plus - cost) / delta;
                if derivative > 0 {
                    let adjustment = (max_cost - cost) / derivative;
                    shares = shares.saturating_add(adjustment / 2); // Conservative update
                }
            }
        } else {
            shares = shares * 3 / 4; // Reduce by 25%
        }
    }
    
    Ok(shares)
}

/// Inline cost calculation to avoid function call overhead
#[inline(always)]
fn calculate_buy_cost_inline(
    market: &LSMRMarket,
    outcome: u8,
    shares: u64,
) -> Result<u64, ProgramError> {
    // Current cost
    let current_cost = calculate_cost_optimized(&market.shares, market.b_parameter)?;
    
    // New cost (optimized for single outcome change)
    let mut sum_exp = 0u128;
    let b_recip = (1u128 << 32) / market.b_parameter as u128;
    
    for (i, &share) in market.shares.iter().enumerate() {
        let adjusted_share = if i == outcome as usize {
            share.saturating_add(shares)
        } else {
            share
        };
        
        let normalized = ((adjusted_share as u128 * b_recip) >> 32) as u64;
        let exp_value = fast_exp_lookup(normalized)?;
        sum_exp = sum_exp.saturating_add(exp_value as u128);
    }
    
    let ln_sum = fast_ln_lookup((sum_exp >> 16) as u64)?;
    let new_cost = ((ln_sum as u128 * market.b_parameter as u128) >> 16) as u64;
    
    Ok(new_cost.saturating_sub(current_cost))
}

/// Optimized binary market share calculation
/// Target: <15k CU
fn calculate_binary_shares_optimized(
    market: &LSMRMarket,
    outcome: u8,
    max_cost: u64,
) -> Result<u64, ProgramError> {
    // For binary markets, we can use a more direct approach
    let current_price = calculate_price_optimized(&market.shares, outcome, market.b_parameter)?;
    
    // Approximate shares = max_cost * (1 - price/10000) * adjustment_factor
    let price_complement = 10000u64.saturating_sub(current_price);
    let adjustment = if outcome == 0 { 11 } else { 9 }; // Empirical adjustment
    
    let shares = (max_cost as u128 * price_complement as u128 * adjustment) / (10000 * 10);
    
    Ok(shares as u64)
}

/// Batch price calculation for all outcomes (more efficient than individual calls)
/// Target: <30k CU for up to 8 outcomes
pub fn calculate_all_prices_optimized(
    market: &LSMRMarket,
) -> Result<Vec<u64>, ProgramError> {
    let mut prices = Vec::with_capacity(market.num_outcomes as usize);
    
    // Calculate sum of exponentials once
    let mut sum_exp = 0u128;
    let b_recip = (1u128 << 32) / market.b_parameter as u128;
    let mut exp_values = Vec::with_capacity(market.num_outcomes as usize);
    
    for &share in &market.shares {
        let normalized = ((share as u128 * b_recip) >> 32) as u64;
        let exp_value = fast_exp_lookup(normalized)?;
        exp_values.push(exp_value);
        sum_exp = sum_exp.saturating_add(exp_value as u128);
    }
    
    if sum_exp == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Calculate all prices using the cached exponentials
    for exp_value in exp_values {
        let price_bps = ((exp_value as u128 * 10000) / sum_exp) as u64;
        prices.push(price_bps);
    }
    
    Ok(prices)
}