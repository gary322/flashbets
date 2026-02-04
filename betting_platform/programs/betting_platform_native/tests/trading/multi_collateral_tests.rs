use betting_platform_native::trading::multi_collateral::*;
use solana_program::pubkey::Pubkey;

#[test]
fn test_collateral_type_values() {
    // Test USDC value calculation
    let usdc_amount = 1_000_000; // 1 USDC
    let usdc_oracle_price = 100_000_000; // $1.00 with 8 decimals
    let usdc_value = CollateralType::USDC.get_usd_value(usdc_amount, usdc_oracle_price).unwrap();
    assert_eq!(usdc_value, 1); // $1 in USDC decimals after all conversions
    
    // Test SOL value calculation (assuming $100/SOL)
    let sol_amount = 1_000_000_000; // 1 SOL
    let sol_oracle_price = 10_000_000_000; // $100.00 with 8 decimals
    let sol_value = CollateralType::SOL.get_usd_value(sol_amount, sol_oracle_price).unwrap();
    assert_eq!(sol_value, 100); // $100 in USDC decimals after all conversions
    
    // Test WBTC value calculation (assuming $50,000/BTC)
    let wbtc_amount = 100_000_000; // 1 WBTC
    let wbtc_oracle_price = 5_000_000_000_000; // $50,000.00 with 8 decimals
    let wbtc_value = CollateralType::WBTC.get_usd_value(wbtc_amount, wbtc_oracle_price).unwrap();
    assert_eq!(wbtc_value, 50_000); // $50,000 in USDC decimals after all conversions
}

#[test]
fn test_ltv_ratios() {
    assert_eq!(CollateralType::USDC.ltv_ratio(), 100);
    assert_eq!(CollateralType::USDT.ltv_ratio(), 100);
    assert_eq!(CollateralType::SOL.ltv_ratio(), 80);
    assert_eq!(CollateralType::WBTC.ltv_ratio(), 80);
    assert_eq!(CollateralType::WETH.ltv_ratio(), 80);
}

#[test]
fn test_borrowing_power_calculation() {
    let vault = MultiCollateralVault {
        discriminator: MultiCollateralVault::DISCRIMINATOR,
        usdc_deposits: 10_000_000_000, // 10,000 USDC
        usdt_deposits: 5_000_000_000,  // 5,000 USDT
        sol_deposits: 100_000_000_000, // 100 SOL
        wbtc_deposits: 20_000_000,     // 0.2 WBTC
        weth_deposits: 300_000_000,    // 3 WETH
        total_usd_value: 0,
        total_borrowed_usd: 0,
        depositor_count: 1,
        last_update: 0,
        bump: 0,
    };
    
    // Oracle prices
    let usdc_price = 100_000_000;        // $1.00
    let usdt_price = 100_000_000;        // $1.00
    let sol_price = 10_000_000_000;      // $100.00
    let wbtc_price = 5_000_000_000_000;  // $50,000.00
    let weth_price = 200_000_000_000;    // $2,000.00
    
    let borrowing_power = calculate_borrowing_power(
        &vault,
        usdc_price,
        usdt_price,
        sol_price,
        wbtc_price,
        weth_price,
    ).unwrap();
    
    // Expected: 10,000 + 5,000 + (100 * 100 * 0.8) + (0.2 * 50,000 * 0.8) + (3 * 2,000 * 0.8)
    // = 10,000 + 5,000 + 8,000 + 8,000 + 4,800 = 35,800
    assert_eq!(borrowing_power, 35_800);
}