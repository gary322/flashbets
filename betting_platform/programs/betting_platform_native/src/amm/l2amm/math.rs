//! L2-AMM mathematical functions
//!
//! Implements L2 norm calculations for continuous distributions

use solana_program::program_error::ProgramError;

use crate::{
    error::BettingPlatformError,
    math::{U64F64, U128F128},
    state::amm_accounts::{L2AMMPool, DistributionBin},
};

/// Calculate L2 norm of the distribution
/// ||x||_2 = sqrt(Σ x_i^2)
pub fn calculate_l2_norm(distribution: &[DistributionBin]) -> Result<U64F64, ProgramError> {
    let mut sum_squares = U128F128::from_num(0u128);

    for bin in distribution {
        let weight = U128F128::from_num(bin.weight as u128);
        let squared = weight.checked_mul(weight)
            .ok_or(BettingPlatformError::MathOverflow)?;
        sum_squares = sum_squares.checked_add(squared)
            .ok_or(BettingPlatformError::MathOverflow)?;
    }

    // Calculate square root
    let norm = sum_squares.sqrt()?;
    Ok(U64F64::from_num(norm.to_num() as u64))
}

/// Calculate normalized distribution (sum to 1)
pub fn normalize_distribution(
    distribution: &[DistributionBin],
) -> Result<Vec<DistributionBin>, ProgramError> {
    let norm = calculate_l2_norm(distribution)?;
    
    if norm.is_zero() {
        return Err(BettingPlatformError::DivisionByZero.into());
    }

    let mut normalized = Vec::with_capacity(distribution.len());
    
    for bin in distribution {
        let normalized_weight = U64F64::from_num(bin.weight)
            .checked_div(norm)?
            .to_num();
        
        normalized.push(DistributionBin {
            lower_bound: bin.lower_bound,
            upper_bound: bin.upper_bound,
            weight: normalized_weight,
        });
    }

    Ok(normalized)
}

/// Calculate cost of buying shares in a range
pub fn calculate_range_buy_cost(
    pool: &L2AMMPool,
    lower_bound: u64,
    upper_bound: u64,
    shares: u64,
) -> Result<u64, ProgramError> {
    // Find bins that overlap with the range
    let affected_bins = find_overlapping_bins(&pool.distribution, lower_bound, upper_bound)?;
    
    if affected_bins.is_empty() {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    // Calculate current L2 norm
    let current_norm = calculate_l2_norm(&pool.distribution)?;

    // Create updated distribution
    let mut new_distribution = pool.distribution.clone();
    
    for &idx in &affected_bins {
        let overlap_fraction = calculate_overlap_fraction(
            &pool.distribution[idx],
            lower_bound,
            upper_bound,
        )?;
        
        let shares_to_add = U64F64::from_num(shares)
            .checked_mul(overlap_fraction)?
            .to_num();
        
        new_distribution[idx].weight = new_distribution[idx].weight
            .saturating_add(shares_to_add);
    }

    // Calculate new L2 norm
    let new_norm = calculate_l2_norm(&new_distribution)?;

    // Cost is the difference in norms times liquidity parameter
    let norm_diff = new_norm.checked_sub(current_norm)?;
    let cost = norm_diff
        .checked_mul(U64F64::from_num(pool.liquidity_parameter))?
        .to_num();

    Ok(cost)
}

/// Calculate payout from selling shares in a range
pub fn calculate_range_sell_payout(
    pool: &L2AMMPool,
    lower_bound: u64,
    upper_bound: u64,
    shares: u64,
) -> Result<u64, ProgramError> {
    // Find bins that overlap with the range
    let affected_bins = find_overlapping_bins(&pool.distribution, lower_bound, upper_bound)?;
    
    if affected_bins.is_empty() {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    // Verify sufficient shares exist
    let mut total_available = 0u64;
    for &idx in &affected_bins {
        let overlap_fraction = calculate_overlap_fraction(
            &pool.distribution[idx],
            lower_bound,
            upper_bound,
        )?;
        
        let available_in_bin = U64F64::from_num(pool.distribution[idx].weight)
            .checked_mul(overlap_fraction)?
            .to_num();
        
        total_available = total_available.saturating_add(available_in_bin);
    }

    if shares > total_available {
        return Err(BettingPlatformError::InsufficientShares.into());
    }

    // Calculate current L2 norm
    let current_norm = calculate_l2_norm(&pool.distribution)?;

    // Create updated distribution
    let mut new_distribution = pool.distribution.clone();
    let mut remaining_shares = shares;
    
    for &idx in &affected_bins {
        if remaining_shares == 0 {
            break;
        }
        
        let overlap_fraction = calculate_overlap_fraction(
            &pool.distribution[idx],
            lower_bound,
            upper_bound,
        )?;
        
        let available_in_bin = U64F64::from_num(pool.distribution[idx].weight)
            .checked_mul(overlap_fraction)?
            .to_num();
        
        let shares_to_remove = remaining_shares.min(available_in_bin);
        
        new_distribution[idx].weight = new_distribution[idx].weight
            .saturating_sub(shares_to_remove);
        
        remaining_shares = remaining_shares.saturating_sub(shares_to_remove);
    }

    // Calculate new L2 norm
    let new_norm = calculate_l2_norm(&new_distribution)?;

    // Payout is the difference in norms times liquidity parameter
    let norm_diff = current_norm.checked_sub(new_norm)?;
    let payout = norm_diff
        .checked_mul(U64F64::from_num(pool.liquidity_parameter))?
        .to_num();

    Ok(payout)
}

/// Find bins that overlap with a given range
pub fn find_overlapping_bins(
    distribution: &[DistributionBin],
    lower_bound: u64,
    upper_bound: u64,
) -> Result<Vec<usize>, ProgramError> {
    if lower_bound >= upper_bound {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    let mut overlapping = Vec::new();
    
    for (idx, bin) in distribution.iter().enumerate() {
        if bin.upper_bound > lower_bound && bin.lower_bound < upper_bound {
            overlapping.push(idx);
        }
    }

    Ok(overlapping)
}

/// Calculate fraction of bin that overlaps with range
pub fn calculate_overlap_fraction(
    bin: &DistributionBin,
    lower_bound: u64,
    upper_bound: u64,
) -> Result<U64F64, ProgramError> {
    let bin_size = bin.upper_bound.saturating_sub(bin.lower_bound);
    if bin_size == 0 {
        return Ok(U64F64::from_num(0));
    }

    let overlap_start = bin.lower_bound.max(lower_bound);
    let overlap_end = bin.upper_bound.min(upper_bound);
    
    if overlap_start >= overlap_end {
        return Ok(U64F64::from_num(0));
    }

    let overlap_size = overlap_end.saturating_sub(overlap_start);
    
    Ok(U64F64::from_num(overlap_size)
        .checked_div(U64F64::from_num(bin_size))?)
}

/// Calculate implied probability density at a point
pub fn calculate_probability_density(
    pool: &L2AMMPool,
    value: u64,
) -> Result<U64F64, ProgramError> {
    // Find the bin containing this value
    for bin in &pool.distribution {
        if value >= bin.lower_bound && value < bin.upper_bound {
            let bin_width = bin.upper_bound.saturating_sub(bin.lower_bound);
            if bin_width == 0 {
                return Err(BettingPlatformError::DivisionByZero.into());
            }
            
            // Normalize by total weight to get probability
            let total_weight: u64 = pool.distribution.iter()
                .map(|b| b.weight)
                .sum();
            
            if total_weight == 0 {
                return Err(BettingPlatformError::DivisionByZero.into());
            }
            
            // Density = weight / (bin_width * total_weight)
            let density = U64F64::from_num(bin.weight)
                .checked_div(U64F64::from_num(bin_width))?
                .checked_div(U64F64::from_num(total_weight))?;
            
            return Ok(density);
        }
    }

    Ok(U64F64::from_num(0))
}

/// Calculate expected value of the distribution
pub fn calculate_expected_value(pool: &L2AMMPool) -> Result<u64, ProgramError> {
    let total_weight: u64 = pool.distribution.iter()
        .map(|b| b.weight)
        .sum();
    
    if total_weight == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }

    let mut weighted_sum = U128F128::from_num(0u128);
    
    for bin in &pool.distribution {
        let bin_midpoint = (bin.lower_bound + bin.upper_bound) / 2;
        let contribution = U128F128::from_num(bin_midpoint as u128)
            .checked_mul(U128F128::from_num(bin.weight as u128))
            .ok_or(BettingPlatformError::MathOverflow)?;
        weighted_sum = weighted_sum.checked_add(contribution)
            .ok_or(BettingPlatformError::MathOverflow)?;
    }

    let expected_value = weighted_sum
        .checked_div(U128F128::from_num(total_weight as u128))
        .ok_or(BettingPlatformError::DivisionByZero)?;
    
    Ok(expected_value.to_num() as u64)
}

/// Calculate variance of the distribution
pub fn calculate_variance(pool: &L2AMMPool) -> Result<u64, ProgramError> {
    let expected_value = calculate_expected_value(pool)?;
    let total_weight: u64 = pool.distribution.iter()
        .map(|b| b.weight)
        .sum();
    
    if total_weight == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }

    let mut variance_sum = U128F128::from_num(0u128);
    
    for bin in &pool.distribution {
        let bin_midpoint = (bin.lower_bound + bin.upper_bound) / 2;
        let deviation = if bin_midpoint >= expected_value {
            bin_midpoint - expected_value
        } else {
            expected_value - bin_midpoint
        };
        
        let squared_deviation = U128F128::from_num(deviation as u128)
            .checked_mul(U128F128::from_num(deviation as u128))
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        let weighted_deviation = squared_deviation
            .checked_mul(U128F128::from_num(bin.weight as u128))
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        variance_sum = variance_sum.checked_add(weighted_deviation)
            .ok_or(BettingPlatformError::MathOverflow)?;
    }

    let variance = variance_sum
        .checked_div(U128F128::from_num(total_weight as u128))
        .ok_or(BettingPlatformError::DivisionByZero)?;
    
    Ok(variance.to_num() as u64)
}

/// Calculate 95% confidence interval
pub fn calculate_confidence_interval(
    pool: &L2AMMPool,
) -> Result<(u64, u64), ProgramError> {
    let mean = calculate_expected_value(pool)?;
    let variance = calculate_variance(pool)?;
    
    // Standard deviation = sqrt(variance)
    let std_dev = U64F64::from_num(variance).sqrt()?.to_num();
    
    // 95% CI ≈ mean ± 1.96 * std_dev
    let margin = std_dev.saturating_mul(196) / 100;
    
    let lower = mean.saturating_sub(margin);
    let upper = mean.saturating_add(margin);
    
    Ok((lower, upper))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_distribution() -> Vec<DistributionBin> {
        vec![
            DistributionBin {
                lower_bound: 0,
                upper_bound: 100,
                weight: 100,
            },
            DistributionBin {
                lower_bound: 100,
                upper_bound: 200,
                weight: 200,
            },
            DistributionBin {
                lower_bound: 200,
                upper_bound: 300,
                weight: 100,
            },
        ]
    }

    #[test]
    fn test_l2_norm() {
        let distribution = create_test_distribution();
        let norm = calculate_l2_norm(&distribution).unwrap();
        
        // sqrt(100^2 + 200^2 + 100^2) = sqrt(60000) ≈ 244.95
        assert!((norm.to_num() as i64 - 245).abs() < 2);
    }

    #[test]
    fn test_overlap_fraction() {
        let bin = DistributionBin {
            lower_bound: 100,
            upper_bound: 200,
            weight: 100,
        };
        
        // Full overlap
        let fraction = calculate_overlap_fraction(&bin, 100, 200).unwrap();
        assert_eq!(fraction.to_num(), 1);
        
        // Half overlap
        let fraction = calculate_overlap_fraction(&bin, 150, 250).unwrap();
        assert_eq!(fraction.to_num() * 2, 1);
        
        // No overlap
        let fraction = calculate_overlap_fraction(&bin, 0, 50).unwrap();
        assert_eq!(fraction.to_num(), 0);
    }
}