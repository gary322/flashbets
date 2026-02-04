//! Market creation service for managing prediction markets

use std::sync::Arc;
use std::collections::HashMap;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
    instruction::{Instruction, AccountMeta},
    system_instruction,
    signer::Signer,
};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, Duration};
use tokio::sync::RwLock;
use borsh::BorshSerialize;

use crate::{
    types::{Market, MarketOutcome, AmmType},
    typed_errors::{AppError, ErrorKind, ErrorContext},
    solana_rpc_service::SolanaRpcService,
    db::fallback::FallbackDatabase,
    tracing_logger::{TracingLogger, CorrelationId},
    websocket::enhanced::{EnhancedWebSocketManager, EnhancedWsMessage},
};

/// Market creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMarketRequest {
    pub title: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub end_time: DateTime<Utc>,
    pub resolution_time: DateTime<Utc>,
    pub category: String,
    pub tags: Vec<String>,
    pub amm_type: AmmType,
    pub initial_liquidity: u64,
    pub creator_fee_bps: u16, // Basis points (0-10000)
    pub platform_fee_bps: u16,
    pub min_bet_amount: u64,
    pub max_bet_amount: u64,
    pub oracle_sources: Vec<OracleSource>,
}

/// Oracle source for market resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSource {
    pub name: String,
    pub url: String,
    pub weight: u8, // 0-100
}

/// Market creation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMarketResponse {
    pub market_id: u128,
    pub market_address: Pubkey,
    pub transaction_signature: String,
    pub created_at: DateTime<Utc>,
}

/// Market update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMarketRequest {
    pub market_id: u128,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub min_bet_amount: Option<u64>,
    pub max_bet_amount: Option<u64>,
}

/// Market validation rules
#[derive(Debug, Clone)]
pub struct MarketValidationRules {
    pub min_outcomes: usize,
    pub max_outcomes: usize,
    pub min_title_length: usize,
    pub max_title_length: usize,
    pub min_description_length: usize,
    pub max_description_length: usize,
    pub min_duration_hours: i64,
    pub max_duration_hours: i64,
    pub min_initial_liquidity: u64,
    pub max_creator_fee_bps: u16,
    pub max_platform_fee_bps: u16,
}

impl Default for MarketValidationRules {
    fn default() -> Self {
        Self {
            min_outcomes: 2,
            max_outcomes: 10,
            min_title_length: 10,
            max_title_length: 200,
            min_description_length: 20,
            max_description_length: 1000,
            min_duration_hours: 1,
            max_duration_hours: 365 * 24, // 1 year
            min_initial_liquidity: 1_000_000, // 1 USDC
            max_creator_fee_bps: 500, // 5%
            max_platform_fee_bps: 300, // 3%
        }
    }
}

/// Market creation service
pub struct MarketCreationService {
    solana_rpc: Arc<SolanaRpcService>,
    database: Arc<FallbackDatabase>,
    ws_manager: Arc<EnhancedWebSocketManager>,
    logger: Arc<TracingLogger>,
    program_id: Pubkey,
    validation_rules: MarketValidationRules,
    market_counter: Arc<RwLock<u128>>,
    pending_markets: Arc<RwLock<HashMap<u128, CreateMarketRequest>>>,
}

impl MarketCreationService {
    /// Create new market creation service
    pub fn new(
        solana_rpc: Arc<SolanaRpcService>,
        database: Arc<FallbackDatabase>,
        ws_manager: Arc<EnhancedWebSocketManager>,
        logger: Arc<TracingLogger>,
        program_id: Pubkey,
    ) -> Self {
        Self {
            solana_rpc,
            database,
            ws_manager,
            logger,
            program_id,
            validation_rules: MarketValidationRules::default(),
            market_counter: Arc::new(RwLock::new(1000)), // Start from 1000
            pending_markets: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Create new market
    pub async fn create_market(
        &self,
        request: CreateMarketRequest,
        creator: Pubkey,
        correlation_id: &CorrelationId,
    ) -> Result<CreateMarketResponse, AppError> {
        // Validate request
        self.validate_market_request(&request)?;
        
        // Generate market ID
        let market_id = self.generate_market_id().await;
        
        // Store pending market
        self.pending_markets.write().await.insert(market_id, request.clone());
        
        // Create market account
        let market_keypair = Keypair::new();
        let market_address = market_keypair.pubkey();
        
        // Build create market instruction
        let instruction = self.build_create_market_instruction(
            market_id,
            &request,
            creator,
            market_address,
        )?;
        
        // Build transaction
        let recent_blockhash = self.solana_rpc.get_recent_blockhash().await.map_err(|e| {
            AppError::new(
                ErrorKind::SolanaRpcError,
                format!("Failed to get recent blockhash: {}", e),
                ErrorContext::new("market_creation", "blockhash"),
            )
        })?;
        let payer = self.solana_rpc.get_payer_pubkey();
        
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer),
            &[&market_keypair],
            recent_blockhash,
        );
        
        // Send transaction
        let signature = self.solana_rpc.send_and_confirm_transaction(&transaction).await.map_err(|e| {
            AppError::new(
                ErrorKind::SolanaRpcError,
                format!("Failed to send transaction: {}", e),
                ErrorContext::new("market_creation", "send_transaction"),
            )
        })?;
        
        // Store market in database
        self.store_market_in_database(
            market_id,
            &request,
            creator,
            market_address,
            &signature,
        ).await?;
        
        // Broadcast market creation
        self.broadcast_market_creation(market_id, &request, creator).await;
        
        // Remove from pending
        self.pending_markets.write().await.remove(&market_id);
        
        // Log market creation
        self.logger.log_operation(
            "market_creation",
            &correlation_id.0,
            HashMap::from([
                ("market_id".to_string(), serde_json::json!(market_id)),
                ("title".to_string(), serde_json::json!(request.title)),
                ("creator".to_string(), serde_json::json!(creator.to_string())),
            ]),
            async { Ok::<_, AppError>(()) },
        ).await?;
        
        Ok(CreateMarketResponse {
            market_id,
            market_address,
            transaction_signature: signature.to_string(),
            created_at: Utc::now(),
        })
    }
    
    /// Update existing market
    pub async fn update_market(
        &self,
        request: UpdateMarketRequest,
        updater: Pubkey,
        correlation_id: &CorrelationId,
    ) -> Result<(), AppError> {
        // Verify market exists and updater has permission
        self.verify_update_permission(request.market_id, updater).await?;
        
        // Update market in database
        self.update_market_in_database(&request).await?;
        
        // Broadcast market update
        self.broadcast_market_update(request.market_id, &request).await;
        
        // Log update
        self.logger.log_operation(
            "market_update",
            &correlation_id.0,
            HashMap::from([
                ("market_id".to_string(), serde_json::json!(request.market_id)),
                ("updater".to_string(), serde_json::json!(updater.to_string())),
            ]),
            async { Ok::<_, AppError>(()) },
        ).await?;
        
        Ok(())
    }
    
    /// Validate market creation request
    fn validate_market_request(&self, request: &CreateMarketRequest) -> Result<(), AppError> {
        let rules = &self.validation_rules;
        let context = ErrorContext::new("market_creation", "validate");
        
        // Validate outcomes
        if request.outcomes.len() < rules.min_outcomes {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Market must have at least {} outcomes", rules.min_outcomes),
                context,
            ));
        }
        
        if request.outcomes.len() > rules.max_outcomes {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Market cannot have more than {} outcomes", rules.max_outcomes),
                context,
            ));
        }
        
        // Validate title
        if request.title.len() < rules.min_title_length {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Title must be at least {} characters", rules.min_title_length),
                context,
            ));
        }
        
        if request.title.len() > rules.max_title_length {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Title cannot exceed {} characters", rules.max_title_length),
                context,
            ));
        }
        
        // Validate description
        if request.description.len() < rules.min_description_length {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Description must be at least {} characters", rules.min_description_length),
                context,
            ));
        }
        
        if request.description.len() > rules.max_description_length {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Description cannot exceed {} characters", rules.max_description_length),
                context,
            ));
        }
        
        // Validate duration
        let duration = request.end_time - Utc::now();
        if duration < Duration::hours(rules.min_duration_hours) {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Market must run for at least {} hours", rules.min_duration_hours),
                context,
            ));
        }
        
        if duration > Duration::hours(rules.max_duration_hours) {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Market cannot run for more than {} hours", rules.max_duration_hours),
                context,
            ));
        }
        
        // Validate liquidity
        if request.initial_liquidity < rules.min_initial_liquidity {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Initial liquidity must be at least {}", rules.min_initial_liquidity),
                context,
            ));
        }
        
        // Validate fees
        if request.creator_fee_bps > rules.max_creator_fee_bps {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Creator fee cannot exceed {} bps", rules.max_creator_fee_bps),
                context,
            ));
        }
        
        if request.platform_fee_bps > rules.max_platform_fee_bps {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Platform fee cannot exceed {} bps", rules.max_platform_fee_bps),
                context,
            ));
        }
        
        // Validate oracle sources
        if request.oracle_sources.is_empty() {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                "At least one oracle source is required",
                context,
            ));
        }
        
        let total_weight: u16 = request.oracle_sources.iter()
            .map(|o| o.weight as u16)
            .sum();
        
        if total_weight != 100 {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                "Oracle weights must sum to 100",
                context,
            ));
        }
        
        Ok(())
    }
    
    /// Generate unique market ID
    async fn generate_market_id(&self) -> u128 {
        let mut counter = self.market_counter.write().await;
        let id = *counter;
        *counter += 1;
        id
    }
    
    /// Build create market instruction
    fn build_create_market_instruction(
        &self,
        market_id: u128,
        request: &CreateMarketRequest,
        creator: Pubkey,
        market_address: Pubkey,
    ) -> Result<Instruction, AppError> {
        // Serialize instruction data
        let data = CreateMarketInstructionData {
            market_id,
            title: request.title.clone(),
            outcomes: request.outcomes.len() as u8,
            end_time: request.end_time.timestamp(),
            creator_fee_bps: request.creator_fee_bps,
            platform_fee_bps: request.platform_fee_bps,
            min_bet_amount: request.min_bet_amount,
            max_bet_amount: request.max_bet_amount,
            amm_type: match request.amm_type {
                AmmType::Lmsr => 0,
                AmmType::PmAmm => 1,
                AmmType::L2Amm => 2,
                AmmType::Hybrid => 3,
                AmmType::Cpmm => 4,
            },
        };
        
        let mut instruction_data = vec![0]; // Instruction discriminator
        data.serialize(&mut instruction_data).map_err(|e| {
            AppError::new(
                ErrorKind::InvalidFormat,
                format!("Failed to serialize instruction: {}", e),
                ErrorContext::new("market_creation", "serialize"),
            )
        })?;
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(creator, true),
                AccountMeta::new(market_address, true),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: instruction_data,
        })
    }
    
    /// Store market in database
    async fn store_market_in_database(
        &self,
        market_id: u128,
        request: &CreateMarketRequest,
        creator: Pubkey,
        market_address: Pubkey,
        signature: &Signature,
    ) -> Result<(), AppError> {
        if let Ok(pool) = self.database.get_pool() {
            let query = r#"
                INSERT INTO markets (
                    market_id, title, description, creator, market_address,
                    outcomes, end_time, resolution_time, category, tags,
                    amm_type, initial_liquidity, creator_fee_bps, platform_fee_bps,
                    min_bet_amount, max_bet_amount, oracle_sources,
                    transaction_signature, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            "#;
            
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("market_creation", "database"),
                )
            })?;
            
            client.execute(
                query,
                &[
                    &(market_id as i64),
                    &request.title,
                    &request.description,
                    &creator.to_string(),
                    &market_address.to_string(),
                    &serde_json::to_value(&request.outcomes).unwrap(),
                    &request.end_time,
                    &request.resolution_time,
                    &request.category,
                    &request.tags,
                    &format!("{:?}", request.amm_type),
                    &(request.initial_liquidity as i64),
                    &(request.creator_fee_bps as i32),
                    &(request.platform_fee_bps as i32),
                    &(request.min_bet_amount as i64),
                    &(request.max_bet_amount as i64),
                    &serde_json::to_value(&request.oracle_sources).unwrap(),
                    &signature.to_string(),
                    &Utc::now(),
                ],
            ).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to insert market: {}", e),
                    ErrorContext::new("market_creation", "insert"),
                )
            })?;
        }
        
        Ok(())
    }
    
    /// Update market in database
    async fn update_market_in_database(
        &self,
        request: &UpdateMarketRequest,
    ) -> Result<(), AppError> {
        if let Ok(pool) = self.database.get_pool() {
            let mut query = String::from("UPDATE markets SET updated_at = $1");
            let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = vec![
                Box::new(Utc::now()),
            ];
            let mut param_count = 2;
            
            if let Some(title) = &request.title {
                query.push_str(&format!(", title = ${}", param_count));
                params.push(Box::new(title.clone()));
                param_count += 1;
            }
            
            if let Some(description) = &request.description {
                query.push_str(&format!(", description = ${}", param_count));
                params.push(Box::new(description.clone()));
                param_count += 1;
            }
            
            if let Some(tags) = &request.tags {
                query.push_str(&format!(", tags = ${}", param_count));
                params.push(Box::new(tags.clone()));
                param_count += 1;
            }
            
            if let Some(min_bet) = request.min_bet_amount {
                query.push_str(&format!(", min_bet_amount = ${}", param_count));
                params.push(Box::new(min_bet as i64));
                param_count += 1;
            }
            
            if let Some(max_bet) = request.max_bet_amount {
                query.push_str(&format!(", max_bet_amount = ${}", param_count));
                params.push(Box::new(max_bet as i64));
                param_count += 1;
            }
            
            query.push_str(&format!(" WHERE market_id = ${}", param_count));
            params.push(Box::new(request.market_id as i64));
            
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("market_update", "database"),
                )
            })?;
            
            let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = 
                params.iter().map(|p| p.as_ref()).collect();
            
            client.execute(&query, &params_refs).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to update market: {}", e),
                    ErrorContext::new("market_update", "execute"),
                )
            })?;
        }
        
        Ok(())
    }
    
    /// Verify update permission
    async fn verify_update_permission(
        &self,
        market_id: u128,
        updater: Pubkey,
    ) -> Result<(), AppError> {
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("market_update", "verify"),
                )
            })?;
            
            let row = client.query_one(
                "SELECT creator FROM markets WHERE market_id = $1",
                &[&(market_id as i64)],
            ).await.map_err(|_| {
                AppError::new(
                    ErrorKind::NotFound,
                    format!("Market {} not found", market_id),
                    ErrorContext::new("market_update", "verify"),
                )
            })?;
            
            let creator: String = row.get(0);
            if creator != updater.to_string() {
                return Err(AppError::new(
                    ErrorKind::Forbidden,
                    "Only market creator can update market",
                    ErrorContext::new("market_update", "permission"),
                ));
            }
        }
        
        Ok(())
    }
    
    /// Broadcast market creation via WebSocket
    async fn broadcast_market_creation(
        &self,
        market_id: u128,
        request: &CreateMarketRequest,
        creator: Pubkey,
    ) {
        let message = EnhancedWsMessage::SystemEvent {
            event_type: "market_created".to_string(),
            message: format!("New market created: {}", request.title),
            severity: "info".to_string(),
            timestamp: Utc::now().timestamp(),
        };
        
        self.ws_manager.broadcast_system_event(message);
        
        // Also broadcast market update
        let market_msg = EnhancedWsMessage::MarketUpdate {
            market_id,
            yes_price: 0.5, // Initial price
            no_price: 0.5,
            volume: 0,
            liquidity: request.initial_liquidity,
            trades_24h: 0,
            timestamp: Utc::now().timestamp(),
        };
        
        self.ws_manager.broadcast_market_update(market_msg);
    }
    
    /// Broadcast market update via WebSocket
    async fn broadcast_market_update(
        &self,
        market_id: u128,
        request: &UpdateMarketRequest,
    ) {
        let message = EnhancedWsMessage::SystemEvent {
            event_type: "market_updated".to_string(),
            message: format!("Market {} updated", market_id),
            severity: "info".to_string(),
            timestamp: Utc::now().timestamp(),
        };
        
        self.ws_manager.broadcast_system_event(message);
    }
}

/// Instruction data for creating market
#[derive(Debug, BorshSerialize)]
struct CreateMarketInstructionData {
    market_id: u128,
    title: String,
    outcomes: u8,
    end_time: i64,
    creator_fee_bps: u16,
    platform_fee_bps: u16,
    min_bet_amount: u64,
    max_bet_amount: u64,
    amm_type: u8,
}