//! Trade execution service for processing and settling trades

use std::sync::Arc;
use std::collections::HashMap;
use std::str::FromStr;
use rust_decimal::Decimal;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
    instruction::{Instruction, AccountMeta},
    system_instruction,
    signer::Signer,
    program_pack::Pack,
};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use tokio::sync::{RwLock, Mutex};
use borsh::{BorshSerialize, BorshDeserialize};
use rust_decimal::prelude::ToPrimitive;

use crate::{
    types::{Market, Position, MarketType, MarketOutcome},
    order_types::{Order, OrderStatus, OrderType, OrderSide, TimeInForce},
    typed_errors::{AppError, ErrorKind, ErrorContext},
    solana_rpc_service::SolanaRpcService,
    trading_engine::{TradingEngine, Trade, Side},
    risk_engine::RiskEngine,
    db::fallback::FallbackDatabase,
    tracing_logger::{TracingLogger, CorrelationId},
    websocket::enhanced::{EnhancedWebSocketManager, EnhancedWsMessage},
    circuit_breaker_middleware::ServiceCircuitBreakers,
};

/// Trade execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecutionRequest {
    pub market_id: u128,
    pub user_wallet: String,
    pub side: TradeSide,
    pub outcome: u8,
    pub amount: u64,
    pub order_type: TradeOrderType,
    pub limit_price: Option<f64>,
    pub slippage_tolerance: Option<f64>, // Percentage (0.01 = 1%)
    pub time_in_force: Option<TimeInForce>,
    pub reduce_only: bool,
    pub post_only: bool,
}

/// Trade side
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Trade order type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeOrderType {
    Market,
    Limit,
    StopLimit,
    StopMarket,
}


/// Trade execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecutionResponse {
    pub trade_id: String,
    pub order_id: String,
    pub market_id: u128,
    pub user_wallet: String,
    pub side: TradeSide,
    pub outcome: u8,
    pub executed_amount: u64,
    pub average_price: f64,
    pub total_cost: u64,
    pub fees: TradeFees,
    pub status: TradeStatus,
    pub transaction_signature: Option<String>,
    pub executed_at: DateTime<Utc>,
}

/// Trade fees breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeFees {
    pub platform_fee: u64,
    pub creator_fee: u64,
    pub liquidity_fee: u64,
    pub gas_fee: u64,
    pub total_fee: u64,
}

/// Trade status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Failed,
    Expired,
}

/// Trade execution service
pub struct TradeExecutionService {
    trading_engine: Arc<TradingEngine>,
    risk_engine: Arc<RiskEngine>,
    solana_rpc: Arc<SolanaRpcService>,
    database: Arc<FallbackDatabase>,
    ws_manager: Arc<EnhancedWebSocketManager>,
    logger: Arc<TracingLogger>,
    circuit_breakers: Arc<ServiceCircuitBreakers>,
    program_id: Pubkey,
    fee_config: FeeConfiguration,
    execution_metrics: Arc<RwLock<ExecutionMetrics>>,
}

/// Fee configuration
#[derive(Debug, Clone)]
pub struct FeeConfiguration {
    pub platform_fee_bps: u16,    // Basis points (100 = 1%)
    pub min_platform_fee: u64,
    pub liquidity_fee_bps: u16,
    pub gas_subsidy_threshold: u64, // Trades above this get gas subsidy
}

impl Default for FeeConfiguration {
    fn default() -> Self {
        Self {
            platform_fee_bps: 30,      // 0.3%
            min_platform_fee: 100_000, // 0.1 USDC
            liquidity_fee_bps: 10,     // 0.1%
            gas_subsidy_threshold: 100_000_000, // 100 USDC
        }
    }
}

/// Execution metrics
#[derive(Debug, Default)]
struct ExecutionMetrics {
    pub total_trades: u64,
    pub total_volume: u64,
    pub failed_trades: u64,
    pub average_execution_time_ms: u64,
    pub slippage_events: u64,
}

impl TradeExecutionService {
    /// Create new trade execution service
    pub fn new(
        trading_engine: Arc<TradingEngine>,
        risk_engine: Arc<RiskEngine>,
        solana_rpc: Arc<SolanaRpcService>,
        database: Arc<FallbackDatabase>,
        ws_manager: Arc<EnhancedWebSocketManager>,
        logger: Arc<TracingLogger>,
        circuit_breakers: Arc<ServiceCircuitBreakers>,
        program_id: Pubkey,
    ) -> Self {
        Self {
            trading_engine,
            risk_engine,
            solana_rpc,
            database,
            ws_manager,
            logger,
            circuit_breakers,
            program_id,
            fee_config: FeeConfiguration::default(),
            execution_metrics: Arc::new(RwLock::new(ExecutionMetrics::default())),
        }
    }
    
    /// Execute trade
    pub async fn execute_trade(
        &self,
        request: TradeExecutionRequest,
        correlation_id: &CorrelationId,
    ) -> Result<TradeExecutionResponse, AppError> {
        let start_time = std::time::Instant::now();
        
        // Validate request
        self.validate_trade_request(&request)?;
        
        // Parse user wallet
        let user_pubkey = Pubkey::from_str(&request.user_wallet).map_err(|_| {
            AppError::new(
                ErrorKind::ValidationError,
                "Invalid wallet address",
                ErrorContext::new("trade_execution", "parse_wallet"),
            )
        })?;
        
        // Check risk limits
        self.check_risk_limits(&request, &user_pubkey, correlation_id).await?;
        
        // Get market info
        let market = self.get_market_info(request.market_id).await?;
        
        // Calculate fees
        let fees = self.calculate_fees(&request, &market)?;
        
        // Create order
        let order = self.create_order_from_request(&request, &user_pubkey)?;
        
        // Execute order through trading engine
        let execution_result = self.execute_order_internal(
            order,
            &market,
            correlation_id,
        ).await?;
        
        // Process on-chain if needed
        let transaction_signature = if execution_result.total_filled > 0 {
            Some(self.process_onchain_trade(
                &execution_result,
                &market,
                &user_pubkey,
                correlation_id,
            ).await?)
        } else {
            None
        };
        
        // Store trade in database
        self.store_trade_in_database(
            &execution_result,
            &request,
            &fees,
            transaction_signature.as_ref(),
        ).await?;
        
        // Update metrics
        self.update_execution_metrics(&execution_result, start_time.elapsed()).await;
        
        // Broadcast trade execution
        self.broadcast_trade_execution(&execution_result, &request).await;
        
        // Log execution
        self.logger.log_operation(
            "trade_execution",
            &correlation_id.0,
            HashMap::from([
                ("trade_id".to_string(), serde_json::json!(execution_result.trade_id)),
                ("market_id".to_string(), serde_json::json!(request.market_id)),
                ("amount".to_string(), serde_json::json!(request.amount)),
                ("executed".to_string(), serde_json::json!(execution_result.total_filled)),
            ]),
            async { Ok::<_, AppError>(()) },
        ).await?;
        
        Ok(TradeExecutionResponse {
            trade_id: execution_result.trade_id.clone(),
            order_id: execution_result.order_id.clone(),
            market_id: request.market_id,
            user_wallet: request.user_wallet,
            side: request.side,
            outcome: request.outcome,
            executed_amount: execution_result.total_filled,
            average_price: execution_result.average_price,
            total_cost: execution_result.total_cost,
            fees,
            status: match execution_result.status {
                OrderStatus::Filled => TradeStatus::Filled,
                OrderStatus::PartiallyFilled { .. } => TradeStatus::PartiallyFilled,
                OrderStatus::Cancelled => TradeStatus::Cancelled,
                _ => TradeStatus::Pending,
            },
            transaction_signature: transaction_signature.map(|s| s.to_string()),
            executed_at: Utc::now(),
        })
    }
    
    /// Cancel order
    pub async fn cancel_order(
        &self,
        order_id: &str,
        user_wallet: &str,
        correlation_id: &CorrelationId,
    ) -> Result<(), AppError> {
        // Verify order ownership
        self.verify_order_ownership(order_id, user_wallet).await?;
        
        // Cancel through trading engine
        self.trading_engine.cancel_order(order_id, user_wallet).await.map_err(|e| {
            AppError::new(
                ErrorKind::OrderRejected,
                format!("Failed to cancel order: {}", e),
                ErrorContext::new("trade_execution", "cancel_order"),
            )
        })?;
        
        // Update database
        self.update_order_status(order_id, "cancelled").await?;
        
        // Log cancellation
        self.logger.log_operation(
            "order_cancellation",
            &correlation_id.0,
            HashMap::from([
                ("order_id".to_string(), serde_json::json!(order_id)),
                ("user_wallet".to_string(), serde_json::json!(user_wallet)),
            ]),
            async { Ok::<_, AppError>(()) },
        ).await?;
        
        Ok(())
    }
    
    /// Validate trade request
    fn validate_trade_request(&self, request: &TradeExecutionRequest) -> Result<(), AppError> {
        let context = ErrorContext::new("trade_execution", "validate");
        
        // Validate amount
        if request.amount == 0 {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                "Trade amount must be greater than 0",
                context,
            ));
        }
        
        // Validate outcome
        if request.outcome > 9 {
            return Err(AppError::new(
                ErrorKind::ValidationError,
                "Invalid outcome index",
                context,
            ));
        }
        
        // Validate limit price for limit orders
        if matches!(request.order_type, TradeOrderType::Limit | TradeOrderType::StopLimit) {
            if request.limit_price.is_none() || request.limit_price.unwrap() <= 0.0 {
                return Err(AppError::new(
                    ErrorKind::ValidationError,
                    "Limit price required for limit orders",
                    context,
                ));
            }
        }
        
        // Validate slippage tolerance
        if let Some(slippage) = request.slippage_tolerance {
            if slippage < 0.0 || slippage > 1.0 {
                return Err(AppError::new(
                    ErrorKind::ValidationError,
                    "Slippage tolerance must be between 0 and 1",
                    context,
                ));
            }
        }
        
        Ok(())
    }
    
    /// Check risk limits
    async fn check_risk_limits(
        &self,
        request: &TradeExecutionRequest,
        user_pubkey: &Pubkey,
        correlation_id: &CorrelationId,
    ) -> Result<(), AppError> {
        // Check position limits
        match self.risk_engine.check_position_limit(
            &user_pubkey.to_string(),
            request.market_id,
            request.amount,
        ).await {
            Ok(_) => {}, // Position check passed
            Err(reason) => {
                return Err(AppError::new(
                    ErrorKind::ValidationError,
                    format!("Position limit exceeded: {}", reason),
                    ErrorContext::new("trade_execution", "risk_check"),
                ));
            }
        }
        
        // Check exposure limits  
        let market = self.get_market(request.market_id).await?;
        match self.risk_engine.check_exposure_limit(
            &user_pubkey.to_string(),
            request.market_id,
            request.amount,
            request.limit_price.unwrap_or(market.current_price),
        ).await {
            Ok(_) => {}, // Exposure check passed
            Err(reason) => {
                return Err(AppError::new(
                    ErrorKind::ValidationError,
                    format!("Exposure limit exceeded: {}", reason),
                    ErrorContext::new("trade_execution", "exposure_check"),
                ));
            }
        }
        
        Ok(())
    }
    
    /// Get market info
    async fn get_market_info(&self, market_id: u128) -> Result<Market, AppError> {
        // Try cache first
        // In production, implement caching
        
        // Get from database
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("trade_execution", "get_market"),
                )
            })?;
            
            let row = client.query_one(
                "SELECT * FROM markets WHERE market_id = $1",
                &[&(market_id as i64)],
            ).await.map_err(|_| {
                AppError::new(
                    ErrorKind::NotFound,
                    format!("Market {} not found", market_id),
                    ErrorContext::new("trade_execution", "market_query"),
                )
            })?;
            
            // Convert row to Market
            // This is simplified - in production, properly deserialize
            Ok(Market {
                id: market_id,
                title: row.get("title"),
                description: row.get("description"),
                creator: Pubkey::from_str(&row.get::<_, String>("creator")).unwrap(),
                outcomes: serde_json::from_value(row.get("outcomes")).unwrap(),
                amm_type: crate::types::AmmType::Cpmm, // Simplified
                total_liquidity: row.get::<_, i64>("initial_liquidity") as u64,
                total_volume: row.get::<_, Option<i64>>("total_volume").unwrap_or(0) as u64,
                resolution_time: row.get::<_, chrono::DateTime<Utc>>("resolution_time").timestamp(),
                resolved: false,
                winning_outcome: None,
                created_at: row.get::<_, chrono::DateTime<Utc>>("created_at").timestamp(),
                verse_id: None,
                current_price: 0.5, // Default price, should be calculated from AMM
            })
        } else {
            Err(AppError::new(
                ErrorKind::ServiceUnavailable,
                "Database not available",
                ErrorContext::new("trade_execution", "get_market"),
            ))
        }
    }
    
    /// Calculate fees
    fn calculate_fees(
        &self,
        request: &TradeExecutionRequest,
        market: &Market,
    ) -> Result<TradeFees, AppError> {
        let trade_value = request.amount;
        
        // Platform fee
        let platform_fee = std::cmp::max(
            (trade_value as u128 * self.fee_config.platform_fee_bps as u128 / 10_000) as u64,
            self.fee_config.min_platform_fee,
        );
        
        // Creator fee (from market)
        let creator_fee = 0; // Simplified - get from market config
        
        // Liquidity fee
        let liquidity_fee = (trade_value as u128 * self.fee_config.liquidity_fee_bps as u128 / 10_000) as u64;
        
        // Gas fee estimate
        let gas_fee = if trade_value >= self.fee_config.gas_subsidy_threshold {
            0 // Subsidized
        } else {
            5000 // 0.000005 SOL
        };
        
        Ok(TradeFees {
            platform_fee,
            creator_fee,
            liquidity_fee,
            gas_fee,
            total_fee: platform_fee + creator_fee + liquidity_fee + gas_fee,
        })
    }
    
    /// Create order from request
    fn create_order_from_request(
        &self,
        request: &TradeExecutionRequest,
        user_pubkey: &Pubkey,
    ) -> Result<Order, AppError> {
        let order_type = match request.order_type {
            TradeOrderType::Market => OrderType::Market,
            TradeOrderType::Limit => OrderType::Limit { price: request.limit_price.unwrap_or(0.0) },
            TradeOrderType::StopLimit => OrderType::Limit { price: request.limit_price.unwrap_or(0.0) }, // Simplified
            TradeOrderType::StopMarket => OrderType::Market, // Simplified
        };
        
        let side = match request.side {
            TradeSide::Buy => Side::Back,
            TradeSide::Sell => Side::Lay,
        };
        
        Ok(Order {
            id: uuid::Uuid::new_v4().to_string(),
            market_id: request.market_id,
            wallet: user_pubkey.to_string(),
            side: match request.side {
                TradeSide::Buy => OrderSide::Buy,
                TradeSide::Sell => OrderSide::Sell,
            },
            outcome: request.outcome,
            order_type,
            amount: request.amount,
            leverage: 1, // Default leverage
            filled_amount: 0,
            average_fill_price: None,
            fees: 0,
            status: OrderStatus::Open,
            time_in_force: request.time_in_force.as_ref().cloned().unwrap_or(TimeInForce::GTC),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: HashMap::new(),
            verse_id: None,
        })
    }
    
    /// Execute order internally
    async fn execute_order_internal(
        &self,
        order: Order,
        market: &Market,
        correlation_id: &CorrelationId,
    ) -> Result<ExecutionResult, AppError> {
        // Convert to trading engine order
        let trading_order = crate::trading_engine::Order {
            id: order.id.clone(),
            market_id: order.market_id,
            outcome: order.outcome,
            user_id: order.wallet.clone(),
            wallet: order.wallet.clone(),
            side: match order.side {
                OrderSide::Buy => Side::Back,
                OrderSide::Sell => Side::Lay,
            },
            order_type: match &order.order_type {
                OrderType::Market => crate::trading_engine::OrderType::Market,
                OrderType::Limit { price } => crate::trading_engine::OrderType::Limit { price: Decimal::from_f64_retain(*price).unwrap_or(Decimal::ZERO) },
                _ => crate::trading_engine::OrderType::Market, // Default to market for other types
            },
            amount: Decimal::from(order.amount),
            price: match &order.order_type {
                OrderType::Limit { price } => Some(Decimal::from_f64_retain(*price).unwrap_or(Decimal::ZERO)),
                _ => None,
            },
            time_in_force: crate::trading_engine::TimeInForce::GTC,
            status: match &order.status {
                OrderStatus::Pending => crate::trading_engine::OrderStatus::New,
                OrderStatus::Open => crate::trading_engine::OrderStatus::New,
                OrderStatus::PartiallyFilled { filled_amount, remaining_amount } => {
                    crate::trading_engine::OrderStatus::PartiallyFilled {
                        filled: Decimal::from(*filled_amount),
                        remaining: Decimal::from(*remaining_amount),
                    }
                },
                OrderStatus::Filled => crate::trading_engine::OrderStatus::Filled,
                OrderStatus::Cancelled => crate::trading_engine::OrderStatus::Cancelled,
                _ => crate::trading_engine::OrderStatus::New,
            },
            filled_amount: Decimal::from(order.filled_amount),
            average_price: order.average_fill_price.map(|p| Decimal::from_f64_retain(p).unwrap_or(Decimal::ZERO)),
            fees_paid: Decimal::from(order.fees),
            created_at: order.created_at,
            updated_at: order.updated_at,
            client_order_id: None,
        };
        
        // Place order through trading engine
        let placed_order = self.trading_engine
            .place_order(trading_order)
            .await
            .map_err(|e| {
                AppError::new(
                    ErrorKind::OrderRejected,
                    format!("Order execution failed: {}", e),
                    ErrorContext::new("trade_execution", "execute_order"),
                )
            })?;
        
        // For now, simulate execution
        // In production, this would integrate with the matching engine
        let total_filled = order.amount;
        let average_price = match &order.order_type {
            OrderType::Limit { price } => *price,
            _ => market.current_price,
        };
        let total_cost = (total_filled as f64 * average_price) as u64;
        
        // Create simulated trades
        let trades = vec![Trade {
            id: Uuid::new_v4().to_string(),
            market_id: placed_order.market_id,
            outcome: placed_order.outcome,
            price: Decimal::from_f64_retain(average_price).unwrap_or(Decimal::ZERO),
            amount: Decimal::from(total_filled),
            maker_order_id: "mock_maker".to_string(),
            taker_order_id: placed_order.id.clone(),
            maker_wallet: "mock_maker_wallet".to_string(),
            taker_wallet: placed_order.wallet.clone(),
            maker_side: match placed_order.side {
                Side::Back => Side::Lay,
                Side::Lay => Side::Back,
            },
            taker_side: placed_order.side,
            maker_fee: Decimal::ZERO,
            taker_fee: Decimal::ZERO,
            timestamp: Utc::now(),
            sequence: 1,
        }];
        
        Ok(ExecutionResult {
            order_id: placed_order.id.clone(),
            trade_id: trades.first().map(|t| t.id.clone()).unwrap_or_default(),
            total_filled,
            average_price,
            total_cost,
            status: match &placed_order.status {
                crate::trading_engine::OrderStatus::New => OrderStatus::Open,
                crate::trading_engine::OrderStatus::PartiallyFilled { filled, remaining } => {
                    OrderStatus::PartiallyFilled {
                        filled_amount: filled.to_u64().unwrap_or(0),
                        remaining_amount: remaining.to_u64().unwrap_or(0),
                    }
                },
                crate::trading_engine::OrderStatus::Filled => OrderStatus::Filled,
                crate::trading_engine::OrderStatus::Cancelled => OrderStatus::Cancelled,
                crate::trading_engine::OrderStatus::Rejected { .. } => OrderStatus::Cancelled,
                crate::trading_engine::OrderStatus::Expired => OrderStatus::Cancelled,
            },
            trades,
        })
    }
    
    /// Process on-chain trade
    async fn process_onchain_trade(
        &self,
        execution: &ExecutionResult,
        market: &Market,
        user_pubkey: &Pubkey,
        correlation_id: &CorrelationId,
    ) -> Result<Signature, AppError> {
        // Build trade instruction
        let instruction = self.build_trade_instruction(
            execution,
            market,
            user_pubkey,
        )?;
        
        // Send transaction with circuit breaker
        let signature = crate::circuit_breaker_middleware::with_solana_circuit_breaker(
            self.circuit_breakers.solana_rpc(),
            || async {
                // Build transaction
                let recent_blockhash = self.solana_rpc.get_recent_blockhash().await.map_err(|e| {
                    AppError::new(
                        ErrorKind::SolanaRpcError,
                        format!("Failed to get recent blockhash: {}", e),
                        ErrorContext::new("trade_execution", "blockhash"),
                    )
                })?;
                let payer = self.solana_rpc.get_payer_pubkey();
                
                // For now, create unsigned transaction - signing would be done by wallet
                let mut transaction = Transaction::new_with_payer(
                    &[instruction],
                    Some(&payer),
                );
                transaction.message.recent_blockhash = recent_blockhash;
                
                // Send transaction
                self.solana_rpc.send_and_confirm_transaction(&transaction).await.map_err(|e| {
                    AppError::new(
                        ErrorKind::SolanaRpcError,
                        format!("Failed to send transaction: {}", e),
                        ErrorContext::new("trade_execution", "send_transaction"),
                    )
                })
            },
        ).await?;
        
        Ok(signature)
    }
    
    /// Build trade instruction
    fn build_trade_instruction(
        &self,
        execution: &ExecutionResult,
        market: &Market,
        user_pubkey: &Pubkey,
    ) -> Result<Instruction, AppError> {
        // Serialize instruction data
        let data = TradeInstructionData {
            market_id: market.id,
            amount: execution.total_filled,
            price: (execution.average_price * 1_000_000.0) as u64, // Convert to fixed point
        };
        
        let mut instruction_data = vec![1]; // Instruction discriminator for trade
        data.serialize(&mut instruction_data).map_err(|e| {
            AppError::new(
                ErrorKind::InvalidFormat,
                format!("Failed to serialize instruction: {}", e),
                ErrorContext::new("trade_execution", "serialize"),
            )
        })?;
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(*user_pubkey, true),
                AccountMeta::new(market.creator, false), // Market account
                AccountMeta::new_readonly(self.program_id, false),
            ],
            data: instruction_data,
        })
    }
    
    /// Store trade in database
    async fn store_trade_in_database(
        &self,
        execution: &ExecutionResult,
        request: &TradeExecutionRequest,
        fees: &TradeFees,
        signature: Option<&Signature>,
    ) -> Result<(), AppError> {
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("trade_execution", "store_trade"),
                )
            })?;
            
            let query = r#"
                INSERT INTO trades (
                    trade_id, order_id, market_id, user_wallet, side, outcome,
                    amount, price, total_cost, platform_fee, creator_fee,
                    liquidity_fee, gas_fee, status, transaction_signature,
                    created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#;
            
            client.execute(
                query,
                &[
                    &execution.trade_id,
                    &execution.order_id,
                    &(request.market_id as i64),
                    &request.user_wallet,
                    &format!("{:?}", request.side),
                    &(request.outcome as i32),
                    &(execution.total_filled as i64),
                    &execution.average_price,
                    &(execution.total_cost as i64),
                    &(fees.platform_fee as i64),
                    &(fees.creator_fee as i64),
                    &(fees.liquidity_fee as i64),
                    &(fees.gas_fee as i64),
                    &format!("{:?}", execution.status),
                    &signature.map(|s| s.to_string()),
                    &Utc::now(),
                ],
            ).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to insert trade: {}", e),
                    ErrorContext::new("trade_execution", "insert"),
                )
            })?;
        }
        
        Ok(())
    }
    
    /// Update order status
    async fn update_order_status(
        &self,
        order_id: &str,
        status: &str,
    ) -> Result<(), AppError> {
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("trade_execution", "update_order"),
                )
            })?;
            
            client.execute(
                "UPDATE orders SET status = $1, updated_at = $2 WHERE order_id = $3",
                &[&status, &Utc::now(), &order_id],
            ).await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to update order: {}", e),
                    ErrorContext::new("trade_execution", "update"),
                )
            })?;
        }
        
        Ok(())
    }
    
    /// Get market details
    async fn get_market(&self, market_id: u128) -> Result<Market, AppError> {
        self.get_market_info(market_id).await
    }
    
    /// Verify order ownership
    async fn verify_order_ownership(
        &self,
        order_id: &str,
        user_wallet: &str,
    ) -> Result<(), AppError> {
        if let Ok(pool) = self.database.get_pool() {
            let client = pool.get().await.map_err(|e| {
                AppError::new(
                    ErrorKind::DatabaseError,
                    format!("Failed to get database connection: {}", e),
                    ErrorContext::new("trade_execution", "verify_ownership"),
                )
            })?;
            
            let row = client.query_one(
                "SELECT user_wallet FROM orders WHERE order_id = $1",
                &[&order_id],
            ).await.map_err(|_| {
                AppError::new(
                    ErrorKind::NotFound,
                    format!("Order {} not found", order_id),
                    ErrorContext::new("trade_execution", "verify"),
                )
            })?;
            
            let owner: String = row.get(0);
            if owner != user_wallet {
                return Err(AppError::new(
                    ErrorKind::Forbidden,
                    "Order does not belong to user",
                    ErrorContext::new("trade_execution", "ownership"),
                ));
            }
        }
        
        Ok(())
    }
    
    /// Update execution metrics
    async fn update_execution_metrics(
        &self,
        execution: &ExecutionResult,
        execution_time: std::time::Duration,
    ) {
        let mut metrics = self.execution_metrics.write().await;
        
        metrics.total_trades += 1;
        metrics.total_volume += execution.total_filled;
        
        if execution.status == OrderStatus::Cancelled {
            metrics.failed_trades += 1;
        }
        
        // Update average execution time
        let current_avg = metrics.average_execution_time_ms;
        let new_time = execution_time.as_millis() as u64;
        metrics.average_execution_time_ms = 
            (current_avg * (metrics.total_trades - 1) + new_time) / metrics.total_trades;
    }
    
    /// Broadcast trade execution
    async fn broadcast_trade_execution(
        &self,
        execution: &ExecutionResult,
        request: &TradeExecutionRequest,
    ) {
        let message = EnhancedWsMessage::TradeExecution {
            market_id: request.market_id,
            price: execution.average_price,
            size: execution.total_filled,
            side: format!("{:?}", request.side),
            timestamp: Utc::now().timestamp(),
        };
        
        self.ws_manager.broadcast_market_update(message);
    }
}

/// Execution result
struct ExecutionResult {
    order_id: String,
    trade_id: String,
    total_filled: u64,
    average_price: f64,
    total_cost: u64,
    status: OrderStatus,
    trades: Vec<Trade>,
}

/// Trade instruction data
#[derive(Debug, BorshSerialize)]
struct TradeInstructionData {
    market_id: u128,
    amount: u64,
    price: u64,
}