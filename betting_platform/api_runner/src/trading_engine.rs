//! Production-ready trading engine with order matching

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn, error};

use crate::{
    types::{Market, MarketOutcome},
    websocket::enhanced::{WsServerMessage, TradeData, OrderData, OrderLevel},
};

/// Order side for trading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Back, // Betting for an outcome (buy)
    Lay,  // Betting against an outcome (sell)
}

/// Order type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit { price: Decimal },
    PostOnly { price: Decimal }, // Maker-only order
}

/// Time in force for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,  // Good Till Cancelled
    IOC,  // Immediate Or Cancel
    FOK,  // Fill Or Kill
    GTD(DateTime<Utc>), // Good Till Date
}

/// Order status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled { filled: Decimal, remaining: Decimal },
    Filled,
    Cancelled,
    Rejected { reason: String },
    Expired,
}

/// Trading order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub market_id: u128,
    pub outcome: u8,
    pub user_id: String,
    pub wallet: String,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: Decimal,
    pub price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub status: OrderStatus,
    pub filled_amount: Decimal,
    pub average_price: Option<Decimal>,
    pub fees_paid: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub client_order_id: Option<String>,
}

impl Order {
    pub fn new(
        market_id: u128,
        outcome: u8,
        user_id: String,
        wallet: String,
        side: Side,
        order_type: OrderType,
        amount: Decimal,
        time_in_force: TimeInForce,
        client_order_id: Option<String>,
    ) -> Self {
        let now = Utc::now();
        let price = match &order_type {
            OrderType::Limit { price } | OrderType::PostOnly { price } => Some(*price),
            OrderType::Market => None,
        };
        
        Self {
            id: Uuid::new_v4().to_string(),
            market_id,
            outcome,
            user_id,
            wallet,
            side,
            order_type,
            amount,
            price,
            time_in_force,
            status: OrderStatus::New,
            filled_amount: Decimal::ZERO,
            average_price: None,
            fees_paid: Decimal::ZERO,
            created_at: now,
            updated_at: now,
            client_order_id,
        }
    }
    
    pub fn remaining_amount(&self) -> Decimal {
        self.amount - self.filled_amount
    }
    
    pub fn is_filled(&self) -> bool {
        matches!(self.status, OrderStatus::Filled)
    }
    
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::New | OrderStatus::PartiallyFilled { .. }
        )
    }
}

/// Trade execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub market_id: u128,
    pub outcome: u8,
    pub price: Decimal,
    pub amount: Decimal,
    pub maker_order_id: String,
    pub taker_order_id: String,
    pub maker_wallet: String,
    pub taker_wallet: String,
    pub maker_side: Side,
    pub taker_side: Side,
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}

/// Order book level
#[derive(Debug, Clone)]
struct BookLevel {
    price: Decimal,
    orders: VecDeque<Order>,
}

impl BookLevel {
    fn new(price: Decimal) -> Self {
        Self {
            price,
            orders: VecDeque::new(),
        }
    }
    
    fn total_amount(&self) -> Decimal {
        self.orders.iter()
            .map(|o| o.remaining_amount())
            .sum()
    }
    
    fn order_count(&self) -> usize {
        self.orders.len()
    }
}

/// Order book for a market outcome
#[derive(Debug)]
struct OutcomeOrderBook {
    market_id: u128,
    outcome: u8,
    backs: BTreeMap<Decimal, BookLevel>, // Price -> Level (descending)
    lays: BTreeMap<Decimal, BookLevel>,  // Price -> Level (ascending)
    sequence: u64,
}

impl OutcomeOrderBook {
    fn new(market_id: u128, outcome: u8) -> Self {
        Self {
            market_id,
            outcome,
            backs: BTreeMap::new(),
            lays: BTreeMap::new(),
            sequence: 0,
        }
    }
    
    fn add_order(&mut self, order: Order) {
        if let Some(price) = order.price {
            let book = match order.side {
                Side::Back => &mut self.backs,
                Side::Lay => &mut self.lays,
            };
            
            book.entry(price)
                .or_insert_with(|| BookLevel::new(price))
                .orders
                .push_back(order);
            
            self.sequence += 1;
        }
    }
    
    fn remove_order(&mut self, order_id: &str) -> Option<Order> {
        // Try backs first
        for (price, level) in self.backs.iter_mut() {
            if let Some(pos) = level.orders.iter().position(|o| o.id == order_id) {
                let order = level.orders.remove(pos)?;
                if level.orders.is_empty() {
                    let price = *price;
                    self.backs.remove(&price);
                }
                self.sequence += 1;
                return Some(order);
            }
        }
        
        // Try lays
        for (price, level) in self.lays.iter_mut() {
            if let Some(pos) = level.orders.iter().position(|o| o.id == order_id) {
                let order = level.orders.remove(pos)?;
                if level.orders.is_empty() {
                    let price = *price;
                    self.lays.remove(&price);
                }
                self.sequence += 1;
                return Some(order);
            }
        }
        
        None
    }
    
    fn get_best_back(&self) -> Option<Decimal> {
        self.backs.keys().next_back().copied()
    }
    
    fn get_best_lay(&self) -> Option<Decimal> {
        self.lays.keys().next().copied()
    }
    
    fn get_depth(&self, side: Side, levels: usize) -> Vec<(Decimal, Decimal, usize)> {
        let book = match side {
            Side::Back => &self.backs,
            Side::Lay => &self.lays,
        };
        
        let iter: Box<dyn Iterator<Item = (&Decimal, &BookLevel)>> = match side {
            Side::Back => Box::new(book.iter().rev()),
            Side::Lay => Box::new(book.iter()),
        };
        
        iter.take(levels)
            .map(|(price, level)| (*price, level.total_amount(), level.order_count()))
            .collect()
    }
}

/// Fee structure
#[derive(Debug, Clone)]
pub struct FeeStructure {
    pub maker_fee_rate: Decimal,
    pub taker_fee_rate: Decimal,
    pub min_fee: Decimal,
}

impl Default for FeeStructure {
    fn default() -> Self {
        Self {
            maker_fee_rate: Decimal::from_str("0.001").unwrap(), // 0.1%
            taker_fee_rate: Decimal::from_str("0.002").unwrap(), // 0.2%
            min_fee: Decimal::from_str("0.01").unwrap(),
        }
    }
}

/// Trading engine configuration
#[derive(Debug, Clone)]
pub struct TradingEngineConfig {
    pub fee_structure: FeeStructure,
    pub min_order_size: Decimal,
    pub max_order_size: Decimal,
    pub price_tick_size: Decimal,
    pub enable_post_only: bool,
    pub enable_self_trade_prevention: bool,
}

impl Default for TradingEngineConfig {
    fn default() -> Self {
        Self {
            fee_structure: FeeStructure::default(),
            min_order_size: Decimal::from_str("1.0").unwrap(),
            max_order_size: Decimal::from_str("1000000.0").unwrap(),
            price_tick_size: Decimal::from_str("0.01").unwrap(),
            enable_post_only: true,
            enable_self_trade_prevention: true,
        }
    }
}

/// Production-ready trading engine
pub struct TradingEngine {
    config: TradingEngineConfig,
    order_books: Arc<RwLock<HashMap<(u128, u8), OutcomeOrderBook>>>,
    orders: Arc<RwLock<HashMap<String, Order>>>,
    user_orders: Arc<RwLock<HashMap<String, Vec<String>>>>,
    trades: Arc<Mutex<Vec<Trade>>>,
    trade_sequence: Arc<Mutex<u64>>,
    ws_manager: Option<Arc<crate::websocket::enhanced::EnhancedWebSocketManager>>,
}

impl TradingEngine {
    pub fn new(
        config: TradingEngineConfig,
        ws_manager: Option<Arc<crate::websocket::enhanced::EnhancedWebSocketManager>>,
    ) -> Self {
        Self {
            config,
            order_books: Arc::new(RwLock::new(HashMap::new())),
            orders: Arc::new(RwLock::new(HashMap::new())),
            user_orders: Arc::new(RwLock::new(HashMap::new())),
            trades: Arc::new(Mutex::new(Vec::new())),
            trade_sequence: Arc::new(Mutex::new(0)),
            ws_manager,
        }
    }
    
    /// Place a new order
    pub async fn place_order(&self, mut order: Order) -> Result<Order, String> {
        // Validate order
        self.validate_order(&order)?;
        
        // Round price to tick size
        if let Some(price) = order.price {
            order.price = Some(self.round_price(price));
        }
        
        info!(
            "Placing order: {} {} {} @ {:?} for market {} outcome {}",
            order.side as i32,
            order.amount,
            order.id,
            order.price,
            order.market_id,
            order.outcome
        );
        
        // Store order
        {
            let mut orders = self.orders.write().await;
            orders.insert(order.id.clone(), order.clone());
            
            let mut user_orders = self.user_orders.write().await;
            user_orders.entry(order.user_id.clone())
                .or_insert_with(Vec::new)
                .push(order.id.clone());
        }
        
        // Process order based on type
        let result = match &order.order_type {
            OrderType::Market => self.process_market_order(order).await,
            OrderType::Limit { .. } => self.process_limit_order(order).await,
            OrderType::PostOnly { .. } => self.process_post_only_order(order).await,
        }?;
        
        // Update stored order
        {
            let mut orders = self.orders.write().await;
            if let Some(stored) = orders.get_mut(&result.id) {
                *stored = result.clone();
            }
        }
        
        // Broadcast order update
        self.broadcast_order_update(&result).await;
        
        Ok(result)
    }
    
    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str, user_id: &str) -> Result<Order, String> {
        let mut orders = self.orders.write().await;
        
        let order = orders.get_mut(order_id)
            .ok_or_else(|| "Order not found".to_string())?;
        
        if order.user_id != user_id {
            return Err("Unauthorized".to_string());
        }
        
        if !order.is_active() {
            return Err("Order is not active".to_string());
        }
        
        // Remove from order book
        let mut order_books = self.order_books.write().await;
        let key = (order.market_id, order.outcome);
        if let Some(book) = order_books.get_mut(&key) {
            book.remove_order(order_id);
        }
        
        // Update status
        order.status = OrderStatus::Cancelled;
        order.updated_at = Utc::now();
        
        let cancelled_order = order.clone();
        drop(orders);
        drop(order_books);
        
        // Broadcast cancellation
        self.broadcast_order_update(&cancelled_order).await;
        
        info!("Order {} cancelled", order_id);
        
        Ok(cancelled_order)
    }
    
    /// Get order book for a market outcome
    pub async fn get_order_book(
        &self,
        market_id: u128,
        outcome: u8,
        depth: usize,
    ) -> OrderBookSnapshot {
        let order_books = self.order_books.read().await;
        let key = (market_id, outcome);
        
        if let Some(book) = order_books.get(&key) {
            let backs = book.get_depth(Side::Back, depth);
            let lays = book.get_depth(Side::Lay, depth);
            
            OrderBookSnapshot {
                market_id,
                outcome,
                backs: backs.into_iter()
                    .map(|(price, amount, count)| PriceLevel { price, amount, orders: count })
                    .collect(),
                lays: lays.into_iter()
                    .map(|(price, amount, count)| PriceLevel { price, amount, orders: count })
                    .collect(),
                sequence: book.sequence,
                timestamp: Utc::now(),
            }
        } else {
            OrderBookSnapshot {
                market_id,
                outcome,
                backs: Vec::new(),
                lays: Vec::new(),
                sequence: 0,
                timestamp: Utc::now(),
            }
        }
    }
    
    /// Get user orders
    pub async fn get_user_orders(&self, user_id: &str) -> Vec<Order> {
        let user_orders = self.user_orders.read().await;
        let orders = self.orders.read().await;
        
        if let Some(order_ids) = user_orders.get(user_id) {
            order_ids.iter()
                .filter_map(|id| orders.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get recent trades
    pub async fn get_recent_trades(&self, market_id: u128, limit: usize) -> Vec<Trade> {
        let trades = self.trades.lock().await;
        trades.iter()
            .rev()
            .filter(|t| t.market_id == market_id)
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Process market order
    async fn process_market_order(&self, mut order: Order) -> Result<Order, String> {
        let mut order_books = self.order_books.write().await;
        let key = (order.market_id, order.outcome);
        let book = order_books.entry(key)
            .or_insert_with(|| OutcomeOrderBook::new(order.market_id, order.outcome));
        
        // Find matching orders
        let matching_side = match order.side {
            Side::Back => &mut book.lays,
            Side::Lay => &mut book.backs,
        };
        
        let mut trades = Vec::new();
        let mut total_cost = Decimal::ZERO;
        
        // Match against best prices
        while order.remaining_amount() > Decimal::ZERO && !matching_side.is_empty() {
            let best_price = match order.side {
                Side::Back => *matching_side.keys().next().unwrap(),
                Side::Lay => *matching_side.keys().next_back().unwrap(),
            };
            
            if let Some(level) = matching_side.get_mut(&best_price) {
                while order.remaining_amount() > Decimal::ZERO && !level.orders.is_empty() {
                    // Pop the order to avoid borrow conflicts
                    let mut matching_order = match level.orders.pop_front() {
                        Some(order) => order,
                        None => break,
                    };
                    
                    // Check self-trade prevention
                    if self.config.enable_self_trade_prevention && 
                       matching_order.user_id == order.user_id {
                        continue;
                    }
                    
                    // Calculate match amount
                    let match_amount = order.remaining_amount()
                        .min(matching_order.remaining_amount());
                    
                    // Create trade
                    let trade = self.create_trade(
                        &order,
                        &matching_order,
                        best_price,
                        match_amount,
                    ).await?;
                    
                    trades.push(trade);
                    total_cost += match_amount * best_price;
                    
                    // Update orders
                    order.filled_amount += match_amount;
                    matching_order.filled_amount += match_amount;
                    
                    if matching_order.filled_amount >= matching_order.amount {
                        matching_order.status = OrderStatus::Filled;
                        
                        // Update stored order
                        if let Some(stored) = self.orders.write().await.get_mut(&matching_order.id) {
                            *stored = matching_order.clone();
                        }
                        
                        // Broadcast fill
                        self.broadcast_order_update(&matching_order).await;
                    } else {
                        matching_order.status = OrderStatus::PartiallyFilled {
                            filled: matching_order.filled_amount,
                            remaining: matching_order.remaining_amount(),
                        };
                        
                        // Put the partially filled order back at the front
                        level.orders.push_front(matching_order);
                    }
                }
                
                if level.orders.is_empty() {
                    matching_side.remove(&best_price);
                }
            }
        }
        
        // Update order status
        if order.filled_amount >= order.amount {
            order.status = OrderStatus::Filled;
            if order.filled_amount > Decimal::ZERO {
                order.average_price = Some(total_cost / order.filled_amount);
            }
        } else if order.filled_amount > Decimal::ZERO {
            match order.time_in_force {
                TimeInForce::IOC | TimeInForce::FOK => {
                    order.status = OrderStatus::Cancelled;
                }
                _ => {
                    order.status = OrderStatus::PartiallyFilled {
                        filled: order.filled_amount,
                        remaining: order.remaining_amount(),
                    };
                }
            }
        } else {
            match order.time_in_force {
                TimeInForce::IOC | TimeInForce::FOK => {
                    order.status = OrderStatus::Cancelled;
                }
                _ => {
                    return Err("No liquidity available".to_string());
                }
            }
        }
        
        order.updated_at = Utc::now();
        
        // Broadcast trades
        for trade in trades {
            self.broadcast_trade(&trade).await;
        }
        
        Ok(order)
    }
    
    /// Process limit order
    async fn process_limit_order(&self, mut order: Order) -> Result<Order, String> {
        let price = order.price.ok_or("Limit order must have price")?;
        
        // Try to match immediately
        let mut order_books = self.order_books.write().await;
        let key = (order.market_id, order.outcome);
        let book = order_books.entry(key)
            .or_insert_with(|| OutcomeOrderBook::new(order.market_id, order.outcome));
        
        // Check if order crosses the spread
        let crosses = match order.side {
            Side::Back => {
                if let Some(best_lay) = book.get_best_lay() {
                    price >= best_lay
                } else {
                    false
                }
            }
            Side::Lay => {
                if let Some(best_back) = book.get_best_back() {
                    price <= best_back
                } else {
                    false
                }
            }
        };
        
        if crosses {
            // Execute as taker
            drop(order_books);
            order.order_type = OrderType::Market;
            self.process_market_order(order).await
        } else {
            // Add to order book as maker
            book.add_order(order.clone());
            self.broadcast_order_book_update(order.market_id, order.outcome).await;
            Ok(order)
        }
    }
    
    /// Process post-only order
    async fn process_post_only_order(&self, order: Order) -> Result<Order, String> {
        let price = order.price.ok_or("Post-only order must have price")?;
        
        let mut order_books = self.order_books.write().await;
        let key = (order.market_id, order.outcome);
        let book = order_books.entry(key)
            .or_insert_with(|| OutcomeOrderBook::new(order.market_id, order.outcome));
        
        // Check if order would cross the spread
        let would_cross = match order.side {
            Side::Back => {
                if let Some(best_lay) = book.get_best_lay() {
                    price >= best_lay
                } else {
                    false
                }
            }
            Side::Lay => {
                if let Some(best_back) = book.get_best_back() {
                    price <= best_back
                } else {
                    false
                }
            }
        };
        
        if would_cross {
            Err("Post-only order would cross the spread".to_string())
        } else {
            book.add_order(order.clone());
            self.broadcast_order_book_update(order.market_id, order.outcome).await;
            Ok(order)
        }
    }
    
    /// Create a trade record
    async fn create_trade(
        &self,
        taker_order: &Order,
        maker_order: &Order,
        price: Decimal,
        amount: Decimal,
    ) -> Result<Trade, String> {
        let mut sequence = self.trade_sequence.lock().await;
        *sequence += 1;
        
        let maker_fee = self.calculate_fee(amount * price, true);
        let taker_fee = self.calculate_fee(amount * price, false);
        
        let trade = Trade {
            id: Uuid::new_v4().to_string(),
            market_id: taker_order.market_id,
            outcome: taker_order.outcome,
            price,
            amount,
            maker_order_id: maker_order.id.clone(),
            taker_order_id: taker_order.id.clone(),
            maker_wallet: maker_order.wallet.clone(),
            taker_wallet: taker_order.wallet.clone(),
            maker_side: maker_order.side,
            taker_side: taker_order.side,
            maker_fee,
            taker_fee,
            timestamp: Utc::now(),
            sequence: *sequence,
        };
        
        // Store trade
        self.trades.lock().await.push(trade.clone());
        
        Ok(trade)
    }
    
    /// Calculate trading fee
    fn calculate_fee(&self, amount: Decimal, is_maker: bool) -> Decimal {
        let rate = if is_maker {
            self.config.fee_structure.maker_fee_rate
        } else {
            self.config.fee_structure.taker_fee_rate
        };
        
        (amount * rate).max(self.config.fee_structure.min_fee)
    }
    
    /// Validate order
    fn validate_order(&self, order: &Order) -> Result<(), String> {
        // Check order size
        if order.amount < self.config.min_order_size {
            return Err(format!("Order size below minimum: {}", self.config.min_order_size));
        }
        
        if order.amount > self.config.max_order_size {
            return Err(format!("Order size above maximum: {}", self.config.max_order_size));
        }
        
        // Check price bounds
        if let Some(price) = order.price {
            if price <= Decimal::ZERO || price >= Decimal::ONE {
                return Err("Price must be between 0 and 1".to_string());
            }
        }
        
        // Validate outcome
        if order.outcome > 1 {
            return Err("Invalid outcome index".to_string());
        }
        
        Ok(())
    }
    
    /// Round price to tick size
    fn round_price(&self, price: Decimal) -> Decimal {
        let tick = self.config.price_tick_size;
        (price / tick).round() * tick
    }
    
    /// Broadcast order update via WebSocket
    async fn broadcast_order_update(&self, order: &Order) {
        if let Some(ws) = &self.ws_manager {
            let msg = WsServerMessage::OrderUpdate {
                order: OrderData {
                    id: order.id.clone(),
                    market_id: order.market_id,
                    user: order.wallet.clone(),
                    status: format!("{:?}", order.status),
                    side: if order.side == Side::Back { "back".to_string() } else { "lay".to_string() },
                    price: order.price.map(|p| p.to_f64().unwrap_or(0.0)),
                    amount: order.amount.to_u64().unwrap_or(0),
                    filled: order.filled_amount.to_u64().unwrap_or(0),
                    timestamp: order.updated_at.timestamp(),
                },
            };
            ws.broadcast_trade_update(msg);
        }
    }
    
    /// Broadcast trade via WebSocket
    async fn broadcast_trade(&self, trade: &Trade) {
        if let Some(ws) = &self.ws_manager {
            let msg = WsServerMessage::TradeExecution {
                trade: TradeData {
                    id: trade.id.clone(),
                    market_id: trade.market_id,
                    outcome: trade.outcome,
                    price: trade.price.to_f64().unwrap_or(0.0),
                    amount: trade.amount.to_u64().unwrap_or(0),
                    timestamp: trade.timestamp.timestamp(),
                    buyer: if trade.taker_side == Side::Back {
                        trade.taker_wallet.clone()
                    } else {
                        trade.maker_wallet.clone()
                    },
                    seller: if trade.taker_side == Side::Lay {
                        trade.taker_wallet.clone()
                    } else {
                        trade.maker_wallet.clone()
                    },
                    side: if trade.taker_side == Side::Back { "buy" } else { "sell" }.to_string(),
                },
            };
            ws.broadcast_trade_update(msg);
        }
    }
    
    /// Broadcast order book update via WebSocket
    async fn broadcast_order_book_update(&self, market_id: u128, outcome: u8) {
        if let Some(ws) = &self.ws_manager {
            let snapshot = self.get_order_book(market_id, outcome, 10).await;
            
            let msg = WsServerMessage::OrderBook {
                market_id,
                bids: snapshot.backs.into_iter()
                    .map(|l| OrderLevel {
                        price: l.price.to_f64().unwrap_or(0.0),
                        amount: l.amount.to_u64().unwrap_or(0),
                        orders: l.orders as u32,
                    })
                    .collect(),
                asks: snapshot.lays.into_iter()
                    .map(|l| OrderLevel {
                        price: l.price.to_f64().unwrap_or(0.0),
                        amount: l.amount.to_u64().unwrap_or(0),
                        orders: l.orders as u32,
                    })
                    .collect(),
            };
            ws.broadcast_trade_update(msg);
        }
    }
}

/// Order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub market_id: u128,
    pub outcome: u8,
    pub backs: Vec<PriceLevel>,
    pub lays: Vec<PriceLevel>,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub amount: Decimal,
    pub orders: usize,
}

/// Order request from API
#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub market_id: u128,
    pub outcome: u8,
    pub side: String, // "back" or "lay"
    pub order_type: String, // "market", "limit", "post_only"
    pub amount: String,
    pub price: Option<String>,
    pub time_in_force: Option<String>, // "GTC", "IOC", "FOK"
    pub client_order_id: Option<String>,
}

impl PlaceOrderRequest {
    pub fn to_order(&self, user_id: String, wallet: String) -> Result<Order, String> {
        let side = match self.side.to_lowercase().as_str() {
            "back" | "buy" => Side::Back,
            "lay" | "sell" => Side::Lay,
            _ => return Err("Invalid side".to_string()),
        };
        
        let amount = Decimal::from_str(&self.amount)
            .map_err(|_| "Invalid amount".to_string())?;
        
        let order_type = match self.order_type.to_lowercase().as_str() {
            "market" => OrderType::Market,
            "limit" => {
                let price = self.price.as_ref()
                    .ok_or("Limit order requires price")?;
                let price = Decimal::from_str(price)
                    .map_err(|_| "Invalid price".to_string())?;
                OrderType::Limit { price }
            }
            "post_only" => {
                let price = self.price.as_ref()
                    .ok_or("Post-only order requires price")?;
                let price = Decimal::from_str(price)
                    .map_err(|_| "Invalid price".to_string())?;
                OrderType::PostOnly { price }
            }
            _ => return Err("Invalid order type".to_string()),
        };
        
        let time_in_force = match self.time_in_force.as_ref() {
            Some(tif) => match tif.to_uppercase().as_str() {
                "IOC" => TimeInForce::IOC,
                "FOK" => TimeInForce::FOK,
                "GTC" => TimeInForce::GTC,
                _ => return Err("Invalid time in force".to_string()),
            },
            None => TimeInForce::GTC,
        };
        
        Ok(Order::new(
            self.market_id,
            self.outcome,
            user_id,
            wallet,
            side,
            order_type,
            amount,
            time_in_force,
            self.client_order_id.clone(),
        ))
    }
}

/// Cancel order request
#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub order_id: String,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_order_matching() {
        let engine = TradingEngine::new(TradingEngineConfig::default(), None);
        
        // Place a limit buy order
        let buy_order = Order::new(
            1,
            0,
            "user1".to_string(),
            "wallet1".to_string(),
            Side::Back,
            OrderType::Limit { price: Decimal::from_str("0.45").unwrap() },
            Decimal::from_str("100").unwrap(),
            TimeInForce::GTC,
            None,
        );
        
        let placed_buy = engine.place_order(buy_order).await.unwrap();
        assert_eq!(placed_buy.status, OrderStatus::New);
        
        // Place a market sell order that should match
        let sell_order = Order::new(
            1,
            0,
            "user2".to_string(),
            "wallet2".to_string(),
            Side::Lay,
            OrderType::Market,
            Decimal::from_str("50").unwrap(),
            TimeInForce::IOC,
            None,
        );
        
        let placed_sell = engine.place_order(sell_order).await.unwrap();
        assert_eq!(placed_sell.status, OrderStatus::Filled);
        assert_eq!(placed_sell.filled_amount, Decimal::from_str("50").unwrap());
        
        // Check trades
        let trades = engine.get_recent_trades(1, 10).await;
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, Decimal::from_str("0.45").unwrap());
        assert_eq!(trades[0].amount, Decimal::from_str("50").unwrap());
    }
}