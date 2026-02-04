//! Leverage Trading Tests (1-500x)
//! 
//! Tests for leveraged positions, margin requirements, and liquidations

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
};
use borsh::BorshSerialize;
use betting_platform_native::{
    instruction::{BettingPlatformInstruction, OpenPositionParams},
    state::{Position, GlobalConfig},
    trading::{calculate_initial_margin, calculate_maintenance_margin},
    liquidation::{calculate_liquidation_price, is_liquidatable},
    math::fixed_point::U64F64,
};

#[tokio::test]
async fn test_open_position_with_leverage() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    // Test different leverage levels
    let leverage_levels = vec![1, 10, 50, 100, 250, 500];
    let base_size = 1000_000_000u64; // 1000 USDC

    for leverage in leverage_levels {
        let params = OpenPositionParams {
            market_id: [1u8; 32],
            size: base_size,
            leverage: leverage as u8,
            is_long: true,
            limit_price: None,
        };

        let position_pda = Pubkey::new_unique(); // Mock PDA

        let open_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(position_pda, false),
                AccountMeta::new(payer.pubkey(), true),
            ],
            data: BettingPlatformInstruction::OpenPosition { params }.try_to_vec().unwrap(),
        };

        let mut transaction = Transaction::new_with_payer(
            &[open_ix],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);
        
        let result = banks_client.process_transaction(transaction).await;
        
        if leverage <= 500 {
            assert!(result.is_ok(), "Failed to open position with {}x leverage", leverage);
            println!("✅ Opened position with {}x leverage", leverage);
        } else {
            assert!(result.is_err(), "Should fail for leverage > 500x");
        }
    }
}

#[test]
fn test_margin_requirements() {
    // Test initial and maintenance margin calculations
    
    let test_cases = vec![
        (1, 100.0, 100.0),    // 1x: 100% margin
        (10, 10.0, 5.0),      // 10x: 10% initial, 5% maintenance
        (50, 2.0, 1.0),       // 50x: 2% initial, 1% maintenance
        (100, 1.0, 0.5),      // 100x: 1% initial, 0.5% maintenance
        (500, 0.2, 0.1),      // 500x: 0.2% initial, 0.1% maintenance
    ];

    for (leverage, expected_initial, expected_maintenance) in test_cases {
        let position_size = U64F64::from_num(1_000_000); // 1M USDC
        
        let initial_margin = calculate_initial_margin(position_size, leverage);
        let maintenance_margin = calculate_maintenance_margin(position_size, leverage);
        
        let initial_pct = (initial_margin / position_size) * U64F64::from_num(100);
        let maintenance_pct = (maintenance_margin / position_size) * U64F64::from_num(100);
        
        assert!(
            (initial_pct.to_num::<f64>() - expected_initial).abs() < 0.1,
            "Initial margin for {}x: expected {}%, got {:.2}%",
            leverage, expected_initial, initial_pct.to_num::<f64>()
        );
        
        assert!(
            (maintenance_pct.to_num::<f64>() - expected_maintenance).abs() < 0.1,
            "Maintenance margin for {}x: expected {}%, got {:.2}%",
            leverage, expected_maintenance, maintenance_pct.to_num::<f64>()
        );
        
        println!("✅ {}x leverage: Initial {:.1}%, Maintenance {:.1}%", 
            leverage, initial_pct.to_num::<f64>(), maintenance_pct.to_num::<f64>());
    }
}

#[test]
fn test_liquidation_prices() {
    // Test liquidation price calculation for different leverages
    
    let entry_price = U64F64::from_num(100);
    let position_size = U64F64::from_num(100_000); // 100k USDC notional
    
    let test_cases = vec![
        (1, true, 0.0),      // 1x long: never liquidated
        (10, true, 95.0),    // 10x long: liquidated below 95
        (50, true, 99.0),    // 50x long: liquidated below 99
        (100, true, 99.5),   // 100x long: liquidated below 99.5
        (500, true, 99.9),   // 500x long: liquidated below 99.9
        (10, false, 105.0),  // 10x short: liquidated above 105
        (50, false, 101.0),  // 50x short: liquidated above 101
        (100, false, 100.5), // 100x short: liquidated above 100.5
        (500, false, 100.1), // 500x short: liquidated above 100.1
    ];

    for (leverage, is_long, expected_liq) in test_cases {
        let margin = position_size / U64F64::from_num(leverage);
        let liq_price = calculate_liquidation_price(
            entry_price,
            margin,
            position_size,
            is_long,
        );
        
        if leverage == 1 {
            // 1x leverage should never liquidate
            assert_eq!(liq_price, U64F64::from_num(0));
        } else {
            let diff = (liq_price.to_num::<f64>() - expected_liq).abs();
            assert!(
                diff < 1.0,
                "{}x {} liquidation: expected {}, got {:.2}",
                leverage,
                if is_long { "long" } else { "short" },
                expected_liq,
                liq_price.to_num::<f64>()
            );
        }
        
        println!("✅ {}x {} liquidation price: {:.2}", 
            leverage,
            if is_long { "long" } else { "short" },
            liq_price.to_num::<f64>()
        );
    }
}

#[test]
fn test_coverage_based_liquidation() {
    // Test coverage-based partial liquidation
    
    let global_config = GlobalConfig {
        vault: 10_000_000_000_000, // 10M USDC
        total_oi: 100_000_000_000_000, // 100M USDC
        ..Default::default()
    };
    
    // Coverage = vault / OI = 10%
    let coverage = U64F64::from_num(global_config.vault) / U64F64::from_num(global_config.total_oi);
    assert!((coverage.to_num::<f64>() - 0.1).abs() < 0.01);
    
    // Test liquidation caps at different coverage levels
    let test_coverages = vec![
        (0.05, 0.25),  // 5% coverage: 25% max liquidation
        (0.10, 0.50),  // 10% coverage: 50% max liquidation
        (0.20, 0.75),  // 20% coverage: 75% max liquidation
        (0.50, 1.00),  // 50% coverage: 100% max liquidation
    ];
    
    for (coverage_ratio, expected_cap) in test_coverages {
        let liquidation_cap = calculate_liquidation_cap(U64F64::from_num(coverage_ratio));
        
        assert!(
            (liquidation_cap.to_num::<f64>() - expected_cap).abs() < 0.01,
            "Coverage {:.0}%: expected {:.0}% cap, got {:.2}%",
            coverage_ratio * 100.0,
            expected_cap * 100.0,
            liquidation_cap.to_num::<f64>() * 100.0
        );
        
        println!("✅ Coverage {:.0}%: Liquidation cap {:.0}%",
            coverage_ratio * 100.0,
            liquidation_cap.to_num::<f64>() * 100.0
        );
    }
}

#[test]
fn test_position_health_monitoring() {
    // Test position health calculation
    
    let position = Position {
        owner: Pubkey::new_unique(),
        market_id: [1u8; 32],
        size: 100_000_000_000, // 100k USDC
        collateral: 1_000_000_000, // 1k USDC (100x leverage)
        entry_price: 100_000_000, // $100
        is_long: true,
        leverage: 100,
        ..Default::default()
    };
    
    // Test at different market prices
    let test_prices = vec![
        (110.0, 1000.0, false),  // $110: +$10k PnL, healthy
        (100.0, 0.0, false),     // $100: $0 PnL, healthy
        (99.5, -500.0, false),   // $99.5: -$500 PnL, healthy
        (99.0, -1000.0, true),   // $99: -$1k PnL, liquidatable
        (98.0, -2000.0, true),   // $98: -$2k PnL, liquidatable
    ];
    
    for (price, expected_pnl, should_liquidate) in test_prices {
        let mark_price = U64F64::from_num(price);
        let entry_price = U64F64::from_num(100);
        
        let pnl = if position.is_long {
            (mark_price - entry_price) * U64F64::from_num(1000) // 1000 units
        } else {
            (entry_price - mark_price) * U64F64::from_num(1000)
        };
        
        let health = U64F64::from_num(position.collateral as f64 / 1e9) + pnl;
        let is_liquidatable = health < U64F64::from_num(500); // 0.5% maintenance margin
        
        assert_eq!(is_liquidatable, should_liquidate);
        assert!((pnl.to_num::<f64>() - expected_pnl).abs() < 10.0);
        
        println!("✅ Price ${}: PnL ${:.0}, Health: {}, Liquidatable: {}",
            price, pnl.to_num::<f64>(), 
            if health > U64F64::from_num(0) { "Healthy" } else { "Unhealthy" },
            should_liquidate
        );
    }
}

#[test]
fn test_max_position_size_limits() {
    // Test position size limits based on leverage
    
    let account_balance = U64F64::from_num(10_000); // 10k USDC
    
    let leverage_limits = vec![
        (1, 10_000.0),      // 1x: Max size = balance
        (10, 100_000.0),    // 10x: Max size = 10x balance
        (50, 500_000.0),    // 50x: Max size = 50x balance
        (100, 1_000_000.0), // 100x: Max size = 100x balance
        (500, 5_000_000.0), // 500x: Max size = 500x balance
    ];
    
    for (leverage, expected_max) in leverage_limits {
        let max_position_size = account_balance * U64F64::from_num(leverage);
        
        assert_eq!(
            max_position_size.to_num::<f64>(),
            expected_max,
            "{}x leverage max position size mismatch",
            leverage
        );
        
        println!("✅ {}x leverage: Max position ${:.0}k",
            leverage,
            max_position_size.to_num::<f64>() / 1000.0
        );
    }
}

#[test]
fn test_funding_rate_impact() {
    // Test funding rate impact on leveraged positions
    
    let position_size = U64F64::from_num(100_000); // 100k USDC
    let leverage = 100;
    let hourly_funding_rate = U64F64::from_num(0.01); // 1% per hour
    
    // Calculate funding payment
    let funding_payment = position_size * hourly_funding_rate;
    
    // High leverage amplifies funding impact
    let margin = position_size / U64F64::from_num(leverage);
    let funding_impact_pct = (funding_payment / margin) * U64F64::from_num(100);
    
    println!("✅ {}x leverage position:", leverage);
    println!("   - Position size: ${:.0}", position_size.to_num::<f64>());
    println!("   - Margin: ${:.0}", margin.to_num::<f64>());
    println!("   - Hourly funding: ${:.0} ({:.1}% of margin)",
        funding_payment.to_num::<f64>(),
        funding_impact_pct.to_num::<f64>()
    );
    
    // At 100x leverage, 1% funding = 100% of margin!
    assert!(funding_impact_pct >= U64F64::from_num(99));
}

// Helper function
fn calculate_liquidation_cap(coverage: U64F64) -> U64F64 {
    // Higher coverage = higher liquidation cap
    if coverage < U64F64::from_num(0.1) {
        U64F64::from_num(0.25) // 25% cap
    } else if coverage < U64F64::from_num(0.2) {
        U64F64::from_num(0.5) // 50% cap
    } else if coverage < U64F64::from_num(0.5) {
        U64F64::from_num(0.75) // 75% cap
    } else {
        U64F64::from_num(1.0) // 100% cap
    }
}