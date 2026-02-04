//! Real-time Polymarket price feed integration

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::time::{interval, Duration};
use tracing::{info, error, debug, warn};
use chrono::Utc;

use super::{
    polymarket_public::PolymarketPublicClient,
    price_feed::{PriceFeedService, AggregationRule, AggregationMethod},
    Platform, ExternalPrice,
};

/// Polymarket real-time price feed
pub struct PolymarketPriceFeed {
    client: Arc<PolymarketPublicClient>,
    price_feed: Arc<PriceFeedService>,
    tracked_markets: Arc<tokio::sync::RwLock<HashMap<String, TrackedMarket>>>,
    update_interval: Duration,
}

#[derive(Debug, Clone)]
struct TrackedMarket {
    polymarket_id: String,
    internal_id: String,
    last_update: i64,
    last_prices: Vec<f64>,
}

impl PolymarketPriceFeed {
    /// Create new Polymarket price feed
    pub fn new(
        client: Arc<PolymarketPublicClient>,
        price_feed: Arc<PriceFeedService>,
        update_interval_seconds: u64,
    ) -> Self {
        Self {
            client,
            price_feed,
            tracked_markets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            update_interval: Duration::from_secs(update_interval_seconds),
        }
    }
    
    /// Start real-time price feed
    pub async fn start(&self) -> Result<()> {
        info!("Starting Polymarket real-time price feed");
        
        let feed = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(feed.update_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = feed.update_prices().await {
                    error!("Failed to update Polymarket prices: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Track a market for price updates
    pub async fn track_market(&self, polymarket_id: String, internal_id: String) -> Result<()> {
        let mut markets = self.tracked_markets.write().await;
        
        markets.insert(polymarket_id.clone(), TrackedMarket {
            polymarket_id,
            internal_id,
            last_update: 0,
            last_prices: vec![],
        });
        
        Ok(())
    }
    
    /// Stop tracking a market
    pub async fn untrack_market(&self, polymarket_id: &str) -> Result<()> {
        let mut markets = self.tracked_markets.write().await;
        markets.remove(polymarket_id);
        Ok(())
    }
    
    /// Update prices for all tracked markets
    async fn update_prices(&self) -> Result<()> {
        let markets = self.tracked_markets.read().await.clone();
        
        if markets.is_empty() {
            return Ok(());
        }
        
        debug!("Updating prices for {} tracked markets", markets.len());
        
        // Fetch current market data from Polymarket
        let polymarket_markets = match self.client.get_current_markets(100).await {
            Ok(markets) => markets,
            Err(e) => {
                warn!("Failed to fetch Polymarket markets: {}", e);
                return Ok(()); // Don't fail, just skip this update
            }
        };
        
        // Update prices for each tracked market
        for (polymarket_id, tracked) in markets.iter() {
            if let Some(market) = polymarket_markets.iter().find(|m| &m.id == polymarket_id) {
                // Parse outcome prices
                let prices: Vec<f64> = match serde_json::from_str(&market.outcome_prices) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("Failed to parse prices for market {}: {}", polymarket_id, e);
                        continue;
                    }
                };
                
                // Check if prices have changed
                if prices != tracked.last_prices {
                    let price_update = ExternalPrice {
                        platform: Platform::Polymarket,
                        market_id: polymarket_id.clone(),
                        outcome_prices: prices.clone(),
                        liquidity: market.liquidity_num,
                        volume_24h: market.volume_24hr,
                        timestamp: Utc::now().timestamp(),
                        confidence: calculate_confidence(market.liquidity_num, market.volume_24hr),
                    };
                    
                    // Send update to price feed service
                    if let Err(e) = self.price_feed.update_price(price_update).await {
                        error!("Failed to update price for market {}: {}", polymarket_id, e);
                    } else {
                        debug!("Updated prices for market {}: {:?}", polymarket_id, prices);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Set up aggregation rules for a market
    pub async fn setup_aggregation(
        &self,
        internal_market_id: String,
        polymarket_id: String,
        additional_sources: Vec<(Platform, String)>,
    ) -> Result<()> {
        let mut sources = vec![(Platform::Polymarket, polymarket_id.clone())];
        sources.extend(additional_sources);
        
        let rule = AggregationRule {
            internal_market_id: internal_market_id.clone(),
            sources,
            method: AggregationMethod::WeightedAverage,
            min_sources: 1, // At least Polymarket
            max_deviation: 0.1, // 10% max deviation
        };
        
        self.price_feed.add_aggregation_rule(rule).await?;
        
        // Track this market
        self.track_market(polymarket_id, internal_market_id).await?;
        
        Ok(())
    }
    
    /// Get current price for a market
    pub async fn get_current_price(&self, polymarket_id: &str) -> Option<Vec<f64>> {
        let markets = self.tracked_markets.read().await;
        
        if let Some(tracked) = markets.get(polymarket_id) {
            if let Some(price) = self.price_feed.get_price(Platform::Polymarket, polymarket_id).await {
                return Some(price.outcome_prices);
            }
        }
        
        None
    }
}

impl Clone for PolymarketPriceFeed {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            price_feed: self.price_feed.clone(),
            tracked_markets: self.tracked_markets.clone(),
            update_interval: self.update_interval,
        }
    }
}

/// Calculate confidence score based on liquidity and volume
fn calculate_confidence(liquidity: f64, volume_24h: f64) -> f64 {
    let liquidity_score = (liquidity / 100_000.0).min(1.0); // $100k = perfect liquidity
    let volume_score = (volume_24h / 50_000.0).min(1.0); // $50k daily = perfect volume
    
    // Weight liquidity more heavily than volume
    (liquidity_score * 0.7 + volume_score * 0.3).min(1.0)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_confidence_calculation() {
        assert_eq!(calculate_confidence(100_000.0, 50_000.0), 1.0);
        assert_eq!(calculate_confidence(50_000.0, 25_000.0), 0.5);
        assert_eq!(calculate_confidence(0.0, 0.0), 0.0);
    }
}