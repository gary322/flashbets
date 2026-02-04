//! Polymarket CLOB (Central Limit Order Book) Client
//! Complete implementation for interacting with Polymarket's CLOB API

use anyhow::{Result, anyhow, Context};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};

use super::polymarket_auth::{PolymarketAuthenticator, PolymarketAuthConfig, PolymarketOrderData};

const CLOB_API_BASE: &str = "https://clob.polymarket.com";
const CLOB_API_BASE_TESTNET: &str = "https://clob.polymarket.com"; // Update for testnet if different

/// Polymarket CLOB client
pub struct PolymarketClobClient {
    client: Client,
    auth: Arc<PolymarketAuthenticator>,
    base_url: String,
    cache: Arc<RwLock<OrderCache>>,
}

impl PolymarketClobClient {
    /// Create new CLOB client
    pub fn new(auth_config: PolymarketAuthConfig, testnet: bool) -> Result<Self> {
        let env_base_url = std::env::var("POLYMARKET_CLOB_BASE_URL").ok();
        let base_url = env_base_url.unwrap_or_else(|| {
            if testnet {
                CLOB_API_BASE_TESTNET.to_string()
            } else {
                CLOB_API_BASE.to_string()
            }
        });

        Self::build(auth_config, base_url)
    }

    /// Create new CLOB client with explicit base URL override (useful for tests/mocks).
    pub fn new_with_base_url(auth_config: PolymarketAuthConfig, base_url: String) -> Result<Self> {
        Self::build(auth_config, base_url)
    }

    fn build(auth_config: PolymarketAuthConfig, base_url: String) -> Result<Self> {
        let auth = Arc::new(PolymarketAuthenticator::new(auth_config)?);
        let base_url = normalize_base_url(base_url);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            auth,
            base_url,
            cache: Arc::new(RwLock::new(OrderCache::new())),
        })
    }
    
    /// Submit a new order to Polymarket
    pub async fn submit_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        let path = "/orders";
        let body = serde_json::to_string(&order)?;
        
        let headers = self.auth
            .generate_headers("POST", path, Some(&body), false)
            .await?;
        
        let response = self.client
            .post(format!("{}{}", self.base_url, path))
            .headers(headers)
            .body(body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            return Err(anyhow!("Order submission failed: {} - {}", status, error_body));
        }
        
        let order_response: OrderResponse = response.json().await?;
        
        // Cache the order
        self.cache.write().await.add_order(order_response.clone());
        
        info!("Order submitted successfully: {}", order_response.order_id);
        Ok(order_response)
    }
    
    /// Get order status
    pub async fn get_order(&self, order_id: &str) -> Result<OrderResponse> {
        // Check cache first
        if let Some(order) = self.cache.read().await.get_order(order_id) {
            return Ok(order);
        }
        
        let path = format!("/orders/{}", order_id);
        
        let headers = self.auth
            .generate_headers("GET", &path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get order: {}", response.status()));
        }
        
        let order: OrderResponse = response.json().await?;
        
        // Update cache
        self.cache.write().await.add_order(order.clone());
        
        Ok(order)
    }
    
    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<CancelResponse> {
        let path = format!("/orders/{}", order_id);
        
        let headers = self.auth
            .generate_headers("DELETE", &path, None, false)
            .await?;
        
        let response = self.client
            .delete(format!("{}{}", self.base_url, path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            return Err(anyhow!("Order cancellation failed: {} - {}", status, error_body));
        }
        
        let cancel_response: CancelResponse = response.json().await?;
        
        // Update cache
        self.cache.write().await.remove_order(order_id);
        
        info!("Order cancelled: {}", order_id);
        Ok(cancel_response)
    }
    
    /// Get user's open orders
    pub async fn get_open_orders(&self, params: OrdersQuery) -> Result<Vec<OrderResponse>> {
        let path = "/orders";
        let query = serde_urlencoded::to_string(&params)?;
        let full_path = format!("{}?{}", path, query);
        
        let headers = self.auth
            .generate_headers("GET", &full_path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, full_path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get orders: {}", response.status()));
        }
        
        let orders: Vec<OrderResponse> = response.json().await?;
        
        // Update cache
        let mut cache = self.cache.write().await;
        for order in &orders {
            cache.add_order(order.clone());
        }
        
        Ok(orders)
    }
    
    /// Get order book for a market
    pub async fn get_order_book(&self, token_id: &str) -> Result<OrderBook> {
        let path = "/book";
        let query = format!("token_id={}", token_id);
        let full_path = format!("{}?{}", path, query);
        
        let headers = self.auth
            .generate_headers("GET", &full_path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, full_path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get order book: {}", response.status()));
        }
        
        let book: OrderBook = response.json().await?;
        Ok(book)
    }
    
    /// Get user's positions
    pub async fn get_positions(&self, address: &str) -> Result<Vec<Position>> {
        let path = "/positions";
        let query = format!("address={}", address);
        let full_path = format!("{}?{}", path, query);
        
        let headers = self.auth
            .generate_headers("GET", &full_path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, full_path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get positions: {}", response.status()));
        }
        
        let positions: Vec<Position> = response.json().await?;
        Ok(positions)
    }
    
    /// Get user's trade history
    pub async fn get_trades(&self, params: TradesQuery) -> Result<Vec<Trade>> {
        let path = "/trades";
        let query = serde_urlencoded::to_string(&params)?;
        let full_path = format!("{}?{}", path, query);
        
        let headers = self.auth
            .generate_headers("GET", &full_path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, full_path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get trades: {}", response.status()));
        }
        
        let trades: Vec<Trade> = response.json().await?;
        Ok(trades)
    }
    
    /// Get market information
    pub async fn get_market(&self, condition_id: &str) -> Result<Market> {
        let path = format!("/markets/{}", condition_id);
        
        let headers = self.auth
            .generate_headers("GET", &path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get market: {}", response.status()));
        }
        
        let market: Market = response.json().await?;
        Ok(market)
    }
    
    /// Get user's balances (USDC and CTF tokens)
    pub async fn get_balances(&self, address: &str) -> Result<Balances> {
        let path = "/balances";
        let query = format!("address={}", address);
        let full_path = format!("{}?{}", path, query);
        
        let headers = self.auth
            .generate_headers("GET", &full_path, None, false)
            .await?;
        
        let response = self.client
            .get(format!("{}{}", self.base_url, full_path))
            .headers(headers)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get balances: {}", response.status()));
        }
        
        let balances: Balances = response.json().await?;
        Ok(balances)
    }
    
    /// Calculate fees for an order
    pub fn calculate_fees(&self, order_amount: f64, fee_rate_bps: u16) -> OrderFees {
        let fee_rate = fee_rate_bps as f64 / 10000.0;
        let platform_fee = order_amount * fee_rate;
        let gas_estimate = 0.01; // Estimated gas in MATIC
        
        OrderFees {
            platform_fee,
            gas_estimate,
            total_fee: platform_fee + gas_estimate,
            fee_rate_bps,
        }
    }
}

fn normalize_base_url(base_url: String) -> String {
    base_url.trim_end_matches('/').to_string()
}

/// Order cache for reducing API calls
struct OrderCache {
    orders: HashMap<String, (OrderResponse, DateTime<Utc>)>,
    max_age_seconds: i64,
}

impl OrderCache {
    fn new() -> Self {
        Self {
            orders: HashMap::new(),
            max_age_seconds: 60, // Cache for 1 minute
        }
    }
    
    fn add_order(&mut self, order: OrderResponse) {
        self.orders.insert(
            order.order_id.clone(),
            (order, Utc::now())
        );
        self.cleanup();
    }
    
    fn get_order(&self, order_id: &str) -> Option<OrderResponse> {
        if let Some((order, timestamp)) = self.orders.get(order_id) {
            if Utc::now().signed_duration_since(*timestamp).num_seconds() < self.max_age_seconds {
                return Some(order.clone());
            }
        }
        None
    }
    
    fn remove_order(&mut self, order_id: &str) {
        self.orders.remove(order_id);
    }
    
    fn cleanup(&mut self) {
        let now = Utc::now();
        self.orders.retain(|_, (_, timestamp)| {
            now.signed_duration_since(*timestamp).num_seconds() < self.max_age_seconds
        });
    }
}

// Request/Response Types

/// Order request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub order: PolymarketOrderData,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Order response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub order_hash: String,
    pub status: OrderStatus,
    pub market_id: String,
    pub outcome: String,
    pub side: OrderSide,
    pub size: String,
    pub price: f64,
    pub filled_amount: String,
    pub remaining_amount: String,
    pub average_fill_price: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Order status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Failed,
}

/// Order side
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Cancel response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelResponse {
    pub order_id: String,
    pub status: String,
    pub cancelled_at: DateTime<Utc>,
}

/// Orders query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersQuery {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub market_id: String,
    pub bids: Vec<OrderBookEntry>,
    pub asks: Vec<OrderBookEntry>,
    pub timestamp: DateTime<Utc>,
}

/// Order book entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookEntry {
    pub price: f64,
    pub size: f64,
    pub num_orders: u32,
}

/// Position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: String,
    pub outcome: String,
    pub shares: String,
    pub average_price: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub trade_id: String,
    pub order_id: String,
    pub market_id: String,
    pub outcome: String,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub fee: f64,
    pub executed_at: DateTime<Utc>,
}

/// Trades query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradesQuery {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub condition_id: String,
    pub question: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub end_date: DateTime<Utc>,
    pub volume: f64,
    pub liquidity: f64,
    pub resolved: bool,
    pub winning_outcome: Option<String>,
}

/// User balances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balances {
    pub usdc_balance: String,
    pub matic_balance: String,
    pub ctf_balances: Vec<CTFBalance>,
}

/// CTF token balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CTFBalance {
    pub token_id: String,
    pub market_id: String,
    pub outcome: String,
    pub balance: String,
}

/// Order fees breakdown
#[derive(Debug, Clone, Serialize)]
pub struct OrderFees {
    pub platform_fee: f64,
    pub gas_estimate: f64,
    pub total_fee: f64,
    pub fee_rate_bps: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::Path,
        routing::{delete, get, post},
        Json, Router,
    };
    use serde_json::json;

    fn test_auth_config() -> PolymarketAuthConfig {
        PolymarketAuthConfig {
            api_key: "test-key".to_string(),
            api_secret: base64::encode("test-secret"),
            api_passphrase: "test-passphrase".to_string(),
            private_key: None,
            address: ethereum_types::Address::zero(),
        }
    }

    async fn start_mock_clob_server() -> (String, tokio::task::JoinHandle<()>) {
        async fn post_orders(Json(_body): Json<serde_json::Value>) -> Json<serde_json::Value> {
            Json(json!({
                "order_id": "order_1",
                "order_hash": "0xdeadbeef",
                "status": "OPEN",
                "market_id": "market_123",
                "outcome": "YES",
                "side": "BUY",
                "size": "100",
                "price": 0.42,
                "filled_amount": "0",
                "remaining_amount": "100",
                "average_fill_price": null,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "updated_at": chrono::Utc::now().to_rfc3339()
            }))
        }

        async fn get_order(Path(order_id): Path<String>) -> Json<serde_json::Value> {
            Json(json!({
                "order_id": order_id,
                "order_hash": "0xdeadbeef",
                "status": "OPEN",
                "market_id": "market_123",
                "outcome": "YES",
                "side": "BUY",
                "size": "100",
                "price": 0.42,
                "filled_amount": "0",
                "remaining_amount": "100",
                "average_fill_price": null,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "updated_at": chrono::Utc::now().to_rfc3339()
            }))
        }

        async fn delete_order(Path(order_id): Path<String>) -> Json<serde_json::Value> {
            Json(json!({
                "order_id": order_id,
                "status": "CANCELLED",
                "cancelled_at": chrono::Utc::now().to_rfc3339()
            }))
        }

        async fn get_orders() -> Json<serde_json::Value> {
            Json(json!([
                {
                    "order_id": "order_1",
                    "order_hash": "0xdeadbeef",
                    "status": "OPEN",
                    "market_id": "market_123",
                    "outcome": "YES",
                    "side": "BUY",
                    "size": "100",
                    "price": 0.42,
                    "filled_amount": "0",
                    "remaining_amount": "100",
                    "average_fill_price": null,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                    "updated_at": chrono::Utc::now().to_rfc3339()
                }
            ]))
        }

        let app = Router::new()
            .route("/orders", post(post_orders).get(get_orders))
            .route("/orders/:order_id", get(get_order).delete(delete_order));

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        listener.set_nonblocking(true).expect("nonblocking");

        let server = axum::Server::from_tcp(listener)
            .expect("server")
            .serve(app.into_make_service());

        let base_url = format!("http://{}", addr);
        let handle = tokio::spawn(async move {
            let _ = server.await;
        });

        (base_url, handle)
    }

    fn test_order_request() -> OrderRequest {
        OrderRequest {
            order: PolymarketOrderData {
                salt: "1".to_string(),
                maker: ethereum_types::Address::zero(),
                signer: ethereum_types::Address::zero(),
                taker: ethereum_types::Address::zero(),
                token_id: "1".to_string(),
                maker_amount: "100".to_string(),
                taker_amount: "100".to_string(),
                expiration: "9999999999".to_string(),
                nonce: "1".to_string(),
                fee_rate_bps: "10".to_string(),
                side: 0,
                signature_type: 0,
            },
            signature: "0x00".to_string(),
            owner: None,
        }
    }

    #[tokio::test]
    async fn submit_get_cancel_orders_against_mock() {
        let (base_url, handle) = start_mock_clob_server().await;

        let client = PolymarketClobClient::new_with_base_url(test_auth_config(), base_url)
            .expect("client");

        let submitted = client
            .submit_order(test_order_request())
            .await
            .expect("submit");
        assert_eq!(submitted.order_id, "order_1");

        let fetched = client.get_order(&submitted.order_id).await.expect("get");
        assert_eq!(fetched.order_id, "order_1");

        let cancelled = client.cancel_order(&submitted.order_id).await.expect("cancel");
        assert_eq!(cancelled.order_id, "order_1");

        let open_orders = client
            .get_open_orders(OrdersQuery {
                address: "0x0".to_string(),
                market: None,
                status: Some("OPEN".to_string()),
                limit: Some(10),
                offset: Some(0),
            })
            .await
            .expect("list");
        assert_eq!(open_orders.len(), 1);

        handle.abort();
    }
}
