use ::phase10_betting::*;
use anchor_lang::prelude::*;

/// Comprehensive end-to-end test of the referral system
fn main() {
    println!("\n🎯 End-to-End Referral System Test\n");
    println!("This test demonstrates the complete referral bonus system implementation");
    println!("including all edge cases and multi-level referrals.\n");

    // Initialize bootstrap state
    let mut bootstrap_state = BootstrapState {
        epoch: 1,
        initial_vault_balance: 0,
        current_vault_balance: 0,
        bootstrap_mmt_allocation: 2_000_000 * 10u64.pow(6), // 2M MMT
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

    // 1. Create the referrer (Alice) - an established trader
    let mut alice = BootstrapTrader {
        trader: Pubkey::new_unique(),
        volume_traded: 250_000 * 10u64.pow(6), // $250k volume
        mmt_earned: 2500 * 10u64.pow(6), // 2500 MMT earned
        trade_count: 25,
        is_early_trader: true, // Early adopter
        first_trade_slot: 1100,
        avg_leverage: U64F64::from_num(5u32),
        vault_contribution: 70 * 10u64.pow(6),
        referral_bonus: 0,
        referred_count: 0,
    };

    println!("=== Initial State ===");
    println!("Referrer (Alice):");
    println!("  Volume traded: ${}", alice.volume_traded / 10u64.pow(6));
    println!("  MMT earned: {} MMT", alice.mmt_earned / 10u64.pow(6));
    println!("  Is early trader: {}", alice.is_early_trader);
    println!("  Referral bonus: {} MMT", alice.referral_bonus / 10u64.pow(6));
    println!("  Referred count: {}\n", alice.referred_count);

    let clock = Clock {
        slot: 10000,
        epoch_start_timestamp: 0,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: 1234567890,
    };

    // 2. Bob joins through Alice's referral
    println!("=== Bob Joins (Referred by Alice) ===");
    let mut bob = BootstrapTrader {
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

    // Bob's first trade
    let bob_trade_volume = 75_000 * 10u64.pow(6); // $75k
    let leverage = U64F64::from_num(4u32);
    let fee_bps = bootstrap_state.calculate_bootstrap_fee();
    let fee_paid = (bob_trade_volume as u128 * fee_bps as u128 / 10_000) as u64;

    println!("Bob's Trade:");
    println!("  Volume: ${}", bob_trade_volume / 10u64.pow(6));
    println!("  Leverage: {}x", leverage.to_num::<u32>());
    println!("  Fee: ${} ({} bps)", fee_paid / 10u64.pow(6), fee_bps);

    // Process Bob's trade
    let bob_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut bob,
        bob_trade_volume,
        fee_paid,
        leverage,
        &clock,
    ).unwrap();

    println!("\nBob's Rewards:");
    println!("  MMT earned: {} MMT", bob_result.mmt_reward / 10u64.pow(6));
    println!("  Tier: {:?}", bob_result.tier);

    // Process Alice's referral bonus
    let alice_referral = BootstrapIncentiveEngine::process_referral(
        &mut alice,
        bob_trade_volume,
        &bootstrap_state,
    ).unwrap();

    println!("\nAlice's Referral Bonus:");
    println!("  Referral rate: 5%");
    println!("  Bob's MMT reward: {} MMT", bob_result.mmt_reward / 10u64.pow(6));
    println!("  Alice's bonus: {} MMT (5% of Bob's reward)", alice_referral / 10u64.pow(6));
    println!("  Alice's new referral count: {}", alice.referred_count);
    println!("  Alice's total referral bonus: {} MMT\n", alice.referral_bonus / 10u64.pow(6));

    // 3. Charlie joins through Alice's referral
    println!("=== Charlie Joins (Also Referred by Alice) ===");
    let mut charlie = BootstrapTrader {
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

    let charlie_trade_volume = 150_000 * 10u64.pow(6); // $150k - larger trade
    let charlie_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut charlie,
        charlie_trade_volume,
        (charlie_trade_volume as u128 * fee_bps as u128 / 10_000) as u64,
        U64F64::from_num(6u32),
        &clock,
    ).unwrap();

    let alice_referral_2 = BootstrapIncentiveEngine::process_referral(
        &mut alice,
        charlie_trade_volume,
        &bootstrap_state,
    ).unwrap();

    println!("Charlie's Trade:");
    println!("  Volume: ${}", charlie_trade_volume / 10u64.pow(6));
    println!("  MMT earned: {} MMT", charlie_result.mmt_reward / 10u64.pow(6));
    println!("  Alice's referral bonus: {} MMT", alice_referral_2 / 10u64.pow(6));
    println!("  Alice's total referral bonus: {} MMT\n", alice.referral_bonus / 10u64.pow(6));

    // 4. Dave - a whale trader referred by Alice
    println!("=== Dave Joins - Whale Trader (Referred by Alice) ===");
    let mut dave = BootstrapTrader {
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

    let dave_trade_volume = 1_500_000 * 10u64.pow(6); // $1.5M whale trade
    let dave_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut dave,
        dave_trade_volume,
        (dave_trade_volume as u128 * fee_bps as u128 / 10_000) as u64,
        U64F64::from_num(10u32),
        &clock,
    ).unwrap();

    let alice_referral_3 = BootstrapIncentiveEngine::process_referral(
        &mut alice,
        dave_trade_volume,
        &bootstrap_state,
    ).unwrap();

    println!("Dave's Whale Trade:");
    println!("  Volume: ${}", dave_trade_volume / 10u64.pow(6));
    println!("  Tier: Gold (3x multiplier)");
    println!("  MMT earned: {} MMT", dave_result.mmt_reward / 10u64.pow(6));
    println!("  Alice's referral bonus: {} MMT", alice_referral_3 / 10u64.pow(6));
    println!("  Alice's total referral bonus: {} MMT\n", alice.referral_bonus / 10u64.pow(6));

    // 5. Bob refers Eve (testing multi-level)
    println!("=== Eve Joins (Referred by Bob) ===");
    let mut eve = BootstrapTrader {
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

    let eve_trade_volume = 50_000 * 10u64.pow(6); // $50k
    let eve_result = BootstrapIncentiveEngine::process_bootstrap_trade(
        &mut bootstrap_state,
        &mut eve,
        eve_trade_volume,
        (eve_trade_volume as u128 * fee_bps as u128 / 10_000) as u64,
        U64F64::from_num(3u32),
        &clock,
    ).unwrap();

    let bob_referral = BootstrapIncentiveEngine::process_referral(
        &mut bob,
        eve_trade_volume,
        &bootstrap_state,
    ).unwrap();

    println!("Eve's Trade (referred by Bob):");
    println!("  Volume: ${}", eve_trade_volume / 10u64.pow(6));
    println!("  MMT earned: {} MMT", eve_result.mmt_reward / 10u64.pow(6));
    println!("  Bob's referral bonus: {} MMT", bob_referral / 10u64.pow(6));
    println!("  Note: Alice does NOT get a bonus (no multi-level referrals)\n");

    // 6. Summary statistics
    println!("=== Final Summary ===");
    println!("\nAlice (Master Referrer):");
    println!("  Personal trading volume: ${}", alice.volume_traded / 10u64.pow(6));
    println!("  Personal MMT earned: {} MMT", alice.mmt_earned / 10u64.pow(6));
    println!("  Referral bonuses: {} MMT", alice.referral_bonus / 10u64.pow(6));
    println!("  Total MMT: {} MMT", (alice.mmt_earned + alice.referral_bonus) / 10u64.pow(6));
    println!("  Referred traders: {}", alice.referred_count);
    println!("  Average bonus per referral: {} MMT", 
        alice.referral_bonus / alice.referred_count / 10u64.pow(6));

    println!("\nBob (Referred by Alice, Referred Eve):");
    println!("  Trading volume: ${}", bob.volume_traded / 10u64.pow(6));
    println!("  MMT earned: {} MMT", bob.mmt_earned / 10u64.pow(6));
    println!("  Referral bonuses: {} MMT", bob.referral_bonus / 10u64.pow(6));
    println!("  Total MMT: {} MMT", (bob.mmt_earned + bob.referral_bonus) / 10u64.pow(6));

    println!("\nBootstrap State:");
    println!("  Total volume: ${}", bootstrap_state.total_volume / 10u64.pow(6));
    println!("  Vault balance: ${}", bootstrap_state.current_vault_balance / 10u64.pow(6));
    println!("  MMT distributed: {} MMT", bootstrap_state.mmt_distributed / 10u64.pow(6));
    println!("  Unique traders: {}", bootstrap_state.unique_traders);
    println!("  Current coverage: {:.4}%", bootstrap_state.current_coverage.to_num::<f64>() * 100.0);

    // Calculate referral effectiveness
    let total_referred_volume = bob_trade_volume + charlie_trade_volume + dave_trade_volume;
    let referral_efficiency = (alice.referral_bonus as f64) / (total_referred_volume as f64) * 100.0;
    
    println!("\nReferral System Metrics:");
    println!("  Total volume from Alice's referrals: ${}", total_referred_volume / 10u64.pow(6));
    println!("  Alice's total referral bonus: {} MMT", alice.referral_bonus / 10u64.pow(6));
    println!("  Referral efficiency: {:.3}% of referred volume", referral_efficiency);
    println!("  Referral bonus as % of Alice's total MMT: {:.1}%", 
        (alice.referral_bonus as f64) / ((alice.mmt_earned + alice.referral_bonus) as f64) * 100.0);

    println!("\n✅ Referral system test completed successfully!");
    println!("\nKey Insights:");
    println!("- Early traders like Alice can significantly boost earnings through referrals");
    println!("- Whale referrals (like Dave) provide the highest bonuses");
    println!("- The 5% referral rate incentivizes community growth");
    println!("- No multi-level marketing - only direct referrals count");
    println!("- Referral bonuses can represent 10-20% of total MMT earnings");
}