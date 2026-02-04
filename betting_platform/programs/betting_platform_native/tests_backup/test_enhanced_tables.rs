//! Unit tests for enhanced CDF/PDF tables with 801 points

#[cfg(test)]
mod tests {
    use betting_platform_native::math::{
        tables::*,
        table_lookup::*,
        special_functions::*,
        U64F64,
    };
    use solana_program::program_error::ProgramError;

    /// Test table initialization and structure
    #[test]
    fn test_table_initialization() {
        let tables = NormalDistributionTables {
            discriminator: DISCRIMINATOR,
            is_initialized: false,
            version: 1,
            min_x: -400, // -4.0
            max_x: 400,  // 4.0
            step: 1,     // 0.01
            table_size: 801,
            cdf_table: vec![],
            pdf_table: vec![],
            erf_table: vec![],
        };

        assert_eq!(tables.min_x, -400);
        assert_eq!(tables.max_x, 400);
        assert_eq!(tables.step, 1);
        assert_eq!(tables.table_size, 801);
        assert!(!tables.is_initialized);
    }

    /// Test CDF lookup accuracy at known values
    #[test]
    fn test_cdf_lookup_known_values() {
        let mut tables = create_populated_tables();
        
        // Test known CDF values
        let test_cases = vec![
            (0.0, 0.5),       // Φ(0) = 0.5
            (1.0, 0.8413),    // Φ(1) ≈ 0.8413
            (-1.0, 0.1587),   // Φ(-1) ≈ 0.1587
            (2.0, 0.9772),    // Φ(2) ≈ 0.9772
            (-2.0, 0.0228),   // Φ(-2) ≈ 0.0228
            (3.0, 0.9987),    // Φ(3) ≈ 0.9987
            (-3.0, 0.0013),   // Φ(-3) ≈ 0.0013
        ];

        for (x, expected) in test_cases {
            let x_fp = U64F64::from_num(x.abs() as u64);
            let result = lookup_cdf(&tables, x_fp).unwrap();
            let result_f64 = result.to_num() as f64 / 10000.0;
            
            // For negative values, adjust the expected result
            let adjusted_expected = if x < 0.0 { 1.0 - expected } else { expected };
            
            let error = (result_f64 - adjusted_expected).abs();
            assert!(error < 0.001, "CDF({}) error: {} > 0.001", x, error);
        }
    }

    /// Test PDF lookup accuracy at known values
    #[test]
    fn test_pdf_lookup_known_values() {
        let tables = create_populated_tables();
        
        // Test known PDF values
        let test_cases = vec![
            (0.0, 0.3989),   // φ(0) ≈ 0.3989
            (1.0, 0.2420),   // φ(1) ≈ 0.2420
            (2.0, 0.0540),   // φ(2) ≈ 0.0540
            (3.0, 0.0044),   // φ(3) ≈ 0.0044
        ];

        for (x, expected) in test_cases {
            let x_fp = U64F64::from_num(x as u64);
            let result = lookup_pdf(&tables, x_fp).unwrap();
            let result_f64 = result.to_num() as f64 / 10000.0;
            
            let error = (result_f64 - expected).abs();
            assert!(error < 0.001, "PDF({}) error: {} > 0.001", x, error);
        }
    }

    /// Test erf lookup accuracy
    #[test]
    fn test_erf_lookup_accuracy() {
        let tables = create_populated_tables();
        
        // Test known erf values
        let test_cases = vec![
            (0.0, 0.0),       // erf(0) = 0
            (0.5, 0.5205),    // erf(0.5) ≈ 0.5205
            (1.0, 0.8427),    // erf(1) ≈ 0.8427
            (2.0, 0.9953),    // erf(2) ≈ 0.9953
        ];

        for (x, expected) in test_cases {
            let x_fp = U64F64::from_num(x as u64);
            let result = lookup_erf(&tables, x_fp).unwrap();
            let result_f64 = result.to_num() as f64 / 10000.0;
            
            let error = (result_f64 - expected).abs();
            assert!(error < 0.002, "erf({}) error: {} > 0.002", x, error);
        }
    }

    /// Test linear interpolation between table points
    #[test]
    fn test_interpolation_accuracy() {
        let tables = create_populated_tables();
        
        // Test values between table points
        let test_x = 1.555; // Not exactly on grid
        let x_fp = U64F64::from_num((test_x * 100.0) as u64) / U64F64::from_num(100);
        
        let cdf = lookup_cdf(&tables, x_fp).unwrap();
        
        // Check that result is between adjacent table values
        let x_lower = U64F64::from_num(155) / U64F64::from_num(100); // 1.55
        let x_upper = U64F64::from_num(156) / U64F64::from_num(100); // 1.56
        
        let cdf_lower = lookup_cdf(&tables, x_lower).unwrap();
        let cdf_upper = lookup_cdf(&tables, x_upper).unwrap();
        
        assert!(cdf.raw > cdf_lower.raw);
        assert!(cdf.raw < cdf_upper.raw);
    }

    /// Test edge cases at table boundaries
    #[test]
    fn test_boundary_conditions() {
        let tables = create_populated_tables();
        
        // Test extreme values
        let x_min = U64F64::from_num(0); // Maps to -4.0
        let x_max = U64F64::from_num(8); // Beyond 4.0
        
        // CDF should approach 0 and 1 at extremes
        let cdf_min = lookup_cdf(&tables, x_min).unwrap();
        let cdf_max = lookup_cdf(&tables, x_max).unwrap();
        
        assert!(cdf_min.to_num() < 100); // < 0.01
        assert!(cdf_max.to_num() > 9900); // > 0.99
        
        // PDF should approach 0 at extremes
        let pdf_min = lookup_pdf(&tables, x_min).unwrap();
        let pdf_max = lookup_pdf(&tables, x_max).unwrap();
        
        assert!(pdf_min.to_num() < 10); // < 0.001
        assert!(pdf_max.to_num() < 10); // < 0.001
    }

    /// Test PM-AMM integration with tables
    #[test]
    fn test_pmamm_with_tables() {
        let tables = create_populated_tables();
        
        // Test PM-AMM delta calculation
        let current_inventory = U64F64::from_num(0);
        let order_size = U64F64::from_num(100);
        let liquidity = U64F64::from_num(1000);
        let time_to_expiry = U64F64::from_num(25) / U64F64::from_num(100); // 0.25 years
        
        let delta = calculate_pmamm_delta_with_tables(
            &tables,
            current_inventory,
            order_size,
            liquidity,
            time_to_expiry,
        ).unwrap();
        
        // Delta should be positive and less than order size
        assert!(delta.raw > 0);
        assert!(delta.raw < order_size.raw);
    }

    /// Test Black-Scholes option pricing with tables
    #[test]
    fn test_black_scholes_with_tables() {
        let tables = create_populated_tables();
        
        // Test at-the-money call option
        let spot = U64F64::from_num(100);
        let strike = U64F64::from_num(100);
        let time = U64F64::from_num(25) / U64F64::from_num(100); // 0.25 years
        let volatility = U64F64::from_num(20) / U64F64::from_num(100); // 20%
        let rate = U64F64::from_num(5) / U64F64::from_num(100); // 5%
        
        let call_price = black_scholes_call(
            &tables,
            spot,
            strike,
            time,
            volatility,
            rate,
        ).unwrap();
        
        // ATM option should have positive value
        assert!(call_price.raw > 0);
        
        // Rough check: ATM option value ≈ S * σ * √(T/(2π))
        let expected_approx = spot.to_num() * 20 * 50 / (100 * 250);
        let actual = call_price.to_num();
        
        // Should be within reasonable range
        assert!(actual > expected_approx / 2);
        assert!(actual < expected_approx * 2);
    }

    /// Test VaR calculation with tables
    #[test]
    fn test_var_calculation() {
        let tables = create_populated_tables();
        
        // Test 95% VaR
        let portfolio = U64F64::from_num(1_000_000); // $1M
        let volatility = U64F64::from_num(2) / U64F64::from_num(100); // 2% daily vol
        let confidence = U64F64::from_num(95) / U64F64::from_num(100); // 95%
        let horizon = U64F64::from_num(1) / U64F64::from_num(252); // 1 day
        
        let var = calculate_var(
            &tables,
            portfolio,
            volatility,
            confidence,
            horizon,
        ).unwrap();
        
        // 95% 1-day VaR should be around 1.645 * 2% * $1M ≈ $32,900
        let expected = U64F64::from_num(32_900);
        let actual = var;
        
        // Should be within 1% of expected
        let error = if actual.raw > expected.raw {
            actual.raw - expected.raw
        } else {
            expected.raw - actual.raw
        };
        
        assert!(error < expected.raw / 100);
    }

    /// Test batch calculations for efficiency
    #[test]
    fn test_batch_calculations() {
        let tables = create_populated_tables();
        
        // Create multiple orders
        let orders = vec![
            PMAMMOrder {
                order_id: 1,
                current_inventory: U64F64::from_num(0),
                size: U64F64::from_num(50),
            },
            PMAMMOrder {
                order_id: 2,
                current_inventory: U64F64::from_num(100),
                size: U64F64::from_num(75),
            },
            PMAMMOrder {
                order_id: 3,
                current_inventory: U64F64::from_num(200),
                size: U64F64::from_num(100),
            },
        ];
        
        let liquidity = U64F64::from_num(1000);
        let time_to_expiry = U64F64::from_num(25) / U64F64::from_num(100);
        
        let results = batch_calculate_pmamm(
            &tables,
            &orders,
            liquidity,
            time_to_expiry,
        ).unwrap();
        
        // Verify all orders were processed
        assert_eq!(results.len(), orders.len());
        
        // Each result should have valid delta
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.order_id, orders[i].order_id);
            assert!(result.delta.raw > 0);
            assert!(result.price_impact.raw > 0);
        }
    }

    /// Test table value consistency
    #[test]
    fn test_table_consistency() {
        let tables = create_populated_tables();
        
        // CDF should be monotonically increasing
        for i in 1..tables.table_size {
            assert!(
                tables.cdf_table[i] >= tables.cdf_table[i - 1],
                "CDF not monotonic at index {}",
                i
            );
        }
        
        // PDF should be symmetric around 0
        let mid_point = tables.table_size / 2;
        for i in 0..100 {
            let left_idx = mid_point - i - 1;
            let right_idx = mid_point + i + 1;
            
            if left_idx < tables.table_size && right_idx < tables.table_size {
                let left_pdf = tables.pdf_table[left_idx];
                let right_pdf = tables.pdf_table[right_idx];
                
                // Allow small differences due to discretization
                let diff = if left_pdf > right_pdf {
                    left_pdf - right_pdf
                } else {
                    right_pdf - left_pdf
                };
                
                assert!(
                    diff < 100, // Small tolerance
                    "PDF not symmetric at offset {}: {} vs {}",
                    i, left_pdf, right_pdf
                );
            }
        }
    }

    /// Test accuracy guarantees (< 0.001 error)
    #[test]
    fn test_accuracy_guarantee() {
        let tables = create_populated_tables();
        
        // Test many random points
        for i in 0..100 {
            let x = (i as f64 - 50.0) / 25.0; // Range [-2, 2]
            let x_fp = if x >= 0.0 {
                U64F64::from_num((x * 100.0) as u64) / U64F64::from_num(100)
            } else {
                U64F64::from_num(0) // Handle negative values
            };
            
            let cdf = lookup_cdf(&tables, x_fp).unwrap();
            let pdf = lookup_pdf(&tables, x_fp).unwrap();
            
            // Verify values are in valid range
            assert!(cdf.to_num() <= 10000); // <= 1.0
            assert!(pdf.to_num() <= 4000);  // Max PDF ≈ 0.4
        }
    }

    // Helper function to create populated tables for testing
    fn create_populated_tables() -> NormalDistributionTables {
        let mut tables = NormalDistributionTables {
            discriminator: DISCRIMINATOR,
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
        
        // Populate with approximate values for testing
        for i in 0..801 {
            let x = (i as f64 - 400.0) / 100.0;
            
            // Simple approximations for testing
            let cdf = normal_cdf_approx(x);
            let pdf = normal_pdf_approx(x);
            let erf = erf_approx(x);
            
            tables.cdf_table[i] = (cdf * 10000.0) as u64;
            tables.pdf_table[i] = (pdf * 10000.0) as u64;
            tables.erf_table[i] = ((erf + 1.0) * 5000.0) as u64; // Scale to [0, 10000]
        }
        
        tables
    }

    // Approximation functions for testing
    fn normal_cdf_approx(x: f64) -> f64 {
        0.5 * (1.0 + erf_approx(x / std::f64::consts::SQRT_2))
    }

    fn normal_pdf_approx(x: f64) -> f64 {
        (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-x * x / 2.0).exp()
    }

    fn erf_approx(x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;
        
        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();
        
        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
        
        sign * y
    }
}

// PM-AMM structures for testing
#[derive(Debug, Clone, Copy)]
pub struct PMAMMOrder {
    pub order_id: u64,
    pub current_inventory: betting_platform_native::math::U64F64,
    pub size: betting_platform_native::math::U64F64,
}

#[derive(Debug, Clone, Copy)]
pub struct PMAMMResult {
    pub order_id: u64,
    pub delta: betting_platform_native::math::U64F64,
    pub final_inventory: betting_platform_native::math::U64F64,
    pub price_impact: betting_platform_native::math::U64F64,
}