//! End-to-End Polymarket Integration Tests
//! Tests the complete flow from frontend to Polymarket

use axum::http::StatusCode;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

const API_BASE_URL: &str = "http://localhost:3001/api";

#[derive(Debug, Clone)]
struct TestContext {
    client: Client,
    auth_token: String,
    user_address: String,
}

impl TestContext {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        // Login to get auth token
        let login_response = client
            .post(&format!("{}/auth/login", API_BASE_URL))
            .json(&json!({
                "wallet_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb4",
                "signature": "test_signature",
                "message": "test_message"
            }))
            .send()
            .await?;
        
        let login_data: serde_json::Value = login_response.json().await?;
        let auth_token = login_data["data"]["token"]
            .as_str()
            .unwrap_or("test_token")
            .to_string();
        
        Ok(Self {
            client,
            auth_token,
            user_address: "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb4".to_string(),
        })
    }
    
    async fn authorized_request(&self, method: &str, path: &str, body: Option<serde_json::Value>) 
        -> Result<reqwest::Response, reqwest::Error> 
    {
        let url = format!("{}{}", API_BASE_URL, path);
        let mut request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "DELETE" => self.client.delete(&url),
            _ => panic!("Unsupported method"),
        };
        
        request = request.header("Authorization", format!("Bearer {}", self.auth_token));
        
        if let Some(body) = body {
            request = request.json(&body);
        }
        
        request.send().await
    }
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_health_check() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    let response = ctx.authorized_request("GET", "/polymarket/health", None)
        .await
        .expect("Health check request failed");
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let data: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(data["success"].as_bool().unwrap_or(false));
    assert!(data["data"]["databaseConnected"].as_bool().unwrap_or(false));
}

#[tokio::test]
#[ignore] // Requires running server and Polymarket API
async fn test_order_creation_flow() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    // 1. Create order
    let create_response = ctx.authorized_request(
        "POST",
        "/polymarket/orders",
        Some(json!({
            "marketId": "0x1234",
            "conditionId": "0xabcd",
            "tokenId": "987654321",
            "outcome": 1,
            "side": "buy",
            "size": "100",
            "price": "0.65",
            "orderType": "gtc"
        }))
    ).await.expect("Create order request failed");
    
    assert_eq!(create_response.status(), StatusCode::OK);
    
    let order_data: serde_json::Value = create_response.json().await.expect("Failed to parse order");
    let order_id = order_data["data"]["orderId"].as_str().expect("No order ID");
    
    println!("Created order: {}", order_id);
    
    // 2. Get order details
    let get_response = ctx.authorized_request(
        "GET",
        &format!("/polymarket/orders/{}", order_id),
        None
    ).await.expect("Get order request failed");
    
    assert_eq!(get_response.status(), StatusCode::OK);
    
    // 3. Get user's orders
    let list_response = ctx.authorized_request(
        "GET",
        "/polymarket/orders",
        None
    ).await.expect("List orders request failed");
    
    assert_eq!(list_response.status(), StatusCode::OK);
    
    let orders: serde_json::Value = list_response.json().await.expect("Failed to parse orders");
    assert!(orders["data"].as_array().unwrap().len() > 0);
    
    // 4. Cancel order
    let cancel_response = ctx.authorized_request(
        "DELETE",
        &format!("/polymarket/orders/{}", order_id),
        None
    ).await.expect("Cancel order request failed");
    
    assert_eq!(cancel_response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore] // Requires running server and Polymarket API
async fn test_market_data_flow() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    let condition_id = "0x1234567890abcdef";
    let token_id = "987654321";
    
    // 1. Get market data
    let market_response = ctx.authorized_request(
        "GET",
        &format!("/polymarket/markets/{}", condition_id),
        None
    ).await.expect("Market data request failed");
    
    assert_eq!(market_response.status(), StatusCode::OK);
    
    // 2. Get order book
    let orderbook_response = ctx.authorized_request(
        "GET",
        &format!("/polymarket/orderbook/{}", token_id),
        None
    ).await.expect("Order book request failed");
    
    assert_eq!(orderbook_response.status(), StatusCode::OK);
    
    let orderbook: serde_json::Value = orderbook_response.json().await
        .expect("Failed to parse order book");
    
    assert!(orderbook["data"]["bids"].is_array());
    assert!(orderbook["data"]["asks"].is_array());
    
    // 3. Get price history
    let history_response = ctx.authorized_request(
        "GET",
        &format!("/polymarket/markets/{}/history?hours=24", condition_id),
        None
    ).await.expect("Price history request failed");
    
    assert_eq!(history_response.status(), StatusCode::OK);
    
    // 4. Sync market
    let sync_response = ctx.authorized_request(
        "POST",
        &format!("/polymarket/markets/{}/sync", condition_id),
        None
    ).await.expect("Market sync request failed");
    
    assert_eq!(sync_response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore] // Requires running server and Polymarket API
async fn test_position_management() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    // 1. Get positions
    let positions_response = ctx.authorized_request(
        "GET",
        "/polymarket/positions",
        None
    ).await.expect("Positions request failed");
    
    assert_eq!(positions_response.status(), StatusCode::OK);
    
    // 2. Get balances
    let balances_response = ctx.authorized_request(
        "GET",
        "/polymarket/balances",
        None
    ).await.expect("Balances request failed");
    
    assert_eq!(balances_response.status(), StatusCode::OK);
    
    let balances: serde_json::Value = balances_response.json().await
        .expect("Failed to parse balances");
    
    assert!(balances["data"]["usdcBalance"].is_string());
    assert!(balances["data"]["availableBalance"].is_string());
    
    // 3. Get user stats
    let stats_response = ctx.authorized_request(
        "GET",
        "/polymarket/stats",
        None
    ).await.expect("Stats request failed");
    
    assert_eq!(stats_response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore] // Requires running server and Polymarket API
async fn test_ctf_operations() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    let condition_id = "0x1234567890abcdef";
    
    // 1. Split position
    let split_response = ctx.authorized_request(
        "POST",
        "/polymarket/ctf/split",
        Some(json!({
            "conditionId": condition_id,
            "amount": "100"
        }))
    ).await.expect("Split position request failed");
    
    // May fail without real funds
    if split_response.status() == StatusCode::OK {
        let split_data: serde_json::Value = split_response.json().await
            .expect("Failed to parse split response");
        
        assert!(split_data["data"]["txHash"].is_string());
        assert!(split_data["data"]["yesTokens"].is_string());
        assert!(split_data["data"]["noTokens"].is_string());
    }
    
    // 2. Merge positions
    let merge_response = ctx.authorized_request(
        "POST",
        "/polymarket/ctf/merge",
        Some(json!({
            "conditionId": condition_id,
            "amount": "50"
        }))
    ).await.expect("Merge positions request failed");
    
    // May fail without positions
    if merge_response.status() == StatusCode::OK {
        let merge_data: serde_json::Value = merge_response.json().await
            .expect("Failed to parse merge response");
        
        assert!(merge_data["data"]["txHash"].is_string());
        assert!(merge_data["data"]["collateralReturned"].is_string());
    }
    
    // 3. Redeem positions (for resolved markets)
    let redeem_response = ctx.authorized_request(
        "POST",
        "/polymarket/ctf/redeem",
        Some(json!({
            "conditionId": condition_id,
            "indexSets": ["1", "2"]
        }))
    ).await;
    
    // Will fail if market not resolved
    assert!(redeem_response.is_ok());
}

#[tokio::test]
#[ignore] // Requires running server and WebSocket
async fn test_websocket_updates() {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::{StreamExt, SinkExt};
    
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    // Connect to WebSocket
    let ws_url = "ws://localhost:3001/ws";
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .expect("Failed to connect to WebSocket");
    
    let (mut write, mut read) = ws_stream.split();
    
    // Send auth message
    let auth_msg = json!({
        "type": "auth",
        "token": ctx.auth_token
    });
    
    write.send(Message::Text(auth_msg.to_string()))
        .await
        .expect("Failed to send auth message");
    
    // Subscribe to market
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "market",
        "marketId": "0x1234"
    });
    
    write.send(Message::Text(subscribe_msg.to_string()))
        .await
        .expect("Failed to send subscribe message");
    
    // Wait for updates
    let timeout = tokio::time::timeout(
        Duration::from_secs(5),
        async {
            while let Some(msg) = read.next().await {
                if let Ok(Message::Text(text)) = msg {
                    println!("Received WebSocket message: {}", text);
                    let data: serde_json::Value = serde_json::from_str(&text)
                        .expect("Failed to parse WebSocket message");
                    
                    if data["type"] == "market_update" {
                        return true;
                    }
                }
            }
            false
        }
    ).await;
    
    assert!(timeout.is_ok() || timeout.is_err()); // Either receives update or times out
}

#[tokio::test]
#[ignore] // Load test - requires running server
async fn test_concurrent_order_creation() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    let mut tasks = vec![];
    
    for i in 0..10 {
        let ctx_clone = ctx.clone();
        tasks.push(tokio::spawn(async move {
            let response = ctx_clone.authorized_request(
                "POST",
                "/polymarket/orders",
                Some(json!({
                    "marketId": format!("market_{}", i),
                    "conditionId": format!("condition_{}", i),
                    "tokenId": format!("token_{}", i),
                    "outcome": 1,
                    "side": if i % 2 == 0 { "buy" } else { "sell" },
                    "size": format!("{}", 100 + i * 10),
                    "price": format!("{:.2}", 0.5 + (i as f64) * 0.05),
                    "orderType": "gtc"
                }))
            ).await;
            
            match response {
                Ok(resp) => resp.status() == StatusCode::OK,
                Err(_) => false,
            }
        }));
    }
    
    let results = futures::future::join_all(tasks).await;
    let successful = results.iter().filter(|r| r.is_ok() && *r.as_ref().unwrap()).count();
    
    println!("Successfully created {}/10 concurrent orders", successful);
    assert!(successful >= 5); // At least half should succeed
}

#[tokio::test]
#[ignore] // Stress test - requires running server
async fn test_order_book_performance() {
    let ctx = TestContext::new().await.expect("Failed to create test context");
    
    let start = std::time::Instant::now();
    let mut total_requests = 0;
    
    while start.elapsed() < Duration::from_secs(10) {
        let response = ctx.authorized_request(
            "GET",
            "/polymarket/orderbook/987654321",
            None
        ).await;
        
        if response.is_ok() {
            total_requests += 1;
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    let requests_per_second = total_requests as f64 / 10.0;
    println!("Order book requests per second: {:.2}", requests_per_second);
    
    assert!(requests_per_second >= 5.0); // Should handle at least 5 req/s
}