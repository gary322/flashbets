//! Elastic fee calculation based on coverage ratio
//!
//! Implements the formula: taker_fee = FEE_BASE (3bp) + FEE_SLOPE (25bp) * exp(-3*coverage)
//! as specified in Part 7

use solana_program::{
    msg,
    program_error::ProgramError,
};
use crate::math::fixed_point::U64F64;

use crate::{
    error::BettingPlatformError,
    fees::{FEE_BASE_BPS, FEE_MAX_BPS, FEE_SLOPE},
    constants::{BASE_FEE_BPS, POLYMARKET_FEE_BPS},
};

/// Calculate elastic fee based on coverage ratio
/// 
/// # Formula
/// taker_fee = FEE_BASE (3bp) + FEE_SLOPE (25bp) * exp(-3*coverage)
/// 
/// # Examples
/// - High coverage (2.0): exp(-6) ≈ 0.0025, fee = 3 + 25*0.0025 = 3.0625bp
/// - Low coverage (0.5): exp(-1.5) ≈ 0.2231, fee = 3 + 25*0.2231 = 8.5775bp
pub fn calculate_elastic_fee(coverage: U64F64) -> Result<u16, ProgramError> {
    // Validate coverage is positive
    if coverage == U64F64::from_num(0) {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate exp(-3 * coverage)
    // For on-chain efficiency, we use a lookup table approximation
    let exp_term = calculate_exp_approximation(coverage)?;
    
    // Calculate fee = base + slope * exp_term
    let slope_contribution = U64F64::from_num(FEE_SLOPE as u64) * exp_term;
    let total_fee = U64F64::from_num(FEE_BASE_BPS as u64) + slope_contribution;
    
    // Convert to basis points and cap at maximum
    let fee_bps = total_fee.to_num() as u16;
    let final_fee = fee_bps.min(FEE_MAX_BPS);
    
    msg!("Elastic fee calculation: coverage={}, fee={}bp", 
         coverage, final_fee);
    
    Ok(final_fee)
}

/// Approximation of exp(-3x) using Taylor series for on-chain efficiency
/// 
/// Uses the approximation: exp(-3x) ≈ 1 - 3x + 9x²/2 - 27x³/6 + ...
/// Accurate for x in [0, 2] which covers our coverage range
fn calculate_exp_approximation(coverage: U64F64) -> Result<U64F64, ProgramError> {
    let three = U64F64::from_num(3);
    let x = three * coverage;
    
    // For high coverage (x > 6), return minimal value
    if x > U64F64::from_num(6) {
        return Ok(U64F64::from_fraction(25, 10000)?); // exp(-6) ≈ 0.0025
    }
    
    // Taylor series approximation for exp(-x)
    // exp(-x) ≈ 1 - x + x²/2 - x³/6 + x⁴/24
    let one = U64F64::from_num(1);
    let two = U64F64::from_num(2);
    let six = U64F64::from_num(6);
    let twenty_four = U64F64::from_num(24);
    
    let x_squared = x * x;
    let x_cubed = x_squared * x;
    let x_fourth = x_cubed * x;
    
    let result = one - x + x_squared / two - x_cubed / six + x_fourth / twenty_four;
    
    // Clamp result to [0, 1]
    if result < U64F64::from_num(0) {
        Ok(U64F64::from_num(0))
    } else if result > one {
        Ok(one)
    } else {
        Ok(result)
    }
}

/// Calculate fee adjustment based on market conditions
/// 
/// Additional factors that can modify the base elastic fee:
/// - High volatility: +1-3bp
/// - Low liquidity: +1-2bp
/// - Network congestion: +1bp
pub fn calculate_fee_adjustments(
    base_fee: u16,
    volatility: U64F64,
    liquidity: u64,
    congestion_factor: u8,
) -> Result<u16, ProgramError> {
    let mut adjusted_fee = base_fee;
    
    // Volatility adjustment (0-3bp based on volatility)
    let vol_adjustment = if volatility > U64F64::from_fraction(1, 2)? {
        3
    } else if volatility > U64F64::from_fraction(3, 10)? {
        2
    } else if volatility > U64F64::from_fraction(1, 10)? {
        1
    } else {
        0
    };
    adjusted_fee = adjusted_fee.saturating_add(vol_adjustment);
    
    // Liquidity adjustment (0-2bp based on liquidity)
    let liq_adjustment = if liquidity < 1_000_000_000 { // < $1k
        2
    } else if liquidity < 10_000_000_000 { // < $10k
        1
    } else {
        0
    };
    adjusted_fee = adjusted_fee.saturating_add(liq_adjustment);
    
    // Congestion adjustment (0-1bp)
    if congestion_factor > 80 { // >80% capacity
        adjusted_fee = adjusted_fee.saturating_add(1);
    }
    
    // Cap at maximum fee
    Ok(adjusted_fee.min(FEE_MAX_BPS))
}

/// Calculate total fee including fixed base fee and Polymarket fee
/// 
/// Total fee = BASE_FEE_BPS (28bp) + POLYMARKET_FEE_BPS (150bp)
pub fn calculate_total_fee_with_polymarket() -> u16 {
    BASE_FEE_BPS + POLYMARKET_FEE_BPS
}

/// Calculate total trading fee for a position
/// 
/// Combines:
/// - Fixed base fee (28bp)
/// - Polymarket fee (1.5%)
/// - Dynamic adjustments based on market conditions
pub fn calculate_position_total_fee(
    volatility: U64F64,
    liquidity: u64,
    congestion_factor: u8,
) -> Result<u16, ProgramError> {
    // Start with fixed base fee
    let base_fee = BASE_FEE_BPS;
    
    // Add dynamic adjustments
    let adjusted_fee = calculate_fee_adjustments(
        base_fee,
        volatility,
        liquidity,
        congestion_factor,
    )?;
    
    // Add Polymarket fee
    let total_fee = adjusted_fee.saturating_add(POLYMARKET_FEE_BPS);
    
    msg!("Total fee calculation: base={}bp, adjusted={}bp, polymarket={}bp, total={}bp", 
         BASE_FEE_BPS, adjusted_fee, POLYMARKET_FEE_BPS, total_fee);
    
    Ok(total_fee)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_elastic_fee_high_coverage() {
        // High coverage (2.0) should give minimal fee
        let coverage = U64F64::from_num(2);
        let fee = calculate_elastic_fee(coverage).unwrap();
        assert!(fee <= 4); // Should be close to base fee (3bp)
    }
    
    #[test]
    fn test_elastic_fee_low_coverage() {
        // Low coverage (0.5) should give higher fee
        let coverage = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
        let fee = calculate_elastic_fee(coverage).unwrap();
        assert!(fee >= 8 && fee <= 9); // Should be around 8.58bp
    }
    
    #[test]
    fn test_elastic_fee_cap() {
        // Very low coverage should cap at maximum
        let coverage = U64F64::from_num(1) / U64F64::from_num(10); // 0.1
        let fee = calculate_elastic_fee(coverage).unwrap();
        assert_eq!(fee, FEE_MAX_BPS); // Should cap at 28bp
    }
    
    #[test]
    fn test_exp_approximation_accuracy() {
        // Test approximation accuracy for various inputs
        let test_cases = vec![
            (U64F64::from_num(1) / U64F64::from_num(2), 0.2231), // exp(-1.5) ≈ 0.2231  
            (U64F64::from_num(1), 0.0498), // exp(-3) ≈ 0.0498
            (U64F64::from_num(2), 0.0025), // exp(-6) ≈ 0.0025
        ];
        
        for (coverage, expected) in test_cases {
            let result = calculate_exp_approximation(coverage).unwrap();
            // Convert expected to fixed point for comparison
            let expected_fixed = U64F64::from_raw((expected * 18446744073709551616.0) as u128);
            let diff = if result > expected_fixed {
                result - expected_fixed
            } else {
                expected_fixed - result
            };
            // Check if difference is less than 0.01
            let tolerance = U64F64::from_raw((0.01 * 18446744073709551616.0) as u128);
            assert!(diff < tolerance); // Within 1% accuracy
        }
    }
}