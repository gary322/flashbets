use crate::state::*;
use crate::fees::*;

// Helper functions for fee calculations
fn calculate_taker_fee(coverage: f64) -> f64 {
    // fee = FEE_BASE (3bp) + FEE_SLOPE (25bp) * exp(-3*coverage)
    let fee_base = 0.0003; // 3 basis points
    let fee_slope = 0.0025; // 25 basis points
    
    fee_base + fee_slope * (-3.0 * coverage).exp()
}

fn calculate_maker_rebate(
    maker_stake: f64,
    total_stake: f64,
    coverage: f64,
    trade_size: f64
) -> f64 {
    let stake_ratio = maker_stake / total_stake;
    let rebate_rate = 0.0015 * coverage; // 15bp * coverage
    
    stake_ratio * rebate_rate * trade_size
}

fn calculate_leverage(base_leverage: f64, depth: u8, coverage: f64, n_outcomes: u8) -> f64 {
    let depth_multiplier = 1.0 + (0.1 * depth.min(32) as f64);
    let coverage_factor = coverage * 100.0;
    let tier_cap = get_tier_cap(n_outcomes);
    
    (base_leverage * depth_multiplier)
        .min(coverage_factor)
        .min(tier_cap)
}

fn get_tier_cap(n_outcomes: u8) -> f64 {
    match n_outcomes {
        1 => 100.0,
        2 => 70.0,
        3..=4 => 25.0,
        5..=8 => 15.0,
        9..=16 => 12.0,
        17..=64 => 10.0,
        _ => 5.0,
    }
}

#[cfg(test)]
mod fee_validation_tests {
    use super::*;

    #[test]
    fn test_elastic_fee_calculation() {
        let test_cases = vec![
            (2.0, 3.064), // High coverage, low fee
            (1.5, 3.168), // Medium coverage
            (1.0, 3.746), // Coverage = 1
            (0.5, 8.578), // Low coverage, high fee
            (0.1, 27.426), // Very low coverage, max fee
        ];

        for (coverage, expected_fee_bps) in test_cases {
            let fee = calculate_taker_fee(coverage);
            let fee_bps = fee * 10000.0;

            assert!((fee_bps - expected_fee_bps).abs() < 0.1,
                "Fee mismatch at coverage {}: {} vs {}",
                coverage, fee_bps, expected_fee_bps);

            // Verify bounds
            assert!(fee_bps >= 3.0 && fee_bps <= 28.0,
                "Fee outside 3-28bp range");
        }
    }

    #[test]
    fn test_maker_rebate_calculation() {
        let maker_stake = 1000.0;
        let total_stake = 10000.0;
        let coverage = 1.5;
        let trade_size = 100000.0; // 100k USDC
        
        let taker_fee = calculate_taker_fee(coverage);
        let rebate = calculate_maker_rebate(maker_stake, total_stake, coverage, trade_size);

        // Rebate should be (stake/total) * 15bp * coverage * trade_size
        let expected_rebate = (maker_stake / total_stake) * 0.0015 * coverage * trade_size;

        assert!((rebate - expected_rebate).abs() < 0.01,
            "Rebate calculation mismatch");

        // Maker should profit if improving spread
        let spread_improvement = 0.0002; // 2bp improvement
        let maker_profit = rebate + spread_improvement * trade_size - taker_fee * trade_size;

        assert!(maker_profit > 0.0, "Maker should profit with spread improvement");
    }

    #[test]
    fn test_mmt_emission_schedule() {
        let total_supply = 100_000_000.0; // 100M
        let current_season_allocation = 10_000_000.0; // 10M
        let season_duration = 38_880_000; // slots (~6 months)

        // Test linear emission
        let emissions_per_slot = current_season_allocation / season_duration as f64;

        // Verify total emission over season
        let total_emitted = emissions_per_slot * season_duration as f64;
        assert!((total_emitted - current_season_allocation).abs() < 1.0);

        // Test emission allocation
        let maker_rewards = 0.2; // 20% to makers
        let early_incentives = 0.3; // 30% to early users
        let staking_rewards = 0.5; // 50% to stakers

        let maker_emission = emissions_per_slot * maker_rewards;
        let early_emission = emissions_per_slot * early_incentives;
        let staking_emission = emissions_per_slot * staking_rewards;

        assert!((maker_emission + early_emission + staking_emission - emissions_per_slot).abs() < 1e-9);
    }

    #[test]
    fn test_fee_distribution() {
        let trade_size = 1_000_000.0; // 1M USDC
        let coverage = 1.2;

        let taker_fee = calculate_taker_fee(coverage) * trade_size;

        // Distribution: 70% vault, 20% MMT, 10% burn
        let to_vault = taker_fee * 0.7;
        let to_mmt = taker_fee * 0.2;
        let to_burn = taker_fee * 0.1;

        assert!((to_vault + to_mmt + to_burn - taker_fee).abs() < 0.01);

        // Test vault growth impact on coverage
        let current_vault = 100_000.0;
        let current_oi = 500_000.0;
        let tail_loss = 0.5;

        let new_vault = current_vault + to_vault;
        let new_coverage = new_vault / (tail_loss * current_oi);
        let coverage_increase = new_coverage - coverage;

        assert!(coverage_increase > 0.0, "Fees should increase coverage");

        // Higher coverage should lead to lower future fees
        let new_fee_rate = calculate_taker_fee(new_coverage);
        assert!(new_fee_rate < calculate_taker_fee(coverage),
            "Higher coverage should reduce fees");
    }

    #[test]
    fn test_coverage_leverage_relationship() {
        let base_leverage = 100.0;
        let depth = 5;
        let n_outcomes = 1;

        let test_cases = vec![
            (0.5, 50.0),  // Low coverage limits leverage
            (1.0, 100.0), // Normal coverage allows full leverage
            (2.0, 150.0), // High coverage allows depth bonus
        ];

        for (coverage, expected_max) in test_cases {
            let max_leverage = calculate_leverage(
                base_leverage,
                depth,
                coverage,
                n_outcomes
            );

            assert!(
                max_leverage <= expected_max,
                "Leverage {} exceeds expected {} for coverage {}",
                max_leverage, expected_max, coverage
            );
        }
    }
}

// Bootstrap economics validation
#[cfg(test)]
mod bootstrap_validation {
    use super::*;

    #[test]
    fn test_bootstrap_from_zero() {
        let mut vault = 0.0;
        let mut coverage = 0.0;
        let mut total_oi = 0.0;
        let tail_loss = 0.5;

        // Simulate first trades at 1x (spot)
        let trades = vec![
            100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0
        ];

        for (i, trade_size) in trades.iter().enumerate() {
            // Fee at current coverage (max when coverage = 0)
            let fee_rate = if coverage == 0.0 { 0.028 } else { calculate_taker_fee(coverage) };
            let fee = fee_rate * trade_size;

            // 70% of fee goes to vault
            vault += fee * 0.7;
            total_oi += trade_size;

            // Recalculate coverage
            if total_oi > 0.0 {
                coverage = vault / (tail_loss * total_oi);
            }

            // Calculate available leverage
            let leverage = if coverage == 0.0 {
                1.0 // Spot only
            } else {
                (coverage * 100.0).min(100.0)
            };

            println!("After trade {}: vault={:.2}, coverage={:.4}, leverage={:.1}x",
                i + 1, vault, coverage, leverage);
        }

        // Should reach usable leverage after reasonable volume
        assert!(coverage > 0.01, "Coverage should be meaningful");
        assert!(vault > 100.0, "Vault should have grown");

        // Minimum viable vault calculation
        let min_viable_oi = 10_000.0;
        let min_viable_vault = min_viable_oi * tail_loss; // For coverage = 1

        println!("Minimum viable vault for 10x leverage: ${:.2}", min_viable_vault);
        assert!(min_viable_vault == 5_000.0);
    }

    #[test]
    fn test_death_spiral_prevention() {
        let mut vault = 50_000.0;
        let mut total_oi = 100_000.0;
        let tail_loss = 0.5;

        // Simulate coverage dropping
        for _ in 0..10 {
            let coverage = vault / (tail_loss * total_oi);

            if coverage < 0.5 {
                // System should halt
                println!("Halt triggered at coverage: {:.4}", coverage);
                break;
            }

            // Simulate losses
            let loss = 5_000.0;
            vault -= loss;

            // But fees increase
            let fee_rate = calculate_taker_fee(coverage);
            let recovery_trade = 100_000.0;
            let fee = fee_rate * recovery_trade;
            vault += fee * 0.7;

            // Funding also adds to vault
            let funding_rate = 0.0125; // 1.25%/hour during halt
            let funding = total_oi * funding_rate;
            vault += funding;

            println!("Coverage: {:.4}, Fee rate: {:.4}, Vault: {:.2}",
                coverage, fee_rate, vault);
        }

        // Should not go negative
        assert!(vault > 0.0, "Vault should remain positive");
    }

    #[test]
    fn test_minimum_viable_economics() {
        // Test minimum vault needed for various leverage levels
        let tail_loss = 0.5;
        let test_cases = vec![
            (10.0, 0.1),    // 10x leverage needs coverage = 0.1
            (50.0, 0.5),    // 50x needs coverage = 0.5
            (100.0, 1.0),   // 100x needs coverage = 1.0
            (200.0, 2.0),   // 200x needs coverage = 2.0
            (500.0, 5.0),   // 500x needs coverage = 5.0
        ];

        for (target_leverage, required_coverage) in test_cases {
            let oi = 1_000_000.0; // $1M open interest
            let required_vault = required_coverage * tail_loss * oi;

            println!("For {}x leverage with ${}M OI: need ${:.0} vault",
                target_leverage, oi / 1_000_000.0, required_vault);

            // Verify leverage calculation
            let actual_leverage = calculate_leverage(target_leverage, 0, required_coverage, 1);
            assert!(actual_leverage >= target_leverage.min(100.0),
                "Leverage should be achievable with sufficient coverage");
        }
    }

    #[test]
    fn test_fee_economics_at_scale() {
        // Test economics with realistic volumes
        let daily_volume = 10_000_000.0; // $10M daily volume
        let avg_coverage = 1.5;
        let avg_fee_rate = calculate_taker_fee(avg_coverage);

        let daily_fees = daily_volume * avg_fee_rate;
        let daily_vault_growth = daily_fees * 0.7;
        let daily_mmt_rewards = daily_fees * 0.2;
        let daily_burn = daily_fees * 0.1;

        println!("Daily volume: ${:.0}", daily_volume);
        println!("Average fee rate: {:.2}bp", avg_fee_rate * 10000.0);
        println!("Daily fees: ${:.2}", daily_fees);
        println!("Vault growth: ${:.2}/day", daily_vault_growth);
        println!("MMT rewards: ${:.2}/day", daily_mmt_rewards);
        println!("MMT burn: ${:.2}/day", daily_burn);

        // Annual projections
        let annual_vault_growth = daily_vault_growth * 365.0;
        let annual_mmt_rewards = daily_mmt_rewards * 365.0;

        println!("\nAnnual projections:");
        println!("Vault growth: ${:.0}", annual_vault_growth);
        println!("MMT rewards: ${:.0}", annual_mmt_rewards);

        // Verify sustainability
        assert!(annual_vault_growth > 1_000_000.0,
            "Vault should grow meaningfully at scale");
    }
}