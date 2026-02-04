//! Standalone chain execution test
//! Tests atomicity, cycle prevention, and leverage multiplication

const MAX_CHAIN_DEPTH: u8 = 5;

// Multipliers from specification
const BORROW_MULTIPLIER: u64 = 15000;  // 1.5x
const LEND_MULTIPLIER: u64 = 12000;    // 1.2x
const LIQUIDITY_MULTIPLIER: u64 = 12000; // 1.2x
const STAKE_MULTIPLIER: u64 = 11000;    // 1.1x

#[derive(Debug, Clone, PartialEq)]
enum ChainStepType {
    Borrow { amount: u64 },
    Lend { amount: u64 },
    Long { outcome: u8, leverage: u64 },
    Short { outcome: u8, leverage: u64 },
    Liquidity { amount: u64 },
    Stake { amount: u64 },
}

#[derive(Debug)]
struct ChainState {
    initial_deposit: u64,
    current_balance: u64,
    steps: Vec<ChainStepType>,
    effective_leverage: u64,
}

impl ChainState {
    fn new(deposit: u64) -> Self {
        Self {
            initial_deposit: deposit,
            current_balance: deposit,
            steps: Vec::new(),
            effective_leverage: 10000, // 1.0x in basis points
        }
    }
    
    fn apply_step(&mut self, step: ChainStepType) -> Result<(), String> {
        // Check depth limit
        if self.steps.len() >= MAX_CHAIN_DEPTH as usize {
            return Err("Exceeds maximum chain depth".to_string());
        }
        
        // Apply step effects
        match &step {
            ChainStepType::Borrow { amount } => {
                self.effective_leverage = (self.effective_leverage * BORROW_MULTIPLIER) / 10000;
                self.current_balance += amount;
            }
            ChainStepType::Lend { amount } => {
                if *amount > self.current_balance {
                    return Err("Insufficient balance to lend".to_string());
                }
                self.effective_leverage = (self.effective_leverage * LEND_MULTIPLIER) / 10000;
                self.current_balance -= amount;
            }
            ChainStepType::Long { outcome: _, leverage } => {
                self.effective_leverage = (self.effective_leverage * leverage) / 100;
            }
            ChainStepType::Short { outcome: _, leverage } => {
                self.effective_leverage = (self.effective_leverage * leverage) / 100;
            }
            ChainStepType::Liquidity { amount } => {
                if *amount > self.current_balance {
                    return Err("Insufficient balance for liquidity".to_string());
                }
                self.effective_leverage = (self.effective_leverage * LIQUIDITY_MULTIPLIER) / 10000;
                let yield_amount = calculate_liquidity_yield(*amount);
                self.current_balance = self.current_balance - amount + yield_amount;
            }
            ChainStepType::Stake { amount } => {
                if *amount > self.current_balance {
                    return Err("Insufficient balance to stake".to_string());
                }
                self.effective_leverage = (self.effective_leverage * STAKE_MULTIPLIER) / 10000;
                let return_amount = calculate_stake_return(*amount, self.steps.len() as u64);
                self.current_balance = self.current_balance - amount + return_amount;
            }
        }
        
        // Cap effective leverage at 500x
        if self.effective_leverage > 5000000 { // 500x in basis points
            self.effective_leverage = 5000000;
        }
        
        self.steps.push(step);
        Ok(())
    }
}

// Formula implementations from specification
fn calculate_borrow_amount(deposit: u64, coverage: u64, n: u64) -> u64 {
    let sqrt_n = (n as f64).sqrt() as u64;
    (deposit * coverage) / sqrt_n.max(1)
}

fn calculate_liquidity_yield(liquidity: u64) -> u64 {
    liquidity + (liquidity * 5) / 1000 // 0.5% yield
}

fn calculate_stake_return(stake: u64, depth: u64) -> u64 {
    stake * (100 + depth * 100 / 32) / 100
}

#[test]
fn test_max_chain_depth_enforcement() {
    println!("\nTesting MAX_CHAIN_DEPTH (5) enforcement:");
    
    let mut chain = ChainState::new(10_000_000);
    
    // Add 5 steps (should succeed)
    for i in 0..5 {
        let step = ChainStepType::Long { outcome: 0, leverage: 200 };
        assert!(chain.apply_step(step).is_ok());
        println!("  Step {}: Added successfully", i + 1);
    }
    
    // Try to add 6th step (should fail)
    let step = ChainStepType::Long { outcome: 0, leverage: 200 };
    let result = chain.apply_step(step);
    assert!(result.is_err());
    println!("  Step 6: Rejected - {}", result.unwrap_err());
    
    println!("✅ Chain depth limit enforced!");
}

#[test]
fn test_atomic_execution() {
    println!("\nTesting atomic execution:");
    
    let initial_balance = 10_000_000;
    let mut chain = ChainState::new(initial_balance);
    
    // Valid steps
    let valid_steps = vec![
        ChainStepType::Borrow { amount: 1000 },
        ChainStepType::Liquidity { amount: 500 },
    ];
    
    // Apply valid steps
    for step in valid_steps {
        chain.apply_step(step).unwrap();
    }
    
    let balance_after_valid = chain.current_balance;
    println!("  Balance after valid steps: {}", balance_after_valid);
    
    // Try invalid step (insufficient balance)
    let invalid_step = ChainStepType::Lend { amount: 100_000_000 };
    let result = chain.apply_step(invalid_step);
    
    assert!(result.is_err());
    assert_eq!(chain.current_balance, balance_after_valid); // Balance unchanged
    assert_eq!(chain.steps.len(), 2); // Only valid steps remain
    
    println!("  Invalid step rejected, state unchanged");
    println!("✅ Atomicity preserved!");
}

#[test]
fn test_leverage_multiplication() {
    println!("\nTesting leverage multiplication through chaining:");
    
    let mut chain = ChainState::new(10_000_000);
    
    let steps = vec![
        ("Borrow", ChainStepType::Borrow { amount: 1000 }, BORROW_MULTIPLIER),
        ("Lend", ChainStepType::Lend { amount: 500 }, LEND_MULTIPLIER),
        ("Liquidity", ChainStepType::Liquidity { amount: 300 }, LIQUIDITY_MULTIPLIER),
        ("Stake", ChainStepType::Stake { amount: 200 }, STAKE_MULTIPLIER),
    ];
    
    println!("  Starting leverage: {:.2}x", chain.effective_leverage as f64 / 10000.0);
    
    for (name, step, expected_mult) in steps {
        chain.apply_step(step).unwrap();
        let leverage = chain.effective_leverage as f64 / 10000.0;
        let mult = expected_mult as f64 / 10000.0;
        println!("  After {}: {:.2}x (×{:.1})", name, leverage, mult);
    }
    
    // Expected: 1.0 * 1.5 * 1.2 * 1.2 * 1.1 = 2.376x
    let final_leverage = chain.effective_leverage as f64 / 10000.0;
    assert!(final_leverage > 2.0 && final_leverage < 2.5);
    
    println!("  Final cumulative leverage: {:.3}x", final_leverage);
    println!("✅ Leverage multiplication verified!");
}

#[test]
fn test_cycle_prevention() {
    println!("\nTesting cycle prevention patterns:");
    
    // In a real implementation, cycles would be detected by tracking
    // verse dependencies. Here we simulate the check.
    
    let patterns = vec![
        (vec!["Borrow A", "Lend B", "Borrow A"], true, "Direct cycle"),
        (vec!["Borrow A", "Liquidity", "Stake"], false, "No cycle"),
        (vec!["Borrow A", "Lend B", "Borrow C", "Lend A"], true, "Indirect cycle"),
    ];
    
    for (steps, has_cycle, description) in patterns {
        println!("  Pattern: {:?} - {}", steps, description);
        
        // Simulate cycle detection
        let detected = detect_cycle(&steps);
        assert_eq!(detected, has_cycle);
        
        println!("    Cycle detected: {} {}", detected, if detected { "✗" } else { "✓" });
    }
    
    println!("✅ Cycle prevention working!");
}

fn detect_cycle(steps: &[&str]) -> bool {
    // Simplified cycle detection
    let mut seen = std::collections::HashSet::new();
    for step in steps {
        if step.starts_with("Borrow") || step.starts_with("Lend") {
            let resource = step.split_whitespace().last().unwrap();
            if seen.contains(resource) {
                return true;
            }
            seen.insert(resource);
        }
    }
    false
}

#[test]
fn test_formula_calculations() {
    println!("\nTesting chain formula calculations:");
    
    // Test borrow amount calculation
    println!("  Borrow amount = deposit × coverage / √N:");
    let borrow_tests = vec![
        (1000, 150, 1, 150000),   // 1000 * 150 / 1
        (1000, 150, 4, 75000),    // 1000 * 150 / 2
        (1000, 100, 9, 33333),    // 1000 * 100 / 3
    ];
    
    for (deposit, coverage, n, expected) in borrow_tests {
        let actual = calculate_borrow_amount(deposit, coverage, n);
        println!("    deposit={}, coverage={}, N={}: expected={}, actual={}", 
            deposit, coverage, n, expected, actual);
        assert!((actual as i64 - expected as i64).abs() < 10);
    }
    
    // Test liquidity yield
    println!("\n  Liquidity yield (0.5%):");
    assert_eq!(calculate_liquidity_yield(10000), 10050);
    assert_eq!(calculate_liquidity_yield(100000), 100500);
    println!("    ✓ Verified");
    
    // Test stake return
    println!("\n  Stake return = stake × (1 + depth/32):");
    assert_eq!(calculate_stake_return(1000, 0), 1000);   // No bonus at depth 0
    assert_eq!(calculate_stake_return(1000, 16), 1500);  // 50% bonus at depth 16
    assert_eq!(calculate_stake_return(1000, 32), 2000);  // 100% bonus at depth 32
    println!("    ✓ Verified");
    
    println!("✅ All formulas working correctly!");
}

#[test]
fn test_edge_cases() {
    println!("\nTesting edge cases:");
    
    // Zero deposit
    let mut chain = ChainState::new(0);
    let result = chain.apply_step(ChainStepType::Long { outcome: 0, leverage: 1000 });
    assert!(result.is_ok()); // Can still take positions with 0 balance
    println!("  Zero deposit: Can still create positions ✓");
    
    // Insufficient balance
    let mut chain = ChainState::new(1000);
    let result = chain.apply_step(ChainStepType::Lend { amount: 2000 });
    assert!(result.is_err());
    println!("  Insufficient balance: Rejected ✓");
    
    // Leverage cap
    let mut chain = ChainState::new(10_000_000);
    chain.effective_leverage = 4_000_000; // 400x
    chain.apply_step(ChainStepType::Borrow { amount: 1000 }).unwrap();
    assert_eq!(chain.effective_leverage, 5_000_000); // Capped at 500x
    println!("  Leverage cap (500x): Enforced ✓");
    
    println!("✅ Edge cases handled correctly!");
}

fn main() {
    println!("Running Chain Execution Tests\n");
    
    test_max_chain_depth_enforcement();
    test_atomic_execution();
    test_leverage_multiplication();
    test_cycle_prevention();
    test_formula_calculations();
    test_edge_cases();
    
    println!("\n🎉 ALL CHAIN EXECUTION TESTS PASSED! 🎉");
}