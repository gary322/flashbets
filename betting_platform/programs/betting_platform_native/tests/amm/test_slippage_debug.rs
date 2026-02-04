use betting_platform_native::amm::pmamm::math::*;
use betting_platform_native::state::amm_accounts::{PMAMMPool, MarketState};
use solana_program::pubkey::Pubkey;

#[test]
fn test_slippage_debug() {
    println!("Testing slippage calculation...");
    
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
    println!("Testing slippage for order_size=10, tau=1000 (0.1)");
    
    match calculate_slippage_pmamm(&pool, 0, 1, 10, 1000) {
        Ok(slippage) => {
            println!("Slippage (delta): {}", slippage);
            
            // Calculate LMSR equivalent for comparison
            let lmsr_slippage = 115; // 11.5 scaled by 10
            let reduction = ((lmsr_slippage - slippage * 10) * 100) / lmsr_slippage;
            println!("LMSR equivalent: {}", lmsr_slippage / 10);
            println!("Reduction vs LMSR: {}%", reduction);
        },
        Err(e) => println!("Slippage calculation failed: {:?}", e),
    }
    
    // Test intermediate values
    println!("\nDebug intermediate calculations:");
    let reserve_in = pool.reserves[0];
    let reserve_out = pool.reserves[1];
    println!("reserve_in: {}, reserve_out: {}", reserve_in, reserve_out);
    
    // Geometric mean
    let liquidity = ((reserve_in as u64) * (reserve_out as u64)).isqrt();
    println!("Liquidity (geometric mean): {}", liquidity);
    
    // LVR * tau
    let lvr_bps = 500u64; // 5%
    let tau = 1000u64; // 0.1
    let lvr_tau = (lvr_bps * tau) / (10000 * 10000);
    println!("LVR * tau: {} / 100000000 = {}", lvr_bps * tau, lvr_tau);
}