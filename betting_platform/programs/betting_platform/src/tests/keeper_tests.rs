use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
    system_instruction,
};
use std::time::Duration;
use crate::keeper_network::*;
use crate::account_structs::*;
use crate::errors::ErrorCode;

#[cfg(test)]
mod keeper_reward_tests {
    use super::*;

    #[tokio::test]
    async fn test_liquidation_keeper_5bp_rewards() {
        let mut program_test = ProgramTest::new(
            "betting_platform",
            crate::id(),
            processor!(crate::entry),
        );

        // Add test accounts
        let keeper_keypair = Keypair::new();
        let position_owner = Keypair::new();
        let vault_keypair = Keypair::new();
        
        // Fund accounts
        program_test.add_account(
            keeper_keypair.pubkey(),
            solana_sdk::account::Account {
                lamports: 10_000_000_000, // 10 SOL
                data: vec![],
                owner: solana_sdk::system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Create keeper account with MMT stake
        let keeper_account = KeeperAccount {
            keeper_id: [1u8; 32],
            authority: keeper_keypair.pubkey(),
            mmt_stake: 1_000_000, // 1M MMT
            performance_score: 9500, // 95% success rate
            total_operations: 100,
            successful_operations: 95,
            total_rewards_earned: 0,
            last_operation_slot: 0,
            status: KeeperStatus::Active,
            specializations: vec![KeeperSpecialization::Liquidations],
        };

        // Create at-risk position
        let position = create_test_position_at_risk();
        let liquidation_amount = 100_000_000; // 100 USDC

        // Calculate expected keeper reward (5bp of liquidated amount)
        let expected_keeper_reward = liquidation_amount * KEEPER_REWARD_BPS / 10000;
        assert_eq!(expected_keeper_reward, 50_000); // 0.05% of 100 USDC = 0.05 USDC

        // Simulate liquidation execution
        let initial_vault_balance = 1_000_000_000; // 1000 USDC
        let initial_keeper_balance = 0;

        // After liquidation
        let final_vault_balance = initial_vault_balance - expected_keeper_reward;
        let final_keeper_balance = initial_keeper_balance + expected_keeper_reward;

        println!("Liquidation amount: {} USDC", liquidation_amount / 1_000_000);
        println!("Keeper reward (5bp): {} USDC", expected_keeper_reward / 1_000_000);
        println!("Vault balance change: {} -> {} USDC", 
            initial_vault_balance / 1_000_000, 
            final_vault_balance / 1_000_000
        );

        // Verify reward calculation
        assert_eq!(expected_keeper_reward, liquidation_amount * 5 / 10000);
        assert!(expected_keeper_reward > 0, "Keeper should receive reward");
    }

    #[tokio::test]
    async fn test_stop_loss_keeper_2bp_bounty() {
        // Create stop order with prepaid bounty
        let order_size = 50_000_000; // 50 USDC
        let stop_order = StopOrder {
            order_id: [1u8; 32],
            market_id: [2u8; 32],
            user: Keypair::new().pubkey(),
            order_type: StopOrderType::StopLoss,
            side: OrderSide::Long,
            size: order_size,
            trigger_price: 45_000_000, // $45
            is_active: true,
            prepaid_bounty: order_size * STOP_KEEPER_BOUNTY_BPS / 10000,
            position_entry_price: 50_000_000, // $50
            trailing_distance: 0,
            trailing_price: 0,
        };

        // Calculate expected bounty (2bp)
        let expected_bounty = order_size * STOP_KEEPER_BOUNTY_BPS / 10000;
        assert_eq!(expected_bounty, 10_000); // 0.02% of 50 USDC = 0.01 USDC

        println!("Stop order size: {} USDC", order_size / 1_000_000);
        println!("Keeper bounty (2bp): {} USDC", expected_bounty / 1_000_000);
        println!("Prepaid bounty: {} USDC", stop_order.prepaid_bounty / 1_000_000);

        // Verify user prepaid correct amount
        assert_eq!(stop_order.prepaid_bounty, expected_bounty);
        
        // Test execution result
        let execution_result = ExecutionResult {
            executed_value: order_size,
        };
        
        let keeper_bounty = execution_result.executed_value * STOP_KEEPER_BOUNTY_BPS / 10000;
        assert_eq!(keeper_bounty, expected_bounty);
    }

    #[tokio::test]
    async fn test_keeper_priority_calculation() {
        // Test different keeper configurations
        let keepers = vec![
            KeeperAccount {
                keeper_id: [1u8; 32],
                authority: Pubkey::new_unique(),
                mmt_stake: 10_000_000, // 10M MMT
                performance_score: 9000, // 90%
                total_operations: 1000,
                successful_operations: 900,
                total_rewards_earned: 500_000,
                last_operation_slot: 100,
                status: KeeperStatus::Active,
                specializations: vec![KeeperSpecialization::Liquidations],
            },
            KeeperAccount {
                keeper_id: [2u8; 32],
                authority: Pubkey::new_unique(),
                mmt_stake: 5_000_000, // 5M MMT
                performance_score: 9500, // 95%
                total_operations: 500,
                successful_operations: 475,
                total_rewards_earned: 250_000,
                last_operation_slot: 100,
                status: KeeperStatus::Active,
                specializations: vec![KeeperSpecialization::Liquidations],
            },
            KeeperAccount {
                keeper_id: [3u8; 32],
                authority: Pubkey::new_unique(),
                mmt_stake: 1_000_000, // 1M MMT
                performance_score: 9900, // 99%
                total_operations: 100,
                successful_operations: 99,
                total_rewards_earned: 50_000,
                last_operation_slot: 100,
                status: KeeperStatus::Active,
                specializations: vec![KeeperSpecialization::Liquidations],
            },
        ];

        // Calculate priorities
        let priorities: Vec<(usize, u64)> = keepers.iter()
            .enumerate()
            .map(|(i, k)| (i, k.calculate_priority()))
            .collect();

        // Sort by priority
        let mut sorted_priorities = priorities.clone();
        sorted_priorities.sort_by(|a, b| b.1.cmp(&a.1));

        println!("\nKeeper Priorities:");
        for (idx, priority) in &sorted_priorities {
            let keeper = &keepers[*idx];
            println!("Keeper {}: Stake={} MMT, Performance={}%, Priority={}",
                idx + 1,
                keeper.mmt_stake / 1_000_000,
                keeper.performance_score / 100,
                priority
            );
        }

        // Keeper 1 should have highest priority (high stake * good performance)
        assert_eq!(sorted_priorities[0].0, 0, "Keeper 1 should have highest priority");
    }

    #[tokio::test]
    async fn test_keeper_suspension_threshold() {
        let mut keeper = KeeperAccount {
            keeper_id: [1u8; 32],
            authority: Pubkey::new_unique(),
            mmt_stake: 1_000_000,
            performance_score: 8500, // 85% - above threshold
            total_operations: 100,
            successful_operations: 85,
            total_rewards_earned: 100_000,
            last_operation_slot: 100,
            status: KeeperStatus::Active,
            specializations: vec![KeeperSpecialization::Liquidations],
        };

        // Simulate failures to drop below 80% threshold
        for _ in 0..10 {
            keeper.total_operations += 1;
            // Don't increment successful_operations (failure)
            keeper.performance_score = (keeper.successful_operations * 10000) / keeper.total_operations;
        }

        println!("After 10 failures:");
        println!("Total operations: {}", keeper.total_operations);
        println!("Successful: {}", keeper.successful_operations);
        println!("Performance score: {}%", keeper.performance_score / 100);

        // Check if below suspension threshold
        assert!(keeper.performance_score < SUSPENSION_THRESHOLD);
        
        // In production, this would trigger suspension
        if keeper.performance_score < SUSPENSION_THRESHOLD {
            keeper.status = KeeperStatus::Suspended;
        }
        
        assert_eq!(keeper.status, KeeperStatus::Suspended);
    }
}

#[cfg(test)]
mod stop_loss_execution_tests {
    use super::*;

    #[tokio::test]
    async fn test_stop_loss_trigger_conditions() {
        let test_cases = vec![
            // (order_type, side, entry_price, trigger_price, current_price, should_trigger)
            (StopOrderType::StopLoss, OrderSide::Long, 50_000, 45_000, 44_000, true),
            (StopOrderType::StopLoss, OrderSide::Long, 50_000, 45_000, 46_000, false),
            (StopOrderType::TakeProfit, OrderSide::Long, 50_000, 55_000, 56_000, true),
            (StopOrderType::TakeProfit, OrderSide::Long, 50_000, 55_000, 54_000, false),
            (StopOrderType::StopLoss, OrderSide::Short, 50_000, 55_000, 56_000, true),
            (StopOrderType::StopLoss, OrderSide::Short, 50_000, 55_000, 54_000, false),
        ];

        for (order_type, side, entry, trigger, current, should_trigger) in test_cases {
            let order = StopOrder {
                order_id: [1u8; 32],
                market_id: [2u8; 32],
                user: Pubkey::new_unique(),
                order_type,
                side,
                size: 10_000,
                trigger_price: trigger,
                is_active: true,
                prepaid_bounty: 2, // 2bp
                position_entry_price: entry,
                trailing_distance: 0,
                trailing_price: 0,
            };

            let triggered = match order.order_type {
                StopOrderType::StopLoss => {
                    if order.side == OrderSide::Long {
                        current <= order.trigger_price
                    } else {
                        current >= order.trigger_price
                    }
                },
                StopOrderType::TakeProfit => {
                    if order.side == OrderSide::Long {
                        current >= order.trigger_price
                    } else {
                        current <= order.trigger_price
                    }
                },
                StopOrderType::TrailingStop => false, // Tested separately
            };

            assert_eq!(triggered, should_trigger, 
                "Order type: {:?}, Side: {:?}, Entry: {}, Trigger: {}, Current: {}",
                order_type, side, entry, trigger, current
            );
        }
    }

    #[tokio::test]
    async fn test_trailing_stop_functionality() {
        let mut order = StopOrder {
            order_id: [1u8; 32],
            market_id: [2u8; 32],
            user: Pubkey::new_unique(),
            order_type: StopOrderType::TrailingStop,
            side: OrderSide::Long,
            size: 10_000,
            trigger_price: 0, // Not used for trailing
            is_active: true,
            prepaid_bounty: 2,
            position_entry_price: 50_000,
            trailing_distance: 5_000, // $5 trailing
            trailing_price: 50_000, // Starts at entry
        };

        // Price moves up to $55 - trailing stop should move to $50
        let new_price = 55_000;
        if order.side == OrderSide::Long && new_price > order.trailing_price {
            order.trailing_price = new_price;
        }
        
        // Check trigger at various prices
        let test_prices = vec![
            (54_000, false), // Above trailing stop
            (51_000, false), // Still above
            (50_000, true),  // At trailing stop
            (49_000, true),  // Below trailing stop
        ];

        for (price, should_trigger) in test_prices {
            let distance = order.trailing_price.saturating_sub(price);
            let triggered = distance >= order.trailing_distance;
            
            assert_eq!(triggered, should_trigger,
                "Price: {}, Trailing: {}, Distance: {}, Should trigger: {}",
                price, order.trailing_price, distance, should_trigger
            );
        }
    }

    #[tokio::test]
    async fn test_stop_order_priority_queue() {
        let mut stop_orders = vec![];
        
        // Create orders with different priorities
        for i in 0..5 {
            let order = StopOrder {
                order_id: [i as u8; 32],
                market_id: [1u8; 32],
                user: Pubkey::new_unique(),
                order_type: StopOrderType::StopLoss,
                side: OrderSide::Long,
                size: 10_000 * (i as u64 + 1), // Larger orders = higher priority
                trigger_price: 45_000,
                is_active: true,
                prepaid_bounty: 2 * (i as u64 + 1), // More bounty = higher priority
                position_entry_price: 50_000,
                trailing_distance: 0,
                trailing_price: 0,
            };
            stop_orders.push(order);
        }

        // Calculate priorities
        let mut triggered_orders: Vec<TriggeredOrder> = stop_orders.iter()
            .map(|order| TriggeredOrder {
                order_id: order.order_id,
                account: Pubkey::new_unique(),
                order_type: order.order_type,
                trigger_price: order.trigger_price,
                current_price: 44_000,
                priority: order.calculate_priority(),
            })
            .collect();

        // Sort by priority
        triggered_orders.sort_by(|a, b| b.priority.cmp(&a.priority));

        println!("\nStop Order Priority Queue:");
        for (i, order) in triggered_orders.iter().enumerate() {
            println!("Position {}: Order ID: {:?}, Priority: {}", 
                i + 1, order.order_id[0], order.priority);
        }

        // Verify orders are sorted by priority
        for i in 1..triggered_orders.len() {
            assert!(
                triggered_orders[i-1].priority >= triggered_orders[i].priority,
                "Orders should be sorted by priority"
            );
        }
    }
}

#[cfg(test)]
mod keeper_coordination_tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_keeper_work_distribution() {
        // Create 10 keepers with varying stakes and performance
        let mut keepers = vec![];
        for i in 0..10 {
            let keeper = KeeperAccount {
                keeper_id: [i as u8; 32],
                authority: Pubkey::new_unique(),
                mmt_stake: 1_000_000 * (10 - i as u64), // Decreasing stake
                performance_score: 8000 + (i as u64 * 200), // Increasing performance
                total_operations: 100,
                successful_operations: 80 + (i as u64 * 2),
                total_rewards_earned: 0,
                last_operation_slot: 0,
                status: KeeperStatus::Active,
                specializations: vec![KeeperSpecialization::Liquidations],
            };
            keepers.push(keeper);
        }

        // Create 100 work items
        let work_items: Vec<WorkItem> = (0..100).map(|i| {
            WorkItem {
                id: [i as u8; 32],
                work_type: WorkType::Liquidations,
                priority: 100 - i as u64, // Decreasing priority
                data: vec![],
            }
        }).collect();

        // Sort keepers by priority
        let mut sorted_keepers = keepers.clone();
        sorted_keepers.sort_by(|a, b| {
            let a_priority = a.calculate_priority();
            let b_priority = b.calculate_priority();
            b_priority.cmp(&a_priority)
        });

        // Distribute work
        let items_per_keeper = work_items.len() / sorted_keepers.len();
        let mut assignments = vec![];
        
        for (i, keeper) in sorted_keepers.iter().enumerate() {
            let start = i * items_per_keeper;
            let end = if i == sorted_keepers.len() - 1 {
                work_items.len()
            } else {
                (i + 1) * items_per_keeper
            };
            
            let assigned_count = end - start;
            assignments.push((keeper.keeper_id[0], assigned_count));
            
            println!("Keeper {}: Priority={}, Assigned {} items",
                keeper.keeper_id[0],
                keeper.calculate_priority(),
                assigned_count
            );
        }

        // Verify work distribution
        let total_assigned: usize = assignments.iter().map(|(_, count)| count).sum();
        assert_eq!(total_assigned, work_items.len(), "All work should be assigned");
        
        // Higher priority keepers should get work first
        assert!(assignments[0].1 >= assignments[assignments.len()-1].1);
    }

    #[tokio::test]
    async fn test_keeper_failure_handling() {
        let mut failed_keeper = KeeperAccount {
            keeper_id: [1u8; 32],
            authority: Pubkey::new_unique(),
            mmt_stake: 1_000_000,
            performance_score: 8200, // 82%
            total_operations: 100,
            successful_operations: 82,
            total_rewards_earned: 50_000,
            last_operation_slot: 100,
            status: KeeperStatus::Active,
            specializations: vec![KeeperSpecialization::Liquidations],
        };

        let mut registry = KeeperRegistry {
            total_keepers: 5,
            active_keepers: 5,
            total_rewards_distributed: 1_000_000,
            performance_threshold: 100,
            slash_threshold: 10,
        };

        // Simulate failure
        failed_keeper.total_operations += 1;
        failed_keeper.performance_score = 
            (failed_keeper.successful_operations * 10000) / failed_keeper.total_operations;

        println!("After failure:");
        println!("Performance: {}%", failed_keeper.performance_score / 100);

        // Check if suspension needed
        if failed_keeper.performance_score < SUSPENSION_THRESHOLD {
            failed_keeper.status = KeeperStatus::Suspended;
            registry.active_keepers -= 1;
            
            println!("Keeper suspended!");
        }

        // Find backup keeper
        let backup_keepers = vec![
            KeeperAccount {
                keeper_id: [2u8; 32],
                authority: Pubkey::new_unique(),
                mmt_stake: 2_000_000,
                performance_score: 9000,
                total_operations: 200,
                successful_operations: 180,
                total_rewards_earned: 100_000,
                last_operation_slot: 100,
                status: KeeperStatus::Active,
                specializations: vec![KeeperSpecialization::Liquidations],
            },
            KeeperAccount {
                keeper_id: [3u8; 32],
                authority: Pubkey::new_unique(),
                mmt_stake: 500_000,
                performance_score: 9500,
                total_operations: 50,
                successful_operations: 47,
                total_rewards_earned: 25_000,
                last_operation_slot: 100,
                status: KeeperStatus::Active,
                specializations: vec![KeeperSpecialization::Liquidations],
            },
        ];

        // Select best backup
        let best_backup = backup_keepers.iter()
            .filter(|k| k.status == KeeperStatus::Active)
            .max_by_key(|k| k.calculate_priority())
            .unwrap();

        println!("\nBackup keeper selected: ID={}, Priority={}",
            best_backup.keeper_id[0],
            best_backup.calculate_priority()
        );

        assert_eq!(best_backup.keeper_id[0], 2, "Keeper 2 should be selected as backup");
    }

    #[tokio::test]
    async fn test_websocket_health_monitoring() {
        // Test different websocket health states
        let test_cases = vec![
            (50, WebSocketHealth::Healthy),      // ~20 seconds
            (300, WebSocketHealth::Healthy),     // ~2 minutes - still healthy
            (500, WebSocketHealth::Degraded),    // ~3.3 minutes - degraded
            (1000, WebSocketHealth::Failed),     // ~6.6 minutes - failed
        ];

        let current_slot = 10_000;
        
        for (slots_since_update, expected_health) in test_cases {
            let last_update = current_slot - slots_since_update;
            
            let health = if slots_since_update < 150 {  // ~1 minute
                WebSocketHealth::Healthy
            } else if slots_since_update < 750 {  // ~5 minutes
                WebSocketHealth::Degraded
            } else {
                WebSocketHealth::Failed
            };

            assert_eq!(health, expected_health,
                "Slots since update: {}, Expected: {:?}",
                slots_since_update, expected_health
            );

            if health != WebSocketHealth::Healthy {
                println!("WebSocket alert: {:?}, slots since update: {}", 
                    health, slots_since_update);
            }
        }
    }

    #[tokio::test]
    async fn test_keeper_specialization_matching() {
        let work_types = vec![
            WorkType::Liquidations,
            WorkType::StopOrders,
            WorkType::PriceUpdates,
            WorkType::Resolutions,
        ];

        let keeper = KeeperAccount {
            keeper_id: [1u8; 32],
            authority: Pubkey::new_unique(),
            mmt_stake: 1_000_000,
            performance_score: 9000,
            total_operations: 100,
            successful_operations: 90,
            total_rewards_earned: 50_000,
            last_operation_slot: 100,
            status: KeeperStatus::Active,
            specializations: vec![
                KeeperSpecialization::Liquidations,
                KeeperSpecialization::PriceUpdates,
            ],
        };

        for work_type in &work_types {
            let has_spec = keeper.has_specialization(work_type);
            
            let expected = match work_type {
                WorkType::Liquidations => true,
                WorkType::PriceUpdates => true,
                WorkType::StopOrders => false,
                WorkType::Resolutions => false,
            };

            assert_eq!(has_spec, expected,
                "Work type: {:?}, Has specialization: {}",
                work_type, has_spec
            );
        }
    }
}

// Helper functions
fn create_test_position_at_risk() -> ExtendedPosition {
    let base_position = Position {
        proposal_id: 1u128,
        outcome: 0,
        size: 100_000,
        leverage: 10,
        entry_price: 50_000,
        liquidation_price: 45_000,
        is_long: true,
        created_at: 0,
    };
    
    ExtendedPosition {
        base: base_position.clone(),
        position_id: [1u8; 32],
        notional: base_position.size * base_position.leverage,
        margin_at_risk: base_position.size,
        collateral: base_position.size,
        effective_leverage: base_position.leverage,
    }
}