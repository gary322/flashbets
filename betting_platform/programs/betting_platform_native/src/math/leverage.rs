//! Leverage calculations for the betting platform
//! Implements both base and effective leverage formulas from specification

use solana_program::msg;

/// Calculate maximum leverage based on specification formula:
/// lev_max = min(100 × (1 + 0.1 × depth), coverage × 100/√N, tier_cap(N))
pub fn calculate_max_leverage(depth: u64, coverage: u64, outcome_count: u64) -> u64 {
    // Base leverage with depth boost
    let depth_multiplier = 10000 + (1000 * depth); // 1 + 0.1 * depth in basis points
    let base_with_depth = (100 * depth_multiplier) / 10000;
    
    // Coverage-based limit
    let sqrt_n = integer_sqrt(outcome_count);
    let coverage_limit = if sqrt_n > 0 {
        (coverage * 100) / sqrt_n
    } else {
        100
    };
    
    // Tier caps based on outcome count
    let tier_cap = get_tier_cap(outcome_count);
    
    // Return minimum of all three
    let result = base_with_depth.min(coverage_limit).min(tier_cap);
    
    msg!(
        "Leverage calc: depth={}, coverage={}, N={}, result={}",
        depth, coverage, outcome_count, result
    );
    
    result
}

/// Calculate bootstrap leverage based on formula: min(100*coverage, tier)
/// This is used during initial chain setup when coverage may be low
pub fn calculate_bootstrap_leverage(coverage: u64, tier_cap: u64) -> u64 {
    let coverage_leverage = coverage.saturating_mul(100);
    coverage_leverage.min(tier_cap)
}

/// Calculate effective leverage through chaining
/// Formula: lev_eff = lev_base × ∏(1 + r_i)
pub fn calculate_effective_leverage(base_leverage: u64, multiplier_bps: u64) -> u64 {
    base_leverage
        .saturating_mul(multiplier_bps)
        .saturating_div(10000)
        .min(500) // Cap at 500x as per spec
}

/// Get tier cap based on number of outcomes
/// Exact tiers from specification: N=1: 100x, N=2: 70x, N=3-4: 25x, 
/// N=5-8: 15x, N=9-16: 12x, N=17-64: 10x, N>64: 5x
fn get_tier_cap(outcome_count: u64) -> u64 {
    match outcome_count {
        1 => 100,     // Binary: 100x max
        2 => 70,      // 2 outcomes: 70x (100/√2 ≈ 70.7)
        3..=4 => 25,  // 3-4 outcomes: 25x max
        5..=8 => 15,  // 5-8 outcomes: 15x max
        9..=16 => 12, // 9-16 outcomes: 12x max
        17..=64 => 10, // 17-64 outcomes: 10x max
        _ => 5,       // >64 outcomes: 5x max
    }
}

/// Integer square root for leverage calculations
fn integer_sqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_max_leverage() {
        // Test base case: depth=0, coverage=1, binary
        assert_eq!(calculate_max_leverage(0, 100, 1), 100);
        
        // Test with depth boost: depth=5 should give +50% boost
        assert_eq!(calculate_max_leverage(5, 100, 1), 100); // Still capped at 100
        
        // Test coverage limit
        assert_eq!(calculate_max_leverage(0, 50, 1), 50);
        
        // Test tier caps for multi-outcome
        assert_eq!(calculate_max_leverage(0, 100, 4), 25); // 4 outcomes = 25x cap
    }

    #[test]
    fn test_calculate_effective_leverage() {
        // Test 100x base with 1.5x multiplier
        assert_eq!(calculate_effective_leverage(100, 15000), 150);
        
        // Test cap at 500x
        assert_eq!(calculate_effective_leverage(400, 20000), 500);
    }

    #[test]
    fn test_integer_sqrt() {
        assert_eq!(integer_sqrt(0), 0);
        assert_eq!(integer_sqrt(1), 1);
        assert_eq!(integer_sqrt(4), 2);
        assert_eq!(integer_sqrt(9), 3);
        assert_eq!(integer_sqrt(16), 4);
        assert_eq!(integer_sqrt(100), 10);
    }
}