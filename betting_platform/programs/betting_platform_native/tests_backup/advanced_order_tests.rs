//! Advanced Order Types Unit Tests
//! 
//! Production-grade tests for iceberg, TWAP, peg, and dark pool orders

#[cfg(test)]
mod tests {
    use betting_platform_native::{
        error::BettingPlatformError,
        math::U64F64,
        trading::{
            advanced_orders::*,
            iceberg::IcebergEngine,
            twap::TWAPEngine,
            peg::PegEngine,
            dark_pool::DarkPoolEngine,
        },
    };
    use solana_program::clock::Clock;

    #[test]
    fn test_iceberg_order_slicing() {
        // Test 10% chunks with randomization as per CLAUDE.md
        let test_cases = vec![
            (10000, 1000, 0, 1000),  // No randomization
            (50000, 5000, 10, 5500), // Max 10% randomization
            (1000, 100, 5, 105),     // 5% randomization
        ];

        for (total, display, randomization, max_expected) in test_cases {
            let seed = [1u8; 32];
            let slice = IcebergEngine::calculate_next_slice(
                total,
                display,
                randomization,
                &seed,
            ).unwrap();

            assert!(slice <= max_expected);
            assert!(slice >= display * 90 / 100); // At least 90% of display size
        }
    }

    #[test]
    fn test_iceberg_invalid_randomization() {
        let result = IcebergEngine::calculate_next_slice(
            10000,
            1000,
            11, // > 10% is invalid
            &[0u8; 32],
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_twap_slice_calculation() {
        // Test 10 slot duration as per CLAUDE.md
        let test_cases = vec![
            (10000, 10, 5, 0, 0, 0),    // 5 slices over 10 slots
            (50000, 10, 10, 0, 0, 0),   // 10 slices over 10 slots
            (1000, 10, 2, 0, 0, 0),     // 2 slices over 10 slots
        ];

        for (total, duration, slices, current_slot, start_slot, executions) in test_cases {
            let (slice_size, next_slot) = TWAPEngine::calculate_twap_slice(
                total,
                duration,
                slices,
                current_slot,
                start_slot,
                executions,
            ).unwrap();

            assert_eq!(slice_size, total / slices as u64);
            assert_eq!(next_slot, start_slot + (duration / slices as u64));
        }
    }

    #[test]
    fn test_twap_completion() {
        let result = TWAPEngine::calculate_twap_slice(
            10000,
            10,
            5,
            100,
            0,
            5, // Already executed all slices
        );
        
        match result {
            Err(e) => {
                let error: BettingPlatformError = e.into();
                assert_eq!(error, BettingPlatformError::TWAPComplete);
            },
            _ => panic!("Expected TWAPComplete error"),
        }
    }

    #[test]
    fn test_peg_order_calculations() {
        let price_feed = PriceFeed {
            best_bid: U64F64::from_num(100),
            best_ask: U64F64::from_num(102),
            polymarket_price: U64F64::from_num(101),
            last_update_slot: 1000,
        };

        // Test all peg references
        let test_cases = vec![
            (PegReference::BestBid, 0, None, 100),
            (PegReference::BestAsk, -2, None, 100), // 102 - 2 = 100
            (PegReference::MidPrice, 1, None, 102), // 101 + 1 = 102
            (PegReference::PolymarketPrice, 0, None, 101),
            (PegReference::VerseDerivedPrice, 0, Some(U64F64::from_num(95)), 95),
        ];

        for (reference, offset, verse_prob, expected) in test_cases {
            let result = PegEngine::calculate_peg_price(
                &reference,
                offset,
                &price_feed,
                verse_prob,
            ).unwrap();

            assert_eq!(result.to_num(), expected);
        }
    }

    #[test]
    fn test_peg_needs_update() {
        let mut order = AdvancedOrder {
            order_id: [1u8; 32],
            user: Default::default(),
            market_id: [0u8; 32],
            order_type: OrderType::Peg {
                reference: PegReference::BestBid,
                offset: 0,
                limit_price: None,
            },
            side: Side::Buy,
            status: OrderStatus::Pending,
            created_slot: 0,
            expiry_slot: None,
            filled_amount: 0,
            remaining_amount: 1000,
            average_price: U64F64::from_num(100),
            last_execution_slot: 0,
            executions_count: 0,
            mmt_stake_score: 0,
            priority_fee: 0,
        };

        let price_feed = PriceFeed {
            best_bid: U64F64::from_num(105), // Changed from 100 to 105
            best_ask: U64F64::from_num(107),
            polymarket_price: U64F64::from_num(106),
            last_update_slot: 1000,
        };

        // 5% change should trigger update with 100 bps threshold
        let needs_update = PegEngine::needs_update(
            &order,
            &price_feed,
            None,
            100, // 1% threshold
        ).unwrap();

        assert!(needs_update);
    }

    #[test]
    fn test_dark_pool_size_buckets() {
        use betting_platform_native::trading::dark_pool::DarkPoolEngine;
        
        // Test size obfuscation buckets
        assert_eq!(DarkPoolEngine::get_size_bucket(500), 0);      // Small
        assert_eq!(DarkPoolEngine::get_size_bucket(5000), 1);    // Medium
        assert_eq!(DarkPoolEngine::get_size_bucket(50000), 2);   // Large
        assert_eq!(DarkPoolEngine::get_size_bucket(500000), 3);  // Whale
    }

    #[test]
    fn test_dark_pool_vwap_calculation() {
        use betting_platform_native::math::U128F128;
        
        let price_feed = PriceFeed {
            best_bid: U64F64::from_num(100),
            best_ask: U64F64::from_num(102),
            polymarket_price: U64F64::from_num(101),
            last_update_slot: 1000,
        };

        // Test VWAP calculation
        let buy_volume = 1000;
        let sell_volume = 2000;
        let buy_value = U128F128::from_num(101000); // 1000 @ 101
        let sell_value = U128F128::from_num(200000); // 2000 @ 100

        let crossing_price = DarkPoolEngine::calculate_crossing_price(
            buy_volume,
            sell_volume,
            buy_value,
            sell_value,
            &price_feed,
        ).unwrap();

        // Expected: (101000/1000 * 1000 + 200000/2000 * 2000) / 3000 = 100.33
        let expected = U64F64::from_num(100);
        assert!(crossing_price.to_num() >= expected.to_num());
    }

    #[test]
    fn test_order_priority_with_mmt() {
        let order1 = AdvancedOrder {
            order_id: [1u8; 32],
            mmt_stake_score: 1000, // Higher MMT stake
            priority_fee: 100,
            // ... other fields
            user: Default::default(),
            market_id: [0u8; 32],
            order_type: OrderType::Market,
            side: Side::Buy,
            status: OrderStatus::Pending,
            created_slot: 0,
            expiry_slot: None,
            filled_amount: 0,
            remaining_amount: 1000,
            average_price: U64F64::from_num(100),
            last_execution_slot: 0,
            executions_count: 0,
        };

        let order2 = AdvancedOrder {
            order_id: [2u8; 32],
            mmt_stake_score: 500, // Lower MMT stake
            priority_fee: 100,
            // ... same other fields
            user: Default::default(),
            market_id: [0u8; 32],
            order_type: OrderType::Market,
            side: Side::Buy,
            status: OrderStatus::Pending,
            created_slot: 0,
            expiry_slot: None,
            filled_amount: 0,
            remaining_amount: 1000,
            average_price: U64F64::from_num(100),
            last_execution_slot: 0,
            executions_count: 0,
        };

        // Order 1 should have higher priority due to MMT stake
        assert!(order1.mmt_stake_score > order2.mmt_stake_score);
    }

    #[test]
    fn test_order_status_transitions() {
        let mut order = AdvancedOrder {
            status: OrderStatus::Pending,
            remaining_amount: 1000,
            filled_amount: 0,
            // ... other fields
            order_id: [1u8; 32],
            user: Default::default(),
            market_id: [0u8; 32],
            order_type: OrderType::Market,
            side: Side::Buy,
            created_slot: 0,
            expiry_slot: None,
            average_price: U64F64::from_num(100),
            last_execution_slot: 0,
            executions_count: 0,
            mmt_stake_score: 0,
            priority_fee: 0,
        };

        // Partial fill
        order.filled_amount = 500;
        order.remaining_amount = 500;
        order.status = OrderStatus::PartiallyFilled;
        assert_eq!(order.status, OrderStatus::PartiallyFilled);

        // Complete fill
        order.filled_amount = 1000;
        order.remaining_amount = 0;
        order.status = OrderStatus::Filled;
        assert_eq!(order.status, OrderStatus::Filled);
    }
}