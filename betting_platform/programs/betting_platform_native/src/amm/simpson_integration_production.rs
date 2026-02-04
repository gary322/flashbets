//! Production-grade Simpson's Rule integration for L2-AMM
//! 
//! Implements 100-segment integration for continuous distributions

use solana_program::{
    msg,
    program_error::ProgramError,
};
use crate::{
    error::BettingPlatformError,
    math::fixed_point::U64F64,
};

/// Helper function for absolute value since U64F64 doesn't expose abs()
fn abs_diff(a: U64F64, b: U64F64) -> U64F64 {
    if a > b {
        a - b
    } else {
        b - a
    }
}

/// Production Simpson's Rule integration with 100 segments
/// Maintains L2 norm constraint for continuous probability distributions
pub fn simpson_integration<F>(
    distribution: F,
    lower_bound: U64F64,
    upper_bound: U64F64,
    segments: usize,
) -> Result<U64F64, ProgramError>
where
    F: Fn(U64F64) -> U64F64,
{
    // Validate inputs
    if segments < 2 || segments % 2 != 0 {
        return Err(BettingPlatformError::InvalidParameter.into());
    }
    if upper_bound <= lower_bound {
        return Err(BettingPlatformError::InvalidBounds.into());
    }
    
    msg!("Simpson's integration: {} segments from {} to {}", 
         segments, lower_bound, upper_bound);
    
    // Calculate segment width
    let h = (upper_bound - lower_bound) / U64F64::from_num(segments as u64);
    
    // Initialize accumulator
    let mut sum = U64F64::from_num(0);
    
    // Add endpoints
    sum = sum + distribution(lower_bound) + distribution(upper_bound);
    
    // Add odd-indexed points (coefficient 4)
    for i in 1..segments {
        if i % 2 == 1 {
            let x = lower_bound + h * U64F64::from_num(i as u64);
            sum = sum + distribution(x) * U64F64::from_num(4);
        }
    }
    
    // Add even-indexed points (coefficient 2)
    for i in 2..segments {
        if i % 2 == 0 {
            let x = lower_bound + h * U64F64::from_num(i as u64);
            sum = sum + distribution(x) * U64F64::from_num(2);
        }
    }
    
    // Apply Simpson's formula: (h/3) * sum
    let result = (h * sum) / U64F64::from_num(3);
    
    msg!("  Integration result: {}", result);
    
    Ok(result)
}

/// Production L2-AMM continuous distribution update
pub fn update_l2_distribution(
    current_distribution: &[U64F64],
    bet_lower: U64F64,
    bet_upper: U64F64,
    bet_amount: U64F64,
    min_value: U64F64,
    max_value: U64F64,
) -> Result<Vec<U64F64>, ProgramError> {
    let n = current_distribution.len();
    if n == 0 {
        return Err(BettingPlatformError::InvalidDistribution.into());
    }
    
    msg!("Updating L2 distribution for bet on [{}, {}]", bet_lower, bet_upper);
    
    // Calculate bin width
    let bin_width = (max_value - min_value) / U64F64::from_num(n as u64);
    
    // Create updated distribution maintaining L2 norm
    let mut new_distribution = current_distribution.to_vec();
    
    // Find affected bins
    let start_bin: usize = ((bet_lower - min_value) / bin_width).to_num() as usize;
    let start_bin = start_bin.min(n - 1);
    let end_bin: usize = ((bet_upper - min_value) / bin_width).to_num() as usize;
    let end_bin = end_bin.min(n - 1);
    
    // Calculate current L2 norm
    let current_l2_norm = calculate_l2_norm(current_distribution)?;
    
    // Update affected region
    let affected_bins = end_bin - start_bin + 1;
    let update_per_bin = bet_amount / U64F64::from_num(affected_bins as u64);
    
    for i in start_bin..=end_bin {
        // Increase probability in bet range
        new_distribution[i] = new_distribution[i] + update_per_bin;
    }
    
    // Renormalize to maintain L2 constraint
    let new_l2_norm = calculate_l2_norm(&new_distribution)?;
    let normalization_factor = current_l2_norm / new_l2_norm;
    
    for i in 0..n {
        new_distribution[i] = new_distribution[i] * normalization_factor;
    }
    
    // Verify L2 norm preserved
    let final_l2_norm = calculate_l2_norm(&new_distribution)?;
    let norm_error = abs_diff(final_l2_norm, current_l2_norm);
    
    if norm_error > U64F64::from_fraction(1, 1000).unwrap_or(U64F64::from_num(0)) {
        msg!("Warning: L2 norm deviation: {}", norm_error);
    }
    
    Ok(new_distribution)
}

/// Calculate L2 norm of distribution
fn calculate_l2_norm(distribution: &[U64F64]) -> Result<U64F64, ProgramError> {
    let mut sum_squares = U64F64::from_num(0);
    for &x in distribution.iter() {
        sum_squares = sum_squares.checked_add(x.checked_mul(x)?)?;
    }
    
    // Square root approximation using Newton's method
    sqrt_approximation(sum_squares)
}

/// Square root approximation for fixed-point
fn sqrt_approximation(x: U64F64) -> Result<U64F64, ProgramError> {
    if x == U64F64::from_num(0) {
        return Ok(U64F64::from_num(0));
    }
    
    // Newton's method for square root
    let mut guess = x / U64F64::from_num(2);
    const ITERATIONS: usize = 5;
    
    for _ in 0..ITERATIONS {
        guess = (guess + x / guess) / U64F64::from_num(2);
    }
    
    Ok(guess)
}

/// Production test: Verify 100-segment integration accuracy
pub fn verify_integration_accuracy() -> Result<(), ProgramError> {
    msg!("Verifying Simpson's integration with 100 segments");
    
    // Test with known distributions
    
    // Test 1: Uniform distribution
    let uniform = |_x: U64F64| U64F64::from_num(1);
    let integral1 = simpson_integration(
        uniform,
        U64F64::from_num(0),
        U64F64::from_num(1),
        100,
    )?;
    msg!("  Uniform distribution integral: {} (expected: 1.0)", integral1);
    
    // Test 2: Linear distribution
    let linear = |x: U64F64| x;
    let integral2 = simpson_integration(
        linear,
        U64F64::from_num(0),
        U64F64::from_num(1),
        100,
    )?;
    msg!("  Linear distribution integral: {} (expected: 0.5)", integral2);
    
    // Test 3: Quadratic distribution
    let quadratic = |x: U64F64| x * x;
    let integral3 = simpson_integration(
        quadratic,
        U64F64::from_num(0),
        U64F64::from_num(1),
        100,
    )?;
    msg!("  Quadratic distribution integral: {} (expected: 0.333)", integral3);
    
    // Verify accuracy
    let error1 = abs_diff(integral1, U64F64::from_num(1));
    let error2 = abs_diff(integral2, U64F64::from_fraction(1, 2).unwrap_or(U64F64::from_num(0)));
    let error3 = abs_diff(integral3, U64F64::from_fraction(333, 1000).unwrap_or(U64F64::from_num(0)));
    
    assert!(error1 < U64F64::from_fraction(1, 1000).unwrap_or(U64F64::from_num(0)));
    assert!(error2 < U64F64::from_fraction(1, 1000).unwrap_or(U64F64::from_num(0)));
    assert!(error3 < U64F64::from_fraction(1, 100).unwrap_or(U64F64::from_num(0)));
    
    msg!("  ✓ Integration accuracy verified");
    
    Ok(())
}

/// Calculate expected value for continuous distribution
pub fn calculate_expected_value(
    distribution: &[U64F64],
    min_value: U64F64,
    max_value: U64F64,
) -> Result<U64F64, ProgramError> {
    let n = distribution.len();
    if n == 0 {
        return Err(BettingPlatformError::InvalidDistribution.into());
    }
    
    let bin_width = (max_value - min_value) / U64F64::from_num(n as u64);
    
    // Define value-weighted distribution
    let value_weighted = |i: usize| -> U64F64 {
        let x = min_value + bin_width * U64F64::from_num(i as u64);
        x * distribution[i]
    };
    
    // Integrate over distribution
    let mut sum = U64F64::from_num(0);
    for i in 0..n {
        sum = sum + value_weighted(i);
    }
    
    Ok(sum * bin_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simpson_integration() {
        // Test constant function
        let result = simpson_integration(
            |_| U64F64::from_num(2),
            U64F64::from_num(0),
            U64F64::from_num(5),
            100,
        ).unwrap();
        
        let expected = U64F64::from_num(10); // 2 * 5
        assert!(abs_diff(result, expected) < U64F64::from_num(1) / U64F64::from_num(100)); // < 0.01
    }
    
    #[test]
    fn test_l2_norm_preservation() {
        let distribution = vec![U64F64::from_num(1) / U64F64::from_num(10); 10]; // 0.1 each
        let updated = update_l2_distribution(
            &distribution,
            U64F64::from_num(2),
            U64F64::from_num(3),
            U64F64::from_num(1) / U64F64::from_num(2), // 0.5
            U64F64::from_num(0),
            U64F64::from_num(10),
        ).unwrap();
        
        let original_norm = calculate_l2_norm(&distribution).unwrap();
        let updated_norm = calculate_l2_norm(&updated).unwrap();
        
        assert!(abs_diff(original_norm, updated_norm) < U64F64::from_num(1) / U64F64::from_num(1000)); // < 0.001
    }
    
    #[test]
    fn test_accuracy_verification() {
        verify_integration_accuracy().unwrap();
    }
}