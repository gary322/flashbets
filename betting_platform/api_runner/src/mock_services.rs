//! Mock services for testing
//! Provides production-grade mock implementations of external services

use anyhow::{Result, Context as AnyhowContext};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

use crate::{
    settlement_service::OracleProvider,
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Mock Oracle Provider for testing settlement
pub struct MockOracleProvider {
    name: String,
    outcomes: Arc<RwLock<HashMap<u128, u8>>>, // market_id -> outcome
    confidence: f64,
    response_delay: Option<Duration>,
    fail_rate: f64,
}

impl MockOracleProvider {
    pub fn new(name: String) -> Self {
        Self {
            name,
            outcomes: Arc::new(RwLock::new(HashMap::new())),
            confidence: 0.95,
            response_delay: None,
            fail_rate: 0.0,
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.response_delay = Some(delay);
        self
    }

    pub fn with_fail_rate(mut self, rate: f64) -> Self {
        self.fail_rate = rate;
        self
    }

    pub async fn set_market_outcome(&self, market_id: u128, outcome: u8) {
        self.outcomes.write().await.insert(market_id, outcome);
    }

    pub async fn set_bulk_outcomes(&self, outcomes: Vec<(u128, u8)>) {
        let mut guard = self.outcomes.write().await;
        for (market_id, outcome) in outcomes {
            guard.insert(market_id, outcome);
        }
    }
}

#[async_trait]
impl OracleProvider for MockOracleProvider {
    async fn get_resolution(&self, market: &crate::settlement_service::Market) -> Result<crate::settlement_service::OracleResult> {
        // Simulate delay if configured
        if let Some(delay) = self.response_delay {
            tokio::time::sleep(delay).await;
        }

        // Simulate failure if configured
        if self.fail_rate > 0.0 {
            let should_fail = rand::random::<f64>() < self.fail_rate;
            if should_fail {
                return Err(anyhow::anyhow!("Oracle request failed (simulated)"));
            }
        }

        let outcomes = self.outcomes.read().await;
        let outcome = outcomes.get(&market.id).copied().unwrap_or(0);

        Ok(crate::settlement_service::OracleResult {
            oracle_name: self.name.clone(),
            outcome,
            confidence: self.confidence,
            timestamp: Utc::now(),
            proof_url: Some(format!("https://mock-oracle.com/proof/{}", market.id)),
            raw_data: Some(serde_json::json!({
                "market_id": market.id,
                "outcome": outcome,
                "source": "mock",
            })),
        })
    }

    async fn verify_resolution(&self, market_id: u128, outcome: u8) -> Result<bool> {
        let outcomes = self.outcomes.read().await;
        Ok(outcomes.get(&market_id).copied() == Some(outcome))
    }
}

/// Mock Solana RPC Client for testing
pub struct MockSolanaRpcClient {
    accounts: Arc<RwLock<HashMap<Pubkey, MockAccount>>>,
    recent_blockhash: Arc<RwLock<solana_sdk::hash::Hash>>,
    signatures: Arc<RwLock<Vec<Signature>>>,
    fail_next_call: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
struct MockAccount {
    lamports: u64,
    data: Vec<u8>,
    owner: Pubkey,
}

impl MockSolanaRpcClient {
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            recent_blockhash: Arc::new(RwLock::new(solana_sdk::hash::Hash::default())),
            signatures: Arc::new(RwLock::new(Vec::new())),
            fail_next_call: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn set_account(&self, pubkey: Pubkey, lamports: u64, data: Vec<u8>, owner: Pubkey) {
        self.accounts.write().await.insert(pubkey, MockAccount {
            lamports,
            data,
            owner,
        });
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        if *self.fail_next_call.read().await {
            *self.fail_next_call.write().await = false;
            return Err(anyhow::anyhow!("RPC call failed"));
        }

        let accounts = self.accounts.read().await;
        Ok(accounts.get(pubkey).map(|a| a.lamports).unwrap_or(0))
    }

    pub async fn get_recent_blockhash(&self) -> Result<(solana_sdk::hash::Hash, u64)> {
        let hash = *self.recent_blockhash.read().await;
        Ok((hash, 100)) // Mock slot
    }

    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        if *self.fail_next_call.read().await {
            *self.fail_next_call.write().await = false;
            return Err(anyhow::anyhow!("Transaction send failed"));
        }

        let sig = Signature::new_unique();
        self.signatures.write().await.push(sig);
        Ok(sig)
    }

    pub async fn set_fail_next(&self) {
        *self.fail_next_call.write().await = true;
    }
}

/// Mock Trading Engine for testing
pub struct MockTradingEngine {
    markets: Arc<RwLock<HashMap<u128, MockMarket>>>,
    orders: Arc<RwLock<Vec<MockOrder>>>,
    positions: Arc<RwLock<HashMap<String, Vec<MockPosition>>>>,
    fail_next_call: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
struct MockMarket {
    id: u128,
    title: String,
    liquidity: u64,
    volume: u64,
    yes_price: f64,
    no_price: f64,
}

#[derive(Debug, Clone)]
struct MockOrder {
    id: String,
    market_id: u128,
    user: String,
    side: OrderSide,
    amount: u64,
    price: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
struct MockPosition {
    id: String,
    market_id: u128,
    outcome: u8,
    amount: u64,
    entry_price: f64,
}

impl MockTradingEngine {
    pub fn new() -> Self {
        Self {
            markets: Arc::new(RwLock::new(HashMap::new())),
            orders: Arc::new(RwLock::new(Vec::new())),
            positions: Arc::new(RwLock::new(HashMap::new())),
            fail_next_call: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn add_market(&self, id: u128, title: String, liquidity: u64) {
        self.markets.write().await.insert(id, MockMarket {
            id,
            title,
            liquidity,
            volume: 0,
            yes_price: 0.5,
            no_price: 0.5,
        });
    }

    pub async fn place_order(
        &self,
        market_id: u128,
        user: String,
        amount: u64,
        buy: bool,
    ) -> Result<String> {
        if *self.fail_next_call.read().await {
            *self.fail_next_call.write().await = false;
            return Err(anyhow::anyhow!("Order placement failed"));
        }

        let order_id = format!("order_{}", uuid::Uuid::new_v4());
        
        let order = MockOrder {
            id: order_id.clone(),
            market_id,
            user: user.clone(),
            side: if buy { OrderSide::Buy } else { OrderSide::Sell },
            amount,
            price: 0.5, // Simplified
            timestamp: Utc::now(),
        };

        self.orders.write().await.push(order);

        // Create position
        let position = MockPosition {
            id: format!("pos_{}", uuid::Uuid::new_v4()),
            market_id,
            outcome: 0, // Simplified to Yes
            amount,
            entry_price: 0.5,
        };

        self.positions
            .write()
            .await
            .entry(user)
            .or_insert_with(Vec::new)
            .push(position);

        // Update market volume
        if let Some(market) = self.markets.write().await.get_mut(&market_id) {
            market.volume += amount;
        }

        Ok(order_id)
    }

    pub async fn get_market_data(&self, market_id: u128) -> Result<(f64, f64, u64)> {
        let markets = self.markets.read().await;
        let market = markets.get(&market_id)
            .ok_or_else(|| anyhow::anyhow!("Market not found"))?;
        
        Ok((market.yes_price, market.no_price, market.volume))
    }

    pub async fn get_user_positions(&self, user: &str) -> Vec<MockPosition> {
        self.positions
            .read()
            .await
            .get(user)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn set_fail_next(&self) {
        *self.fail_next_call.write().await = true;
    }
}

/// Mock WebSocket Manager for testing
pub struct MockWebSocketManager {
    connections: Arc<RwLock<HashMap<String, MockWsConnection>>>,
    broadcast_messages: Arc<RwLock<Vec<serde_json::Value>>>,
}

#[derive(Debug, Clone)]
struct MockWsConnection {
    id: String,
    user: Option<String>,
    subscriptions: Vec<String>,
    connected_at: DateTime<Utc>,
}

impl MockWebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            broadcast_messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn add_connection(&self, id: String, user: Option<String>) -> String {
        let connection = MockWsConnection {
            id: id.clone(),
            user,
            subscriptions: Vec::new(),
            connected_at: Utc::now(),
        };

        self.connections.write().await.insert(id.clone(), connection);
        id
    }

    pub async fn remove_connection(&self, id: &str) {
        self.connections.write().await.remove(id);
    }

    pub async fn subscribe(&self, connection_id: &str, topic: String) -> Result<()> {
        let mut connections = self.connections.write().await;
        let connection = connections.get_mut(connection_id)
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))?;
        
        connection.subscriptions.push(topic);
        Ok(())
    }

    pub async fn broadcast(&self, message: serde_json::Value) {
        self.broadcast_messages.write().await.push(message.clone());
        info!("Mock broadcast: {:?}", message);
    }

    pub async fn get_broadcast_history(&self) -> Vec<serde_json::Value> {
        self.broadcast_messages.read().await.clone()
    }

    pub async fn get_connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}

/// Mock External API Client for testing
pub struct MockExternalApiClient {
    responses: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    request_log: Arc<RwLock<Vec<(String, String)>>>, // (method, url)
    fail_pattern: Arc<RwLock<Option<String>>>,
}

impl MockExternalApiClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(HashMap::new())),
            request_log: Arc::new(RwLock::new(Vec::new())),
            fail_pattern: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_response(&self, endpoint: String, response: serde_json::Value) {
        self.responses.write().await.insert(endpoint, response);
    }

    pub async fn set_fail_pattern(&self, pattern: Option<String>) {
        *self.fail_pattern.write().await = pattern;
    }

    pub async fn get(&self, url: &str) -> Result<serde_json::Value> {
        self.request_log.write().await.push(("GET".to_string(), url.to_string()));

        // Check if should fail
        if let Some(pattern) = &*self.fail_pattern.read().await {
            if url.contains(pattern) {
                return Err(anyhow::anyhow!("Request failed (pattern match)"));
            }
        }

        // Return mock response
        let responses = self.responses.read().await;
        for (endpoint, response) in responses.iter() {
            if url.contains(endpoint) {
                return Ok(response.clone());
            }
        }

        // Default response
        Ok(serde_json::json!({
            "status": "ok",
            "data": {},
            "timestamp": Utc::now(),
        }))
    }

    pub async fn post(&self, url: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        self.request_log.write().await.push(("POST".to_string(), url.to_string()));

        // Check if should fail
        if let Some(pattern) = &*self.fail_pattern.read().await {
            if url.contains(pattern) {
                return Err(anyhow::anyhow!("Request failed (pattern match)"));
            }
        }

        // Return mock response based on body
        Ok(serde_json::json!({
            "status": "created",
            "id": uuid::Uuid::new_v4().to_string(),
            "request": body,
            "timestamp": Utc::now(),
        }))
    }

    pub async fn get_request_log(&self) -> Vec<(String, String)> {
        self.request_log.read().await.clone()
    }
}

/// Mock Price Feed for testing
pub struct MockPriceFeed {
    prices: Arc<RwLock<HashMap<String, f64>>>,
    price_history: Arc<RwLock<HashMap<String, Vec<(DateTime<Utc>, f64)>>>>,
    update_interval: Option<Duration>,
}

impl MockPriceFeed {
    pub fn new() -> Self {
        Self {
            prices: Arc::new(RwLock::new(HashMap::new())),
            price_history: Arc::new(RwLock::new(HashMap::new())),
            update_interval: None,
        }
    }

    pub async fn set_price(&self, symbol: String, price: f64) {
        self.prices.write().await.insert(symbol.clone(), price);
        
        // Add to history
        self.price_history
            .write()
            .await
            .entry(symbol)
            .or_insert_with(Vec::new)
            .push((Utc::now(), price));
    }

    pub async fn get_price(&self, symbol: &str) -> Result<f64> {
        self.prices
            .read()
            .await
            .get(symbol)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Price not found for symbol: {}", symbol))
    }

    pub async fn get_price_history(&self, symbol: &str, limit: usize) -> Vec<(DateTime<Utc>, f64)> {
        self.price_history
            .read()
            .await
            .get(symbol)
            .map(|history| {
                history
                    .iter()
                    .rev()
                    .take(limit)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn start_price_updates(self: Arc<Self>, symbols: Vec<String>) -> tokio::task::JoinHandle<()> {
        let interval = self.update_interval.unwrap_or(Duration::from_secs(5));
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            
            loop {
                interval.tick().await;
                
                for symbol in &symbols {
                    let current_price = self.prices.read().await.get(symbol).copied().unwrap_or(100.0);
                    
                    // Simulate price movement (±2%)
                    let change = (rand::random::<f64>() - 0.5) * 0.04;
                    let new_price = current_price * (1.0 + change);
                    
                    self.set_price(symbol.clone(), new_price).await;
                    debug!("Updated {} price to {:.2}", symbol, new_price);
                }
            }
        })
    }
}

/// Mock service factory for easy setup
pub struct MockServiceFactory;

impl MockServiceFactory {
    /// Create a complete set of mock services
    pub fn create_all() -> MockServices {
        MockServices {
            oracle_provider: Arc::new(MockOracleProvider::new("TestOracle".to_string())),
            solana_rpc: Arc::new(MockSolanaRpcClient::new()),
            trading_engine: Arc::new(MockTradingEngine::new()),
            websocket_manager: Arc::new(MockWebSocketManager::new()),
            external_api: Arc::new(MockExternalApiClient::new()),
            price_feed: Arc::new(MockPriceFeed::new()),
        }
    }

    /// Create mock services with realistic test data
    pub async fn create_with_test_data() -> MockServices {
        let services = Self::create_all();

        // Set up some test markets
        for i in 0..5 {
            services.trading_engine.add_market(
                1000 + i as u128,
                format!("Test Market {}", i),
                100000 * (i as u64 + 1),
            ).await;
        }

        // Set up some price data
        services.price_feed.set_price("BTC".to_string(), 45000.0).await;
        services.price_feed.set_price("ETH".to_string(), 2800.0).await;
        services.price_feed.set_price("SOL".to_string(), 95.0).await;

        // Set up some oracle outcomes
        services.oracle_provider.set_bulk_outcomes(vec![
            (1000, 0),
            (1001, 1),
            (1002, 0),
        ]).await;

        services
    }
}

/// Container for all mock services
pub struct MockServices {
    pub oracle_provider: Arc<MockOracleProvider>,
    pub solana_rpc: Arc<MockSolanaRpcClient>,
    pub trading_engine: Arc<MockTradingEngine>,
    pub websocket_manager: Arc<MockWebSocketManager>,
    pub external_api: Arc<MockExternalApiClient>,
    pub price_feed: Arc<MockPriceFeed>,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_oracle_provider() {
        let oracle = MockOracleProvider::new("TestOracle".to_string());
        oracle.set_market_outcome(1000, 1).await;

        let market = crate::settlement_service::Market {
            id: 1000,
            pubkey: Pubkey::new_unique(),
            creator: "test".to_string(),
            title: "Test Market".to_string(),
            description: "Test".to_string(),
            category: "test".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            total_liquidity: 100000,
            total_volume: 50000,
            status: "open".to_string(),
            end_time: None,
            resolution_time: None,
            created_at: Utc::now(),
            current_price: 0.5,
        };

        let result = oracle.get_resolution(&market).await.unwrap();
        assert_eq!(result.outcome, 1);
        assert_eq!(result.oracle_name, "TestOracle");
    }

    #[tokio::test]
    async fn test_mock_trading_engine() {
        let engine = MockTradingEngine::new();
        engine.add_market(1000, "Test Market".to_string(), 100000).await;

        let order_id = engine.place_order(1000, "user1".to_string(), 1000, true).await.unwrap();
        assert!(!order_id.is_empty());

        let (yes_price, no_price, volume) = engine.get_market_data(1000).await.unwrap();
        assert_eq!(yes_price, 0.5);
        assert_eq!(no_price, 0.5);
        assert_eq!(volume, 1000);

        let positions = engine.get_user_positions("user1").await;
        assert_eq!(positions.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_services_factory() {
        let services = MockServiceFactory::create_with_test_data().await;
        
        let btc_price = services.price_feed.get_price("BTC").await.unwrap();
        assert_eq!(btc_price, 45000.0);

        let connection_id = services.websocket_manager.add_connection(
            "test_conn".to_string(),
            Some("user1".to_string()),
        ).await;
        assert_eq!(connection_id, "test_conn");
    }
}