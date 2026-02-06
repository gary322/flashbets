//! WebSocket handler for real-time updates

pub mod enhanced;
pub mod real_events;

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::broadcast;
use tracing::{info, debug, error};

use crate::{AppState, types::WsMessage};

pub struct WebSocketManager {
    tx: broadcast::Sender<WsMessage>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        // Increased channel size to prevent message drops under high load
        let (tx, _) = broadcast::channel(1000);
        Self { tx }
    }

    pub fn broadcast(&self, msg: WsMessage) {
        let _ = self.tx.send(msg);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }
}

pub async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("New WebSocket connection established");
    
    let mut rx = state.ws_manager.subscribe();
    
    // Send initial connection message
    let welcome_msg = WsMessage::Notification {
        title: "Connected".to_string(),
        message: "Connected to Quantum Betting Platform".to_string(),
        level: "info".to_string(),
    };
    
    if let Ok(msg) = serde_json::to_string(&welcome_msg) {
        let _ = socket.send(Message::Text(msg)).await;
    }
    
    // Handle bidirectional communication
    loop {
        tokio::select! {
            // Receive messages from client
            Some(msg) = socket.recv() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received WebSocket message: {}", text);
                        // Handle client messages if needed
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket connection closed by client");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            
            // Send broadcast messages to client
            Ok(msg) = rx.recv() => {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if socket.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    
    info!("WebSocket connection closed");
}

/// Start market update loop
pub async fn start_market_updates(state: AppState) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
    
    loop {
        interval.tick().await;
        
        // Get real market data
        if let Ok(markets) = state.platform_client.get_markets().await {
            for market in markets.iter().take(5) { // Update top 5 markets
                // Calculate real prices based on stake distribution
                let total_stake: u64 = market.outcomes.iter().map(|o| o.total_stake).sum();
                let yes_price = if total_stake > 0 && market.outcomes.len() >= 2 {
                    market.outcomes[0].total_stake as f64 / total_stake as f64
                } else {
                    0.5
                };
                let no_price = 1.0 - yes_price;
                
                let update = WsMessage::MarketUpdate {
                    market_id: crate::serialization::SafeU128(market.id),
                    yes_price,
                    no_price,
                    volume: market.total_volume,
                };
        
                state.ws_manager.broadcast(update);
                debug!("Broadcast market update for market {}", market.id);
            }
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    
    #[tokio::test]
    async fn test_websocket_manager_creation() {
        let manager = WebSocketManager::new();
        // Manager should be created successfully
        let _ = manager.subscribe(); // Test that we can create a receiver
    }
    
    #[tokio::test]
    async fn test_broadcast_subscribe() {
        let manager = WebSocketManager::new();
        let mut rx = manager.subscribe();
        
        let msg = WsMessage::Notification {
            title: "Test".to_string(),
            message: "Test message".to_string(),
            level: "info".to_string(),
        };
        
        manager.broadcast(msg.clone());
        
        // Should receive the broadcast message
        match rx.recv().await {
            Ok(received) => match (received, msg) {
                (WsMessage::Notification { title: t1, message: m1, level: l1 },
                 WsMessage::Notification { title: t2, message: m2, level: l2 }) => {
                    assert_eq!(t1, t2);
                    assert_eq!(m1, m2);
                    assert_eq!(l1, l2);
                }
                _ => panic!("Message types don't match"),
            },
            Err(_) => panic!("Failed to receive message"),
        }
    }
    
    #[tokio::test]
    async fn test_multiple_subscribers() {
        let manager = WebSocketManager::new();
        let mut rx1 = manager.subscribe();
        let mut rx2 = manager.subscribe();
        let mut rx3 = manager.subscribe();
        
        let msg = WsMessage::MarketUpdate {
            market_id: crate::serialization::SafeU128(1000),
            yes_price: 0.6,
            no_price: 0.4,
            volume: 100000,
        };
        
        manager.broadcast(msg.clone());
        
        // All subscribers should receive the message
        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
        assert!(rx3.recv().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_broadcast_different_message_types() {
        let manager = WebSocketManager::new();
        let mut rx = manager.subscribe();
        
        // Test notification
        let notification = WsMessage::Notification {
            title: "Alert".to_string(),
            message: "Something happened".to_string(),
            level: "warning".to_string(),
        };
        manager.broadcast(notification);
        assert!(rx.recv().await.is_ok());
        
        // Test market update
        let market_update = WsMessage::MarketUpdate {
            market_id: crate::serialization::SafeU128(12345),
            yes_price: 0.75,
            no_price: 0.25,
            volume: 50000,
        };
        manager.broadcast(market_update);
        assert!(rx.recv().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_dropped_receiver() {
        let manager = WebSocketManager::new();
        
        // Create and immediately drop a receiver
        {
            let _rx = manager.subscribe();
        } // rx dropped here
        
        // Broadcasting should still work for other receivers
        let mut rx2 = manager.subscribe();
        
        let msg = WsMessage::Notification {
            title: "Still works".to_string(),
            message: "Even with dropped receivers".to_string(),
            level: "info".to_string(),
        };
        
        manager.broadcast(msg);
        assert!(rx2.recv().await.is_ok());
    }
    
    #[test]
    fn test_websocket_manager_default() {
        let manager = WebSocketManager::new();
        let _rx = manager.subscribe();
        // Should be able to create manager and subscribe
    }
    
    #[test]
    fn test_ws_message_serialization() {
        let messages = vec![
            WsMessage::Notification {
                title: "Trade Executed".to_string(),
                message: "Your trade has been executed".to_string(),
                level: "success".to_string(),
            },
            WsMessage::MarketUpdate {
                market_id: crate::serialization::SafeU128(1234),
                yes_price: 0.55,
                no_price: 0.45,
                volume: 1000000,
            },
        ];
        
        // Test serialization/deserialization
        for msg in messages {
            let serialized = serde_json::to_string(&msg).unwrap();
            let deserialized: WsMessage = serde_json::from_str(&serialized).unwrap();
            
            // Compare by serializing again (since we can't directly compare)
            let reserialized = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(serialized, reserialized);
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_broadcasts() {
        use std::sync::Arc;
        let manager = Arc::new(WebSocketManager::new());
        let mut handles = vec![];
        
        // Create a receiver
        let mut rx = manager.subscribe();
        
        // Spawn multiple tasks broadcasting messages
        for i in 0..10 {
            let mgr = manager.clone();
            let handle = tokio::spawn(async move {
                let msg = WsMessage::Notification {
                    title: format!("Message {}", i),
                    message: "Concurrent test".to_string(),
                    level: "info".to_string(),
                };
                mgr.broadcast(msg);
            });
            handles.push(handle);
        }
        
        // Wait for all broadcasts
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Should receive multiple messages
        let mut count = 0;
        while let Ok(_) = rx.try_recv() {
            count += 1;
        }
        assert!(count > 0);
    }
    
    #[tokio::test]
    async fn test_message_ordering() {
        let manager = WebSocketManager::new();
        let mut rx = manager.subscribe();
        
        // Send messages in order
        for i in 0..5 {
            let msg = WsMessage::Notification {
                title: format!("Message {}", i),
                message: "Order test".to_string(),
                level: "info".to_string(),
            };
            manager.broadcast(msg);
        }
        
        // Verify messages are received in order
        for i in 0..5 {
            match rx.recv().await {
                Ok(WsMessage::Notification { title, .. }) => {
                    assert_eq!(title, format!("Message {}", i));
                }
                _ => panic!("Unexpected message type or error"),
            }
        }
    }
}
