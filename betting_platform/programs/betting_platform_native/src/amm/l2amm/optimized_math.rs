// Optimized L2 AMM Math for <50k CU per trade
// Uses approximations and lookup tables for continuous distributions

use solana_program::program_error::ProgramError;
use crate::{
    error::BettingPlatformError,
    state::{
        amm_accounts::L2AMMMarket,
        l2_distribution_state::L2DistributionState as L2Distribution,
    },
    math::tables::{SQRT_LOOKUP, NORMAL_CDF_LOOKUP},
};

/// Fast integer square root using lookup table + Newton refinement
/// Target: <300 CU
#[inline(always)]
fn fast_sqrt(x: u64) -> u64 {
    if x < SQRT_LOOKUP.len() as u64 {
        return SQRT_LOOKUP[x as usize].to_num();
    }
    
    // For larger values, use bit manipulation + Newton
    if x == 0 {
        return 0;
    }
    
    // Initial guess based on highest bit
    let mut guess = 1u64 << ((64 - x.leading_zeros()) / 2);
    
    // Two Newton iterations (sufficient for 0.1% accuracy)
    guess = (guess + x / guess) / 2;
    guess = (guess + x / guess) / 2;
    
    guess
}

/// Fast normal CDF approximation using lookup table
/// Target: <500 CU
#[inline(always)]
fn fast_normal_cdf(x: i32) -> u32 {
    // x is in fixed point with 3 decimal places
    // Table covers [-4.0, 4.0] with 0.01 precision
    let table_x = x + 4000; // Shift to positive range
    
    if table_x < 0 {
        return 0; // CDF(-4) ≈ 0
    }
    if table_x >= NORMAL_CDF_LOOKUP.len() as i32 {
        return 10000; // CDF(4) ≈ 1
    }
    
    NORMAL_CDF_LOOKUP[table_x as usize].to_num() as u32
}

/// Optimized L2 norm calculation for price vector
/// Target: <5k CU
pub fn calculate_l2_norm_optimized(prices: &[u32]) -> Result<u64, ProgramError> {
    // Sum of squares using u64 to prevent overflow
    let mut sum_squares = 0u64;
    
    // Unroll loop for common case (4 outcomes)
    if prices.len() == 4 {
        sum_squares = (prices[0] as u64).pow(2) +
                     (prices[1] as u64).pow(2) +
                     (prices[2] as u64).pow(2) +
                     (prices[3] as u64).pow(2);
    } else {
        for &price in prices {
            let square = (price as u64).saturating_pow(2);
            sum_squares = sum_squares.saturating_add(square);
        }
    }
    
    // Fast square root
    Ok(fast_sqrt(sum_squares))
}

/// Optimized price normalization
/// Target: <3k CU
pub fn normalize_prices_optimized(prices: &mut [u32]) -> Result<(), ProgramError> {
    let norm = calculate_l2_norm_optimized(prices)?;
    
    if norm == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Use reciprocal multiplication instead of division
    let norm_recip = (1u64 << 32) / norm;
    
    for price in prices.iter_mut() {
        *price = ((*price as u64 * norm_recip * 10000) >> 32) as u32;
    }
    
    Ok(())
}

/// Optimized L2 AMM price update
/// Target: <20k CU (down from 70k)
pub fn update_prices_optimized(
    market: &mut L2AMMMarket,
    outcome: u8,
    trade_amount: i64,
) -> Result<(u64, u64), ProgramError> {
    if outcome as usize >= market.positions.len() {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Store initial position for slippage calculation
    let initial_position = market.positions[outcome as usize];
    
    // Apply trade impact using approximation
    // Δp_i ≈ k * trade_amount / liquidity
    let k = market.k_parameter;
    let liquidity = market.total_liquidity;
    
    if liquidity == 0 {
        return Err(BettingPlatformError::InsufficientLiquidity.into());
    }
    
    // Calculate price change (scaled by 1000 for precision)
    let impact = (trade_amount.abs() as u64 * k * 1000) / liquidity;
    
    // Update position with bounds checking
    let new_position = if trade_amount > 0 {
        // Buying increases position
        initial_position.saturating_add(impact / 100)
    } else {
        // Selling decreases position
        initial_position.saturating_sub(impact / 100)
    };
    
    // Update position
    market.positions[outcome as usize] = new_position;
    
    // Calculate price from position (simple conversion)
    let price = (new_position * 10000) / market.total_shares;
    
    // Calculate cost using trapezoidal approximation
    let initial_price = (initial_position * 10000) / market.total_shares;
    let avg_price = (initial_price + price) / 2;
    let cost = (trade_amount.abs() as u64 * avg_price) / 10000;
    
    Ok((cost, price.min(10000)))
}

/// Optimized continuous distribution fitting
/// Target: <25k CU
pub fn fit_distribution_optimized(
    distribution: &mut L2Distribution,
    observations: &[(u32, u32)], // (value, weight) pairs
) -> Result<(), ProgramError> {
    if observations.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate weighted mean using integer arithmetic
    let mut sum_weighted = 0u64;
    let mut sum_weights = 0u64;
    
    for &(value, weight) in observations {
        sum_weighted = sum_weighted.saturating_add(value as u64 * weight as u64);
        sum_weights = sum_weights.saturating_add(weight as u64);
    }
    
    if sum_weights == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    let mean = sum_weighted / sum_weights;
    
    // Calculate variance using single pass
    let mut variance_sum = 0u64;
    
    for &(value, weight) in observations {
        let diff = (value as i64 - mean as i64).abs() as u64;
        variance_sum = variance_sum.saturating_add(diff.pow(2) * weight as u64);
    }
    
    let variance = variance_sum / sum_weights;
    let std_dev = fast_sqrt(variance);
    
    // Update distribution parameters
    distribution.mean = mean as u32;
    distribution.std_dev = std_dev as u32;
    
    // Update prices based on normal distribution
    update_prices_from_distribution_optimized(distribution)?;
    
    Ok(())
}

/// Update prices based on distribution parameters
/// Target: <15k CU
fn update_prices_from_distribution_optimized(
    distribution: &mut L2Distribution,
) -> Result<(), ProgramError> {
    let num_buckets = distribution.prices.len();
    let mean = distribution.mean;
    let std_dev = distribution.std_dev.max(100); // Minimum std dev to avoid division issues
    
    // For each price bucket, calculate probability mass
    for (i, price) in distribution.prices.iter_mut().enumerate() {
        // Map bucket index to value range
        let bucket_center = (i as u32 * 10000) / (num_buckets as u32 - 1);
        
        // Calculate z-score (scaled by 1000)
        let z = ((bucket_center as i32 - mean as i32) * 1000) / std_dev as i32;
        
        // Use fast normal CDF lookup
        let cdf_upper = fast_normal_cdf(z + 500); // Upper bound of bucket
        let cdf_lower = fast_normal_cdf(z - 500); // Lower bound of bucket
        
        // Probability mass in this bucket
        *price = cdf_upper.saturating_sub(cdf_lower);
    }
    
    // Normalize to ensure sum = 10000
    let sum: u32 = distribution.prices.iter().sum();
    if sum > 0 {
        for price in distribution.prices.iter_mut() {
            *price = (*price as u64 * 10000 / sum as u64) as u32;
        }
    }
    
    Ok(())
}

/// Optimized multi-modal distribution support
/// Target: <30k CU
pub fn fit_multimodal_optimized(
    distribution: &mut L2Distribution,
    modes: &[(u32, u32, u32)], // (mean, std_dev, weight) for each mode
) -> Result<(), ProgramError> {
    if modes.is_empty() || modes.len() > 4 {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Clear prices
    for price in distribution.prices.iter_mut() {
        *price = 0;
    }
    
    let num_buckets = distribution.prices.len();
    let total_weight: u32 = modes.iter().map(|(_, _, w)| w).sum();
    
    if total_weight == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }
    
    // Accumulate probability from each mode
    for &(mean, std_dev, weight) in modes {
        let std_dev = std_dev.max(100);
        let weight_factor = (weight as u64 * 10000) / total_weight as u64;
        
        for (i, price) in distribution.prices.iter_mut().enumerate() {
            let bucket_center = (i as u32 * 10000) / (num_buckets as u32 - 1);
            let z = ((bucket_center as i32 - mean as i32) * 1000) / std_dev as i32;
            
            let cdf_upper = fast_normal_cdf(z + 500);
            let cdf_lower = fast_normal_cdf(z - 500);
            let mass = cdf_upper.saturating_sub(cdf_lower);
            
            *price = price.saturating_add(((mass as u64 * weight_factor) / 10000) as u32);
        }
    }
    
    // Final normalization
    normalize_prices_optimized(&mut distribution.prices)?;
    
    Ok(())
}

/// Calculate expected value from distribution
/// Target: <5k CU
pub fn calculate_expected_value_optimized(
    distribution: &L2Distribution,
) -> Result<u32, ProgramError> {
    let num_buckets = distribution.prices.len();
    let mut expected_value = 0u64;
    
    for (i, &price) in distribution.prices.iter().enumerate() {
        let bucket_value = (i as u64 * 10000) / (num_buckets as u64 - 1);
        expected_value = expected_value.saturating_add(bucket_value * price as u64);
    }
    
    // Normalize by total probability (should be 10000)
    Ok((expected_value / 10000) as u32)
}

/// Calculate percentile from distribution
/// Target: <3k CU
pub fn calculate_percentile_optimized(
    distribution: &L2Distribution,
    percentile: u8, // 0-100
) -> Result<u32, ProgramError> {
    if percentile > 100 {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    let target_cumulative = (percentile as u32 * 100); // Scale to basis points
    let mut cumulative = 0u32;
    
    for (i, &price) in distribution.prices.iter().enumerate() {
        cumulative = cumulative.saturating_add(price);
        
        if cumulative >= target_cumulative {
            // Found the bucket containing the percentile
            let bucket_value = (i as u32 * 10000) / (distribution.prices.len() as u32 - 1);
            return Ok(bucket_value);
        }
    }
    
    // Return max value if we didn't find it (shouldn't happen with normalized distribution)
    Ok(10000)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fast_sqrt() {
        assert_eq!(fast_sqrt(0), 0);
        assert_eq!(fast_sqrt(1), 1);
        assert_eq!(fast_sqrt(4), 2);
        assert_eq!(fast_sqrt(100), 10);
        
        // Test larger values
        let x = 123456;
        let sqrt_x = fast_sqrt(x);
        assert!(((sqrt_x * sqrt_x) as i64 - x as i64) < 1000); // Less than 1% error
    }
    
    #[test]
    fn test_l2_norm_optimized() {
        let prices = vec![3000, 4000, 5000];
        let norm = calculate_l2_norm_optimized(&prices).unwrap();
        
        // sqrt(9 + 16 + 25) * 1000 = sqrt(50) * 1000 ≈ 7071
        assert!((norm as i64 - 7071).abs() < 100);
    }
    
    #[test]
    fn test_distribution_fitting() {
        let mut distribution = L2Distribution {
            discriminator: [0; 8],
            distribution_type: 0,
            mean: 5000,
            std_dev: 1000,
            skew: 0,
            kurtosis: 0,
            prices: vec![0; 10],
            liquidity: 1_000_000,
            k_constant: 100,
            last_update_slot: 0,
        };
        
        let observations = vec![
            (4500, 10),
            (5000, 20),
            (5500, 10),
        ];
        
        fit_distribution_optimized(&mut distribution, &observations).unwrap();
        
        // Check that prices sum to approximately 10000
        let sum: u32 = distribution.prices.iter().sum();
        assert!((sum as i32 - 10000).abs() < 100);
    }
}