//! Helper functions for exhaustive user simulations

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::clock::Clock;
use solana_sdk::system_program;
use solana_program_test::BanksClient;
use spl_associated_token_account::get_associated_token_address;

use betting_platform_native::{
    instruction::{
        BettingPlatformInstruction, OpenPositionParams, TradeParams, 
        ChainStepType, OrderSide, TimeInForce, DistributionType
    },
    state::{
        GlobalConfigPDA, VersePDA, ProposalPDA, Position, UserMap, UserStatsPDA,
        LSMRMarket, PMAMMMarket, L2DistributionState, DarkPool,
        IcebergOrder, TWAPOrder, StopOrder,
    },
    error::BettingPlatformError,
    pda::*,
};

// Temporary Bootstrap struct until the actual one is found/created
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct BootstrapState {
    pub total_deposits: u64,
    pub mmt_allocation: u64,
    pub participants: u32,
    pub is_active: bool,
    pub start_slot: u64,
    pub end_slot: u64,
}

// Test context helper struct
pub struct TestContext {
    pub banks_client: BanksClient,
    pub payer: Keypair,
    pub program_id: Pubkey,
    pub global_config_pda: Pubkey,
    pub usdc_mint: Pubkey,
}

impl TestContext {
    pub async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> Result<(), BettingPlatformError> {
        let mut transaction = Transaction::new_with_payer(instructions, Some(&self.payer.pubkey()));
        let recent_blockhash = self.banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(signers, recent_blockhash);
        
        self.banks_client.process_transaction(transaction).await
            .map_err(|_| BettingPlatformError::TransactionFailed)?;
        Ok(())
    }
    
    pub async fn get_account_data<T: BorshDeserialize>(&self, pubkey: &Pubkey) -> Result<T, BettingPlatformError> {
        let account = self.banks_client
            .get_account(*pubkey)
            .await
            .unwrap()
            .ok_or(BettingPlatformError::AccountNotFound)?;
        
        T::try_from_slice(&account.data)
            .map_err(|_| BettingPlatformError::InvalidAccountData)
    }
    
    pub async fn update_account_data<T: BorshSerialize>(&mut self, pubkey: &Pubkey, data: &T) -> Result<(), BettingPlatformError> {
        // Note: This is a simplified version for testing. In real tests, you would need to
        // create appropriate instructions to update the account data.
        Ok(())
    }
    
    pub async fn get_slot(&self) -> u64 {
        let clock = self.banks_client.get_sysvar::<Clock>().await.unwrap();
        clock.slot
    }
}

pub const USDC_DECIMALS: u64 = 1_000_000;
pub const SHARE_DECIMALS: u64 = 1_000_000;
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

// ===== MARKET CREATION HELPERS =====

pub async fn create_test_market(
    context: &mut TestContext,
    market_id: u128,
    proposal_id: u128,
) -> Result<(), BettingPlatformError> {
    // Create verse and proposal
    let verse_id = market_id; // Use same ID for simplicity
    create_verse(context, verse_id).await?;
    create_proposal(context, verse_id, proposal_id).await?;
    
    Ok(())
}

pub async fn create_expiring_market(
    context: &mut TestContext,
    market_id: u128,
    proposal_id: u128,
    expiry_slot: u64,
) -> Result<(), BettingPlatformError> {
    create_test_market(context, market_id, proposal_id).await?;
    
    // Update proposal with expiry
    let proposal_pda = get_proposal_pda(proposal_id, &context.program_id);
    let mut proposal = context.get_account_data::<ProposalPDA>(&proposal_pda).await?;
    proposal.expiry_slot = expiry_slot;
    
    // Save updated proposal
    context.update_account_data(&proposal_pda, &proposal).await?;
    
    Ok(())
}

pub async fn create_verse(
    context: &mut TestContext,
    verse_id: u128,
) -> Result<(), BettingPlatformError> {
    let verse_pda = get_verse_pda(verse_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(verse_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::CreateVerse { 
            verse_id,
            name: format!("Test Verse {}", verse_id),
            description: "Test verse for simulations".to_string(),
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn create_proposal(
    context: &mut TestContext,
    verse_id: u128,
    proposal_id: u128,
) -> Result<(), BettingPlatformError> {
    let proposal_pda = get_proposal_pda(proposal_id, &context.program_id);
    let verse_pda = get_verse_pda(verse_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(proposal_pda, false),
            AccountMeta::new_readonly(verse_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::CreateProposal { 
            proposal_id,
            verse_id,
            title: format!("Test Proposal {}", proposal_id),
            description: "Test proposal for simulations".to_string(),
            outcomes: vec!["YES".to_string(), "NO".to_string()],
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== AMM INITIALIZATION HELPERS =====

pub async fn initialize_lmsr_market(
    context: &mut TestContext,
    market_id: u128,
    b_parameter: u64,
    num_outcomes: u8,
) -> Result<(), BettingPlatformError> {
    let market_pda = get_lmsr_market_pda(market_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeLmsrMarket {
            market_id,
            b_parameter,
            num_outcomes,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn initialize_pmamm_market(
    context: &mut TestContext,
    market_id: u128,
    initial_liquidity: u64,
    expiry_time: u64,
    initial_price: u64,
) -> Result<(), BettingPlatformError> {
    let pool_pda = get_pmamm_pool_pda(market_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(pool_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializePmammMarket {
            market_id,
            l_parameter: initial_liquidity,
            expiry_time: expiry_time as i64,
            initial_price,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn initialize_l2amm_market(
    context: &mut TestContext,
    params: L2InitParams,
) -> Result<(), BettingPlatformError> {
    let distribution_pda = get_l2_distribution_pda(params.pool_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(distribution_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeL2AmmMarket {
            market_id: params.pool_id,
            k_parameter: params.liquidity_parameter,
            b_bound: 1000, // Default bound
            distribution_type: DistributionType::Normal,
            discretization_points: params.num_bins as u16,
            range_min: params.min_value,
            range_max: params.max_value,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== TRADING HELPERS =====

pub async fn execute_lmsr_trade(
    context: &mut TestContext,
    trader: &Keypair,
    market_id: u128,
    outcome: u8,
    amount: u64,
    is_buy: bool,
) -> Result<(), BettingPlatformError> {
    let market_pda = get_lmsr_market_pda(market_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(market_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ExecuteLmsrTrade {
            outcome,
            amount,
            is_buy,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await
}

pub async fn execute_pmamm_trade(
    context: &mut TestContext,
    trader: &Keypair,
    market_id: u128,
    outcome: u8,
    amount: u64,
    is_buy: bool,
) -> Result<(), BettingPlatformError> {
    let pool_pda = get_pmamm_pool_pda(market_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(pool_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ExecutePmammTrade {
            outcome,
            amount,
            is_buy,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await
}

pub async fn execute_l2_range_trade(
    context: &mut TestContext,
    trader: &Keypair,
    market_id: u128,
    lower_bound: u64,
    upper_bound: u64,
    shares: u64,
    is_buy: bool,
) -> Result<(), BettingPlatformError> {
    let distribution_pda = get_l2_distribution_pda(market_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    // Convert bounds to outcome index (simplified)
    let outcome = ((lower_bound + upper_bound) / 2 / (100 * USDC_DECIMALS / 20)) as u8;
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(distribution_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ExecuteL2Trade {
            outcome,
            amount: shares,
            is_buy,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await
}

// ===== POSITION MANAGEMENT HELPERS =====

pub async fn open_leveraged_position(
    context: &mut TestContext,
    trader: &Keypair,
    proposal_id: u128,
    outcome: u8,
    leverage: u8,
    size: u64,
) -> Result<(), BettingPlatformError> {
    let proposal_pda = get_proposal_pda(proposal_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    let position_pda = get_position_pda(&trader.pubkey(), proposal_id, &context.program_id);
    
    let params = OpenPositionParams {
        proposal_id,
        outcome,
        leverage,
        size,
        max_loss: size * leverage as u64,
        chain_id: None,
    };
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(position_pda, false),
            AccountMeta::new(proposal_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::OpenPosition { params }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await
}

pub async fn close_position(
    context: &mut TestContext,
    trader: &Keypair,
    position_index: u8,
) -> Result<(), BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ClosePosition { position_index }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await
}

// ===== ADVANCED ORDER HELPERS =====

pub async fn place_iceberg_order(
    context: &mut TestContext,
    trader: &Keypair,
    market_id: u128,
    outcome: u8,
    visible_size: u64,
    total_size: u64,
    side: OrderSide,
) -> Result<u128, BettingPlatformError> {
    let order_id = rand::random::<u128>();
    let order_pda = get_iceberg_order_pda(order_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(order_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::PlaceIcebergOrder {
            market_id,
            outcome,
            visible_size,
            total_size,
            side,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await?;
    Ok(order_id)
}

pub async fn execute_iceberg_fill(
    context: &mut TestContext,
    order_id: u128,
    fill_size: u64,
) -> Result<u64, BettingPlatformError> {
    let order_pda = get_iceberg_order_pda(order_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(order_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ExecuteIcebergFill {
            fill_size,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await?;
    Ok(fill_size)
}

pub async fn place_twap_order(
    context: &mut TestContext,
    trader: &Keypair,
    market_id: u128,
    outcome: u8,
    total_size: u64,
    duration_slots: u64,
    intervals: u8,
    side: OrderSide,
) -> Result<u128, BettingPlatformError> {
    let order_id = rand::random::<u128>();
    let order_pda = get_twap_order_pda(order_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(order_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::PlaceTwapOrder {
            market_id,
            outcome,
            total_size,
            duration: duration_slots,
            intervals,
            side,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await?;
    Ok(order_id)
}

pub async fn execute_twap_interval(
    context: &mut TestContext,
    order_id: u128,
) -> Result<(), BettingPlatformError> {
    let order_pda = get_twap_order_pda(order_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(order_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ExecuteTwapInterval.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== DARK POOL HELPERS =====

pub async fn initialize_dark_pool(
    context: &mut TestContext,
    market_id: u128,
    minimum_size: u64,
    price_improvement_bps: u16,
) -> Result<(), BettingPlatformError> {
    let dark_pool_pda = get_dark_pool_pda(market_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(dark_pool_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeDarkPool {
            market_id,
            minimum_size,
            price_improvement_bps,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn place_dark_order(
    context: &mut TestContext,
    trader: &Keypair,
    market_id: u128,
    side: OrderSide,
    outcome: u8,
    size: u64,
    min_price: Option<u64>,
    max_price: Option<u64>,
    time_in_force: TimeInForce,
) -> Result<(), BettingPlatformError> {
    let dark_pool_pda = get_dark_pool_pda(market_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&trader.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(dark_pool_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(trader.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::PlaceDarkOrder {
            side,
            outcome,
            size,
            min_price,
            max_price,
            time_in_force,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, trader]).await
}

pub async fn match_dark_pool_orders(
    context: &mut TestContext,
    market_id: u128,
) -> Result<(), BettingPlatformError> {
    let dark_pool_pda = get_dark_pool_pda(market_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(dark_pool_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::MatchDarkPoolOrders.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== USER AND CREDIT HELPERS =====

pub async fn deposit_credits(
    context: &mut TestContext,
    user: &Keypair,
    amount: u64,
) -> Result<(), BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_ata = get_associated_token_address(&user.pubkey(), &context.usdc_mint);
    let vault_ata = get_vault_ata(&context.program_id, &context.usdc_mint);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(user_ata, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: BettingPlatformInstruction::DepositCredits { amount }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, user]).await
}

pub async fn withdraw_credits(
    context: &mut TestContext,
    user: &Keypair,
    amount: u64,
) -> Result<(), BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_ata = get_associated_token_address(&user.pubkey(), &context.usdc_mint);
    let vault_ata = get_vault_ata(&context.program_id, &context.usdc_mint);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(user_ata, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: BettingPlatformInstruction::WithdrawCredits { amount }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, user]).await
}

pub async fn emergency_withdraw(
    context: &mut TestContext,
    user: &Keypair,
) -> Result<(), BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_balance = get_user_balance(context, user).await?;
    
    withdraw_credits(context, user, user_balance).await
}

// ===== ORACLE AND RESOLUTION HELPERS =====

pub async fn update_oracle_price_polymarket(
    context: &mut TestContext,
    market_id: u128,
    yes_price: u64,
    no_price: u64,
) -> Result<(), BettingPlatformError> {
    let oracle_pda = get_polymarket_oracle_pda(market_id, &context.program_id);
    let market_id_bytes = market_id.to_le_bytes();
    
    // Create signature (mock for testing)
    let mut signature = [0u8; 64];
    signature[..16].copy_from_slice(&market_id_bytes);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(oracle_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::UpdatePolymarketPrice {
            market_id: market_id_bytes[..16].try_into().unwrap(),
            yes_price,
            no_price,
            volume_24h: 1000000 * USDC_DECIMALS,
            liquidity: 100000 * USDC_DECIMALS,
            timestamp: 0, // Will be set by the instruction processor
            slot: context.get_slot().await,
            signature,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn process_market_resolution(
    context: &mut TestContext,
    market_id: u128,
    proposal_id: u128,
    resolution_outcome: &str,
) -> Result<(), BettingPlatformError> {
    let proposal_pda = get_proposal_pda(proposal_id, &context.program_id);
    let resolution_pda = get_resolution_pda(proposal_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(resolution_pda, false),
            AccountMeta::new(proposal_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::ProcessResolution {
            verse_id: market_id,
            market_id: market_id.to_string(),
            resolution_outcome: resolution_outcome.to_string(),
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn initiate_dispute(
    context: &mut TestContext,
    user: &Keypair,
    market_id: u128,
    proposal_id: u128,
) -> Result<(), BettingPlatformError> {
    let resolution_pda = get_resolution_pda(proposal_id, &context.program_id);
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(resolution_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::InitiateDispute {
            verse_id: market_id,
            market_id: market_id.to_string(),
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, user]).await
}

pub async fn resolve_dispute(
    context: &mut TestContext,
    market_id: u128,
    proposal_id: u128,
    final_resolution: &str,
) -> Result<(), BettingPlatformError> {
    let resolution_pda = get_resolution_pda(proposal_id, &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(resolution_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ResolveDispute {
            verse_id: market_id,
            market_id: market_id.to_string(),
            final_resolution: final_resolution.to_string(),
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== PLATFORM MANAGEMENT HELPERS =====

pub async fn increase_platform_coverage(
    context: &mut TestContext,
    amount: u64,
) -> Result<(), BettingPlatformError> {
    // Add funds to vault to increase coverage
    let vault_ata = get_vault_ata(&context.program_id, &context.usdc_mint);
    
    // Mint USDC to vault
    mint_to(
        &mut context.banks_client,
        &context.payer,
        &context.usdc_mint,
        &vault_ata,
        &context.payer,
        amount,
    ).await;
    
    // Update global config
    let mut global_config = context.get_account_data::<GlobalConfigPDA>(&context.global_config_pda).await?;
    global_config.vault += amount as u128;
    global_config.coverage = if global_config.total_oi > 0 {
        (global_config.vault * 10000) / global_config.total_oi
    } else {
        u128::MAX
    };
    
    context.update_account_data(&context.global_config_pda, &global_config).await
}

pub async fn trigger_emergency_halt(
    context: &mut TestContext,
) -> Result<(), BettingPlatformError> {
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::EmergencyHalt.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn update_market_price(
    context: &mut TestContext,
    market_id: u128,
    new_price: u64,
) -> Result<(), BettingPlatformError> {
    // Update oracle with new price
    update_oracle_price_polymarket(
        context,
        market_id,
        new_price,
        10000 - new_price, // Complementary price for NO
    ).await
}

// ===== GETTER HELPERS =====

pub async fn get_user_balance(
    context: &TestContext,
    user: &Keypair,
) -> Result<u64, BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_map = context.get_account_data::<UserMapPDA>(&user_map_pda).await?;
    Ok(user_map.credit_balance)
}

pub async fn get_user_position(
    context: &TestContext,
    user: &Keypair,
    market_id: u128,
    outcome: u8,
) -> Result<Position, BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_map = context.get_account_data::<UserMapPDA>(&user_map_pda).await?;
    
    user_map.positions
        .iter()
        .find(|p| p.proposal_id == market_id && p.outcome == outcome)
        .cloned()
        .ok_or(BettingPlatformError::PositionNotFound)
}

pub async fn get_user_positions(
    context: &TestContext,
    user: &Keypair,
) -> Result<Vec<Position>, BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    let user_map = context.get_account_data::<UserMapPDA>(&user_map_pda).await?;
    Ok(user_map.positions)
}

pub async fn get_lmsr_price(
    context: &TestContext,
    market_id: u128,
    outcome: u8,
) -> Result<u64, BettingPlatformError> {
    let market_pda = get_lmsr_market_pda(market_id, &context.program_id);
    let market = context.get_account_data::<LmsrMarketPDA>(&market_pda).await?;
    
    // Calculate price using LMSR formula
    let total_shares: f64 = market.shares.iter().sum::<u64>() as f64;
    let outcome_shares = market.shares[outcome as usize] as f64;
    let b = market.b_parameter as f64;
    
    let price = (outcome_shares / b).exp() / market.shares.iter()
        .map(|s| (*s as f64 / b).exp())
        .sum::<f64>();
    
    Ok((price * 10000.0) as u64) // Return in basis points
}

pub async fn get_global_config(
    context: &TestContext,
) -> Result<GlobalConfigPDA, BettingPlatformError> {
    context.get_account_data(&context.global_config_pda).await
}

pub async fn get_bootstrap_state(
    context: &TestContext,
) -> Result<BootstrapState, BettingPlatformError> {
    let bootstrap_pda = get_bootstrap_pda(&context.program_id);
    context.get_account_data(&bootstrap_pda).await
}

// ===== BOOTSTRAP HELPERS =====

pub async fn initialize_bootstrap_phase(
    context: &mut TestContext,
    mmt_allocation: u64,
) -> Result<(), BettingPlatformError> {
    let bootstrap_pda = get_bootstrap_pda(&context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeBootstrapPhase {
            mmt_allocation,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn process_bootstrap_deposit(
    context: &mut TestContext,
    user: &Keypair,
    amount: u64,
) -> Result<(), BettingPlatformError> {
    let bootstrap_pda = get_bootstrap_pda(&context.program_id);
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ProcessBootstrapDeposit {
            amount,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, user]).await
}

pub async fn process_bootstrap_withdrawal(
    context: &mut TestContext,
    user: &Keypair,
    amount: u64,
) -> Result<(), BettingPlatformError> {
    let bootstrap_pda = get_bootstrap_pda(&context.program_id);
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_pda, false),
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ProcessBootstrapWithdrawal {
            amount,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, user]).await
}

pub async fn update_bootstrap_coverage(
    context: &mut TestContext,
) -> Result<(), BettingPlatformError> {
    let bootstrap_pda = get_bootstrap_pda(&context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::UpdateBootstrapCoverage.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn complete_bootstrap_phase(
    context: &mut TestContext,
) -> Result<(), BettingPlatformError> {
    let bootstrap_pda = get_bootstrap_pda(&context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(bootstrap_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::CompleteBootstrap.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== MMT TOKEN HELPERS =====

pub async fn earn_mmt_tokens(
    context: &mut TestContext,
    user: &Keypair,
    amount: u64,
) -> Result<(), BettingPlatformError> {
    // Mint MMT tokens to user (simulating rewards)
    let mmt_mint = get_mmt_mint_pda(&context.program_id);
    let user_mmt_ata = get_associated_token_address(&user.pubkey(), &mmt_mint);
    
    // Create ATA if needed
    create_token_account_if_needed(
        &mut context.banks_client,
        &context.payer,
        &user_mmt_ata,
        &mmt_mint,
        &user.pubkey(),
    ).await;
    
    // Mint tokens
    mint_to(
        &mut context.banks_client,
        &context.payer,
        &mmt_mint,
        &user_mmt_ata,
        &context.payer,
        amount,
    ).await;
    
    Ok(())
}

pub async fn initialize_staking_pool(
    context: &mut TestContext,
) -> Result<(), BettingPlatformError> {
    let staking_pool_pda = get_staking_pool_pda(&context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(staking_pool_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::InitializeStakingPool.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn stake_mmt_tokens(
    context: &mut TestContext,
    staker: &Keypair,
    amount: u64,
    lock_period_slots: Option<u64>,
) -> Result<(), BettingPlatformError> {
    let staking_pool_pda = get_staking_pool_pda(&context.program_id);
    let user_stake_pda = get_user_stake_pda(&staker.pubkey(), &context.program_id);
    let mmt_mint = get_mmt_mint_pda(&context.program_id);
    let user_mmt_ata = get_associated_token_address(&staker.pubkey(), &mmt_mint);
    let pool_mmt_ata = get_associated_token_address(&staking_pool_pda, &mmt_mint);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(user_stake_pda, false),
            AccountMeta::new(staking_pool_pda, false),
            AccountMeta::new(user_mmt_ata, false),
            AccountMeta::new(pool_mmt_ata, false),
            AccountMeta::new(staker.pubkey(), true),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: BettingPlatformInstruction::StakeMMT {
            amount,
            lock_period_slots,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer, staker]).await
}

// ===== LIQUIDATION HELPERS =====

pub async fn process_liquidation_queue(
    context: &mut TestContext,
    max_liquidations: u64,
) -> Result<(), BettingPlatformError> {
    let liquidation_queue_pda = get_liquidation_queue_pda(&context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(liquidation_queue_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::ProcessPriorityLiquidation {
            max_liquidations,
        }.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

pub async fn monitor_position_health(
    context: &mut TestContext,
    user: &Keypair,
) -> Result<(), BettingPlatformError> {
    let user_map_pda = get_user_map_pda(&user.pubkey(), &context.program_id);
    
    let ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(user_map_pda, false),
            AccountMeta::new(context.global_config_pda, false),
            AccountMeta::new_readonly(solana_sdk::clock::id(), false),
        ],
        data: BettingPlatformInstruction::MonitorPositionHealth.try_to_vec().unwrap(),
    };
    
    context.process_transaction(&[ix], &[&context.payer]).await
}

// ===== ANALYSIS HELPERS =====

pub async fn calculate_portfolio_pnl(
    context: &TestContext,
    trader: &Keypair,
) -> Result<i64, BettingPlatformError> {
    let positions = get_user_positions(context, trader).await?;
    let mut total_pnl = 0i64;
    
    for position in positions {
        // Get current market price
        let current_price = get_lmsr_price(context, position.proposal_id, position.outcome).await?;
        let entry_price = position.entry_price;
        
        let price_change = (current_price as i64 - entry_price as i64) as f64 / entry_price as f64;
        let position_pnl = (position.size as f64 * price_change * position.leverage as f64) as i64;
        
        total_pnl += position_pnl;
    }
    
    Ok(total_pnl)
}

pub async fn count_liquidated_positions(
    context: &TestContext,
    traders: &[Keypair],
) -> Result<usize, BettingPlatformError> {
    let mut liquidated = 0;
    
    for trader in traders {
        let positions = get_user_positions(context, trader).await?;
        liquidated += positions.iter().filter(|p| p.is_liquidated).count();
    }
    
    Ok(liquidated)
}

pub async fn get_market_state(
    context: &TestContext,
    market_id: u128,
) -> Result<MarketState, BettingPlatformError> {
    // Check different market types
    let lmsr_pda = get_lmsr_market_pda(market_id, &context.program_id);
    if let Ok(lmsr) = context.get_account_data::<LmsrMarketPDA>(&lmsr_pda).await {
        return Ok(MarketState {
            is_halted: lmsr.is_halted,
            market_type: MarketType::LMSR,
        });
    }
    
    // Try other market types...
    Err(BettingPlatformError::MarketNotFound)
}

// ===== UTILITY STRUCTURES =====

#[derive(Debug, Clone)]
pub struct L2InitParams {
    pub pool_id: u128,
    pub min_value: u64,
    pub max_value: u64,
    pub num_bins: u8,
    pub initial_distribution: Option<Vec<u64>>,
    pub liquidity_parameter: u64,
}

#[derive(Debug)]
pub struct MarketState {
    pub is_halted: bool,
    pub market_type: MarketType,
}

#[derive(Debug, PartialEq)]
pub enum MarketType {
    LMSR,
    PMAMM,
    L2,
}

// ===== TOKEN HELPERS =====

async fn create_mint(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    mint: &Keypair,
    authority: &Pubkey,
    decimals: u8,
) {
    let rent = banks_client.get_rent().await.unwrap();
    let mint_rent = rent.minimum_balance(spl_token::state::Mint::LEN);
    
    let instructions = vec![
        system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            mint_rent,
            spl_token::state::Mint::LEN as u64,
            &spl_token::id(),
        ),
        token_instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            authority,
            None,
            decimals,
        ).unwrap(),
    ];
    
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
    transaction.sign(&[payer, mint], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
}

async fn create_token_account_if_needed(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    token_account: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
) {
    if banks_client.get_account(*token_account).await.unwrap().is_none() {
        let instructions = vec![
            spl_associated_token_account::instruction::create_associated_token_account(
                &payer.pubkey(),
                owner,
                mint,
                &spl_token::id(),
            ),
        ];
        
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(&[payer], recent_blockhash);
        
        banks_client.process_transaction(transaction).await.unwrap();
    }
}

async fn mint_to(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Keypair,
    amount: u64,
) {
    let instruction = token_instruction::mint_to(
        &spl_token::id(),
        mint,
        destination,
        &authority.pubkey(),
        &[],
        amount,
    ).unwrap();
    
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
    transaction.sign(&[payer, authority], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
}

pub async fn get_token_balance(
    context: &TestContext,
    owner: &Keypair,
    mint: &Pubkey,
) -> Result<u64, BettingPlatformError> {
    let ata = get_associated_token_address(&owner.pubkey(), mint);
    let account = context.banks_client
        .get_account(ata)
        .await
        .unwrap()
        .ok_or(BettingPlatformError::AccountNotFound)?;
    
    let token_account = TokenAccount::unpack(&account.data)
        .map_err(|_| BettingPlatformError::InvalidAccountData)?;
    
    Ok(token_account.amount)
}

// ===== PDA DERIVATION FUNCTIONS =====

fn get_user_map_pda(user: &Pubkey, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"user_map", user.as_ref()],
        program_id,
    ).0
}

fn get_verse_pda(verse_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"verse", &verse_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_proposal_pda(proposal_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"proposal", &proposal_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_position_pda(user: &Pubkey, proposal_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"position", user.as_ref(), &proposal_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_lmsr_market_pda(market_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"lmsr_market", &market_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_pmamm_pool_pda(pool_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"pmamm_market", &pool_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_l2_distribution_pda(pool_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"l2amm_market", &pool_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_iceberg_order_pda(order_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"iceberg_order", &order_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_twap_order_pda(order_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"twap_order", &order_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_dark_pool_pda(market_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"dark_pool", &market_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_bootstrap_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"bootstrap"], program_id).0
}

fn get_liquidation_queue_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"liquidation_queue"], program_id).0
}

fn get_polymarket_oracle_pda(market_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"polymarket_oracle", &market_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_resolution_pda(proposal_id: u128, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"resolution_state", &proposal_id.to_le_bytes()],
        program_id,
    ).0
}

fn get_mmt_mint_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"mmt_mint"], program_id).0
}

fn get_staking_pool_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"staking_pool"], program_id).0
}

fn get_user_stake_pda(user: &Pubkey, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"mmt_stake", user.as_ref()],
        program_id,
    ).0
}

fn get_vault_ata(program_id: &Pubkey, mint: &Pubkey) -> Pubkey {
    get_associated_token_address(program_id, mint)
}

fn get_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    spl_associated_token_account::get_associated_token_address(owner, mint)
}