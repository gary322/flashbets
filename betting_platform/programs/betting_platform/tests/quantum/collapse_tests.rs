#[cfg(test)]
mod quantum_tests {
    use anchor_lang::prelude::*;
    use fixed::types::U64F64;
    use crate::quantum::*;

    #[test]
    fn test_quantum_credit_allocation() {
        let credits = QuantumCredits::deposit_and_allocate(
            Pubkey::new_unique(),
            [0u8; 32],
            1000, // $10 deposit
            5,    // 5 proposals
        ).unwrap();

        assert_eq!(credits.credits_per_proposal, 1000);
        assert_eq!(credits.used_credits.len(), 5);
    }

    #[test]
    fn test_quantum_collapse() {
        let mut market = QuantumMarket::new(
            [0u8; 32],
            vec!["Option A".to_string(), "Option B".to_string(), "Option C".to_string()],
            1000, // Settle at slot 1000
            CollapseRule::MaxProbability,
        ).unwrap();

        // Set probabilities
        market.proposals[0].current_probability = U64F64::from_num(0.2);
        market.proposals[1].current_probability = U64F64::from_num(0.5); // Highest
        market.proposals[2].current_probability = U64F64::from_num(0.3);

        // Trigger collapse
        market.state = QuantumState::Collapsing;
        market.execute_collapse().unwrap();

        assert_eq!(market.winner_index, Some(1));
        assert_eq!(market.state, QuantumState::Collapsed);
    }

    #[test]
    fn test_refund_calculation() {
        let mut credits = QuantumCredits::deposit_and_allocate(
            Pubkey::new_unique(),
            [0u8; 32],
            1000,
            3,
        ).unwrap();

        // Use credits on different proposals
        credits.use_credits(0, 500, 10).unwrap(); // Use 500 on proposal 0
        credits.use_credits(1, 1000, 5).unwrap(); // Use all on proposal 1
        credits.use_credits(2, 200, 20).unwrap(); // Use 200 on proposal 2

        // Calculate refunds with proposal 1 as winner
        let outcomes = vec![
            ProposalOutcome { final_price: 0, avg_entry_price: 0 },
            ProposalOutcome { final_price: 100, avg_entry_price: 50 },
            ProposalOutcome { final_price: 0, avg_entry_price: 0 },
        ];

        credits.calculate_refunds(1, &outcomes).unwrap();

        // Should refund unused from losing proposals
        assert_eq!(credits.refund_amount, 500 + 800); // 500 unused from 0, 800 from 2
    }

    #[test]
    fn test_collapse_buffer_period() {
        let mut market = QuantumMarket::new(
            [0u8; 32],
            vec!["A".to_string(), "B".to_string()],
            1000,
            CollapseRule::MaxProbability,
        ).unwrap();

        // Before buffer period
        assert!(!market.check_collapse_trigger(800).unwrap());
        assert_eq!(market.state, QuantumState::Active);

        // During buffer period (1000 - 100 = 900)
        assert!(market.check_collapse_trigger(900).unwrap());
        assert_eq!(market.state, QuantumState::PreCollapse);

        // At settle time
        assert!(market.check_collapse_trigger(1000).unwrap());
        assert_eq!(market.state, QuantumState::Collapsing);
    }

    #[test]
    fn test_weighted_composite_collapse() {
        let mut market = QuantumMarket::new(
            [0u8; 32],
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            1000,
            CollapseRule::WeightedComposite,
        ).unwrap();

        // Set different metrics for each proposal
        market.proposals[0].current_probability = U64F64::from_num(0.6); // High prob
        market.proposals[0].total_volume = 100;
        market.proposals[0].unique_traders = 10;

        market.proposals[1].current_probability = U64F64::from_num(0.3);
        market.proposals[1].total_volume = 500; // High volume
        market.proposals[1].unique_traders = 50;

        market.proposals[2].current_probability = U64F64::from_num(0.1);
        market.proposals[2].total_volume = 200;
        market.proposals[2].unique_traders = 200; // High traders

        market.total_deposits = 1000;
        market.state = QuantumState::Collapsing;
        market.execute_collapse().unwrap();

        // Winner should be determined by weighted score
        // Exact winner depends on the weighting formula
        assert!(market.winner_index.is_some());
    }

    #[test]
    fn test_credit_leverage() {
        let mut credits = QuantumCredits::deposit_and_allocate(
            Pubkey::new_unique(),
            [0u8; 32],
            1000,
            2,
        ).unwrap();

        // Use credits with leverage
        credits.use_credits(0, 100, 10).unwrap(); // 10x leverage

        assert_eq!(credits.used_credits[0].amount_used, 100);
        assert_eq!(credits.used_credits[0].leverage_applied, 10);

        // Calculate PnL with leverage
        let outcome = ProposalOutcome { 
            final_price: 150, 
            avg_entry_price: 100 
        };
        
        let pnl = credits.calculate_position_pnl(&credits.used_credits[0], &outcome).unwrap();
        // PnL should be scaled by leverage
        assert!(pnl > 0);
    }

    #[test]
    fn test_insufficient_credits() {
        let mut credits = QuantumCredits::deposit_and_allocate(
            Pubkey::new_unique(),
            [0u8; 32],
            1000,
            2,
        ).unwrap();

        // Use all credits on one proposal
        credits.use_credits(0, 1000, 1).unwrap();

        // Try to use more credits than available
        let result = credits.use_credits(0, 100, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_collapse_rules() {
        let rules = vec![
            CollapseRule::MaxProbability,
            CollapseRule::MaxVolume,
            CollapseRule::MaxTraders,
            CollapseRule::WeightedComposite,
        ];

        for rule in rules {
            let mut market = QuantumMarket::new(
                [0u8; 32],
                vec!["A".to_string(), "B".to_string()],
                1000,
                rule,
            ).unwrap();

            // Set different winning conditions
            market.proposals[0].current_probability = U64F64::from_num(0.7);
            market.proposals[0].total_volume = 100;
            market.proposals[0].unique_traders = 10;

            market.proposals[1].current_probability = U64F64::from_num(0.3);
            market.proposals[1].total_volume = 200;
            market.proposals[1].unique_traders = 20;

            market.state = QuantumState::Collapsing;
            let result = market.execute_collapse();
            assert!(result.is_ok());
            assert!(market.winner_index.is_some());
        }
    }
}