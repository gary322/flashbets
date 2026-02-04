//! Polymarket WebSocket Client
//! Handles real-time updates from Polymarket CLOB

use anyhow::{Result, anyhow, Context};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as WsMessage, protocol::CloseFrame},
    WebSocketStream,
    MaybeTlsStream,
};
use tokio::net::TcpStream;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};
use std::time::Duration;
use std::pin::Pin;
use std::future::Future;

const WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com";
const WS_URL_TESTNET: &str = "wss://ws-subscriptions-clob.polymarket.com"; // Update for testnet

/// WebSocket client for Polymarket
pub struct PolymarketWsClient {
    url: String,
    auth_token: Option<String>,
    subscriptions: Arc<RwLock<Vec<Subscription>>>,
    event_sender: mpsc::UnboundedSender<MarketEvent>,
    event_receiver: Option<mpsc::UnboundedReceiver<MarketEvent>>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
}

impl PolymarketWsClient {
    /// Create new WebSocket client
    pub fn new(auth_token: Option<String>, testnet: bool) -> Self {
        let url = if testnet {
            WS_URL_TESTNET.to_string()
        } else {
            WS_URL.to_string()
        };
        
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            url,
            auth_token,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            event_sender,
            event_receiver: Some(event_receiver),
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
        }
    }
    
    /// Connect to WebSocket and start listening
    pub fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
        info!("Connecting to Polymarket WebSocket at {}", self.url);
        
        let (mut ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to WebSocket")?;
        
        info!("WebSocket connected successfully");
        
        // Authenticate if token is provided
        if let Some(token) = &self.auth_token {
            self.authenticate(&mut ws_stream, token).await?;
        }
        
        // Resubscribe to existing subscriptions
        let subs = self.subscriptions.read().await.clone();
        for sub in subs {
            self.send_subscription(&mut ws_stream, &sub).await?;
        }
        
        // Start message handler
        self.handle_messages(ws_stream).await;
        
        Ok(())
        })
    }
    
    /// Authenticate with the WebSocket
    async fn authenticate(
        &self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        token: &str,
    ) -> Result<()> {
        let auth_msg = serde_json::json!({
            "type": "authenticate",
            "token": token,
        });
        
        ws.send(WsMessage::Text(auth_msg.to_string()))
            .await
            .context("Failed to send auth message")?;
        
        Ok(())
    }
    
    /// Subscribe to market updates
    pub async fn subscribe_market(&mut self, market_id: String) -> Result<()> {
        let subscription = Subscription::Market { market_id };
        self.add_subscription(subscription).await
    }
    
    /// Subscribe to order updates for a user
    pub async fn subscribe_orders(&mut self, address: String) -> Result<()> {
        let subscription = Subscription::Orders { address };
        self.add_subscription(subscription).await
    }
    
    /// Subscribe to trades for a market
    pub async fn subscribe_trades(&mut self, market_id: String) -> Result<()> {
        let subscription = Subscription::Trades { market_id };
        self.add_subscription(subscription).await
    }
    
    /// Subscribe to order book updates
    pub async fn subscribe_order_book(&mut self, token_id: String) -> Result<()> {
        let subscription = Subscription::OrderBook { token_id };
        self.add_subscription(subscription).await
    }
    
    /// Add subscription and send to WebSocket
    async fn add_subscription(&mut self, subscription: Subscription) -> Result<()> {
        self.subscriptions.write().await.push(subscription.clone());
        
        // If connected, send subscription immediately
        // This would be done through the active WebSocket connection
        info!("Added subscription: {:?}", subscription);
        
        Ok(())
    }
    
    /// Send subscription message
    async fn send_subscription(
        &self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        subscription: &Subscription,
    ) -> Result<()> {
        let msg = match subscription {
            Subscription::Market { market_id } => {
                serde_json::json!({
                    "type": "subscribe",
                    "channel": "market",
                    "market_id": market_id,
                })
            }
            Subscription::Orders { address } => {
                serde_json::json!({
                    "type": "subscribe",
                    "channel": "orders",
                    "address": address,
                })
            }
            Subscription::Trades { market_id } => {
                serde_json::json!({
                    "type": "subscribe",
                    "channel": "trades",
                    "market_id": market_id,
                })
            }
            Subscription::OrderBook { token_id } => {
                serde_json::json!({
                    "type": "subscribe",
                    "channel": "book",
                    "token_id": token_id,
                })
            }
        };
        
        ws.send(WsMessage::Text(msg.to_string()))
            .await
            .context("Failed to send subscription")?;
        
        debug!("Sent subscription: {}", msg);
        
        Ok(())
    }
    
    /// Handle incoming WebSocket messages
    async fn handle_messages(&mut self, mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>) {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = ws.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(text))) => {
                            if let Err(e) = self.process_message(&text).await {
                                error!("Failed to process message: {}", e);
                            }
                        }
                        Some(Ok(WsMessage::Binary(bin))) => {
                            debug!("Received binary message: {} bytes", bin.len());
                        }
                        Some(Ok(WsMessage::Ping(data))) => {
                            if ws.send(WsMessage::Pong(data)).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(WsMessage::Pong(_))) => {
                            debug!("Received pong");
                        }
                        Some(Ok(WsMessage::Close(frame))) => {
                            info!("WebSocket closed: {:?}", frame);
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }
                
                // Send periodic ping
                _ = ping_interval.tick() => {
                    if ws.send(WsMessage::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
            }
        }
        
        // Attempt reconnection
        self.reconnect().await;
    }
    
    /// Process incoming message
    async fn process_message(&self, text: &str) -> Result<()> {
        let msg: Value = serde_json::from_str(text)
            .context("Failed to parse WebSocket message")?;
        
        let msg_type = msg["type"].as_str().unwrap_or("");
        
        match msg_type {
            "market_update" => {
                let event = MarketEvent::MarketUpdate {
                    market_id: msg["market_id"].as_str().unwrap_or("").to_string(),
                    price: msg["price"].as_f64().unwrap_or(0.0),
                    volume: msg["volume"].as_f64().unwrap_or(0.0),
                    timestamp: Utc::now(),
                };
                self.event_sender.send(event)?;
            }
            "order_update" => {
                let event = MarketEvent::OrderUpdate {
                    order_id: msg["order_id"].as_str().unwrap_or("").to_string(),
                    status: msg["status"].as_str().unwrap_or("").to_string(),
                    filled_amount: msg["filled_amount"].as_str().unwrap_or("0").to_string(),
                    timestamp: Utc::now(),
                };
                self.event_sender.send(event)?;
            }
            "trade" => {
                let event = MarketEvent::Trade {
                    trade_id: msg["trade_id"].as_str().unwrap_or("").to_string(),
                    market_id: msg["market_id"].as_str().unwrap_or("").to_string(),
                    price: msg["price"].as_f64().unwrap_or(0.0),
                    size: msg["size"].as_f64().unwrap_or(0.0),
                    side: msg["side"].as_str().unwrap_or("").to_string(),
                    timestamp: Utc::now(),
                };
                self.event_sender.send(event)?;
            }
            "book_update" => {
                let event = MarketEvent::BookUpdate {
                    token_id: msg["token_id"].as_str().unwrap_or("").to_string(),
                    bids: parse_book_levels(&msg["bids"]),
                    asks: parse_book_levels(&msg["asks"]),
                    timestamp: Utc::now(),
                };
                self.event_sender.send(event)?;
            }
            "error" => {
                error!("WebSocket error: {}", msg["message"].as_str().unwrap_or("Unknown"));
            }
            "subscribed" => {
                info!("Successfully subscribed to {}", msg["channel"].as_str().unwrap_or(""));
            }
            "authenticated" => {
                info!("Successfully authenticated");
            }
            _ => {
                debug!("Unknown message type: {}", msg_type);
            }
        }
        
        Ok(())
    }
    
    /// Reconnect to WebSocket
    fn reconnect(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            if self.reconnect_attempts >= self.max_reconnect_attempts {
                error!("Max reconnection attempts reached");
                return;
            }
            
            self.reconnect_attempts += 1;
            let delay = Duration::from_secs(2u64.pow(self.reconnect_attempts));
            
            info!("Reconnecting in {:?} (attempt {})", delay, self.reconnect_attempts);
            tokio::time::sleep(delay).await;
            
            if let Err(e) = self.connect().await {
                error!("Reconnection failed: {}", e);
                self.reconnect().await;
            } else {
                self.reconnect_attempts = 0;
            }
        })
    }
    
    /// Get event receiver
    pub fn get_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<MarketEvent>> {
        self.event_receiver.take()
    }
}

/// Parse order book levels from JSON
fn parse_book_levels(value: &Value) -> Vec<BookLevel> {
    if let Some(levels) = value.as_array() {
        levels
            .iter()
            .filter_map(|level| {
                if let (Some(price), Some(size)) = (
                    level[0].as_f64(),
                    level[1].as_f64(),
                ) {
                    Some(BookLevel { price, size })
                } else {
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

/// Subscription types
#[derive(Debug, Clone)]
enum Subscription {
    Market { market_id: String },
    Orders { address: String },
    Trades { market_id: String },
    OrderBook { token_id: String },
}

/// Market events from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    MarketUpdate {
        market_id: String,
        price: f64,
        volume: f64,
        timestamp: DateTime<Utc>,
    },
    OrderUpdate {
        order_id: String,
        status: String,
        filled_amount: String,
        timestamp: DateTime<Utc>,
    },
    Trade {
        trade_id: String,
        market_id: String,
        price: f64,
        size: f64,
        side: String,
        timestamp: DateTime<Utc>,
    },
    BookUpdate {
        token_id: String,
        bids: Vec<BookLevel>,
        asks: Vec<BookLevel>,
        timestamp: DateTime<Utc>,
    },
}

/// Order book level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: f64,
    pub size: f64,
}

/// WebSocket manager for handling multiple connections
pub struct PolymarketWsManager {
    clients: Vec<Arc<RwLock<PolymarketWsClient>>>,
    event_aggregator: mpsc::UnboundedSender<MarketEvent>,
}

impl PolymarketWsManager {
    /// Create new WebSocket manager
    pub fn new() -> (Self, mpsc::UnboundedReceiver<MarketEvent>) {
        let (event_aggregator, event_receiver) = mpsc::unbounded_channel();
        
        (
            Self {
                clients: Vec::new(),
                event_aggregator,
            },
            event_receiver,
        )
    }
    
    /// Add a new WebSocket client
    pub async fn add_client(&mut self, mut client: PolymarketWsClient) -> Result<()> {
        // Connect the client
        client.connect().await?;
        
        // Get event receiver and forward to aggregator
        if let Some(mut receiver) = client.get_event_receiver() {
            let aggregator = self.event_aggregator.clone();
            tokio::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    let _ = aggregator.send(event);
                }
            });
        }
        
        self.clients.push(Arc::new(RwLock::new(client)));
        Ok(())
    }
    
    /// Subscribe all clients to a market
    pub async fn subscribe_market_all(&self, market_id: String) -> Result<()> {
        for client in &self.clients {
            client.write().await.subscribe_market(market_id.clone()).await?;
        }
        Ok(())
    }
}