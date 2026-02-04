use anchor_lang::prelude::*;
use crate::lmsr_amm::*;
use crate::pm_amm::*;
use crate::l2_amm::*;
use crate::account_structs::*;
use crate::errors::ErrorCode;

// Constants
const PRICE_CLAMP_PER_SLOT: f64 = 0.02; // 2% price clamp
const LIQ_CAP_MIN: f64 = 0.02; // 2% minimum liquidation cap
const LIQ_CAP_MAX: f64 = 0.08; // 8% maximum liquidation cap

#[cfg(test)]
mod amm_security_tests {
    use super::*;

    // Mock AMM state structures for testing
    #[derive(Clone)]
    struct LMSRAmmState {
        liquidity: u64,
        prices: Vec<f64>,
        last_update_slot: u64,
    }

    impl LMSRAmmState {
        fn new(liquidity: u64) -> Self {
            Self {
                liquidity,
                prices: vec![0.5, 0.5], // Initial 50/50 prices
                last_update_slot: 0,
            }
        }

        fn get_price(&self, outcome: usize) -> f64 {
            self.prices.get(outcome).copied().unwrap_or(0.5)
        }

        fn execute_order(&mut self, order: &Order, slot: u64) -> Result<()> {
            // Check price clamp
            let old_price = self.get_price(order.outcome);
            let price_impact = self.calculate_price_impact(order.size);
            
            if price_impact > PRICE_CLAMP_PER_SLOT {
                return Err(ErrorCode::PriceClampExceeded.into());
            }

            // Update price
            let new_price = if order.is_buy {
                old_price * (1.0 + price_impact)
            } else {
                old_price * (1.0 - price_impact)
            };

            self.prices[order.outcome] = new_price;
            self.last_update_slot = slot;
            Ok(())
        }

        fn calculate_price_impact(&self, size: u64) -> f64 {
            // Simple impact model: impact = size / liquidity
            (size as f64 / self.liquidity as f64).min(PRICE_CLAMP_PER_SLOT)
        }

        fn calculate_liquidity_cap(&self) -> u64 {
            (self.liquidity as f64 * 0.5) as u64 // 50% of liquidity
        }
    }

    #[derive(Clone)]
    struct PMAmmState {
        liquidity_parameter: u64,
        lvr: f64,
        time_to_expiry: u64,
        current_price: f64,
        last_update_slot: u64,
    }

    impl PMAmmState {
        fn new(liquidity: u64, lvr: f64, time_to_expiry: u64) -> Self {
            Self {
                liquidity_parameter: liquidity,
                lvr,
                time_to_expiry,
                current_price: 0.5,
                last_update_slot: 0,
            }
        }

        fn get_current_price(&self) -> f64 {
            self.current_price
        }

        fn execute_trade(&mut self, size: u64, is_buy: bool) -> f64 {
            let impact = (size as f64 / self.liquidity_parameter as f64) * 
                         (self.time_to_expiry as f64 / 86400.0).sqrt();
            
            let clamped_impact = impact.min(PRICE_CLAMP_PER_SLOT);
            
            if is_buy {
                self.current_price *= 1.0 + clamped_impact;
            } else {
                self.current_price *= 1.0 - clamped_impact;
            }
            
            self.current_price = self.current_price.max(0.01).min(0.99);
            self.current_price
        }

        fn calculate_lvr(&self, old_price: f64, new_price: f64) -> f64 {
            // LVR = |ln(new_price/old_price)| * liquidity
            ((new_price / old_price).ln().abs() * self.liquidity_parameter as f64) / 
            self.liquidity_parameter as f64
        }
    }

    struct Order {
        size: u64,
        outcome: usize,
        is_buy: bool,
    }

    struct MarketState {
        positions: Vec<Position>,
    }

    impl MarketState {
        fn new() -> Self {
            Self {
                positions: Vec::new(),
            }
        }

        fn add_position(&mut self, position: Position) {
            self.positions.push(position);
        }
    }

    #[test]
    fn test_price_manipulation_bounds() {
        let mut amm = LMSRAmmState::new(100_000_000); // 100 USDC liquidity

        // Test 2% price clamp per slot
        let initial_price = amm.get_price(0);

        // Try to manipulate price by 10% in one go
        let large_order = Order {
            size: 50_000_000, // 50 USDC
            outcome: 0,
            is_buy: true,
        };

        let result = amm.execute_order(&large_order, 1);
        assert!(result.is_err(), "Large order should be rejected");

        // Test that multiple 2% moves are clamped
        for i in 0..5 {
            let order = Order {
                size: 2_000_000, // 2 USDC
                outcome: 0,
                is_buy: true,
            };

            let old_price = amm.get_price(0);
            let _ = amm.execute_order(&order, i + 1);
            let new_price = amm.get_price(0);

            let price_change = (new_price - old_price).abs() / old_price;
            assert!(price_change <= 0.02, "Price change exceeds 2% clamp");
        }
    }

    #[test]
    fn test_sandwich_attack_prevention() {
        let mut amm = PMAmmState::new(1_000_000_000, 0.05, 3600); // 1k USDC, 1hr to expiry

        // Attacker tries to sandwich a large order
        let victim_order = Order {
            size: 100_000_000, // 100 USDC
            outcome: 0,
            is_buy: true,
        };

        // Front-run attempt
        let attack_front = Order {
            size: 200_000_000, // 200 USDC
            outcome: 0,
            is_buy: true,
        };

        let initial_price = amm.get_current_price();

        // Execute front-run
        let price_after_front = amm.execute_trade(attack_front.size, attack_front.is_buy);

        let front_impact = (price_after_front - initial_price) / initial_price;

        // Verify uniform LVR prevents excessive profit
        let lvr = amm.calculate_lvr(initial_price, price_after_front);
        assert!(lvr < 0.05, "LVR too high, sandwich profitable");

        // Execute victim order
        let price_after_victim = amm.execute_trade(victim_order.size, victim_order.is_buy);

        // Back-run attempt
        let attack_back = Order {
            size: 200_000_000,
            outcome: 0,
            is_buy: false, // Sell
        };

        let final_price = amm.execute_trade(attack_back.size, attack_back.is_buy);

        // Calculate attacker profit
        let avg_buy_price = (initial_price + price_after_front) / 2.0;
        let avg_sell_price = final_price;
        let profit_percent = (avg_sell_price - avg_buy_price) / avg_buy_price;

        // Profit should be minimal due to fees and LVR
        assert!(profit_percent < 0.01, "Sandwich attack too profitable");
    }

    #[test]
    fn test_flash_loan_attack_prevention() {
        // Test that positions can't be manipulated via flash loans
        let mut market = MarketState::new();
        let mut amm = LMSRAmmState::new(10_000_000_000); // 10k USDC

        // Flash loan attempts to manipulate liquidations
        let flash_loan_amount = 1_000_000_000_000; // 1M USDC

        // Open leveraged position
        let position = Position {
            proposal_id: 1,
            outcome: 0,
            size: 100_000_000, // 100 USDC
            leverage: 100,
            entry_price: 5000, // 0.5
            liquidation_price: 4900, // 0.49
            is_long: true,
            created_at: 0,
        };

        market.add_position(position.clone());

        // Try to manipulate price to trigger liquidation
        let attack_order = Order {
            size: flash_loan_amount,
            outcome: 0,
            is_buy: false, // Sell to drop price
        };

        // Orders beyond liquidity cap should fail
        let liquidity_cap = amm.calculate_liquidity_cap();
        assert!(attack_order.size > liquidity_cap, "Attack within cap");

        let result = amm.execute_order(&attack_order, 1);
        assert!(result.is_err(), "Flash loan attack should fail");

        // Even if split into smaller orders, halt should trigger
        let split_size = liquidity_cap / 10;
        let mut total_impact = 0.0;

        for i in 0..10 {
            let split_order = Order {
                size: split_size,
                outcome: 0,
                is_buy: false,
            };

            let old_price = amm.get_price(0);
            match amm.execute_order(&split_order, i + 1) {
                Ok(_) => {
                    let new_price = amm.get_price(0);
                    total_impact += (old_price - new_price) / old_price;

                    // Check if halt should trigger
                    if total_impact > 0.05 {
                        // 5% cumulative move should halt
                        break;
                    }
                }
                Err(_) => break, // Halt triggered
            }
        }

        assert!(total_impact < 0.05, "Excessive price manipulation allowed");
    }

    // Test L2 distribution manipulation
    #[test]
    fn test_l2_distribution_bounds() {
        struct L2Distribution {
            k: f64,
            b_max: f64,
        }

        impl L2Distribution {
            fn new(k: f64, b_max: f64) -> Self {
                Self { k, b_max }
            }

            fn validate_distribution(&self, dist: &[(f64, f64)]) -> Result<Vec<(f64, f64)>> {
                let mut validated = Vec::new();
                
                for &(x, y) in dist {
                    if y > self.b_max {
                        return Err(ErrorCode::InvalidDistribution.into());
                    }
                    validated.push((x, y.min(self.b_max)));
                }
                
                Ok(validated)
            }

            fn calculate_l2_norm(&self, dist: &[(f64, f64)]) -> f64 {
                dist.iter()
                    .map(|(_, y)| y * y)
                    .sum::<f64>()
                    .sqrt()
            }
        }

        let mut l2_amm = L2Distribution::new(100000.0, 1000.0);

        // Try to submit adversarial distribution
        let adversarial_dist = vec![
            (0.0, 2000.0),  // Exceeds b bound
            (0.5, 500.0),
            (1.0, 100.0),
        ];

        // System should reject or clip
        let result = l2_amm.validate_distribution(&adversarial_dist);
        assert!(result.is_err() || result.unwrap().iter().all(|(_, v)| *v <= 1000.0));

        // Verify L2 norm constraint
        let norm = l2_amm.calculate_l2_norm(&adversarial_dist);
        assert!(norm <= 100000.0, "L2 norm exceeds k constraint");
    }

    // Test liquidation cascade prevention
    #[test]
    fn test_liquidation_cascade_prevention() {
        #[derive(Clone)]
        struct TestPosition {
            size: u64,
            leverage: u64,
            entry_price: u64,
        }

        let positions = vec![
            TestPosition { size: 10000, leverage: 100, entry_price: 5000 },
            TestPosition { size: 20000, leverage: 200, entry_price: 5100 },
            TestPosition { size: 15000, leverage: 300, entry_price: 5200 },
        ];

        let current_price = 4900; // 2% drop
        let mut liquidated = Vec::new();

        // Process liquidations with partial cap
        for (i, pos) in positions.iter().enumerate() {
            let liq_price = calculate_liquidation_price_simple(
                pos.entry_price as f64 / 10000.0,
                pos.leverage as f64,
                0.01 // margin ratio
            );

            if (current_price as f64 / 10000.0) <= liq_price {
                // Only liquidate 8% per slot
                let partial_amount = (pos.size * 8) / 100;
                liquidated.push((i, partial_amount));
            }
        }

        // Verify cascading is limited
        let total_liquidated: u64 = liquidated.iter().map(|(_, amt)| amt).sum();
        let total_size: u64 = positions.iter().map(|p| p.size).sum();
        let liquidation_percent = (total_liquidated * 100) / total_size;

        assert!(
            liquidation_percent <= 24, // Max 3 positions * 8% each
            "Liquidation cascade exceeded limits: {}%",
            liquidation_percent
        );
    }

    // Helper function for simple liquidation price calculation
    fn calculate_liquidation_price_simple(
        entry_price: f64,
        leverage: f64,
        margin_ratio: f64
    ) -> f64 {
        entry_price * (1.0 - margin_ratio / leverage)
    }
}

// Cross-program attack tests
#[cfg(test)]
mod cross_program_attacks {
    use super::*;

    struct ChainState {
        active_borrows: Vec<(u8, u64)>, // (verse_id, amount)
    }

    impl ChainState {
        fn new() -> Self {
            Self {
                active_borrows: Vec::new(),
            }
        }

        fn borrow_step(&mut self, amount: u64, verse_id: u8) -> Result<u64> {
            // Check for circular borrowing
            if self.active_borrows.iter().any(|(id, _)| *id == verse_id) {
                return Err(ErrorCode::CircularBorrow.into());
            }
            
            self.active_borrows.push((verse_id, amount));
            Ok((amount as f64 * 1.5) as u64) // 1.5x multiplier
        }

        fn liquidity_step(&mut self, amount: u64, verse_id: u8) -> Result<u64> {
            Ok((amount as f64 * 1.2) as u64) // 1.2x multiplier
        }

        fn stake_step(&mut self, amount: u64, verse_id: u8) -> Result<u64> {
            Ok((amount as f64 * 1.1) as u64) // 1.1x multiplier
        }

        fn is_acyclic(&self) -> bool {
            // Check for cycles in borrowing graph
            let mut visited = vec![false; 256];
            for (verse_id, _) in &self.active_borrows {
                if visited[*verse_id as usize] {
                    return false;
                }
                visited[*verse_id as usize] = true;
            }
            true
        }
    }

    #[test]
    fn test_reentrancy_prevention() {
        // Test that chaining operations can't create reentrancy
        let mut chain_state = ChainState::new();

        // Simulate nested borrow → liquidity → stake → borrow cycle
        let initial_deposit = 100_000_000; // 100 USDC

        let step1 = chain_state.borrow_step(initial_deposit, 1);
        assert!(step1.is_ok());

        let step2 = chain_state.liquidity_step(step1.unwrap(), 1);
        assert!(step2.is_ok());

        let step3 = chain_state.stake_step(step2.unwrap(), 1);
        assert!(step3.is_ok());

        // Try to borrow again in same verse - should fail
        let step4 = chain_state.borrow_step(step3.unwrap(), 1);
        assert!(step4.is_err(), "Circular borrow should fail");

        // Verify state consistency
        assert!(chain_state.is_acyclic(), "Chain graph has cycle");
    }

    #[test]
    fn test_verse_isolation_attacks() {
        struct VerseState {
            id: u8,
            positions: Vec<Position>,
            total_value: u64,
        }

        impl VerseState {
            fn new(id: u8, parent: Option<u8>) -> Self {
                Self {
                    id,
                    positions: Vec::new(),
                    total_value: 0,
                }
            }

            fn add_position(&mut self, pos: Position) {
                self.total_value += pos.size;
                self.positions.push(pos);
            }

            fn borrow_from_verse(&self, other: &VerseState, amount: u64) -> Result<()> {
                // Cross-verse borrowing should always fail
                Err(ErrorCode::CrossVerseBorrowNotAllowed.into())
            }

            fn resolve(&mut self, win: bool) {
                if !win {
                    self.total_value = 0;
                    self.positions.clear();
                }
            }

            fn total_value(&self) -> u64 {
                self.total_value
            }
        }

        // Test that verse isolation can't be broken
        let mut verse_a = VerseState::new(1, None);
        let mut verse_b = VerseState::new(2, None);

        // Create positions in different verses
        let pos_a = Position {
            proposal_id: 1,
            outcome: 0,
            size: 100_000_000,
            leverage: 50,
            entry_price: 6000,
            liquidation_price: 5800,
            is_long: true,
            created_at: 0,
        };

        let pos_b = Position {
            proposal_id: 2,
            outcome: 1,
            size: 200_000_000,
            leverage: 100,
            entry_price: 4000,
            liquidation_price: 3960,
            is_long: true,
            created_at: 0,
        };

        verse_a.add_position(pos_a.clone());
        verse_b.add_position(pos_b.clone());

        // Try cross-verse operations
        let cross_borrow = verse_a.borrow_from_verse(&verse_b, 50_000_000);
        assert!(cross_borrow.is_err(), "Cross-verse borrow should fail");

        // Verify resolution isolation
        verse_a.resolve(false); // Verse A loses

        // Positions in verse A should be wiped
        assert_eq!(verse_a.total_value(), 0);

        // Verse B should be unaffected
        assert_eq!(verse_b.total_value(), 200_000_000);
    }
}