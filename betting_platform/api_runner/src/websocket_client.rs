//! WebSocket client implementation for testing and SDK

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error as WsError, Message},
};
use tracing::{debug, error, info};
use url::Url;

use crate::websocket_server::{
    ChannelSubscription, WsClientMessage, WsServerMessage, OrderRequest,
};

/// WebSocket client configuration
#[derive(Debug, Clone)]
pub struct WsClientConfig {
    pub url: String,
    pub auth_token: Option<String>,
    pub auto_reconnect: bool,
    pub reconnect_interval: std::time::Duration,
    pub ping_interval: std::time::Duration,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8081/ws/v2".to_string(),
            auth_token: None,
            auto_reconnect: true,
            reconnect_interval: std::time::Duration::from_secs(5),
            ping_interval: std::time::Duration::from_secs(30),
        }
    }
}

/// WebSocket client events
#[derive(Debug, Clone)]
pub enum WsClientEvent {
    Connected { connection_id: String },
    Disconnected { reason: String },
    Message(WsServerMessage),
    Error { error: String },
}

/// WebSocket client handle for sending commands
#[derive(Clone)]
pub struct WsClientHandle {
    tx: mpsc::UnboundedSender<WsClientCommand>,
}

impl WsClientHandle {
    /// Authenticate with token
    pub async fn authenticate(&self, token: String) -> Result<(), String> {
        self.send_command(WsClientCommand::Authenticate { token })
    }
    
    /// Subscribe to channels
    pub async fn subscribe(&self, channels: Vec<ChannelSubscription>) -> Result<(), String> {
        self.send_command(WsClientCommand::Subscribe { channels })
    }
    
    /// Unsubscribe from channels
    pub async fn unsubscribe(&self, channels: Vec<ChannelSubscription>) -> Result<(), String> {
        self.send_command(WsClientCommand::Unsubscribe { channels })
    }
    
    /// Place an order
    pub async fn place_order(&self, order: OrderRequest) -> Result<(), String> {
        self.send_command(WsClientCommand::PlaceOrder { order })
    }
    
    /// Cancel an order
    pub async fn cancel_order(&self, order_id: String) -> Result<(), String> {
        self.send_command(WsClientCommand::CancelOrder { order_id })
    }
    
    /// Close the connection
    pub async fn close(&self) -> Result<(), String> {
        self.send_command(WsClientCommand::Close)
    }
    
    fn send_command(&self, cmd: WsClientCommand) -> Result<(), String> {
        self.tx.send(cmd).map_err(|_| "Failed to send command".to_string())
    }
}

/// Internal client commands
#[derive(Debug)]
enum WsClientCommand {
    Authenticate { token: String },
    Subscribe { channels: Vec<ChannelSubscription> },
    Unsubscribe { channels: Vec<ChannelSubscription> },
    PlaceOrder { order: OrderRequest },
    CancelOrder { order_id: String },
    Close,
}

/// WebSocket client
pub struct WsClient {
    config: WsClientConfig,
    event_tx: mpsc::UnboundedSender<WsClientEvent>,
}

impl WsClient {
    /// Create new WebSocket client
    pub fn new(
        config: WsClientConfig,
    ) -> (Self, WsClientHandle, mpsc::UnboundedReceiver<WsClientEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        
        let client = Self {
            config,
            event_tx,
        };
        
        let handle = WsClientHandle { tx: cmd_tx };
        
        // Start client task
        let client_clone = client.clone();
        tokio::spawn(async move {
            client_clone.run(cmd_rx).await;
        });
        
        (client, handle, event_rx)
    }
    
    /// Clone the client (for internal use)
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            event_tx: self.event_tx.clone(),
        }
    }
    
    /// Run the client
    async fn run(self, mut cmd_rx: mpsc::UnboundedReceiver<WsClientCommand>) {
        loop {
            match self.connect_and_run(&mut cmd_rx).await {
                Ok(_) => {
                    info!("WebSocket client disconnected normally");
                    if !self.config.auto_reconnect {
                        break;
                    }
                }
                Err(e) => {
                    error!("WebSocket client error: {}", e);
                    self.send_event(WsClientEvent::Error {
                        error: e.to_string(),
                    });
                    
                    if !self.config.auto_reconnect {
                        break;
                    }
                }
            }
            
            if self.config.auto_reconnect {
                info!("Reconnecting in {:?}", self.config.reconnect_interval);
                tokio::time::sleep(self.config.reconnect_interval).await;
            }
        }
    }
    
    /// Connect and handle messages
    async fn connect_and_run(
        &self,
        cmd_rx: &mut mpsc::UnboundedReceiver<WsClientCommand>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Build URL with optional token
        let mut url = Url::parse(&self.config.url)?;
        if let Some(token) = &self.config.auth_token {
            url.query_pairs_mut().append_pair("token", token);
        }
        
        info!("Connecting to WebSocket: {}", url);
        let (ws_stream, _) = connect_async(url).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Create ping interval
        let mut ping_interval = tokio::time::interval(self.config.ping_interval);
        
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(server_msg) = serde_json::from_str::<WsServerMessage>(&text) {
                                // Handle connected message
                                if let WsServerMessage::Connected { connection_id, .. } = &server_msg {
                                    self.send_event(WsClientEvent::Connected {
                                        connection_id: connection_id.clone(),
                                    });
                                }
                                
                                self.send_event(WsClientEvent::Message(server_msg));
                            } else {
                                debug!("Failed to parse server message: {}", text);
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            self.send_event(WsClientEvent::Disconnected {
                                reason: "Server closed connection".to_string(),
                            });
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            return Err(Box::new(e));
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }
                
                // Handle client commands
                cmd = cmd_rx.recv() => {
                    if let Some(command) = cmd {
                        match command {
                            WsClientCommand::Authenticate { token } => {
                                let msg = WsClientMessage::Authenticate { token };
                                self.send_message(&mut ws_sender, msg).await?;
                            }
                            WsClientCommand::Subscribe { channels } => {
                                let msg = WsClientMessage::Subscribe { channels };
                                self.send_message(&mut ws_sender, msg).await?;
                            }
                            WsClientCommand::Unsubscribe { channels } => {
                                let msg = WsClientMessage::Unsubscribe { channels };
                                self.send_message(&mut ws_sender, msg).await?;
                            }
                            WsClientCommand::PlaceOrder { order } => {
                                let msg = WsClientMessage::PlaceOrder { order };
                                self.send_message(&mut ws_sender, msg).await?;
                            }
                            WsClientCommand::CancelOrder { order_id } => {
                                let msg = WsClientMessage::CancelOrder { order_id };
                                self.send_message(&mut ws_sender, msg).await?;
                            }
                            WsClientCommand::Close => {
                                info!("Closing WebSocket connection");
                                ws_sender.close().await?;
                                break;
                            }
                        }
                    }
                }
                
                // Send periodic pings
                _ = ping_interval.tick() => {
                    let msg = WsClientMessage::Ping {
                        timestamp: chrono::Utc::now().timestamp(),
                    };
                    self.send_message(&mut ws_sender, msg).await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Send message to server
    async fn send_message<T>(
        &self,
        ws_sender: &mut futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
            >,
            Message
        >,
        msg: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize,
    {
        let json = serde_json::to_string(&msg)?;
        ws_sender.send(Message::Text(json)).await?;
        Ok(())
    }
    
    /// Send event to application
    fn send_event(&self, event: WsClientEvent) {
        let _ = self.event_tx.send(event);
    }
}

/// Example WebSocket client usage
#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_websocket_client() {
        // Create client
        let config = WsClientConfig {
            url: "ws://localhost:8081/ws/v2".to_string(),
            auth_token: Some("test_token".to_string()),
            ..Default::default()
        };
        
        let (_client, handle, mut events) = WsClient::new(config);
        
        // Subscribe to markets
        let _ = handle.subscribe(vec![
            ChannelSubscription::Markets { filter: None },
            ChannelSubscription::SystemStatus,
        ]).await;
        
        // Listen for events
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                match event {
                    WsClientEvent::Connected { connection_id } => {
                        println!("Connected: {}", connection_id);
                    }
                    WsClientEvent::Message(msg) => {
                        println!("Message: {:?}", msg);
                    }
                    WsClientEvent::Error { error } => {
                        println!("Error: {}", error);
                    }
                    WsClientEvent::Disconnected { reason } => {
                        println!("Disconnected: {}", reason);
                    }
                }
            }
        });
        
        // Keep running for a bit
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}