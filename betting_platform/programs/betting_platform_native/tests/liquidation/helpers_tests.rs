use betting_platform_native::liquidation::helpers::*;
use betting_platform_native::math::U64F64;

#[test]
fn test_calculate_liquidation_amount() {
    let position_size = 1_000_000_000; // $1000
    let coverage = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
    
    let amount = calculate_liquidation_amount(position_size, coverage).unwrap();
    
    // Should be ~16.67% (8.33% * 2)
    assert!(amount > 150_000_000 && amount < 200_000_000);
}

#[test]
fn test_calculate_keeper_reward() {
    let liquidation_amount = 100_000_000; // $100
    let base_reward_bps = 50; // 0.5%
    
    let reward = calculate_keeper_reward(liquidation_amount, base_reward_bps).unwrap();
    
    // Should be at least minimum reward
    assert!(reward >= 1_000_000);
}

#[test]
fn test_liquidation_priority() {
    let margin_ratio = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
    let position_size = 10_000_000_000; // $10k
    let time_since_warning = 300; // 5 minutes
    
    let priority = calculate_liquidation_priority(margin_ratio, position_size, time_since_warning);
    
    // Should have high priority due to low margin ratio
    assert!(priority > 100);
}