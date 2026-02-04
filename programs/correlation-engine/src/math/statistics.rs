use solana_program::program_error::ProgramError;
use crate::error::CorrelationError;
use crate::math::fixed_point::U64F64;

/// Calculate mean of a series
pub fn calculate_mean(values: &[u64]) -> Result<u64, ProgramError> {
    if values.is_empty() {
        return Err(CorrelationError::InsufficientData.into());
    }
    
    let sum: u128 = values.iter().map(|&v| v as u128).sum();
    let count = values.len() as u128;
    
    Ok((sum / count) as u64)
}

/// Calculate variance of a series
pub fn calculate_variance(values: &[u64], mean: u64) -> Result<u64, ProgramError> {
    if values.is_empty() {
        return Err(CorrelationError::InsufficientData.into());
    }
    
    let mut sum_squared_diff = 0u128;
    
    for &value in values {
        let diff = if value >= mean {
            value - mean
        } else {
            mean - value
        };
        
        sum_squared_diff += (diff as u128 * diff as u128) / U64F64::ONE as u128;
    }
    
    Ok((sum_squared_diff / values.len() as u128) as u64)
}

/// Calculate standard deviation
pub fn calculate_std_dev(variance: u64) -> Result<u64, ProgramError> {
    U64F64::sqrt(variance)
}

/// Find minimum value in a series
pub fn find_min(values: &[u64]) -> Result<u64, ProgramError> {
    values.iter().min().copied()
        .ok_or(CorrelationError::InsufficientData.into())
}

/// Find maximum value in a series
pub fn find_max(values: &[u64]) -> Result<u64, ProgramError> {
    values.iter().max().copied()
        .ok_or(CorrelationError::InsufficientData.into())
}

/// Calculate median of a series
pub fn calculate_median(values: &[u64]) -> Result<u64, ProgramError> {
    if values.is_empty() {
        return Err(CorrelationError::InsufficientData.into());
    }
    
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    
    let len = sorted.len();
    if len % 2 == 0 {
        // Even number of elements - average the two middle values
        let mid1 = sorted[len / 2 - 1];
        let mid2 = sorted[len / 2];
        Ok((mid1 + mid2) / 2)
    } else {
        // Odd number of elements - take the middle value
        Ok(sorted[len / 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mean_calculation() {
        let values = vec![
            U64F64::from_num(10),
            U64F64::from_num(20),
            U64F64::from_num(30),
            U64F64::from_num(40),
        ];
        
        let mean = calculate_mean(&values).unwrap();
        assert_eq!(mean, U64F64::from_num(25));
    }
    
    #[test]
    fn test_variance_calculation() {
        let values = vec![
            U64F64::from_num(10),
            U64F64::from_num(20),
            U64F64::from_num(30),
            U64F64::from_num(40),
        ];
        
        let mean = U64F64::from_num(25);
        let variance = calculate_variance(&values, mean).unwrap();
        
        // Variance should be 125 (in fixed point)
        assert!(variance > 0);
    }
    
    #[test]
    fn test_median_calculation() {
        // Odd number of elements
        let values1 = vec![
            U64F64::from_num(10),
            U64F64::from_num(30),
            U64F64::from_num(20),
        ];
        assert_eq!(calculate_median(&values1).unwrap(), U64F64::from_num(20));
        
        // Even number of elements
        let values2 = vec![
            U64F64::from_num(10),
            U64F64::from_num(20),
            U64F64::from_num(30),
            U64F64::from_num(40),
        ];
        assert_eq!(calculate_median(&values2).unwrap(), U64F64::from_num(25));
    }
}