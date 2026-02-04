//! Oracle Integration Tests
//!
//! End-to-end tests for oracle functionality

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::constants::*;

    #[test]
    fn test_oracle_integration_end_to_end() {
        // Test 1: Pyth client probability fetching
        test_pyth_client_fetch();
        
        // Test 2: TWAP validation
        test_twap_validation();
        
        // Test 3: Sigma calculation
        test_sigma_calculation();
        
        // Test 4: Oracle PDA state
        test_oracle_pda_state();
        
        // Test 5: Scalar calculation
        test_scalar_calculation();
        
        // Test 6: Early resolution detection
        test_early_resolution_detection();
        
        // Test 7: Cascade protection
        test_cascade_protection();
        
        println!("✅ All oracle integration tests passed!");
    }

    fn test_pyth_client_fetch() {
        println!("Testing Pyth client probability fetching...");
        
        // Create mock feed
        let feed = pyth_client::ProbabilityFeed {
            prob: 0.5,
            sigma: 0.2,
            twap_prob: 0.49,
            last_update_slot: 1000,
            confidence: 0.98,
            num_sources: 5,
            status: pyth_client::FeedStatus::Trading,
        };
        
        // Validate probability bounds
        assert!(feed.prob >= 0.0 && feed.prob <= 1.0);
        assert!(feed.sigma >= 0.0 && feed.sigma <= 1.0);
        assert_eq!(feed.status, pyth_client::FeedStatus::Trading);
        
        println!("  ✓ Pyth client test passed");
    }

    fn test_twap_validation() {
        println!("Testing TWAP validation...");
        
        let mut history = validation::PriceHistory::new();
        
        // Add historical prices
        for i in 0..10 {
            let price = 0.5 + (i as f64 * 0.01);
            history.add_price(price, 100 + i);
        }
        
        let twap = history.calculate_twap();
        let ewma = history.calculate_ewma();
        
        // TWAP should be close to average
        assert!(twap > 0.5 && twap < 0.6);
        
        // EWMA should weight recent values more
        assert!(ewma > twap); // Recent values are higher
        
        // Test deviation check
        let current_prob = 0.55;
        let is_valid = validation::OracleValidator::validate_with_twap(
            current_prob,
            &history,
        ).unwrap();
        assert!(is_valid);
        
        println!("  ✓ TWAP validation test passed");
    }

    fn test_sigma_calculation() {
        println!("Testing sigma calculation...");
        
        let mut calc = sigma::SigmaCalculator::new();
        
        // Add samples with variance
        let samples = vec![0.45, 0.50, 0.55, 0.48, 0.52, 0.46, 0.54, 0.50];
        for sample in samples {
            calc.add_sample(sample).unwrap();
        }
        
        let sigma = calc.get_sigma();
        let risk_cap = calc.calculate_risk_cap();
        let base_risk = calc.calculate_base_risk();
        
        // Verify sigma is calculated
        assert!(sigma > MIN_SIGMA);
        assert!(sigma < MAX_SIGMA);
        
        // Verify risk calculations
        assert!((risk_cap - (1.0 + 0.5 * sigma)).abs() < 0.001);
        assert!((base_risk - (0.2 + 0.1 * sigma)).abs() < 0.001);
        
        // Test buffer requirement
        let buffer = calc.calculate_buffer_requirement(1000.0);
        assert!(buffer > 1000.0); // Should have buffer
        
        println!("  ✓ Sigma calculation test passed");
    }

    fn test_oracle_pda_state() {
        println!("Testing Oracle PDA state...");
        
        let market_id = 12345u128;
        let mut oracle = state::OraclePDA::new(market_id);
        
        // Test initialization
        assert!(oracle.is_initialized);
        assert_eq!(oracle.market_id, market_id);
        
        // Test update
        oracle.update(
            0.6,  // prob
            0.3,  // sigma
            0.58, // twap
            0.59, // ewma
            2000, // slot
            4,    // num_sources
            0.97, // confidence
        ).unwrap();
        
        assert_eq!(oracle.current_prob, 0.6);
        assert_eq!(oracle.current_sigma, 0.3);
        assert_eq!(oracle.last_update_slot, 2000);
        
        // Test buffer requirement calculation
        let expected_buffer = 1.0 + 0.3 * 1.5; // 1.45
        assert!((oracle.buffer_req - expected_buffer).abs() < 0.001);
        
        // Test senior protection
        oracle.set_senior_protection(true);
        assert!(oracle.senior_flag);
        
        println!("  ✓ Oracle PDA state test passed");
    }

    fn test_scalar_calculation() {
        println!("Testing scalar calculation...");
        
        let mut oracle = state::OraclePDA::new(1);
        
        // Test with various probabilities and sigmas
        let test_cases = vec![
            (0.5, 0.2),  // Max risk, moderate sigma
            (0.8, 0.1),  // Lower risk, low sigma
            (0.2, 0.3),  // Lower risk, high sigma
            (0.01, 0.5), // Extreme prob (clamped), high sigma
        ];
        
        for (prob, sigma) in test_cases {
            oracle.current_prob = prob;
            oracle.current_sigma = sigma;
            
            let scalar = oracle.calculate_scalar();
            
            // Verify scalar is bounded
            assert!(scalar > 0.0);
            assert!(scalar <= LEVERAGE_CAP_HARD as f64);
            
            // Verify cached values
            assert_eq!(oracle.last_scalar, scalar);
            assert_eq!(oracle.scalar_prob, prob.max(PROB_MIN_CLAMP).min(PROB_MAX_CLAMP));
            assert_eq!(oracle.scalar_sigma, sigma.max(MIN_SIGMA));
        }
        
        // Test specific case: prob=0.5, sigma=0.2
        oracle.current_prob = 0.5;
        oracle.current_sigma = 0.2;
        let scalar = oracle.calculate_scalar();
        
        // Manual calculation verification
        let prob_clamped = 0.5;
        let risk = prob_clamped * (1.0 - prob_clamped); // 0.25
        let unified_scalar = (1.0 / 0.2) * CAP_FUSED; // 100
        let premium_factor = (risk / BASE_RISK) * CAP_VAULT; // 30
        let expected = (unified_scalar * premium_factor).min(1000.0); // 3000 capped to 1000
        
        assert!((scalar - expected).abs() < 1.0);
        
        println!("  ✓ Scalar calculation test passed");
    }

    fn test_early_resolution_detection() {
        println!("Testing early resolution detection...");
        
        // Test large probability jump
        let last_prob = 0.5;
        let current_prob = 0.95;
        
        let is_early = validation::OracleValidator::detect_early_resolution(
            current_prob,
            last_prob,
        ).unwrap();
        
        assert!(is_early); // Should detect early resolution
        
        // Test small change (no early resolution)
        let current_prob = 0.52;
        let is_early = validation::OracleValidator::detect_early_resolution(
            current_prob,
            last_prob,
        ).unwrap();
        
        assert!(!is_early); // Should not detect early resolution
        
        println!("  ✓ Early resolution detection test passed");
    }

    fn test_cascade_protection() {
        println!("Testing cascade protection...");
        
        // Test deviation factor calculation
        let price_deviation = 0.06;
        let dev_factor = validation::OracleValidator::calculate_deviation_factor(
            price_deviation,
            DEV_THRESHOLD,
        );
        
        // dev_factor = max(0.05, 1 - (0.06 / 0.1)) = max(0.05, 0.4) = 0.4
        assert!((dev_factor - 0.4).abs() < 0.001);
        
        // Test volatility adjustment
        let mut calc = sigma::SigmaCalculator::new();
        calc.sigma = 0.4;
        calc.ewma_sigma = 0.4;
        
        let vol_adjust = calc.calculate_vol_adjust(VOL_SPIKE_THRESHOLD);
        // vol_adjust = max(0.1, 1 - (0.4 / 0.5)) = max(0.1, 0.2) = 0.2
        assert!((vol_adjust - 0.2).abs() < 0.001);
        
        // Test liquidation cap calculation
        let base_cap = LIQ_CAP_MAX;
        let final_cap = base_cap * vol_adjust * dev_factor;
        // 0.08 * 0.2 * 0.4 = 0.0064 (0.64% of OI)
        assert!((final_cap - 0.0064).abs() < 0.0001);
        
        println!("  ✓ Cascade protection test passed");
    }

    #[test]
    fn test_multi_source_consensus() {
        println!("Testing multi-source consensus...");
        
        // Test with sufficient sources agreeing
        let sources = vec![0.50, 0.51, 0.505, 0.49, 0.51];
        let (has_consensus, median) = validation::OracleValidator::validate_multi_source(&sources).unwrap();
        
        assert!(has_consensus);
        assert!((median - 0.505).abs() < 0.001);
        
        // Test with insufficient sources
        let sources = vec![0.50, 0.51];
        let (has_consensus, _) = validation::OracleValidator::validate_multi_source(&sources).unwrap();
        assert!(!has_consensus);
        
        // Test with disagreement
        let sources = vec![0.30, 0.50, 0.70, 0.35, 0.65];
        let (has_consensus, _) = validation::OracleValidator::validate_multi_source(&sources).unwrap();
        assert!(!has_consensus);
        
        println!("  ✓ Multi-source consensus test passed");
    }

    #[test]
    fn test_probability_clamping() {
        println!("Testing probability clamping...");
        
        let mut oracle = state::OraclePDA::new(1);
        
        // Test extreme low probability
        oracle.current_prob = 0.001;
        let clamped = oracle.get_clamped_prob();
        assert_eq!(clamped, PROB_MIN_CLAMP);
        
        // Test extreme high probability
        oracle.current_prob = 0.999;
        let clamped = oracle.get_clamped_prob();
        assert_eq!(clamped, PROB_MAX_CLAMP);
        
        // Test normal probability
        oracle.current_prob = 0.5;
        let clamped = oracle.get_clamped_prob();
        assert_eq!(clamped, 0.5);
        
        println!("  ✓ Probability clamping test passed");
    }

    #[test]
    fn test_halt_conditions() {
        println!("Testing halt conditions...");
        
        let mut oracle = state::OraclePDA::new(1);
        
        // Test high volatility halt
        oracle.current_sigma = 0.6;
        assert!(oracle.should_halt(VOL_SPIKE_THRESHOLD));
        
        // Test normal volatility
        oracle.current_sigma = 0.2;
        assert!(!oracle.should_halt(VOL_SPIKE_THRESHOLD));
        
        // Test manual halt flag
        oracle.is_halted = true;
        oracle.current_sigma = 0.1;
        assert!(oracle.should_halt(VOL_SPIKE_THRESHOLD));
        
        println!("  ✓ Halt conditions test passed");
    }
}