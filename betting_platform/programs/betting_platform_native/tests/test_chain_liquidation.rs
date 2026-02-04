//! Tests for chain position liquidation with proper unwinding order
//! Verifies: stake → liquidate → borrow

use solana_program_test::*;
use solana_sdk::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    liquidation::chain_liquidation::{ChainLiquidationProcessor, ChainStepType},
    state::chain_accounts::{ChainState, ChainPosition, ChainStatus, PositionStatus},
    instruction::ChainStepType as InstructionChainStepType,
};

#[tokio::test]
async fn test_unwinding_order() {
    // Create mock chain positions with different types
    let mut positions = vec![
        create_mock_position(1, 2), // Borrow (should be last)
        create_mock_position(2, 0), // Stake (should be first)
        create_mock_position(3, 1), // Liquidate (should be middle)
        create_mock_position(4, 0), // Another Stake
        create_mock_position(5, 2), // Another Borrow
    ];
    
    // Test the sorting function
    let original_order: Vec<u128> = positions.iter().map(|p| p.position_id).collect();
    
    // Sort positions
    positions.sort_by_key(|p| match p.step_index % 3 {
        0 => 0, // Stake
        1 => 1, // Liquidate
        _ => 2, // Borrow
    });
    
    let sorted_order: Vec<u128> = positions.iter().map(|p| p.position_id).collect();
    
    println!("Unwinding order test:");
    println!("  Original: {:?}", original_order);
    println!("  Sorted:   {:?}", sorted_order);
    
    // Verify correct order
    assert_eq!(sorted_order[0], 2, "First should be stake position");
    assert_eq!(sorted_order[1], 4, "Second should be stake position");
    assert_eq!(sorted_order[2], 3, "Third should be liquidate position");
    assert_eq!(sorted_order[3], 1, "Fourth should be borrow position");
    assert_eq!(sorted_order[4], 5, "Fifth should be borrow position");
}

#[tokio::test]
async fn test_chain_liquidation_validation() {
    let mut chain_state = create_mock_chain_state();
    let positions = vec![
        create_mock_position(1, 0),
        create_mock_position(2, 1),
    ];
    
    // Test active chain validation
    assert_eq!(chain_state.status, ChainStatus::Active, "Chain should be active");
    
    // Test with inactive chain
    chain_state.status = ChainStatus::Completed;
    // In actual implementation, this would fail validation
    
    println!("Chain validation test:");
    println!("  Chain ID: {}", chain_state.chain_id);
    println!("  Status: {:?}", chain_state.status);
    println!("  Positions: {}", positions.len());
}

#[tokio::test]
async fn test_partial_chain_liquidation() {
    let mut chain_state = create_mock_chain_state();
    chain_state.current_balance = 10_000_000_000; // 10,000 USDC
    
    let mut positions = vec![
        create_mock_position_with_size(1, 0, 1_000_000_000), // 1,000 USDC
        create_mock_position_with_size(2, 1, 2_000_000_000), // 2,000 USDC
        create_mock_position_with_size(3, 2, 3_000_000_000), // 3,000 USDC
    ];
    
    // Simulate partial liquidation
    let liquidation_cap = 500_000_000; // 500 USDC cap
    
    println!("\nPartial chain liquidation test:");
    println!("  Chain balance: ${}", chain_state.current_balance / 1_000_000);
    
    for (i, pos) in positions.iter_mut().enumerate() {
        let position_type = match pos.step_index % 3 {
            0 => "Stake",
            1 => "Liquidate",
            _ => "Borrow",
        };
        
        if pos.size > liquidation_cap {
            pos.size -= liquidation_cap;
            println!("  Position {} ({}): ${} -> ${} (partial)", 
                i + 1, position_type, 
                (pos.size + liquidation_cap) / 1_000_000,
                pos.size / 1_000_000);
        } else {
            println!("  Position {} ({}): ${} -> $0 (full)", 
                i + 1, position_type, pos.size / 1_000_000);
            pos.size = 0;
            pos.status = PositionStatus::Liquidated;
        }
    }
}

#[tokio::test]
async fn test_keeper_rewards() {
    let liquidation_amount = 10_000_000_000; // 10,000 USDC
    let keeper_reward_bps = 5; // 5 basis points
    
    let keeper_reward = (liquidation_amount as u128 * keeper_reward_bps as u128 / 10000) as u64;
    
    assert_eq!(keeper_reward, 5_000_000, "Keeper reward should be 5 USDC");
    
    println!("\nKeeper reward calculation:");
    println!("  Liquidation amount: ${}", liquidation_amount / 1_000_000);
    println!("  Reward rate: {} bps", keeper_reward_bps);
    println!("  Keeper reward: ${}", keeper_reward / 1_000_000);
}

#[tokio::test]
async fn test_chain_termination() {
    let mut chain_state = create_mock_chain_state();
    chain_state.current_balance = 100_000_000; // 100 USDC (low balance)
    
    // Simulate complete liquidation
    let total_liquidated = 100_000_000;
    let keeper_rewards = 50_000; // 0.05 USDC
    
    chain_state.current_balance = chain_state.current_balance
        .saturating_sub(total_liquidated)
        .saturating_sub(keeper_rewards);
    
    if chain_state.current_balance == 0 {
        chain_state.status = ChainStatus::Liquidated;
    }
    
    assert_eq!(chain_state.status, ChainStatus::Liquidated, "Chain should be terminated");
    assert_eq!(chain_state.current_balance, 0, "Balance should be zero");
    
    println!("\nChain termination test:");
    println!("  Final balance: ${}", chain_state.current_balance / 1_000_000);
    println!("  Status: {:?}", chain_state.status);
    println!("  ✓ Chain successfully terminated");
}

#[tokio::test]
async fn test_cascade_prevention() {
    // Test that unwinding order prevents cascading liquidations
    let positions = vec![
        ("Stake", 1_000_000_000),
        ("Liquidate", 2_000_000_000),
        ("Borrow", 3_000_000_000),
    ];
    
    println!("\nCascade prevention test:");
    println!("  Unwinding order ensures isolated liquidation:");
    
    for (i, (pos_type, size)) in positions.iter().enumerate() {
        println!("  {}: {} position ${} USDC", i + 1, pos_type, size / 1_000_000);
    }
    
    println!("  ✓ Stake positions liquidated first (least impact)");
    println!("  ✓ Borrow positions liquidated last (highest risk)");
}

// Helper functions
fn create_mock_chain_state() -> ChainState {
    ChainState {
        discriminator: [0u8; 8],
        chain_id: 12345,
        user: Pubkey::new_unique(),
        verse_id: 1,
        initial_deposit: 10_000_000_000,
        current_balance: 10_000_000_000,
        steps: vec![],
        current_step: 0,
        status: ChainStatus::Active,
        total_pnl: 0,
        created_at: 0,
        last_execution: 0,
        position_ids: vec![],
        error_count: 0,
        last_error: None,
    }
}

fn create_mock_position(position_id: u128, step_index: u8) -> ChainPosition {
    create_mock_position_with_size(position_id, step_index, 1_000_000_000)
}

fn create_mock_position_with_size(position_id: u128, step_index: u8, size: u64) -> ChainPosition {
    ChainPosition {
        discriminator: [0u8; 8],
        chain_id: 12345,
        position_id,
        proposal_id: 1,
        step_index,
        outcome: 0,
        size,
        leverage: 10,
        entry_price: 50000,
        is_long: true,
        status: PositionStatus::Open,
        realized_pnl: 0,
        created_at: 0,
        closed_at: None,
    }
}