use solana_program::program_error::ProgramError;
use crate::error::CorrelationError;
use crate::math::fixed_point::U64F64;

/// Calculate Pearson correlation coefficient between two price series
/// Returns a value in the range [0, 2*ONE] where:
/// - 0 represents -1 correlation
/// - ONE represents 0 correlation  
/// - 2*ONE represents +1 correlation
pub fn calculate_pearson_correlation(
    prices_1: &[u64],
    prices_2: &[u64],
) -> Result<u64, ProgramError> {
    if prices_1.len() != prices_2.len() {
        return Err(CorrelationError::MismatchedDataLength.into());
    }
    
    if prices_1.len() < 2 {
        return Err(CorrelationError::InsufficientData.into());
    }
    
    let n = prices_1.len() as u64;
    
    // Calculate means
    let sum_x: u64 = prices_1.iter().sum();
    let sum_y: u64 = prices_2.iter().sum();
    
    // Simple division for mean (no fixed-point scaling needed here)
    let mean_x = sum_x / n;
    let mean_y = sum_y / n;
    
    // Calculate covariance and variances
    let mut cov_sum = 0i128; // Use signed for covariance
    let mut var_x_sum = 0u128;
    let mut var_y_sum = 0u128;
    
    for i in 0..prices_1.len() {
        let x = prices_1[i];
        let y = prices_2[i];
        
        // Calculate differences from mean (can be negative)
        let diff_x = x as i128 - mean_x as i128;
        let diff_y = y as i128 - mean_y as i128;
        
        // Covariance term (signed) - don't divide by ONE yet
        cov_sum += diff_x * diff_y;
        
        // Variance terms (always positive) - don't divide by ONE yet
        var_x_sum += (diff_x * diff_x) as u128;
        var_y_sum += (diff_y * diff_y) as u128;
    }
    
    // Calculate variances (divide by n and scale)
    let var_x = (var_x_sum / n as u128 / U64F64::ONE as u128) as u64;
    let var_y = (var_y_sum / n as u128 / U64F64::ONE as u128) as u64;
    
    // Check for zero variance
    // Variance is in fixed point (scale = 1e6). A threshold of 10 means ~1e-5 variance
    // (std dev ~0.0032), which avoids divide-by-zero while still working for typical
    // prediction-market price ranges.
    const VARIANCE_THRESHOLD: u64 = 10;
    if var_x < VARIANCE_THRESHOLD || var_y < VARIANCE_THRESHOLD {
        // No variance means undefined correlation, return 0 correlation (midpoint)
        return Ok(U64F64::ONE);
    }
    
    let std_x = U64F64::sqrt(var_x)?;
    let std_y = U64F64::sqrt(var_y)?;
    
    // Calculate correlation coefficient
    let denominator = U64F64::checked_mul(std_x, std_y)?;
    
    // Calculate covariance (keep sign for proper mapping)
    let covariance_scaled = cov_sum / n as i128 / U64F64::ONE as i128;
    let covariance_abs = covariance_scaled.abs() as u64;
    let correlation = U64F64::checked_div(covariance_abs, denominator)?;
    
    // Map correlation to [0, 2*ONE] range based on sign of covariance
    // If covariance was negative, map to [0, ONE]
    // If covariance was positive, map to [ONE, 2*ONE]
    let result = if cov_sum < 0 {
        // Negative correlation: map [-1, 0] to [0, ONE]
        U64F64::ONE.saturating_sub(correlation.min(U64F64::ONE))
    } else {
        // Positive correlation: map [0, 1] to [ONE, 2*ONE]
        U64F64::ONE + correlation.min(U64F64::ONE)
    };
    
    
    Ok(result)
}

/// Calculate correlation factor for a verse (average of pairwise correlations)
/// For tail loss calculation, we need the absolute correlation value between 0 and 1
pub fn calculate_correlation_factor(
    correlation_matrix: &[(usize, usize, u64)],
    num_markets: usize,
) -> Result<u64, ProgramError> {
    if correlation_matrix.is_empty() || num_markets < 2 {
        return Ok(U64F64::ZERO);
    }
    
    let mut sum = 0u128;
    let mut count = 0u64;
    
    for (_, _, correlation) in correlation_matrix {
        // Convert from [-1, 1] representation (0 to 2*ONE) to absolute value (0 to 1*ONE)
        // In our representation: -1 = 0, 0 = ONE, +1 = 2*ONE
        let abs_corr = if *correlation > U64F64::ONE {
            // Positive correlation: map from [ONE, 2*ONE] to [0, ONE]
            correlation - U64F64::ONE
        } else {
            // Negative correlation: map from [0, ONE] to [ONE, 0] then to [0, ONE]
            U64F64::ONE - correlation
        };
        
        sum += abs_corr as u128;
        count += 1;
    }
    
    if count == 0 {
        return Ok(U64F64::ZERO);
    }
    
    Ok((sum / count as u128) as u64)
}

/// Convert correlation from [-1,1] representation to [0,1] for tail loss
pub fn correlation_to_tail_loss_factor(correlation: u64) -> u64 {
    // In our representation: -1 = 0, 0 = ONE, +1 = 2*ONE
    // For tail loss, we want absolute correlation [0, 1]
    if correlation > U64F64::ONE {
        // Positive correlation
        correlation - U64F64::ONE
    } else {
        // Negative correlation
        U64F64::ONE - correlation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_perfect_positive_correlation() {
        let prices_1 = vec![
            100_000_000, // 100
            110_000_000, // 110
            120_000_000, // 120
            130_000_000, // 130
        ];
        let prices_2 = prices_1.clone();
        
        let correlation = calculate_pearson_correlation(&prices_1, &prices_2).unwrap();
        
        // Perfect positive correlation should be close to 2 * ONE (representing +1)
        // In our representation: -1 = 0, 0 = 1_000_000, +1 = 2_000_000
        assert!(correlation >= 2 * U64F64::ONE - 100_000); // Should be ~2_000_000
    }
    
    #[test]
    fn test_perfect_negative_correlation() {
        let prices_1 = vec![
            1_000_000,  // 1.0
            2_000_000,  // 2.0
            3_000_000,  // 3.0
            4_000_000,  // 4.0
        ];
        let prices_2 = vec![
            4_000_000,  // 4.0
            3_000_000,  // 3.0
            2_000_000,  // 2.0
            1_000_000,  // 1.0
        ];
        
        let correlation = calculate_pearson_correlation(&prices_1, &prices_2).unwrap();
        
        // Perfect negative correlation should be close to 0 (representing -1)
        assert!(correlation < 100_000); // Should be ~0
    }
    
    #[test]
    fn test_no_correlation() {
        let prices_1 = vec![
            100_000_000, // 100 with 6 decimals
            110_000_000, // 110
            100_000_000, // 100
            110_000_000, // 110
        ];
        let prices_2 = vec![
            100_000_000, // 100
            100_000_000, // 100
            100_000_000, // 100
            100_000_000, // 100
        ];
        
        let correlation = calculate_pearson_correlation(&prices_1, &prices_2).unwrap();
        
        // No variance in prices_2 should give 0 correlation (represented as ONE in our mapping)
        assert_eq!(correlation, U64F64::ONE);
    }
    
    #[test]
    fn test_correlation_to_tail_loss_factor() {
        // Test positive correlation (+0.8 -> 0.8)
        let pos_corr = U64F64::ONE + 800_000; // 1.8 represents +0.8
        let tail_loss_factor = correlation_to_tail_loss_factor(pos_corr);
        assert_eq!(tail_loss_factor, 800_000); // Should be 0.8
        
        // Test negative correlation (-0.8 -> 0.8)
        let neg_corr = U64F64::ONE - 800_000; // 0.2 represents -0.8
        let tail_loss_factor = correlation_to_tail_loss_factor(neg_corr);
        assert_eq!(tail_loss_factor, 800_000); // Should be 0.8
        
        // Test zero correlation (0 -> 0)
        let zero_corr = U64F64::ONE; // 1.0 represents 0
        let tail_loss_factor = correlation_to_tail_loss_factor(zero_corr);
        assert_eq!(tail_loss_factor, 0); // Should be 0
    }
}
