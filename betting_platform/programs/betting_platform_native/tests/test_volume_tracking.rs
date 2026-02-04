//! Test volume tracking in open and close position operations

#[cfg(test)]
mod tests {
    use solana_program_test::*;
    use solana_sdk::{
        account::Account,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::Transaction,
        system_instruction,
        clock::Clock,
    };
    use borsh::{BorshDeserialize, BorshSerialize};
    
    use betting_platform_native::{
        instruction::{BettingPlatformInstruction, OpenPositionParams},
        state::accounts::{Position, UserMap},
        pda::{PositionPDA, UserMapPDA},
    };
    
    async fn setup_test() -> (ProgramTest, Pubkey) {
        let program_id = Pubkey::new_unique();
        let program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(betting_platform_native::processor::process_instruction),
        );
        (program_test, program_id)
    }
    
    #[tokio::test]
    async fn test_volume_tracking_in_close_position() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize platform
        initialize_platform(&mut context, &program_id).await;
        
        // Create user and open position
        let user = Keypair::new();
        let proposal_id = 12345u128;
        let position_size = 1_000_000_000; // $1000
        let leverage = 10u8;
        
        // Fund user
        transfer_sol(&mut context, &user.pubkey(), 10_000_000_000).await;
        
        // Open position to create UserMap
        open_position(
            &mut context,
            &program_id,
            &user,
            proposal_id,
            0, // outcome
            leverage,
            position_size,
        ).await;
        
        // Check initial volume in UserMap
        let (user_map_pda, _) = UserMapPDA::derive(&program_id, &user.pubkey());
        let user_map_account = context.banks_client.get_account(user_map_pda).await.unwrap().unwrap();
        let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
        
        let expected_volume = position_size * leverage as u64;
        assert_eq!(user_map.total_volume_7d, expected_volume);
        assert!(user_map.last_volume_update > 0);
        
        // Close position
        close_position(
            &mut context,
            &program_id,
            &user,
            proposal_id,
            0, // position_index
        ).await;
        
        // Check volume was updated on close
        let user_map_account = context.banks_client.get_account(user_map_pda).await.unwrap().unwrap();
        let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
        
        // Volume should include both open and close (2x the trade volume)
        let expected_total_volume = expected_volume * 2;
        assert_eq!(user_map.total_volume_7d, expected_total_volume);
    }
    
    #[tokio::test]
    async fn test_volume_reset_after_7_days() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize platform
        initialize_platform(&mut context, &program_id).await;
        
        let user = Keypair::new();
        let proposal_id = 12345u128;
        
        // Fund user
        transfer_sol(&mut context, &user.pubkey(), 10_000_000_000).await;
        
        // Open first position
        open_position(
            &mut context,
            &program_id,
            &user,
            proposal_id,
            0,
            5,
            500_000_000, // $500
        ).await;
        
        // Get initial volume
        let (user_map_pda, _) = UserMapPDA::derive(&program_id, &user.pubkey());
        let user_map_account = context.banks_client.get_account(user_map_pda).await.unwrap().unwrap();
        let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
        let initial_volume = user_map.total_volume_7d;
        assert_eq!(initial_volume, 500_000_000 * 5); // $2500
        
        // Warp time forward by 8 days
        let mut clock = context.banks_client.get_sysvar::<Clock>().await.unwrap();
        clock.unix_timestamp += 8 * 24 * 60 * 60; // 8 days in seconds
        context.set_sysvar(&clock);
        
        // Open another position to trigger volume reset
        open_position(
            &mut context,
            &program_id,
            &user,
            proposal_id + 1,
            0,
            10,
            1_000_000_000, // $1000
        ).await;
        
        // Check volume was reset and only includes new trade
        let user_map_account = context.banks_client.get_account(user_map_pda).await.unwrap().unwrap();
        let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
        assert_eq!(user_map.total_volume_7d, 1_000_000_000 * 10); // Only new trade volume
        
        // Now close the position - should add to fresh 7-day volume
        close_position(
            &mut context,
            &program_id,
            &user,
            proposal_id + 1,
            0,
        ).await;
        
        // Verify volume includes both open and close of new position
        let user_map_account = context.banks_client.get_account(user_map_pda).await.unwrap().unwrap();
        let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
        assert_eq!(user_map.total_volume_7d, 1_000_000_000 * 10 * 2); // Open + close volume
    }
    
    #[tokio::test]
    async fn test_volume_accumulation_multiple_trades() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize platform
        initialize_platform(&mut context, &program_id).await;
        
        let user = Keypair::new();
        transfer_sol(&mut context, &user.pubkey(), 10_000_000_000).await;
        
        let trades = vec![
            (100_000_000, 5),   // $100, 5x leverage = $500 volume
            (200_000_000, 10),  // $200, 10x leverage = $2000 volume
            (300_000_000, 20),  // $300, 20x leverage = $6000 volume
        ];
        
        let mut expected_volume = 0u64;
        
        // Open and close multiple positions
        for (i, (size, leverage)) in trades.iter().enumerate() {
            let proposal_id = 10000 + i as u128;
            
            // Open position
            open_position(
                &mut context,
                &program_id,
                &user,
                proposal_id,
                0,
                *leverage,
                *size,
            ).await;
            
            expected_volume += size * (*leverage as u64);
            
            // Close position
            close_position(
                &mut context,
                &program_id,
                &user,
                proposal_id,
                0,
            ).await;
            
            expected_volume += size * (*leverage as u64);
            
            // Verify cumulative volume
            let (user_map_pda, _) = UserMapPDA::derive(&program_id, &user.pubkey());
            let user_map_account = context.banks_client.get_account(user_map_pda).await.unwrap().unwrap();
            let user_map = UserMap::try_from_slice(&user_map_account.data).unwrap();
            
            assert_eq!(
                user_map.total_volume_7d, 
                expected_volume,
                "Volume mismatch after trade {}", 
                i + 1
            );
        }
        
        // Final volume should be sum of all trades (open + close)
        // (500 + 2000 + 6000) * 2 = 17000
        assert_eq!(expected_volume, 17_000_000_000);
    }
    
    // Helper functions
    
    async fn initialize_platform(context: &mut ProgramTestContext, program_id: &Pubkey) {
        // Implementation would initialize global config, etc.
        // Simplified for test
    }
    
    async fn transfer_sol(context: &mut ProgramTestContext, to: &Pubkey, amount: u64) {
        let transfer_instruction = system_instruction::transfer(
            &context.payer.pubkey(),
            to,
            amount,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[transfer_instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
    }
    
    async fn open_position(
        context: &mut ProgramTestContext,
        program_id: &Pubkey,
        user: &Keypair,
        proposal_id: u128,
        outcome: u8,
        leverage: u8,
        size: u64,
    ) {
        let params = OpenPositionParams {
            proposal_id,
            outcome,
            leverage,
            size,
            max_loss: size,
            chain_id: None,
        };
        
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                // Add required accounts for open_position
                AccountMeta::new(user.pubkey(), true),
                // ... other accounts
            ],
            data: BettingPlatformInstruction::OpenPosition { params }
                .try_to_vec()
                .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, user],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
    }
    
    async fn close_position(
        context: &mut ProgramTestContext,
        program_id: &Pubkey,
        user: &Keypair,
        proposal_id: u128,
        position_index: u8,
    ) {
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                // Add required accounts for close_position
                AccountMeta::new(user.pubkey(), true),
                // ... other accounts
            ],
            data: BettingPlatformInstruction::ClosePosition { position_index }
                .try_to_vec()
                .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, user],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
    }
}