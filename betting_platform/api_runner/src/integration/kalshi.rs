//! Kalshi integration client

use anyhow::{Result, anyhow};
use reqwest::{Client, header::{HeaderMap, HeaderValue}};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};
use std::time::Duration;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64;

use super::{Platform, ExternalPrice};

const KALSHI_API_BASE: &str = "https://api.elections.kalshi.com/v1";
const KALSHI_TRADING_API: &str = "https://trading-api.kalshi.com/v1";

/// Kalshi client for API interactions
pub struct KalshiClient {
    client: Client,
    api_key: Option<String>,
    api_secret: Option<String>,
}

impl KalshiClient {
    /// Create new Kalshi client
    pub fn new(api_key: Option<String>, api_secret: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
            
        Ok(Self {
            client,
            api_key,
            api_secret,
        })
    }
    
    /// Authenticate and get session token
    pub async fn authenticate(&self) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(anyhow!("API credentials required for Kalshi"));
        }
        
        let auth_request = AuthRequest {
            email: self.api_key.as_ref().unwrap().clone(),
            password: self.api_secret.as_ref().unwrap().clone(),
        };
        
        let response = self.client
            .post(&format!("{}/login", KALSHI_API_BASE))
            .json(&auth_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Kalshi authentication failed: {}", response.status()));
        }
        
        let auth_response: AuthResponse = response.json().await?;
        Ok(auth_response.token)
    }
    
    /// Get active markets
    pub async fn get_markets(&self, limit: usize, status: &str) -> Result<Vec<KalshiMarket>> {
        let url = format!("{}/markets?limit={}&status={}", KALSHI_API_BASE, limit, status);
        
        debug!("Fetching Kalshi markets from: {}", url);
        
        let mut headers = HeaderMap::new();
        if let Some(key) = &self.api_key {
            headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", key))?);
        }
        
        let response = self.client
            .get(&url)
            .headers(headers)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("Kalshi API error: {} - {}", status, body));
        }
        
        let markets_response: MarketsResponse = response.json().await?;
        
        info!("Fetched {} markets from Kalshi", markets_response.markets.len());
        
        Ok(markets_response.markets)
    }
    
    /// Get specific market details
    pub async fn get_market(&self, ticker: &str) -> Result<KalshiMarket> {
        let url = format!("{}/markets/{}", KALSHI_API_BASE, ticker);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch market {}: {}", ticker, response.status()));
        }
        
        let market_response: MarketResponse = response.json().await?;
        Ok(market_response.market)
    }
    
    /// Get order book for a market
    pub async fn get_order_book(&self, ticker: &str) -> Result<KalshiOrderBook> {
        let url = format!("{}/markets/{}/orderbook", KALSHI_API_BASE, ticker);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch order book: {}", response.status()));
        }
        
        let book: KalshiOrderBook = response.json().await?;
        Ok(book)
    }
    
    /// Get market history
    pub async fn get_market_history(
        &self,
        ticker: &str,
        start_ts: i64,
        end_ts: i64,
    ) -> Result<Vec<MarketSnapshot>> {
        let url = format!(
            "{}/markets/{}/history?start_ts={}&end_ts={}",
            KALSHI_API_BASE, ticker, start_ts, end_ts
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch market history: {}", response.status()));
        }
        
        let history: MarketHistoryResponse = response.json().await?;
        Ok(history.history)
    }
    
    /// Convert Kalshi market to internal price format
    pub fn market_to_price(&self, market: &KalshiMarket) -> ExternalPrice {
        let yes_price = market.last_price as f64 / 100.0;
        let no_price = 1.0 - yes_price;
        
        ExternalPrice {
            platform: Platform::Kalshi,
            market_id: market.ticker.clone(),
            outcome_prices: vec![yes_price, no_price],
            liquidity: market.open_interest as f64 * market.last_price as f64 / 100.0,
            volume_24h: market.volume_24h as f64,
            timestamp: chrono::Utc::now().timestamp(),
            confidence: calculate_kalshi_confidence(market),
        }
    }
    
    /// Subscribe to market data stream
    pub async fn subscribe_to_stream(
        &self, 
        tickers: Vec<String>,
        callback: impl Fn(StreamUpdate) + Send + 'static,
    ) -> Result<()> {
        // Kalshi uses WebSocket for streaming
        // Implementation would establish WebSocket connection
        // and handle incoming updates
        
        info!("Subscribed to Kalshi market stream for {} tickers", tickers.len());
        Ok(())
    }
    
    /// Generate request signature for authenticated endpoints
    fn generate_signature(&self, method: &str, path: &str, timestamp: i64) -> Result<String> {
        if let Some(secret) = &self.api_secret {
            let message = format!("{}{}{}", timestamp, method, path);
            
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
            mac.update(message.as_bytes());
            
            let signature = mac.finalize();
            Ok(base64::encode(signature.into_bytes()))
        } else {
            Err(anyhow!("API secret required for signature"))
        }
    }
}

/// Calculate confidence score for Kalshi market
fn calculate_kalshi_confidence(market: &KalshiMarket) -> f64 {
    let mut confidence = 0.0;
    
    // Open interest factor (0-40 points)
    let oi_score = (market.open_interest as f64 / 100_000.0).min(1.0) * 40.0;
    confidence += oi_score;
    
    // Volume factor (0-30 points)
    let volume_score = (market.volume_24h as f64 / 50_000.0).min(1.0) * 30.0;
    confidence += volume_score;
    
    // Spread factor (0-20 points)
    let spread = (market.yes_ask - market.yes_bid) as f64;
    let spread_score = (1.0 - (spread / 10.0).min(1.0)) * 20.0;
    confidence += spread_score;
    
    // Trade count factor (0-10 points)
    if market.trade_count_24h > 100 {
        confidence += 10.0;
    } else {
        confidence += (market.trade_count_24h as f64 / 100.0) * 10.0;
    }
    
    confidence / 100.0
}

/// Authentication request
#[derive(Debug, Serialize)]
struct AuthRequest {
    email: String,
    password: String,
}

/// Authentication response
#[derive(Debug, Deserialize)]
struct AuthResponse {
    token: String,
    member_id: String,
}

/// Markets response
#[derive(Debug, Deserialize)]
struct MarketsResponse {
    markets: Vec<KalshiMarket>,
    cursor: Option<String>,
}

/// Single market response
#[derive(Debug, Deserialize)]
struct MarketResponse {
    market: KalshiMarket,
}

/// Kalshi market structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalshiMarket {
    pub ticker: String,
    pub event_ticker: String,
    pub title: String,
    pub subtitle: String,
    pub status: String,
    pub yes_bid: i32,
    pub yes_ask: i32,
    pub no_bid: i32,
    pub no_ask: i32,
    pub last_price: i32,
    pub open_interest: i32,
    pub volume_24h: i32,
    pub trade_count_24h: i32,
    pub close_time: String,
    pub expiration_time: String,
    pub strike_price: Option<f64>,
    pub result: Option<String>,
    pub can_close_early: bool,
    pub settlement_timer_seconds: Option<i32>,
}

/// Order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalshiOrderBook {
    pub yes_bids: Vec<OrderLevel>,
    pub yes_asks: Vec<OrderLevel>,
    pub no_bids: Vec<OrderLevel>,
    pub no_asks: Vec<OrderLevel>,
    pub last_trade_price: i32,
    pub last_trade_size: i32,
}

/// Order level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: i32,
    pub size: i32,
}

/// Market history response
#[derive(Debug, Deserialize)]
struct MarketHistoryResponse {
    history: Vec<MarketSnapshot>,
}

/// Market snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub timestamp: i64,
    pub yes_price: i32,
    pub volume: i32,
    pub open_interest: i32,
}

/// Stream update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUpdate {
    pub ticker: String,
    pub update_type: String,
    pub data: serde_json::Value,
    pub timestamp: i64,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kalshi_confidence_calculation() {
        let market = KalshiMarket {
            ticker: "TEST-24".to_string(),
            event_ticker: "TEST".to_string(),
            title: "Test Market".to_string(),
            subtitle: "Will test pass?".to_string(),
            status: "active".to_string(),
            yes_bid: 55,
            yes_ask: 57,
            no_bid: 43,
            no_ask: 45,
            last_price: 56,
            open_interest: 50_000,
            volume_24h: 25_000,
            trade_count_24h: 150,
            close_time: "2024-12-31T23:59:59Z".to_string(),
            expiration_time: "2024-12-31T23:59:59Z".to_string(),
            strike_price: None,
            result: None,
            can_close_early: false,
            settlement_timer_seconds: None,
        };
        
        let confidence = calculate_kalshi_confidence(&market);
        assert!(confidence > 0.5 && confidence <= 1.0);
    }
}