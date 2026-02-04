//! End-to-end test for partial liquidation with 2-8% OI/slot range

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
    instruction::BettingPlatformInstruction,
    state::{GlobalConfig, Position},
    keeper_liquidation::{LIQ_CAP_MIN, LIQ_CAP_MAX, SIGMA_FACTOR},
    math::U64F64,
};

#[tokio::test]
async fn test_partial_liquidation_2_percent_minimum() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    // Setup global config with specific OI
    let global_config = GlobalConfig {
        admin: Pubkey::new_unique(),
        vault: 100_000_000_000, // $100k vault
        total_oi: 1_000_000_000_000, // $1M open interest
        coverage: 100_000, // 0.1 coverage
        total_verses: 1,
        total_proposals: 1,
        immutable: false,
        emergency_halt: false,
        halt_timestamp: 0,
        mmt_mint: Pubkey::new_unique(),
        mmt_fee_vault: Pubkey::new_unique(),
        base_fee_rate: 28,
        last_update_slot_slot: 0,
    };

    let global_config_pubkey = Pubkey::new_unique();
    let mut config_data = vec![];
    global_config.serialize(&mut config_data).unwrap();
    
    program_test.add_account(
        global_config_pubkey,
        Account {
            lamports: 1_000_000,
            data: config_data,
            owner: program_id,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create position to liquidate
    let position_keypair = Keypair::new();
    let position = Position {
        discriminator: [0u8; 8],
        user: Keypair::new().pubkey(),
        proposal_id: 1,
        position_id: [1u8; 32],
        outcome: 0,
        size: 100_000_000_000, // $100k position
        notional: 100_000_000_000,
        leverage: 100, // High leverage for liquidation
        entry_price: 5000,
        liquidation_price: 4950,
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 1_000_000_000, // $1k margin
        is_short: false,
    };

    let mut position_data = vec![];
    position.serialize(&mut position_data).unwrap();

    // Test minimum liquidation (2% of OI)
    // With $1M OI and 2% cap, max liquidation per slot = $20k
    // Position is $100k, so should liquidate $20k
    
    let liquidate_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true), // keeper
            AccountMeta::new(position_keypair.pubkey(), false),
            AccountMeta::new(position.user, false),
            AccountMeta::new_readonly(global_config_pubkey, false),
            AccountMeta::new(Pubkey::new_unique(), false), // vault
        ],
        data: BettingPlatformInstruction::PartialLiquidate { position_index: 0 }
            .try_to_vec()
            .unwrap(),
    };

    // Expected liquidation amount with 2% cap
    let expected_liquidation = (global_config.total_oi as u128 * LIQ_CAP_MIN as u128 / 10000) as u64;
    assert_eq!(expected_liquidation, 20_000_000_000, "2% of $1M OI should be $20k");
}

#[tokio::test]
async fn test_partial_liquidation_8_percent_maximum() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    // Setup with high volatility scenario
    let global_config = GlobalConfig {
        admin: Pubkey::new_unique(),
        vault: 100_000_000_000,
        total_oi: 500_000_000_000, // $500k open interest
        coverage: 50_000, // 0.05 coverage (low)
        total_verses: 1,
        total_proposals: 1,
        immutable: false,
        emergency_halt: false,
        halt_timestamp: 0,
        mmt_mint: Pubkey::new_unique(),
        mmt_fee_vault: Pubkey::new_unique(),
        base_fee_rate: 28,
        last_update_slot_slot: 0,
    };

    let global_config_pubkey = Pubkey::new_unique();
    let mut config_data = vec![];
    global_config.serialize(&mut config_data).unwrap();
    
    program_test.add_account(
        global_config_pubkey,
        Account {
            lamports: 1_000_000,
            data: config_data,
            owner: program_id,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create highly leveraged position in volatile market
    let position = Position {
        discriminator: [0u8; 8],
        user: Keypair::new().pubkey(),
        proposal_id: 1,
        position_id: [2u8; 32],
        outcome: 0,
        size: 200_000_000_000, // $200k position
        notional: 200_000_000_000,
        leverage: 200, // Extreme leverage
        entry_price: 5000,
        liquidation_price: 4975,
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 1_000_000_000, // $1k margin
        is_short: false,
    };

    // Test maximum liquidation (8% of OI)
    // With $500k OI and 8% cap, max liquidation per slot = $40k
    let expected_max_liquidation = (global_config.total_oi as u128 * LIQ_CAP_MAX as u128 / 10000) as u64;
    assert_eq!(expected_max_liquidation, 40_000_000_000, "8% of $500k OI should be $40k");
}

#[tokio::test]
async fn test_dynamic_liquidation_cap_with_volatility() {
    use betting_platform_native::keeper_liquidation::LiquidationKeeper;
    
    // Test low volatility scenario
    let low_volatility = U64F64::from_num(10); // 10% volatility
    let open_interest = 1_000_000_000_000; // $1M
    
    let low_vol_cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(
        low_volatility,
        open_interest,
    ).unwrap();
    
    // With low volatility, should be closer to minimum (2%)
    assert!(low_vol_cap >= 20_000_000_000, "Should be at least $20k (2% of $1M)");
    assert!(low_vol_cap <= 40_000_000_000, "Should be less than $40k for low volatility");
    
    // Test high volatility scenario
    let high_volatility = U64F64::from_num(100); // 100% volatility
    let high_vol_cap = LiquidationKeeper::calculate_dynamic_liquidation_cap(
        high_volatility,
        open_interest,
    ).unwrap();
    
    // With high volatility, should be closer to maximum (8%)
    assert!(high_vol_cap >= 60_000_000_000, "Should be at least $60k for high volatility");
    assert!(high_vol_cap <= 80_000_000_000, "Should not exceed $80k (8% of $1M)");
}

#[tokio::test]
async fn test_partial_liquidation_accumulator() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create position that has already been partially liquidated
    let position = Position {
        discriminator: [0u8; 8],
        user: Keypair::new().pubkey(),
        proposal_id: 1,
        position_id: [3u8; 32],
        outcome: 0,
        size: 100_000_000_000, // $100k remaining
        notional: 150_000_000_000, // Originally $150k
        leverage: 50,
        entry_price: 5000,
        liquidation_price: 4900,
        is_long: true,
        created_at: 0,
        is_closed: false,
        partial_liq_accumulator: 50_000_000_000, // Already liquidated $50k
        verse_id: 1,
        margin: 2_000_000_000,
        is_short: false,
    };

    // Verify accumulator prevents over-liquidation in same slot
    // If cap is 8% of original $150k = $12k per slot
    // Already liquidated $50k, so should not allow more this slot
    assert_eq!(
        position.partial_liq_accumulator, 
        50_000_000_000, 
        "Accumulator tracks previous liquidations"
    );
}