use betting_platform_native::amm::pmamm::math::*;
use betting_platform_native::state::amm_accounts::{PMAMMPool, MarketState};
use solana_program::pubkey::Pubkey;

#[test]
fn test_pmamm_debug() {
    println!("Testing PM-AMM calculations...");
    
    let pool = PMAMMPool {
        discriminator: [112, 78, 45, 209, 156, 34, 89, 167],
        market_id: 1,
        pool_id: 1,
        l_parameter: 6000,
        expiry_time: 1735689600,
        num_outcomes: 3,
        reserves: vec![1000, 2000, 3000],
        total_liquidity: 6000,
        total_lp_supply: 1000000,
        liquidity_providers: 1,
        state: MarketState::Active,
        initial_price: 5000,
        probabilities: vec![3333, 3333, 3334],
        fee_bps: 30,
        oracle: Pubkey::new_unique(),
        total_volume: 0,
        created_at: 1704067200,
        last_update: 1704067200,
    };
    
    // Test invariant calculation
    println!("\nTesting calculate_invariant:");
    match calculate_invariant(&pool.reserves) {
        Ok(invariant) => {
            println!("Invariant = {:?}", invariant);
            println!("Invariant.to_num() = {}", invariant.to_num());
            
            // Recalculate by calling again
            match calculate_invariant(&pool.reserves) {
                Ok(recalc) => {
                    println!("Recalculated = {:?}", recalc);
                    println!("Recalculated.to_num() = {}", recalc.to_num());
                    
                    let diff = if invariant > recalc {
                        invariant.saturating_sub(recalc)
                    } else {
                        recalc.saturating_sub(invariant)
                    };
                    println!("Difference = {:?}", diff);
                    println!("Difference.to_num() = {}", diff.to_num());
                },
                Err(e) => println!("Recalculation failed: {:?}", e),
            }
        },
        Err(e) => println!("Invariant calculation failed: {:?}", e),
    }
    
    // Test probabilities
    println!("\nTesting calculate_probabilities:");
    
    // Debug the calculation step by step
    println!("Reserves: {:?}", pool.reserves);
    
    // Calculate 1/reserve for each
    for (i, &reserve) in pool.reserves.iter().enumerate() {
        let one = betting_platform_native::math::U128F128::from_num(1u128);
        let reserve_val = betting_platform_native::math::U128F128::from_num(reserve as u128);
        match one.checked_div(reserve_val) {
            Some(inv) => {
                println!("1/{} = {:?}", reserve, inv);
                println!("  to_num() = {}", inv.to_num());
            },
            None => println!("Division 1/{} failed", reserve),
        }
    }
    
    match calculate_probabilities(&pool) {
        Ok(probs) => {
            println!("Final probabilities = {:?}", probs);
            let sum: u64 = probs.iter().sum();
            println!("Sum = {}", sum);
        },
        Err(e) => println!("Probability calculation failed: {:?}", e),
    }
}