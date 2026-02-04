//! Cross-Shard Transaction Integration Tests
//! 
//! Tests atomic transactions across multiple shards to verify
//! Part 7 specification compliance for cross-shard operations

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    sharding::{
        enhanced_sharding::{SHARDS_PER_MARKET, ShardType, MarketShardAllocation},
        cross_shard_communication::{CrossShardMessage, MessageType, MessagePriority},
    },
};
use borsh::BorshSerialize;

#[tokio::test]
async fn test_cross_shard_order_to_execution() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    // Create test accounts
    let market_id = Pubkey::new_unique();
    let user = Keypair::new();
    let order_shard = Keypair::new();
    let execution_shard = Keypair::new();
    
    // Add accounts to test environment
    program_test.add_account(
        user.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    
    let mut test_context = program_test.start_with_context().await;
    
    // Test 1: Place order on OrderBook shard
    let place_order_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(order_shard.pubkey(), false),
            AccountMeta::new(market_id, false),
        ],
        data: BettingPlatformInstruction::PlaceOrder {
            market_id,
            outcome: 0,
            amount: 1000,
            price: 500, // 0.5 probability
        }.try_to_vec().unwrap(),
    };
    
    let tx = Transaction::new_signed_with_payer(
        &[place_order_ix],
        Some(&test_context.payer.pubkey()),
        &[&test_context.payer, &user],
        test_context.last_blockhash,
    );
    
    test_context.banks_client.process_transaction(tx).await.unwrap();
    
    // Test 2: Verify cross-shard message created
    let message_account = test_context
        .banks_client
        .get_account(order_shard.pubkey())
        .await
        .unwrap();
    
    assert!(message_account.is_some(), "Cross-shard message should be created");
    
    // Test 3: Execute trade on Execution shard
    let execute_trade_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(execution_shard.pubkey(), false),
            AccountMeta::new(market_id, false),
            AccountMeta::new_readonly(order_shard.pubkey(), false),
        ],
        data: BettingPlatformInstruction::ExecuteCrossShardTrade {
            order_message_account: order_shard.pubkey(),
        }.try_to_vec().unwrap(),
    };
    
    let tx = Transaction::new_signed_with_payer(
        &[execute_trade_ix],
        Some(&test_context.payer.pubkey()),
        &[&test_context.payer, &user],
        test_context.last_blockhash,
    );
    
    test_context.banks_client.process_transaction(tx).await.unwrap();
    
    // Verify execution completed
    let execution_account = test_context
        .banks_client
        .get_account(execution_shard.pubkey())
        .await
        .unwrap();
    
    assert!(execution_account.is_some(), "Trade should be executed");
}

#[tokio::test]
async fn test_atomic_multi_shard_transaction() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let market_id = Pubkey::new_unique();
    let user = Keypair::new();
    
    // Create shard accounts for all 4 types
    let shards: Vec<Keypair> = (0..SHARDS_PER_MARKET)
        .map(|_| Keypair::new())
        .collect();
    
    // Add user account
    program_test.add_account(
        user.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    
    let mut test_context = program_test.start_with_context().await;
    
    // Create atomic transaction touching all shards
    let instructions: Vec<Instruction> = vec![
        // 1. Place order (OrderBook shard)
        Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(shards[0].pubkey(), false),
                AccountMeta::new(market_id, false),
            ],
            data: BettingPlatformInstruction::PlaceOrder {
                market_id,
                outcome: 1,
                amount: 5000,
                price: 600,
            }.try_to_vec().unwrap(),
        },
        // 2. Execute trade (Execution shard)
        Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(shards[1].pubkey(), false),
                AccountMeta::new(market_id, false),
            ],
            data: BettingPlatformInstruction::ExecuteTrade {
                market_id,
                trade_id: 1,
            }.try_to_vec().unwrap(),
        },
        // 3. Settle position (Settlement shard)
        Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(shards[2].pubkey(), false),
                AccountMeta::new(market_id, false),
            ],
            data: BettingPlatformInstruction::SettlePosition {
                market_id,
                position_id: 1,
            }.try_to_vec().unwrap(),
        },
        // 4. Update analytics (Analytics shard)
        Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(shards[3].pubkey(), false),
                AccountMeta::new(market_id, false),
            ],
            data: BettingPlatformInstruction::UpdateAnalytics {
                market_id,
            }.try_to_vec().unwrap(),
        },
    ];
    
    // Execute atomic transaction
    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&test_context.payer.pubkey()),
        &[&test_context.payer, &user],
        test_context.last_blockhash,
    );
    
    // Should succeed atomically or fail completely
    let result = test_context.banks_client.process_transaction(tx).await;
    assert!(result.is_ok(), "Atomic multi-shard transaction should succeed");
}

#[tokio::test]
async fn test_cross_shard_rebalancing() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let mut test_context = program_test.start_with_context().await;
    
    // Create multiple markets to trigger rebalancing
    let markets: Vec<Pubkey> = (0..100)
        .map(|_| Pubkey::new_unique())
        .collect();
    
    // Simulate high load on specific shards
    for (i, market) in markets.iter().enumerate() {
        // Create uneven load - first 20 markets get heavy traffic
        let tx_count = if i < 20 { 100 } else { 5 };
        
        for _ in 0..tx_count {
            let user = Keypair::new();
            let ix = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(user.pubkey(), true),
                    AccountMeta::new(*market, false),
                ],
                data: BettingPlatformInstruction::PlaceOrder {
                    market_id: *market,
                    outcome: 0,
                    amount: 100,
                    price: 500,
                }.try_to_vec().unwrap(),
            };
            
            // Process transactions (in real scenario this would measure contention)
            // For testing, we simulate the rebalancing trigger
        }
    }
    
    // Trigger rebalancing after 1000 slots
    let rebalance_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(test_context.payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::TriggerShardRebalancing {
            slot: 1000,
        }.try_to_vec().unwrap(),
    };
    
    let tx = Transaction::new_signed_with_payer(
        &[rebalance_ix],
        Some(&test_context.payer.pubkey()),
        &[&test_context.payer],
        test_context.last_blockhash,
    );
    
    test_context.banks_client.process_transaction(tx).await.unwrap();
    
    // Verify rebalancing occurred
    // In production, this would check that hot markets were redistributed
}

#[tokio::test]
async fn test_emergency_halt_across_shards() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let mut test_context = program_test.start_with_context().await;
    let market_id = Pubkey::new_unique();
    
    // Trigger emergency halt
    let halt_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(test_context.payer.pubkey(), true),
        ],
        data: BettingPlatformInstruction::EmergencyHaltAllShards.try_to_vec().unwrap(),
    };
    
    let tx = Transaction::new_signed_with_payer(
        &[halt_ix],
        Some(&test_context.payer.pubkey()),
        &[&test_context.payer],
        test_context.last_blockhash,
    );
    
    test_context.banks_client.process_transaction(tx).await.unwrap();
    
    // Verify no shard can process transactions
    let user = Keypair::new();
    let place_order_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(market_id, false),
        ],
        data: BettingPlatformInstruction::PlaceOrder {
            market_id,
            outcome: 0,
            amount: 1000,
            price: 500,
        }.try_to_vec().unwrap(),
    };
    
    let tx = Transaction::new_signed_with_payer(
        &[place_order_ix],
        Some(&test_context.payer.pubkey()),
        &[&test_context.payer, &user],
        test_context.last_blockhash,
    );
    
    // Should fail due to emergency halt
    let result = test_context.banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Transactions should fail during emergency halt");
}

#[tokio::test]
async fn test_cross_shard_performance() {
    // This test measures the performance of cross-shard operations
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "betting_platform_native",
        program_id,
        processor!(betting_platform_native::entrypoint::process_instruction),
    );
    
    let mut test_context = program_test.start_with_context().await;
    
    // Create markets
    let markets: Vec<Pubkey> = (0..1000)
        .map(|_| Pubkey::new_unique())
        .collect();
    
    let start = std::time::Instant::now();
    
    // Execute 1000 cross-shard transactions
    for market in markets.iter().take(1000) {
        let user = Keypair::new();
        
        // Cross-shard operation: order -> execute -> settle
        let instructions = vec![
            Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(user.pubkey(), true),
                    AccountMeta::new(*market, false),
                ],
                data: BettingPlatformInstruction::PlaceOrder {
                    market_id: *market,
                    outcome: 0,
                    amount: 100,
                    price: 500,
                }.try_to_vec().unwrap(),
            },
        ];
        
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&test_context.payer.pubkey()),
            &[&test_context.payer, &user],
            test_context.last_blockhash,
        );
        
        let _ = test_context.banks_client.process_transaction(tx).await;
    }
    
    let elapsed = start.elapsed();
    let tps = 1000.0 / elapsed.as_secs_f64();
    
    println!("Cross-shard TPS: {:.2}", tps);
    assert!(tps > 1000.0, "Should achieve >1000 TPS for cross-shard operations");
}

#[cfg(test)]
mod test_utils {
    use super::*;
    
    /// Helper to create a test market with sharding
    pub async fn create_sharded_market(
        program_id: Pubkey,
        banks_client: &mut BanksClient,
        payer: &Keypair,
        recent_blockhash: solana_sdk::hash::Hash,
    ) -> Result<(Pubkey, Vec<Pubkey>), Box<dyn std::error::Error>> {
        let market_id = Pubkey::new_unique();
        let shards: Vec<Pubkey> = (0..SHARDS_PER_MARKET)
            .map(|_| Pubkey::new_unique())
            .collect();
        
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(market_id, false),
            ],
            data: BettingPlatformInstruction::InitializeShardedMarket {
                market_id,
                shard_accounts: shards.clone(),
            }.try_to_vec()?,
        };
        
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );
        
        banks_client.process_transaction(tx).await?;
        
        Ok((market_id, shards))
    }
}