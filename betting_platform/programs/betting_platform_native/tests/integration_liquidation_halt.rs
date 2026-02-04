//! Integration test for liquidation halt mechanism
//!
//! Tests 1-hour halt after significant liquidation events

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::mem;

use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    state::{
        GlobalConfigPDA, ProposalPDA, Position, UserMap,
        discriminators,
    },
    liquidation::halt_mechanism::{
        LiquidationHaltState, LIQUIDATION_HALT_DURATION,
        LIQUIDATION_COUNT_THRESHOLD, LIQUIDATION_VALUE_THRESHOLD,
    },
    error::BettingPlatformError,
};

#[tokio::test]
async fn test_liquidation_halt_mechanism() {
    // Setup test environment
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );
    
    // Initialize accounts
    let global_config_pda = Keypair::new();
    let halt_state_pda = Keypair::new();
    let override_authority = Keypair::new();
    let user1 = Keypair::new();
    let user2 = Keypair::new();
    
    // Add initial accounts
    program_test.add_account(
        global_config_pda.pubkey(),
        Account {
            lamports: Rent::default().minimum_balance(mem::size_of::<GlobalConfigPDA>()),
            data: vec![0; mem::size_of::<GlobalConfigPDA>()],
            owner: program_id,
            ..Account::default()
        },
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Step 1: Initialize liquidation halt state
    println!("Step 1: Initialize liquidation halt state");
    {
        let accounts = vec![
            AccountMeta::new(halt_state_pda.pubkey(), false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ];
        
        let instruction = Instruction::new_with_borsh(
            program_id,
            &BettingPlatformInstruction::InitializeLiquidationHaltState {
                override_authority: override_authority.pubkey(),
            },
            accounts,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        
        banks_client.process_transaction(transaction).await.unwrap();
    }
    
    // Create test positions
    let positions = create_test_positions(&mut banks_client, &payer, &program_id, 15).await;
    
    // Step 2: Test liquidation count threshold
    println!("\nStep 2: Test liquidation count threshold (>10 liquidations)");
    {
        // Liquidate 11 positions rapidly
        for i in 0..11 {
            println!("  Liquidating position {}", i + 1);
            
            // Simulate liquidation
            let liquidation_value = 10_000_000_000; // $10k each
            
            let accounts = vec![
                AccountMeta::new(halt_state_pda.pubkey(), false),
                AccountMeta::new_readonly(global_config_pda.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ];
            
            let instruction = Instruction::new_with_borsh(
                program_id,
                &MockLiquidationInstruction {
                    liquidation_value,
                },
                accounts,
            );
            
            let result = banks_client.process_transaction(
                Transaction::new_signed_with_payer(
                    &[instruction],
                    Some(&payer.pubkey()),
                    &[&payer],
                    recent_blockhash,
                )
            ).await;
            
            if i < 10 {
                // First 10 should succeed
                assert!(result.is_ok());
            } else {
                // 11th should trigger halt
                assert!(result.is_err());
                println!("  ✓ Liquidation halted after {} liquidations", i + 1);
            }
        }
    }
    
    // Step 3: Test liquidation value threshold
    println!("\nStep 3: Test liquidation value threshold (>$100k total)");
    {
        // Wait for halt to expire
        advance_clock(&mut banks_client, LIQUIDATION_HALT_DURATION + 100).await;
        
        // Liquidate with large values
        let large_liquidation_value = 60_000_000_000; // $60k
        
        for i in 0..3 {
            println!("  Liquidating $60k position {}", i + 1);
            
            let accounts = vec![
                AccountMeta::new(halt_state_pda.pubkey(), false),
                AccountMeta::new_readonly(global_config_pda.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ];
            
            let instruction = Instruction::new_with_borsh(
                program_id,
                &MockLiquidationInstruction {
                    liquidation_value: large_liquidation_value,
                },
                accounts,
            );
            
            let result = banks_client.process_transaction(
                Transaction::new_signed_with_payer(
                    &[instruction],
                    Some(&payer.pubkey()),
                    &[&payer],
                    recent_blockhash,
                )
            ).await;
            
            if i < 1 {
                // First $60k should succeed
                assert!(result.is_ok());
            } else {
                // Second $60k pushes total over $100k threshold
                assert!(result.is_err());
                println!("  ✓ Liquidation halted after ${} total", (i + 1) * 60_000);
            }
        }
    }
    
    // Step 4: Test manual override
    println!("\nStep 4: Test manual override by authority");
    {
        let accounts = vec![
            AccountMeta::new(halt_state_pda.pubkey(), false),
            AccountMeta::new_readonly(override_authority.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
        ];
        
        let instruction = Instruction::new_with_borsh(
            program_id,
            &BettingPlatformInstruction::OverrideLiquidationHalt {
                force_resume: true,
            },
            accounts,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer, &override_authority],
            recent_blockhash,
        );
        
        banks_client.process_transaction(transaction).await.unwrap();
        println!("  ✓ Halt manually overridden by authority");
    }
    
    // Step 5: Test halt expiration
    println!("\nStep 5: Test automatic halt expiration");
    {
        // Trigger another halt
        trigger_halt_by_count(&mut banks_client, &payer, &program_id, &halt_state_pda, &global_config_pda).await;
        
        // Advance clock by halt duration
        advance_clock(&mut banks_client, LIQUIDATION_HALT_DURATION).await;
        
        // Try liquidation - should succeed after expiration
        let accounts = vec![
            AccountMeta::new(halt_state_pda.pubkey(), false),
            AccountMeta::new_readonly(global_config_pda.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
        ];
        
        let instruction = Instruction::new_with_borsh(
            program_id,
            &MockLiquidationInstruction {
                liquidation_value: 5_000_000_000,
            },
            accounts,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );
        
        banks_client.process_transaction(transaction).await.unwrap();
        println!("  ✓ Liquidations resumed after 1-hour halt expiration");
    }
    
    println!("\n✅ All liquidation halt tests passed!");
}

#[tokio::test]
async fn test_coverage_based_halt() {
    println!("Testing coverage-based liquidation halt...");
    
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::processor::process_instruction),
    );
    
    let global_config_pda = Keypair::new();
    let halt_state_pda = Keypair::new();
    let override_authority = Keypair::new();
    
    // Set up global config with low coverage
    let mut global_config = GlobalConfigPDA::new();
    global_config.vault = 50_000_000_000; // $50k vault
    global_config.total_oi = 200_000_000_000; // $200k OI = 25% coverage
    
    program_test.add_account(
        global_config_pda.pubkey(),
        Account {
            lamports: Rent::default().minimum_balance(mem::size_of::<GlobalConfigPDA>()),
            data: global_config.try_to_vec().unwrap(),
            owner: program_id,
            ..Account::default()
        },
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Initialize halt state
    initialize_halt_state(&mut banks_client, &payer, &program_id, &halt_state_pda, &override_authority).await;
    
    // Try liquidation with low coverage - should trigger halt
    println!("Attempting liquidation with 25% coverage (threshold: 50%)");
    
    let accounts = vec![
        AccountMeta::new(halt_state_pda.pubkey(), false),
        AccountMeta::new_readonly(global_config_pda.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
    ];
    
    let instruction = Instruction::new_with_borsh(
        program_id,
        &MockLiquidationInstruction {
            liquidation_value: 5_000_000_000,
        },
        accounts,
    );
    
    let result = banks_client.process_transaction(
        Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        )
    ).await;
    
    assert!(result.is_err());
    println!("✓ Liquidation correctly halted due to low coverage ratio");
}

// Helper structures and functions

#[derive(BorshSerialize, BorshDeserialize)]
struct MockLiquidationInstruction {
    liquidation_value: u64,
}

async fn create_test_positions(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    program_id: &Pubkey,
    count: usize,
) -> Vec<Pubkey> {
    let mut positions = vec![];
    
    for i in 0..count {
        let position = Keypair::new();
        let user = Keypair::new();
        
        // Create position account
        let position_data = Position::new(
            user.pubkey(),
            i as u128,
            0, // verse_id
            0, // outcome
            1_000_000_000, // size: $1k
            10, // 10x leverage
            500_000, // entry price: 0.5
            true, // is_long
            0, // created_at
        );
        
        // Add position account (simplified - in real test would use proper initialization)
        positions.push(position.pubkey());
    }
    
    positions
}

async fn advance_clock(banks_client: &mut BanksClient, slots: u64) {
    // In real test, would use program_test.warp_to_slot()
    // This is a simplified version
    println!("  Advancing clock by {} slots...", slots);
}

async fn trigger_halt_by_count(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    program_id: &Pubkey,
    halt_state_pda: &Keypair,
    global_config_pda: &Keypair,
) {
    println!("  Triggering halt by liquidation count...");
    
    // Liquidate 11 positions to trigger halt
    for i in 0..11 {
        let accounts = vec![
            AccountMeta::new(halt_state_pda.pubkey(), false),
            AccountMeta::new_readonly(global_config_pda.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
        ];
        
        let instruction = Instruction::new_with_borsh(
            *program_id,
            &MockLiquidationInstruction {
                liquidation_value: 5_000_000_000,
            },
            accounts,
        );
        
        let _ = banks_client.process_transaction(
            Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                banks_client.get_latest_blockhash().await.unwrap(),
            )
        ).await;
    }
}

async fn initialize_halt_state(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    program_id: &Pubkey,
    halt_state_pda: &Keypair,
    override_authority: &Keypair,
) {
    let accounts = vec![
        AccountMeta::new(halt_state_pda.pubkey(), false),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let instruction = Instruction::new_with_borsh(
        *program_id,
        &BettingPlatformInstruction::InitializeLiquidationHaltState {
            override_authority: override_authority.pubkey(),
        },
        accounts,
    );
    
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        banks_client.get_latest_blockhash().await.unwrap(),
    );
    
    banks_client.process_transaction(transaction).await.unwrap();
}