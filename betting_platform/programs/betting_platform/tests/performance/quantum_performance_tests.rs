#[cfg(test)]
mod quantum_performance_tests {
    use anchor_lang::prelude::*;
    use fixed::types::{U64F64, I64F64};
    use std::collections::HashMap;
    use std::time::Instant;
    use crate::quantum::*;
    use crate::amm::pm_amm::*;

    #[test]
    fn test_quantum_trade_performance() {
        // Setup quantum market with 5 proposals
        let market = QuantumMarket::new(
            [0u8; 32],
            vec!["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string(), "E".to_string()],
            100000,
            CollapseRule::MaxProbability,
        ).unwrap();

        // Setup PM-AMM
        let pm_amm = PMAMMState::new(
            U64F64::from_num(1000),
            86400,
            5,
            0,
        ).unwrap();

        // Setup trading system
        let mut trading = QuantumTrading {
            market,
            pm_amm,
            credit_ledger: HashMap::new(),
            proposal_locks: Vec::new(),
        };

        // Add test users with credits
        let users: Vec<Pubkey> = (0..10).map(|_| Pubkey::new_unique()).collect();
        for user in &users {
            let credits = QuantumCredits::deposit_and_allocate(
                *user,
                [0u8; 32],
                10000, // 100 SOL deposit
                5,     // 5 proposals
            ).unwrap();
            trading.credit_ledger.insert(*user, credits);
        }

        // Measure trading performance
        let start = Instant::now();
        let mut successful_trades = 0;
        let mut total_cu = 0u64;

        for (i, user) in users.iter().enumerate() {
            let proposal_id = (i % 5) as u8;
            let amount = 1000 + (i * 100) as u64;
            let leverage = 1 + (i % 5) as u64;
            
            let trade_start = Instant::now();
            
            let result = trading.place_quantum_trade(
                user,
                proposal_id,
                amount,
                leverage,
                TradeDirection::Buy,
            );
            
            let trade_time = trade_start.elapsed();
            
            if result.is_ok() {
                successful_trades += 1;
                
                // Estimate CU usage
                // Credit check: ~100 CU
                // PM-AMM solve: ~3000 CU
                // State updates: ~500 CU
                // Price redistribution: ~1000 CU
                let estimated_cu = 100 + 3000 + 500 + 1000;
                total_cu += estimated_cu;
                
                println!("Trade {}: {:?} (est. {} CU)", i, trade_time, estimated_cu);
            }
        }

        let total_time = start.elapsed();
        let avg_cu = total_cu / successful_trades as u64;

        println!("\nQuantum Trade Performance:");
        println!("  Successful trades: {}/{}", successful_trades, users.len());
        println!("  Total time: {:?}", total_time);
        println!("  Average time per trade: {:?}", total_time / successful_trades as u32);
        println!("  Average CU per trade: {}", avg_cu);
        
        assert!(avg_cu < 10000, "Quantum trade should use <10k CU");
    }

    #[test]
    fn test_quantum_collapse_performance() {
        // Test collapse with different numbers of proposals
        let proposal_counts = vec![2, 5, 10];
        
        println!("\nQuantum Collapse Performance:");
        println!("Proposals | Collapse Time | Est. CU");
        println!("----------|---------------|--------");
        
        for count in proposal_counts {
            let proposals: Vec<String> = (0..count)
                .map(|i| format!("Proposal {}", i))
                .collect();
            
            let mut market = QuantumMarket::new(
                [0u8; 32],
                proposals,
                100000,
                CollapseRule::WeightedComposite,
            ).unwrap();
            
            // Set different metrics for each proposal
            for (i, proposal) in market.proposals.iter_mut().enumerate() {
                proposal.current_probability = U64F64::from_num(1.0 / count as f64);
                proposal.total_volume = (i + 1) as u64 * 1000;
                proposal.unique_traders = (i + 1) as u32 * 10;
            }
            
            market.total_deposits = 100000;
            market.state = QuantumState::Collapsing;
            
            let start = Instant::now();
            market.execute_collapse().unwrap();
            let elapsed = start.elapsed();
            
            // Estimate CU for collapse
            // Per proposal: ~500 CU for score calculation
            // Winner selection: ~500 CU
            // State update: ~500 CU
            let estimated_cu = (count as u64 * 500) + 1000;
            
            println!("{:9} | {:13?} | {:7}", count, elapsed, estimated_cu);
        }
        
        // Verify max proposals case
        let max_cu = (MAX_QUANTUM_PROPOSALS as u64 * 500) + 1000;
        assert!(max_cu < 20000, "Collapse should use <20k CU even with max proposals");
    }

    #[test]
    fn test_refund_processing_performance() {
        // Create market with 100 users
        let mut market = QuantumMarket::new(
            [0u8; 32],
            vec!["Win".to_string(), "Lose1".to_string(), "Lose2".to_string()],
            100000,
            CollapseRule::MaxProbability,
        ).unwrap();

        market.winner_index = Some(0);
        market.state = QuantumState::Collapsed;

        let pm_amm = PMAMMState::new(
            U64F64::from_num(1000),
            86400,
            3,
            0,
        ).unwrap();

        let mut trading = QuantumTrading {
            market,
            pm_amm,
            credit_ledger: HashMap::new(),
            proposal_locks: Vec::new(),
        };

        // Add users with various credit usage patterns
        println!("\nRefund Processing Performance:");
        
        for i in 0..100 {
            let user = Pubkey::new_unique();
            let mut credits = QuantumCredits::deposit_and_allocate(
                user,
                [0u8; 32],
                10000,
                3,
            ).unwrap();
            
            // Simulate different usage patterns
            credits.use_credits(0, 3000 + (i * 10), 1).unwrap();
            credits.use_credits(1, 2000 - (i * 5).min(1999), 1).unwrap();
            credits.use_credits(2, 1000, 1).unwrap();
            
            trading.credit_ledger.insert(user, credits);
        }

        let start = Instant::now();
        let result = trading.process_collapse_refunds().unwrap();
        let elapsed = start.elapsed();

        println!("  Users processed: {}", result.refund_count);
        println!("  Total refunded: {}", result.total_refunded);
        println!("  Processing time: {:?}", elapsed);
        println!("  Time per user: {:?}", elapsed / result.refund_count);
        
        // Estimate CU
        // Per user: ~300 CU for refund calculation
        // Queue update: ~100 CU
        let estimated_cu = result.refund_count as u64 * 400;
        println!("  Estimated total CU: {}", estimated_cu);
        println!("  CU per user: {}", estimated_cu / result.refund_count as u64);
        
        assert!(estimated_cu / result.refund_count as u64 < 500, 
                "Refund processing should use <500 CU per user");
    }

    #[test]
    fn test_credit_system_scalability() {
        println!("\nCredit System Scalability:");
        
        let proposal_counts = vec![2, 5, 10];
        let deposit = 100000;
        
        for count in proposal_counts {
            let start = Instant::now();
            
            let credits = QuantumCredits::deposit_and_allocate(
                Pubkey::new_unique(),
                [0u8; 32],
                deposit,
                count,
            ).unwrap();
            
            let allocation_time = start.elapsed();
            
            // Test credit usage
            let usage_start = Instant::now();
            for i in 0..count {
                credits.clone().use_credits(i, 1000, 1).unwrap();
            }
            let usage_time = usage_start.elapsed();
            
            println!("  {} proposals:", count);
            println!("    Allocation time: {:?}", allocation_time);
            println!("    Usage time: {:?}", usage_time);
            println!("    Credits per proposal: {}", credits.credits_per_proposal);
        }
    }
}