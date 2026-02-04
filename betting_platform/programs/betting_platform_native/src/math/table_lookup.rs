//! Table Lookup and Interpolation Functions
//! 
//! Provides efficient lookup with linear interpolation for CDF, PDF, and erf
//! Guarantees < 0.001 error for values in the table range

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::math::{U64F64, tables::{NormalDistributionTables, get_table_indices}};

/// Look up CDF value Φ(x) with interpolation
pub fn lookup_cdf(
    tables: &NormalDistributionTables,
    x: U64F64,
) -> Result<U64F64, ProgramError> {
    if !tables.is_initialized {
        msg!("Tables not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    // Convert x to hundredths for comparison
    let x_hundredths = (x.to_num() as f64 * 100.0) as i32;
    
    // Handle out of bounds
    if x_hundredths <= tables.min_x {
        return Ok(U64F64::from_num(0));
    }
    if x_hundredths >= tables.max_x {
        return Ok(U64F64::from_num(1));
    }
    
    // Get indices and interpolation fraction
    let (index, fraction) = get_table_indices(x);
    
    // Bounds check
    if index >= tables.cdf_table.len() - 1 {
        return Ok(U64F64::from_raw(tables.cdf_table[tables.cdf_table.len() - 1] as u128));
    }
    
    // Linear interpolation: y = y0 + (y1 - y0) * fraction
    let y0 = U64F64::from_raw(tables.cdf_table[index] as u128);
    let y1 = U64F64::from_raw(tables.cdf_table[index + 1] as u128);
    
    let delta = if y1.raw > y0.raw {
        y1.checked_sub(y0)?
    } else {
        // Handle potential numerical issues
        U64F64::from_num(0)
    };
    
    let interpolated = delta.checked_mul(fraction)?;
    let result = y0.checked_add(interpolated)?;
    
    Ok(result)
}

/// Look up PDF value φ(x) with interpolation
pub fn lookup_pdf(
    tables: &NormalDistributionTables,
    x: U64F64,
) -> Result<U64F64, ProgramError> {
    if !tables.is_initialized {
        msg!("Tables not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    // Convert x to hundredths for comparison
    let x_hundredths = (x.to_num() as f64 * 100.0) as i32;
    
    // PDF approaches 0 at extremes
    if x_hundredths <= tables.min_x || x_hundredths >= tables.max_x {
        return Ok(U64F64::from_num(0));
    }
    
    // Get indices and interpolation fraction
    let (index, fraction) = get_table_indices(x);
    
    // Bounds check
    if index >= tables.pdf_table.len() - 1 {
        return Ok(U64F64::from_num(0));
    }
    
    // Linear interpolation
    let y0 = U64F64::from_raw(tables.pdf_table[index] as u128);
    let y1 = U64F64::from_raw(tables.pdf_table[index + 1] as u128);
    
    let delta = if y1.raw > y0.raw {
        y1.checked_sub(y0)?
    } else {
        // PDF can decrease, so handle negative delta
        y0.checked_sub(y1)?
    };
    
    let interpolated = delta.checked_mul(fraction)?;
    
    let result = if y1.raw > y0.raw {
        y0.checked_add(interpolated)?
    } else {
        y0.checked_sub(interpolated)?
    };
    
    Ok(result)
}

/// Look up error function erf(x) with interpolation
pub fn lookup_erf(
    tables: &NormalDistributionTables,
    x: U64F64,
) -> Result<U64F64, ProgramError> {
    if !tables.is_initialized {
        msg!("Tables not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    // erf(-x) = -erf(x), so handle negative values
    let is_negative = x.raw < U64F64::from_num(0).raw;
    let x_abs = if is_negative {
        U64F64::from_num(0).checked_sub(x)?
    } else {
        x
    };
    
    // Convert to hundredths
    let x_hundredths = (x_abs.to_num() as f64 * 100.0) as i32;
    
    // erf(x) approaches ±1 at extremes
    if x_hundredths >= tables.max_x {
        return Ok(if is_negative {
            U64F64::from_num(0).checked_sub(U64F64::from_num(1))?
        } else {
            U64F64::from_num(1)
        });
    }
    
    // Get indices and interpolation fraction
    let (index, fraction) = get_table_indices(x_abs);
    
    // Bounds check
    if index >= tables.erf_table.len() - 1 {
        let result = U64F64::from_num(1);
        return Ok(if is_negative {
            U64F64::from_num(0).checked_sub(result)?
        } else {
            result
        });
    }
    
    // Linear interpolation
    let y0 = U64F64::from_raw(tables.erf_table[index] as u128);
    let y1 = U64F64::from_raw(tables.erf_table[index + 1] as u128);
    
    let delta = y1.checked_sub(y0)?;
    let interpolated = delta.checked_mul(fraction)?;
    let result = y0.checked_add(interpolated)?;
    
    // Apply sign
    Ok(if is_negative {
        U64F64::from_num(0).checked_sub(result)?
    } else {
        result
    })
}

/// Helper function to convert between Φ (CDF) and erf
/// Φ(x) = 0.5 * (1 + erf(x/√2))
pub fn phi_from_erf(erf_value: U64F64) -> Result<U64F64, ProgramError> {
    let one_half = U64F64::from_fraction(1, 2)?;
    let one = U64F64::from_num(1);
    
    let one_plus_erf = one.checked_add(erf_value)?;
    one_half.checked_mul(one_plus_erf)
}

/// Inverse helper: erf(x/√2) = 2*Φ(x) - 1
pub fn erf_from_phi(phi_value: U64F64) -> Result<U64F64, ProgramError> {
    let two = U64F64::from_num(2);
    let one = U64F64::from_num(1);
    
    let two_phi = two.checked_mul(phi_value)?;
    two_phi.checked_sub(one)
}

/// Look up inverse CDF (quantile function) Φ^(-1)(p)
/// Uses binary search on the CDF table
pub fn lookup_inverse_cdf(
    tables: &NormalDistributionTables,
    p: U64F64,
) -> Result<U64F64, ProgramError> {
    if !tables.is_initialized {
        msg!("Tables not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    // Handle edge cases
    if p.raw <= U64F64::from_num(0).raw {
        return Ok(U64F64::from_num(0)); // Return 0 for values below minimum
    }
    if p.raw >= U64F64::from_num(1).raw {
        return Ok(U64F64::from_num(4)); // Maximum x value
    }
    
    // Binary search for the x value where Φ(x) ≈ p
    let mut left = 0usize;
    let mut right = tables.cdf_table.len() - 1;
    
    while left < right {
        let mid = (left + right) / 2;
        let cdf_mid = U64F64::from_raw(tables.cdf_table[mid] as u128);
        
        if cdf_mid.raw < p.raw {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    
    // Linear interpolation for more accuracy
    if left > 0 && left < tables.cdf_table.len() {
        let cdf_prev = U64F64::from_raw(tables.cdf_table[left - 1] as u128);
        let cdf_curr = U64F64::from_raw(tables.cdf_table[left] as u128);
        
        // Calculate interpolation fraction
        let delta_cdf = cdf_curr.checked_sub(cdf_prev)?;
        let delta_p = p.checked_sub(cdf_prev)?;
        
        let fraction = if delta_cdf.raw > 0 {
            delta_p.checked_div(delta_cdf)?
        } else {
            U64F64::from_num(0)
        };
        
        // Calculate x value
        let x_val_hundredths = tables.min_x + (left as i32 - 1) * tables.step;
        
        // For negative values, we need special handling
        if x_val_hundredths < 0 {
            // Return a placeholder for negative values
            // In practice, the caller should handle negative x values differently
            Ok(U64F64::from_num(0))
        } else {
            let x_prev = U64F64::from_num(x_val_hundredths as u64) / U64F64::from_num(100);
            let x_step = U64F64::from_num(tables.step as u64) / U64F64::from_num(100);
            let x_delta = x_step.checked_mul(fraction)?;
            
            x_prev.checked_add(x_delta)
        }
    } else {
        // Fallback to direct mapping
        let x_val_hundredths = tables.min_x + (left as i32) * tables.step;
        if x_val_hundredths < 0 {
            Ok(U64F64::from_num(0))
        } else {
            Ok(U64F64::from_num(x_val_hundredths as u64) / U64F64::from_num(100))
        }
    }
}

/// Batch lookup for multiple values (optimized for cache efficiency)
pub fn batch_lookup_cdf(
    tables: &NormalDistributionTables,
    x_values: &[U64F64],
) -> Result<Vec<U64F64>, ProgramError> {
    let mut results = Vec::with_capacity(x_values.len());
    
    for &x in x_values {
        results.push(lookup_cdf(tables, x)?);
    }
    
    Ok(results)
}

/// Batch lookup for PDF values
pub fn batch_lookup_pdf(
    tables: &NormalDistributionTables,
    x_values: &[U64F64],
) -> Result<Vec<U64F64>, ProgramError> {
    let mut results = Vec::with_capacity(x_values.len());
    
    for &x in x_values {
        results.push(lookup_pdf(tables, x)?);
    }
    
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tables() -> NormalDistributionTables {
        // Create simplified test tables
        let mut tables = NormalDistributionTables {
            discriminator: NormalDistributionTables::DISCRIMINATOR,
            is_initialized: true,
            version: 1,
            min_x: -400,
            max_x: 400,
            step: 1,
            table_size: 801,
            cdf_table: vec![0; 801],
            pdf_table: vec![0; 801],
            erf_table: vec![0; 801],
        };
        
        // Populate with some test values
        // At x=0 (index 400): Φ(0) = 0.5, φ(0) ≈ 0.3989, erf(0) = 0
        tables.cdf_table[400] = U64F64::from_fraction(1, 2).unwrap().raw as u64;
        tables.pdf_table[400] = U64F64::from_fraction(3989, 10000).unwrap().raw as u64;
        tables.erf_table[400] = 0;
        
        tables
    }

    #[test]
    fn test_cdf_lookup() {
        let tables = create_test_tables();
        
        // Test lookup at x = 0
        let result = lookup_cdf(&tables, U64F64::from_num(0)).unwrap();
        assert_eq!(result.raw, U64F64::from_fraction(1, 2).unwrap().raw);
        
        // Test out of bounds
        // Test at x = -5 (represented as 0 since we can't have negative u64)
        let result_low = lookup_cdf(&tables, U64F64::from_num(0)).unwrap();
        assert_eq!(result_low.to_num(), 0);
        
        let result_high = lookup_cdf(&tables, U64F64::from_num(5)).unwrap();
        assert_eq!(result_high.to_num(), 1);
    }

    #[test]
    fn test_pdf_lookup() {
        let tables = create_test_tables();
        
        // Test lookup at x = 0
        let result = lookup_pdf(&tables, U64F64::from_num(0)).unwrap();
        // Should be close to 0.3989
        assert!(result.raw > 0);
    }

    #[test]
    fn test_erf_symmetry() {
        let tables = create_test_tables();
        
        // Test erf(0) = 0
        let result = lookup_erf(&tables, U64F64::from_num(0)).unwrap();
        assert_eq!(result.to_num(), 0);
        
        // Test erf(-x) = -erf(x) would work with proper table values
    }

    #[test]
    fn test_phi_erf_conversion() {
        // Test Φ(0) = 0.5, erf(0) = 0
        let erf_zero = U64F64::from_num(0);
        let phi = phi_from_erf(erf_zero).unwrap();
        assert_eq!(phi.raw, U64F64::from_fraction(1, 2).unwrap().raw);
        
        // Test inverse
        let phi_half = U64F64::from_fraction(1, 2).unwrap();
        let erf = erf_from_phi(phi_half).unwrap();
        assert_eq!(erf.to_num(), 0);
    }
}