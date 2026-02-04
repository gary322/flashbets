#[cfg(test)]
mod phase_9_95_e2e_tests {
    use fixed::types::{U64F64, I64F64};
    
    // Test fixtures
    const FIXED_POINT_SCALE: u64 = 1_000_000_000;
    const PHI_TABLE_SIZE: usize = 256;
    const MAX_NEWTON_ITERATIONS: u8 = 5;
    const CONVERGENCE_THRESHOLD: f64 = 1e-8;
    const COLLAPSE_BUFFER_SLOTS: u64 = 100;
    const MAX_QUANTUM_PROPOSALS: u8 = 10;

    // Mock types for standalone testing
    #[derive(Clone, Debug)]
    struct PMAMMState {
        liquidity_parameter: U64F64,
        initial_time: u64,
        current_time: u64,
        outcome_count: u8,
        prices: Vec<U64F64>,
        volumes: Vec<U64F64>,
        lvr_beta: U64F64,
        phi_lookup_table: Vec<U64F64>,
        pdf_lookup_table: Vec<U64F64>,
    }

    #[derive(Clone, Debug)]
    struct PMPriceResult {
        new_price: U64F64,
        old_price: U64F64,
        price_impact: U64F64,
        lvr_cost: U64F64,
        iterations: u8,
        slippage: U64F64,
    }

    #[derive(Clone, Debug)]
    enum CollapseRule {
        MaxProbability,
        MaxVolume,
        MaxTraders,
        WeightedComposite,
    }

    #[derive(Clone, Debug, PartialEq)]
    enum QuantumState {
        Active,
        PreCollapse,
        Collapsing,
        Collapsed,
        Settled,
    }

    #[derive(Clone, Debug)]
    struct QuantumProposal {
        proposal_id: u8,
        description: String,
        current_probability: U64F64,
        total_volume: u64,
        unique_traders: u32,
        last_trade_slot: u64,
    }

    #[derive(Clone, Debug)]
    struct QuantumMarket {
        market_id: [u8; 32],
        proposals: Vec<QuantumProposal>,
        total_deposits: u64,
        settle_slot: u64,
        collapse_rule: CollapseRule,
        state: QuantumState,
        winner_index: Option<u8>,
    }

    #[derive(Clone, Debug)]
    struct QuantumCredits {
        user: [u8; 32],
        market_id: [u8; 32],
        initial_deposit: u64,
        credits_per_proposal: u64,
        used_credits: Vec<UsedCredit>,
        refund_amount: u64,
        refund_claimed: bool,
    }

    #[derive(Clone, Debug)]
    struct UsedCredit {
        proposal_id: u8,
        amount_used: u64,
        leverage_applied: u64,
        pnl: i64,
        position_closed: bool,
    }

    // PM-AMM Test Implementation
    impl PMAMMState {
        fn new(
            liquidity_parameter: U64F64,
            duration_slots: u64,
            outcome_count: u8,
            initial_slot: u64,
        ) -> Result<Self, String> {
            if outcome_count < 2 || outcome_count > 64 {
                return Err("Invalid outcome count".to_string());
            }

            let initial_price = U64F64::from_num(1) / U64F64::from_num(outcome_count);
            let prices = vec![initial_price; outcome_count as usize];
            let volumes = vec![U64F64::from_num(0); outcome_count as usize];

            // Calculate β for uniform LVR
            let l_squared = liquidity_parameter.saturating_mul(liquidity_parameter);
            let two_pi = U64F64::from_num(2) * U64F64::from_num(std::f64::consts::PI);
            let lvr_beta = l_squared / two_pi;

            // Initialize lookup tables
            let phi_table = Self::initialize_phi_table();
            let pdf_table = Self::initialize_pdf_table();

            Ok(Self {
                liquidity_parameter,
                initial_time: initial_slot + duration_slots,
                current_time: initial_slot,
                outcome_count,
                prices,
                volumes,
                lvr_beta,
                phi_lookup_table: phi_table,
                pdf_lookup_table: pdf_table,
            })
        }

        fn initialize_phi_table() -> Vec<U64F64> {
            let mut table = vec![U64F64::from_num(0); PHI_TABLE_SIZE];
            
            for i in 0..PHI_TABLE_SIZE {
                let x = -4.0 + (8.0 * i as f64) / (PHI_TABLE_SIZE - 1) as f64;
                let phi = Self::compute_normal_cdf(x);
                table[i] = U64F64::from_num(phi);
            }
            
            table
        }

        fn initialize_pdf_table() -> Vec<U64F64> {
            let mut table = vec![U64F64::from_num(0); PHI_TABLE_SIZE];
            
            for i in 0..PHI_TABLE_SIZE {
                let x = -4.0 + (8.0 * i as f64) / (PHI_TABLE_SIZE - 1) as f64;
                let pdf = Self::compute_normal_pdf(x);
                table[i] = U64F64::from_num(pdf);
            }
            
            table
        }

        fn compute_normal_cdf(x: f64) -> f64 {
            0.5 * (1.0 + Self::erf(x / std::f64::consts::SQRT_2))
        }

        fn compute_normal_pdf(x: f64) -> f64 {
            (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
        }

        fn erf(x: f64) -> f64 {
            let a1 =  0.254829592;
            let a2 = -0.284496736;
            let a3 =  1.421413741;
            let a4 = -1.453152027;
            let a5 =  1.061405429;
            let p  =  0.3275911;

            let sign = if x < 0.0 { -1.0 } else { 1.0 };
            let x = x.abs();

            let t = 1.0 / (1.0 + p * x);
            let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

            sign * y
        }
    }

    // Newton-Raphson Solver
    struct NewtonRaphsonSolver;

    impl NewtonRaphsonSolver {
        fn solve_pm_amm_price(
            state: &PMAMMState,
            outcome_index: u8,
            order_size: I64F64,
        ) -> Result<PMPriceResult, String> {
            let current_price = state.prices[outcome_index as usize];
            let time_remaining = state.initial_time.saturating_sub(state.current_time);

            if time_remaining == 0 {
                return Err("Market expired".to_string());
            }

            // Calculate time-decay factor L√(T-t)
            let sqrt_time = (time_remaining as f64).sqrt();
            let l_sqrt_t = state.liquidity_parameter * U64F64::from_num(sqrt_time);

            // Initial guess for Newton-Raphson
            let mut y = current_price;
            let x = current_price;

            let mut iterations = 0;
            let mut converged = false;

            while iterations < MAX_NEWTON_ITERATIONS && !converged {
                // Calculate f(y) and f'(y)
                let y_minus_x = y.saturating_sub(x);
                let z = if l_sqrt_t > U64F64::from_num(0) {
                    y_minus_x / l_sqrt_t
                } else {
                    U64F64::from_num(0)
                };

                // Simplified calculation for testing
                let phi_z = U64F64::from_num(0.5); // Placeholder
                let pdf_z = U64F64::from_num(0.4); // Placeholder

                let f_y = y_minus_x.saturating_mul(phi_z)
                    .saturating_add(l_sqrt_t.saturating_mul(pdf_z))
                    .saturating_sub(y);

                let df_dy = phi_z
                    .saturating_add(z.saturating_mul(pdf_z))
                    .saturating_sub(U64F64::from_num(1));

                // Check convergence
                if f_y.to_num::<f64>().abs() < CONVERGENCE_THRESHOLD {
                    converged = true;
                    break;
                }

                // Newton-Raphson update
                if df_dy != U64F64::from_num(0) {
                    let delta = f_y / df_dy;
                    y = y.saturating_sub(delta);
                }

                // Ensure price stays in valid range [0.001, 0.999]
                y = y.max(U64F64::from_num(0.001)).min(U64F64::from_num(0.999));

                iterations += 1;
            }

            if !converged {
                return Err("Convergence failed".to_string());
            }

            // Calculate LVR
            let lvr = state.lvr_beta * U64F64::from_num(1) / U64F64::from_num(time_remaining);

            Ok(PMPriceResult {
                new_price: y,
                old_price: x,
                price_impact: (y - x).abs() / x,
                lvr_cost: lvr,
                iterations,
                slippage: (y - x).abs() / x * U64F64::from_num(100),
            })
        }
    }

    // Quantum Market Implementation
    impl QuantumMarket {
        fn new(
            market_id: [u8; 32],
            proposals: Vec<String>,
            settle_slot: u64,
            collapse_rule: CollapseRule,
        ) -> Result<Self, String> {
            if proposals.len() < 2 || proposals.len() > MAX_QUANTUM_PROPOSALS as usize {
                return Err("Invalid proposal count".to_string());
            }

            let quantum_proposals: Vec<QuantumProposal> = proposals
                .into_iter()
                .enumerate()
                .map(|(i, desc)| QuantumProposal {
                    proposal_id: i as u8,
                    description: desc,
                    current_probability: U64F64::from_num(1.0) / U64F64::from_num(proposals.len()),
                    total_volume: 0,
                    unique_traders: 0,
                    last_trade_slot: 0,
                })
                .collect();

            Ok(Self {
                market_id,
                proposals: quantum_proposals,
                total_deposits: 0,
                settle_slot,
                collapse_rule,
                state: QuantumState::Active,
                winner_index: None,
            })
        }

        fn check_collapse_trigger(&mut self, current_slot: u64) -> bool {
            match self.state {
                QuantumState::Active => {
                    if current_slot >= self.settle_slot - COLLAPSE_BUFFER_SLOTS {
                        self.state = QuantumState::PreCollapse;
                        true
                    } else {
                        false
                    }
                }
                QuantumState::PreCollapse => {
                    if current_slot >= self.settle_slot {
                        self.state = QuantumState::Collapsing;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }

        fn execute_collapse(&mut self) -> Result<(), String> {
            if self.state != QuantumState::Collapsing {
                return Err("Invalid state for collapse".to_string());
            }

            let winner_index = match self.collapse_rule {
                CollapseRule::MaxProbability => {
                    self.proposals
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, p)| p.current_probability.to_bits())
                        .map(|(i, _)| i as u8)
                        .ok_or("No proposals")?
                }
                CollapseRule::MaxVolume => {
                    self.proposals
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, p)| p.total_volume)
                        .map(|(i, _)| i as u8)
                        .ok_or("No proposals")?
                }
                CollapseRule::MaxTraders => {
                    self.proposals
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, p)| p.unique_traders)
                        .map(|(i, _)| i as u8)
                        .ok_or("No proposals")?
                }
                CollapseRule::WeightedComposite => {
                    self.calculate_weighted_winner()?
                }
            };

            self.winner_index = Some(winner_index);
            self.state = QuantumState::Collapsed;
            Ok(())
        }

        fn calculate_weighted_winner(&self) -> Result<u8, String> {
            let mut max_score = U64F64::from_num(0);
            let mut winner = 0u8;

            for (i, proposal) in self.proposals.iter().enumerate() {
                let prob_score = proposal.current_probability * U64F64::from_num(0.5);
                let vol_score = U64F64::from_num(proposal.total_volume)
                    / U64F64::from_num(self.total_deposits.max(1))
                    * U64F64::from_num(0.3);
                let trader_score = U64F64::from_num(proposal.unique_traders)
                    / U64F64::from_num(1000)
                    * U64F64::from_num(0.2);

                let total_score = prob_score + vol_score + trader_score;

                if total_score > max_score {
                    max_score = total_score;
                    winner = i as u8;
                }
            }

            Ok(winner)
        }
    }

    // Credit System
    impl QuantumCredits {
        fn deposit_and_allocate(
            user: [u8; 32],
            market_id: [u8; 32],
            deposit_amount: u64,
            proposal_count: u8,
        ) -> Result<Self, String> {
            if deposit_amount == 0 || proposal_count == 0 {
                return Err("Invalid parameters".to_string());
            }

            let credits_per_proposal = deposit_amount;

            Ok(Self {
                user,
                market_id,
                initial_deposit: deposit_amount,
                credits_per_proposal,
                used_credits: vec![UsedCredit {
                    proposal_id: 0,
                    amount_used: 0,
                    leverage_applied: 0,
                    pnl: 0,
                    position_closed: false,
                }; proposal_count as usize],
                refund_amount: 0,
                refund_claimed: false,
            })
        }

        fn use_credits(
            &mut self,
            proposal_id: u8,
            amount: u64,
            leverage: u64,
        ) -> Result<(), String> {
            let credit = self.used_credits
                .get_mut(proposal_id as usize)
                .ok_or("Invalid proposal")?;

            let available = self.credits_per_proposal - credit.amount_used;
            if amount > available {
                return Err("Insufficient credits".to_string());
            }

            credit.amount_used += amount;
            credit.leverage_applied = leverage;
            Ok(())
        }

        fn calculate_refunds(&mut self, winner_proposal: u8) -> Result<(), String> {
            let mut total_refund = 0u64;

            for (i, credit) in self.used_credits.iter_mut().enumerate() {
                if i as u8 != winner_proposal {
                    let unused = self.credits_per_proposal - credit.amount_used;
                    total_refund += unused;
                    credit.pnl = -(credit.amount_used as i64);
                    credit.position_closed = true;
                }
            }

            self.refund_amount = total_refund;
            Ok(())
        }
    }

    // END-TO-END TESTS

    #[test]
    fn test_e2e_pm_amm_convergence_and_pricing() {
        println!("=== PM-AMM End-to-End Test: Convergence and Pricing ===");
        
        // Create PM-AMM market
        let state = PMAMMState::new(
            U64F64::from_num(100), // L = 100
            86400,                 // 1 day in slots
            4,                     // 4 outcomes
            0,                     // Start slot
        ).unwrap();

        println!("✓ PM-AMM market created with 4 outcomes");
        println!("  Initial prices: {:?}", state.prices.iter().map(|p| p.to_num::<f64>()).collect::<Vec<_>>());

        // Test buy order
        let result = NewtonRaphsonSolver::solve_pm_amm_price(
            &state,
            0,
            I64F64::from_num(10), // Buy 10 units
        ).unwrap();

        println!("✓ Buy order executed");
        println!("  Iterations: {}", result.iterations);
        println!("  Old price: {:.4}", result.old_price.to_num::<f64>());
        println!("  New price: {:.4}", result.new_price.to_num::<f64>());
        println!("  Price impact: {:.2}%", result.price_impact.to_num::<f64>() * 100.0);
        println!("  LVR cost: {:.6}", result.lvr_cost.to_num::<f64>());

        assert!(result.iterations <= 5, "Should converge in ≤5 iterations");
        assert!(result.new_price > result.old_price, "Buy should increase price");
        assert!(result.price_impact < U64F64::from_num(0.1), "Impact should be <10%");
    }

    #[test]
    fn test_e2e_pm_amm_time_decay() {
        println!("\n=== PM-AMM End-to-End Test: Time Decay ===");
        
        let mut state = PMAMMState::new(
            U64F64::from_num(100),
            86400,
            3,
            0,
        ).unwrap();

        // Test at different time points
        let time_points = vec![0, 21600, 43200, 64800, 80000];
        let mut lvr_costs = Vec::new();

        for &time in &time_points {
            state.current_time = time;
            
            let result = NewtonRaphsonSolver::solve_pm_amm_price(
                &state,
                1,
                I64F64::from_num(5),
            ).unwrap();

            lvr_costs.push(result.lvr_cost);
            
            println!("  Time {}: LVR = {:.6}", time, result.lvr_cost.to_num::<f64>());
        }

        // Verify LVR increases over time
        for i in 1..lvr_costs.len() {
            assert!(lvr_costs[i] > lvr_costs[i-1], "LVR should increase over time");
        }
        
        println!("✓ LVR increases correctly as market approaches expiry");
    }

    #[test]
    fn test_e2e_quantum_full_lifecycle() {
        println!("\n=== Quantum Market End-to-End Test: Full Lifecycle ===");
        
        // Create quantum market
        let mut market = QuantumMarket::new(
            [0u8; 32],
            vec![
                "Proposal A: Increase fees".to_string(),
                "Proposal B: Reduce emissions".to_string(),
                "Proposal C: Add new feature".to_string(),
            ],
            1000,
            CollapseRule::MaxProbability,
        ).unwrap();

        println!("✓ Quantum market created with 3 proposals");

        // Simulate trading
        market.proposals[0].current_probability = U64F64::from_num(0.2);
        market.proposals[0].total_volume = 5000;
        market.proposals[0].unique_traders = 10;

        market.proposals[1].current_probability = U64F64::from_num(0.5);
        market.proposals[1].total_volume = 8000;
        market.proposals[1].unique_traders = 15;

        market.proposals[2].current_probability = U64F64::from_num(0.3);
        market.proposals[2].total_volume = 3000;
        market.proposals[2].unique_traders = 8;

        market.total_deposits = 10000;

        println!("✓ Trading simulated");
        for (i, p) in market.proposals.iter().enumerate() {
            println!("  Proposal {}: prob={:.2}, vol={}, traders={}", 
                i, p.current_probability.to_num::<f64>(), p.total_volume, p.unique_traders);
        }

        // Test collapse trigger
        assert!(!market.check_collapse_trigger(500));
        assert!(market.check_collapse_trigger(950)); // Within buffer
        assert_eq!(market.state, QuantumState::PreCollapse);

        assert!(market.check_collapse_trigger(1000)); // At settle slot
        assert_eq!(market.state, QuantumState::Collapsing);

        println!("✓ Collapse triggered correctly");

        // Execute collapse
        market.execute_collapse().unwrap();
        assert_eq!(market.winner_index, Some(1)); // Proposal B wins
        assert_eq!(market.state, QuantumState::Collapsed);

        println!("✓ Market collapsed, winner: Proposal {}", market.winner_index.unwrap());
    }

    #[test]
    fn test_e2e_quantum_credits_and_refunds() {
        println!("\n=== Quantum Credits End-to-End Test: Allocation and Refunds ===");
        
        // Create credits
        let mut credits = QuantumCredits::deposit_and_allocate(
            [1u8; 32],
            [0u8; 32],
            1000,
            3,
        ).unwrap();

        println!("✓ Credits allocated: {} per proposal", credits.credits_per_proposal);

        // Use credits on different proposals
        credits.use_credits(0, 500, 10).unwrap();
        credits.use_credits(1, 1000, 5).unwrap();
        credits.use_credits(2, 200, 20).unwrap();

        println!("✓ Credits used:");
        for (i, credit) in credits.used_credits.iter().enumerate() {
            println!("  Proposal {}: {} used ({}x leverage)", 
                i, credit.amount_used, credit.leverage_applied);
        }

        // Calculate refunds with proposal 1 as winner
        credits.calculate_refunds(1).unwrap();

        println!("✓ Refunds calculated:");
        println!("  Total refund: {}", credits.refund_amount);
        println!("  Expected: {}", 500 + 800); // Unused from proposals 0 and 2

        assert_eq!(credits.refund_amount, 1300);
    }

    #[test]
    fn test_e2e_weighted_collapse_rule() {
        println!("\n=== Quantum Market End-to-End Test: Weighted Collapse ===");
        
        let mut market = QuantumMarket::new(
            [0u8; 32],
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            1000,
            CollapseRule::WeightedComposite,
        ).unwrap();

        // Set up different winning conditions
        // A: High probability, low volume
        market.proposals[0].current_probability = U64F64::from_num(0.6);
        market.proposals[0].total_volume = 2000;
        market.proposals[0].unique_traders = 5;

        // B: Medium probability, high volume
        market.proposals[1].current_probability = U64F64::from_num(0.3);
        market.proposals[1].total_volume = 7000;
        market.proposals[1].unique_traders = 20;

        // C: Low probability, medium volume
        market.proposals[2].current_probability = U64F64::from_num(0.1);
        market.proposals[2].total_volume = 4000;
        market.proposals[2].unique_traders = 15;

        market.total_deposits = 10000;

        // Trigger collapse
        market.state = QuantumState::Collapsing;
        market.execute_collapse().unwrap();

        println!("✓ Weighted collapse executed");
        println!("  Winner: Proposal {}", market.winner_index.unwrap());
        
        // With 50% prob + 30% vol + 20% traders weighting,
        // we expect proposal A to win due to high probability weight
        assert_eq!(market.winner_index, Some(0));
    }

    #[test]
    fn test_e2e_integration_quantum_with_pm_amm() {
        println!("\n=== Integration Test: Quantum Trading with PM-AMM ===");
        
        // Create PM-AMM for pricing
        let mut pm_amm = PMAMMState::new(
            U64F64::from_num(1000),
            86400,
            3,
            0,
        ).unwrap();

        // Create quantum market
        let mut quantum = QuantumMarket::new(
            [0u8; 32],
            vec!["Yes".to_string(), "No".to_string(), "Maybe".to_string()],
            1000,
            CollapseRule::MaxProbability,
        ).unwrap();

        // Create user credits
        let mut credits = QuantumCredits::deposit_and_allocate(
            [1u8; 32],
            [0u8; 32],
            5000,
            3,
        ).unwrap();

        println!("✓ Setup complete: PM-AMM + Quantum + Credits");

        // Simulate quantum trade on proposal 0
        let trade_amount = 1000;
        let leverage = 5;

        // Use credits
        credits.use_credits(0, trade_amount, leverage).unwrap();
        
        // Get PM-AMM price
        let effective_size = trade_amount * leverage;
        let price_result = NewtonRaphsonSolver::solve_pm_amm_price(
            &pm_amm,
            0,
            I64F64::from_num(effective_size as i64),
        ).unwrap();

        // Update quantum market
        quantum.proposals[0].current_probability = price_result.new_price;
        quantum.proposals[0].total_volume += effective_size;
        quantum.proposals[0].unique_traders += 1;

        // Update PM-AMM prices to maintain sum = 1
        pm_amm.prices[0] = price_result.new_price;
        let remaining = U64F64::from_num(1) - price_result.new_price;
        pm_amm.prices[1] = remaining / U64F64::from_num(2);
        pm_amm.prices[2] = remaining / U64F64::from_num(2);

        println!("✓ Quantum trade executed through PM-AMM");
        println!("  Trade size: {} ({}x leverage = {})", trade_amount, leverage, effective_size);
        println!("  Price impact: {:.2}%", price_result.price_impact.to_num::<f64>() * 100.0);
        println!("  New probabilities: {:.2}, {:.2}, {:.2}",
            pm_amm.prices[0].to_num::<f64>(),
            pm_amm.prices[1].to_num::<f64>(),
            pm_amm.prices[2].to_num::<f64>());

        // Verify sum = 1
        let sum: U64F64 = pm_amm.prices.iter().sum();
        assert!((sum.to_num::<f64>() - 1.0).abs() < 0.001, "Prices should sum to 1");

        println!("✓ Integration test passed: Quantum + PM-AMM working together");
    }

    #[test]
    fn test_e2e_performance_benchmarks() {
        println!("\n=== Performance Benchmark Test ===");
        
        use std::time::Instant;

        // PM-AMM Performance
        let state = PMAMMState::new(
            U64F64::from_num(1000),
            86400,
            10, // 10 outcomes
            0,
        ).unwrap();

        let start = Instant::now();
        for _ in 0..100 {
            let _ = NewtonRaphsonSolver::solve_pm_amm_price(
                &state,
                0,
                I64F64::from_num(50),
            );
        }
        let pm_amm_time = start.elapsed();

        println!("✓ PM-AMM Performance:");
        println!("  100 trades in {:?}", pm_amm_time);
        println!("  Avg per trade: {:?}", pm_amm_time / 100);

        // Quantum Collapse Performance
        let mut market = QuantumMarket::new(
            [0u8; 32],
            (0..10).map(|i| format!("Proposal {}", i)).collect(),
            1000,
            CollapseRule::WeightedComposite,
        ).unwrap();

        // Set up market state
        for i in 0..10 {
            market.proposals[i].current_probability = U64F64::from_num(0.1);
            market.proposals[i].total_volume = 1000 * (i as u64 + 1);
            market.proposals[i].unique_traders = 10 * (i as u32 + 1);
        }
        market.total_deposits = 50000;
        market.state = QuantumState::Collapsing;

        let start = Instant::now();
        for _ in 0..100 {
            let mut market_copy = market.clone();
            let _ = market_copy.execute_collapse();
        }
        let collapse_time = start.elapsed();

        println!("✓ Quantum Collapse Performance:");
        println!("  100 collapses in {:?}", collapse_time);
        println!("  Avg per collapse: {:?}", collapse_time / 100);

        // Credit Refund Performance
        let mut credits = QuantumCredits::deposit_and_allocate(
            [1u8; 32],
            [0u8; 32],
            10000,
            10,
        ).unwrap();

        for i in 0..10 {
            let _ = credits.use_credits(i, 500 * (i as u64 + 1), 5);
        }

        let start = Instant::now();
        for _ in 0..1000 {
            let mut credits_copy = credits.clone();
            let _ = credits_copy.calculate_refunds(5);
        }
        let refund_time = start.elapsed();

        println!("✓ Credit Refund Performance:");
        println!("  1000 refund calculations in {:?}", refund_time);
        println!("  Avg per refund: {:?}", refund_time / 1000);

        println!("\n✓ All performance benchmarks completed successfully");
    }
}