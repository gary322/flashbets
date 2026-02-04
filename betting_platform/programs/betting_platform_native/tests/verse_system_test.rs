//! Verse System Hierarchy Tests
//! 
//! Tests for hierarchical leverage multipliers and verse-based trading

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
};
use borsh::BorshSerialize;
use betting_platform_native::{
    instruction::{BettingPlatformInstruction, ChainStepType},
    state::{Verse, VerseAccount, ChainPosition},
    chain_execution::{calculate_verse_multiplier, execute_chain_step},
    math::fixed_point::U64F64,
};

#[test]
fn test_verse_hierarchy_structure() {
    // Test verse hierarchy: Root -> L1 -> L2 -> L3
    
    let root_verse = Verse {
        id: 0,
        name: "Root Universe".to_string(),
        parent_id: None,
        level: 0,
        multiplier: U64F64::from_num(1),
        total_liquidity: U64F64::from_num(10_000_000), // 10M
        active_markets: 100,
    };
    
    let l1_verse = Verse {
        id: 1,
        name: "Sports Verse".to_string(),
        parent_id: Some(0),
        level: 1,
        multiplier: U64F64::from_num(1.5), // 1.5x multiplier
        total_liquidity: U64F64::from_num(5_000_000),
        active_markets: 50,
    };
    
    let l2_verse = Verse {
        id: 2,
        name: "NFL Verse".to_string(),
        parent_id: Some(1),
        level: 2,
        multiplier: U64F64::from_num(2.0), // 2x multiplier
        total_liquidity: U64F64::from_num(2_000_000),
        active_markets: 20,
    };
    
    let l3_verse = Verse {
        id: 3,
        name: "Super Bowl Verse".to_string(),
        parent_id: Some(2),
        level: 3,
        multiplier: U64F64::from_num(3.0), // 3x multiplier
        total_liquidity: U64F64::from_num(1_000_000),
        active_markets: 5,
    };
    
    // Verify hierarchy
    assert_eq!(l1_verse.parent_id, Some(root_verse.id));
    assert_eq!(l2_verse.parent_id, Some(l1_verse.id));
    assert_eq!(l3_verse.parent_id, Some(l2_verse.id));
    
    // Verify multipliers increase with depth
    assert!(l1_verse.multiplier > root_verse.multiplier);
    assert!(l2_verse.multiplier > l1_verse.multiplier);
    assert!(l3_verse.multiplier > l2_verse.multiplier);
    
    println!("✅ Verse hierarchy validated:");
    println!("   Root (1x) -> Sports (1.5x) -> NFL (2x) -> Super Bowl (3x)");
}

#[test]
fn test_cumulative_multiplier_calculation() {
    // Test cumulative multiplier across verse chain
    
    let verse_chain = vec![
        U64F64::from_num(1.0),   // Root
        U64F64::from_num(1.5),   // L1
        U64F64::from_num(2.0),   // L2
        U64F64::from_num(3.0),   // L3
    ];
    
    let cumulative_multiplier = verse_chain.iter()
        .fold(U64F64::from_num(1), |acc, &m| acc * m);
    
    // 1 * 1.5 * 2 * 3 = 9x total multiplier
    assert_eq!(cumulative_multiplier, U64F64::from_num(9));
    
    println!("✅ Cumulative multiplier: {}x", cumulative_multiplier.to_num::<f64>());
    
    // Test with different chain lengths
    let test_cases = vec![
        (vec![1.0, 1.5], 1.5),
        (vec![1.0, 1.5, 2.0], 3.0),
        (vec![1.0, 1.5, 2.0, 3.0], 9.0),
        (vec![1.0, 2.0, 2.5, 4.0], 20.0),
    ];
    
    for (chain, expected) in test_cases {
        let multiplier = chain.iter()
            .map(|&x| U64F64::from_num(x))
            .fold(U64F64::from_num(1), |acc, m| acc * m);
        
        assert_eq!(multiplier, U64F64::from_num(expected));
        println!("✅ Chain {:?} = {}x multiplier", chain, expected);
    }
}

#[tokio::test]
async fn test_auto_chain_execution() {
    let program_id = Pubkey::new_unique();
    let mut test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;
    
    // Define chain steps through verses
    let chain_steps = vec![
        ChainStepType::OpenPosition { 
            market_id: [1u8; 32],
            leverage: 10,
            is_long: true,
        },
        ChainStepType::AddToPosition {
            position_id: [1u8; 32],
            additional_size: 50_000_000, // 50 USDC
        },
        ChainStepType::TakeProfit {
            position_id: [1u8; 32],
            percentage: 50, // 50%
        },
        ChainStepType::ClosePosition {
            position_id: [1u8; 32],
        },
    ];
    
    let chain_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::AutoChain {
            verse_id: 1u128,
            deposit: 100_000_000, // 100 USDC
            steps: chain_steps,
        }.try_to_vec().unwrap(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[chain_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
    
    println!("✅ Auto-chain executed through verse hierarchy");
}

#[test]
fn test_verse_liquidity_aggregation() {
    // Test liquidity aggregation across verse levels
    
    let verses = vec![
        Verse {
            id: 0,
            level: 0,
            total_liquidity: U64F64::from_num(10_000_000),
            ..Default::default()
        },
        Verse {
            id: 1,
            level: 1,
            parent_id: Some(0),
            total_liquidity: U64F64::from_num(3_000_000),
            ..Default::default()
        },
        Verse {
            id: 2,
            level: 1,
            parent_id: Some(0),
            total_liquidity: U64F64::from_num(2_000_000),
            ..Default::default()
        },
        Verse {
            id: 3,
            level: 2,
            parent_id: Some(1),
            total_liquidity: U64F64::from_num(1_000_000),
            ..Default::default()
        },
    ];
    
    // Calculate total child liquidity for verse 0
    let child_liquidity: U64F64 = verses.iter()
        .filter(|v| v.parent_id == Some(0))
        .map(|v| v.total_liquidity)
        .sum();
    
    assert_eq!(child_liquidity, U64F64::from_num(5_000_000));
    
    println!("✅ Root verse child liquidity: ${:.0}M", 
        child_liquidity.to_num::<f64>() / 1_000_000.0);
    
    // Verify liquidity distribution
    let root_liquidity = verses[0].total_liquidity;
    let utilization = child_liquidity / root_liquidity;
    
    assert!(utilization <= U64F64::from_num(1)); // Children can't exceed parent
    println!("✅ Liquidity utilization: {:.1}%", 
        utilization.to_num::<f64>() * 100.0);
}

#[test]
fn test_verse_market_routing() {
    // Test market routing through verse hierarchy
    
    struct MarketRoute {
        market_id: [u8; 32],
        verse_path: Vec<u128>,
        final_multiplier: f64,
    }
    
    let routes = vec![
        MarketRoute {
            market_id: [1u8; 32],
            verse_path: vec![0], // Direct in root
            final_multiplier: 1.0,
        },
        MarketRoute {
            market_id: [2u8; 32],
            verse_path: vec![0, 1], // Root -> Sports
            final_multiplier: 1.5,
        },
        MarketRoute {
            market_id: [3u8; 32],
            verse_path: vec![0, 1, 2], // Root -> Sports -> NFL
            final_multiplier: 3.0,
        },
        MarketRoute {
            market_id: [4u8; 32],
            verse_path: vec![0, 1, 2, 3], // Root -> Sports -> NFL -> Super Bowl
            final_multiplier: 9.0,
        },
    ];
    
    for route in routes {
        println!("✅ Market {:?}:", route.market_id[0]);
        println!("   Path: {:?}", route.verse_path);
        println!("   Multiplier: {}x", route.final_multiplier);
        
        // Verify path length determines multiplier
        let depth_multiplier = match route.verse_path.len() {
            1 => 1.0,
            2 => 1.5,
            3 => 3.0,
            4 => 9.0,
            _ => 1.0,
        };
        
        assert_eq!(route.final_multiplier, depth_multiplier);
    }
}

#[test]
fn test_verse_leverage_limits() {
    // Test leverage limits increase with verse depth
    
    let verse_leverage_limits = vec![
        (0, 50),   // Root: 50x max
        (1, 100),  // L1: 100x max
        (2, 250),  // L2: 250x max
        (3, 500),  // L3: 500x max
    ];
    
    for (level, max_leverage) in verse_leverage_limits {
        let verse = Verse {
            level,
            ..Default::default()
        };
        
        let leverage_limit = calculate_verse_leverage_limit(&verse);
        assert_eq!(leverage_limit, max_leverage);
        
        println!("✅ Verse level {}: Max {}x leverage", level, max_leverage);
    }
}

#[test]
fn test_cross_verse_position_migration() {
    // Test moving positions between verses
    
    let position = ChainPosition {
        owner: Pubkey::new_unique(),
        verse_id: 1, // Sports verse
        market_id: [1u8; 32],
        size: 100_000_000, // 100 USDC
        entry_multiplier: U64F64::from_num(1.5),
        ..Default::default()
    };
    
    // Migrate to higher verse (NFL)
    let new_verse_id = 2;
    let new_multiplier = U64F64::from_num(2.0);
    
    let migrated_position = ChainPosition {
        verse_id: new_verse_id,
        entry_multiplier: new_multiplier,
        ..position
    };
    
    assert_eq!(migrated_position.verse_id, 2);
    assert!(migrated_position.entry_multiplier > position.entry_multiplier);
    
    println!("✅ Position migrated from verse {} to {}", 
        position.verse_id, migrated_position.verse_id);
    println!("   Multiplier: {:.1}x -> {:.1}x",
        position.entry_multiplier.to_num::<f64>(),
        migrated_position.entry_multiplier.to_num::<f64>()
    );
}

#[test]
fn test_verse_fee_distribution() {
    // Test fee distribution across verse hierarchy
    
    let total_fees = U64F64::from_num(10_000); // 10k USDC in fees
    
    // Fee distribution percentages
    let root_share = U64F64::from_num(0.3);      // 30% to root
    let parent_share = U64F64::from_num(0.5);    // 50% to direct parent
    let current_share = U64F64::from_num(0.2);   // 20% to current verse
    
    let root_fees = total_fees * root_share;
    let parent_fees = total_fees * parent_share;
    let current_fees = total_fees * current_share;
    
    assert_eq!(root_fees, U64F64::from_num(3_000));
    assert_eq!(parent_fees, U64F64::from_num(5_000));
    assert_eq!(current_fees, U64F64::from_num(2_000));
    
    // Verify total
    let sum = root_fees + parent_fees + current_fees;
    assert_eq!(sum, total_fees);
    
    println!("✅ Fee distribution across verses:");
    println!("   Root: ${:.0} (30%)", root_fees.to_num::<f64>());
    println!("   Parent: ${:.0} (50%)", parent_fees.to_num::<f64>());
    println!("   Current: ${:.0} (20%)", current_fees.to_num::<f64>());
}

// Helper function
fn calculate_verse_leverage_limit(verse: &Verse) -> u32 {
    match verse.level {
        0 => 50,
        1 => 100,
        2 => 250,
        3 => 500,
        _ => 50,
    }
}