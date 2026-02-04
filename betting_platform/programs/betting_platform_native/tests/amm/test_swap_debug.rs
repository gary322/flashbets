use betting_platform_native::amm::pmamm::math::*;
use betting_platform_native::state::amm_accounts::{PMAMMPool, MarketState};
use solana_program::pubkey::Pubkey;

#[test]
fn test_swap_debug() {
    println!("Testing PM-AMM swap calculation...");
    
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
    
    println!("Pool reserves: {:?}", pool.reserves);
    println!("Fee: {} bps", pool.fee_bps);
    
    // Test swap 100 of outcome 0 for outcome 1
    match calculate_swap_output(&pool, 0, 1, 100) {
        Ok((output, fee)) => {
            println!("\nSwap 100 of outcome 0 for outcome 1:");
            println!("  Output: {}", output);
            println!("  Fee: {}", fee);
            
            // Calculate new reserves
            let new_reserve_0 = pool.reserves[0] + 100;
            let new_reserve_1 = pool.reserves[1] - output;
            println!("\nNew reserves would be:");
            println!("  Outcome 0: {} -> {}", pool.reserves[0], new_reserve_0);
            println!("  Outcome 1: {} -> {}", pool.reserves[1], new_reserve_1);
            
            // Check constant product
            let old_product = pool.reserves[0] * pool.reserves[1];
            let new_product = new_reserve_0 * new_reserve_1;
            println!("\nConstant product check:");
            println!("  Old: {} * {} = {}", pool.reserves[0], pool.reserves[1], old_product);
            println!("  New: {} * {} = {}", new_reserve_0, new_reserve_1, new_product);
        },
        Err(e) => println!("Swap failed: {:?}", e),
    }
}