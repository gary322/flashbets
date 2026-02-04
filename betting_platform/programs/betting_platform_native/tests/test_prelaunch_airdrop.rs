//! Test Pre-launch Airdrop System (0.1% MMT to influencers)

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
    };
    use borsh::{BorshDeserialize, BorshSerialize};
    use spl_token::{
        state::Account as TokenAccount,
        state::Mint,
    };
    
    use betting_platform_native::{
        instruction::BettingPlatformInstruction,
        mmt::{
            prelaunch_airdrop::{
                PreLaunchAirdropConfig, InfluencerAccount,
                PreLaunchAirdropPDA, InfluencerPDA,
            },
            constants::{MMT_DECIMALS, MMT_TOTAL_SUPPLY},
        },
        error::BettingPlatformError,
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
    async fn test_initialize_prelaunch_airdrop() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize global config and MMT first
        initialize_platform(&mut context, &program_id).await;
        
        // Setup accounts
        let authority = Keypair::new();
        let claim_start_slot = 1000;
        let claim_end_slot = 10000;
        
        // Get PDAs
        let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
        let (mmt_config_pda, _) = Pubkey::find_program_address(&[b"mmt_config"], &program_id);
        
        // Initialize pre-launch airdrop
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(mmt_config_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ],
            data: BettingPlatformInstruction::InitializePreLaunchAirdrop {
                claim_start_slot,
                claim_end_slot,
            }
            .try_to_vec()
            .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &authority],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        // Verify config
        let config_account = context.banks_client.get_account(config_pda).await.unwrap().unwrap();
        let config = PreLaunchAirdropConfig::deserialize(&mut &config_account.data[..]).unwrap();
        
        assert_eq!(config.total_allocation, 100_000 * 10u64.pow(MMT_DECIMALS));
        assert_eq!(config.max_influencers, 1000);
        assert_eq!(config.allocation_per_influencer, 100 * 10u64.pow(MMT_DECIMALS));
        assert_eq!(config.claim_start_slot, claim_start_slot);
        assert_eq!(config.claim_end_slot, claim_end_slot);
        assert_eq!(config.authority, authority.pubkey());
        assert!(config.is_active);
    }
    
    #[tokio::test]
    async fn test_register_influencer() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize everything
        initialize_platform(&mut context, &program_id).await;
        let authority = initialize_airdrop(&mut context, &program_id).await;
        
        // Register influencer
        let influencer = Keypair::new();
        let social_handle = "crypto_influencer".to_string();
        let platform = 1; // Twitter
        let follower_count = 50_000;
        
        let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
        let (influencer_pda, _) = InfluencerPDA::derive(&program_id, &influencer.pubkey());
        
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(influencer_pda, false),
                AccountMeta::new_readonly(influencer.pubkey(), false),
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ],
            data: BettingPlatformInstruction::RegisterInfluencer {
                social_handle: social_handle.clone(),
                platform,
                follower_count,
            }
            .try_to_vec()
            .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &authority],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        // Verify influencer account
        let influencer_account = context.banks_client.get_account(influencer_pda).await.unwrap().unwrap();
        let influencer_data = InfluencerAccount::deserialize(&mut &influencer_account.data[..]).unwrap();
        
        assert_eq!(influencer_data.influencer, influencer.pubkey());
        assert_eq!(influencer_data.platform, platform);
        assert_eq!(influencer_data.follower_count, follower_count);
        assert!(!influencer_data.has_claimed);
        assert_eq!(influencer_data.allocation, 100 * 10u64.pow(MMT_DECIMALS)); // Base allocation
        
        // Verify config updated
        let config_account = context.banks_client.get_account(config_pda).await.unwrap().unwrap();
        let config = PreLaunchAirdropConfig::deserialize(&mut &config_account.data[..]).unwrap();
        assert_eq!(config.influencer_count, 1);
    }
    
    #[tokio::test]
    async fn test_register_influencer_with_bonus() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize everything
        initialize_platform(&mut context, &program_id).await;
        let authority = initialize_airdrop(&mut context, &program_id).await;
        
        // Test different follower tiers
        let test_cases = vec![
            (10_000, 100),      // 10k followers = 100 MMT base
            (100_000, 125),     // 100k followers = 125 MMT (25% bonus)
            (1_000_000, 150),   // 1M followers = 150 MMT (50% bonus)
        ];
        
        for (i, (follower_count, expected_mmt)) in test_cases.iter().enumerate() {
            let influencer = Keypair::new();
            let social_handle = format!("influencer_{}", i);
            
            let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
            let (influencer_pda, _) = InfluencerPDA::derive(&program_id, &influencer.pubkey());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(config_pda, false),
                    AccountMeta::new(influencer_pda, false),
                    AccountMeta::new_readonly(influencer.pubkey(), false),
                    AccountMeta::new(authority.pubkey(), true),
                    AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                    AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                    AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
                ],
                data: BettingPlatformInstruction::RegisterInfluencer {
                    social_handle,
                    platform: 1,
                    follower_count: *follower_count,
                }
                .try_to_vec()
                .unwrap(),
            };
            
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&context.payer.pubkey()),
                &[&context.payer, &authority],
                context.last_blockhash,
            );
            
            context.banks_client.process_transaction(transaction).await.unwrap();
            
            // Verify allocation
            let influencer_account = context.banks_client.get_account(influencer_pda).await.unwrap().unwrap();
            let influencer_data = InfluencerAccount::deserialize(&mut &influencer_account.data[..]).unwrap();
            
            assert_eq!(
                influencer_data.allocation, 
                expected_mmt * 10u64.pow(MMT_DECIMALS),
                "Wrong allocation for {} followers", 
                follower_count
            );
        }
    }
    
    #[tokio::test]
    async fn test_register_influencer_insufficient_followers() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize everything
        initialize_platform(&mut context, &program_id).await;
        let authority = initialize_airdrop(&mut context, &program_id).await;
        
        // Try to register with too few followers
        let influencer = Keypair::new();
        let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
        let (influencer_pda, _) = InfluencerPDA::derive(&program_id, &influencer.pubkey());
        
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(influencer_pda, false),
                AccountMeta::new_readonly(influencer.pubkey(), false),
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ],
            data: BettingPlatformInstruction::RegisterInfluencer {
                social_handle: "small_account".to_string(),
                platform: 1,
                follower_count: 5_000, // Below 10k minimum
            }
            .try_to_vec()
            .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &authority],
            context.last_blockhash,
        );
        
        let result = context.banks_client.process_transaction(transaction).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_claim_airdrop() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize everything
        initialize_platform(&mut context, &program_id).await;
        let authority = initialize_airdrop_with_times(&mut context, &program_id, 0, 100000).await;
        
        // Register and claim
        let influencer = Keypair::new();
        register_influencer(&mut context, &program_id, &authority, &influencer, 100_000).await;
        
        // Create MMT token account for influencer
        let mmt_mint_pda = get_mmt_mint_pda(&program_id);
        let influencer_token_account = create_token_account(
            &mut context,
            &influencer.pubkey(),
            &mmt_mint_pda,
        ).await;
        
        // Claim airdrop
        let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
        let (influencer_pda, _) = InfluencerPDA::derive(&program_id, &influencer.pubkey());
        let (treasury_pda, _) = Pubkey::find_program_address(&[b"mmt_treasury"], &program_id);
        
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(influencer_pda, false),
                AccountMeta::new_readonly(influencer.pubkey(), true),
                AccountMeta::new(influencer_token_account, false),
                AccountMeta::new(treasury_pda, false),
                AccountMeta::new_readonly(mmt_mint_pda, false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ],
            data: BettingPlatformInstruction::ClaimPreLaunchAirdrop
                .try_to_vec()
                .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &influencer],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        // Verify claim
        let influencer_account = context.banks_client.get_account(influencer_pda).await.unwrap().unwrap();
        let influencer_data = InfluencerAccount::deserialize(&mut &influencer_account.data[..]).unwrap();
        assert!(influencer_data.has_claimed);
        
        // Verify tokens received
        let token_account = context.banks_client.get_account(influencer_token_account).await.unwrap().unwrap();
        let token_data = TokenAccount::unpack(&token_account.data).unwrap();
        assert_eq!(token_data.amount, 125 * 10u64.pow(MMT_DECIMALS)); // 125 MMT (25% bonus)
    }
    
    #[tokio::test]
    async fn test_double_claim_fails() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize and claim once
        initialize_platform(&mut context, &program_id).await;
        let authority = initialize_airdrop_with_times(&mut context, &program_id, 0, 100000).await;
        let influencer = Keypair::new();
        register_influencer(&mut context, &program_id, &authority, &influencer, 100_000).await;
        
        let mmt_mint_pda = get_mmt_mint_pda(&program_id);
        let influencer_token_account = create_token_account(
            &mut context,
            &influencer.pubkey(),
            &mmt_mint_pda,
        ).await;
        
        // First claim
        claim_airdrop(&mut context, &program_id, &influencer, influencer_token_account).await;
        
        // Try to claim again
        let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
        let (influencer_pda, _) = InfluencerPDA::derive(&program_id, &influencer.pubkey());
        let (treasury_pda, _) = Pubkey::find_program_address(&[b"mmt_treasury"], &program_id);
        
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(influencer_pda, false),
                AccountMeta::new_readonly(influencer.pubkey(), true),
                AccountMeta::new(influencer_token_account, false),
                AccountMeta::new(treasury_pda, false),
                AccountMeta::new_readonly(mmt_mint_pda, false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ],
            data: BettingPlatformInstruction::ClaimPreLaunchAirdrop
                .try_to_vec()
                .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &influencer],
            context.last_blockhash,
        );
        
        let result = context.banks_client.process_transaction(transaction).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_end_airdrop() {
        let (mut program_test, program_id) = setup_test().await;
        let mut context = program_test.start_with_context().await;
        
        // Initialize
        initialize_platform(&mut context, &program_id).await;
        let authority = initialize_airdrop(&mut context, &program_id).await;
        
        // Register some influencers
        for i in 0..3 {
            let influencer = Keypair::new();
            register_influencer(&mut context, &program_id, &authority, &influencer, 50_000 + i * 50_000).await;
        }
        
        // End airdrop
        let (config_pda, _) = PreLaunchAirdropPDA::derive(&program_id);
        
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new_readonly(authority.pubkey(), true),
            ],
            data: BettingPlatformInstruction::EndPreLaunchAirdrop
                .try_to_vec()
                .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &authority],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        // Verify airdrop is inactive
        let config_account = context.banks_client.get_account(config_pda).await.unwrap().unwrap();
        let config = PreLaunchAirdropConfig::deserialize(&mut &config_account.data[..]).unwrap();
        assert!(!config.is_active);
        assert_eq!(config.influencer_count, 3);
    }
    
    // Helper functions
    
    async fn initialize_platform(context: &mut ProgramTestContext, program_id: &Pubkey) {
        // Initialize global config
        let seed = 12345u128;
        let (global_config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
        
        let init_instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new(global_config_pda, false),
                AccountMeta::new_readonly(context.payer.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ],
            data: BettingPlatformInstruction::Initialize { seed }.try_to_vec().unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[init_instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        // Initialize MMT
        let mmt_instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                // Add MMT initialization accounts
            ],
            data: BettingPlatformInstruction::InitializeMMTToken.try_to_vec().unwrap(),
        };
        
        // Note: In real implementation, you'd need to set up all MMT accounts properly
    }
    
    async fn initialize_airdrop(context: &mut ProgramTestContext, program_id: &Pubkey) -> Keypair {
        initialize_airdrop_with_times(context, program_id, 1000, 10000).await
    }
    
    async fn initialize_airdrop_with_times(
        context: &mut ProgramTestContext,
        program_id: &Pubkey,
        claim_start_slot: u64,
        claim_end_slot: u64,
    ) -> Keypair {
        let authority = Keypair::new();
        let (config_pda, _) = PreLaunchAirdropPDA::derive(program_id);
        let (mmt_config_pda, _) = Pubkey::find_program_address(&[b"mmt_config"], program_id);
        
        // Fund authority
        let transfer_instruction = system_instruction::transfer(
            &context.payer.pubkey(),
            &authority.pubkey(),
            1_000_000_000, // 1 SOL
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[transfer_instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        // Initialize airdrop
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(mmt_config_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ],
            data: BettingPlatformInstruction::InitializePreLaunchAirdrop {
                claim_start_slot,
                claim_end_slot,
            }
            .try_to_vec()
            .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &authority],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        authority
    }
    
    async fn register_influencer(
        context: &mut ProgramTestContext,
        program_id: &Pubkey,
        authority: &Keypair,
        influencer: &Keypair,
        follower_count: u64,
    ) {
        let (config_pda, _) = PreLaunchAirdropPDA::derive(program_id);
        let (influencer_pda, _) = InfluencerPDA::derive(program_id, &influencer.pubkey());
        
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(influencer_pda, false),
                AccountMeta::new_readonly(influencer.pubkey(), false),
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ],
            data: BettingPlatformInstruction::RegisterInfluencer {
                social_handle: format!("influencer_{}", follower_count),
                platform: 1,
                follower_count,
            }
            .try_to_vec()
            .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &authority],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
    }
    
    async fn claim_airdrop(
        context: &mut ProgramTestContext,
        program_id: &Pubkey,
        influencer: &Keypair,
        token_account: Pubkey,
    ) {
        let (config_pda, _) = PreLaunchAirdropPDA::derive(program_id);
        let (influencer_pda, _) = InfluencerPDA::derive(program_id, &influencer.pubkey());
        let (treasury_pda, _) = Pubkey::find_program_address(&[b"mmt_treasury"], program_id);
        let mmt_mint_pda = get_mmt_mint_pda(program_id);
        
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new(config_pda, false),
                AccountMeta::new(influencer_pda, false),
                AccountMeta::new_readonly(influencer.pubkey(), true),
                AccountMeta::new(token_account, false),
                AccountMeta::new(treasury_pda, false),
                AccountMeta::new_readonly(mmt_mint_pda, false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            ],
            data: BettingPlatformInstruction::ClaimPreLaunchAirdrop
                .try_to_vec()
                .unwrap(),
        };
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &influencer],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
    }
    
    fn get_mmt_mint_pda(program_id: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[b"mmt_mint"], program_id).0
    }
    
    async fn create_token_account(
        context: &mut ProgramTestContext,
        owner: &Pubkey,
        mint: &Pubkey,
    ) -> Pubkey {
        let token_account = Keypair::new();
        
        // Create account
        let rent = context.banks_client.get_rent().await.unwrap();
        let account_size = TokenAccount::LEN;
        let lamports = rent.minimum_balance(account_size);
        
        let create_instruction = system_instruction::create_account(
            &context.payer.pubkey(),
            &token_account.pubkey(),
            lamports,
            account_size as u64,
            &spl_token::id(),
        );
        
        let init_instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &token_account.pubkey(),
            mint,
            owner,
        ).unwrap();
        
        let transaction = Transaction::new_signed_with_payer(
            &[create_instruction, init_instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &token_account],
            context.last_blockhash,
        );
        
        context.banks_client.process_transaction(transaction).await.unwrap();
        
        token_account.pubkey()
    }
}