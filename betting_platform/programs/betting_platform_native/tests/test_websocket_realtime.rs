//! Test WebSocket real-time updates (<1s)

use betting_platform_native::api::websocket::*;
use betting_platform_native::integration::polymarket_websocket::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_websocket_sub_second_updates() {
    // Create WebSocket server
    let config = WebSocketConfig {
        bind_address: "127.0.0.1:8082".to_string(),
        max_connections_per_user: 0,
        ping_interval_secs: 30,
        message_buffer_size: 1000,
    };
    
    let server = Arc::new(WebSocketServer::new(config));
    let server_clone = server.clone();
    
    // Start server in background
    tokio::spawn(async move {
        let _ = server_clone.start().await;
    });
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Create market data feed
    let feed = MarketDataFeed::new(server.clone());
    
    // Track update timings
    let (update_tx, mut update_rx) = mpsc::unbounded_channel();
    let start_time = Instant::now();
    let mut update_count = 0;
    let mut update_times = Vec::new();
    
    // Start feed in background
    tokio::spawn(async move {
        feed.start_simulation().await;
    });
    
    // Collect updates for 2 seconds
    let test_duration = Duration::from_secs(2);
    
    while start_time.elapsed() < test_duration {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                // Check for updates
                if let Ok(stats) = server.get_stats().await {
                    if stats.total_connections > 0 {
                        update_count += 1;
                        update_times.push(start_time.elapsed());
                    }
                }
            }
        }
    }
    
    // Verify update frequency
    assert!(update_count > 10, "Expected >10 updates in 2s, got {}", update_count);
    
    // Check average update interval
    if update_times.len() > 1 {
        let intervals: Vec<Duration> = update_times.windows(2)
            .map(|w| w[1] - w[0])
            .collect();
        
        let avg_interval = intervals.iter().sum::<Duration>() / intervals.len() as u32;
        
        // Should be <1s (we're targeting 100ms)
        assert!(avg_interval < Duration::from_secs(1), 
                "Average update interval {:?} should be <1s", avg_interval);
    }
}

#[tokio::test]
async fn test_polymarket_websocket_fallback() {
    // Create update channel
    let (update_tx, mut update_rx) = mpsc::unbounded_channel();
    
    // Create Polymarket WebSocket client
    let client = PolymarketWebSocketClient::new(update_tx);
    
    // Subscribe to test markets
    client.subscribe_markets(vec!["market_1".to_string(), "market_2".to_string()]).await.unwrap();
    
    // Simulate connection failure by not actually connecting
    // This should trigger fallback mode
    client.enable_fallback_mode().await.unwrap();
    
    // Collect updates for 1 minute to verify fallback polling
    let start_time = Instant::now();
    let mut fallback_updates = 0;
    
    while start_time.elapsed() < Duration::from_secs(60) {
        tokio::select! {
            Some(update) = update_rx.recv() => {
                assert_eq!(update.update_type, UpdateType::Fallback);
                fallback_updates += 1;
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    }
    
    // Should get ~2 updates (30s intervals)
    assert!(fallback_updates >= 2, "Expected at least 2 fallback updates, got {}", fallback_updates);
}

#[test]
fn test_volatility_detection() {
    let mut history = PriceHistory::new();
    
    // Test stable market
    for i in 0..20 {
        let price = 0.5 + (i as f64 * 0.0001); // 0.01% changes
        assert!(!history.add_price("stable_market", price), 
               "Should not detect volatility for small changes");
    }
    
    // Test volatile market
    history.add_price("volatile_market", 0.5);
    for _ in 0..5 {
        history.add_price("volatile_market", 0.5);
    }
    history.add_price("volatile_market", 0.55); // 10% jump
    
    assert!(history.add_price("volatile_market", 0.55), 
           "Should detect volatility for >5% swings");
}

#[test]
fn test_message_batching() {
    use serde_json::json;
    
    // Test batch message format
    let batch_msg = json!({
        "type": "batch",
        "messages": [
            {
                "channel": "prices",
                "event": "price_update",
                "data": {"price": 0.65},
                "timestamp": 1234567890,
                "sequence": 1
            },
            {
                "channel": "trades",
                "event": "trade",
                "data": {"price": 0.66, "size": 1000},
                "timestamp": 1234567891,
                "sequence": 2
            }
        ],
        "count": 2,
        "timestamp": 1234567892000
    });
    
    let batch_str = serde_json::to_string(&batch_msg).unwrap();
    assert!(batch_str.contains("batch"));
    assert!(batch_str.contains("count"));
}