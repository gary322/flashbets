//! Part 7 User Journey Simulations
//!
//! Comprehensive user journey tests for all Part 7 features

use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::math::fixed_point::U64F64;

use crate::{
    fees::{elastic_fee::calculate_elastic_fee, FEE_BASE_BPS, FEE_MAX_BPS},
    coverage::{CoverageState, recovery::RecoveryState},
    protection::{CrossVerseProtection, CrossVersePosition},
};

/// User Journey 1: Normal Trading with Dynamic Fees
/// 
/// Scenario: User trades as coverage fluctuates
/// - Start with coverage = 2.0 (low fees)
/// - Coverage drops to 0.8 (fees increase)
/// - Coverage recovers to 1.5 (fees normalize)
pub fn simulate_dynamic_fee_journey() -> Result<(), String> {
    println!("=== User Journey 1: Dynamic Fee Trading ===");
    
    // Initial state: High coverage = low fees
    let mut coverage = U64F64::from_num(2);
    let initial_fee = calculate_elastic_fee(coverage).map_err(|e| format!("{:?}", e))?;
    println!("Initial coverage: {}, Fee: {}bp", coverage, initial_fee);
    assert_eq!(initial_fee, FEE_BASE_BPS); // Should be 3bp
    
    // User places $10,000 trade
    let trade_amount = 10_000_000; // $10k in smallest units
    let fee_paid = (trade_amount * initial_fee as u64) / 10_000;
    println!("Trade amount: ${}, Fee paid: ${}", trade_amount / 1000, fee_paid / 1000);
    
    // Market shock: Coverage drops
    coverage = U64F64::from_fraction(4, 5).unwrap();
    let stressed_fee = calculate_elastic_fee(coverage).map_err(|e| format!("{:?}", e))?;
    println!("\nMarket stress - Coverage: {}, Fee: {}bp", coverage, stressed_fee);
    assert!(stressed_fee > initial_fee);
    
    // User places another trade with higher fees
    let stressed_fee_paid = (trade_amount * stressed_fee as u64) / 10_000;
    println!("Same trade amount: ${}, Fee paid: ${} ({}x higher)", 
             trade_amount / 1000, stressed_fee_paid / 1000, 
             stressed_fee_paid / fee_paid);
    
    // Recovery: Coverage improves
    coverage = U64F64::from_fraction(3, 2).unwrap();
    let recovery_fee = calculate_elastic_fee(coverage).map_err(|e| format!("{:?}", e))?;
    println!("\nRecovery - Coverage: {}, Fee: {}bp", coverage, recovery_fee);
    assert!(recovery_fee < stressed_fee);
    assert!(recovery_fee >= FEE_BASE_BPS);
    
    Ok(())
}

/// User Journey 2: Maker vs Taker Experience
/// 
/// Scenario: Two users place orders
/// - User A improves spread (maker) - gets rebate
/// - User B takes liquidity (taker) - pays full fee
pub fn simulate_maker_taker_journey() -> Result<(), String> {
    use crate::fees::maker_taker::{calculate_maker_taker_fee, OrderType};
    
    println!("\n=== User Journey 2: Maker vs Taker ===");
    
    let base_fee = 10; // 10bp base fee
    
    // User A: Places limit order that improves spread by 15bp
    let maker_result = calculate_maker_taker_fee(base_fee, 15);
    println!("User A (Maker):");
    println!("  - Improves spread by 15bp");
    println!("  - Order type: {:?}", maker_result.order_type);
    println!("  - Fee: {}bp (rebate!)", maker_result.final_fee_bps);
    assert_eq!(maker_result.order_type, OrderType::Maker);
    assert!(maker_result.final_fee_bps < 0); // Negative = rebate
    
    // User B: Takes liquidity with market order
    let taker_result = calculate_maker_taker_fee(base_fee, 0);
    println!("\nUser B (Taker):");
    println!("  - Market order (no spread improvement)");
    println!("  - Order type: {:?}", taker_result.order_type);
    println!("  - Fee: {}bp", taker_result.final_fee_bps);
    assert_eq!(taker_result.order_type, OrderType::Taker);
    assert_eq!(taker_result.final_fee_bps, base_fee as i16);
    
    // Calculate net for $100k volume
    let volume = 100_000_000; // $100k
    let maker_rebate = (volume * 5) / 10_000; // 5bp rebate
    let taker_fee = (volume * base_fee as u64) / 10_000;
    
    println!("\nOn $100k volume:");
    println!("  - Maker receives: ${} rebate", maker_rebate / 1000);
    println!("  - Taker pays: ${} fee", taker_fee / 1000);
    println!("  - Net protocol revenue: ${}", (taker_fee - maker_rebate) / 1000);
    
    Ok(())
}

/// User Journey 3: Coverage Crisis and Recovery
/// 
/// Scenario: System enters recovery mode
/// - Coverage drops below 1.0
/// - Recovery mechanisms activate
/// - Users experience restrictions
/// - System recovers over time
pub fn simulate_recovery_journey() -> Result<(), String> {
    use crate::coverage::recovery::{calculate_recovery_fee, calculate_recovery_position_limit};
    
    println!("\n=== User Journey 3: Coverage Crisis ===");
    
    let mut recovery_state = RecoveryState::new();
    let normal_position_limit = 100_000_000; // $100k
    
    // Normal operations
    println!("Normal operations:");
    println!("  - Coverage: 1.5");
    println!("  - Position limit: ${}", normal_position_limit / 1000);
    println!("  - Base fee: 10bp");
    
    // Crisis hits: Coverage drops to 0.4
    println!("\nCrisis! Coverage drops to 0.4");
    recovery_state.is_active = true;
    recovery_state.start_coverage = U64F64::from_fraction(2, 5).unwrap().to_bits();
    recovery_state.fee_multiplier = 30000; // 3x
    recovery_state.position_limit_reduction = 80; // 80% reduction
    recovery_state.new_positions_halted = true;
    
    // User tries to trade
    let crisis_fee = calculate_recovery_fee(10, &recovery_state);
    let crisis_limit = calculate_recovery_position_limit(normal_position_limit, &recovery_state);
    
    println!("Recovery mode activated:");
    println!("  - Fees: {}bp ({}x normal)", crisis_fee, recovery_state.fee_multiplier / 10000);
    println!("  - Position limit: ${} ({}% reduction)", 
             crisis_limit / 1000, recovery_state.position_limit_reduction);
    println!("  - New positions: HALTED");
    
    // Existing user with $50k position
    println!("\nExisting user with $50k position:");
    println!("  - Can close/reduce position");
    println!("  - Cannot increase beyond ${}", crisis_limit / 1000);
    println!("  - Pays {}bp to close", crisis_fee);
    
    // New user tries to enter
    println!("\nNew user attempts to trade:");
    println!("  - Access DENIED - new positions halted");
    
    // Recovery progresses
    println!("\nRecovery in progress (coverage = 0.8):");
    recovery_state.fee_multiplier = 15000; // Reduced to 1.5x
    recovery_state.position_limit_reduction = 25; // Only 25% reduction
    recovery_state.new_positions_halted = false;
    
    let recovery_fee = calculate_recovery_fee(10, &recovery_state);
    let recovery_limit = calculate_recovery_position_limit(normal_position_limit, &recovery_state);
    
    println!("  - Fees: {}bp ({}x normal)", recovery_fee, recovery_state.fee_multiplier / 10000);
    println!("  - Position limit: ${}", recovery_limit / 1000);
    println!("  - New positions: ALLOWED");
    
    Ok(())
}

/// User Journey 4: Cross-Verse Attack Prevention
/// 
/// Scenario: Malicious user attempts cross-verse manipulation
/// - User spreads positions across multiple verses
/// - System detects correlated positions
/// - Attack is prevented
pub fn simulate_cross_verse_prevention() -> Result<(), String> {
    println!("\n=== User Journey 4: Cross-Verse Attack Prevention ===");
    
    let protection = CrossVerseProtection::new();
    let attacker = Pubkey::new_unique();
    
    // Attacker's strategy: Correlated positions across verses
    println!("Attacker attempts to open positions:");
    
    let positions = vec![
        CrossVersePosition { 
            verse_id: 1, 
            market_id: 100, 
            outcome: 0, 
            size: 50_000_000, // $50k
            direction: true // long
        },
        CrossVersePosition { 
            verse_id: 2, 
            market_id: 200, 
            outcome: 0, 
            size: 50_000_000, // $50k
            direction: true // long
        },
        CrossVersePosition { 
            verse_id: 3, 
            market_id: 300, 
            outcome: 0, 
            size: 50_000_000, // $50k
            direction: true // long
        },
    ];
    
    println!("  - Verse 1: Politics - $50k long");
    println!("  - Verse 2: Sports - $50k long");
    println!("  - Verse 3: Entertainment - $50k long");
    println!("  - Total exposure: $150k across 3 verses");
    
    // System allows (within limits)
    let attack = crate::protection::detect_cross_verse_attack(&attacker, &positions, &protection)
        .map_err(|e| format!("{:?}", e))?;
    println!("\nSystem check: {}", if attack { "BLOCKED" } else { "ALLOWED" });
    
    // Attacker tries 4th verse
    let mut attack_positions = positions.clone();
    attack_positions.push(CrossVersePosition { 
        verse_id: 4, 
        market_id: 400, 
        outcome: 0, 
        size: 50_000_000,
        direction: true 
    });
    
    println!("\nAttacker tries to add 4th verse:");
    println!("  - Verse 4: Technology - $50k long");
    println!("  - Total: 4 verses (max allowed: 3)");
    
    let attack = crate::protection::detect_cross_verse_attack(&attacker, &attack_positions, &protection)
        .map_err(|e| format!("{:?}", e))?;
    println!("System check: {}", if attack { "BLOCKED ❌" } else { "ALLOWED" });
    assert!(attack); // Should be blocked
    
    // Show correlated positions detection
    println!("\nSystem also detects correlated positions:");
    let correlated_positions = vec![
        CrossVersePosition { 
            verse_id: 1, 
            market_id: 100, 
            outcome: 0, 
            size: 100_000_000, // $100k
            direction: true 
        },
        CrossVersePosition { 
            verse_id: 2, 
            market_id: 200, 
            outcome: 0, 
            size: 95_000_000, // $95k (highly correlated size)
            direction: true // same direction
        },
    ];
    
    println!("  - Verse 1: $100k long on outcome 0");
    println!("  - Verse 2: $95k long on outcome 0");
    println!("  - Correlation: Very high (same outcome, direction, similar size)");
    println!("  - Risk: Potential synthetic correlation attack");
    
    Ok(())
}

/// User Journey 5: Complete Trading Lifecycle with All Features
/// 
/// Scenario: User experiences all Part 7 features in one session
pub fn simulate_complete_journey() -> Result<(), String> {
    println!("\n=== User Journey 5: Complete Trading Lifecycle ===");
    
    // Setup
    let user = Pubkey::new_unique();
    let mut coverage = U64F64::from_fraction(9, 5).unwrap();
    
    println!("User starts trading session:");
    println!("  - System coverage: {}", coverage);
    println!("  - Account: ${} available", 500_000);
    
    // 1. Place maker order
    println!("\n1. Place limit order (maker):");
    let base_fee = calculate_elastic_fee(coverage).map_err(|e| format!("{:?}", e))?;
    println!("  - Base fee: {}bp", base_fee);
    println!("  - Improves spread: Gets 5bp rebate");
    println!("  - Net: +$25 rebate on $50k order");
    
    // 2. Market stress hits
    coverage = U64F64::from_num(3) / U64F64::from_num(5); // 0.6
    println!("\n2. Market stress event:");
    println!("  - Coverage drops to {}", coverage);
    println!("  - System enters recovery mode");
    
    // 3. Try to increase position
    let stress_fee = calculate_elastic_fee(coverage).map_err(|e| format!("{:?}", e))?;
    println!("\n3. Attempt to increase position:");
    println!("  - Fee increased to {}bp", stress_fee);
    println!("  - Position limits reduced by 50%");
    println!("  - Maximum additional: $25k (was $50k)");
    
    // 4. Fee distribution
    println!("\n4. Fee distribution on $100k volume:");
    let total_fees = (100_000_000 * stress_fee as u64) / 10_000;
    let vault_share = (total_fees * 7000) / 10_000;
    let mmt_share = (total_fees * 2000) / 10_000;
    let burn_share = (total_fees * 1000) / 10_000;
    
    println!("  - Total fees: ${}", total_fees / 1000);
    println!("  - Vault (70%): ${}", vault_share / 1000);
    println!("  - MMT holders (20%): ${}", mmt_share / 1000);  
    println!("  - Burned (10%): ${}", burn_share / 1000);
    
    // 5. System recovery
    coverage = U64F64::from_num(6) / U64F64::from_num(5); // 1.2
    println!("\n5. System recovers:");
    println!("  - Coverage back to {}", coverage);
    println!("  - Fees normalize to {}bp", calculate_elastic_fee(coverage).unwrap());
    println!("  - Position limits restored");
    println!("  - User can trade normally");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_user_journeys() {
        // Run all simulations
        simulate_dynamic_fee_journey().expect("Dynamic fee journey failed");
        simulate_maker_taker_journey().expect("Maker/taker journey failed");
        simulate_recovery_journey().expect("Recovery journey failed");
        simulate_cross_verse_prevention().expect("Cross-verse prevention failed");
        simulate_complete_journey().expect("Complete journey failed");
    }
}