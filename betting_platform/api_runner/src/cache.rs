//! Redis caching layer for improved performance

use anyhow::Result;
use redis::{AsyncCommands, Client, Connection};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub redis_url: String,
    pub default_ttl: u64, // seconds
    pub connection_timeout: u64, // seconds
    pub retry_attempts: u32,
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            default_ttl: 300, // 5 minutes
            connection_timeout: 5,
            retry_attempts: 3,
            enabled: true,
        }
    }
}

/// Cache service with Redis backend
pub struct CacheService {
    client: Option<Client>,
    config: CacheConfig,
    connection_pool: Arc<RwLock<Vec<redis::aio::Connection>>>,
    stats: Arc<RwLock<CacheStats>>,
}

/// Cache statistics
#[derive(Debug, Default, Clone, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub errors: u64,
    pub sets: u64,
    pub deletes: u64,
    pub hit_rate: f64,
}

impl CacheStats {
    pub fn calculate_hit_rate(&mut self) {
        let total = self.hits + self.misses;
        self.hit_rate = if total > 0 {
            self.hits as f64 / total as f64
        } else {
            0.0
        };
    }
}

impl CacheService {
    /// Create new cache service
    pub async fn new(config: CacheConfig) -> Result<Self> {
        if !config.enabled {
            info!("Cache service disabled");
            return Ok(Self {
                client: None,
                config,
                connection_pool: Arc::new(RwLock::new(Vec::new())),
                stats: Arc::new(RwLock::new(CacheStats::default())),
            });
        }

        info!("Connecting to Redis at: {}", config.redis_url);
        
        let client = match Client::open(config.redis_url.clone()) {
            Ok(client) => {
                // Test connection
                match client.get_async_connection().await {
                    Ok(_) => {
                        info!("Successfully connected to Redis");
                        Some(client)
                    }
                    Err(e) => {
                        warn!("Failed to connect to Redis: {}. Cache disabled.", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create Redis client: {}. Cache disabled.", e);
                None
            }
        };

        Ok(Self {
            client,
            config,
            connection_pool: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        })
    }

    /// Get connection from pool or create new one
    async fn get_connection(&self) -> Result<redis::aio::Connection> {
        if let Some(client) = &self.client {
            // Try to get from pool first
            {
                let mut pool = self.connection_pool.write().await;
                if let Some(conn) = pool.pop() {
                    return Ok(conn);
                }
            }

            // Create new connection
            match client.get_async_connection().await {
                Ok(conn) => Ok(conn),
                Err(e) => {
                    error!("Failed to get Redis connection: {}", e);
                    Err(e.into())
                }
            }
        } else {
            Err(anyhow::anyhow!("Redis client not available"))
        }
    }

    /// Return connection to pool
    async fn return_connection(&self, conn: redis::aio::Connection) {
        let mut pool = self.connection_pool.write().await;
        if pool.len() < 10 { // Max 10 connections in pool
            pool.push(conn);
        }
        // Drop excess connections to prevent memory leak
    }

    /// Get value from cache
    pub async fn get<T>(&self, key: &str) -> Option<T>
    where
        T: DeserializeOwned,
    {
        if self.client.is_none() {
            return None;
        }

        debug!("Cache GET: {}", key);

        match self.get_connection().await {
            Ok(mut conn) => {
                match conn.get::<_, String>(key).await {
                    Ok(data) => {
                        match serde_json::from_str::<T>(&data) {
                            Ok(value) => {
                                self.increment_hits().await;
                                debug!("Cache HIT: {}", key);
                                self.return_connection(conn).await;
                                Some(value)
                            }
                            Err(e) => {
                                warn!("Failed to deserialize cached value for key {}: {}", key, e);
                                self.increment_errors().await;
                                self.return_connection(conn).await;
                                None
                            }
                        }
                    }
                    Err(_) => {
                        self.increment_misses().await;
                        debug!("Cache MISS: {}", key);
                        self.return_connection(conn).await;
                        None
                    }
                }
            }
            Err(e) => {
                error!("Failed to get cache connection: {}", e);
                self.increment_errors().await;
                None
            }
        }
    }

    /// Set value in cache
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<u64>) -> Result<()>
    where
        T: Serialize,
    {
        if self.client.is_none() {
            return Ok(());
        }

        debug!("Cache SET: {} (TTL: {:?})", key, ttl);

        let data = serde_json::to_string(value)?;
        let ttl = ttl.unwrap_or(self.config.default_ttl);

        match self.get_connection().await {
            Ok(mut conn) => {
                match conn.set_ex::<_, _, ()>(key, data, ttl).await {
                    Ok(_) => {
                        self.increment_sets().await;
                        debug!("Cache SET successful: {}", key);
                        self.return_connection(conn).await;
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to set cache value for key {}: {}", key, e);
                        self.increment_errors().await;
                        self.return_connection(conn).await;
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Failed to get cache connection: {}", e);
                self.increment_errors().await;
                Err(e)
            }
        }
    }

    /// Delete value from cache
    pub async fn delete(&self, key: &str) -> Result<()> {
        if self.client.is_none() {
            return Ok(());
        }

        debug!("Cache DELETE: {}", key);

        match self.get_connection().await {
            Ok(mut conn) => {
                match conn.del::<_, ()>(key).await {
                    Ok(_) => {
                        self.increment_deletes().await;
                        debug!("Cache DELETE successful: {}", key);
                        self.return_connection(conn).await;
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to delete cache key {}: {}", key, e);
                        self.increment_errors().await;
                        self.return_connection(conn).await;
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Failed to get cache connection: {}", e);
                self.increment_errors().await;
                Err(e)
            }
        }
    }

    /// Check if key exists in cache
    pub async fn exists(&self, key: &str) -> bool {
        if self.client.is_none() {
            return false;
        }

        match self.get_connection().await {
            Ok(mut conn) => {
                match conn.exists::<_, bool>(key).await {
                    Ok(exists) => {
                        self.return_connection(conn).await;
                        exists
                    }
                    Err(_) => {
                        self.return_connection(conn).await;
                        false
                    }
                }
            }
            Err(_) => false,
        }
    }

    /// Increment TTL for existing key
    pub async fn expire(&self, key: &str, ttl: u64) -> Result<()> {
        if self.client.is_none() {
            return Ok(());
        }

        match self.get_connection().await {
            Ok(mut conn) => {
                match conn.expire::<_, ()>(key, ttl as i64).await {
                    Ok(_) => {
                        self.return_connection(conn).await;
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to set expiry for key {}: {}", key, e);
                        self.return_connection(conn).await;
                        Err(e.into())
                    }
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let mut stats = self.stats.read().await.clone();
        stats.calculate_hit_rate();
        stats
    }

    /// Clear all cache statistics
    pub async fn clear_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = CacheStats::default();
    }

    /// Health check
    pub async fn health_check(&self) -> bool {
        if self.client.is_none() {
            return false;
        }

        match self.get_connection().await {
            Ok(mut conn) => {
                match conn.set::<_, _, ()>("__ping__", "pong").await {
                    Ok(_) => {
                        self.return_connection(conn).await;
                        true
                    }
                    Err(_) => {
                        self.return_connection(conn).await;
                        false
                    }
                }
            }
            Err(_) => false,
        }
    }

    // Statistics methods
    async fn increment_hits(&self) {
        let mut stats = self.stats.write().await;
        stats.hits += 1;
    }

    async fn increment_misses(&self) {
        let mut stats = self.stats.write().await;
        stats.misses += 1;
    }

    async fn increment_errors(&self) {
        let mut stats = self.stats.write().await;
        stats.errors += 1;
    }

    async fn increment_sets(&self) {
        let mut stats = self.stats.write().await;
        stats.sets += 1;
    }

    async fn increment_deletes(&self) {
        let mut stats = self.stats.write().await;
        stats.deletes += 1;
    }
}

/// Cache key builder helper
pub struct CacheKey;

impl CacheKey {
    pub fn market(market_id: u128) -> String {
        format!("market:{}", market_id)
    }

    pub fn markets_list() -> String {
        "markets:list".to_string()
    }

    pub fn wallet_balance(wallet: &str) -> String {
        format!("wallet:balance:{}", wallet)
    }

    pub fn user_positions(wallet: &str) -> String {
        format!("positions:{}", wallet)
    }

    pub fn portfolio(wallet: &str) -> String {
        format!("portfolio:{}", wallet)
    }

    pub fn risk_metrics(wallet: &str) -> String {
        format!("risk:{}", wallet)
    }

    pub fn verses_list() -> String {
        "verses:list".to_string()
    }

    pub fn verse_matches(query: &str) -> String {
        format!("verse:matches:{}", query.replace(" ", "_").to_lowercase())
    }

    pub fn external_markets(source: &str) -> String {
        format!("external:{}:markets", source)
    }

    pub fn quantum_positions(wallet: &str) -> String {
        format!("quantum:positions:{}", wallet)
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cache_key_generation() {
        assert_eq!(CacheKey::market(123), "market:123");
        assert_eq!(CacheKey::wallet_balance("test_wallet"), "wallet:balance:test_wallet");
        assert_eq!(CacheKey::verse_matches("political markets"), "verse:matches:political_markets");
    }

    #[tokio::test]
    async fn test_cache_service_disabled() {
        let config = CacheConfig {
            enabled: false,
            ..Default::default()
        };
        
        let cache = CacheService::new(config).await.unwrap();
        assert!(cache.client.is_none());
        
        // Operations should work but do nothing
        let result = cache.set("test", &json!({"test": "value"}), None).await;
        assert!(result.is_ok());
        
        let value: Option<serde_json::Value> = cache.get("test").await;
        assert!(value.is_none());
    }
}