#[cfg(test)]
mod quantum_user_journey {
    use anchor_lang::prelude::*;
    use fixed::types::{U64F64, I64F64};
    use std::collections::HashMap;
    use crate::quantum::*;
    use crate::amm::pm_amm::*;

    #[test]
    fn test_complete_quantum_trading_flow() {
        println!("\n=== Quantum Market User Journey: DAO Governance Proposals ===\n");

        // Step 1: Create quantum market for DAO proposals
        println!("Step 1: Creating quantum market with 4 governance proposals");
        let proposals = vec![
            "Increase staking rewards by 20%".to_string(),
            "Launch new liquidity mining program".to_string(),
            "Buyback and burn 10% of tokens".to_string(),
            "Fund development grants program".to_string(),
        ];

        let mut market = QuantumMarket::new(
            [1u8; 32], // Market ID
            proposals.clone(),
            100000, // Settle in ~28 hours
            CollapseRule::WeightedComposite,
        ).unwrap();

        println!("  Market created with proposals:");
        for (i, desc) in proposals.iter().enumerate() {
            println!("    {}: {}", i, desc);
        }
        println!("  Settlement slot: {}", market.settle_slot);
        println!("  Collapse rule: WeightedComposite (50% prob, 30% vol, 20% traders)");

        // Step 2: Setup PM-AMM for pricing
        let pm_amm = PMAMMState::new(
            U64F64::from_num(5000),
            100000, // Same duration as quantum market
            4,      // 4 proposals
            0,
        ).unwrap();

        // Step 3: Initialize trading system
        let mut trading = QuantumTrading {
            market: market.clone(),
            pm_amm,
            credit_ledger: HashMap::new(),
            proposal_locks: Vec::new(),
        };

        // Step 4: Users deposit and receive phantom credits
        println!("\nStep 2: Users deposit and receive phantom credits");
        
        let alice = Pubkey::new_unique();
        let bob = Pubkey::new_unique();
        let charlie = Pubkey::new_unique();
        
        // Alice deposits 1000 tokens
        let alice_credits = QuantumCredits::deposit_and_allocate(
            alice,
            [1u8; 32],
            1000,
            4,
        ).unwrap();
        trading.credit_ledger.insert(alice, alice_credits);
        println!("  Alice deposits 1000 tokens → 1000 credits per proposal");

        // Bob deposits 2000 tokens
        let bob_credits = QuantumCredits::deposit_and_allocate(
            bob,
            [1u8; 32],
            2000,
            4,
        ).unwrap();
        trading.credit_ledger.insert(bob, bob_credits);
        println!("  Bob deposits 2000 tokens → 2000 credits per proposal");

        // Charlie deposits 500 tokens
        let charlie_credits = QuantumCredits::deposit_and_allocate(
            charlie,
            [1u8; 32],
            500,
            4,
        ).unwrap();
        trading.credit_ledger.insert(charlie, charlie_credits);
        println!("  Charlie deposits 500 tokens → 500 credits per proposal");

        trading.market.total_deposits = 3500;

        // Step 5: Trading phase
        println!("\nStep 3: Trading phase begins");
        
        // Alice strongly supports staking rewards
        let alice_trade1 = trading.place_quantum_trade(
            &alice,
            0, // Staking rewards proposal
            800,
            2, // 2x leverage
            TradeDirection::Buy,
        ).unwrap();
        println!("  Alice: Buy 800 credits on 'Staking rewards' with 2x leverage");
        println!("    New probability: {:.1}%", alice_trade1.new_probability.to_num::<f64>() * 100.0);
        println!("    Credits remaining: {}", alice_trade1.credits_remaining);

        // Bob likes liquidity mining
        let bob_trade1 = trading.place_quantum_trade(
            &bob,
            1, // Liquidity mining
            1500,
            1, // No leverage
            TradeDirection::Buy,
        ).unwrap();
        println!("  Bob: Buy 1500 credits on 'Liquidity mining'");
        println!("    New probability: {:.1}%", bob_trade1.new_probability.to_num::<f64>() * 100.0);

        // Charlie prefers buyback
        let charlie_trade1 = trading.place_quantum_trade(
            &charlie,
            2, // Buyback proposal
            400,
            3, // 3x leverage
            TradeDirection::Buy,
        ).unwrap();
        println!("  Charlie: Buy 400 credits on 'Buyback' with 3x leverage");
        println!("    New probability: {:.1}%", charlie_trade1.new_probability.to_num::<f64>() * 100.0);

        // More trading activity
        trading.place_quantum_trade(&alice, 3, 100, 1, TradeDirection::Buy).ok();
        trading.place_quantum_trade(&bob, 0, 300, 1, TradeDirection::Buy).ok();
        trading.place_quantum_trade(&charlie, 1, 100, 1, TradeDirection::Buy).ok();
        
        // Bob changes mind and sells some liquidity position
        let bob_sell = trading.place_quantum_trade(
            &bob,
            1,
            200,
            1,
            TradeDirection::Sell,
        ).unwrap();
        println!("  Bob: Sells 200 credits of 'Liquidity mining'");
        println!("    New probability: {:.1}%", bob_sell.new_probability.to_num::<f64>() * 100.0);

        // Step 6: Show market state before collapse
        println!("\nStep 4: Market state before collapse");
        println!("  Current probabilities:");
        for (i, proposal) in trading.market.proposals.iter().enumerate() {
            println!("    Proposal {}: {:.1}% (Vol: {}, Traders: {})",
                i,
                proposal.current_probability.to_num::<f64>() * 100.0,
                proposal.total_volume,
                proposal.unique_traders
            );
        }

        // Step 7: Time passes, approach settlement
        println!("\nStep 5: Approaching settlement time");
        
        // Check pre-collapse trigger
        trading.market.check_collapse_trigger(99900).unwrap();
        println!("  Market enters pre-collapse state at slot 99900");
        
        // Lock volatile proposal
        trading.proposal_locks.push(ProposalLock {
            proposal_id: 0,
            locked_until_slot: 100100,
            reason: LockReason::PreCollapse,
        });
        println!("  Proposal 0 locked due to pre-collapse period");

        // Step 8: Collapse execution
        println!("\nStep 6: Market collapse at settlement");
        trading.market.check_collapse_trigger(100000).unwrap();
        trading.market.execute_collapse().unwrap();
        
        let winner = trading.market.winner_index.unwrap();
        println!("  Market collapsed!");
        println!("  Winner: Proposal {} - {}", winner, proposals[winner as usize]);
        
        // Show weighted scores
        println!("\n  Weighted scores breakdown:");
        for (i, proposal) in trading.market.proposals.iter().enumerate() {
            let prob_score = proposal.current_probability.to_num::<f64>() * 0.5;
            let vol_score = (proposal.total_volume as f64 / trading.market.total_deposits as f64) * 0.3;
            let trader_score = (proposal.unique_traders as f64 / 1000.0) * 0.2;
            let total = prob_score + vol_score + trader_score;
            
            println!("    Proposal {}: {:.3} (prob: {:.3}, vol: {:.3}, traders: {:.3})",
                i, total, prob_score, vol_score, trader_score
            );
        }

        // Step 9: Process refunds
        println!("\nStep 7: Processing automatic refunds");
        let refund_summary = trading.process_collapse_refunds().unwrap();
        
        println!("  Refund summary:");
        println!("    Total refunded: {} tokens", refund_summary.total_refunded);
        println!("    Users processed: {}", refund_summary.refund_count);
        
        // Show individual refunds
        println!("\n  Individual results:");
        
        let alice_credits = trading.credit_ledger.get(&alice).unwrap();
        let alice_unused: u64 = alice_credits.used_credits.iter()
            .enumerate()
            .filter(|(i, _)| *i as u8 != winner)
            .map(|(_, c)| alice_credits.credits_per_proposal - c.amount_used)
            .sum();
        println!("    Alice: {} tokens refunded from losing proposals", alice_unused);
        
        let bob_credits = trading.credit_ledger.get(&bob).unwrap();
        let bob_unused: u64 = bob_credits.used_credits.iter()
            .enumerate()
            .filter(|(i, _)| *i as u8 != winner)
            .map(|(_, c)| bob_credits.credits_per_proposal - c.amount_used)
            .sum();
        println!("    Bob: {} tokens refunded from losing proposals", bob_unused);
        
        let charlie_credits = trading.credit_ledger.get(&charlie).unwrap();
        let charlie_unused: u64 = charlie_credits.used_credits.iter()
            .enumerate()
            .filter(|(i, _)| *i as u8 != winner)
            .map(|(_, c)| charlie_credits.credits_per_proposal - c.amount_used)
            .sum();
        println!("    Charlie: {} tokens refunded from losing proposals", charlie_unused);

        // Step 10: Final summary
        println!("\n=== Quantum Market Summary ===");
        println!("  Total deposits: {} tokens", trading.market.total_deposits);
        println!("  Winning proposal: {}", proposals[winner as usize]);
        println!("  Total refunds: {} tokens", refund_summary.total_refunded);
        println!("  Market state: {:?}", trading.market.state);
        
        // Verify all users got refunds
        for (user, credits) in trading.credit_ledger.iter() {
            assert!(credits.refund_claimed, "User {:?} should have refund claimed", user);
        }
        
        println!("\n✅ Quantum market lifecycle completed successfully!");
    }

    #[test]
    fn test_quantum_edge_cases() {
        println!("\n=== Quantum Market Edge Cases ===\n");
        
        // Test 1: User tries to exceed credit limit
        println!("Test 1: Credit limit enforcement");
        let mut credits = QuantumCredits::deposit_and_allocate(
            Pubkey::new_unique(),
            [2u8; 32],
            1000,
            2,
        ).unwrap();
        
        credits.use_credits(0, 900, 1).unwrap();
        let result = credits.use_credits(0, 200, 1);
        assert!(result.is_err());
        println!("  ✓ Cannot use more credits than available");
        
        // Test 2: Market with maximum proposals
        println!("\nTest 2: Maximum proposals (10)");
        let max_proposals: Vec<String> = (0..10)
            .map(|i| format!("Proposal {}", i))
            .collect();
        
        let market = QuantumMarket::new(
            [3u8; 32],
            max_proposals,
            100000,
            CollapseRule::MaxVolume,
        ).unwrap();
        
        assert_eq!(market.proposals.len(), 10);
        println!("  ✓ Market created with maximum 10 proposals");
        
        // Test 3: Different collapse rules
        println!("\nTest 3: Testing all collapse rules");
        let rules = vec![
            CollapseRule::MaxProbability,
            CollapseRule::MaxVolume,
            CollapseRule::MaxTraders,
            CollapseRule::WeightedComposite,
        ];
        
        for rule in rules {
            let mut market = QuantumMarket::new(
                [4u8; 32],
                vec!["A".to_string(), "B".to_string()],
                100000,
                rule.clone(),
            ).unwrap();
            
            market.state = QuantumState::Collapsing;
            market.execute_collapse().unwrap();
            assert!(market.winner_index.is_some());
            println!("  ✓ {:?} collapse rule works", rule);
        }
    }
}