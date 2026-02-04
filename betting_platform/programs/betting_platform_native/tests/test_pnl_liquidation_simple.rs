//! Simple PnL liquidation test that can run independently
//! This test verifies the core PnL-based liquidation logic

fn test_pnl_liquidation_basic() {
    println!("\n=== Testing Basic PnL Liquidation Logic ===");
    
    // Test the effective leverage formula
    let test_effective_leverage = |base_lev: u64, pnl_pct: i64| -> u64 {
        // effective_leverage = position_leverage × (1 - unrealized_pnl_pct)
        // where unrealized_pnl_pct is in basis points (10000 = 100%)
        
        // Calculate (1 - unrealized_pnl_pct) in basis points
        let adjustment_factor = 10000i64 - pnl_pct;
        
        // Ensure adjustment factor doesn't go below 10% (minimum 0.1x multiplier)
        let safe_adjustment = adjustment_factor.max(1000);
        
        // Calculate effective leverage
        let effective = (base_lev as i64 * safe_adjustment) / 10000;
        
        // Ensure minimum leverage of 1x and maximum of 500x
        effective.max(1).min(500) as u64
    };
    
    // Test cases
    assert_eq!(test_effective_leverage(10, 0), 10);
    println!("✓ No PnL: 10x → 10x");
    
    assert_eq!(test_effective_leverage(10, 2000), 8);
    println!("✓ 20% profit: 10x → 8x");
    
    assert_eq!(test_effective_leverage(10, -1000), 11);
    println!("✓ 10% loss: 10x → 11x");
    
    assert_eq!(test_effective_leverage(10, 9000), 1);
    println!("✓ 90% profit: 10x → 1x (minimum)");
    
    assert_eq!(test_effective_leverage(10, -5000), 15);
    println!("✓ 50% loss: 10x → 15x");
    
    // Test liquidation price calculation
    let test_liquidation_price = |entry_price: u64, effective_leverage: u64, is_long: bool| -> u64 {
        if is_long {
            // Long positions: liquidate when price drops
            entry_price * (effective_leverage - 1) / effective_leverage
        } else {
            // Short positions: liquidate when price rises
            entry_price * (effective_leverage + 1) / effective_leverage
        }
    };
    
    println!("\n=== Testing Liquidation Price Calculations ===");
    
    // Long position tests
    let liq_price = test_liquidation_price(100_000_000, 10, true);
    assert_eq!(liq_price, 90_000_000);
    println!("✓ 10x long at $100: liquidates at $90");
    
    let liq_price = test_liquidation_price(100_000_000, 8, true);
    assert_eq!(liq_price, 87_500_000);
    println!("✓ 8x long at $100: liquidates at $87.50");
    
    // Short position tests
    let liq_price = test_liquidation_price(100_000_000, 10, false);
    assert_eq!(liq_price, 110_000_000);
    println!("✓ 10x short at $100: liquidates at $110");
    
    // Test PnL calculation
    let test_pnl = |entry_price: u64, current_price: u64, size: u64, is_long: bool| -> (i64, i64) {
        let price_diff = if is_long {
            // Long: profit when price goes up
            current_price as i64 - entry_price as i64
        } else {
            // Short: profit when price goes down
            entry_price as i64 - current_price as i64
        };
        
        // PnL = price_diff * size / entry_price
        let pnl = (price_diff * size as i64) / entry_price as i64;
        
        // PnL percentage in basis points
        let pnl_pct = (price_diff * 10000) / entry_price as i64;
        
        (pnl, pnl_pct)
    };
    
    println!("\n=== Testing PnL Calculations ===");
    
    let (pnl, pnl_pct) = test_pnl(100_000_000, 120_000_000, 1_000_000_000, true);
    assert_eq!(pnl, 200_000_000);
    assert_eq!(pnl_pct, 2000);
    println!("✓ Long $1000 from $100 to $120: PnL = $200 (20%)");
    
    let (pnl, pnl_pct) = test_pnl(100_000_000, 80_000_000, 1_000_000_000, false);
    assert_eq!(pnl, 200_000_000);
    assert_eq!(pnl_pct, 2000);
    println!("✓ Short $1000 from $100 to $80: PnL = $200 (20%)");
    
    // Test complete scenario
    println!("\n=== Testing Complete Scenario ===");
    
    let entry_price = 100_000_000; // $100
    let initial_leverage = 20;
    let size = 1_000_000_000; // $1000
    let is_long = true;
    
    // Initial liquidation price
    let initial_liq = test_liquidation_price(entry_price, initial_leverage, is_long);
    println!("Initial position: 20x long at $100, liquidates at ${}", initial_liq / 1_000_000);
    
    // Price moves to $110 (10% profit)
    let current_price = 110_000_000;
    let (pnl, pnl_pct) = test_pnl(entry_price, current_price, size, is_long);
    let effective_lev = test_effective_leverage(initial_leverage, pnl_pct);
    let new_liq = test_liquidation_price(entry_price, effective_lev, is_long);
    
    println!("\nAfter price moves to $110:");
    println!("  PnL: ${} ({}%)", pnl / 1_000_000, pnl_pct / 100);
    println!("  Effective leverage: {}x (was {}x)", effective_lev, initial_leverage);
    println!("  New liquidation price: ${} (was ${})", new_liq / 1_000_000, initial_liq / 1_000_000);
    
    assert!(new_liq < initial_liq);
    println!("  ✓ Position is safer (liquidation price decreased)");
    
    println!("\n✅ All basic PnL liquidation tests passed!");
}

fn main() {
    println!("Running simple PnL liquidation tests...");
    test_pnl_liquidation_basic();
}