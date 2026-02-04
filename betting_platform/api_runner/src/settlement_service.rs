//! Settlement service with oracle integration
//! Handles market resolution and settlement of positions

use anyhow::{anyhow, Result, Context};
use axum::async_trait;
use borsh::{BorshSerialize, BorshDeserialize};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::{
    db::fallback::FallbackDatabase as Database,
    jwt_validation::AuthenticatedUser,
    market_creation_service::OracleSource,
    solana_rpc_service::SolanaRpcService,
    trading_engine::TradingEngine,
    tracing_logger::CorrelationId,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    websocket::enhanced::EnhancedWebSocketManager,
};

/// Database market structure
#[derive(Debug, Clone)]
pub struct Market {
    pub id: u128,
    pub pubkey: Pubkey,
    pub creator: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub outcomes: Vec<String>,
    pub total_liquidity: u64,
    pub total_volume: u64,
    pub status: String,
    pub end_time: Option<DateTime<Utc>>,
    pub resolution_time: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub current_price: f64,
}

/// Database position structure
#[derive(Debug, Clone)]
struct Position {
    id: String,
    wallet: String,
    market_id: u128,
    outcome: u8,
    shares: u64,
    locked_amount: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Settlement status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SettlementStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "oracle_pending")]
    OraclePending,
    #[serde(rename = "oracle_confirmed")]
    OracleConfirmed,
    #[serde(rename = "settling")]
    Settling,
    #[serde(rename = "settled")]
    Settled,
    #[serde(rename = "disputed")]
    Disputed,
    #[serde(rename = "failed")]
    Failed,
}

/// Oracle result for market resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleResult {
    pub oracle_name: String,
    pub outcome: u8,
    pub confidence: f64, // 0.0 to 1.0
    pub timestamp: DateTime<Utc>,
    pub proof_url: Option<String>,
    pub raw_data: Option<serde_json::Value>,
}

/// Settlement request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementRequest {
    pub market_id: u128,
    pub oracle_results: Vec<OracleResult>,
    pub admin_override: Option<u8>, // Admin can override oracle results
    pub reason: Option<String>,
}

/// Settlement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementResult {
    pub settlement_id: String,
    pub market_id: u128,
    pub winning_outcome: u8,
    pub total_positions_settled: u64,
    pub total_payout: u64,
    pub oracle_consensus: f64,
    pub settlement_time: DateTime<Utc>,
    pub transaction_signature: String,
}

/// Position settlement details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSettlement {
    pub position_id: String,
    pub wallet: String,
    pub market_id: u128,
    pub outcome: u8,
    pub shares: u64,
    pub entry_price: f64,
    pub settlement_price: f64,
    pub payout: u64,
    pub pnl: i64,
    pub fees: u64,
    pub settled_at: DateTime<Utc>,
}

/// Settlement batch for processing multiple positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementBatch {
    pub batch_id: String,
    pub market_id: u128,
    pub winning_outcome: u8,
    pub positions: Vec<PositionSettlement>,
    pub total_payout: u64,
    pub total_fees: u64,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Oracle provider trait
#[async_trait]
pub trait OracleProvider: Send + Sync {
    /// Get resolution for a market
    async fn get_resolution(&self, market: &Market) -> Result<OracleResult>;
    
    /// Verify a previous resolution
    async fn verify_resolution(&self, market_id: u128, outcome: u8) -> Result<bool>;
}

/// Settlement service
pub struct SettlementService {
    solana_rpc: Arc<SolanaRpcService>,
    database: Arc<Database>,
    trading_engine: Arc<TradingEngine>,
    ws_manager: Option<Arc<EnhancedWebSocketManager>>,
    oracle_providers: Arc<RwLock<HashMap<String, Box<dyn OracleProvider>>>>,
    program_id: Pubkey,
    settlement_authority: Pubkey,
    tx_manager: Option<Arc<crate::solana_transaction_manager::SolanaTransactionManager>>,
}

impl SettlementService {
    /// Create new settlement service
    pub fn new(
        solana_rpc: Arc<SolanaRpcService>,
        database: Arc<Database>,
        trading_engine: Arc<TradingEngine>,
        ws_manager: Option<Arc<EnhancedWebSocketManager>>,
        program_id: Pubkey,
        settlement_authority: Pubkey,
    ) -> Self {
        Self {
            solana_rpc,
            database,
            trading_engine,
            ws_manager,
            oracle_providers: Arc::new(RwLock::new(HashMap::new())),
            program_id,
            settlement_authority,
            tx_manager: None,
        }
    }
    
    /// Set transaction manager
    pub fn with_tx_manager(mut self, tx_manager: Arc<crate::solana_transaction_manager::SolanaTransactionManager>) -> Self {
        self.tx_manager = Some(tx_manager);
        self
    }
    
    /// Register an oracle provider
    pub async fn register_oracle(&self, name: String, provider: Box<dyn OracleProvider>) {
        self.oracle_providers.write().await.insert(name, provider);
    }
    
    /// Initiate market settlement
    pub async fn initiate_settlement(
        &self,
        request: SettlementRequest,
        correlation_id: &CorrelationId,
    ) -> Result<SettlementResult, AppError> {
        let context = ErrorContext::new("settlement_service", "initiate_settlement");
        
        info!(
            correlation_id = %correlation_id,
            market_id = %request.market_id,
            "Initiating market settlement"
        );
        
        // Get market from database
        let market = self.get_market(request.market_id).await
            .map_err(|e| AppError::new(
                ErrorKind::NotFound,
                format!("Market not found: {}", e),
                context.clone(),
            ))?;
        
        // Validate market can be settled
        self.validate_settlement(&market, &request)?;
        
        // Determine winning outcome from oracle results
        let (winning_outcome, consensus) = self.determine_outcome(&request.oracle_results, &market)?;
        
        // Create settlement batch
        let batch = self.create_settlement_batch(
            market.id,
            winning_outcome,
            correlation_id,
        ).await?;
        
        // Process settlement on-chain
        let signature = self.process_settlement_onchain(
            &market,
            winning_outcome,
            &batch,
            correlation_id,
        ).await?;
        
        // Update positions in database
        self.update_positions_database(&batch, &signature).await?;
        
        // Broadcast settlement event
        self.broadcast_settlement_event(&market, winning_outcome, &batch).await;
        
        Ok(SettlementResult {
            settlement_id: batch.batch_id,
            market_id: market.id,
            winning_outcome,
            total_positions_settled: batch.positions.len() as u64,
            total_payout: batch.total_payout,
            oracle_consensus: consensus,
            settlement_time: Utc::now(),
            transaction_signature: signature.to_string(),
        })
    }
    
    /// Query oracle providers for market resolution
    pub async fn query_oracles(
        &self,
        market: &Market,
        correlation_id: &CorrelationId,
    ) -> Result<Vec<OracleResult>, AppError> {
        let context = ErrorContext::new("settlement_service", "query_oracles");
        
        info!(
            correlation_id = %correlation_id,
            market_id = %market.id,
            "Querying oracles for market resolution"
        );
        
        let mut results = Vec::new();
        let providers = self.oracle_providers.read().await;
        
        for (name, provider) in providers.iter() {
            match provider.get_resolution(market).await {
                Ok(result) => {
                    info!(
                        oracle = %name,
                        outcome = %result.outcome,
                        confidence = %result.confidence,
                        "Oracle returned result"
                    );
                    results.push(result);
                },
                Err(e) => {
                    warn!(
                        oracle = %name,
                        error = %e,
                        "Oracle failed to return result"
                    );
                }
            }
        }
        
        if results.is_empty() {
            return Err(AppError::new(
                ErrorKind::ExternalServiceError,
                "No oracle results available",
                context,
            ));
        }
        
        Ok(results)
    }
    
    /// Get settlement status for a market
    pub async fn get_settlement_status(
        &self,
        market_id: u128,
    ) -> Result<SettlementStatus, AppError> {
        let context = ErrorContext::new("settlement_service", "get_settlement_status");
        
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    context.clone(),
                )
            })?;
            
            let row = client.query_one(
                "SELECT settlement_status FROM markets WHERE id = $1",
                &[&(market_id as i64)],
            ).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to query settlement status: {}", e),
                    context,
                )
            })?;
            
            let status: String = row.get(0);
            Ok(serde_json::from_str::<SettlementStatus>(&format!("\"{}\"", status))
                .unwrap_or(SettlementStatus::Pending))
        } else {
            Ok(SettlementStatus::Pending)
        }
    }
    
    /// Get positions for settlement
    async fn get_positions_for_settlement(
        &self,
        market_id: u128,
    ) -> Result<Vec<Position>, AppError> {
        let context = ErrorContext::new("settlement_service", "get_positions");
        
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    context.clone(),
                )
            })?;
            
            let rows = client.query(
                r#"
                SELECT 
                    id, wallet, market_id, outcome, shares,
                    average_price, locked_amount, created_at, updated_at
                FROM positions
                WHERE market_id = $1 AND shares > 0
                "#,
                &[&(market_id as i64)],
            ).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to query positions: {}", e),
                    context,
                )
            })?;
            
            Ok(rows.iter().map(|row| Position {
                id: row.get(0),
                wallet: row.get(1),
                market_id: row.get::<_, i64>(2) as u128,
                outcome: row.get::<_, i32>(3) as u8,
                shares: row.get::<_, i64>(4) as u64,
                locked_amount: row.get::<_, i64>(6) as u64,
                created_at: row.get(7),
                updated_at: row.get(8),
            }).collect())
        } else {
            Ok(Vec::new())
        }
    }
    
    /// Validate settlement request
    fn validate_settlement(
        &self,
        market: &Market,
        request: &SettlementRequest,
    ) -> Result<(), AppError> {
        let context = ErrorContext::new("settlement_service", "validate_settlement");
        
        // Check market status
        if market.status != "open" && market.status != "closed" {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Market cannot be settled in status: {}", market.status),
                context,
            ));
        }
        
        // Check resolution time
        if let Some(resolution_time) = market.resolution_time {
            if Utc::now() < resolution_time {
                return Err(AppError::new(
                    ErrorKind::ValidationError,
                    "Market resolution time not reached",
                    context,
                ));
            }
        }
        
        // Validate oracle results or admin override
        if request.oracle_results.is_empty() && request.admin_override.is_none() {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                "Either oracle results or admin override required",
                context,
            ));
        }
        
        Ok(())
    }
    
    /// Determine winning outcome from oracle results
    fn determine_outcome(
        &self,
        oracle_results: &[OracleResult],
        market: &Market,
    ) -> Result<(u8, f64), AppError> {
        let context = ErrorContext::new("settlement_service", "determine_outcome");
        
        if oracle_results.is_empty() {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                "No oracle results provided",
                context,
            ));
        }
        
        // Calculate weighted consensus
        let mut outcome_weights: HashMap<u8, f64> = HashMap::new();
        let mut total_weight = 0.0;
        
        for result in oracle_results {
            let weight = result.confidence;
            *outcome_weights.entry(result.outcome).or_insert(0.0) += weight;
            total_weight += weight;
        }
        
        // Find outcome with highest weight
        let (winning_outcome, weight) = outcome_weights.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .ok_or_else(|| AppError::new(
                ErrorKind::ExternalServiceError,
                "Failed to determine winning outcome",
                context.clone(),
            ))?;
        
        let consensus = weight / total_weight;
        
        // Require minimum consensus threshold
        if consensus < 0.66 {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Insufficient oracle consensus: {:.2}%", consensus * 100.0),
                context,
            ));
        }
        
        Ok((*winning_outcome, consensus))
    }
    
    /// Create settlement batch
    async fn create_settlement_batch(
        &self,
        market_id: u128,
        winning_outcome: u8,
        correlation_id: &CorrelationId,
    ) -> Result<SettlementBatch, AppError> {
        let context = ErrorContext::new("settlement_service", "create_batch");
        
        info!(
            correlation_id = %correlation_id,
            market_id = %market_id,
            winning_outcome = %winning_outcome,
            "Creating settlement batch"
        );
        
        let positions = self.get_positions_for_settlement(market_id).await?;
        let mut settlements = Vec::new();
        let mut total_payout = 0u64;
        let mut total_fees = 0u64;
        
        for position in positions {
            let (payout, fees) = if position.outcome == winning_outcome {
                // Winner gets their shares paid out at 1.0
                let payout = position.shares;
                let fees = payout / 100; // 1% settlement fee
                (payout - fees, fees)
            } else {
                // Loser gets nothing
                (0, 0)
            };
            
            settlements.push(PositionSettlement {
                position_id: position.id.clone(),
                wallet: position.wallet.clone(),
                market_id: position.market_id,
                outcome: position.outcome,
                shares: position.shares,
                entry_price: 0.5, // TODO: Get actual entry price
                settlement_price: if position.outcome == winning_outcome { 1.0 } else { 0.0 },
                payout,
                pnl: payout as i64 - position.locked_amount as i64,
                fees,
                settled_at: Utc::now(),
            });
            
            total_payout += payout;
            total_fees += fees;
        }
        
        Ok(SettlementBatch {
            batch_id: Uuid::new_v4().to_string(),
            market_id,
            winning_outcome,
            positions: settlements,
            total_payout,
            total_fees,
            created_at: Utc::now(),
            processed_at: None,
        })
    }
    
    /// Process settlement on-chain
    async fn process_settlement_onchain(
        &self,
        market: &Market,
        winning_outcome: u8,
        batch: &SettlementBatch,
        correlation_id: &CorrelationId,
    ) -> Result<Signature, AppError> {
        let context = ErrorContext::new("settlement_service", "process_onchain");
        
        info!(
            correlation_id = %correlation_id,
            market_id = %market.id,
            batch_id = %batch.batch_id,
            "Processing settlement on-chain"
        );
        
        // Build settlement instruction
        let instruction = self.build_settlement_instruction(
            market,
            winning_outcome,
            batch,
        )?;
        
        // Build and send transaction
        let tx_manager = self.tx_manager.as_ref()
            .ok_or_else(|| AppError::new(
                ErrorKind::ExternalServiceError,
                "Transaction manager not configured",
                context.clone(),
            ))?;
        
        // Build transaction
        let recent_blockhash = self.solana_rpc.get_recent_blockhash().await
            .map_err(|e| AppError::new(
                ErrorKind::SolanaRpcError,
                format!("Failed to get blockhash: {}", e),
                context.clone(),
            ))?;
            
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.settlement_authority),
            &[&Keypair::from_bytes(&[0u8; 64]).unwrap()], // Placeholder
            recent_blockhash,
        );
        
        let signature = tx_manager
            .send_and_confirm_transaction(
                transaction,
                "settlement",
                crate::solana_transaction_manager::TransactionPriority::High
            )
            .await
            .map_err(|e| AppError::new(
                ErrorKind::SolanaRpcError,
                format!("Failed to send settlement transaction: {}", e),
                context,
            ))?;
        
        info!(
            correlation_id = %correlation_id,
            signature = %signature,
            "Settlement transaction sent"
        );
        
        Ok(signature)
    }
    
    /// Build settlement instruction
    fn build_settlement_instruction(
        &self,
        market: &Market,
        winning_outcome: u8,
        batch: &SettlementBatch,
    ) -> Result<Instruction, AppError> {
        let context = ErrorContext::new("settlement_service", "build_instruction");
        
        // Settlement instruction data
        let data = borsh::to_vec(&SettlementInstructionData {
            market_id: market.id,
            winning_outcome,
            batch_id: batch.batch_id.clone(),
            total_payout: batch.total_payout,
        }).map_err(|e| AppError::new(
            ErrorKind::InternalError,
            format!("Failed to serialize instruction data: {}", e),
            context,
        ))?;
        
        // TODO: Get actual market PDA
        let market_pda = Pubkey::new_unique();
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(market_pda, false),
                AccountMeta::new_readonly(self.settlement_authority, true),
            ],
            data,
        })
    }
    
    /// Update positions in database
    async fn update_positions_database(
        &self,
        batch: &SettlementBatch,
        signature: &Signature,
    ) -> Result<(), AppError> {
        let context = ErrorContext::new("settlement_service", "update_positions");
        
        if let Ok(pool) = self.database.get_pool() {
            let mut client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    context.clone(),
                )
            })?;
            
            let transaction = client.transaction().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to start transaction: {}", e),
                    context.clone(),
                )
            })?;
            
            // Update each position
            for settlement in &batch.positions {
                transaction.execute(
                    r#"
                    UPDATE positions
                    SET 
                        settlement_price = $1,
                        payout = $2,
                        pnl = $3,
                        settlement_fees = $4,
                        settled_at = $5,
                        settlement_tx = $6,
                        status = 'settled'
                    WHERE id = $7
                    "#,
                    &[
                        &settlement.settlement_price,
                        &(settlement.payout as i64),
                        &settlement.pnl,
                        &(settlement.fees as i64),
                        &settlement.settled_at,
                        &signature.to_string(),
                        &settlement.position_id,
                    ],
                ).await.map_err(|e| {
                    AppError::new(
                        ErrorKind::DatabaseError,
                        format!("Failed to update position: {}", e),
                        context.clone(),
                    )
                })?;
            }
            
            // Update market status
            transaction.execute(
                r#"
                UPDATE markets
                SET 
                    status = 'resolved',
                    winning_outcome = $1,
                    settlement_time = $2,
                    settlement_tx = $3,
                    settlement_status = $4
                WHERE id = $5
                "#,
                &[
                    &(batch.winning_outcome as i32),
                    &Utc::now(),
                    &signature.to_string(),
                    &"settled",
                    &(batch.market_id as i64),
                ],
            ).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to update market: {}", e),
                    context.clone(),
                )
            })?;
            
            transaction.commit().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to commit transaction: {}", e),
                    context,
                )
            })?;
        }
        
        Ok(())
    }
    
    /// Broadcast settlement event via WebSocket
    async fn broadcast_settlement_event(
        &self,
        market: &Market,
        winning_outcome: u8,
        batch: &SettlementBatch,
    ) {
        if let Some(ws_manager) = &self.ws_manager {
            let event = serde_json::json!({
                "type": "market_settled",
                "market_id": market.id,
                "title": market.title,
                "winning_outcome": winning_outcome,
                "total_positions": batch.positions.len(),
                "total_payout": batch.total_payout,
                "timestamp": Utc::now(),
            });
            
            ws_manager.broadcast_system_event(
                crate::websocket::enhanced::EnhancedWsMessage::SystemEvent {
                    event_type: "market_settlement".to_string(),
                    message: format!("Market {} settled with outcome {}", market.id, winning_outcome),
                    severity: "info".to_string(),
                    timestamp: Utc::now().timestamp(),
                }
            );
        }
    }
    
    /// Get market from database
    async fn get_market(&self, market_id: u128) -> Result<Market> {
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await?;
            
            let row = client.query_one(
                r#"
                SELECT 
                    id, creator, title, description, 
                    category, tags, end_time, resolution_time,
                    status, total_volume, total_liquidity,
                    outcome_count, created_at, current_price
                FROM markets
                WHERE id = $1
                "#,
                &[&(market_id as i64)],
            ).await?;
            
            Ok(Market {
                id: row.get::<_, i64>(0) as u128,
                pubkey: Pubkey::new_unique(), // TODO: Get actual pubkey
                creator: row.get(1),
                title: row.get(2),
                description: row.get(3),
                category: row.get(4),
                outcomes: vec![], // TODO: Load outcomes
                total_liquidity: row.get::<_, i64>(10) as u64,
                total_volume: row.get::<_, i64>(9) as u64,
                status: row.get(8),
                end_time: row.get(6),
                resolution_time: row.get(7),
                created_at: row.get(12),
                current_price: row.get(13),
            })
        } else {
            Err(anyhow!("Database not available"))
        }
    }
}

/// Settlement instruction data for Borsh serialization
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
struct SettlementInstructionData {
    market_id: u128,
    winning_outcome: u8,
    batch_id: String,
    total_payout: u64,
}

/// HTTP oracle provider implementation
pub struct HttpOracleProvider {
    name: String,
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl HttpOracleProvider {
    pub fn new(name: String, base_url: String, api_key: Option<String>) -> Self {
        Self {
            name,
            base_url,
            api_key,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }
}

#[async_trait]
impl OracleProvider for HttpOracleProvider {
    async fn get_resolution(&self, market: &Market) -> Result<OracleResult> {
        // TODO: Implement actual HTTP oracle logic
        // This is a placeholder implementation
        Ok(OracleResult {
            oracle_name: self.name.clone(),
            outcome: 0,
            confidence: 0.95,
            timestamp: Utc::now(),
            proof_url: Some(format!("{}/proofs/{}", self.base_url, market.id)),
            raw_data: None,
        })
    }
    
    async fn verify_resolution(&self, market_id: u128, outcome: u8) -> Result<bool> {
        // TODO: Implement verification logic
        Ok(true)
    }
}