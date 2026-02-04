//! Advanced order types for the betting platform

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit {
        price: f64,
    },
    StopLoss {
        trigger_price: f64,
    },
    TakeProfit {
        trigger_price: f64,
    },
    StopLimit {
        stop_price: f64,
        limit_price: f64,
    },
    TrailingStop {
        trail_amount: f64,
        trail_percent: Option<f64>,
    },
    OCO {
        // One-Cancels-Other
        limit_price: f64,
        stop_price: f64,
    },
    Bracket {
        // Entry + Stop Loss + Take Profit
        entry_price: f64,
        stop_loss: f64,
        take_profit: f64,
    },
    Iceberg {
        // Hidden volume order
        visible_size: u64,
        total_size: u64,
    },
    TWAP {
        // Time-Weighted Average Price
        duration_minutes: u32,
        intervals: u32,
    },
    VWAP {
        // Volume-Weighted Average Price
        target_volume: u64,
        max_participation: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled {
        filled_amount: u64,
        remaining_amount: u64,
    },
    Filled,
    Cancelled,
    Rejected {
        reason: String,
    },
    Expired,
    Triggered, // For stop orders
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,  // Good Till Cancelled
    IOC,  // Immediate Or Cancel
    FOK,  // Fill Or Kill
    GTD(DateTime<Utc>), // Good Till Date
    GTT(u64), // Good Till Time (seconds)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub market_id: u128,
    pub wallet: String,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub amount: u64,
    pub outcome: u8,
    pub leverage: u32,
    pub status: OrderStatus,
    pub time_in_force: TimeInForce,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub filled_amount: u64,
    pub average_fill_price: Option<f64>,
    pub fees: u64,
    pub verse_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub market_id: u128,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub last_update: DateTime<Utc>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub size: u64,
    pub order_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub market_id: u128,
    pub order_id: String,
    pub price: f64,
    pub size: u64,
    pub side: OrderSide,
    pub maker_wallet: String,
    pub taker_wallet: String,
    pub timestamp: DateTime<Utc>,
    pub fee: u64,
}

/// Order matching engine
pub struct OrderMatchingEngine {
    order_books: Arc<Mutex<HashMap<u128, OrderBook>>>,
    pending_orders: Arc<Mutex<HashMap<String, Order>>>,
    trades: Arc<Mutex<Vec<Trade>>>,
}

impl OrderMatchingEngine {
    pub fn new() -> Self {
        Self {
            order_books: Arc::new(Mutex::new(HashMap::new())),
            pending_orders: Arc::new(Mutex::new(HashMap::new())),
            trades: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Place a new order
    pub fn place_order(&self, order: Order) -> Result<Order, String> {
        match order.order_type.clone() {
            OrderType::Market => self.execute_market_order(order),
            OrderType::Limit { price } => self.place_limit_order(order, price),
            OrderType::StopLoss { trigger_price } => self.place_stop_order(order, trigger_price, true),
            OrderType::TakeProfit { trigger_price } => self.place_stop_order(order, trigger_price, false),
            OrderType::StopLimit { stop_price, limit_price } => {
                self.place_stop_limit_order(order, stop_price, limit_price)
            }
            OrderType::TrailingStop { trail_amount, trail_percent } => {
                self.place_trailing_stop(order, trail_amount, trail_percent)
            }
            OrderType::OCO { limit_price, stop_price } => {
                self.place_oco_order(order, limit_price, stop_price)
            }
            OrderType::Bracket { entry_price, stop_loss, take_profit } => {
                self.place_bracket_order(order, entry_price, stop_loss, take_profit)
            }
            OrderType::Iceberg { visible_size, total_size } => {
                self.place_iceberg_order(order, visible_size, total_size)
            }
            OrderType::TWAP { duration_minutes, intervals } => {
                self.place_twap_order(order, duration_minutes, intervals)
            }
            OrderType::VWAP { target_volume, max_participation } => {
                self.place_vwap_order(order, target_volume, max_participation)
            }
        }
    }

    /// Execute market order immediately
    fn execute_market_order(&self, mut order: Order) -> Result<Order, String> {
        let mut order_books = self.order_books.lock().unwrap();
        let book = order_books.entry(order.market_id).or_insert_with(|| OrderBook {
            market_id: order.market_id,
            bids: vec![],
            asks: vec![],
            last_update: Utc::now(),
            sequence: 0,
        });

        let levels = match order.side {
            OrderSide::Buy => &book.asks,
            OrderSide::Sell => &book.bids,
        };

        if levels.is_empty() {
            return Err("No liquidity available".to_string());
        }

        // Calculate execution
        let mut remaining = order.amount;
        let mut total_cost = 0u64;
        let mut fills = vec![];

        for level in levels {
            if remaining == 0 {
                break;
            }

            let fill_size = remaining.min(level.size);
            let fill_cost = (fill_size as f64 * level.price) as u64;
            
            fills.push((level.price, fill_size));
            total_cost += fill_cost;
            remaining -= fill_size;
        }

        if remaining > 0 {
            // Partial fill
            order.status = OrderStatus::PartiallyFilled {
                filled_amount: order.amount - remaining,
                remaining_amount: remaining,
            };
        } else {
            order.status = OrderStatus::Filled;
        }

        order.filled_amount = order.amount - remaining;
        order.average_fill_price = Some(total_cost as f64 / order.filled_amount as f64);
        order.updated_at = Utc::now();

        // Record trades
        let mut trades = self.trades.lock().unwrap();
        for (price, size) in fills {
            trades.push(Trade {
                id: uuid::Uuid::new_v4().to_string(),
                market_id: order.market_id,
                order_id: order.id.clone(),
                price,
                size,
                side: order.side.clone(),
                maker_wallet: "market-maker".to_string(),
                taker_wallet: order.wallet.clone(),
                timestamp: Utc::now(),
                fee: (size as f64 * price * 0.001) as u64, // 0.1% fee
            });
        }

        Ok(order)
    }

    /// Place limit order in order book
    fn place_limit_order(&self, mut order: Order, price: f64) -> Result<Order, String> {
        // Check for immediate execution
        let mut order_books = self.order_books.lock().unwrap();
        let book = order_books.entry(order.market_id).or_insert_with(|| OrderBook {
            market_id: order.market_id,
            bids: vec![],
            asks: vec![],
            last_update: Utc::now(),
            sequence: 0,
        });

        let can_execute = match order.side {
            OrderSide::Buy => book.asks.first().map(|l| price >= l.price).unwrap_or(false),
            OrderSide::Sell => book.bids.first().map(|l| price <= l.price).unwrap_or(false),
        };

        if can_execute && matches!(order.time_in_force, TimeInForce::IOC | TimeInForce::FOK) {
            // Execute immediately if possible
            drop(order_books);
            return self.execute_market_order(order);
        }

        // Add to order book
        order.status = OrderStatus::Open;
        order.updated_at = Utc::now();

        let level = OrderBookLevel {
            price,
            size: order.amount,
            order_count: 1,
        };

        match order.side {
            OrderSide::Buy => {
                book.bids.push(level);
                book.bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
            }
            OrderSide::Sell => {
                book.asks.push(level);
                book.asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
            }
        }

        book.last_update = Utc::now();
        book.sequence += 1;

        // Store in pending orders
        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(order.id.clone(), order.clone());

        Ok(order)
    }

    /// Place stop order (stop-loss or take-profit)
    fn place_stop_order(&self, mut order: Order, trigger_price: f64, is_stop_loss: bool) -> Result<Order, String> {
        order.status = OrderStatus::Pending;
        order.updated_at = Utc::now();

        // Store in pending orders with trigger monitoring
        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(order.id.clone(), order.clone());

        Ok(order)
    }

    /// Place stop-limit order
    fn place_stop_limit_order(&self, mut order: Order, stop_price: f64, limit_price: f64) -> Result<Order, String> {
        order.status = OrderStatus::Pending;
        order.updated_at = Utc::now();

        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(order.id.clone(), order.clone());

        Ok(order)
    }

    /// Place trailing stop order
    fn place_trailing_stop(&self, mut order: Order, trail_amount: f64, trail_percent: Option<f64>) -> Result<Order, String> {
        order.status = OrderStatus::Pending;
        order.updated_at = Utc::now();

        // Initialize trailing stop metadata
        order.metadata.insert("trail_amount".to_string(), trail_amount.to_string());
        if let Some(percent) = trail_percent {
            order.metadata.insert("trail_percent".to_string(), percent.to_string());
        }
        order.metadata.insert("best_price".to_string(), "0".to_string());

        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(order.id.clone(), order.clone());

        Ok(order)
    }

    /// Place OCO (One-Cancels-Other) order
    fn place_oco_order(&self, order: Order, limit_price: f64, stop_price: f64) -> Result<Order, String> {
        // Create two linked orders
        let mut limit_order = order.clone();
        limit_order.id = format!("{}-limit", order.id);
        limit_order.order_type = OrderType::Limit { price: limit_price };
        limit_order.metadata.insert("oco_pair".to_string(), format!("{}-stop", order.id));

        let mut stop_order = order.clone();
        stop_order.id = format!("{}-stop", order.id);
        stop_order.order_type = OrderType::StopLoss { trigger_price: stop_price };
        stop_order.metadata.insert("oco_pair".to_string(), limit_order.id.clone());

        // Place both orders
        self.place_limit_order(limit_order.clone(), limit_price)?;
        self.place_stop_order(stop_order, stop_price, true)?;

        Ok(limit_order)
    }

    /// Place bracket order (entry + stop-loss + take-profit)
    fn place_bracket_order(&self, mut order: Order, entry_price: f64, stop_loss: f64, take_profit: f64) -> Result<Order, String> {
        // First place the entry order
        order.order_type = OrderType::Limit { price: entry_price };
        let entry_order = self.place_limit_order(order.clone(), entry_price)?;

        // Create linked stop-loss and take-profit orders
        let mut sl_order = order.clone();
        sl_order.id = format!("{}-sl", order.id);
        sl_order.order_type = OrderType::StopLoss { trigger_price: stop_loss };
        sl_order.metadata.insert("parent_order".to_string(), entry_order.id.clone());
        sl_order.status = OrderStatus::Pending;

        let mut tp_order = order.clone();
        tp_order.id = format!("{}-tp", order.id);
        tp_order.order_type = OrderType::TakeProfit { trigger_price: take_profit };
        tp_order.metadata.insert("parent_order".to_string(), entry_order.id.clone());
        tp_order.status = OrderStatus::Pending;

        // Store child orders
        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(sl_order.id.clone(), sl_order);
        pending.insert(tp_order.id.clone(), tp_order);

        Ok(entry_order)
    }

    /// Place iceberg order (hidden volume)
    fn place_iceberg_order(&self, mut order: Order, visible_size: u64, total_size: u64) -> Result<Order, String> {
        if visible_size > total_size {
            return Err("Visible size cannot exceed total size".to_string());
        }

        order.metadata.insert("iceberg_total".to_string(), total_size.to_string());
        order.metadata.insert("iceberg_visible".to_string(), visible_size.to_string());
        order.metadata.insert("iceberg_remaining".to_string(), total_size.to_string());

        // Only show visible portion
        order.amount = visible_size;
        
        // Place as limit order
        if let OrderType::Iceberg { .. } = order.order_type {
            // Get price from metadata or use market price
            let price = order.metadata.get("limit_price")
                .and_then(|p| p.parse::<f64>().ok())
                .unwrap_or(1.0);
            
            self.place_limit_order(order, price)
        } else {
            Err("Invalid iceberg order configuration".to_string())
        }
    }

    /// Place TWAP order
    fn place_twap_order(&self, mut order: Order, duration_minutes: u32, intervals: u32) -> Result<Order, String> {
        if intervals == 0 {
            return Err("Intervals must be greater than 0".to_string());
        }

        let slice_size = order.amount / intervals as u64;
        let interval_seconds = (duration_minutes * 60) / intervals;

        order.metadata.insert("twap_slice_size".to_string(), slice_size.to_string());
        order.metadata.insert("twap_interval".to_string(), interval_seconds.to_string());
        order.metadata.insert("twap_remaining".to_string(), order.amount.to_string());
        order.metadata.insert("twap_executed".to_string(), "0".to_string());

        order.status = OrderStatus::Open;
        order.updated_at = Utc::now();

        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(order.id.clone(), order.clone());

        Ok(order)
    }

    /// Place VWAP order
    fn place_vwap_order(&self, mut order: Order, target_volume: u64, max_participation: f64) -> Result<Order, String> {
        if max_participation <= 0.0 || max_participation > 1.0 {
            return Err("Max participation must be between 0 and 1".to_string());
        }

        order.metadata.insert("vwap_target_volume".to_string(), target_volume.to_string());
        order.metadata.insert("vwap_max_participation".to_string(), max_participation.to_string());
        order.metadata.insert("vwap_executed_volume".to_string(), "0".to_string());

        order.status = OrderStatus::Open;
        order.updated_at = Utc::now();

        let mut pending = self.pending_orders.lock().unwrap();
        pending.insert(order.id.clone(), order.clone());

        Ok(order)
    }

    /// Cancel an order
    pub fn cancel_order(&self, order_id: &str) -> Result<Order, String> {
        let mut pending = self.pending_orders.lock().unwrap();
        
        if let Some(mut order) = pending.remove(order_id) {
            order.status = OrderStatus::Cancelled;
            order.updated_at = Utc::now();

            // Handle OCO cancellation
            if let Some(oco_pair) = order.metadata.get("oco_pair") {
                if let Some(mut pair_order) = pending.remove(oco_pair) {
                    pair_order.status = OrderStatus::Cancelled;
                    pair_order.updated_at = Utc::now();
                }
            }

            // Handle bracket order cancellation
            if order.metadata.contains_key("parent_order") {
                // This is a child order, don't cancel parent
            } else {
                // This might be a parent order, cancel children
                let sl_id = format!("{}-sl", order_id);
                let tp_id = format!("{}-tp", order_id);
                
                if let Some(mut sl_order) = pending.remove(&sl_id) {
                    sl_order.status = OrderStatus::Cancelled;
                }
                if let Some(mut tp_order) = pending.remove(&tp_id) {
                    tp_order.status = OrderStatus::Cancelled;
                }
            }

            Ok(order)
        } else {
            Err("Order not found".to_string())
        }
    }

    /// Get order by ID
    pub fn get_order(&self, order_id: &str) -> Option<Order> {
        let pending = self.pending_orders.lock().unwrap();
        pending.get(order_id).cloned()
    }

    /// Get all orders for a wallet
    pub fn get_orders_by_wallet(&self, wallet: &str) -> Vec<Order> {
        let pending = self.pending_orders.lock().unwrap();
        pending.values()
            .filter(|o| o.wallet == wallet)
            .cloned()
            .collect()
    }

    /// Get order book for market
    pub fn get_order_book(&self, market_id: u128) -> Option<OrderBook> {
        let books = self.order_books.lock().unwrap();
        books.get(&market_id).cloned()
    }

    /// Update market price (triggers stop orders)
    pub fn update_market_price(&self, market_id: u128, price: f64) -> Vec<Order> {
        let mut triggered_orders = Vec::new();
        let mut pending = self.pending_orders.lock().unwrap();

        let orders_to_trigger: Vec<String> = pending.iter()
            .filter_map(|(id, order)| {
                if order.market_id != market_id {
                    return None;
                }

                match &order.order_type {
                    OrderType::StopLoss { trigger_price } => {
                        if price <= *trigger_price {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }
                    OrderType::TakeProfit { trigger_price } => {
                        if price >= *trigger_price {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }
                    OrderType::StopLimit { stop_price, .. } => {
                        if price <= *stop_price {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }
                    OrderType::TrailingStop { .. } => {
                        // Update trailing stop
                        let best_price = order.metadata.get("best_price")
                            .and_then(|p| p.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        
                        if price > best_price {
                            // Update best price
                            None
                        } else {
                            let trail_amount = order.metadata.get("trail_amount")
                                .and_then(|a| a.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            
                            if best_price - price >= trail_amount {
                                Some(id.clone())
                            } else {
                                None
                            }
                        }
                    }
                    _ => None,
                }
            })
            .collect();

        // Trigger orders
        for order_id in orders_to_trigger {
            if let Some(mut order) = pending.remove(&order_id) {
                order.status = OrderStatus::Triggered;
                order.updated_at = Utc::now();
                triggered_orders.push(order);
            }
        }

        triggered_orders
    }

    /// Process algorithmic orders (TWAP/VWAP)
    pub fn process_algo_orders(&self) -> Vec<Trade> {
        let mut new_trades = Vec::new();
        let mut pending = self.pending_orders.lock().unwrap();

        let algo_orders: Vec<String> = pending.iter()
            .filter_map(|(id, order)| {
                match &order.order_type {
                    OrderType::TWAP { .. } | OrderType::VWAP { .. } => Some(id.clone()),
                    _ => None,
                }
            })
            .collect();

        for order_id in algo_orders {
            if let Some(order) = pending.get_mut(&order_id) {
                match &order.order_type {
                    OrderType::TWAP { .. } => {
                        // Process TWAP slice
                        let slice_size = order.metadata.get("twap_slice_size")
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0);
                        
                        if slice_size > 0 {
                            // Execute slice as market order
                            let mut slice_order = order.clone();
                            slice_order.amount = slice_size;
                            slice_order.order_type = OrderType::Market;
                            
                            if let Ok(executed) = self.execute_market_order(slice_order) {
                                if let Some(trade) = new_trades.last() {
                                    // Update TWAP tracking
                                    let executed_amount = order.metadata.get("twap_executed")
                                        .and_then(|e| e.parse::<u64>().ok())
                                        .unwrap_or(0) + slice_size;
                                    
                                    order.metadata.insert("twap_executed".to_string(), executed_amount.to_string());
                                    
                                    if executed_amount >= order.amount {
                                        order.status = OrderStatus::Filled;
                                    }
                                }
                            }
                        }
                    }
                    OrderType::VWAP { max_participation, .. } => {
                        // Calculate volume-based execution
                        // This would integrate with real-time volume data
                        // For now, execute a portion based on participation rate
                        let portion = (order.amount as f64 * max_participation) as u64;
                        
                        let mut vwap_order = order.clone();
                        vwap_order.amount = portion;
                        vwap_order.order_type = OrderType::Market;
                        
                        if let Ok(_) = self.execute_market_order(vwap_order) {
                            let executed_volume = order.metadata.get("vwap_executed_volume")
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(0) + portion;
                            
                            order.metadata.insert("vwap_executed_volume".to_string(), executed_volume.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        new_trades
    }
}

/// Order validation
pub fn validate_order(order: &Order) -> Result<(), String> {
    if order.amount == 0 {
        return Err("Order amount must be greater than 0".to_string());
    }

    if order.leverage == 0 || order.leverage > 100 {
        return Err("Leverage must be between 1 and 100".to_string());
    }

    match &order.order_type {
        OrderType::Limit { price } => {
            if *price <= 0.0 {
                return Err("Limit price must be positive".to_string());
            }
        }
        OrderType::StopLoss { trigger_price } | OrderType::TakeProfit { trigger_price } => {
            if *trigger_price <= 0.0 {
                return Err("Trigger price must be positive".to_string());
            }
        }
        OrderType::StopLimit { stop_price, limit_price } => {
            if *stop_price <= 0.0 || *limit_price <= 0.0 {
                return Err("Stop and limit prices must be positive".to_string());
            }
        }
        OrderType::TrailingStop { trail_amount, .. } => {
            if *trail_amount <= 0.0 {
                return Err("Trail amount must be positive".to_string());
            }
        }
        OrderType::OCO { limit_price, stop_price } => {
            if *limit_price <= 0.0 || *stop_price <= 0.0 {
                return Err("OCO prices must be positive".to_string());
            }
        }
        OrderType::Bracket { entry_price, stop_loss, take_profit } => {
            if *entry_price <= 0.0 || *stop_loss <= 0.0 || *take_profit <= 0.0 {
                return Err("All bracket prices must be positive".to_string());
            }
            
            // Validate bracket logic
            match order.side {
                OrderSide::Buy => {
                    if *stop_loss >= *entry_price {
                        return Err("Stop loss must be below entry for buy orders".to_string());
                    }
                    if *take_profit <= *entry_price {
                        return Err("Take profit must be above entry for buy orders".to_string());
                    }
                }
                OrderSide::Sell => {
                    if *stop_loss <= *entry_price {
                        return Err("Stop loss must be above entry for sell orders".to_string());
                    }
                    if *take_profit >= *entry_price {
                        return Err("Take profit must be below entry for sell orders".to_string());
                    }
                }
            }
        }
        OrderType::Iceberg { visible_size, total_size } => {
            if *visible_size == 0 || *visible_size > *total_size {
                return Err("Invalid iceberg configuration".to_string());
            }
        }
        OrderType::TWAP { duration_minutes, intervals } => {
            if *duration_minutes == 0 || *intervals == 0 {
                return Err("TWAP duration and intervals must be positive".to_string());
            }
        }
        OrderType::VWAP { target_volume, max_participation } => {
            if *target_volume == 0 {
                return Err("VWAP target volume must be positive".to_string());
            }
            if *max_participation <= 0.0 || *max_participation > 1.0 {
                return Err("VWAP max participation must be between 0 and 1".to_string());
            }
        }
        _ => {}
    }

    Ok(())
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_order_execution() {
        let engine = OrderMatchingEngine::new();
        
        // Add liquidity to order book
        let mut books = engine.order_books.lock().unwrap();
        books.insert(1, OrderBook {
            market_id: 1,
            bids: vec![
                OrderBookLevel { price: 0.49, size: 1000000, order_count: 1 },
                OrderBookLevel { price: 0.48, size: 2000000, order_count: 2 },
            ],
            asks: vec![
                OrderBookLevel { price: 0.51, size: 1500000, order_count: 1 },
                OrderBookLevel { price: 0.52, size: 2500000, order_count: 2 },
            ],
            last_update: Utc::now(),
            sequence: 1,
        });
        drop(books);

        // Place market buy order
        let order = Order {
            id: "test-1".to_string(),
            market_id: 1,
            wallet: "test-wallet".to_string(),
            order_type: OrderType::Market,
            side: OrderSide::Buy,
            amount: 2000000,
            outcome: 0,
            leverage: 2,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::IOC,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            filled_amount: 0,
            average_fill_price: None,
            fees: 0,
            verse_id: None,
            metadata: HashMap::new(),
        };

        let result = engine.place_order(order).unwrap();
        assert_eq!(result.filled_amount, 2000000);
        assert!(result.average_fill_price.is_some());
    }

    #[test]
    fn test_limit_order_placement() {
        let engine = OrderMatchingEngine::new();
        
        let order = Order {
            id: "test-2".to_string(),
            market_id: 1,
            wallet: "test-wallet".to_string(),
            order_type: OrderType::Limit { price: 0.50 },
            side: OrderSide::Buy,
            amount: 1000000,
            outcome: 0,
            leverage: 1,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::GTC,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            filled_amount: 0,
            average_fill_price: None,
            fees: 0,
            verse_id: None,
            metadata: HashMap::new(),
        };

        let result = engine.place_order(order).unwrap();
        assert_eq!(result.status, OrderStatus::Open);
        
        // Verify order book updated
        let book = engine.get_order_book(1).unwrap();
        assert!(!book.bids.is_empty());
    }

    #[test]
    fn test_stop_order_trigger() {
        let engine = OrderMatchingEngine::new();
        
        let order = Order {
            id: "test-3".to_string(),
            market_id: 1,
            wallet: "test-wallet".to_string(),
            order_type: OrderType::StopLoss { trigger_price: 0.45 },
            side: OrderSide::Sell,
            amount: 1000000,
            outcome: 0,
            leverage: 1,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::GTC,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            filled_amount: 0,
            average_fill_price: None,
            fees: 0,
            verse_id: None,
            metadata: HashMap::new(),
        };

        engine.place_order(order).unwrap();
        
        // Update market price to trigger stop
        let triggered = engine.update_market_price(1, 0.44);
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0].status, OrderStatus::Triggered);
    }

    #[test]
    fn test_bracket_order() {
        let engine = OrderMatchingEngine::new();
        
        let order = Order {
            id: "test-4".to_string(),
            market_id: 1,
            wallet: "test-wallet".to_string(),
            order_type: OrderType::Bracket {
                entry_price: 0.50,
                stop_loss: 0.45,
                take_profit: 0.55,
            },
            side: OrderSide::Buy,
            amount: 1000000,
            outcome: 0,
            leverage: 2,
            status: OrderStatus::Pending,
            time_in_force: TimeInForce::GTC,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            filled_amount: 0,
            average_fill_price: None,
            fees: 0,
            verse_id: None,
            metadata: HashMap::new(),
        };

        let result = engine.place_order(order).unwrap();
        
        // Verify child orders created
        let sl_order = engine.get_order(&format!("{}-sl", result.id));
        assert!(sl_order.is_some());
        
        let tp_order = engine.get_order(&format!("{}-tp", result.id));
        assert!(tp_order.is_some());
    }
}