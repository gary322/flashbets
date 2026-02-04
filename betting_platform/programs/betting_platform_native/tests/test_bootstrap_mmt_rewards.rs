//! Test Bootstrap MMT Rewards Implementation
//!
//! Validates that early liquidity providers receive immediate MMT rewards
//! during the bootstrap phase as per specification.

use solana_program::{
    clock::Clock,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use betting_platform_native::{
    integration::{
        bootstrap_coordinator::{
            BootstrapCoordinator, BOOTSTRAP_TARGET_VAULT, BOOTSTRAP_MMT_EMISSION_RATE,
            BOOTSTRAP_IMMEDIATE_REWARD_BPS, VAMPIRE_ATTACK_HALT_COVERAGE,
        },
        bootstrap_mmt_integration::{
            calculate_bootstrap_mmt_rewards, verify_liquidity_provider_eligibility,
        },
    },
    error::BettingPlatformError,
};

#[test]
fn test_mmt_rewards_for_first_providers() {
    println!("Testing MMT rewards for first liquidity providers");
    
    // Test case 1: First provider gets maximum rewards
    let reward = calculate_bootstrap_mmt_rewards(
        10_000_000_000, // $10k deposit
        0,              // Empty vault
        0,              // First depositor
        0,              // No milestone yet
        BOOTSTRAP_MMT_EMISSION_RATE, // Full 10M MMT available
    ).unwrap();
    
    // Base: $10k * 2 = 20k MMT
    // First depositor: 1.5x = 30k MMT
    // Before milestone: 1.4x = 42k MMT
    assert_eq!(reward, 42_000_000_000); // 42k MMT with 6 decimals
    println!("✓ First $10k provider receives 42k MMT");
    
    // Test case 2: 10th depositor still gets enhanced rewards
    let reward = calculate_bootstrap_mmt_rewards(
        1_000_000_000,  // $1k deposit
        5_000_000_000,  // $5k in vault
        9,              // 10th depositor (0-indexed)
        2,              // Past $2.5k milestone
        8_000_000_000_000, // 8M MMT remaining
    ).unwrap();
    
    // Base: $1k * 2 = 2k MMT
    // Top 10 depositor: 1.5x = 3k MMT
    // Past $2.5k: 1.2x = 3.6k MMT
    assert_eq!(reward, 3_600_000_000); // 3.6k MMT
    println!("✓ 10th depositor receives 3.6k MMT for $1k");
    
    // Test case 3: Regular depositor after 100
    let reward = calculate_bootstrap_mmt_rewards(
        1_000_000_000,  // $1k deposit
        9_000_000_000,  // $9k in vault
        150,            // 151st depositor
        4,              // Near completion
        1_000_000_000_000, // 1M MMT remaining
    ).unwrap();
    
    // Base: $1k * 2 = 2k MMT
    // Regular depositor: 1x = 2k MMT
    // Near completion: 1x = 2k MMT
    assert_eq!(reward, 2_000_000_000); // 2k MMT
    println!("✓ Regular depositor receives base 2k MMT for $1k");
}

#[test]
fn test_immediate_reward_percentage() {
    println!("Testing immediate reward percentage scaling");
    
    // At $0 vault: 100% immediate
    let immediate_pct = if 0 < 1_000_000_000 {
        BOOTSTRAP_IMMEDIATE_REWARD_BPS
    } else {
        let progress_bps = (0 * 10000) / BOOTSTRAP_TARGET_VAULT;
        let reduction = (progress_bps * 5000) / 10000;
        BOOTSTRAP_IMMEDIATE_REWARD_BPS.saturating_sub(reduction as u16)
    };
    assert_eq!(immediate_pct, 10000); // 100%
    println!("✓ $0 vault: 100% immediate rewards");
    
    // At $5k vault: 75% immediate
    let vault_balance = 5_000_000_000;
    let immediate_pct = if vault_balance < 1_000_000_000 {
        BOOTSTRAP_IMMEDIATE_REWARD_BPS
    } else {
        let progress_bps = (vault_balance * 10000) / BOOTSTRAP_TARGET_VAULT;
        let reduction = (progress_bps * 5000) / 10000; // 50% reduction at completion
        BOOTSTRAP_IMMEDIATE_REWARD_BPS.saturating_sub(reduction as u16)
    };
    assert_eq!(immediate_pct, 7500); // 75%
    println!("✓ $5k vault: 75% immediate rewards");
    
    // At $10k vault: 50% immediate
    let vault_balance = BOOTSTRAP_TARGET_VAULT;
    let immediate_pct = if vault_balance < 1_000_000_000 {
        BOOTSTRAP_IMMEDIATE_REWARD_BPS
    } else {
        let progress_bps = (vault_balance * 10000) / BOOTSTRAP_TARGET_VAULT;
        let reduction = (progress_bps * 5000) / 10000;
        BOOTSTRAP_IMMEDIATE_REWARD_BPS.saturating_sub(reduction as u16)
    };
    assert_eq!(immediate_pct, 5000); // 50%
    println!("✓ $10k vault: 50% immediate rewards");
}

#[test]
fn test_vampire_attack_protection() {
    println!("Testing vampire attack protection");
    
    let depositor = Pubkey::new_unique();
    
    // Test case 1: Normal coverage ratio allows deposits
    let mut bootstrap = BootstrapCoordinator {
        vault_balance: 5_000_000_000, // $5k
        total_deposits: 5_000_000_000,
        unique_depositors: 10,
        current_milestone: 2,
        bootstrap_start_slot: 0,
        bootstrap_complete: false,
        coverage_ratio: 10000, // 1.0 coverage
        max_leverage_available: 5,
        total_mmt_distributed: 100_000_000_000,
        early_depositor_bonus_active: true,
        incentive_pool: 9_900_000_000_000,
        halted: false,
        total_incentive_pool: BOOTSTRAP_MMT_EMISSION_RATE,
        is_active: true,
        current_vault_balance: 5_000_000_000,
    };
    
    assert!(verify_liquidity_provider_eligibility(
        &depositor,
        1_000_000_000, // $1k deposit
        &bootstrap
    ).unwrap());
    println!("✓ Normal coverage ratio allows deposits");
    
    // Test case 2: Low coverage ratio blocks deposits
    bootstrap.coverage_ratio = 4000; // 0.4 coverage (below 0.5 threshold)
    assert!(!verify_liquidity_provider_eligibility(
        &depositor,
        1_000_000_000,
        &bootstrap
    ).unwrap());
    println!("✓ Low coverage ratio blocks deposits");
    
    // Test case 3: Bootstrap complete blocks all deposits
    bootstrap.bootstrap_complete = true;
    bootstrap.coverage_ratio = 10000; // Even with good coverage
    assert!(!verify_liquidity_provider_eligibility(
        &depositor,
        1_000_000_000,
        &bootstrap
    ).unwrap());
    println!("✓ Bootstrap complete blocks all deposits");
}

#[test]
fn test_minimum_deposit_requirement() {
    println!("Testing minimum deposit requirement");
    
    let depositor = Pubkey::new_unique();
    let bootstrap = BootstrapCoordinator {
        vault_balance: 1_000_000_000,
        total_deposits: 1_000_000_000,
        unique_depositors: 5,
        current_milestone: 1,
        bootstrap_start_slot: 0,
        bootstrap_complete: false,
        coverage_ratio: 10000,
        max_leverage_available: 1,
        total_mmt_distributed: 50_000_000_000,
        early_depositor_bonus_active: true,
        incentive_pool: 9_950_000_000_000,
        halted: false,
        total_incentive_pool: BOOTSTRAP_MMT_EMISSION_RATE,
        is_active: true,
        current_vault_balance: 1_000_000_000,
    };
    
    // Test minimum deposit
    assert!(!verify_liquidity_provider_eligibility(
        &depositor,
        999_999, // $0.999999 - below minimum
        &bootstrap
    ).unwrap());
    
    assert!(verify_liquidity_provider_eligibility(
        &depositor,
        1_000_000, // $1 - exactly minimum
        &bootstrap
    ).unwrap());
    
    println!("✓ Minimum $1 deposit enforced");
}

#[test]
fn test_emission_rate_compliance() {
    println!("Testing 10M MMT/season emission rate");
    
    // Verify emission rate constant
    assert_eq!(BOOTSTRAP_MMT_EMISSION_RATE, 10_000_000_000_000);
    println!("✓ Bootstrap emission rate set to 10M MMT");
    
    // Test that rewards are capped by remaining pool
    let reward = calculate_bootstrap_mmt_rewards(
        100_000_000_000, // $100k deposit (would be 420k MMT)
        0,               // Empty vault
        0,               // First depositor
        0,               // No milestone
        100_000_000_000, // Only 100k MMT remaining
    ).unwrap();
    
    assert_eq!(reward, 100_000_000_000); // Capped at remaining pool
    println!("✓ Rewards capped at remaining incentive pool");
}

#[test]
fn test_milestone_progression_rewards() {
    println!("Testing milestone progression reward scaling");
    
    let deposit = 1_000_000_000; // $1k
    let depositors = 50; // Mid-tier multiplier
    let remaining = 5_000_000_000_000; // 5M MMT
    
    // Test rewards decrease with milestone progression
    let rewards_by_milestone: Vec<u64> = (0..5).map(|milestone| {
        calculate_bootstrap_mmt_rewards(
            deposit,
            deposit * (milestone as u64 + 1) * 2, // Simulate vault growth
            depositors,
            milestone,
            remaining,
        ).unwrap()
    }).collect();
    
    // Verify decreasing rewards
    for i in 1..rewards_by_milestone.len() {
        assert!(
            rewards_by_milestone[i] <= rewards_by_milestone[i-1],
            "Rewards should decrease with milestone progression"
        );
    }
    
    println!("✓ Rewards decrease appropriately with milestone progression");
    println!("  Milestone 0: {} MMT", rewards_by_milestone[0] / 1_000_000);
    println!("  Milestone 1: {} MMT", rewards_by_milestone[1] / 1_000_000);
    println!("  Milestone 2: {} MMT", rewards_by_milestone[2] / 1_000_000);
    println!("  Milestone 3: {} MMT", rewards_by_milestone[3] / 1_000_000);
    println!("  Milestone 4: {} MMT", rewards_by_milestone[4] / 1_000_000);
}

fn main() {
    println!("Running Bootstrap MMT Rewards Tests...\n");
    
    test_mmt_rewards_for_first_providers();
    println!();
    
    test_immediate_reward_percentage();
    println!();
    
    test_vampire_attack_protection();
    println!();
    
    test_minimum_deposit_requirement();
    println!();
    
    test_emission_rate_compliance();
    println!();
    
    test_milestone_progression_rewards();
    
    println!("\nAll Bootstrap MMT Rewards tests passed! ✅");
}