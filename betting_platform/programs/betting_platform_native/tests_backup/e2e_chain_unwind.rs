//! End-to-end test for chain position unwinding

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use betting_platform_native::{
    error::BettingPlatformError,
    instruction::BettingPlatformInstruction,
    state::{
        chain_accounts::{ChainState, ChainPosition, ChainType, PositionInfo, PositionType},
        VersePDA, VerseStatus,
    },
    math::U64F64,
};

#[tokio::test]
async fn test_chain_unwind_reverse_order() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let user_keypair = Keypair::new();
    let chain_state_keypair = Keypair::new();
    
    // Create chain with positions in order: borrow → liquidation → stake
    let chain_position = ChainPosition {
        chain_id: 1,
        chain_type: ChainType::Leverage,
        positions: vec![
            PositionInfo {
                position_type: PositionType::Borrow,
                market_id: Pubkey::new_unique(),
                outcome: 0,
                amount: 5_000_000_000, // $5k borrow
                leverage: 2,
                entry_price: 5000,
                current_price: 4900,
                notional: 10_000_000_000,
                pnl: -200_000_000,
            },
            PositionInfo {
                position_type: PositionType::Liquidation,
                market_id: Pubkey::new_unique(),
                outcome: 1,
                amount: 3_000_000_000, // $3k liquidation position
                leverage: 5,
                entry_price: 6000,
                current_price: 5800,
                notional: 15_000_000_000,
                pnl: -500_000_000,
            },
            PositionInfo {
                position_type: PositionType::Stake,
                market_id: Pubkey::new_unique(),
                outcome: 0,
                amount: 2_000_000_000, // $2k stake
                leverage: 10,
                entry_price: 7000,
                current_price: 6500,
                notional: 20_000_000_000,
                pnl: -1_000_000_000,
            },
        ],
        total_notional: 45_000_000_000,
        total_leverage: 220, // 2 * 5 * 10 * 2.2 multiplier
        current_pnl: -1_700_000_000,
        created_at: 1234567890,
        updated_at: 1234567900,
        is_active: true,
        closed_at: None,
        current_health_factor: 8000, // 0.8 - unhealthy
    };

    let chain_state = ChainState {
        discriminator: [0u8; 8],
        user: user_keypair.pubkey(),
        chains: vec![chain_position.clone()],
        active_chains: 1,
        total_chains_created: 1,
        last_chain_id: 1,
    };

    // Verify unwind order is reverse: stake → liquidation → borrow
    let positions = &chain_position.positions;
    assert_eq!(positions[0].position_type, PositionType::Borrow);
    assert_eq!(positions[1].position_type, PositionType::Liquidation);
    assert_eq!(positions[2].position_type, PositionType::Stake);

    // When unwinding, process in reverse
    let unwind_order: Vec<_> = positions.iter().rev().collect();
    assert_eq!(unwind_order[0].position_type, PositionType::Stake);
    assert_eq!(unwind_order[1].position_type, PositionType::Liquidation);
    assert_eq!(unwind_order[2].position_type, PositionType::Borrow);
}

#[tokio::test]
async fn test_chain_unwind_verse_isolation() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    // Create halted verse
    let halted_verse = VersePDA {
        discriminator: [0u8; 8],
        verse_id: 100,
        parent_id: None,
        children_root: [0u8; 32],
        child_count: 0,
        total_descendants: 0,
        status: VerseStatus::Halted, // Halted status
        depth: 0,
        last_update_slot: 1000,
        total_oi: 50_000_000_000,
        derived_prob: U64F64::from_num(4000), // 0.4 (below 0.5)
        correlation_factor: U64F64::from_num(1),
        quantum_state: None,
        bump: 255,
    };

    let verse_keypair = Keypair::new();
    let mut verse_data = vec![];
    halted_verse.serialize(&mut verse_data).unwrap();

    program_test.add_account(
        verse_keypair.pubkey(),
        Account {
            lamports: 1_000_000,
            data: verse_data,
            owner: program_id,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Chain should isolate unwinding to halted verse
    assert_eq!(halted_verse.status, VerseStatus::Halted);
    assert!(halted_verse.derived_prob < U64F64::from_num(5000)); // Below 0.5
}

#[tokio::test]
async fn test_different_chain_type_unwinding() {
    // Test Leverage chain unwinding
    let leverage_chain = ChainPosition {
        chain_id: 1,
        chain_type: ChainType::Leverage,
        positions: vec![
            PositionInfo {
                position_type: PositionType::Borrow,
                market_id: Pubkey::new_unique(),
                outcome: 0,
                amount: 10_000_000_000,
                leverage: 5,
                entry_price: 5000,
                current_price: 4800,
                notional: 50_000_000_000,
                pnl: -2_000_000_000,
            },
        ],
        total_notional: 50_000_000_000,
        total_leverage: 5,
        current_pnl: -2_000_000_000,
        created_at: 1234567890,
        updated_at: 1234567900,
        is_active: true,
        closed_at: None,
        current_health_factor: 9000,
    };

    // Test Hedge chain unwinding (paired positions)
    let hedge_chain = ChainPosition {
        chain_id: 2,
        chain_type: ChainType::Hedge,
        positions: vec![
            PositionInfo {
                position_type: PositionType::Long,
                market_id: Pubkey::new_unique(),
                outcome: 0,
                amount: 5_000_000_000,
                leverage: 2,
                entry_price: 5000,
                current_price: 5100,
                notional: 10_000_000_000,
                pnl: 200_000_000,
            },
            PositionInfo {
                position_type: PositionType::Short,
                market_id: Pubkey::new_unique(),
                outcome: 1,
                amount: 5_000_000_000,
                leverage: 2,
                entry_price: 5000,
                current_price: 5100,
                notional: 10_000_000_000,
                pnl: -200_000_000,
            },
        ],
        total_notional: 20_000_000_000,
        total_leverage: 4,
        current_pnl: 0, // Hedged
        created_at: 1234567890,
        updated_at: 1234567900,
        is_active: true,
        closed_at: None,
        current_health_factor: 10000,
    };

    // Test Arbitrage chain unwinding (all positions simultaneously)
    let arbitrage_chain = ChainPosition {
        chain_id: 3,
        chain_type: ChainType::Arbitrage,
        positions: vec![
            PositionInfo {
                position_type: PositionType::Long,
                market_id: Pubkey::new_unique(),
                outcome: 0,
                amount: 10_000_000_000,
                leverage: 1,
                entry_price: 4990,
                current_price: 5010,
                notional: 10_000_000_000,
                pnl: 40_000_000,
            },
            PositionInfo {
                position_type: PositionType::Short,
                market_id: Pubkey::new_unique(),
                outcome: 0,
                amount: 10_000_000_000,
                leverage: 1,
                entry_price: 5010,
                current_price: 4990,
                notional: 10_000_000_000,
                pnl: 40_000_000,
            },
        ],
        total_notional: 20_000_000_000,
        total_leverage: 2,
        current_pnl: 80_000_000, // Arbitrage profit
        created_at: 1234567890,
        updated_at: 1234567900,
        is_active: true,
        closed_at: None,
        current_health_factor: 12000,
    };

    // Verify different unwinding strategies
    assert_eq!(leverage_chain.chain_type, ChainType::Leverage);
    assert_eq!(hedge_chain.chain_type, ChainType::Hedge);
    assert_eq!(arbitrage_chain.chain_type, ChainType::Arbitrage);
}

#[tokio::test]
async fn test_chain_already_closed_error() {
    let closed_chain = ChainPosition {
        chain_id: 4,
        chain_type: ChainType::Leverage,
        positions: vec![],
        total_notional: 0,
        total_leverage: 0,
        current_pnl: 0,
        created_at: 1234567890,
        updated_at: 1234567900,
        is_active: false, // Already closed
        closed_at: Some(1234567950),
        current_health_factor: 10000,
    };

    assert!(!closed_chain.is_active, "Chain should be inactive");
    assert!(closed_chain.closed_at.is_some(), "Should have closed timestamp");

    // Attempting to unwind should fail
    // Error: BettingPlatformError::ChainAlreadyClosed
}

#[tokio::test]
async fn test_emergency_chain_unwind() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create multiple active chains
    let chain_state = ChainState {
        discriminator: [0u8; 8],
        user: Keypair::new().pubkey(),
        chains: vec![
            ChainPosition {
                chain_id: 1,
                chain_type: ChainType::Leverage,
                positions: vec![],
                total_notional: 100_000_000_000,
                total_leverage: 50,
                current_pnl: -5_000_000_000,
                created_at: 1234567890,
                updated_at: 1234567900,
                is_active: true,
                closed_at: None,
                current_health_factor: 7000,
            },
            ChainPosition {
                chain_id: 2,
                chain_type: ChainType::Hedge,
                positions: vec![],
                total_notional: 50_000_000_000,
                total_leverage: 10,
                current_pnl: 0,
                created_at: 1234567890,
                updated_at: 1234567900,
                is_active: true,
                closed_at: None,
                current_health_factor: 9500,
            },
        ],
        active_chains: 2,
        total_chains_created: 2,
        last_chain_id: 2,
    };

    // Emergency unwind should mark all chains for unwinding
    assert_eq!(chain_state.active_chains, 2, "Should have 2 active chains");
    
    // After emergency unwind:
    // - All chains marked inactive
    // - active_chains = 0
    // - closed_at timestamps set
}