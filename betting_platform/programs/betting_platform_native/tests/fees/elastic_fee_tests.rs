use betting_platform_native::fees::elastic_fee::*;
use betting_platform_native::fees::{FEE_MAX_BPS};
use betting_platform_native::math::U64F64;

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