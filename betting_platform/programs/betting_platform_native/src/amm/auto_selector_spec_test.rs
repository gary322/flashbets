//! Specification compliance tests for AMM auto-selection
//! 
//! Tests verify:
//! - N=1 → LMSR
//! - 2≤N≤64 → PM-AMM  
//! - N>64 → L2
//! - continuous → L2

#[cfg(test)]
mod spec_compliance_tests {
    use crate::amm::auto_selector::select_amm_type;
    use crate::state::accounts::AMMType;
    
    #[test]
    fn test_spec_n_equals_1_to_lmsr() {
        let result = select_amm_type(1, None, None, 1000000).unwrap();
        assert_eq!(result, AMMType::LMSR, "Spec violation: N=1 must select LMSR");
    }
    
    #[test]
    fn test_spec_n_2_to_64_to_pmamm() {
        // Test N=2
        let result = select_amm_type(2, None, None, 1000000).unwrap();
        assert_eq!(result, AMMType::PMAMM, "Spec violation: N=2 must select PM-AMM");
        
        // Test boundaries
        let result = select_amm_type(64, None, None, 1000000).unwrap();
        assert_eq!(result, AMMType::PMAMM, "Spec violation: N=64 must select PM-AMM");
        
        // Test middle values
        for n in vec![5, 10, 20, 30, 50] {
            let result = select_amm_type(n, None, None, 1000000).unwrap();
            assert_eq!(result, AMMType::PMAMM, "Spec violation: N={} must select PM-AMM", n);
        }
    }
    
    #[test]
    fn test_spec_n_greater_64_to_l2() {
        // Test N=65 (boundary)
        let result = select_amm_type(65, None, None, 1000000).unwrap();
        assert_eq!(result, AMMType::L2AMM, "Spec violation: N=65 must select L2-AMM");
        
        // Test higher values
        for n in vec![70, 80, 90, 100] {
            let result = select_amm_type(n, None, None, 1000000).unwrap();
            assert_eq!(result, AMMType::L2AMM, "Spec violation: N={} must select L2-AMM", n);
        }
    }
    
    #[test]
    fn test_spec_continuous_to_l2() {
        // Test that continuous markets always use L2 regardless of N
        let continuous_types = vec!["range", "continuous", "distribution"];
        
        for outcome_type in continuous_types {
            // Test with low N that would normally be PM-AMM
            let result = select_amm_type(5, Some(outcome_type), None, 1000000).unwrap();
            assert_eq!(result, AMMType::L2AMM, 
                "Spec violation: continuous type '{}' must select L2-AMM", outcome_type);
        }
    }
    
    #[test] 
    fn test_spec_invalid_outcome_counts() {
        // N=0 should error
        assert!(select_amm_type(0, None, None, 1000000).is_err(), 
            "N=0 must return error");
        
        // N>100 should error
        assert!(select_amm_type(101, None, None, 1000000).is_err(),
            "N>100 must return error");
    }
}