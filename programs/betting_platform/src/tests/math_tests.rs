// Comprehensive tests for fixed-point math
// Native Solana - NO ANCHOR

#[cfg(test)]
mod fixed_point_tests {
    use crate::math::fixed_point::{U64F64, MathError, ONE, HALF, E, PI, SQRT2, LN2};
    use crate::math::functions::MathFunctions;
    use crate::math::trigonometry::TrigFunctions;
    use crate::math::utils::{MathUtils, LeverageUtils, FeeUtils};
    
    #[test]
    fn test_basic_arithmetic() {
        // Test addition
        let a = U64F64::from_num(10);
        let b = U64F64::from_num(5);
        let sum = a + b;
        assert_eq!(sum.to_num::<u64>(), 15);
        
        // Test subtraction
        let diff = a - b;
        assert_eq!(diff.to_num::<u64>(), 5);
        
        // Test multiplication
        let product = a * b;
        assert_eq!(product.to_num::<u64>(), 50);
        
        // Test division
        let quotient = a / b;
        assert_eq!(quotient.to_num::<u64>(), 2);
    }
    
    #[test]
    fn test_fractional_operations() {
        // 2.5 * 3.5 = 8.75
        let a = U64F64::from_raw((2u128 << 64) + (1u128 << 63)); // 2.5
        let b = U64F64::from_raw((3u128 << 64) + (1u128 << 63)); // 3.5
        
        let product = a * b;
        let expected = U64F64::from_raw((8u128 << 64) + (3u128 << 62)); // 8.75
        
        // Allow small rounding error
        assert!((product.0 as i128 - expected.0 as i128).abs() < 1000);
        
        // Test division: 7.5 / 2.5 = 3
        let c = U64F64::from_raw((7u128 << 64) + (1u128 << 63)); // 7.5
        let d = U64F64::from_raw((2u128 << 64) + (1u128 << 63)); // 2.5
        let quotient = c / d;
        assert_eq!(quotient.to_num::<u64>(), 3);
    }
    
    #[test]
    fn test_overflow_protection() {
        let max = U64F64::MAX;
        let one = U64F64::ONE;
        
        // Saturating operations should not panic
        let result = max.saturating_add(one);
        assert_eq!(result, U64F64::MAX);
        
        let result = U64F64::ZERO.saturating_sub(one);
        assert_eq!(result, U64F64::ZERO);
        
        // Checked operations should return None on overflow
        assert_eq!(max.checked_add(one), None);
        assert_eq!(U64F64::ZERO.checked_sub(one), None);
    }
    
    #[test]
    fn test_sqrt_accuracy() {
        // Test perfect squares
        let test_cases = vec![
            (4, 2),
            (9, 3),
            (16, 4),
            (25, 5),
            (100, 10),
        ];
        
        for (input, expected) in test_cases {
            let x = U64F64::from_num(input);
            let result = MathFunctions::sqrt(x).unwrap();
            assert_eq!(result.to_num::<u64>(), expected);
        }
        
        // Test sqrt(2) ≈ 1.414
        let x = U64F64::from_num(2);
        let result = MathFunctions::sqrt(x).unwrap();
        let expected = U64F64::from_raw(SQRT2);
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 20)); // Very small difference
    }
    
    #[test]
    fn test_exp_accuracy() {
        // Test exp(0) = 1
        let x = U64F64::ZERO;
        let result = MathFunctions::exp(x).unwrap();
        assert_eq!(result, U64F64::ONE);
        
        // Test exp(1) ≈ e
        let x = U64F64::ONE;
        let result = MathFunctions::exp(x).unwrap();
        let expected = U64F64::from_raw(E);
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 10)); // Reasonable tolerance
        
        // Test exp(-1) ≈ 1/e ≈ 0.368
        let neg_one = U64F64::ZERO.saturating_sub(U64F64::ONE);
        let result = MathFunctions::exp(neg_one).unwrap();
        let one_over_e = U64F64::ONE.checked_div(U64F64::from_raw(E)).unwrap();
        let diff = result.abs_diff(one_over_e);
        assert!(diff.0 < (ONE >> 8));
    }
    
    #[test]
    fn test_ln_accuracy() {
        // Test ln(1) = 0
        let x = U64F64::ONE;
        let result = MathFunctions::ln(x).unwrap();
        assert_eq!(result, U64F64::ZERO);
        
        // Test ln(e) ≈ 1
        let x = U64F64::from_raw(E);
        let result = MathFunctions::ln(x).unwrap();
        let diff = result.abs_diff(U64F64::ONE);
        assert!(diff.0 < (ONE >> 10));
        
        // Test ln(2) ≈ 0.693
        let x = U64F64::from_num(2);
        let result = MathFunctions::ln(x).unwrap();
        let expected = U64F64::from_raw(LN2);
        let diff = result.abs_diff(expected);
        assert!(diff.0 < (ONE >> 10));
    }
    
    #[test]
    fn test_normal_distribution() {
        // Test Φ(0) = 0.5
        let x = U64F64::ZERO;
        let cdf = TrigFunctions::normal_cdf(x).unwrap();
        let expected = U64F64::from_raw(HALF);
        let diff = cdf.abs_diff(expected);
        assert!(diff.0 < (ONE >> 20));
        
        // Test φ(0) ≈ 0.3989 (1/√(2π))
        let pdf = TrigFunctions::normal_pdf(x).unwrap();
        let expected = U64F64::from_raw(7365300113010534); // ≈ 0.3989
        let diff = pdf.abs_diff(expected);
        assert!(diff.0 < (ONE >> 10));
        
        // Test symmetry: Φ(x) + Φ(-x) = 1
        let x = U64F64::from_num(1);
        let neg_x = U64F64::ZERO.saturating_sub(x);
        let cdf_x = TrigFunctions::normal_cdf(x).unwrap();
        let cdf_neg_x = TrigFunctions::normal_cdf(neg_x).unwrap();
        let sum = cdf_x + cdf_neg_x;
        let diff = sum.abs_diff(U64F64::ONE);
        assert!(diff.0 < (ONE >> 10));
    }
    
    #[test]
    fn test_leverage_calculation() {
        // Test basic leverage calculation
        let depth = 5;
        let coverage = U64F64::from_num(2);
        let n_outcomes = 4;
        
        let max_lev = LeverageUtils::calculate_max_leverage(depth, coverage, n_outcomes).unwrap();
        
        // Should be limited by tier cap of 25x for 4 outcomes
        assert_eq!(max_lev.to_num::<u64>(), 25);
        
        // Test with binary outcome (should allow up to 100x)
        let max_lev_binary = LeverageUtils::calculate_max_leverage(depth, coverage, 1).unwrap();
        assert!(max_lev_binary.to_num::<u64>() <= 100);
        
        // Test effective leverage with chaining
        let base_leverage = U64F64::from_num(10);
        let chain_returns = vec![
            U64F64::from_raw(ONE / 10), // 0.1 (10% return)
            U64F64::from_raw(ONE / 5),  // 0.2 (20% return)
        ];
        
        let effective = LeverageUtils::calculate_effective_leverage(base_leverage, &chain_returns).unwrap();
        // 10 * 1.1 * 1.2 = 13.2
        assert_eq!(effective.to_num::<u64>(), 13);
    }
    
    #[test]
    fn test_fee_calculation() {
        // Test elastic fee with high coverage (should be close to 3bp)
        let high_coverage = U64F64::from_num(10);
        let low_fee = FeeUtils::calculate_elastic_fee(high_coverage).unwrap();
        let three_bp = MathUtils::calculate_percentage_bps(U64F64::ONE, 3).unwrap();
        let diff = low_fee.abs_diff(three_bp);
        assert!(diff.0 < (ONE >> 20));
        
        // Test with low coverage (should approach 28bp cap)
        let low_coverage = U64F64::from_raw(ONE / 10); // 0.1
        let high_fee = FeeUtils::calculate_elastic_fee(low_coverage).unwrap();
        let twenty_eight_bp = MathUtils::calculate_percentage_bps(U64F64::ONE, 28).unwrap();
        assert_eq!(high_fee, twenty_eight_bp);
        
        // Test medium coverage
        let medium_coverage = U64F64::from_num(1);
        let medium_fee = FeeUtils::calculate_elastic_fee(medium_coverage).unwrap();
        assert!(medium_fee > three_bp);
        assert!(medium_fee < twenty_eight_bp);
    }
    
    #[test]
    fn test_polymarket_conversion() {
        // Test various probability conversions
        let test_cases = vec![
            (0.0, 0.0),
            (0.25, 25.0),
            (0.5, 50.0),
            (0.75, 75.0),
            (1.0, 100.0),
        ];
        
        for (prob, expected_percentage) in test_cases {
            let fixed = MathUtils::from_polymarket_prob(prob).unwrap();
            let percentage = MathUtils::to_float_percentage(fixed);
            assert!((percentage - expected_percentage).abs() < 0.001);
        }
        
        // Test invalid probabilities
        assert!(MathUtils::from_polymarket_prob(-0.1).is_err());
        assert!(MathUtils::from_polymarket_prob(1.1).is_err());
    }
    
    #[test]
    fn test_compound_interest() {
        // Test compound interest calculation
        let principal = U64F64::from_num(1000);
        let rate = U64F64::from_raw(ONE / 10); // 10% = 0.1
        let periods = 3;
        
        // 1000 * (1.1)^3 = 1331
        let result = MathUtils::compound_interest(principal, rate, periods).unwrap();
        assert_eq!(result.to_num::<u64>(), 1331);
    }
}