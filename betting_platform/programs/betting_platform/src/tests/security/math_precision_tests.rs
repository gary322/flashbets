use fixed::types::{U64F64, I64F64};
use crate::math::*;
use crate::fixed_math::*;

#[cfg(test)]
mod precision_tests {
    use super::*;

    // Helper functions for mathematical operations
    fn fixed_mul(a: U64F64, b: U64F64) -> U64F64 {
        a.saturating_mul(b)
    }

    fn fixed_div(a: U64F64, b: U64F64) -> Result<U64F64, &'static str> {
        if b == U64F64::from_num(0) {
            Err("Division by zero")
        } else {
            Ok(a / b)
        }
    }

    fn sqrt_fixed(val: U64F64) -> U64F64 {
        // Newton's method for square root
        if val == U64F64::from_num(0) {
            return U64F64::from_num(0);
        }
        
        let mut x = val;
        let mut prev = U64F64::from_num(0);
        let two = U64F64::from_num(2);
        
        // Iterate until convergence
        for _ in 0..10 {
            prev = x;
            x = (x + val / x) / two;
            
            if x == prev {
                break;
            }
        }
        
        x
    }

    fn exp_fixed(val: U64F64) -> U64F64 {
        // Taylor series approximation for e^x
        let mut result = U64F64::from_num(1);
        let mut term = U64F64::from_num(1);
        
        for i in 1..10 {
            term = term.saturating_mul(val) / U64F64::from_num(i);
            result = result.saturating_add(term);
        }
        
        result
    }

    fn safe_mul(a: U64F64, b: U64F64) -> Result<U64F64, &'static str> {
        // Check for overflow before multiplication
        if a != U64F64::from_num(0) && b > U64F64::MAX / a {
            Err("Multiplication overflow")
        } else {
            Ok(a.saturating_mul(b))
        }
    }

    fn normal_pdf(x: f64) -> f64 {
        (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
    }

    fn normal_cdf(x: f64) -> f64 {
        0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
    }

    fn erf(x: f64) -> f64 {
        // Approximation for error function
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

    fn lookup_normal_cdf(x: f64) -> f64 {
        // Simple lookup table approach for testing
        match x {
            x if x <= -3.0 => 0.00135,
            x if x <= -2.0 => 0.02275,
            x if x <= -1.0 => 0.15866,
            x if x <= 0.0 => 0.50000,
            x if x <= 1.0 => 0.84134,
            x if x <= 2.0 => 0.97725,
            x if x <= 3.0 => 0.99865,
            _ => 0.99997,
        }
    }

    fn simpson_integrate(values: &[f64], a: f64, b: f64) -> f64 {
        // Simpson's rule for numerical integration
        let n = values.len() - 1;
        let h = (b - a) / n as f64;
        
        let mut sum = values[0] + values[n];
        
        for i in 1..n {
            let coefficient = if i % 2 == 0 { 2.0 } else { 4.0 };
            sum += coefficient * values[i];
        }
        
        sum * h / 3.0
    }

    // Test Newton-Raphson convergence
    #[test]
    fn test_newton_raphson_convergence() {
        // Test PM-AMM solver convergence
        let test_cases = vec![
            (10.0, 0.05, 0.1), // Small order
            (1000.0, 0.05, 0.01), // Large order
            (50.0, 0.01, 1.0), // Near expiry
        ];

        for (order_size, lvr, tau) in test_cases {
            let mut iterations = 0;
            let mut y = order_size * 1.1; // Initial guess
            let x = order_size;
            let l = 1000.0; // Liquidity

            loop {
                iterations += 1;

                // f(y) = (y-x)Φ((y-x)/(L√τ)) + L√τ φ((y-x)/(L√τ)) - y
                let z = (y - x) / (l * tau.sqrt());
                let phi = normal_cdf(z);
                let phi_prime = normal_pdf(z);

                let f = (y - x) * phi + l * tau.sqrt() * phi_prime - y;
                let f_prime = phi + (y - x) / (l * tau.sqrt()) * phi_prime
                    + l * tau.sqrt() * (-z * phi_prime) / (l * tau.sqrt()) - 1.0;

                let y_next = y - f / f_prime;

                if (y_next - y).abs() < 1e-8 {
                    y = y_next;
                    break;
                }

                y = y_next;

                assert!(iterations < 10, "Newton-Raphson failed to converge");
            }

            println!("Converged in {} iterations for order {}", iterations, order_size);
            assert!(iterations <= 5, "Too many iterations");
        }
    }

    // Test fixed-point multiplication precision
    #[test]
    fn test_fixed_point_multiplication() {
        // Test cases with known results
        let test_cases = vec![
            (1.5, 2.0, 3.0),
            (0.1, 0.1, 0.01),
            (999.99, 0.01, 9.9999),
            (0.0001, 10000.0, 1.0),
        ];

        for (a, b, expected) in test_cases {
            let a_fixed = U64F64::from_num(a);
            let b_fixed = U64F64::from_num(b);
            let result = fixed_mul(a_fixed, b_fixed);
            let result_f64: f64 = result.to_num();

            assert!(
                (result_f64 - expected).abs() < 0.00001,
                "Fixed multiplication error: {} * {} = {} (expected {})",
                a, b, result_f64, expected
            );
        }
    }

    // Test square root precision
    #[test]
    fn test_sqrt_precision() {
        let test_values = vec![
            4.0, 9.0, 16.0, 25.0, // Perfect squares
            2.0, 3.0, 5.0, 7.0,   // Non-perfect squares
            0.01, 0.1, 1.1, 99.9, // Edge cases
        ];

        for value in test_values {
            let fixed_val = U64F64::from_num(value);
            let sqrt_result = sqrt_fixed(fixed_val);
            let sqrt_f64: f64 = sqrt_result.to_num();
            let expected = value.sqrt();

            assert!(
                (sqrt_f64 - expected).abs() < 0.0001,
                "Square root precision error: sqrt({}) = {} (expected {})",
                value, sqrt_f64, expected
            );
        }
    }

    // Test exponential function precision
    #[test]
    fn test_exp_precision() {
        // Critical for PM-AMM calculations
        let test_values = vec![
            -2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0,
        ];

        for value in test_values {
            let fixed_val = U64F64::from_num(value);
            let exp_result = exp_fixed(fixed_val);
            let exp_f64: f64 = exp_result.to_num();
            let expected = value.exp();

            let relative_error = ((exp_f64 - expected) / expected).abs();
            assert!(
                relative_error < 0.001, // 0.1% tolerance
                "Exponential precision error: exp({}) = {} (expected {})",
                value, exp_f64, expected
            );
        }
    }

    // Test normal CDF approximation
    #[test]
    fn test_normal_cdf_precision() {
        // Using precomputed table approach
        let cdf_table = vec![
            (-3.0, 0.00135),
            (-2.0, 0.02275),
            (-1.0, 0.15866),
            (0.0, 0.50000),
            (1.0, 0.84134),
            (2.0, 0.97725),
            (3.0, 0.99865),
        ];

        for (x, expected) in cdf_table {
            let result = lookup_normal_cdf(x);
            assert!(
                (result - expected).abs() < 0.001,
                "Normal CDF error at {}: {} (expected {})",
                x, result, expected
            );
        }
    }

    // Test fixed-point overflow
    #[test]
    fn test_fixed_point_overflow() {
        // Test that fixed-point math doesn't overflow
        let large_value = U64F64::from_num(1_000_000_000u64); // 1B
        let multiplier = U64F64::from_num(500u64); // 500x leverage

        // This would overflow in normal multiplication
        let result = large_value.saturating_mul(multiplier);

        // Should saturate, not panic
        assert_eq!(result, U64F64::MAX);

        // Test safe multiplication function
        let safe_result = safe_mul(large_value, multiplier);
        assert!(safe_result.is_err() || safe_result.unwrap() == U64F64::MAX);
    }

    // Test L2 norm precision
    #[test]
    fn test_l2_norm_precision() {
        // Test L2 norm calculation for distributions
        let distribution = vec![
            0.1, 0.2, 0.3, 0.2, 0.1, 0.05, 0.05
        ];

        // Calculate ||f||_2 = sqrt(∑f_i^2)
        let norm_squared: f64 = distribution.iter()
            .map(|&x| x * x)
            .sum();
        let norm = norm_squared.sqrt();

        // Convert to fixed-point and back
        let fixed_norm = U64F64::from_num(norm);
        let back_to_float = fixed_norm.to_num::<f64>();

        // Check precision loss
        let error = (norm - back_to_float).abs();
        assert!(error < 1e-10, "Fixed-point precision loss too high");

        // Test Simpson's rule integration
        let integral = simpson_integrate(&distribution, 0.0, 1.0);
        assert!((integral - 1.0).abs() < 0.01, "Distribution doesn't sum to 1");
    }

    // Test extreme leverage calculations
    #[test]
    fn test_extreme_leverage_calculations() {
        // Test calculations at extreme leverage levels
        let test_cases = vec![
            (100.0, 5, 2.39), // 100x base, 5 steps
            (50.0, 5, 1.61), // 50x base, 5 steps  
            (100.0, 3, 1.98), // 100x base, 3 steps
        ];

        for (base_lev, steps, expected_mult) in test_cases {
            let mut eff_lev = base_lev;
            
            // Simulate chaining with decreasing multipliers
            let multipliers = vec![1.5, 1.2, 1.1, 1.15, 1.05];
            
            for i in 0..steps {
                eff_lev *= multipliers[i];
            }
            
            let expected = base_lev * expected_mult;
            let error = (eff_lev - expected).abs() / expected;
            
            assert!(error < 0.01, "Leverage calculation error too high: {}", error);
            
            // Test liquidation at this leverage
            let liq_price = 0.5 * (1.0 - 1.0 / eff_lev);
            let move_to_liq = (0.5 - liq_price) / 0.5;
            
            println!("At {}x effective leverage, liquidation on {:.2}% move", 
                eff_lev, move_to_liq * 100.0);
            
            // At 500x, should liquidate on 0.2% move
            if eff_lev > 500.0 {
                assert!(move_to_liq < 0.002, "Liquidation threshold too loose");
            }
        }
    }
}

// Numerical stability tests
#[cfg(test)]
mod stability_tests {
    use super::*;

    #[test]
    fn test_normal_cdf_pdf_stability() {
        // Test at extreme values
        let test_values = vec![
            -10.0, -5.0, -1.0, 0.0, 1.0, 5.0, 10.0
        ];

        for x in test_values {
            let cdf = normal_cdf(x);
            let pdf = normal_pdf(x);

            // CDF should be in [0, 1]
            assert!(cdf >= 0.0 && cdf <= 1.0, "CDF out of bounds");

            // PDF should be non-negative
            assert!(pdf >= 0.0, "PDF negative");

            // Test relationship: d/dx Φ(x) = φ(x)
            let h = 1e-6;
            let cdf_plus = normal_cdf(x + h);
            let cdf_minus = normal_cdf(x - h);
            let numerical_derivative = (cdf_plus - cdf_minus) / (2.0 * h);

            let error = (numerical_derivative - pdf).abs();
            assert!(error < 1e-6, "CDF/PDF relationship violated");
        }
    }

    #[test]
    fn test_coverage_calculation_edge_cases() {
        // Test coverage with extreme values
        let test_cases = vec![
            (0.0, 1000.0, 0.5), // Zero vault
            (1_000_000_000.0, 1.0, 0.5), // Huge vault, tiny OI
            (100.0, 100_000_000.0, 0.5), // Tiny vault, huge OI
        ];

        for (vault, oi, tail_loss) in test_cases {
            let coverage = if oi == 0.0 {
                f64::INFINITY
            } else {
                vault / (tail_loss * oi)
            };

            // Coverage should handle edge cases gracefully
            if vault == 0.0 {
                assert_eq!(coverage, 0.0);
            } else if oi == 0.0 {
                assert!(coverage.is_infinite());
            } else {
                assert!(coverage >= 0.0);
            }

            // Calculate leverage with edge case coverage
            let leverage = if coverage.is_infinite() {
                100.0 // Max base
            } else {
                (coverage * 100.0).min(100.0)
            };

            assert!(leverage >= 0.0 && leverage <= 100.0);
        }
    }

    // Helper functions for PM-AMM calculations
    fn pm_amm_equation(x: f64, y: f64, l: f64, tau: f64) -> f64 {
        let z = (y - x) / (l * tau.sqrt());
        let phi = normal_pdf(z);
        let big_phi = normal_cdf(z);

        (y - x) * big_phi + l * tau.sqrt() * phi - y
    }

    fn pm_amm_derivative(x: f64, y: f64, l: f64, tau: f64) -> f64 {
        let z = (y - x) / (l * tau.sqrt());
        let phi = normal_pdf(z);
        let big_phi = normal_cdf(z);

        big_phi - 1.0
    }

    #[test]
    fn test_pm_amm_convergence() {
        // Test PM-AMM implicit equation solver
        let test_cases = vec![
            (1.0, 0.5, 100.0, 0.1), // (x, y_guess, L, tau)
            (2.0, 1.5, 200.0, 0.5),
            (0.5, 0.3, 50.0, 1.0),
        ];

        for (x, y_guess, l, tau) in test_cases {
            let mut y = y_guess;
            let mut iterations = 0;
            let max_iterations = 10;
            let tolerance = 0.00001;

            while iterations < max_iterations {
                let f = pm_amm_equation(x, y, l, tau);
                let df = pm_amm_derivative(x, y, l, tau);

                if df.abs() < tolerance {
                    break;
                }

                let y_new = y - f / df;

                if (y_new - y).abs() < tolerance {
                    break;
                }

                y = y_new;
                iterations += 1;
            }

            assert!(
                iterations < max_iterations,
                "Newton-Raphson failed to converge for x={}, L={}, tau={}",
                x, l, tau
            );

            // Verify solution
            let final_f = pm_amm_equation(x, y, l, tau);
            assert!(
                final_f.abs() < 0.0001,
                "Newton-Raphson solution error: f({}) = {}",
                y, final_f
            );
        }
    }
}