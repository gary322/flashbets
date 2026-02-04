//! Simple test for Uniform LVR Implementation

#[cfg(test)]
mod tests {
    use crate::amm::pmamm::math::{calculate_uniform_lvr, PMAMMPool};
    
    #[test]
    fn test_uniform_lvr_basic() {
        // Create a simple pool for testing
        let pool = PMAMMPool {
            l_parameter: 100_000_000_000, // 100k
            reserves: vec![50_000_000_000, 50_000_000_000], // 50k each
            fee_bps: 30, // 0.3%
        };
        
        // Test amount
        let amount_in = 1_000_000_000; // $1k
        
        // Calculate uniform LVR
        let lvr_fee = calculate_uniform_lvr(&pool, 0, 1, amount_in).unwrap();
        
        // Uniform LVR should be exactly 5% (500 bps)
        let expected_fee = amount_in * 500 / 10_000;
        assert_eq!(lvr_fee, expected_fee, 
            "Uniform LVR fee mismatch: expected {}, got {}", 
            expected_fee, lvr_fee);
        
        println!("✓ Uniform LVR test passed: {} -> {} (5%)", amount_in, lvr_fee);
    }
}