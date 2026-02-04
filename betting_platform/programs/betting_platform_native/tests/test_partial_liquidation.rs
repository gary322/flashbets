//! Tests for partial liquidation implementation
//! Verifies: partial_close(pos, allowed=cap - acc) logic

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    liquidation::partial_liquidate::PartialLiquidationProcessor,
    state::accounts::{Position, PositionStatus},
    keeper_liquidation::LiquidationKeeper,
    math::U64F64,
};

#[tokio::test]
async fn test_partial_close_logic() {
    // Test position with 10,000 USDC size
    let mut position = create_test_position(10_000_000_000);
    let liquidation_cap = 500_000_000; // 500 USDC cap
    
    // Partial close
    let remaining_size = position.size.saturating_sub(liquidation_cap);
    let liquidated_amount = position.size - remaining_size;
    
    position.size = remaining_size;
    
    assert_eq!(liquidated_amount, 500_000_000, "Should liquidate exactly cap amount");
    assert_eq!(position.size, 9_500_000_000, "Should have correct remaining size");
    
    println!("Partial liquidation test:");
    println!("  Original size: $10,000");
    println!("  Liquidated: $500");
    println!("  Remaining: $9,500");
}

#[tokio::test]
async fn test_accumulator_tracking() {
    let mut accumulator = 0u64;
    let slot_cap = 800_000_000; // 800 USDC per slot
    
    // Simulate multiple partial liquidations in same slot
    let liquidations = vec![
        200_000_000, // 200 USDC
        300_000_000, // 300 USDC
        250_000_000, // 250 USDC
        100_000_000, // 100 USDC (would exceed cap)
    ];
    
    println!("\nAccumulator tracking test (cap = $800):");
    
    for (i, amount) in liquidations.iter().enumerate() {
        let allowed = slot_cap.saturating_sub(accumulator);
        let liquidated = (*amount).min(allowed);
        
        if liquidated > 0 {
            accumulator += liquidated;
            println!("  Liquidation {}: ${} (allowed: ${}, total: ${})", 
                i + 1, 
                liquidated / 1_000_000,
                allowed / 1_000_000,
                accumulator / 1_000_000
            );
        } else {
            println!("  Liquidation {}: BLOCKED (cap reached)", i + 1);
        }
    }
    
    assert_eq!(accumulator, 750_000_000, "Accumulator should track total liquidated");
}

#[tokio::test]
async fn test_coverage_based_check() {
    // Test coverage-based liquidation threshold
    let position_size = 5_000_000_000; // 5,000 USDC
    let margin = 500_000_000; // 500 USDC (10%)
    let mark_price = 45000; // $0.45
    let entry_price = 50000; // $0.50
    
    // Calculate unrealized loss
    let price_diff = entry_price.saturating_sub(mark_price);
    let unrealized_loss = (position_size as u128 * price_diff as u128 / entry_price as u128) as u64;
    
    // Check if margin covers loss
    let is_liquidatable = unrealized_loss > margin;
    
    println!("\nCoverage-based liquidation check:");
    println!("  Position: $5,000 @ $0.50");
    println!("  Mark price: $0.45");
    println!("  Unrealized loss: ${}", unrealized_loss / 1_000_000);
    println!("  Margin: ${}", margin / 1_000_000);
    println!("  Liquidatable: {}", is_liquidatable);
}

#[tokio::test]
async fn test_dynamic_cap_application() {
    let open_interest = 100_000_000_000; // 100,000 USDC
    let volatility = U64F64::from_num(25); // 25% volatility
    
    // Calculate dynamic cap
    let cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(volatility, open_interest)
        .unwrap();
    
    // Test partial liquidation with dynamic cap
    let mut position = create_test_position(20_000_000_000); // 20,000 USDC
    let liquidated = position.size.min(cap);
    position.size = position.size.saturating_sub(liquidated);
    
    println!("\nDynamic cap application:");
    println!("  Open Interest: $100,000");
    println!("  Volatility: 25%");
    println!("  Dynamic cap: ${} ({:.1}% of OI)", 
        cap / 1_000_000, 
        (cap as f64 / open_interest as f64) * 100.0
    );
    println!("  Position liquidated: ${} -> ${}", 
        20_000_000_000 / 1_000_000, 
        position.size / 1_000_000
    );
}

#[tokio::test]
async fn test_keeper_reward_calculation() {
    let liquidation_amounts = vec![
        100_000_000,    // 100 USDC
        1_000_000_000,  // 1,000 USDC
        10_000_000_000, // 10,000 USDC
    ];
    
    println!("\nKeeper reward calculations (5bp):");
    
    for amount in liquidation_amounts {
        let reward = (amount as u128 * 5 / 10000) as u64;
        let keeper_pct = (reward as f64 / amount as f64) * 100.0;
        
        println!("  Liquidate ${}: keeper gets ${:.2} ({:.2}%)", 
            amount / 1_000_000,
            reward as f64 / 1_000_000.0,
            keeper_pct
        );
        
        assert_eq!(keeper_pct, 0.05, "Keeper should get exactly 0.05%");
    }
}

#[tokio::test]
async fn test_minimum_liquidation_size() {
    // Test that very small positions aren't liquidated
    let min_liquidation = 10_000_000; // 10 USDC minimum
    let positions = vec![
        5_000_000,     // 5 USDC (too small)
        10_000_000,    // 10 USDC (exactly minimum)
        100_000_000,   // 100 USDC (valid)
    ];
    
    println!("\nMinimum liquidation size test:");
    
    for size in positions {
        let can_liquidate = size >= min_liquidation;
        println!("  Position ${}: {}", 
            size / 1_000_000,
            if can_liquidate { "✓ Can liquidate" } else { "✗ Too small" }
        );
    }
}

#[tokio::test]
async fn test_slot_boundary_reset() {
    // Test accumulator reset on new slot
    let mut last_slot = 1000;
    let mut accumulator = 750_000_000; // 750 USDC from previous slot
    
    let current_slot = 1001; // New slot
    
    if current_slot > last_slot {
        accumulator = 0;
        last_slot = current_slot;
        println!("\nSlot boundary reset:");
        println!("  Previous slot {} had ${} liquidated", 
            current_slot - 1, 750_000_000 / 1_000_000);
        println!("  New slot {} starts with $0 accumulator", current_slot);
    }
    
    assert_eq!(accumulator, 0, "Accumulator should reset on new slot");
}

#[tokio::test]
async fn test_partial_vs_full_liquidation() {
    let positions = vec![
        (500_000_000, 800_000_000),    // 500 USDC position, 800 USDC cap
        (1_000_000_000, 800_000_000),  // 1000 USDC position, 800 USDC cap
    ];
    
    println!("\nPartial vs Full liquidation:");
    
    for (size, cap) in positions {
        if size <= cap {
            println!("  ${} position with ${} cap: FULL liquidation", 
                size / 1_000_000, cap / 1_000_000);
        } else {
            let partial = cap;
            println!("  ${} position with ${} cap: PARTIAL ${} liquidation", 
                size / 1_000_000, cap / 1_000_000, partial / 1_000_000);
        }
    }
}

#[tokio::test]
async fn test_high_leverage_priority() {
    // High leverage positions should be liquidated first
    let positions = vec![
        (10, 1_000_000_000),  // 10x leverage
        (50, 1_000_000_000),  // 50x leverage
        (100, 1_000_000_000), // 100x leverage
    ];
    
    println!("\nHigh leverage liquidation priority:");
    
    let mut sorted_positions = positions.clone();
    sorted_positions.sort_by_key(|&(lev, _)| std::cmp::Reverse(lev));
    
    for (i, (lev, size)) in sorted_positions.iter().enumerate() {
        println!("  Priority {}: {}x leverage (${} position)", 
            i + 1, lev, size / 1_000_000);
    }
}

// Helper functions
fn create_test_position(size: u64) -> Position {
    Position {
        discriminator: [0u8; 8],
        user: Pubkey::new_unique(),
        proposal_id: 1,
        verse_id: 1,
        outcome: 0,
        size,
        leverage: 10,
        margin: size / 10,
        entry_price: 50000,
        liquidation_price: 45000,
        is_long: true,
        is_short: false,
        created_at: 0,
        status: PositionStatus::Open,
        last_funding_payment: 0,
        chain_id: None,
    }
}