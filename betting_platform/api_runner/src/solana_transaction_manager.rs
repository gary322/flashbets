//! Solana transaction manager for handling transaction creation, signing, and submission

use std::{sync::Arc, collections::HashMap, time::Duration};
use tokio::sync::{RwLock, Mutex};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
    hash::Hash,
    system_program,
    compute_budget::ComputeBudgetInstruction,
};
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use borsh::{BorshSerialize, BorshDeserialize};

use crate::{
    solana_rpc_service::{SolanaRpcService, TransactionStatus},
    types::{Market, Position, OrderType},
    pda,
};

/// Transaction priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionPriority {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl TransactionPriority {
    /// Get priority fee in microlamports per compute unit
    pub fn fee_microlamports(&self) -> u64 {
        match self {
            TransactionPriority::Low => 1_000,
            TransactionPriority::Medium => 10_000,
            TransactionPriority::High => 100_000,
            TransactionPriority::VeryHigh => 1_000_000,
        }
    }
}

/// Transaction manager configuration
#[derive(Debug, Clone)]
pub struct TransactionManagerConfig {
    pub program_id: Pubkey,
    pub compute_budget_units: u32,
    pub default_priority: TransactionPriority,
    pub enable_versioned_transactions: bool,
    pub enable_priority_fees: bool,
    pub max_transaction_retries: u32,
    pub confirmation_timeout: Duration,
}

impl Default for TransactionManagerConfig {
    fn default() -> Self {
        Self {
            program_id: Pubkey::default(),
            compute_budget_units: 200_000,
            default_priority: TransactionPriority::Medium,
            enable_versioned_transactions: true,
            enable_priority_fees: true,
            max_transaction_retries: 3,
            confirmation_timeout: Duration::from_secs(30),
        }
    }
}

/// Solana transaction manager
pub struct SolanaTransactionManager {
    config: TransactionManagerConfig,
    rpc_service: Arc<SolanaRpcService>,
    recent_blockhashes: Arc<RwLock<Vec<(Hash, std::time::Instant)>>>,
    pending_transactions: Arc<RwLock<HashMap<Signature, PendingTransaction>>>,
}

/// Pending transaction information
#[derive(Debug, Clone)]
struct PendingTransaction {
    signature: Signature,
    transaction_type: String,
    created_at: std::time::Instant,
    priority: TransactionPriority,
    retries: u32,
}

/// Betting platform instructions
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum BettingInstruction {
    CreateMarket {
        market_id: u128,
        title: String,
        description: String,
        outcomes: Vec<String>,
        end_time: i64,
        creator_fee_bps: u16,
    },
    PlaceTrade {
        market_id: u128,
        outcome: u8,
        amount: u64,
        side: u8, // 0 = back, 1 = lay
        leverage: u32,
    },
    ClosePosition {
        position_id: u128,
        market_id: u128,
    },
    CreateDemoAccount {
        initial_balance: u64,
    },
    SettleMarket {
        market_id: u128,
        winning_outcome: u8,
    },
    ClaimWinnings {
        market_id: u128,
        position_id: u128,
    },
    UpdateMarketOdds {
        market_id: u128,
        outcome: u8,
        new_odds: u32,
    },
}

impl SolanaTransactionManager {
    /// Create new transaction manager
    pub fn new(
        config: TransactionManagerConfig,
        rpc_service: Arc<SolanaRpcService>,
    ) -> Self {
        let manager = Self {
            config,
            rpc_service,
            recent_blockhashes: Arc::new(RwLock::new(Vec::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Start blockhash refresh task
        manager.start_blockhash_refresh();
        
        // Start transaction monitoring task
        manager.start_transaction_monitor();
        
        manager
    }
    
    /// Start blockhash refresh task
    fn start_blockhash_refresh(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = manager.refresh_blockhash().await {
                    error!("Failed to refresh blockhash: {}", e);
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }
    
    /// Start transaction monitoring task
    fn start_transaction_monitor(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = manager.monitor_pending_transactions().await {
                    error!("Failed to monitor transactions: {}", e);
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
    
    /// Refresh recent blockhash
    async fn refresh_blockhash(&self) -> Result<()> {
        let blockhash = self.rpc_service.get_latest_blockhash().await?;
        let mut blockhashes = self.recent_blockhashes.write().await;
        
        // Keep last 5 blockhashes
        blockhashes.push((blockhash, std::time::Instant::now()));
        if blockhashes.len() > 5 {
            blockhashes.remove(0);
        }
        
        debug!("Refreshed blockhash: {}", blockhash);
        Ok(())
    }
    
    /// Get recent blockhash
    async fn get_recent_blockhash(&self) -> Result<Hash> {
        let blockhashes = self.recent_blockhashes.read().await;
        
        // Use cached blockhash if available and fresh
        if let Some((hash, time)) = blockhashes.last() {
            if time.elapsed() < Duration::from_secs(60) {
                return Ok(*hash);
            }
        }
        
        // Fetch new blockhash
        drop(blockhashes);
        self.refresh_blockhash().await?;
        
        let blockhashes = self.recent_blockhashes.read().await;
        blockhashes.last()
            .map(|(hash, _)| *hash)
            .ok_or_else(|| anyhow::anyhow!("No blockhash available"))
    }
    
    /// Monitor pending transactions
    async fn monitor_pending_transactions(&self) -> Result<()> {
        let mut pending = self.pending_transactions.write().await;
        let mut to_remove = Vec::new();
        
        for (sig, tx) in pending.iter() {
            if tx.created_at.elapsed() > self.config.confirmation_timeout {
                warn!("Transaction {} timed out", sig);
                to_remove.push(*sig);
                continue;
            }
            
            if let Ok(Some(status)) = self.rpc_service.get_transaction_status(sig).await {
                if status.confirmed {
                    info!("Transaction {} confirmed", sig);
                    to_remove.push(*sig);
                } else if let Some(err) = status.err {
                    error!("Transaction {} failed: {}", sig, err);
                    to_remove.push(*sig);
                }
            }
        }
        
        for sig in to_remove {
            pending.remove(&sig);
        }
        
        Ok(())
    }
    
    /// Create market instruction
    pub async fn create_market_instruction(
        &self,
        market_id: u128,
        creator: &Pubkey,
        title: String,
        description: String,
        outcomes: Vec<String>,
        end_time: i64,
        creator_fee_bps: u16,
    ) -> Result<Instruction> {
        let market_pda = pda::get_market_pda(&self.config.program_id, market_id);
        let global_config_pda = pda::get_global_config_pda(&self.config.program_id);
        
        let instruction_data = BettingInstruction::CreateMarket {
            market_id,
            title,
            description,
            outcomes,
            end_time,
            creator_fee_bps,
        }.try_to_vec()?;
        
        Ok(Instruction {
            program_id: self.config.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(market_pda, false),
                solana_sdk::instruction::AccountMeta::new(*creator, true),
                solana_sdk::instruction::AccountMeta::new_readonly(global_config_pda, false),
                solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: instruction_data,
        })
    }
    
    /// Create place trade instruction
    pub async fn create_place_trade_instruction(
        &self,
        market_id: u128,
        trader: &Pubkey,
        outcome: u8,
        amount: u64,
        side: u8,
        leverage: u32,
    ) -> Result<Instruction> {
        let market_pda = pda::get_market_pda(&self.config.program_id, market_id);
        let position_pda = pda::get_position_pda(&self.config.program_id, trader, market_id);
        let demo_account_pda = pda::get_demo_account_pda(&self.config.program_id, trader);
        
        let instruction_data = BettingInstruction::PlaceTrade {
            market_id,
            outcome,
            amount,
            side,
            leverage,
        }.try_to_vec()?;
        
        Ok(Instruction {
            program_id: self.config.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(market_pda, false),
                solana_sdk::instruction::AccountMeta::new(position_pda, false),
                solana_sdk::instruction::AccountMeta::new(demo_account_pda, false),
                solana_sdk::instruction::AccountMeta::new(*trader, true),
                solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: instruction_data,
        })
    }
    
    /// Create close position instruction
    pub async fn create_close_position_instruction(
        &self,
        position_id: u128,
        market_id: u128,
        trader: &Pubkey,
    ) -> Result<Instruction> {
        let market_pda = pda::get_market_pda(&self.config.program_id, market_id);
        let position_pda = pda::get_position_pda(&self.config.program_id, trader, market_id);
        let demo_account_pda = pda::get_demo_account_pda(&self.config.program_id, trader);
        
        let instruction_data = BettingInstruction::ClosePosition {
            position_id,
            market_id,
        }.try_to_vec()?;
        
        Ok(Instruction {
            program_id: self.config.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(market_pda, false),
                solana_sdk::instruction::AccountMeta::new(position_pda, false),
                solana_sdk::instruction::AccountMeta::new(demo_account_pda, false),
                solana_sdk::instruction::AccountMeta::new(*trader, true),
            ],
            data: instruction_data,
        })
    }
    
    /// Create demo account instruction
    pub async fn create_demo_account_instruction(
        &self,
        user: &Pubkey,
        initial_balance: u64,
    ) -> Result<Instruction> {
        let demo_account_pda = pda::get_demo_account_pda(&self.config.program_id, user);
        
        let instruction_data = BettingInstruction::CreateDemoAccount {
            initial_balance,
        }.try_to_vec()?;
        
        Ok(Instruction {
            program_id: self.config.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(demo_account_pda, false),
                solana_sdk::instruction::AccountMeta::new(*user, true),
                solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: instruction_data,
        })
    }
    
    /// Build transaction with priority fees
    pub async fn build_transaction(
        &self,
        instructions: Vec<Instruction>,
        payer: &Pubkey,
        priority: Option<TransactionPriority>,
    ) -> Result<Transaction> {
        let blockhash = self.get_recent_blockhash().await?;
        let priority = priority.unwrap_or(self.config.default_priority);
        
        let mut all_instructions = Vec::new();
        
        // Add compute budget instructions if enabled
        if self.config.enable_priority_fees {
            // Set compute unit limit
            all_instructions.push(
                ComputeBudgetInstruction::set_compute_unit_limit(self.config.compute_budget_units)
            );
            
            // Set priority fee
            all_instructions.push(
                ComputeBudgetInstruction::set_compute_unit_price(priority.fee_microlamports())
            );
        }
        
        // Add actual instructions
        all_instructions.extend(instructions);
        
        let transaction = Transaction::new_with_payer(
            &all_instructions,
            Some(payer),
        );
        
        Ok(transaction)
    }
    
    /// Send and confirm transaction
    pub async fn send_and_confirm_transaction(
        &self,
        transaction: Transaction,
        transaction_type: &str,
        priority: TransactionPriority,
    ) -> Result<Signature> {
        // Simulate transaction first
        self.rpc_service.simulate_transaction(&transaction).await
            .context("Transaction simulation failed")?;
        
        // Send transaction
        let signature = self.rpc_service.send_and_confirm_transaction(&transaction).await?;
        
        // Track pending transaction
        let pending_tx = PendingTransaction {
            signature,
            transaction_type: transaction_type.to_string(),
            created_at: std::time::Instant::now(),
            priority,
            retries: 0,
        };
        
        self.pending_transactions.write().await.insert(signature, pending_tx);
        
        info!("Sent {} transaction: {}", transaction_type, signature);
        
        Ok(signature)
    }
    
    /// Create and send market transaction
    pub async fn create_market(
        &self,
        creator_keypair: &Keypair,
        market_id: u128,
        title: String,
        description: String,
        outcomes: Vec<String>,
        end_time: i64,
        creator_fee_bps: u16,
    ) -> Result<Signature> {
        let instruction = self.create_market_instruction(
            market_id,
            &creator_keypair.pubkey(),
            title,
            description,
            outcomes,
            end_time,
            creator_fee_bps,
        ).await?;
        
        let mut transaction = self.build_transaction(
            vec![instruction],
            &creator_keypair.pubkey(),
            Some(TransactionPriority::High),
        ).await?;
        
        let blockhash = self.get_recent_blockhash().await?;
        transaction.sign(&[creator_keypair], blockhash);
        
        self.send_and_confirm_transaction(
            transaction,
            "create_market",
            TransactionPriority::High,
        ).await
    }
    
    /// Create and send place trade transaction
    pub async fn place_trade(
        &self,
        trader_keypair: &Keypair,
        market_id: u128,
        outcome: u8,
        amount: u64,
        side: u8,
        leverage: u32,
    ) -> Result<Signature> {
        let instruction = self.create_place_trade_instruction(
            market_id,
            &trader_keypair.pubkey(),
            outcome,
            amount,
            side,
            leverage,
        ).await?;
        
        let mut transaction = self.build_transaction(
            vec![instruction],
            &trader_keypair.pubkey(),
            Some(TransactionPriority::Medium),
        ).await?;
        
        let blockhash = self.get_recent_blockhash().await?;
        transaction.sign(&[trader_keypair], blockhash);
        
        self.send_and_confirm_transaction(
            transaction,
            "place_trade",
            TransactionPriority::Medium,
        ).await
    }
    
    /// Get transaction manager status
    pub async fn get_status(&self) -> TransactionManagerStatus {
        let pending = self.pending_transactions.read().await;
        let blockhashes = self.recent_blockhashes.read().await;
        
        TransactionManagerStatus {
            pending_transactions: pending.len(),
            cached_blockhashes: blockhashes.len(),
            config: TransactionManagerConfig {
                program_id: self.config.program_id,
                compute_budget_units: self.config.compute_budget_units,
                default_priority: self.config.default_priority,
                enable_versioned_transactions: self.config.enable_versioned_transactions,
                enable_priority_fees: self.config.enable_priority_fees,
                max_transaction_retries: self.config.max_transaction_retries,
                confirmation_timeout: self.config.confirmation_timeout,
            },
        }
    }
}

impl Clone for SolanaTransactionManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            rpc_service: self.rpc_service.clone(),
            recent_blockhashes: self.recent_blockhashes.clone(),
            pending_transactions: self.pending_transactions.clone(),
        }
    }
}

/// Transaction manager status
#[derive(Debug, Serialize)]
pub struct TransactionManagerStatus {
    pub pending_transactions: usize,
    pub cached_blockhashes: usize,
    pub config: TransactionManagerConfig,
}

// Make TransactionManagerConfig serializable
impl Serialize for TransactionManagerConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        
        let mut state = serializer.serialize_struct("TransactionManagerConfig", 7)?;
        state.serialize_field("program_id", &self.program_id.to_string())?;
        state.serialize_field("compute_budget_units", &self.compute_budget_units)?;
        state.serialize_field("default_priority", &format!("{:?}", self.default_priority))?;
        state.serialize_field("enable_versioned_transactions", &self.enable_versioned_transactions)?;
        state.serialize_field("enable_priority_fees", &self.enable_priority_fees)?;
        state.serialize_field("max_transaction_retries", &self.max_transaction_retries)?;
        state.serialize_field("confirmation_timeout_secs", &self.confirmation_timeout.as_secs())?;
        state.end()
    }
}