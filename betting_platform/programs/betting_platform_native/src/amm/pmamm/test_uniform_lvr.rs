//! Tests for Uniform LVR Implementation

#[cfg(test)]
mod tests {
    use crate::{
        amm::pmamm::math::{calculate_uniform_lvr, calculate_swap_output_with_uniform_lvr},
        state::amm_accounts::{PMAMMMarket, MarketState},
        error::BettingPlatformError,
    };
    use solana_program::program_error::ProgramError;

    fn create_test_pool() -> PMAMMMarket {
        PMAMMMarket {
            discriminator: [0; 8],
            market_id: 1,
            pool_id: 1,
            l_parameter: 100_000_000_000, // 100k
            expiry_time: 0,
            num_outcomes: 2,
            reserves: vec![50_000_000_000, 50_000_000_000], // 50k each
            total_liquidity: 100_000_000_000,
            total_lp_supply: 100_000_000_000,
            liquidity_providers: 1,
            state: crate::state::amm_accounts::MarketState::Active,
            initial_price: 500_000, // 0.5
            probabilities: vec![5000, 5000], // 50% each
            fee_bps: 30, // 0.3%
            oracle: solana_program::pubkey::Pubkey::default(),
            total_volume: 0,
            created_at: 0,
            last_update: 0,
            use_uniform_lvr: true, // Enable uniform LVR
        }
    }

    #[test]
    fn test_uniform_lvr_calculation() {
        let pool = create_test_pool();
        
        // Test various trade sizes
        let test_amounts = vec![
            1_000_000,      // $1
            100_000_000,    // $100
            1_000_000_000,  // $1k
            10_000_000_000, // $10k
        ];
        
        for amount in test_amounts {
            let lvr_fee = calculate_uniform_lvr(&pool, 0, 1, amount).unwrap();
            
            // Uniform LVR should be exactly 5% (500 bps)
            let expected_fee = amount * 500 / 10_000;
            assert_eq!(lvr_fee, expected_fee, 
                "Uniform LVR fee mismatch for amount {}: expected {}, got {}", 
                amount, expected_fee, lvr_fee);
        }
    }

    #[test]
    fn test_swap_with_uniform_lvr() {
        let pool = create_test_pool();
        let amount_in = 1_000_000_000; // $1k
        
        // Calculate output with uniform LVR
        let (output, total_fees) = calculate_swap_output_with_uniform_lvr(&pool, 0, 1, amount_in).unwrap();
        
        // Expected fees: 0.3% base fee + 5% LVR = 5.3%
        let base_fee = amount_in * 30 / 10_000;
        let lvr_fee = amount_in * 500 / 10_000;
        let expected_total_fees = base_fee + lvr_fee;
        
        assert_eq!(total_fees, expected_total_fees,
            "Total fees mismatch: expected {} (base: {} + lvr: {}), got {}",
            expected_total_fees, base_fee, lvr_fee, total_fees);
        
        // Output should be less than input minus fees
        assert!(output < amount_in - total_fees,
            "Output {} should be less than input {} minus fees {}",
            output, amount_in, total_fees);
    }

    #[test]
    fn test_uniform_vs_scaled_lvr() {
        // Create two pools - one with uniform LVR, one without
        let mut uniform_pool = create_test_pool();
        uniform_pool.use_uniform_lvr = true;
        
        let mut scaled_pool = create_test_pool();
        scaled_pool.use_uniform_lvr = false;
        
        let amount_in = 10_000_000_000; // $10k (large trade)
        
        // Get outputs from both pools
        let (uniform_output, uniform_fees) = calculate_swap_output_with_uniform_lvr(&uniform_pool, 0, 1, amount_in).unwrap();
        let (scaled_output, scaled_fees) = calculate_swap_output_with_uniform_lvr(&scaled_pool, 0, 1, amount_in).unwrap();
        
        // For large trades, scaled LVR should charge more than uniform LVR
        assert!(scaled_fees > uniform_fees,
            "Scaled LVR ({}) should charge more than uniform LVR ({}) for large trades",
            scaled_fees, uniform_fees);
        
        // Uniform LVR fees should be exactly 5.3% (0.3% base + 5% LVR)
        let expected_uniform_fees = amount_in * 530 / 10_000;
        assert_eq!(uniform_fees, expected_uniform_fees,
            "Uniform LVR total fees mismatch: expected {}, got {}",
            expected_uniform_fees, uniform_fees);
    }

    #[test]
    fn test_uniform_lvr_edge_cases() {
        let pool = create_test_pool();
        
        // Test zero amount
        let zero_fee = calculate_uniform_lvr(&pool, 0, 1, 0).unwrap();
        assert_eq!(zero_fee, 0, "Zero amount should result in zero fee");
        
        // Test with zero reserves (should error)
        let mut empty_pool = pool.clone();
        empty_pool.reserves = vec![0, 0];
        let result = calculate_uniform_lvr(&empty_pool, 0, 1, 1_000_000);
        assert!(result.is_err(), "Should error with zero reserves");
    }
}