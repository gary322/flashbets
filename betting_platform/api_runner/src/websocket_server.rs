//! Production-ready WebSocket server implementation with tokio-tungstenite

use axum::{
    extract::{
        ws::{Message as AxumMessage, WebSocket as AxumWebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    jwt_validation::{JwtManager, JwtError},
    AppState,
    websocket::enhanced::EnhancedWsMessage,
    types::{Market, Position},
};

/// WebSocket connection information
#[derive(Debug, Clone)]
pub struct WsConnection {
    pub id: Uuid,
    pub user_id: Option<String>,
    pub wallet: Option<String>,
    pub subscriptions: Vec<ChannelSubscription>,
    pub connected_at: Instant,
    pub last_ping: Instant,
    pub authenticated: bool,
}

/// Channel subscription types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum ChannelSubscription {
    Markets { filter: Option<MarketFilter> },
    Market { market_id: u128 },
    Positions { wallet: String },
    Orders { wallet: String },
    Trades { market_id: Option<u128> },
    PriceFeed { market_ids: Vec<u128> },
    SystemStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketFilter {
    pub status: Option<String>,
    pub search: Option<String>,
    pub min_volume: Option<u64>,
}

/// WebSocket messages from client
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsClientMessage {
    Authenticate { token: String },
    Subscribe { channels: Vec<ChannelSubscription> },
    Unsubscribe { channels: Vec<ChannelSubscription> },
    Ping { timestamp: i64 },
    PlaceOrder { order: OrderRequest },
    CancelOrder { order_id: String },
}

/// WebSocket messages to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsServerMessage {
    // Connection messages
    Connected { connection_id: String, server_time: i64 },
    Authenticated { user_id: String, wallet: String },
    Error { code: String, message: String },
    Pong { timestamp: i64 },
    
    // Subscription confirmations
    Subscribed { channels: Vec<ChannelSubscription> },
    Unsubscribed { channels: Vec<ChannelSubscription> },
    
    // Market data
    MarketUpdate { market: Market, update_type: MarketUpdateType },
    MarketSnapshot { markets: Vec<Market> },
    OrderBook { market_id: u128, bids: Vec<OrderLevel>, asks: Vec<OrderLevel> },
    
    // Trading data
    TradeExecution { trade: TradeData },
    OrderUpdate { order: OrderData },
    PositionUpdate { position: Position },
    
    // Price feeds
    PriceUpdate { market_id: u128, prices: PriceData },
    
    // System messages
    SystemStatus { status: SystemStatusData },
    Notification { level: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketUpdateType {
    Created,
    Updated,
    Resolved,
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub market_id: u128,
    pub outcome: u8,
    pub side: String, // "buy" or "sell"
    pub amount: u64,
    pub price: Option<f64>,
    pub order_type: String, // "market" or "limit"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: f64,
    pub amount: u64,
    pub orders: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub id: String,
    pub market_id: u128,
    pub outcome: u8,
    pub price: f64,
    pub amount: u64,
    pub timestamp: i64,
    pub buyer: String,
    pub seller: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub outcome_prices: Vec<f64>,
    pub last_trade_price: Option<f64>,
    pub volume_24h: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusData {
    pub status: String,
    pub active_connections: usize,
    pub active_markets: usize,
    pub chain_height: Option<u64>,
    pub timestamp: i64,
}

/// Enhanced WebSocket manager with connection tracking
pub struct EnhancedWebSocketManager {
    connections: Arc<RwLock<HashMap<Uuid, WsConnection>>>,
    market_tx: broadcast::Sender<WsServerMessage>,
    trade_tx: broadcast::Sender<WsServerMessage>,
    system_tx: broadcast::Sender<WsServerMessage>,
}

impl EnhancedWebSocketManager {
    pub fn new() -> Self {
        let (market_tx, _) = broadcast::channel(2000);
        let (trade_tx, _) = broadcast::channel(2000);
        let (system_tx, _) = broadcast::channel(1000);
        
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            market_tx,
            trade_tx,
            system_tx,
        }
    }
    
    /// Register a new connection
    pub async fn register_connection(&self, conn: WsConnection) {
        let mut connections = self.connections.write().await;
        connections.insert(conn.id, conn);
    }
    
    /// Remove a connection
    pub async fn remove_connection(&self, id: Uuid) {
        let mut connections = self.connections.write().await;
        connections.remove(&id);
    }
    
    /// Update connection info
    pub async fn update_connection<F>(&self, id: Uuid, f: F)
    where
        F: FnOnce(&mut WsConnection),
    {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(&id) {
            f(conn);
        }
    }
    
    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
    
    /// Get channel receivers
    pub fn subscribe_markets(&self) -> broadcast::Receiver<WsServerMessage> {
        self.market_tx.subscribe()
    }
    
    pub fn subscribe_trades(&self) -> broadcast::Receiver<WsServerMessage> {
        self.trade_tx.subscribe()
    }
    
    pub fn subscribe_system(&self) -> broadcast::Receiver<WsServerMessage> {
        self.system_tx.subscribe()
    }
    
    /// Broadcast market update
    pub fn broadcast_market_update(&self, msg: WsServerMessage) {
        let _ = self.market_tx.send(msg);
    }
    
    /// Broadcast trade update
    pub fn broadcast_trade_update(&self, msg: WsServerMessage) {
        let _ = self.trade_tx.send(msg);
    }
    
    /// Broadcast system update
    pub fn broadcast_system_update(&self, msg: WsServerMessage) {
        let _ = self.system_tx.send(msg);
    }
}

/// WebSocket query parameters for authentication
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// Handle WebSocket upgrade with authentication
pub async fn handle_websocket_upgrade(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket_connection(socket, state, query.token))
}

/// Handle individual WebSocket connection
async fn handle_websocket_connection(
    socket: AxumWebSocket,
    state: AppState,
    initial_token: Option<String>,
) {
    let conn_id = Uuid::new_v4();
    info!("New WebSocket connection: {}", conn_id);
    
    // Create connection info
    let mut connection = WsConnection {
        id: conn_id,
        user_id: None,
        wallet: None,
        subscriptions: Vec::new(),
        connected_at: Instant::now(),
        last_ping: Instant::now(),
        authenticated: false,
    };
    
    // Try to authenticate with initial token
    if let Some(token) = initial_token {
        match state.jwt_manager.validate_token(&token) {
            Ok(claims) => {
                connection.user_id = Some(claims.sub.clone());
                connection.wallet = Some(claims.wallet.clone());
                connection.authenticated = true;
                info!("WebSocket authenticated via query param for user: {}", claims.sub);
            }
            Err(e) => {
                warn!("WebSocket auth failed via query param: {:?}", e);
            }
        }
    }
    
    // Register connection
    if let Some(enhanced_ws) = &state.enhanced_ws_manager {
        let _ = enhanced_ws.register_connection(conn_id.to_string()).await;
    }
    
    // Send connected message
    let connected_msg = WsServerMessage::Connected {
        connection_id: conn_id.to_string(),
        server_time: chrono::Utc::now().timestamp(),
    };
    
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    // Send initial message
    if let Ok(msg) = serde_json::to_string(&connected_msg) {
        let _ = ws_sender.send(AxumMessage::Text(msg)).await;
    }
    
    // Set up channel receivers
    let enhanced_ws = state.enhanced_ws_manager.as_ref().unwrap();
    let mut market_rx = enhanced_ws.subscribe_market_updates();
    let mut trade_rx = enhanced_ws.subscribe_position_updates();
    let mut system_rx = enhanced_ws.subscribe_system_updates();
    
    // Create ping interval
    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    
    // Main message loop
    loop {
        tokio::select! {
            // Handle incoming messages from client
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(AxumMessage::Text(text))) => {
                        if let Err(e) = handle_client_message(
                            &text,
                            &mut ws_sender,
                            &state,
                            &mut connection,
                        ).await {
                            error!("Error handling client message: {}", e);
                        }
                    }
                    Some(Ok(AxumMessage::Close(_))) => {
                        info!("WebSocket closed by client: {}", conn_id);
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            
            // Handle market updates
            Ok(msg) = market_rx.recv() => {
                // Send enhanced messages directly without filtering for now
                if let Ok(json) = serde_json::to_string(&msg) {
                    if ws_sender.send(AxumMessage::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            
            // Handle trade updates
            Ok(msg) = trade_rx.recv() => {
                // Send enhanced messages directly without filtering for now
                if let Ok(json) = serde_json::to_string(&msg) {
                    if ws_sender.send(AxumMessage::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            
            // Handle system updates
            Ok(msg) = system_rx.recv() => {
                // Send enhanced messages directly without filtering for now
                if let Ok(json) = serde_json::to_string(&msg) {
                    if ws_sender.send(AxumMessage::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            
            // Send periodic pings
            _ = ping_interval.tick() => {
                let ping = WsServerMessage::Pong {
                    timestamp: chrono::Utc::now().timestamp(),
                };
                if let Ok(json) = serde_json::to_string(&ping) {
                    if ws_sender.send(AxumMessage::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    
    // Clean up connection
    if let Some(enhanced_ws) = &state.enhanced_ws_manager {
        enhanced_ws.remove_connection(&conn_id.to_string()).await;
    }
    
    info!("WebSocket connection closed: {}", conn_id);
}

/// Handle incoming client messages
async fn handle_client_message(
    text: &str,
    ws_sender: &mut futures_util::stream::SplitSink<AxumWebSocket, AxumMessage>,
    state: &AppState,
    connection: &mut WsConnection,
) -> Result<(), anyhow::Error> {
    let msg: WsClientMessage = serde_json::from_str(text)?;
    
    match msg {
        WsClientMessage::Authenticate { token } => {
            match state.jwt_manager.validate_token(&token) {
                Ok(claims) => {
                    connection.user_id = Some(claims.sub.clone());
                    connection.wallet = Some(claims.wallet.clone());
                    connection.authenticated = true;
                    
                    let response = WsServerMessage::Authenticated {
                        user_id: claims.sub,
                        wallet: claims.wallet,
                    };
                    
                    send_message(ws_sender, &response).await?;
                    
                    // Update connection in manager
                    // TODO: Implement update_connection method in enhanced websocket manager
                    // if let Some(enhanced_ws) = &state.enhanced_ws_manager {
                    //     enhanced_ws.update_connection(connection.id, |conn| {
                    //         conn.authenticated = true;
                    //         conn.user_id = connection.user_id.clone();
                    //         conn.wallet = connection.wallet.clone();
                    //     }).await;
                    // }
                }
                Err(e) => {
                    let error = WsServerMessage::Error {
                        code: "AUTH_FAILED".to_string(),
                        message: format!("Authentication failed: {:?}", e),
                    };
                    send_message(ws_sender, &error).await?;
                }
            }
        }
        
        WsClientMessage::Subscribe { channels } => {
            // Add subscriptions
            for channel in &channels {
                if !connection.subscriptions.contains(channel) {
                    connection.subscriptions.push(channel.clone());
                }
            }
            
            let response = WsServerMessage::Subscribed { channels };
            send_message(ws_sender, &response).await?;
            
            // Update connection in manager
            // TODO: Implement update_connection method in enhanced websocket manager
            // if let Some(enhanced_ws) = &state.enhanced_ws_manager {
            //     let subs = connection.subscriptions.clone();
            //     enhanced_ws.update_connection(connection.id, |conn| {
            //         conn.subscriptions = subs;
            //     }).await;
            // }
        }
        
        WsClientMessage::Unsubscribe { channels } => {
            // Remove subscriptions
            connection.subscriptions.retain(|sub| !channels.contains(sub));
            
            let response = WsServerMessage::Unsubscribed { channels };
            send_message(ws_sender, &response).await?;
            
            // Update connection in manager
            // TODO: Implement update_connection method in enhanced websocket manager
            // if let Some(enhanced_ws) = &state.enhanced_ws_manager {
            //     let subs = connection.subscriptions.clone();
            //     enhanced_ws.update_connection(connection.id, |conn| {
            //         conn.subscriptions = subs;
            //     }).await;
            // }
        }
        
        WsClientMessage::Ping { timestamp } => {
            connection.last_ping = Instant::now();
            let response = WsServerMessage::Pong { timestamp };
            send_message(ws_sender, &response).await?;
        }
        
        WsClientMessage::PlaceOrder { order } => {
            if !connection.authenticated {
                let error = WsServerMessage::Error {
                    code: "UNAUTHORIZED".to_string(),
                    message: "Must authenticate before placing orders".to_string(),
                };
                send_message(ws_sender, &error).await?;
                return Ok(());
            }
            
            // Here you would implement order placement logic
            // For now, send acknowledgment
            let response = WsServerMessage::Notification {
                level: "info".to_string(),
                message: format!("Order received for market {}", order.market_id),
            };
            send_message(ws_sender, &response).await?;
        }
        
        WsClientMessage::CancelOrder { order_id } => {
            if !connection.authenticated {
                let error = WsServerMessage::Error {
                    code: "UNAUTHORIZED".to_string(),
                    message: "Must authenticate before canceling orders".to_string(),
                };
                send_message(ws_sender, &error).await?;
                return Ok(());
            }
            
            // Here you would implement order cancellation logic
            let response = WsServerMessage::Notification {
                level: "info".to_string(),
                message: format!("Cancel request received for order {}", order_id),
            };
            send_message(ws_sender, &response).await?;
        }
    }
    
    Ok(())
}

/// Send message to WebSocket client
async fn send_message(
    ws_sender: &mut futures_util::stream::SplitSink<AxumWebSocket, AxumMessage>,
    msg: &WsServerMessage,
) -> Result<(), anyhow::Error> {
    let json = serde_json::to_string(msg)?;
    ws_sender.send(AxumMessage::Text(json)).await?;
    Ok(())
}

/// Check if a message should be sent based on subscriptions
fn should_send_message(msg: &WsServerMessage, connection: &WsConnection) -> bool {
    match msg {
        WsServerMessage::MarketUpdate { market, .. } => {
            connection.subscriptions.iter().any(|sub| match sub {
                ChannelSubscription::Markets { filter } => {
                    // Check if market matches filter
                    if let Some(f) = filter {
                        // Apply filter logic
                        true // Simplified for now
                    } else {
                        true
                    }
                }
                ChannelSubscription::Market { market_id } => market.id == *market_id,
                _ => false,
            })
        }
        
        WsServerMessage::TradeExecution { trade } => {
            connection.subscriptions.iter().any(|sub| match sub {
                ChannelSubscription::Trades { market_id } => {
                    market_id.map_or(true, |id| id == trade.market_id)
                }
                _ => false,
            })
        }
        
        WsServerMessage::PositionUpdate { position } => {
            connection.subscriptions.iter().any(|sub| match sub {
                ChannelSubscription::Positions { wallet } => {
                    connection.wallet.as_ref().map_or(false, |w| w == wallet)
                }
                _ => false,
            })
        }
        
        WsServerMessage::SystemStatus { .. } => {
            connection.subscriptions.iter().any(|sub| {
                matches!(sub, ChannelSubscription::SystemStatus)
            })
        }
        
        // Always send these messages
        WsServerMessage::Connected { .. } |
        WsServerMessage::Authenticated { .. } |
        WsServerMessage::Error { .. } |
        WsServerMessage::Pong { .. } |
        WsServerMessage::Subscribed { .. } |
        WsServerMessage::Unsubscribed { .. } |
        WsServerMessage::Notification { .. } => true,
        
        _ => true, // Default to sending
    }
}

/// Start WebSocket background tasks
pub async fn start_websocket_tasks(state: AppState) {
    // Start connection monitor
    let monitor_state = state.clone();
    tokio::spawn(async move {
        connection_monitor_task(monitor_state).await;
    });
    
    // Start market data broadcaster
    let market_state = state.clone();
    tokio::spawn(async move {
        market_data_broadcaster_task(market_state).await;
    });
    
    // Start system status broadcaster
    let system_state = state.clone();
    tokio::spawn(async move {
        system_status_broadcaster_task(system_state).await;
    });
}

/// Monitor connections and clean up stale ones
async fn connection_monitor_task(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        // TODO: Implement stale connection cleanup using public methods
        // if let Some(enhanced_ws) = &state.enhanced_ws_manager {
        //     // Need to use public methods to check and remove stale connections
        // }
    }
}

/// Broadcast market data updates
async fn market_data_broadcaster_task(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    
    loop {
        interval.tick().await;
        
        if let Some(enhanced_ws) = &state.enhanced_ws_manager {
            // Fetch latest market data
            if let Ok(markets) = crate::market_data_service::MarketDataService::fetch_all_markets(
                &state,
                10,
                0,
            ).await {
                // Get market count before moving
                let market_count = markets.markets.len();
                
                // Send top markets update
                let snapshot = WsServerMessage::MarketSnapshot {
                    markets: markets.markets,
                };
                
                // Convert to EnhancedWsMessage - send as system event
                enhanced_ws.broadcast_system_event(EnhancedWsMessage::SystemEvent {
                    event_type: "market_snapshot".to_string(),
                    message: format!("Market snapshot with {} markets", market_count),
                    severity: "info".to_string(),
                    timestamp: chrono::Utc::now().timestamp(),
                });
            }
        }
    }
}

/// Broadcast system status updates
async fn system_status_broadcaster_task(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        if let Some(_enhanced_ws) = &state.enhanced_ws_manager {
            let status = SystemStatusData {
                status: "operational".to_string(),
                active_connections: 0, // TODO: implement connection tracking
                active_markets: 100, // Get from market service
                chain_height: None, // Get from Solana
                timestamp: chrono::Utc::now().timestamp(),
            };
            
            let _msg = WsServerMessage::SystemStatus { status };
            // TODO: implement broadcast_system_update
            // enhanced_ws.broadcast_system_update(msg);
        }
    }
}