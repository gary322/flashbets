//! Polymarket integration client

use anyhow::{Result, anyhow};
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};
use std::time::Duration;
use chrono::{DateTime, Utc};

use super::{Platform, ExternalPrice, eip712_types::PolymarketOrder};

/// Order submission response from Polymarket CLOB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSubmissionResponse {
    pub order_id: String,
    pub order_hash: String,
    pub status: String,
    pub created_at: i64,
}

/// Order status response from Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusResponse {
    pub order_id: String,
    pub order_hash: String,
    pub status: String,
    pub side: String,
    pub outcome: u8,
    pub market_id: String,
    pub amount: String,
    pub price: f64,
    pub filled_amount: String,
    pub remaining_amount: String,
    pub average_price: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// User position on Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketPosition {
    pub market_id: String,
    pub outcome: u8,
    pub shares: String,
    pub average_price: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

const POLYMARKET_API_BASE: &str = "https://clob.polymarket.com";
const POLYMARKET_GAMMA_API: &str = "https://gamma-api.polymarket.com";

/// Polymarket client for API interactions
pub struct PolymarketClient {
    client: Client,
    api_key: Option<String>,
    webhook_secret: Option<String>,
}

impl PolymarketClient {
    /// Create new Polymarket client
    pub fn new(api_key: Option<String>, webhook_secret: Option<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", HeaderValue::from_static("BoomPlatform/1.0"));
        
        if let Some(key) = &api_key {
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", key))?);
        }
        
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()?;
            
        Ok(Self {
            client,
            api_key,
            webhook_secret,
        })
    }
    
    /// Submit order to Polymarket CLOB
    pub async fn submit_order(&self, order: &PolymarketOrder, signature: &str) -> Result<OrderSubmissionResponse> {
        let url = format!("{}/orders", POLYMARKET_API_BASE);
        
        let request_body = serde_json::json!({
            "order": order,
            "signature": signature,
        });
        
        debug!("Submitting order to Polymarket CLOB: {:?}", request_body);
        
        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("Polymarket order submission failed: {} - {}", status, body));
        }
        
        let result: OrderSubmissionResponse = response.json().await?;
        info!("Order submitted to Polymarket. ID: {}", result.order_id);
        
        Ok(result)
    }
    
    /// Get order status from Polymarket
    pub async fn get_order_status(&self, order_id: &str) -> Result<OrderStatusResponse> {
        let url = format!("{}/orders/{}", POLYMARKET_API_BASE, order_id);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get order status: {}", response.status()));
        }
        
        let status: OrderStatusResponse = response.json().await?;
        Ok(status)
    }
    
    /// Cancel order on Polymarket
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let url = format!("{}/orders/{}/cancel", POLYMARKET_API_BASE, order_id);
        
        let response = self.client
            .delete(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("Failed to cancel order: {} - {}", status, body));
        }
        
        info!("Order {} cancelled successfully", order_id);
        Ok(())
    }
    
    /// Get user positions from Polymarket
    pub async fn get_positions(&self, address: &str) -> Result<Vec<PolymarketPosition>> {
        let url = format!("{}/positions?address={}", POLYMARKET_API_BASE, address);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get positions: {}", response.status()));
        }
        
        let positions: Vec<PolymarketPosition> = response.json().await?;
        Ok(positions)
    }
    
    /// Get user's open orders
    pub async fn get_open_orders(&self, address: &str, market_id: Option<&str>) -> Result<Vec<PolymarketOrder>> {
        let mut url = format!("{}/orders?address={}&status=OPEN", POLYMARKET_API_BASE, address);
        
        if let Some(market) = market_id {
            url.push_str(&format!("&market={}", market));
        }
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get open orders: {}", response.status()));
        }
        
        let orders: Vec<PolymarketOrder> = response.json().await?;
        Ok(orders)
    }
    
    /// Get active markets from Polymarket
    pub async fn get_markets(&self, limit: usize) -> Result<Vec<PolymarketMarket>> {
        let url = format!("{}/markets?limit={}&active=true", POLYMARKET_API_BASE, limit);
        
        debug!("Fetching Polymarket markets from: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(anyhow!("Polymarket API error: {} - {}", status, body));
        }
        
        // API returns {data: [...]} format, but data can be null
        let api_response: PolymarketApiResponse = response.json().await?;
        let markets = api_response.data.unwrap_or_default();
        
        info!("Fetched {} markets from Polymarket", markets.len());
        
        Ok(markets)
    }
    
    /// Get specific market details
    pub async fn get_market(&self, condition_id: &str) -> Result<PolymarketMarket> {
        let url = format!("{}/markets/{}", POLYMARKET_API_BASE, condition_id);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch market {}: {}", condition_id, response.status()));
        }
        
        let market: PolymarketMarket = response.json().await?;
        Ok(market)
    }
    
    /// Get order book for a market
    pub async fn get_order_book(&self, token_id: &str) -> Result<OrderBook> {
        let url = format!("{}/book?token_id={}", POLYMARKET_API_BASE, token_id);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch order book: {}", response.status()));
        }
        
        let book: OrderBook = response.json().await?;
        Ok(book)
    }
    
    /// Get price history for a market
    pub async fn get_price_history(
        &self, 
        condition_id: &str,
        start_ts: i64,
        end_ts: i64,
        interval: &str,
    ) -> Result<Vec<PricePoint>> {
        let url = format!(
            "{}/prices?conditionId={}&startTs={}&endTs={}&interval={}",
            POLYMARKET_GAMMA_API, condition_id, start_ts, end_ts, interval
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch price history: {}", response.status()));
        }
        
        let history: PriceHistoryResponse = response.json().await?;
        Ok(history.history)
    }
    
    /// Convert Polymarket market to internal price format
    pub fn market_to_price(&self, market: &PolymarketMarket) -> ExternalPrice {
        let outcome_prices = market.tokens
            .iter()
            .map(|t| t.price)
            .collect();
            
        // Calculate estimated liquidity and volume from token data
        let estimated_liquidity = market.tokens.len() as f64 * market.minimum_order_size * 100.0;
        let estimated_volume = estimated_liquidity * 0.1; // Rough estimate
            
        ExternalPrice {
            platform: Platform::Polymarket,
            market_id: market.condition_id.clone(),
            outcome_prices,
            liquidity: estimated_liquidity,
            volume_24h: estimated_volume,
            timestamp: chrono::Utc::now().timestamp(),
            confidence: calculate_confidence(market),
        }
    }
    
    /// Subscribe to market updates via webhook
    pub async fn subscribe_to_updates(&self, market_ids: Vec<String>, webhook_url: &str) -> Result<()> {
        if self.api_key.is_none() {
            return Err(anyhow!("API key required for webhook subscription"));
        }
        
        let subscription = WebhookSubscription {
            market_ids,
            webhook_url: webhook_url.to_string(),
            events: vec!["price_update".to_string(), "market_resolved".to_string()],
        };
        
        let response = self.client
            .post(&format!("{}/webhooks/subscribe", POLYMARKET_API_BASE))
            .json(&subscription)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to subscribe to webhooks: {}", response.status()));
        }
        
        info!("Successfully subscribed to Polymarket webhooks");
        Ok(())
    }
    
    /// Verify webhook signature
    pub fn verify_webhook(&self, payload: &[u8], signature: &str) -> bool {
        if let Some(secret) = &self.webhook_secret {
            // Implement HMAC verification
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            
            type HmacSha256 = Hmac<Sha256>;
            
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(payload);
            
            let expected = mac.finalize();
            let expected_hex = hex::encode(expected.into_bytes());
            
            expected_hex == signature
        } else {
            false
        }
    }
}

/// Calculate confidence score for a market
fn calculate_confidence(market: &PolymarketMarket) -> f64 {
    let mut confidence = 0.0;
    
    // Market activity factor (0-30 points)
    if market.active && market.accepting_orders {
        confidence += 30.0;
    } else if market.active {
        confidence += 15.0;
    }
    
    // Spread factor (0-30 points) - better when prices are close to 0.5/0.5
    if market.tokens.len() >= 2 {
        let price_sum: f64 = market.tokens.iter().map(|t| t.price).sum();
        let avg_price = price_sum / market.tokens.len() as f64;
        let ideal_price = 0.5;
        let spread_score = (1.0 - (avg_price - ideal_price).abs().min(0.5) / 0.5) * 30.0;
        confidence += spread_score;
    }
    
    // Order size factor (0-20 points) - better when minimum order size is reasonable
    let order_size_score = if market.minimum_order_size <= 10.0 {
        20.0
    } else if market.minimum_order_size <= 50.0 {
        15.0
    } else {
        5.0
    };
    confidence += order_size_score;
    
    // Market freshness (0-20 points) - active markets with recent updates
    if !market.closed && !market.archived {
        confidence += 20.0;
    } else if !market.archived {
        confidence += 10.0;
    }
    
    confidence / 100.0
}

/// Polymarket API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct PolymarketApiResponse {
    pub data: Option<Vec<PolymarketMarket>>,
}

/// Polymarket market structure (matches actual API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketMarket {
    pub condition_id: String,
    pub question: String,
    pub description: Option<String>,
    pub market_slug: String,
    pub end_date_iso: Option<String>,
    pub game_start_time: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub accepting_orders: bool,
    pub accepting_order_timestamp: Option<String>,
    pub minimum_order_size: f64,
    pub minimum_tick_size: f64,
    pub question_id: String,
    pub seconds_delay: u64,
    pub fpmm: String,
    pub maker_base_fee: f64,
    pub taker_base_fee: f64,
    pub notifications_enabled: bool,
    pub neg_risk: bool,
    pub neg_risk_market_id: String,
    pub neg_risk_request_id: String,
    pub icon: String,
    pub image: String,
    pub rewards: MarketRewards,
    pub is_50_50_outcome: bool,
    pub tokens: Vec<MarketToken>,
    pub tags: Vec<String>,
    pub enable_order_book: bool,
}

/// Market rewards structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRewards {
    pub rates: Option<serde_json::Value>,
    pub min_size: f64,
    pub max_spread: f64,
}

/// Market token info (matches actual API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketToken {
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
    pub winner: bool,
}

/// Order book structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub market: String,
    pub asset_id: String,
    pub timestamp: i64,
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
}

/// Order level in book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: String,
    pub size: String,
}

/// Price history response
#[derive(Debug, Serialize, Deserialize)]
pub struct PriceHistoryResponse {
    pub history: Vec<PricePoint>,
}

/// Historical price point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: i64,
    pub price: f64,
    pub volume: f64,
}

/// Webhook subscription request
#[derive(Debug, Serialize)]
struct WebhookSubscription {
    pub market_ids: Vec<String>,
    pub webhook_url: String,
    pub events: Vec<String>,
}

/// Webhook event
#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    pub event_type: String,
    pub market_id: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_confidence_calculation() {
        let market = PolymarketMarket {
            condition_id: "test".to_string(),
            question: "Test market".to_string(),
            description: None,
            market_slug: "test-market".to_string(),
            end_date_iso: None,
            game_start_time: None,
            active: true,
            closed: false,
            archived: false,
            accepting_orders: true,
            accepting_order_timestamp: None,
            minimum_order_size: 15.0,
            minimum_tick_size: 0.01,
            question_id: "test_q".to_string(),
            seconds_delay: 0,
            fpmm: "test_fpmm".to_string(),
            maker_base_fee: 0.0,
            taker_base_fee: 0.0,
            notifications_enabled: true,
            neg_risk: false,
            neg_risk_market_id: "".to_string(),
            neg_risk_request_id: "".to_string(),
            icon: "".to_string(),
            image: "".to_string(),
            rewards: MarketRewards {
                rates: None,
                min_size: 0.0,
                max_spread: 0.0,
            },
            is_50_50_outcome: false,
            tokens: vec![
                MarketToken { 
                    token_id: "token1".to_string(), 
                    outcome: "Yes".to_string(), 
                    price: 0.6, 
                    winner: false 
                },
                MarketToken { 
                    token_id: "token2".to_string(), 
                    outcome: "No".to_string(), 
                    price: 0.4, 
                    winner: false 
                },
            ],
            tags: vec![],
            enable_order_book: false,
        };
        
        let confidence = calculate_confidence(&market);
        assert!(confidence > 0.5 && confidence <= 1.0);
    }
}