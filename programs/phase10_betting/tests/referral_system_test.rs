use anchor_lang::prelude::*;
use phase10_betting::*;
use phase10_betting::types::U64F64;
use phase10_betting::state::{BootstrapState, BootstrapTrader, BootstrapStatus};
use phase10_betting::bootstrap::BootstrapIncentiveEngine;

/// Comprehensive test for the referral bonus system
#[test]
fn test_referral_system_end_to_end() {
    println!("\n=== End-to-End Referral System Test ===\n");

    // 1. Initialize bootstrap state
    let mut bootstrap_state = BootstrapState {
        epoch: 1,
        initial_vault_balance: 0,
        current_vault_balance: 0,
        bootstrap_mmt_allocation: 2_000_000 * 10u64.pow(6),
        mmt_distributed: 0,
        unique_traders: 0,
        total_volume: 0,
        status: BootstrapStatus::Active,
        initial_coverage: U64F64::zero(),
        current_coverage: U64F64::zero(),
        target_coverage: U64F64::one(),
        start_slot: 1000,
        expected_end_slot: 1000 + 38_880_000,
        early_bonus_multiplier: U64F64::from_num(2u32),
        early_traders_count: 0,
        max_early_traders: 100,
        min_trade_size: 10 * 10u64.pow(6),
        bootstrap_fee_bps: 28,
        _padding: [0; 256],
    };

    // 2. Create referrer (existing trader)
    let mut referrer = BootstrapTrader {
        trader: Pubkey::new_unique(),
        volume_traded: 100_000 * 10u64.pow(6), // $100k previous volume
        mmt_earned: 1000 * 10u64.pow(6),
        trade_count: 10,
        is_early_trader: true,
        first_trade_slot: 1000,
        avg_leverage: U64F64::from_num(5u32),
        vault_contribution: 28 * 10u64.pow(6),
        referral_bonus: 0,
        referred_count: 0,
    };

    println!("Referrer State Before:");
    println!("  Volume traded: ${}", referrer.volume_traded / 10u64.pow(6));
    println!("  MMT earned: {} MMT", referrer.mmt_earned / 10u64.pow(6));
    println!("  Referral bonus: {} MMT", referrer.referral_bonus / 10u64.pow(6));
    println!("  Referred count: {}\n", referrer.referred_count);

    // 3. Create referred trader
    let mut referred_trader = BootstrapTrader {
        trader: Pubkey::new_unique(),
        volume_traded: 0,
        mmt_earned: 0,
        trade_count: 0,
        is_early_trader: false,
        first_trade_slot: 0,
        avg_leverage: U64F64::zero(),
        vault_contribution: 0,
        referral_bonus: 0,
        referred_count: 0,
    };

    let clock = Clock {
        slot: 5000,
        epoch_start_timestamp: 0,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: 1234567890,
    };

    // 4. Referred trader makes a trade
    let referred_trade_volume = 50_000 * 10u64.pow(6); // $50k trade
    let leverage = U64F64::from_num(3u32);
    let fee_bps = bootstrap_state.calculate_bootstrap_fee();
    let fee_paid = (referred_trade_volume as u128 * fee_bps as u128 / 10_000) as u64;

    println!("Referred Trader Trade:");
    println!("  Volume: ${}", referred_trade_volume / 10u64.pow(6));
    println!("  Leverage: {}x", leverage.to_num::<u32>());
    println!("  Fee: ${} ({} bps)\n", fee_paid / 10u64.pow(6), fee_bps);

    // Process the trade
    let trade_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut referred_trader,
        referred_trade_volume,
        fee_paid,
        leverage,
        &clock,
    ).unwrap();

    println!("Trade Results:");
    println!("  MMT reward: {} MMT", trade_result.mmt_reward / 10u64.pow(6));
    println!("  Net fee to vault: ${}\n", trade_result.net_fee / 10u64.pow(6));

    // 5. Process referral bonus
    let referral_bonus = BootstrapIncentiveEngine::process_referral(
        &mut referrer,
        referred_trade_volume,
        &bootstrap_state,
    ).unwrap();

    println!("Referral Bonus Calculation:");
    println!("  Referred volume: ${}", referred_trade_volume / 10u64.pow(6));
    println!("  Referred trader MMT reward: {} MMT", trade_result.mmt_reward / 10u64.pow(6));
    println!("  Referral rate: 5%");
    println!("  Referral bonus: {} MMT\n", referral_bonus / 10u64.pow(6));

    // Verify calculations
    let expected_referral_bonus = (trade_result.mmt_reward as u128 * 5 / 100) as u64;
    assert_eq!(referral_bonus, expected_referral_bonus, "Referral bonus calculation mismatch");

    println!("Referrer State After:");
    println!("  Volume traded: ${}", referrer.volume_traded / 10u64.pow(6));
    println!("  MMT earned: {} MMT", referrer.mmt_earned / 10u64.pow(6));
    println!("  Referral bonus: {} MMT", referrer.referral_bonus / 10u64.pow(6));
    println!("  Referred count: {}", referrer.referred_count);
    println!("  Total MMT (earned + referrals): {} MMT\n", 
        (referrer.mmt_earned + referrer.referral_bonus) / 10u64.pow(6));

    // 6. Multiple referrals test
    println!("Testing Multiple Referrals:");
    let mut total_referral_bonus = referrer.referral_bonus;
    
    for i in 1..=5 {
        let mut new_referred = BootstrapTrader {
            trader: Pubkey::new_unique(),
            volume_traded: 0,
            mmt_earned: 0,
            trade_count: 0,
            is_early_trader: false,
            first_trade_slot: 0,
            avg_leverage: U64F64::zero(),
            vault_contribution: 0,
            referral_bonus: 0,
            referred_count: 0,
        };

        let trade_vol = (20_000 + i * 5_000) * 10u64.pow(6);
        let fee = (trade_vol as u128 * fee_bps as u128 / 10_000) as u64;

        let result = BootstrapIncentiveEngine::process_bootstrap_trade(
            &mut bootstrap_state,
            &mut new_referred,
            trade_vol,
            fee,
            U64F64::from_num(4u32),
            &clock,
        ).unwrap();

        let bonus = BootstrapIncentiveEngine::process_referral(
            &mut referrer,
            trade_vol,
            &bootstrap_state,
        ).unwrap();

        total_referral_bonus += bonus;
        
        println!("  Referral {}: ${} volume -> {} MMT bonus", 
            i, 
            trade_vol / 10u64.pow(6),
            bonus / 10u64.pow(6)
        );
    }

    println!("\nFinal Referrer Stats:");
    println!("  Total referred traders: {}", referrer.referred_count);
    println!("  Total referral bonus: {} MMT", referrer.referral_bonus / 10u64.pow(6));
    println!("  Average bonus per referral: {} MMT", 
        referrer.referral_bonus / referrer.referred_count / 10u64.pow(6));

    // 7. Test edge cases
    println!("\n=== Testing Edge Cases ===");

    // Edge case 1: Referrer refers someone with larger volume than themselves
    let mut whale_referred = BootstrapTrader {
        trader: Pubkey::new_unique(),
        volume_traded: 0,
        mmt_earned: 0,
        trade_count: 0,
        is_early_trader: false,
        first_trade_slot: 0,
        avg_leverage: U64F64::zero(),
        vault_contribution: 0,
        referral_bonus: 0,
        referred_count: 0,
    };

    let whale_volume = 1_000_000 * 10u64.pow(6); // $1M trade
    let whale_fee = (whale_volume as u128 * fee_bps as u128 / 10_000) as u64;

    let whale_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut whale_referred,
        whale_volume,
        whale_fee,
        U64F64::from_num(10u32),
        &clock,
    ).unwrap();

    let whale_referral_bonus = BootstrapIncentiveEngine::process_referral(
        &mut referrer,
        whale_volume,
        &bootstrap_state,
    ).unwrap();

    println!("\nWhale Referral:");
    println!("  Whale volume: ${}", whale_volume / 10u64.pow(6));
    println!("  Whale MMT reward: {} MMT", whale_result.mmt_reward / 10u64.pow(6));
    println!("  Referrer bonus from whale: {} MMT", whale_referral_bonus / 10u64.pow(6));

    // Verify all calculations
    assert!(referrer.referral_bonus > 0, "Referrer should have earned referral bonuses");
    assert_eq!(referrer.referred_count, 7, "Referrer should have 7 referred traders");
    assert!(bootstrap_state.mmt_distributed > 0, "MMT should have been distributed");

    println!("\n✅ All referral system tests passed!");
}

/// Test referral bonus tiers based on volume
#[test]
fn test_referral_tier_calculations() {
    println!("\n=== Referral Tier Calculations Test ===\n");

    let bootstrap_state = BootstrapState {
        bootstrap_mmt_allocation: 2_000_000 * 10u64.pow(6),
        mmt_distributed: 0,
        early_bonus_multiplier: U64F64::from_num(2u32),
        ..Default::default()
    };

    // Test different trader tiers
    let tiers = vec![
        (0, "Base"),
        (10_000 * 10u64.pow(6), "Bronze"),
        (100_000 * 10u64.pow(6), "Silver"),
        (1_000_000 * 10u64.pow(6), "Gold"),
    ];

    for (min_volume, tier_name) in tiers {
        let tier = BootstrapIncentiveEngine::get_trader_tier(min_volume);
        let trade_volume = 50_000 * 10u64.pow(6);
        
        // Calculate base reward
        let base_reward = bootstrap_state.calculate_mmt_reward(
            trade_volume,
            false,
            &tier,
        );

        // Calculate referral bonus (5% of base reward)
        let referral_bonus = (base_reward as u128 * 5 / 100) as u64;

        println!("{} Tier (${} min volume):", tier_name, min_volume / 10u64.pow(6));
        println!("  Reward multiplier: {}x", tier.reward_multiplier.to_num::<f64>());
        println!("  Fee rebate: {} bps", tier.fee_rebate_bps);
        println!("  Base reward for $50k: {} MMT", base_reward / 10u64.pow(6));
        println!("  Referral bonus (5%): {} MMT\n", referral_bonus / 10u64.pow(6));
    }
}

/// Test referral chain (referrer refers someone who refers someone else)
#[test]
fn test_referral_chain() {
    println!("\n=== Referral Chain Test ===\n");

    let mut bootstrap_state = BootstrapState {
        bootstrap_mmt_allocation: 2_000_000 * 10u64.pow(6),
        status: BootstrapStatus::Active,
        ..Default::default()
    };

    // Create chain: Alice -> Bob -> Charlie
    let mut alice = BootstrapTrader {
        trader: Pubkey::new_unique(),
        volume_traded: 50_000 * 10u64.pow(6),
        mmt_earned: 500 * 10u64.pow(6),
        trade_count: 5,
        is_early_trader: true,
        ..Default::default()
    };

    let mut bob = BootstrapTrader {
        trader: Pubkey::new_unique(),
        ..Default::default()
    };

    let mut charlie = BootstrapTrader {
        trader: Pubkey::new_unique(),
        ..Default::default()
    };

    let clock = Clock {
        slot: 2000,
        ..Default::default()
    };

    // Bob trades (referred by Alice)
    let bob_volume = 30_000 * 10u64.pow(6);
    let bob_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut bob,
        bob_volume,
        bob_volume * 28 / 10_000,
        U64F64::from_num(5u32),
        &clock,
    ).unwrap();

    let alice_bonus_from_bob = BootstrapIncentiveEngine::process_referral(
        &mut alice,
        bob_volume,
        &bootstrap_state,
    ).unwrap();

    println!("Alice refers Bob:");
    println!("  Bob's volume: ${}", bob_volume / 10u64.pow(6));
    println!("  Bob's MMT reward: {} MMT", bob_result.mmt_reward / 10u64.pow(6));
    println!("  Alice's referral bonus: {} MMT\n", alice_bonus_from_bob / 10u64.pow(6));

    // Charlie trades (referred by Bob)
    let charlie_volume = 20_000 * 10u64.pow(6);
    let charlie_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut charlie,
        charlie_volume,
        charlie_volume * 28 / 10_000,
        U64F64::from_num(3u32),
        &clock,
    ).unwrap();

    let bob_bonus_from_charlie = BootstrapIncentiveEngine::process_referral(
        &mut bob,
        charlie_volume,
        &bootstrap_state,
    ).unwrap();

    println!("Bob refers Charlie:");
    println!("  Charlie's volume: ${}", charlie_volume / 10u64.pow(6));
    println!("  Charlie's MMT reward: {} MMT", charlie_result.mmt_reward / 10u64.pow(6));
    println!("  Bob's referral bonus: {} MMT\n", bob_bonus_from_charlie / 10u64.pow(6));

    println!("Final Stats:");
    println!("  Alice: {} MMT earned + {} MMT referral = {} MMT total",
        alice.mmt_earned / 10u64.pow(6),
        alice.referral_bonus / 10u64.pow(6),
        (alice.mmt_earned + alice.referral_bonus) / 10u64.pow(6)
    );
    println!("  Bob: {} MMT earned + {} MMT referral = {} MMT total",
        bob.mmt_earned / 10u64.pow(6),
        bob.referral_bonus / 10u64.pow(6),
        (bob.mmt_earned + bob.referral_bonus) / 10u64.pow(6)
    );
    println!("  Charlie: {} MMT earned",
        charlie.mmt_earned / 10u64.pow(6)
    );

    // Note: In this implementation, only direct referrals get bonuses
    // Alice doesn't get a bonus from Charlie's trades
    println!("\n✅ Referral chain test completed!");
}

fn main() {
    test_referral_system_end_to_end();
    test_referral_tier_calculations();
    test_referral_chain();
}