//! Polymarket Order Management Service
//! Handles the complete order lifecycle from creation to settlement

use anyhow::{Result, anyhow, Context};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use ethereum_types::{Address, U256};
use uuid::Uuid;

use crate::db::polymarket_repository::{PolymarketRepository, OrderStatus};
use crate::integration::{
    polymarket_auth::{PolymarketAuthenticator, PolymarketOrderData},
    polymarket_clob::{
        PolymarketClobClient, OrderRequest, OrderResponse, 
        OrderSide as ClobOrderSide, OrderStatus as ClobOrderStatus
    },
    polymarket_ws::{PolymarketWsClient, MarketEvent},
    polymarket_ctf::{PolymarketCtfClient, SplitPositionResult},
};

/// Order lifecycle states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderLifecycle {
    Created,
    Signed,
    Submitted,
    Acknowledged,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Failed,
    Settled,
}

/// Order side
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderParams {
    pub wallet_address: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: u8,
    pub side: OrderSide,
    pub size: Decimal,
    pub price: Decimal,
    pub order_type: String, // gtc, fok, ioc
    pub expiration: Option<u64>,
    pub fee_rate_bps: u16,
}

/// Order submission result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSubmissionResult {
    pub order_id: String,
    pub order_hash: String,
    pub status: OrderStatus,
    pub submitted_at: DateTime<Utc>,
    pub estimated_fees: Decimal,
}

/// Order tracking info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTracking {
    pub order_id: String,
    pub lifecycle: OrderLifecycle,
    pub filled_amount: Decimal,
    pub remaining_amount: Decimal,
    pub average_price: Option<Decimal>,
    pub trades: Vec<TradeExecution>,
    pub last_update: DateTime<Utc>,
}

/// Trade execution details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecution {
    pub trade_id: String,
    pub price: Decimal,
    pub size: Decimal,
    pub fee: Decimal,
    pub executed_at: DateTime<Utc>,
}

/// Polymarket Order Service
pub struct PolymarketOrderService {
    repository: Arc<PolymarketRepository>,
    clob_client: Arc<PolymarketClobClient>,
    ctf_client: Arc<PolymarketCtfClient>,
    ws_client: Arc<RwLock<PolymarketWsClient>>,
    authenticator: Arc<PolymarketAuthenticator>,
    order_cache: Arc<RwLock<OrderCache>>,
    config: OrderServiceConfig,
}

/// Service configuration
#[derive(Debug, Clone)]
pub struct OrderServiceConfig {
    pub max_order_size: Decimal,
    pub min_order_size: Decimal,
    pub max_price_slippage: Decimal,
    pub order_timeout_seconds: u64,
    pub auto_cancel_on_expiry: bool,
    pub enable_partial_fills: bool,
    pub max_retries: u32,
}

impl Default for OrderServiceConfig {
    fn default() -> Self {
        Self {
            max_order_size: Decimal::from(10000),
            min_order_size: Decimal::from(1),
            max_price_slippage: Decimal::from_str_exact("0.02").unwrap(), // 2%
            order_timeout_seconds: 3600, // 1 hour
            auto_cancel_on_expiry: true,
            enable_partial_fills: true,
            max_retries: 3,
        }
    }
}

impl PolymarketOrderService {
    /// Create new order service
    pub fn new(
        repository: Arc<PolymarketRepository>,
        clob_client: Arc<PolymarketClobClient>,
        ctf_client: Arc<PolymarketCtfClient>,
        ws_client: Arc<RwLock<PolymarketWsClient>>,
        authenticator: Arc<PolymarketAuthenticator>,
        config: OrderServiceConfig,
    ) -> Self {
        Self {
            repository,
            clob_client,
            ctf_client,
            ws_client,
            authenticator,
            order_cache: Arc::new(RwLock::new(OrderCache::new())),
            config,
        }
    }
    
    /// Create and sign a new order
    pub async fn create_order(&self, params: CreateOrderParams) -> Result<PolymarketOrderData> {
        // Validate parameters
        self.validate_order_params(&params)?;
        
        // Check user has sufficient balance
        self.check_balance(&params).await?;
        
        // Build order data
        let order_data = self.build_order_data(params)?;
        
        info!("Created order for signing: {:?}", order_data);
        
        Ok(order_data)
    }
    
    /// Submit signed order to Polymarket
    pub async fn submit_order(
        &self,
        order_data: PolymarketOrderData,
        signature: String,
    ) -> Result<OrderSubmissionResult> {
        let order_id = Uuid::new_v4().to_string();
        
        // Store order in database with pending status
        let market = self.repository
            .get_market_by_condition(&order_data.token_id)
            .await?
            .ok_or_else(|| anyhow!("Market not found"))?;
        
        let db_id = self.repository.create_order(
            &order_id,
            &format!("{:?}", order_data.maker),
            market.id,
            &order_data.token_id,
            &order_data.token_id,
            if order_data.side == 0 { "buy" } else { "sell" },
            Decimal::from_str_exact(&order_data.maker_amount)?,
            self.calculate_price(&order_data)?,
            &signature,
        ).await?;
        
        // Create order request for CLOB
        let order_request = OrderRequest {
            order: order_data.clone(),
            signature: signature.clone(),
            owner: Some(format!("{:?}", order_data.maker)),
        };
        
        // Submit to Polymarket CLOB
        let submission = self.clob_client
            .submit_order(order_request)
            .await?;
        
        // Update order status in database
        self.repository.update_order_status(
            &submission.order_id,
            OrderStatus::Open,
            None,
            None,
        ).await?;
        
        // Cache the order for tracking
        self.order_cache.write().await.add_order(
            submission.order_id.clone(),
            OrderTracking {
                order_id: submission.order_id.clone(),
                lifecycle: OrderLifecycle::Submitted,
                filled_amount: Decimal::ZERO,
                remaining_amount: Decimal::from_str_exact(&order_data.maker_amount)?,
                average_price: None,
                trades: Vec::new(),
                last_update: Utc::now(),
            },
        );
        
        // Subscribe to order updates via WebSocket
        self.ws_client.write().await
            .subscribe_orders(format!("{:?}", order_data.maker))
            .await?;
        
        info!("Order submitted successfully: {}", submission.order_id);
        
        Ok(OrderSubmissionResult {
            order_id: submission.order_id,
            order_hash: submission.order_hash,
            status: submission.status.into(), // This should work with the From impl
            submitted_at: Utc::now(),
            estimated_fees: self.calculate_fees(&order_data)?,
        })
    }
    
    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        info!("Cancelling order: {}", order_id);
        
        // Cancel on Polymarket
        let cancel_response = self.clob_client
            .cancel_order(order_id)
            .await?;
        
        // Update database
        self.repository.update_order_status(
            order_id,
            OrderStatus::Cancelled,
            None,
            None,
        ).await?;
        
        // Update cache
        if let Some(mut tracking) = self.order_cache.write().await.get_order(order_id) {
            tracking.lifecycle = OrderLifecycle::Cancelled;
            tracking.last_update = Utc::now();
        }
        
        info!("Order cancelled: {:?}", cancel_response);
        
        Ok(())
    }
    
    /// Get order status
    pub async fn get_order_status(&self, order_id: &str) -> Result<OrderTracking> {
        // Check cache first
        if let Some(tracking) = self.order_cache.read().await.get_order(order_id) {
            return Ok(tracking);
        }
        
        // Fetch from Polymarket
        let order = self.clob_client
            .get_order(order_id)
            .await?;
        
        // Update database
        self.repository.update_order_status(
            order_id,
            order.status.clone().into(),
            Some(Decimal::from_str_exact(&order.filled_amount)?),
            order.average_fill_price.map(Decimal::from_f64_retain).flatten(),
        ).await?;
        
        // Build tracking info
        let tracking = OrderTracking {
            order_id: order_id.to_string(),
            lifecycle: self.map_clob_status_to_lifecycle(&order.status),
            filled_amount: Decimal::from_str_exact(&order.filled_amount)?,
            remaining_amount: Decimal::from_str_exact(&order.remaining_amount)?,
            average_price: order.average_fill_price.map(Decimal::from_f64_retain).flatten(),
            trades: Vec::new(), // Would fetch from trades table
            last_update: order.updated_at,
        };
        
        // Update cache
        self.order_cache.write().await.add_order(order_id.to_string(), tracking.clone());
        
        Ok(tracking)
    }
    
    /// Process order update from WebSocket
    pub async fn process_order_update(&self, event: MarketEvent) -> Result<()> {
        match event {
            MarketEvent::OrderUpdate { order_id, status, filled_amount, .. } => {
                let filled = Decimal::from_str_exact(&filled_amount)?;
                
                // Update database
                let order_status = match status.as_str() {
                    "open" => OrderStatus::Open,
                    "partially_filled" => OrderStatus::PartiallyFilled,
                    "filled" => OrderStatus::Filled,
                    "cancelled" => OrderStatus::Cancelled,
                    _ => OrderStatus::Open,
                };
                
                self.repository.update_order_status(
                    &order_id,
                    order_status.clone(),
                    Some(filled),
                    None,
                ).await?;
                
                // Update cache
                if let Some(mut tracking) = self.order_cache.write().await.get_order(&order_id) {
                    tracking.filled_amount = filled;
                    tracking.lifecycle = self.map_status_to_lifecycle(&order_status);
                    tracking.last_update = Utc::now();
                }
                
                // If fully filled, process settlement
                if status == "filled" {
                    self.process_settlement(&order_id).await?;
                }
            }
            MarketEvent::Trade { trade_id, price, size, .. } => {
                // Record trade in database
                // This would be handled by trade processing
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Process settlement for filled orders
    async fn process_settlement(&self, order_id: &str) -> Result<()> {
        info!("Processing settlement for order: {}", order_id);
        
        // Get order details
        let order = self.repository.get_order(order_id)
            .await?
            .ok_or_else(|| anyhow!("Order not found"))?;
        
        // Update CTF positions
        self.repository.upsert_ctf_position(
            &order.wallet_address,
            &order.condition_id,
            &format!("{}_{}", order.condition_id, order.token_id),
            0, // outcome_index - would be determined from order
            order.filled_amount,
        ).await?;
        
        // Update lifecycle
        if let Some(mut tracking) = self.order_cache.write().await.get_order(order_id) {
            tracking.lifecycle = OrderLifecycle::Settled;
            tracking.last_update = Utc::now();
        }
        
        info!("Settlement processed for order: {}", order_id);
        
        Ok(())
    }
    
    /// Monitor and expire old orders
    pub async fn monitor_orders(&self) -> Result<()> {
        let open_orders = self.repository.get_user_open_orders("").await?; // Get all open orders
        
        for order in open_orders {
            let age = Utc::now() - order.created_at;
            
            if age > Duration::seconds(self.config.order_timeout_seconds as i64) {
                if self.config.auto_cancel_on_expiry {
                    info!("Auto-cancelling expired order: {}", order.order_id);
                    let _ = self.cancel_order(&order.order_id).await;
                }
            }
        }
        
        Ok(())
    }
    
    // Helper methods
    
    fn validate_order_params(&self, params: &CreateOrderParams) -> Result<()> {
        if params.size < self.config.min_order_size {
            return Err(anyhow!("Order size below minimum"));
        }
        
        if params.size > self.config.max_order_size {
            return Err(anyhow!("Order size above maximum"));
        }
        
        if params.price <= Decimal::ZERO || params.price >= Decimal::ONE {
            return Err(anyhow!("Invalid price (must be between 0 and 1)"));
        }
        
        Ok(())
    }
    
    async fn check_balance(&self, params: &CreateOrderParams) -> Result<()> {
        let balance = self.ctf_client
            .get_usdc_balance(&params.wallet_address)
            .await?;
        
        let required = params.size * params.price;
        let required_u256 = U256::from_dec_str(&required.to_string())?;
        
        if balance < required_u256 {
            return Err(anyhow!("Insufficient balance"));
        }
        
        Ok(())
    }
    
    fn build_order_data(&self, params: CreateOrderParams) -> Result<PolymarketOrderData> {
        let salt = U256::from(rand::random::<u64>()).to_string();
        let nonce = U256::from(Utc::now().timestamp() as u64).to_string();
        let expiration = params.expiration
            .unwrap_or_else(|| (Utc::now() + Duration::hours(24)).timestamp() as u64)
            .to_string();
        
        let maker_amount = (params.size * Decimal::from(10u64.pow(6))).to_string(); // USDC has 6 decimals
        let taker_amount = maker_amount.clone(); // Simplified
        
        Ok(PolymarketOrderData {
            salt,
            maker: params.wallet_address.parse()?,
            signer: params.wallet_address.parse()?,
            taker: Address::zero(),
            token_id: params.token_id,
            maker_amount,
            taker_amount,
            expiration,
            nonce,
            fee_rate_bps: params.fee_rate_bps.to_string(),
            side: match params.side {
                OrderSide::Buy => 0,
                OrderSide::Sell => 1,
            },
            signature_type: 0,
        })
    }
    
    fn calculate_price(&self, order: &PolymarketOrderData) -> Result<Decimal> {
        let maker = Decimal::from_str_exact(&order.maker_amount)?;
        let taker = Decimal::from_str_exact(&order.taker_amount)?;
        
        if taker.is_zero() {
            return Ok(Decimal::ZERO);
        }
        
        Ok(maker / taker)
    }
    
    fn calculate_fees(&self, order: &PolymarketOrderData) -> Result<Decimal> {
        let amount = Decimal::from_str_exact(&order.maker_amount)?;
        let fee_rate = Decimal::from_str_exact(&order.fee_rate_bps)? / Decimal::from(10000);
        
        Ok(amount * fee_rate)
    }
    
    fn map_status_to_lifecycle(&self, status: &OrderStatus) -> OrderLifecycle {
        match status {
            OrderStatus::Pending => OrderLifecycle::Created,
            OrderStatus::Open => OrderLifecycle::Open,
            OrderStatus::PartiallyFilled => OrderLifecycle::PartiallyFilled,
            OrderStatus::Filled => OrderLifecycle::Filled,
            OrderStatus::Cancelled => OrderLifecycle::Cancelled,
            OrderStatus::Expired => OrderLifecycle::Expired,
            OrderStatus::Failed => OrderLifecycle::Failed,
        }
    }
    
    fn map_clob_status_to_lifecycle(&self, status: &ClobOrderStatus) -> OrderLifecycle {
        match status {
            ClobOrderStatus::Pending => OrderLifecycle::Created,
            ClobOrderStatus::Open => OrderLifecycle::Open,
            ClobOrderStatus::PartiallyFilled => OrderLifecycle::PartiallyFilled,
            ClobOrderStatus::Filled => OrderLifecycle::Filled,
            ClobOrderStatus::Cancelled => OrderLifecycle::Cancelled,
            ClobOrderStatus::Expired => OrderLifecycle::Expired,
            ClobOrderStatus::Failed => OrderLifecycle::Failed,
        }
    }
}

/// Order cache for fast lookups
struct OrderCache {
    orders: std::collections::HashMap<String, OrderTracking>,
    max_size: usize,
}

impl OrderCache {
    fn new() -> Self {
        Self {
            orders: std::collections::HashMap::new(),
            max_size: 1000,
        }
    }
    
    fn add_order(&mut self, order_id: String, tracking: OrderTracking) {
        if self.orders.len() >= self.max_size {
            // Remove oldest order
            if let Some(oldest) = self.orders.values()
                .min_by_key(|t| t.last_update)
                .map(|t| t.order_id.clone()) {
                self.orders.remove(&oldest);
            }
        }
        
        self.orders.insert(order_id, tracking);
    }
    
    fn get_order(&self, order_id: &str) -> Option<OrderTracking> {
        self.orders.get(order_id).cloned()
    }
}

/// Batch order manager for bulk operations
pub struct BatchOrderManager {
    service: Arc<PolymarketOrderService>,
    max_batch_size: usize,
}

impl BatchOrderManager {
    pub fn new(service: Arc<PolymarketOrderService>) -> Self {
        Self {
            service,
            max_batch_size: 10,
        }
    }
    
    /// Submit multiple orders in batch
    pub async fn submit_batch(
        &self,
        orders: Vec<(PolymarketOrderData, String)>,
    ) -> Result<Vec<OrderSubmissionResult>> {
        let mut results = Vec::new();
        
        for batch in orders.chunks(self.max_batch_size) {
            let mut batch_results = Vec::new();
            
            for (order_data, signature) in batch {
                match self.service.submit_order(order_data.clone(), signature.clone()).await {
                    Ok(result) => batch_results.push(result),
                    Err(e) => {
                        error!("Failed to submit order: {}", e);
                        // Continue with other orders
                    }
                }
            }
            
            results.extend(batch_results);
            
            // Small delay between batches
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        
        Ok(results)
    }
    
    /// Cancel multiple orders
    pub async fn cancel_batch(&self, order_ids: Vec<String>) -> Result<Vec<String>> {
        let mut cancelled = Vec::new();
        
        for order_id in order_ids {
            match self.service.cancel_order(&order_id).await {
                Ok(_) => cancelled.push(order_id),
                Err(e) => {
                    error!("Failed to cancel order {}: {}", order_id, e);
                }
            }
        }
        
        Ok(cancelled)
    }
}