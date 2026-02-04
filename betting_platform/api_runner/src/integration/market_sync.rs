//! Market synchronization service for cross-platform integration

use anyhow::{Result, anyhow};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, Duration};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error, warn, debug};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{
    Platform, MarketMapping, ExternalPrice, IntegrationConfig,
    polymarket::PolymarketClient,
    kalshi::KalshiClient,
};
use crate::rpc_client::BettingPlatformClient;
use solana_sdk::pubkey::Pubkey;

/// Market synchronization service
pub struct MarketSyncService {
    config: IntegrationConfig,
    polymarket_client: Option<PolymarketClient>,
    kalshi_client: Option<KalshiClient>,
    platform_client: Arc<BettingPlatformClient>,
    market_mappings: Arc<RwLock<HashMap<u128, Vec<MarketMapping>>>>,
    price_cache: Arc<RwLock<HashMap<String, ExternalPrice>>>,
    sync_status: Arc<Mutex<SyncStatus>>,
}

/// Sync status tracking
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub last_sync: DateTime<Utc>,
    pub next_sync: DateTime<Utc>,
    pub total_syncs: u64,
    pub failed_syncs: u64,
    pub active_mappings: usize,
    pub is_running: bool,
}

impl MarketSyncService {
    /// Create new market sync service
    pub fn new(
        config: IntegrationConfig,
        platform_client: Arc<BettingPlatformClient>,
    ) -> Result<Self> {
        let polymarket_client = if config.polymarket_enabled {
            Some(PolymarketClient::new(
                config.polymarket_api_key.clone(),
                config.polymarket_webhook_secret.clone(),
            )?)
        } else {
            None
        };
        
        let kalshi_client = if config.kalshi_enabled {
            Some(KalshiClient::new(
                config.kalshi_api_key.clone(),
                config.kalshi_api_secret.clone(),
            )?)
        } else {
            None
        };
        
        Ok(Self {
            config,
            polymarket_client,
            kalshi_client,
            platform_client,
            market_mappings: Arc::new(RwLock::new(HashMap::new())),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            sync_status: Arc::new(Mutex::new(SyncStatus {
                last_sync: Utc::now(),
                next_sync: Utc::now(),
                total_syncs: 0,
                failed_syncs: 0,
                active_mappings: 0,
                is_running: false,
            })),
        })
    }
    
    /// Start synchronization service
    pub async fn start(&self) -> anyhow::Result<()> {
        {
            let mut status = self.sync_status.lock().await;
            if status.is_running {
                return Err(anyhow!("Sync service already running"));
            }
            status.is_running = true;
        }
        
        info!("Starting market synchronization service");
        
        // Initial sync
        self.sync_all_markets().await?;
        
        // Start periodic sync
        let sync_interval = Duration::from_secs(self.config.sync_interval_seconds);
        let mut interval = interval(sync_interval);
        
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                if let Err(e) = service.sync_all_markets().await {
                    error!("Market sync failed: {}", e);
                    let mut status = service.sync_status.lock().await;
                    status.failed_syncs += 1;
                }
            }
        });
        
        Ok(())
    }
    
    /// Sync all markets
    pub async fn sync_all_markets(&self) -> anyhow::Result<()> {
        let start_time = Utc::now();
        debug!("Starting market synchronization");
        
        let mut synced_markets = 0;
        let mut errors = Vec::new();
        
        // Sync Polymarket
        if self.polymarket_client.is_some() {
            match self.sync_polymarket_markets().await {
                Ok(count) => synced_markets += count,
                Err(e) => errors.push(format!("Polymarket: {}", e)),
            }
        }
        
        // Sync Kalshi
        if self.kalshi_client.is_some() {
            match self.sync_kalshi_markets().await {
                Ok(count) => synced_markets += count,
                Err(e) => errors.push(format!("Kalshi: {}", e)),
            }
        }
        
        // Update sync status
        {
            let mut status = self.sync_status.lock().await;
            status.last_sync = start_time;
            status.next_sync = Utc::now() + chrono::Duration::seconds(self.config.sync_interval_seconds as i64);
            status.total_syncs += 1;
            
            if !errors.is_empty() {
                status.failed_syncs += 1;
                error!("Sync completed with errors: {:?}", errors);
            }
        }
        
        let duration = Utc::now().signed_duration_since(start_time);
        info!(
            "Market sync completed: {} markets synced in {}ms",
            synced_markets,
            duration.num_milliseconds()
        );
        
        Ok(())
    }
    
    /// Sync Polymarket markets
    async fn sync_polymarket_markets(&self) -> Result<usize> {
        let client = self.polymarket_client.as_ref()
            .ok_or_else(|| anyhow!("Polymarket client not initialized"))?;
            
        // Fetch active markets
        let markets = client.get_markets(50).await?;
        let mut synced_count = 0;
        
        for market in markets {
            // Convert to price data
            let price = client.market_to_price(&market);
            
            // Check if we should sync this market
            if self.should_sync_market(&price).await {
                // Find or create mapping
                if let Some(internal_id) = self.find_internal_market(&market.question).await {
                    // Update existing mapping
                    self.update_price_cache(internal_id, price.clone()).await?;
                    
                    // Optionally update on-chain oracle
                    if let Err(e) = self.update_onchain_oracle(internal_id, &price).await {
                        warn!("Failed to update on-chain oracle for market {}: {}", internal_id, e);
                    }
                    
                    synced_count += 1;
                } else {
                    // Create new market mirror
                    match self.create_market_mirror(&market.question, &price).await {
                        Ok(id) => {
                            info!("Created new market mirror: {} -> {}", market.condition_id, id);
                            synced_count += 1;
                        }
                        Err(e) => warn!("Failed to create market mirror: {}", e),
                    }
                }
            }
            
            // Cache the price
            self.price_cache.write().await.insert(
                format!("polymarket:{}", market.condition_id),
                price,
            );
        }
        
        Ok(synced_count)
    }
    
    /// Sync Kalshi markets
    async fn sync_kalshi_markets(&self) -> Result<usize> {
        let client = self.kalshi_client.as_ref()
            .ok_or_else(|| anyhow!("Kalshi client not initialized"))?;
            
        // Fetch active markets
        let markets = client.get_markets(50, "active").await?;
        let mut synced_count = 0;
        
        for market in markets {
            // Convert to price data
            let price = client.market_to_price(&market);
            
            // Check if we should sync this market
            if self.should_sync_market(&price).await {
                // Find or create mapping
                if let Some(internal_id) = self.find_internal_market(&market.title).await {
                    // Update existing mapping
                    self.update_price_cache(internal_id, price.clone()).await?;
                    
                    // Optionally update on-chain oracle
                    if let Err(e) = self.update_onchain_oracle(internal_id, &price).await {
                        warn!("Failed to update on-chain oracle for market {}: {}", internal_id, e);
                    }
                    
                    synced_count += 1;
                } else {
                    // Create new market mirror
                    match self.create_market_mirror(&market.title, &price).await {
                        Ok(id) => {
                            info!("Created new market mirror: {} -> {}", market.ticker, id);
                            synced_count += 1;
                        }
                        Err(e) => warn!("Failed to create market mirror: {}", e),
                    }
                }
            }
            
            // Cache the price
            self.price_cache.write().await.insert(
                format!("kalshi:{}", market.ticker),
                price,
            );
        }
        
        Ok(synced_count)
    }
    
    /// Check if market should be synced
    async fn should_sync_market(&self, price: &ExternalPrice) -> bool {
        // Check liquidity threshold
        if price.liquidity < self.config.min_liquidity_usd {
            return false;
        }
        
        // Check if market has valid prices
        if price.outcome_prices.is_empty() || 
           price.outcome_prices.iter().any(|&p| p < 0.0 || p > 1.0) {
            return false;
        }
        
        // Check confidence
        if price.confidence < 0.5 {
            return false;
        }
        
        true
    }
    
    /// Find internal market ID by title/question
    async fn find_internal_market(&self, title: &str) -> Option<u128> {
        // In production, this would query the on-chain markets
        // For now, use a simple hash-based approach
        let hash = calculate_title_hash(title);
        
        // Check if we have a mapping
        let mappings = self.market_mappings.read().await;
        mappings.get(&hash).and_then(|maps| {
            maps.iter().find(|m| m.sync_enabled).map(|m| m.internal_id)
        })
    }
    
    /// Create a new market mirror
    async fn create_market_mirror(&self, title: &str, price: &ExternalPrice) -> Result<u128> {
        // Generate market ID
        let market_id = generate_market_id(title);
        
        // Create market on-chain
        // In production, this would create the actual market
        // For now, just create the mapping
        
        let mapping = MarketMapping {
            internal_id: market_id,
            platform: price.platform,
            external_id: price.market_id.clone(),
            last_sync: chrono::Utc::now().timestamp(),
            sync_enabled: true,
        };
        
        // Store mapping
        let mut mappings = self.market_mappings.write().await;
        mappings.entry(market_id)
            .or_insert_with(Vec::new)
            .push(mapping);
            
        Ok(market_id)
    }
    
    /// Update price cache
    async fn update_price_cache(&self, market_id: u128, price: ExternalPrice) -> anyhow::Result<()> {
        let mut cache = self.price_cache.write().await;
        cache.insert(format!("internal:{}", market_id), price);
        Ok(())
    }
    
    /// Update on-chain oracle with external price
    async fn update_onchain_oracle(&self, market_id: u128, price: &ExternalPrice) -> anyhow::Result<()> {
        // Convert price to on-chain format
        let yes_price = (price.outcome_prices.get(0).unwrap_or(&0.5) * 1e8) as u64;
        let no_price = (price.outcome_prices.get(1).unwrap_or(&0.5) * 1e8) as u64;
        
        // In production, this would submit an oracle update transaction
        debug!(
            "Would update oracle for market {}: yes={}, no={}",
            market_id, yes_price, no_price
        );
        
        Ok(())
    }
    
    /// Get cached price for a market
    pub async fn get_cached_price(&self, key: &str) -> Option<ExternalPrice> {
        let cache = self.price_cache.read().await;
        cache.get(key).cloned()
    }
    
    /// Get all cached prices
    pub async fn get_all_cached_prices(&self) -> HashMap<String, ExternalPrice> {
        let cache = self.price_cache.read().await;
        cache.clone()
    }
    
    /// Get sync status
    pub async fn get_sync_status(&self) -> SyncStatus {
        let status = self.sync_status.lock().await;
        status.clone()
    }
    
    /// Add manual market mapping
    pub async fn add_market_mapping(
        &self,
        internal_id: u128,
        platform: Platform,
        external_id: String,
    ) -> anyhow::Result<()> {
        let mapping = MarketMapping {
            internal_id,
            platform,
            external_id: external_id.clone(),
            last_sync: 0,
            sync_enabled: true,
        };
        
        info!("Added market mapping: {} -> {}:{}", internal_id, platform, &external_id);
        
        let mut mappings = self.market_mappings.write().await;
        mappings.entry(internal_id)
            .or_insert_with(Vec::new)
            .push(mapping);
        Ok(())
    }
    
    /// Toggle market sync
    pub async fn toggle_market_sync(&self, internal_id: u128, enabled: bool) -> anyhow::Result<()> {
        let mut mappings = self.market_mappings.write().await;
        if let Some(maps) = mappings.get_mut(&internal_id) {
            for map in maps.iter_mut() {
                map.sync_enabled = enabled;
            }
            Ok(())
        } else {
            Err(anyhow!("Market mapping not found"))
        }
    }
}

impl Clone for MarketSyncService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            polymarket_client: None, // Don't clone HTTP clients
            kalshi_client: None,
            platform_client: self.platform_client.clone(),
            market_mappings: self.market_mappings.clone(),
            price_cache: self.price_cache.clone(),
            sync_status: self.sync_status.clone(),
        }
    }
}

/// Calculate hash for market title
fn calculate_title_hash(title: &str) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    title.to_lowercase().hash(&mut hasher);
    hasher.finish() as u128
}

/// Generate deterministic market ID
fn generate_market_id(title: &str) -> u128 {
    let hash = calculate_title_hash(title);
    // Add some randomness to avoid collisions
    let timestamp = chrono::Utc::now().timestamp() as u128;
    hash ^ (timestamp << 64)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_title_hash() {
        let hash1 = calculate_title_hash("Will BTC reach $100k?");
        let hash2 = calculate_title_hash("will btc reach $100k?");
        assert_eq!(hash1, hash2); // Case insensitive
    }
    
    #[test]
    fn test_market_id_generation() {
        let id1 = generate_market_id("Test Market 1");
        let id2 = generate_market_id("Test Market 2");
        assert_ne!(id1, id2); // Different due to different titles
    }
}