//! Real-time price feed service

use anyhow::{Result, anyhow};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error, debug};
use serde::{Deserialize, Serialize};

use super::{Platform, ExternalPrice};

/// Price update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub market_id: String,
    pub platform: Platform,
    pub old_prices: Vec<f64>,
    pub new_prices: Vec<f64>,
    pub liquidity: f64,
    pub volume_24h: f64,
    pub timestamp: i64,
    pub confidence: f64,
}

/// Price feed subscriber
pub type PriceSubscriber = broadcast::Receiver<PriceUpdate>;

/// Price feed service for real-time updates
pub struct PriceFeedService {
    price_cache: Arc<RwLock<HashMap<String, ExternalPrice>>>,
    update_sender: broadcast::Sender<PriceUpdate>,
    subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>, // market_id -> subscriber_ids
    aggregation_rules: Arc<RwLock<HashMap<String, AggregationRule>>>,
}

/// Price aggregation rule
#[derive(Debug, Clone)]
pub struct AggregationRule {
    pub internal_market_id: String,
    pub sources: Vec<(Platform, String)>, // (platform, external_id)
    pub method: AggregationMethod,
    pub min_sources: usize,
    pub max_deviation: f64,
}

/// Aggregation methods
#[derive(Debug, Clone, Copy)]
pub enum AggregationMethod {
    Median,
    WeightedAverage,
    BestPrice,
    Conservative, // Most conservative price
}

impl PriceFeedService {
    /// Create new price feed service
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        
        Self {
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            update_sender: tx,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            aggregation_rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Subscribe to price updates
    pub fn subscribe(&self) -> PriceSubscriber {
        self.update_sender.subscribe()
    }
    
    /// Subscribe to specific market updates
    pub async fn subscribe_to_market(&self, subscriber_id: String, market_id: String) -> Result<()> {
        let mut subscribers = self.subscribers.write().await;
        subscribers.entry(market_id)
            .or_insert_with(Vec::new)
            .push(subscriber_id);
        Ok(())
    }
    
    /// Update price from external source
    pub async fn update_price(&self, price: ExternalPrice) -> Result<()> {
        let key = format!("{}:{}", price.platform, price.market_id);
        
        // Get old price for comparison
        let old_price = {
            let cache = self.price_cache.read().await;
            cache.get(&key).cloned()
        };
        
        // Check if price has changed significantly
        if let Some(old) = &old_price {
            if !self.has_significant_change(old, &price) {
                return Ok(());
            }
        }
        
        // Update cache
        {
            let mut cache = self.price_cache.write().await;
            cache.insert(key.clone(), price.clone());
        }
        
        // Create price update event
        let update = PriceUpdate {
            market_id: price.market_id.clone(),
            platform: price.platform,
            old_prices: old_price.map(|p| p.outcome_prices).unwrap_or_default(),
            new_prices: price.outcome_prices.clone(),
            liquidity: price.liquidity,
            volume_24h: price.volume_24h,
            timestamp: price.timestamp,
            confidence: price.confidence,
        };
        
        // Broadcast update
        if let Err(e) = self.update_sender.send(update.clone()) {
            debug!("No active price subscribers: {}", e);
        }
        
        // Check aggregation rules
        self.check_aggregation_rules(&price).await?;
        
        Ok(())
    }
    
    /// Check if price has changed significantly
    fn has_significant_change(&self, old: &ExternalPrice, new: &ExternalPrice) -> bool {
        // Check price changes
        if old.outcome_prices.len() != new.outcome_prices.len() {
            return true;
        }
        
        for (old_price, new_price) in old.outcome_prices.iter().zip(&new.outcome_prices) {
            let change = (new_price - old_price).abs() / old_price;
            if change > 0.001 { // 0.1% change threshold
                return true;
            }
        }
        
        // Check liquidity changes
        let liq_change = (new.liquidity - old.liquidity).abs() / old.liquidity.max(1.0);
        if liq_change > 0.1 { // 10% liquidity change
            return true;
        }
        
        false
    }
    
    /// Check and apply aggregation rules
    async fn check_aggregation_rules(&self, price: &ExternalPrice) -> Result<()> {
        let rules = self.aggregation_rules.read().await;
        
        for (market_id, rule) in rules.iter() {
            // Check if this price update affects the rule
            let affects_rule = rule.sources.iter().any(|(platform, ext_id)| {
                *platform == price.platform && ext_id == &price.market_id
            });
            
            if affects_rule {
                // Aggregate prices for this market
                if let Ok(aggregated) = self.aggregate_prices(market_id, rule).await {
                    // Broadcast aggregated price
                    let update = PriceUpdate {
                        market_id: market_id.clone(),
                        platform: Platform::Internal,
                        old_prices: vec![], // TODO: Track old aggregated prices
                        new_prices: aggregated.outcome_prices,
                        liquidity: aggregated.liquidity,
                        volume_24h: aggregated.volume_24h,
                        timestamp: aggregated.timestamp,
                        confidence: aggregated.confidence,
                    };
                    
                    let _ = self.update_sender.send(update);
                }
            }
        }
        
        Ok(())
    }
    
    /// Aggregate prices from multiple sources
    async fn aggregate_prices(
        &self,
        market_id: &str,
        rule: &AggregationRule,
    ) -> Result<ExternalPrice> {
        let cache = self.price_cache.read().await;
        let mut source_prices = Vec::new();
        
        // Collect prices from all sources
        for (platform, external_id) in &rule.sources {
            let key = format!("{}:{}", platform, external_id);
            if let Some(price) = cache.get(&key) {
                source_prices.push(price.clone());
            }
        }
        
        // Check minimum sources
        if source_prices.len() < rule.min_sources {
            return Err(anyhow!(
                "Insufficient price sources: {} < {}",
                source_prices.len(),
                rule.min_sources
            ));
        }
        
        // Aggregate based on method
        let aggregated = match rule.method {
            AggregationMethod::Median => self.aggregate_median(&source_prices),
            AggregationMethod::WeightedAverage => self.aggregate_weighted(&source_prices),
            AggregationMethod::BestPrice => self.aggregate_best_price(&source_prices),
            AggregationMethod::Conservative => self.aggregate_conservative(&source_prices),
        }?;
        
        // Check deviation
        if !self.check_deviation(&source_prices, &aggregated, rule.max_deviation) {
            return Err(anyhow!("Price deviation exceeds maximum allowed"));
        }
        
        Ok(aggregated)
    }
    
    /// Aggregate using median method
    fn aggregate_median(&self, prices: &[ExternalPrice]) -> Result<ExternalPrice> {
        if prices.is_empty() {
            return Err(anyhow!("No prices to aggregate"));
        }
        
        let outcome_count = prices[0].outcome_prices.len();
        let mut aggregated_prices = vec![0.0; outcome_count];
        
        // Calculate median for each outcome
        for i in 0..outcome_count {
            let mut outcome_prices: Vec<f64> = prices.iter()
                .map(|p| p.outcome_prices.get(i).copied().unwrap_or(0.0))
                .collect();
            outcome_prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let median = if outcome_prices.len() % 2 == 0 {
                let mid = outcome_prices.len() / 2;
                (outcome_prices[mid - 1] + outcome_prices[mid]) / 2.0
            } else {
                outcome_prices[outcome_prices.len() / 2]
            };
            
            aggregated_prices[i] = median;
        }
        
        // Aggregate other metrics
        let total_liquidity: f64 = prices.iter().map(|p| p.liquidity).sum();
        let total_volume: f64 = prices.iter().map(|p| p.volume_24h).sum();
        let avg_confidence: f64 = prices.iter().map(|p| p.confidence).sum::<f64>() / prices.len() as f64;
        
        Ok(ExternalPrice {
            platform: Platform::Internal,
            market_id: "aggregated".to_string(),
            outcome_prices: aggregated_prices,
            liquidity: total_liquidity,
            volume_24h: total_volume,
            timestamp: chrono::Utc::now().timestamp(),
            confidence: avg_confidence,
        })
    }
    
    /// Aggregate using weighted average
    fn aggregate_weighted(&self, prices: &[ExternalPrice]) -> Result<ExternalPrice> {
        if prices.is_empty() {
            return Err(anyhow!("No prices to aggregate"));
        }
        
        let outcome_count = prices[0].outcome_prices.len();
        let mut aggregated_prices = vec![0.0; outcome_count];
        
        // Calculate total weight (based on liquidity)
        let total_weight: f64 = prices.iter().map(|p| p.liquidity).sum();
        
        if total_weight == 0.0 {
            return self.aggregate_median(prices); // Fallback to median
        }
        
        // Calculate weighted average for each outcome
        for i in 0..outcome_count {
            let weighted_sum: f64 = prices.iter()
                .map(|p| {
                    let price = p.outcome_prices.get(i).copied().unwrap_or(0.0);
                    let weight = p.liquidity / total_weight;
                    price * weight
                })
                .sum();
            
            aggregated_prices[i] = weighted_sum;
        }
        
        // Aggregate other metrics
        let total_liquidity: f64 = prices.iter().map(|p| p.liquidity).sum();
        let total_volume: f64 = prices.iter().map(|p| p.volume_24h).sum();
        let weighted_confidence: f64 = prices.iter()
            .map(|p| p.confidence * (p.liquidity / total_weight))
            .sum();
        
        Ok(ExternalPrice {
            platform: Platform::Internal,
            market_id: "aggregated".to_string(),
            outcome_prices: aggregated_prices,
            liquidity: total_liquidity,
            volume_24h: total_volume,
            timestamp: chrono::Utc::now().timestamp(),
            confidence: weighted_confidence,
        })
    }
    
    /// Aggregate using best price method (most favorable to traders)
    fn aggregate_best_price(&self, prices: &[ExternalPrice]) -> Result<ExternalPrice> {
        if prices.is_empty() {
            return Err(anyhow!("No prices to aggregate"));
        }
        
        let outcome_count = prices[0].outcome_prices.len();
        let mut best_prices = vec![0.0; outcome_count];
        
        // For binary markets, best price means highest for YES, lowest for NO
        for i in 0..outcome_count {
            let outcome_prices: Vec<f64> = prices.iter()
                .map(|p| p.outcome_prices.get(i).copied().unwrap_or(0.0))
                .collect();
            
            if i == 0 { // YES outcome
                best_prices[i] = outcome_prices.iter().cloned().fold(0.0, f64::max);
            } else { // NO outcome or others
                best_prices[i] = outcome_prices.iter().cloned().fold(1.0, f64::min);
            }
        }
        
        // Use best liquidity source
        let best_liquidity = prices.iter().map(|p| p.liquidity).fold(0.0, f64::max);
        let total_volume: f64 = prices.iter().map(|p| p.volume_24h).sum();
        let best_confidence = prices.iter().map(|p| p.confidence).fold(0.0, f64::max);
        
        Ok(ExternalPrice {
            platform: Platform::Internal,
            market_id: "aggregated".to_string(),
            outcome_prices: best_prices,
            liquidity: best_liquidity,
            volume_24h: total_volume,
            timestamp: chrono::Utc::now().timestamp(),
            confidence: best_confidence,
        })
    }
    
    /// Aggregate using conservative method (least risk)
    fn aggregate_conservative(&self, prices: &[ExternalPrice]) -> Result<ExternalPrice> {
        if prices.is_empty() {
            return Err(anyhow!("No prices to aggregate"));
        }
        
        let outcome_count = prices[0].outcome_prices.len();
        let mut conservative_prices = vec![0.5; outcome_count]; // Start at 50%
        
        // Conservative means prices closer to 50% (maximum uncertainty)
        for i in 0..outcome_count {
            let outcome_prices: Vec<f64> = prices.iter()
                .map(|p| p.outcome_prices.get(i).copied().unwrap_or(0.5))
                .collect();
            
            // Find price closest to 0.5
            conservative_prices[i] = outcome_prices.into_iter()
                .min_by_key(|&p| ((p - 0.5).abs() * 1000.0) as i64)
                .unwrap_or(0.5);
        }
        
        // Use minimum liquidity (most conservative)
        let min_liquidity = prices.iter().map(|p| p.liquidity).fold(f64::MAX, f64::min);
        let total_volume: f64 = prices.iter().map(|p| p.volume_24h).sum();
        let min_confidence = prices.iter().map(|p| p.confidence).fold(1.0, f64::min);
        
        Ok(ExternalPrice {
            platform: Platform::Internal,
            market_id: "aggregated".to_string(),
            outcome_prices: conservative_prices,
            liquidity: min_liquidity,
            volume_24h: total_volume,
            timestamp: chrono::Utc::now().timestamp(),
            confidence: min_confidence,
        })
    }
    
    /// Check if aggregated price deviates too much from sources
    fn check_deviation(
        &self,
        sources: &[ExternalPrice],
        aggregated: &ExternalPrice,
        max_deviation: f64,
    ) -> bool {
        for (i, agg_price) in aggregated.outcome_prices.iter().enumerate() {
            for source in sources {
                if let Some(source_price) = source.outcome_prices.get(i) {
                    let deviation = (agg_price - source_price).abs() / source_price.max(0.001);
                    if deviation > max_deviation {
                        return false;
                    }
                }
            }
        }
        true
    }
    
    /// Add aggregation rule
    pub async fn add_aggregation_rule(&self, rule: AggregationRule) -> Result<()> {
        let mut rules = self.aggregation_rules.write().await;
        rules.insert(rule.internal_market_id.clone(), rule);
        Ok(())
    }
    
    /// Get current price for market
    pub async fn get_price(&self, platform: Platform, market_id: &str) -> Option<ExternalPrice> {
        let key = format!("{}:{}", platform, market_id);
        let cache = self.price_cache.read().await;
        cache.get(&key).cloned()
    }
    
    /// Get all prices for a market (from all platforms)
    pub async fn get_all_prices_for_market(&self, internal_id: &str) -> Vec<ExternalPrice> {
        let rules = self.aggregation_rules.read().await;
        let cache = self.price_cache.read().await;
        
        if let Some(rule) = rules.get(internal_id) {
            rule.sources.iter()
                .filter_map(|(platform, external_id)| {
                    let key = format!("{}:{}", platform, external_id);
                    cache.get(&key).cloned()
                })
                .collect()
        } else {
            vec![]
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_median_aggregation() {
        let service = PriceFeedService::new();
        
        let prices = vec![
            ExternalPrice {
                platform: Platform::Polymarket,
                market_id: "test1".to_string(),
                outcome_prices: vec![0.6, 0.4],
                liquidity: 10000.0,
                volume_24h: 5000.0,
                timestamp: 0,
                confidence: 0.8,
            },
            ExternalPrice {
                platform: Platform::Kalshi,
                market_id: "test2".to_string(),
                outcome_prices: vec![0.65, 0.35],
                liquidity: 15000.0,
                volume_24h: 7000.0,
                timestamp: 0,
                confidence: 0.9,
            },
            ExternalPrice {
                platform: Platform::Internal,
                market_id: "test3".to_string(),
                outcome_prices: vec![0.62, 0.38],
                liquidity: 12000.0,
                volume_24h: 6000.0,
                timestamp: 0,
                confidence: 0.85,
            },
        ];
        
        let aggregated = service.aggregate_median(&prices).unwrap();
        assert_eq!(aggregated.outcome_prices[0], 0.62);
        assert_eq!(aggregated.outcome_prices[1], 0.38);
    }
}