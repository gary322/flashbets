//! LMSR mathematical functions
//!
//! Core math for Logarithmic Market Scoring Rule

use solana_program::program_error::ProgramError;
use crate::{
    error::BettingPlatformError,
    math::U64F64,
    state::amm_accounts::LSMRMarket,
};

/// Calculate the cost function C(q) = b * ln(Σ e^(q_i/b))
pub fn calculate_cost_function(
    shares: &[u64],
    b_parameter: u64,
) -> Result<u64, ProgramError> {
    if b_parameter == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Calculate sum of exponentials
    let mut sum_exp = U64F64::from_num(0);
    
    for &share in shares {
        let exponent = U64F64::from_fraction(share, b_parameter)?;
        let exp_value = exponent.exp()?;
        sum_exp = sum_exp.checked_add(exp_value)?;
    }
    
    // Calculate ln(sum_exp)
    let ln_sum = sum_exp.ln()?;
    
    // Multiply by b
    let b_fixed = U64F64::from_num(b_parameter);
    let cost = b_fixed.checked_mul(ln_sum)?;
    
    Ok(cost.to_num())
}

/// Calculate price for outcome i: p_i = e^(q_i/b) / Σ e^(q_j/b)
pub fn calculate_price(
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
    
    // Calculate exponentials
    let mut sum_exp = U64F64::from_num(0);
    let mut outcome_exp = U64F64::from_num(0);
    
    for (i, &share) in shares.iter().enumerate() {
        let exponent = U64F64::from_fraction(share, b_parameter)?;
        let exp_value = exponent.exp()?;
        
        if i == outcome as usize {
            outcome_exp = exp_value;
        }
        sum_exp = sum_exp.checked_add(exp_value)?;
    }
    
    if sum_exp.is_zero() {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Calculate price as ratio (scaled to basis points)
    let price_ratio = outcome_exp.checked_div(sum_exp)?;
    let price_bps = price_ratio.checked_mul(U64F64::from_num(10000))?;
    
    Ok(price_bps.to_num())
}

/// Validate that all LMSR probabilities sum to 1.0 (10000 basis points)
pub fn validate_probability_sum(
    shares: &[u64],
    b_parameter: u64,
) -> Result<(), ProgramError> {
    if shares.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if b_parameter == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Calculate sum of all probabilities
    let mut total_probability = 0u64;
    
    for i in 0..shares.len() {
        let price = calculate_price(shares, i as u8, b_parameter)?;
        total_probability = total_probability.saturating_add(price);
    }
    
    // Allow small tolerance for rounding errors (±10 basis points)
    const TOLERANCE: u64 = 10;
    const TARGET_SUM: u64 = 10000; // 100% in basis points
    
    if total_probability < TARGET_SUM.saturating_sub(TOLERANCE) ||
       total_probability > TARGET_SUM.saturating_add(TOLERANCE) {
        return Err(BettingPlatformError::InvalidProbabilitySum.into());
    }
    
    Ok(())
}

/// Calculate shares to buy for a given cost
pub fn calculate_shares_to_buy(
    market: &LSMRMarket,
    outcome: u8,
    max_cost: u64,
) -> Result<u64, ProgramError> {
    if outcome >= market.num_outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Binary search for optimal shares
    let mut low = 0u64;
    let mut high = max_cost.saturating_mul(10); // Upper bound
    let mut best_shares = 0u64;
    
    while low <= high {
        let mid = low.saturating_add(high) / 2;
        
        // Calculate cost of buying 'mid' shares
        let mut new_shares = market.shares.clone();
        new_shares[outcome as usize] = new_shares[outcome as usize].saturating_add(mid);
        
        let old_cost = calculate_cost_function(&market.shares, market.b_parameter)?;
        let new_cost = calculate_cost_function(&new_shares, market.b_parameter)?;
        let cost_diff = new_cost.saturating_sub(old_cost);
        
        if cost_diff <= max_cost {
            best_shares = mid;
            low = mid.saturating_add(1);
        } else {
            if mid == 0 {
                break;
            }
            high = mid.saturating_sub(1);
        }
    }
    
    Ok(best_shares)
}

/// Calculate cost of buying specific shares
pub fn calculate_buy_cost(
    market: &LSMRMarket,
    outcome: u8,
    shares: u64,
) -> Result<u64, ProgramError> {
    if outcome >= market.num_outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Calculate current cost
    let current_cost = calculate_cost_function(&market.shares, market.b_parameter)?;
    
    // Calculate new cost after purchase
    let mut new_shares = market.shares.clone();
    new_shares[outcome as usize] = new_shares[outcome as usize].saturating_add(shares);
    let new_cost = calculate_cost_function(&new_shares, market.b_parameter)?;
    
    // Cost difference is what user pays
    Ok(new_cost.saturating_sub(current_cost))
}

/// Calculate payout from selling shares
pub fn calculate_sell_payout(
    market: &LSMRMarket,
    outcome: u8,
    shares: u64,
) -> Result<u64, ProgramError> {
    if outcome >= market.num_outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    if shares > market.shares[outcome as usize] {
        return Err(BettingPlatformError::InsufficientShares.into());
    }
    
    // Calculate current cost
    let current_cost = calculate_cost_function(&market.shares, market.b_parameter)?;
    
    // Calculate new cost after sale
    let mut new_shares = market.shares.clone();
    new_shares[outcome as usize] = new_shares[outcome as usize].saturating_sub(shares);
    let new_cost = calculate_cost_function(&new_shares, market.b_parameter)?;
    
    // Payout is the cost reduction
    Ok(current_cost.saturating_sub(new_cost))
}

/// Calculate implied probabilities for all outcomes
pub fn calculate_probabilities(
    market: &LSMRMarket,
) -> Result<Vec<u64>, ProgramError> {
    let mut probabilities = Vec::with_capacity(market.num_outcomes as usize);
    
    for i in 0..market.num_outcomes {
        let price = calculate_price(&market.shares, i, market.b_parameter)?;
        probabilities.push(price);
    }
    
    Ok(probabilities)
}

/// Calculate market depth (liquidity) at current state
pub fn calculate_market_depth(
    market: &LSMRMarket,
    price_impact_bps: u16,
) -> Result<u64, ProgramError> {
    // Market depth is the amount that can be traded with given price impact
    let mut total_depth = 0u64;
    
    for outcome in 0..market.num_outcomes {
        let current_price = calculate_price(&market.shares, outcome, market.b_parameter)?;
        
        // Binary search for shares that move price by price_impact_bps
        let mut low = 0u64;
        let mut high = market.b_parameter.saturating_mul(100);
        
        while low < high {
            let mid = low.saturating_add(high) / 2;
            
            let mut test_shares = market.shares.clone();
            test_shares[outcome as usize] = test_shares[outcome as usize].saturating_add(mid);
            
            let new_price = calculate_price(&test_shares, outcome, market.b_parameter)?;
            let price_change = if new_price > current_price {
                new_price.saturating_sub(current_price)
            } else {
                current_price.saturating_sub(new_price)
            };
            
            let price_impact = price_change.saturating_mul(10000) / current_price;
            
            if price_impact < price_impact_bps as u64 {
                low = mid.saturating_add(1);
            } else {
                high = mid;
            }
        }
        
        let depth_cost = calculate_buy_cost(market, outcome, low)?;
        total_depth = total_depth.saturating_add(depth_cost);
    }
    
    Ok(total_depth / market.num_outcomes as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_price_calculation() {
        let shares = vec![100, 100]; // Equal shares
        let b_parameter = 1000;
        
        let price0 = calculate_price(&shares, 0, b_parameter).unwrap();
        let price1 = calculate_price(&shares, 1, b_parameter).unwrap();
        
        // Should be approximately 50% each
        assert!((price0 as i64 - 5000).abs() < 100);
        assert!((price1 as i64 - 5000).abs() < 100);
    }
    
    #[test]
    fn test_cost_function() {
        let shares = vec![100, 200];
        let b_parameter = 1000;
        
        let cost = calculate_cost_function(&shares, b_parameter).unwrap();
        assert!(cost > 0);
    }
    
    #[test]
    fn test_validate_probability_sum() {
        // Test with equal shares - should sum to 1.0
        let shares = vec![100, 100, 100];
        let b_parameter = 1000;
        
        assert!(validate_probability_sum(&shares, b_parameter).is_ok());
        
        // Verify actual sum
        let mut sum = 0u64;
        for i in 0..shares.len() {
            sum += calculate_price(&shares, i as u8, b_parameter).unwrap();
        }
        assert!((sum as i64 - 10000).abs() <= 10); // Within tolerance
        
        // Test with different shares
        let shares2 = vec![0, 100, 200, 300];
        assert!(validate_probability_sum(&shares2, b_parameter).is_ok());
        
        // Test edge cases
        assert!(validate_probability_sum(&[], b_parameter).is_err()); // Empty shares
        assert!(validate_probability_sum(&shares, 0).is_err()); // Zero b parameter
    }
}