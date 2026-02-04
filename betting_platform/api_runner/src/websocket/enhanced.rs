//! Enhanced WebSocket handler with comprehensive real-time updates

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{info, debug, error, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use chrono::Utc;

use crate::{AppState, types::WsMessage};

/// WebSocket connection state
#[derive(Debug, Clone)]
struct WsConnection {
    id: String,
    subscriptions: Vec<Subscription>,
    authenticated: bool,
    user_wallet: Option<String>,
    connected_at: i64,
    last_ping: i64,
}

/// Subscription types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Subscription {
    MarketUpdates { market_id: u128 },
    PositionUpdates { wallet: String },
    OrderBookUpdates { market_id: u128 },
    PriceFeeds { markets: Vec<u128> },
    SystemStatus,
    AllMarkets,
}

/// Enhanced WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EnhancedWsMessage {
    // Market updates
    MarketUpdate {
        market_id: u128,
        yes_price: f64,
        no_price: f64,
        volume: u64,
        liquidity: u64,
        trades_24h: u32,
        timestamp: i64,
    },
    
    // Order book updates
    OrderBookUpdate {
        market_id: u128,
        bids: Vec<OrderLevel>,
        asks: Vec<OrderLevel>,
        spread: f64,
        mid_price: f64,
        timestamp: i64,
    },
    
    // Position updates
    PositionUpdate {
        wallet: String,
        market_id: u128,
        position: PositionInfo,
        action: String, // "opened", "closed", "liquidated", "updated"
        timestamp: i64,
    },
    
    // Trade execution
    TradeExecution {
        market_id: u128,
        price: f64,
        size: u64,
        side: String, // "buy", "sell"
        timestamp: i64,
    },
    
    // System events
    SystemEvent {
        event_type: String,
        message: String,
        severity: String, // "info", "warning", "error", "critical"
        timestamp: i64,
    },
    
    // Circuit breaker alerts
    CircuitBreakerAlert {
        breaker_type: String,
        market_id: Option<u128>,
        triggered: bool,
        message: String,
        timestamp: i64,
    },
    
    // Heartbeat/Ping
    Heartbeat {
        timestamp: i64,
        server_time: i64,
    },
    
    // Subscription confirmation
    SubscriptionConfirmed {
        subscription: Subscription,
        status: String,
    },
    
    // Error message
    Error {
        code: String,
        message: String,
        timestamp: i64,
    },
}

/// Order level for order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: f64,
    pub amount: u64,
    pub orders: u32,
}

/// WebSocket server message - now an enum to match trading_engine expectations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsServerMessage {
    OrderUpdate {
        order: OrderData,
    },
    TradeExecution {
        trade: TradeData,
    },
    OrderBook {
        market_id: u128,
        bids: Vec<OrderLevel>,
        asks: Vec<OrderLevel>,
    },
    MarketData {
        market_id: u128,
        message_type: String,
        data: serde_json::Value,
    },
}

/// Trade data for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub id: String,
    pub market_id: u128,
    pub outcome: u8,
    pub price: f64,
    pub amount: u64,
    pub buyer: String,
    pub seller: String,
    pub side: String,
    pub timestamp: i64,
}

/// Order data for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderData {
    pub id: String,
    pub market_id: u128,
    pub user: String,
    pub status: String,
    pub side: String,
    pub price: Option<f64>,
    pub amount: u64,
    pub filled: u64,
    pub timestamp: i64,
}

/// Position info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub size: u64,
    pub entry_price: f64,
    pub current_price: f64,
    pub pnl: f64,
    pub pnl_percentage: f64,
    pub leverage: u32,
    pub liquidation_price: f64,
}

/// WebSocket request from client
#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
enum WsRequest {
    Subscribe { subscriptions: Vec<Subscription> },
    Unsubscribe { subscriptions: Vec<Subscription> },
    Authenticate { wallet: String, signature: String },
    Ping { timestamp: i64 },
    GetSnapshot { market_id: u128 },
}

/// Enhanced WebSocket manager
pub struct EnhancedWebSocketManager {
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    market_tx: broadcast::Sender<EnhancedWsMessage>,
    position_tx: broadcast::Sender<EnhancedWsMessage>,
    system_tx: broadcast::Sender<EnhancedWsMessage>,
}

impl EnhancedWebSocketManager {
    pub fn new() -> Self {
        let (market_tx, _) = broadcast::channel(1000);
        let (position_tx, _) = broadcast::channel(500);
        let (system_tx, _) = broadcast::channel(100);
        
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            market_tx,
            position_tx,
            system_tx,
        }
    }
    
    /// Register new connection
    pub async fn register_connection(&self, conn_id: String) {
        let connection = WsConnection {
            id: conn_id.clone(),
            subscriptions: Vec::new(),
            authenticated: false,
            user_wallet: None,
            connected_at: Utc::now().timestamp(),
            last_ping: Utc::now().timestamp(),
        };
        
        self.connections.write().await.insert(conn_id, connection);
    }
    
    /// Remove connection
    pub async fn remove_connection(&self, conn_id: &str) {
        self.connections.write().await.remove(conn_id);
    }
    
    /// Subscribe to market updates
    pub fn subscribe_market_updates(&self) -> broadcast::Receiver<EnhancedWsMessage> {
        self.market_tx.subscribe()
    }
    
    /// Subscribe to position updates
    pub fn subscribe_position_updates(&self) -> broadcast::Receiver<EnhancedWsMessage> {
        self.position_tx.subscribe()
    }
    
    /// Subscribe to system updates
    pub fn subscribe_system_updates(&self) -> broadcast::Receiver<EnhancedWsMessage> {
        self.system_tx.subscribe()
    }
    
    /// Update connection subscriptions
    pub async fn update_subscriptions(
        &self,
        conn_id: &str,
        subscriptions: Vec<Subscription>,
    ) -> Result<(), String> {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(conn_id) {
            conn.subscriptions = subscriptions;
            Ok(())
        } else {
            Err("Connection not found".to_string())
        }
    }
    
    /// Broadcast market update
    pub fn broadcast_market_update(&self, msg: EnhancedWsMessage) {
        let _ = self.market_tx.send(msg);
    }
    
    /// Broadcast position update
    pub fn broadcast_position_update(&self, msg: EnhancedWsMessage) {
        let _ = self.position_tx.send(msg);
    }
    
    /// Broadcast system event
    pub fn broadcast_system_event(&self, msg: EnhancedWsMessage) {
        let _ = self.system_tx.send(msg);
    }
    
    /// Broadcast order book update
    pub fn broadcast_order_book_update(&self, msg: EnhancedWsMessage) {
        // Order book updates go through the market channel
        let _ = self.market_tx.send(msg);
    }
    
    /// Broadcast trade update (for trading engine compatibility)
    pub fn broadcast_trade_update(&self, msg: WsServerMessage) {
        // Convert WsServerMessage to EnhancedWsMessage for broadcasting
        match msg {
            WsServerMessage::TradeExecution { trade } => {
                let enhanced_msg = EnhancedWsMessage::TradeExecution {
                    market_id: trade.market_id,
                    price: trade.price,
                    size: trade.amount,
                    side: trade.side.clone(),
                    timestamp: trade.timestamp,
                };
                let _ = self.market_tx.send(enhanced_msg);
            }
            WsServerMessage::OrderUpdate { order } => {
                // Convert to appropriate EnhancedWsMessage if needed
                let enhanced_msg = EnhancedWsMessage::MarketUpdate {
                    market_id: order.market_id,
                    yes_price: order.price.unwrap_or(0.0),
                    no_price: 1.0 - order.price.unwrap_or(0.0),
                    volume: order.amount,
                    liquidity: 0,
                    trades_24h: 0,
                    timestamp: order.timestamp,
                };
                let _ = self.market_tx.send(enhanced_msg);
            }
            WsServerMessage::OrderBook { market_id, bids, asks } => {
                let enhanced_msg = EnhancedWsMessage::OrderBookUpdate {
                    market_id,
                    bids: bids.into_iter().map(|l| OrderLevel {
                        price: l.price,
                        amount: l.amount,
                        orders: l.orders,
                    }).collect(),
                    asks: asks.into_iter().map(|l| OrderLevel {
                        price: l.price,
                        amount: l.amount,
                        orders: l.orders,
                    }).collect(),
                    spread: 0.0,
                    mid_price: 0.0,
                    timestamp: chrono::Utc::now().timestamp(),
                };
                let _ = self.market_tx.send(enhanced_msg);
            }
            _ => {}
        }
    }
    
    /// Get connection stats
    pub async fn get_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        
        ConnectionStats {
            total_connections: connections.len(),
            authenticated_connections: connections.values().filter(|c| c.authenticated).count(),
            total_subscriptions: connections.values().map(|c| c.subscriptions.len()).sum(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub authenticated_connections: usize,
    pub total_subscriptions: usize,
}

/// Handle enhanced WebSocket connection
pub async fn handle_enhanced_socket(socket: WebSocket, state: AppState) {
    let conn_id = uuid::Uuid::new_v4().to_string();
    info!("New enhanced WebSocket connection: {}", conn_id);
    
    let (mut sender, mut receiver) = socket.split();
    
    // Register connection
    let enhanced_manager = state.enhanced_ws_manager.as_ref()
        .expect("Enhanced WebSocket manager not initialized");
    enhanced_manager.register_connection(conn_id.clone()).await;
    
    // Create local connection info for filtering
    let connection = WsConnection {
        id: conn_id.clone(),
        subscriptions: Vec::new(),
        authenticated: false,
        user_wallet: None,
        connected_at: Utc::now().timestamp(),
        last_ping: Utc::now().timestamp(),
    };
    
    // Subscribe to broadcast channels
    let mut market_rx = enhanced_manager.market_tx.subscribe();
    let mut position_rx = enhanced_manager.position_tx.subscribe();
    let mut system_rx = enhanced_manager.system_tx.subscribe();
    
    // Send welcome message
    let welcome = EnhancedWsMessage::SystemEvent {
        event_type: "connection".to_string(),
        message: "Connected to Boom Platform WebSocket".to_string(),
        severity: "info".to_string(),
        timestamp: Utc::now().timestamp(),
    };
    
    if let Err(e) = send_message(&mut sender, &welcome).await {
        error!("Failed to send welcome message: {}", e);
        return;
    }
    
    // Start heartbeat task
    let heartbeat_task = tokio::spawn({
        let conn_id = conn_id.clone();
        let manager = enhanced_manager.clone();
        async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                // Check connection health
                let connections = manager.connections.read().await;
                if let Some(conn) = connections.get(&conn_id) {
                    let now = Utc::now().timestamp();
                    if now - conn.last_ping > 60 {
                        warn!("Connection {} appears to be stale", conn_id);
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    });
    
    // Handle messages
    loop {
        tokio::select! {
            // Handle incoming messages from client
            Some(msg) = receiver.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = handle_client_message(
                            &text,
                            &conn_id,
                            &mut sender,
                            enhanced_manager,
                            &state,
                        ).await {
                            error!("Error handling client message: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket {} closed by client", conn_id);
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket {} error: {}", conn_id, e);
                        break;
                    }
                    _ => {}
                }
            }
            
            // Handle market updates
            Ok(msg) = market_rx.recv() => {
                if should_send_message(&connection, &msg) {
                    if let Err(e) = send_message(&mut sender, &msg).await {
                        error!("Failed to send market update: {}", e);
                        break;
                    }
                }
            }
            
            // Handle position updates
            Ok(msg) = position_rx.recv() => {
                if should_send_message(&connection, &msg) {
                    if let Err(e) = send_message(&mut sender, &msg).await {
                        error!("Failed to send position update: {}", e);
                        break;
                    }
                }
            }
            
            // Handle system events
            Ok(msg) = system_rx.recv() => {
                if let Err(e) = send_message(&mut sender, &msg).await {
                    error!("Failed to send system event: {}", e);
                    break;
                }
            }
        }
    }
    
    // Cleanup
    heartbeat_task.abort();
    enhanced_manager.remove_connection(&conn_id).await;
    info!("WebSocket {} disconnected", conn_id);
}

/// Handle client message
async fn handle_client_message(
    text: &str,
    conn_id: &str,
    sender: &mut SplitSink<WebSocket, Message>,
    manager: &EnhancedWebSocketManager,
    _state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: WsRequest = serde_json::from_str(text)?;
    
    match request {
        WsRequest::Subscribe { subscriptions } => {
            manager.update_subscriptions(conn_id, subscriptions.clone()).await?;
            
            for sub in subscriptions {
                let confirmation = EnhancedWsMessage::SubscriptionConfirmed {
                    subscription: sub,
                    status: "active".to_string(),
                };
                send_message(sender, &confirmation).await?;
            }
        }
        
        WsRequest::Unsubscribe { subscriptions } => {
            let mut connections = manager.connections.write().await;
            if let Some(conn) = connections.get_mut(conn_id) {
                conn.subscriptions.retain(|s| !subscriptions.iter().any(|unsub| {
                    matches!((s, unsub), 
                        (Subscription::MarketUpdates { market_id: id1 }, 
                         Subscription::MarketUpdates { market_id: id2 }) if id1 == id2)
                }));
            }
        }
        
        WsRequest::Authenticate { wallet, signature: _ } => {
            // In production, verify signature
            let mut connections = manager.connections.write().await;
            if let Some(conn) = connections.get_mut(conn_id) {
                conn.authenticated = true;
                conn.user_wallet = Some(wallet);
            }
            
            let response = EnhancedWsMessage::SystemEvent {
                event_type: "authentication".to_string(),
                message: "Authentication successful".to_string(),
                severity: "info".to_string(),
                timestamp: Utc::now().timestamp(),
            };
            send_message(sender, &response).await?;
        }
        
        WsRequest::Ping { timestamp } => {
            let mut connections = manager.connections.write().await;
            if let Some(conn) = connections.get_mut(conn_id) {
                conn.last_ping = Utc::now().timestamp();
            }
            
            let pong = EnhancedWsMessage::Heartbeat {
                timestamp,
                server_time: Utc::now().timestamp(),
            };
            send_message(sender, &pong).await?;
        }
        
        WsRequest::GetSnapshot { market_id: _ } => {
            // TODO: Implement market snapshot
            let error = EnhancedWsMessage::Error {
                code: "NOT_IMPLEMENTED".to_string(),
                message: "Market snapshot not yet implemented".to_string(),
                timestamp: Utc::now().timestamp(),
            };
            send_message(sender, &error).await?;
        }
    }
    
    Ok(())
}

/// Check if message should be sent to connection
fn should_send_message(connection: &WsConnection, msg: &EnhancedWsMessage) -> bool {
    match msg {
        EnhancedWsMessage::MarketUpdate { market_id, .. } => {
            connection.subscriptions.iter().any(|sub| {
                matches!(sub, Subscription::MarketUpdates { market_id: id } if id == market_id) ||
                matches!(sub, Subscription::AllMarkets)
            })
        }
        
        EnhancedWsMessage::OrderBookUpdate { market_id, .. } => {
            connection.subscriptions.iter().any(|sub| matches!(sub,
                Subscription::OrderBookUpdates { market_id: id } if id == market_id
            ))
        }
        
        EnhancedWsMessage::PositionUpdate { wallet, .. } => {
            connection.user_wallet.as_ref() == Some(wallet) &&
            connection.subscriptions.iter().any(|sub| matches!(sub,
                Subscription::PositionUpdates { .. }
            ))
        }
        
        EnhancedWsMessage::SystemEvent { .. } |
        EnhancedWsMessage::CircuitBreakerAlert { .. } => {
            connection.subscriptions.iter().any(|sub| matches!(sub,
                Subscription::SystemStatus
            ))
        }
        
        _ => true, // Send other messages to all connections
    }
}

/// Send message to WebSocket
async fn send_message(
    sender: &mut SplitSink<WebSocket, Message>,
    msg: &EnhancedWsMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(msg)?;
    sender.send(Message::Text(json)).await?;
    Ok(())
}

/// Start enhanced market updates
pub async fn start_enhanced_market_updates(state: AppState) {
    let manager = state.enhanced_ws_manager.as_ref()
        .expect("Enhanced WebSocket manager not initialized");
        
    let mut interval = interval(Duration::from_secs(2)); // More frequent updates
    
    loop {
        interval.tick().await;
        
        // Get real market data from platform
        if let Ok(markets) = state.platform_client.get_markets().await {
            for market in markets.iter().take(10) { // Limit to top 10 active markets
                // Calculate prices based on stake distribution
                let total_stake: u64 = market.outcomes.iter().map(|o| o.total_stake).sum();
                let yes_price = if total_stake > 0 && market.outcomes.len() >= 2 {
                    market.outcomes[0].total_stake as f64 / total_stake as f64
                } else {
                    0.5
                };
                let no_price = 1.0 - yes_price;
                
                let update = EnhancedWsMessage::MarketUpdate {
                    market_id: market.id,
                    yes_price,
                    no_price,
                    volume: market.total_volume,
                    liquidity: market.total_liquidity,
                    trades_24h: 0, // Not available in current Market struct
                    timestamp: Utc::now().timestamp(),
                };
                
                manager.broadcast_market_update(update);
            }
        }
        
        // Also check for system events
        check_system_events(manager).await;
        
        // Broadcast order book updates
        broadcast_order_book_updates(&state, manager).await;
    }
}

/// Check for system events to broadcast
async fn check_system_events(manager: &EnhancedWebSocketManager) {
    // Check connection stats
    let stats = manager.get_stats().await;
    if stats.total_connections > 100 {
        let event = EnhancedWsMessage::SystemEvent {
            event_type: "high_load".to_string(),
            message: format!("High connection count: {}", stats.total_connections),
            severity: "warning".to_string(),
            timestamp: Utc::now().timestamp(),
        };
        manager.broadcast_system_event(event);
    }
}

/// Broadcast order book updates for active markets
async fn broadcast_order_book_updates(state: &AppState, manager: &EnhancedWebSocketManager) {
    // Get active markets from the client
    if let Ok(markets) = state.platform_client.get_markets().await {
        for market in markets {
            // Calculate base price from market data
            let base_price = if market.outcomes.len() >= 2 && market.total_volume > 0 {
                // Simple price calculation based on stakes
                let yes_stake = market.outcomes[0].total_stake as f64;
                let no_stake = market.outcomes[1].total_stake as f64;
                let total = yes_stake + no_stake;
                if total > 0.0 {
                    yes_stake / total
                } else {
                    0.5
                }
            } else {
                0.5
            };
            
            // Generate sample order book data (in production, this would come from actual order matching)
            let bids: Vec<OrderLevel> = (0..5).map(|i| OrderLevel {
                price: base_price - (i as f64 * 0.01),
                amount: (1000 * (5 - i)) as u64,
                orders: (5 - i) as u32,
            }).collect();
            
            let asks: Vec<OrderLevel> = (0..5).map(|i| OrderLevel {
                price: base_price + (i as f64 * 0.01),
                amount: (1000 * (5 - i)) as u64,
                orders: (5 - i) as u32,
            }).collect();
            
            let update = EnhancedWsMessage::OrderBookUpdate {
                market_id: market.id,
                bids,
                asks,
                spread: 0.02,
                mid_price: base_price,
                timestamp: Utc::now().timestamp(),
            };
            
            manager.broadcast_order_book_update(update);
        }
    }
}

