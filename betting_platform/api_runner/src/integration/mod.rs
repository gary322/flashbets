//! Cross-platform integration module

pub mod polymarket;
pub mod polymarket_public;
pub mod polymarket_price_feed;
pub mod polymarket_auth;
pub mod polymarket_clob;
pub mod polymarket_ws;
pub mod polymarket_ctf;
pub mod polygon_wallet_http;
pub mod kalshi;
pub mod market_sync;
pub mod price_feed;
pub mod eip712_types;
pub mod eip712_verifier;

pub use market_sync::MarketSyncService;
pub use price_feed::PriceFeedService;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported external platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    Polymarket,
    Kalshi,
    Internal,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Polymarket => write!(f, "Polymarket"),
            Platform::Kalshi => write!(f, "Kalshi"),
            Platform::Internal => write!(f, "Internal"),
        }
    }
}

/// Market mapping between internal and external platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMapping {
    pub internal_id: u128,
    pub platform: Platform,
    pub external_id: String,
    pub last_sync: i64,
    pub sync_enabled: bool,
}

/// Price data from external platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPrice {
    pub platform: Platform,
    pub market_id: String,
    pub outcome_prices: Vec<f64>,
    pub liquidity: f64,
    pub volume_24h: f64,
    pub timestamp: i64,
    pub confidence: f64,
}

/// Integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub polymarket_enabled: bool,
    pub polymarket_api_key: Option<String>,
    pub polymarket_webhook_secret: Option<String>,
    pub kalshi_enabled: bool,
    pub kalshi_api_key: Option<String>,
    pub kalshi_api_secret: Option<String>,
    pub sync_interval_seconds: u64,
    pub max_price_deviation: f64,
    pub min_liquidity_usd: f64,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            polymarket_enabled: true,
            polymarket_api_key: None,
            polymarket_webhook_secret: None,
            kalshi_enabled: true,
            kalshi_api_key: None,
            kalshi_api_secret: None,
            sync_interval_seconds: 60,
            max_price_deviation: 0.05,
            min_liquidity_usd: 10_000.0,
        }
    }
}