//! Production Test: VaR Calculation Validation
//!
//! Validates VaR calculations against specification examples

#[cfg(test)]
mod tests {
    use crate::{
        math::{
            U64F64,
            special_functions::{
                NormalDistributionTables,
                calculate_var_specific,
                inverse_normal_cdf,
            },
        },
    };
    
    /// Test the exact VaR formula from specification
    /// VaR = -deposit × Φ⁻¹(0.05) × σ × √t
    /// For deposit=100, σ=0.2, t=1, should return ~32.9
    #[test]
    fn test_var_specification_example() {
        // Create normal distribution tables
        let tables = NormalDistributionTables::new();
        
        // Test parameters from specification
        let deposit = U64F64::from_num(100);
        let sigma = U64F64::from_num(0.2); // 20% volatility
        let time = U64F64::from_num(1); // 1 day
        
        // Calculate VaR
        let var = calculate_var_specific(&tables, deposit, sigma, time).unwrap();
        
        // Convert to f64 for verification
        let var_value = var.to_num() as f64;
        
        println!("VaR Calculation Test:");
        println!("====================");
        println!("Deposit: 100");
        println!("Sigma (volatility): 0.2 (20%)");
        println!("Time: 1 day");
        println!("Calculated VaR: {:.1}", var_value);
        
        // Should be approximately 32.9
        assert!(var_value > 32.0 && var_value < 34.0, 
                "VaR should be ~32.9, got {}", var_value);
        
        // More precise check
        let expected = 32.9;
        let tolerance = 0.5;
        assert!((var_value - expected).abs() < tolerance,
                "VaR {} should be within {} of {}", var_value, tolerance, expected);
    }
    
    /// Test inverse normal CDF accuracy
    #[test]
    fn test_inverse_normal_cdf_accuracy() {
        let tables = NormalDistributionTables::new();
        
        // Test Φ⁻¹(0.05) should be approximately -1.645
        let p_005 = U64F64::from_num(0.05);
        let inv_005 = inverse_normal_cdf(&tables, p_005).unwrap();
        let inv_005_f64 = inv_005.to_num() as f64;
        
        println!("Inverse Normal CDF Tests:");
        println!("========================");
        println!("Φ⁻¹(0.05) = {:.4}", inv_005_f64);
        
        assert!(inv_005_f64 > -1.70 && inv_005_f64 < -1.60,
                "Φ⁻¹(0.05) should be ~-1.645, got {}", inv_005_f64);
        
        // Test other common values
        let test_cases = vec![
            (0.01, -2.326),  // 99% confidence
            (0.025, -1.96),  // 97.5% confidence  
            (0.05, -1.645),  // 95% confidence
            (0.10, -1.282),  // 90% confidence
            (0.50, 0.0),     // Median
        ];
        
        for (p, expected) in test_cases {
            let p_fixed = U64F64::from_num(p);
            let inv = inverse_normal_cdf(&tables, p_fixed).unwrap();
            let inv_f64 = inv.to_num() as f64;
            
            println!("Φ⁻¹({:.3}) = {:.4} (expected ~{:.3})", p, inv_f64, expected);
            
            assert!((inv_f64 - expected).abs() < 0.1,
                    "Φ⁻¹({}) = {} should be close to {}", p, inv_f64, expected);
        }
    }
    
    /// Test VaR with different time horizons
    #[test]
    fn test_var_time_scaling() {
        let tables = NormalDistributionTables::new();
        let deposit = U64F64::from_num(100);
        let sigma = U64F64::from_num(0.2);
        
        // Test different time horizons
        let time_horizons = vec![
            (1, "1 day"),
            (5, "5 days (1 week)"),
            (21, "21 days (1 month)"),
            (252, "252 days (1 year)"),
        ];
        
        println!("\nVaR Time Scaling Test:");
        println!("=====================");
        
        for (days, label) in time_horizons {
            let time = U64F64::from_num(days);
            let var = calculate_var_specific(&tables, deposit, sigma, time).unwrap();
            let var_value = var.to_num() as f64;
            
            // VaR should scale with sqrt(time)
            let expected_scaling = 32.9 * (days as f64).sqrt();
            
            println!("{}: VaR = {:.1} (scaling factor: {:.2})", 
                     label, var_value, (days as f64).sqrt());
            
            assert!((var_value - expected_scaling).abs() < 2.0,
                    "VaR scaling incorrect for {} days", days);
        }
    }
    
    /// Test VaR with different volatility levels
    #[test]
    fn test_var_volatility_scaling() {
        let tables = NormalDistributionTables::new();
        let deposit = U64F64::from_num(100);
        let time = U64F64::from_num(1);
        
        // Test different volatility levels
        let volatilities = vec![
            (0.1, "Low volatility (10%)"),
            (0.2, "Normal volatility (20%)"),
            (0.3, "High volatility (30%)"),
            (0.5, "Extreme volatility (50%)"),
        ];
        
        println!("\nVaR Volatility Scaling Test:");
        println!("===========================");
        
        for (vol, label) in volatilities {
            let sigma = U64F64::from_num(vol);
            let var = calculate_var_specific(&tables, deposit, sigma, time).unwrap();
            let var_value = var.to_num() as f64;
            
            // VaR should scale linearly with volatility
            let expected = 164.5 * vol; // 164.5 = 100 * 1.645
            
            println!("{}: VaR = {:.1} (σ = {})", label, var_value, vol);
            
            assert!((var_value - expected).abs() < 2.0,
                    "VaR scaling incorrect for volatility {}", vol);
        }
    }
    
    /// Test portfolio VaR calculation
    #[test]
    fn test_portfolio_var() {
        let tables = NormalDistributionTables::new();
        
        // Portfolio with multiple positions
        let positions = vec![
            (1_000_000, 0.15),  // $1M position, 15% volatility
            (500_000, 0.25),    // $500k position, 25% volatility
            (750_000, 0.20),    // $750k position, 20% volatility
        ];
        
        let total_value = positions.iter().map(|(v, _)| v).sum::<u64>();
        
        // Calculate individual VaRs
        let mut total_var = 0.0;
        
        println!("\nPortfolio VaR Test:");
        println!("==================");
        println!("Total Portfolio Value: ${}", total_value);
        
        for (i, (value, vol)) in positions.iter().enumerate() {
            let deposit = U64F64::from_num(*value as f64 / 1_000_000.0);
            let sigma = U64F64::from_num(*vol);
            let time = U64F64::from_num(1);
            
            let var = calculate_var_specific(&tables, deposit, sigma, time).unwrap();
            let var_value = var.to_num() as f64 * 1_000_000.0;
            
            println!("Position {}: ${} @ {}% vol => VaR ${:.0}", 
                     i + 1, value, (vol * 100.0), var_value);
            
            total_var += var_value;
        }
        
        // Portfolio VaR (assuming no correlation)
        let portfolio_var = total_var;
        let var_percentage = (portfolio_var / total_value as f64) * 100.0;
        
        println!("Total VaR (no correlation): ${:.0} ({:.1}%)", 
                 portfolio_var, var_percentage);
        
        // Verify reasonable range
        assert!(var_percentage > 2.0 && var_percentage < 10.0,
                "Portfolio VaR percentage should be reasonable");
    }
    
    /// Test edge cases and error handling
    #[test]
    fn test_var_edge_cases() {
        let tables = NormalDistributionTables::new();
        
        // Test zero deposit
        let zero_deposit = U64F64::from_num(0);
        let sigma = U64F64::from_num(0.2);
        let time = U64F64::from_num(1);
        
        let var = calculate_var_specific(&tables, zero_deposit, sigma, time).unwrap();
        assert_eq!(var.to_num(), 0, "VaR of zero deposit should be zero");
        
        // Test zero volatility
        let deposit = U64F64::from_num(100);
        let zero_sigma = U64F64::from_num(0);
        
        let var = calculate_var_specific(&tables, deposit, zero_sigma, time).unwrap();
        assert_eq!(var.to_num(), 0, "VaR with zero volatility should be zero");
        
        // Test very small values (precision test)
        let small_deposit = U64F64::from_num(0.01); // 1 cent
        let var = calculate_var_specific(&tables, small_deposit, sigma, time).unwrap();
        let var_value = var.to_num() as f64;
        
        assert!(var_value > 0.0 && var_value < 0.01,
                "Small deposit VaR should scale appropriately");
    }
    
    /// Test VaR formula components separately
    #[test]
    fn test_var_formula_components() {
        let tables = NormalDistributionTables::new();
        
        // Break down the formula: VaR = -deposit × Φ⁻¹(0.05) × σ × √t
        
        // Component 1: Φ⁻¹(0.05)
        let alpha = U64F64::from_num(0.05);
        let inv_cdf = inverse_normal_cdf(&tables, alpha).unwrap();
        let inv_cdf_value = inv_cdf.to_num() as f64;
        println!("\nVaR Formula Components:");
        println!("======================");
        println!("Φ⁻¹(0.05) = {:.4}", inv_cdf_value);
        
        // Component 2: -Φ⁻¹(0.05) (should be positive)
        let neg_inv_cdf = -inv_cdf_value;
        println!("-Φ⁻¹(0.05) = {:.4}", neg_inv_cdf);
        assert!(neg_inv_cdf > 0.0, "Negated inverse CDF should be positive");
        
        // Component 3: Full calculation
        let deposit = 100.0;
        let sigma = 0.2;
        let sqrt_t = 1.0; // sqrt(1) = 1
        
        let manual_var = deposit * neg_inv_cdf * sigma * sqrt_t;
        println!("Manual calculation: {} × {:.4} × {} × {} = {:.1}",
                 deposit, neg_inv_cdf, sigma, sqrt_t, manual_var);
        
        // Compare with function result
        let deposit_fixed = U64F64::from_num(deposit);
        let sigma_fixed = U64F64::from_num(sigma);
        let time_fixed = U64F64::from_num(1);
        
        let function_var = calculate_var_specific(&tables, deposit_fixed, sigma_fixed, time_fixed).unwrap();
        let function_var_value = function_var.to_num() as f64;
        
        println!("Function result: {:.1}", function_var_value);
        
        assert!((manual_var - function_var_value).abs() < 1.0,
                "Manual and function calculations should match");
    }
}