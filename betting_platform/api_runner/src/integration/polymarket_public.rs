//! Polymarket public API client (no authentication required)

use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, error};
use std::time::Duration;
use std::env;

const POLYMARKET_API_BASE: &str = "https://clob.polymarket.com";
const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";
const CLOB_API_BASE: &str = "https://clob.polymarket.com";

fn normalize_base_url(url: String) -> String {
    url.trim_end_matches('/').to_string()
}

fn gamma_base_url() -> String {
    normalize_base_url(env::var("POLYMARKET_GAMMA_BASE_URL").unwrap_or_else(|_| GAMMA_API_BASE.to_string()))
}

fn clob_base_url() -> String {
    normalize_base_url(env::var("POLYMARKET_CLOB_BASE_URL").unwrap_or_else(|_| CLOB_API_BASE.to_string()))
}

/// Simplified Polymarket client for public API access
pub struct PolymarketPublicClient {
    client: Client,
}

impl PolymarketPublicClient {
    /// Create new public API client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("BettingPlatform/1.0")
            .build()?;
            
        Ok(Self { client })
    }
    
    /// Get active markets from Polymarket public API
    pub async fn get_markets(&self, limit: usize) -> Result<Vec<PolymarketPublicMarket>> {
        let url = format!("{}/markets?limit={}&active=true", gamma_base_url(), limit);
        
        debug!("Fetching Polymarket markets from: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch markets: {}", response.status()));
        }
        
        let text = response.text().await?;
        debug!("Raw response length: {}", text.len());
        debug!("First 200 chars of response: {}", &text.chars().take(200).collect::<String>());
        
        // Check if response is actually JSON
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| anyhow!("Failed to parse JSON: {} - First 500 chars: {}", e, &text.chars().take(500).collect::<String>()))?;
        
        debug!("Parsed JSON type: {:?}", parsed);
        
        // Check if it's an array or object
        match parsed {
            serde_json::Value::Array(arr) => {
                info!("Got array response with {} items", arr.len());
                // Log first market for debugging
                if let Some(first) = arr.first() {
                    debug!("First market keys: {:?}", first.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                }
                let markets: Vec<PolymarketPublicMarket> = serde_json::from_value(serde_json::Value::Array(arr))
                    .map_err(|e| anyhow!("Failed to parse array into markets: {}", e))?;
                Ok(markets)
            }
            serde_json::Value::Object(_) => {
                // Might be an error response
                Err(anyhow!("Got object response instead of array. Response: {}", serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| "unparseable".to_string())))
            }
            _ => {
                Err(anyhow!("Unexpected response type: {:?}", parsed))
            }
        }
    }
    
    /// Get current active markets from CLOB API
    pub async fn get_current_markets(&self, limit: usize) -> Result<Vec<PolymarketPublicMarket>> {
        let url = format!("{}/markets?limit={}", clob_base_url(), limit);
        
        debug!("Fetching current markets from CLOB API: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch markets: {}", response.status()));
        }
        
        let text = response.text().await?;
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        
        // CLOB API returns either {data: [...]} or an array depending on endpoint/version.
        let data: Option<Vec<serde_json::Value>> = if let Some(arr) = parsed.as_array() {
            Some(arr.clone())
        } else {
            parsed.get("data").and_then(|d| d.as_array()).cloned()
        };

        if let Some(data) = data {
            // Filter for active, non-closed markets
            let active_markets: Vec<serde_json::Value> = data.iter()
                .filter(|m| {
                    let active = m.get("active").and_then(|a| a.as_bool()).unwrap_or(false);
                    let closed = m.get("closed").and_then(|c| c.as_bool()).unwrap_or(true);
                    let archived = m.get("archived").and_then(|a| a.as_bool()).unwrap_or(true);
                    active && !closed && !archived
                })
                .cloned()
                .collect();
            
            info!("Found {} active markets from CLOB API", active_markets.len());
            
            // If no active markets found, fall back to gamma API
            if active_markets.is_empty() {
                info!("No active markets found in CLOB API, falling back to gamma API");
                return self.get_markets(limit).await;
            }
            
            // Convert CLOB format to our format
            let mut markets = Vec::new();
            for market in active_markets.iter().take(limit) {
                let outcomes_json = market.get("tokens")
                    .and_then(|t| t.as_array())
                    .map(|tokens| {
                        let outcomes: Vec<String> = tokens.iter()
                            .filter_map(|t| t.get("outcome").and_then(|o| o.as_str()).map(|s| s.to_string()))
                            .collect();
                        serde_json::to_string(&outcomes).unwrap_or_else(|_| r#"["Yes","No"]"#.to_string())
                    })
                    .unwrap_or_else(|| r#"["Yes","No"]"#.to_string());
                    
                let prices_json = market.get("tokens")
                    .and_then(|t| t.as_array())
                    .map(|tokens| {
                        let prices: Vec<String> = tokens.iter()
                            .filter_map(|t| t.get("price").and_then(|p| p.as_f64()).map(|p| p.to_string()))
                            .collect();
                        serde_json::to_string(&prices).unwrap_or_else(|_| r#"["0.5","0.5"]"#.to_string())
                    })
                    .unwrap_or_else(|| r#"["0.5","0.5"]"#.to_string());
                
                markets.push(PolymarketPublicMarket {
                    id: market.get("condition_id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    condition_id: market.get("condition_id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    question: market.get("question").and_then(|q| q.as_str()).unwrap_or("Unknown").to_string(),
                    description: market.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
                    active: true,
                    closed: false,
                    archived: false,
                    category: market.get("category").and_then(|c| c.as_str()).unwrap_or("Prediction").to_string(),
                    outcomes: outcomes_json,
                    outcome_prices: prices_json,
                    volume: "0".to_string(),
                    volume_num: 0.0,
                    liquidity: "0".to_string(),
                    liquidity_num: 0.0,
                    volume_24hr: 0.0,
                    end_date: market.get("end_date_iso").and_then(|e| e.as_str()).map(|s| s.to_string()),
                    end_date_iso: market.get("end_date_iso").and_then(|e| e.as_str()).map(|s| s.to_string()),
                    icon: market.get("icon").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    image: market.get("image").and_then(|i| i.as_str()).unwrap_or_else(|| 
                        market.get("icon").and_then(|i| i.as_str()).unwrap_or("")
                    ).to_string(),
                    slug: market.get("market_slug").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                    market_type: "polymarket".to_string(),
                    restricted: false,
                    last_trade_price: None,
                    tags: None,
                });
            }
            
            return Ok(markets);
        }
        
        // If CLOB API doesn't work, fall back to gamma API
        info!("CLOB API response format unexpected, falling back to gamma API");
        self.get_markets(limit).await
    }
    
    /// Search markets by query
    pub async fn search_markets(&self, query: &str, limit: usize) -> Result<Vec<PolymarketPublicMarket>> {
        // First get current markets then filter locally
        let all_markets = self.get_current_markets(100).await?;
        
        let query_lower = query.to_lowercase();
        let filtered: Vec<PolymarketPublicMarket> = all_markets
            .into_iter()
            .filter(|market| {
                market.question.to_lowercase().contains(&query_lower) ||
                market.description.to_lowercase().contains(&query_lower) ||
                market.tags.as_ref().map_or(false, |tags| tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower)))
            })
            .take(limit)
            .collect();
            
        Ok(filtered)
    }
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolymarketPublicMarket {
    pub id: String,
    #[serde(rename = "conditionId")]
    pub condition_id: String,
    pub question: String,
    pub description: String,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub category: String,
    pub outcomes: String, // JSON string array
    #[serde(rename = "outcomePrices")]
    pub outcome_prices: String, // JSON string array
    pub volume: String,
    #[serde(rename = "volumeNum")]
    pub volume_num: f64,
    pub liquidity: String,
    #[serde(rename = "liquidityNum")]
    pub liquidity_num: f64,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: f64,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    #[serde(rename = "endDateIso")]
    pub end_date_iso: Option<String>,
    pub icon: String,
    pub image: String,
    pub slug: String,
    #[serde(rename = "marketType")]
    pub market_type: String,
    pub restricted: bool,
    #[serde(rename = "lastTradePrice")]
    pub last_trade_price: Option<f64>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketToken {
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
    pub winner: bool,
}


/// Convert Polymarket format to our internal format
impl PolymarketPublicMarket {
    pub fn to_internal_format(&self) -> serde_json::Value {
        // Parse outcomes from JSON string
        let outcomes: Vec<String> = serde_json::from_str(&self.outcomes).unwrap_or_default();
        let outcome_prices: Vec<String> = serde_json::from_str(&self.outcome_prices).unwrap_or_default();
        
        // Convert string prices to floats
        let prices: Vec<f64> = outcome_prices.iter()
            .map(|p| p.parse::<f64>().unwrap_or(0.0))
            .collect();
        
        serde_json::json!({
            "id": self.condition_id.clone(),
            "title": self.question.clone(),
            "description": self.description.clone(),
            "active": self.active,
            "closed": self.closed,
            "total_volume": self.volume_num,
            "volume_24h": self.volume_24hr,  
            "total_liquidity": self.liquidity_num,
            "outcomes": outcomes.iter().enumerate().map(|(i, outcome)| {
                serde_json::json!({
                    "name": outcome.clone(),
                    "price": prices.get(i).cloned().unwrap_or(0.0),
                    "token_id": format!("{}-{}", self.condition_id, i),
                    "total_stake": 0, // Not calculable without volume
                })
            }).collect::<Vec<_>>(),
            "source": "polymarket",
            "created_at": chrono::Utc::now().timestamp(),
            "end_date": self.end_date_iso.clone().or(self.end_date.clone()),
            "tags": self.tags.clone().unwrap_or_default(),
            "resolution_time": 0, // Would need to parse end_date
            "verse_id": self.get_verse_id(),
            "icon": Some(self.icon.clone()),
            "market_slug": Some(self.slug.clone()),
        })
    }
    
    fn get_verse_id(&self) -> i32 {
        // Map to verse IDs based on category/tags
        let question_lower = self.question.to_lowercase();
        let category_lower = self.category.to_lowercase();
        
        if category_lower.contains("crypto") || question_lower.contains("bitcoin") || question_lower.contains("btc") {
            20 // Crypto verse
        } else if category_lower.contains("politic") || category_lower.contains("us-current-affairs") || question_lower.contains("election") || question_lower.contains("president") {
            1 // Politics verse
        } else if question_lower.contains("super bowl") || question_lower.contains("nfl") {
            10 // Sports verse
        } else if question_lower.contains("s&p") || question_lower.contains("stock") {
            30 // Finance verse
        } else {
            50 // General verse
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_public_api() {
        let client = PolymarketPublicClient::new().unwrap();
        let markets = client.get_markets(5).await;
        assert!(markets.is_ok());
        let markets = markets.unwrap();
        assert!(!markets.is_empty());
        println!("Fetched {} markets", markets.len());
    }
}
