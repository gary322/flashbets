#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::fixed_math::*;
    use crate::lmsr_amm::LSMRMarket;
    use crate::pm_amm::PMAMMMarket;
    use crate::l2_amm::{L2DistributionAMM, DistributionType, DistributionParams};

    #[test]
    fn test_lmsr_price_sum() {
        // Create binary market
        let b = FixedPoint::from_u64(100);
        let market = LSMRMarket::new(b, 2);
        
        // Get all prices
        let prices = market.all_prices().unwrap();
        assert_eq!(prices.len(), 2);
        
        // Verify sum equals 1
        let sum = prices[0].add(&prices[1]).unwrap();
        let one = FixedPoint::from_u64(1);
        let epsilon = FixedPoint::from_float(0.000001);
        
        let diff = sum.sub(&one).unwrap().abs().unwrap();
        assert!(diff < epsilon, "Price sum should equal 1");
    }

    #[test]
    fn test_lmsr_buy_cost() {
        let b = FixedPoint::from_u64(1000);
        let market = LSMRMarket::new(b, 2);
        
        let shares = FixedPoint::from_u64(100);
        let cost = market.buy_cost(0, shares).unwrap();
        
        // Cost should be positive
        assert!(cost > FixedPoint::zero(), "Buy cost should be positive");
        
        // For small trades relative to b, cost should be close to shares * price
        let price = market.price(0).unwrap();
        let expected_cost = shares.mul(&price).unwrap();
        let cost_ratio = cost.div(&expected_cost).unwrap();
        
        // Should be within 10% for small trades
        let one = FixedPoint::from_u64(1);
        let tolerance = FixedPoint::from_float(0.1);
        let diff = if cost_ratio > one {
            cost_ratio.sub(&one).unwrap()
        } else {
            one.sub(&cost_ratio).unwrap()
        };
        assert!(diff < tolerance, "Cost should be close to shares * price for small trades");
    }

    #[test]
    fn test_pmamm_lvr_calculation() {
        let l = FixedPoint::from_u64(100);
        let t = FixedPoint::from_u64(86400); // 1 day in seconds
        let current_price = FixedPoint::from_float(0.5);
        let inventory = FixedPoint::from_u64(1000);
        
        let market = PMAMMMarket {
            l,
            t,
            current_price,
            inventory,
        };
        
        let current_time = FixedPoint::from_u64(0);
        let lvr = market.calculate_lvr(current_time).unwrap();
        
        // LVR should be positive
        assert!(lvr > FixedPoint::zero(), "LVR should be positive");
        
        // LVR = 0.05 * inventory * price / time_remaining
        let expected_lvr = FixedPoint::from_float(0.05)
            .mul(&inventory).unwrap()
            .mul(&current_price).unwrap()
            .div(&t).unwrap();
        
        let diff = if lvr > expected_lvr {
            lvr.sub(&expected_lvr).unwrap()
        } else {
            expected_lvr.sub(&lvr).unwrap()
        };
        let epsilon = FixedPoint::from_float(0.000001);
        assert!(diff < epsilon, "LVR calculation should match expected");
    }

    #[test]
    fn test_l2_distribution_normal() {
        let k = FixedPoint::from_u64(10);
        let b = FixedPoint::from_u64(5);
        
        let amm = L2DistributionAMM {
            k,
            b,
            distribution_type: DistributionType::Normal {
                mean: 500_000_000_000_000_000, // 0.5 in fixed point
                variance: 100_000_000_000_000_000, // 0.1 in fixed point
            },
            parameters: DistributionParams {
                discretization_points: 10,
                range_min: FixedPoint::from_u64(0),
                range_max: FixedPoint::from_u64(1),
            },
        };
        
        let distribution = amm.calculate_distribution().unwrap();
        
        // Check we have the right number of points
        assert_eq!(distribution.len(), 10);
        
        // Check all values are bounded by b
        for (_, f) in &distribution {
            assert!(*f <= b, "Distribution values should be bounded by b");
        }
        
        // Check values are non-negative
        for (_, f) in &distribution {
            assert!(*f >= FixedPoint::zero(), "Distribution values should be non-negative");
        }
    }

    #[test]
    fn test_hybrid_amm_selection() {
        use crate::hybrid_amm::select_amm_type;
        use crate::hybrid_amm::AMMType;
        
        // Test L2 selection for continuous markets
        let amm_type = select_amm_type(2, 100000, "range_market", 0);
        match amm_type {
            AMMType::L2Distribution => {},
            _ => panic!("Should select L2 for range markets"),
        }
        
        // Test PM-AMM selection for short expiry multi-outcome
        let amm_type = select_amm_type(5, 43200, "multi_outcome", 0);
        match amm_type {
            AMMType::PMAMM => {},
            _ => panic!("Should select PM-AMM for short expiry multi-outcome"),
        }
        
        // Test LMSR selection for binary
        let amm_type = select_amm_type(2, 100000, "binary", 0);
        match amm_type {
            AMMType::LMSR => {},
            _ => panic!("Should select LMSR for binary markets"),
        }
    }

    #[test]
    fn test_advanced_order_generation() {
        use crate::advanced_orders::generate_order_id;
        
        let id1 = generate_order_id();
        let id2 = generate_order_id();
        
        // IDs should be unique
        assert_ne!(id1, id2, "Generated order IDs should be unique");
        
        // IDs should be non-zero
        assert!(id1 > 0, "Order ID should be positive");
        assert!(id2 > 0, "Order ID should be positive");
    }

    #[test]
    fn test_iceberg_order_visibility_rules() {
        let total_size = 1000u64;
        let visible_size = 100u64;
        
        // Check visibility constraint
        assert!(visible_size <= total_size / 10, "Visible size should be at most 10% of total");
        
        // Test reveal logic
        let mut revealed = visible_size;
        let mut remaining = total_size;
        
        // Execute visible portion
        let fill_size = visible_size;
        remaining -= fill_size;
        revealed -= fill_size;
        
        // Reveal next chunk
        if revealed == 0 && remaining > 0 {
            revealed = visible_size.min(remaining);
        }
        
        assert_eq!(revealed, 100, "Should reveal next 100");
        assert_eq!(remaining, 900, "Should have 900 remaining");
    }

    #[test]
    fn test_twap_interval_calculation() {
        let total_size = 1000u64;
        let intervals = 10u8;
        let duration = 1000u64;
        
        let size_per_interval = total_size / intervals as u64;
        let interval_duration = duration / intervals as u64;
        
        assert_eq!(size_per_interval, 100, "Should execute 100 per interval");
        assert_eq!(interval_duration, 100, "Each interval should be 100 slots");
        
        // Simulate full execution
        let mut executed = 0u64;
        for _ in 0..intervals {
            executed += size_per_interval;
        }
        
        assert_eq!(executed, total_size, "Should execute full size");
    }

    #[test]
    fn test_dark_pool_price_improvement() {
        use crate::dark_pool::calculate_improved_price;
        use crate::advanced_orders::OrderSide;
        
        let reference_price = 500_000_000_000_000_000u64; // 0.5
        let improvement_bps = 50u16; // 0.5%
        
        // Test buy side improvement
        let buy_price = calculate_improved_price(reference_price, improvement_bps, &OrderSide::Buy).unwrap();
        let expected_buy = reference_price - (reference_price * improvement_bps as u64 / 10000);
        assert_eq!(buy_price, expected_buy, "Buy price should be improved");
        
        // Test sell side improvement
        let sell_price = calculate_improved_price(reference_price, improvement_bps, &OrderSide::Sell).unwrap();
        let expected_sell = reference_price + (reference_price * improvement_bps as u64 / 10000);
        assert_eq!(sell_price, expected_sell, "Sell price should be improved");
    }
}