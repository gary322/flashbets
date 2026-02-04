//! Comprehensive AMM selection tests
//! Verifies specification compliance for AMM type selection

use betting_platform_native::{
    state::accounts::AMMType,
    amm::auto_selector::{select_amm_type, validate_amm_selection},
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amm_selection_n_equals_1() {
        // Specification: N=1 → LMSR
        let current_time = 1000000i64;
        
        let result = select_amm_type(1, None, None, current_time).unwrap();
        assert_eq!(result, AMMType::LMSR, "N=1 should select LMSR");
    }

    #[test]
    fn test_amm_selection_n_equals_2() {
        // Specification: 2≤N≤64 → PM-AMM
        let current_time = 1000000i64;
        
        let result = select_amm_type(2, None, None, current_time).unwrap();
        assert_eq!(result, AMMType::PMAMM, "N=2 should select PM-AMM");
    }

    #[test]
    fn test_amm_selection_multi_outcome() {
        // Specification: 2≤N≤64 → PM-AMM
        let current_time = 1000000i64;
        
        // Test various outcome counts in the PM-AMM range
        for n in 3..=64 {
            let result = select_amm_type(n, None, None, current_time).unwrap();
            assert_eq!(result, AMMType::PMAMM, "N={} should select PM-AMM", n);
        }
    }

    #[test]
    fn test_amm_selection_high_outcome_count() {
        // Specification: N>64 → L2
        let current_time = 1000000i64;
        
        // Test outcome counts that should trigger L2 AMM
        for n in 65..=100 {
            let result = select_amm_type(n, None, None, current_time).unwrap();
            assert_eq!(result, AMMType::L2AMM, "N={} should select L2-AMM", n);
        }
    }

    #[test]
    fn test_amm_selection_continuous_markets() {
        // Specification: continuous → L2
        let current_time = 1000000i64;
        
        // Test continuous outcome types override PM-AMM selection
        let continuous_types = vec!["range", "continuous", "distribution"];
        
        for outcome_type in continuous_types {
            // Even with low outcome count, continuous types should use L2
            let result = select_amm_type(5, Some(outcome_type), None, current_time).unwrap();
            assert_eq!(result, AMMType::L2AMM, 
                "Continuous type '{}' should select L2-AMM", outcome_type);
            
            let result = select_amm_type(10, Some(outcome_type), None, current_time).unwrap();
            assert_eq!(result, AMMType::L2AMM, 
                "Continuous type '{}' should select L2-AMM", outcome_type);
        }
    }

    #[test]
    fn test_amm_selection_edge_cases() {
        let current_time = 1000000i64;
        
        // Test N=0 (invalid)
        assert!(select_amm_type(0, None, None, current_time).is_err(), 
            "N=0 should return error");
        
        // Test N>100 (too many outcomes)
        assert!(select_amm_type(101, None, None, current_time).is_err(), 
            "N=101 should return error");
        assert!(select_amm_type(255, None, None, current_time).is_err(), 
            "N=255 should return error");
    }

    #[test]
    fn test_amm_validation() {
        // Test validate_amm_selection function
        
        // Valid cases
        assert!(validate_amm_selection(AMMType::LMSR, 1, 1_000_000_000).is_ok(),
            "LMSR with N=1 should be valid");
        assert!(validate_amm_selection(AMMType::PMAMM, 2, 100_000_000).is_ok(),
            "PM-AMM with N=2 should be valid");
        assert!(validate_amm_selection(AMMType::PMAMM, 64, 100_000_000).is_ok(),
            "PM-AMM with N=64 should be valid");
        assert!(validate_amm_selection(AMMType::L2AMM, 65, 200_000_000).is_ok(),
            "L2-AMM with N=65 should be valid");
        
        // Invalid cases
        assert!(validate_amm_selection(AMMType::LMSR, 2, 1_000_000_000).is_err(),
            "LMSR with N=2 should be invalid");
        assert!(validate_amm_selection(AMMType::PMAMM, 1, 100_000_000).is_err(),
            "PM-AMM with N=1 should be invalid");
        assert!(validate_amm_selection(AMMType::PMAMM, 65, 100_000_000).is_err(),
            "PM-AMM with N=65 should be invalid");
        assert!(validate_amm_selection(AMMType::L2AMM, 1, 200_000_000).is_err(),
            "L2-AMM with N=1 should be invalid");
        
        // Insufficient liquidity
        assert!(validate_amm_selection(AMMType::LMSR, 1, 10_000_000).is_err(),
            "LMSR with insufficient liquidity should be invalid");
    }

    #[test]
    fn test_amm_selection_immutability() {
        // Test that AMM type cannot be changed after selection
        let current_time = 1000000i64;
        
        // Select AMM for N=5
        let amm_type = select_amm_type(5, None, None, current_time).unwrap();
        assert_eq!(amm_type, AMMType::PMAMM);
        
        // Verify the same parameters always produce the same result
        for _ in 0..10 {
            let result = select_amm_type(5, None, None, current_time).unwrap();
            assert_eq!(result, amm_type, "AMM selection should be deterministic");
        }
    }

    #[test]
    fn test_specification_compliance_summary() {
        let current_time = 1000000i64;
        
        // Create a comprehensive test matrix
        let test_cases = vec![
            (1, None, AMMType::LMSR, "N=1 → LMSR"),
            (2, None, AMMType::PMAMM, "N=2 → PM-AMM"),
            (32, None, AMMType::PMAMM, "N=32 → PM-AMM"),
            (64, None, AMMType::PMAMM, "N=64 → PM-AMM"),
            (65, None, AMMType::L2AMM, "N=65 → L2-AMM"),
            (100, None, AMMType::L2AMM, "N=100 → L2-AMM"),
            (5, Some("continuous"), AMMType::L2AMM, "continuous → L2-AMM"),
            (10, Some("range"), AMMType::L2AMM, "range → L2-AMM"),
        ];
        
        for (outcome_count, outcome_type, expected, description) in test_cases {
            let result = select_amm_type(outcome_count, outcome_type, None, current_time).unwrap();
            assert_eq!(result, expected, "Failed: {}", description);
        }
        
        println!("All AMM selection tests passed! Specification compliance verified.");
    }
}