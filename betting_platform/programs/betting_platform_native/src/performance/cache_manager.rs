//! High-Performance Cache Manager
//!
//! Implements caching strategies to reduce redundant computations

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
};

/// Cache entry TTL in slots
pub const CACHE_TTL_SLOTS: u64 = 150; // ~1 minute at 400ms/slot

/// Maximum cache entries
pub const MAX_CACHE_ENTRIES: usize = 1000;

/// Cache discriminator
pub const CACHE_DISCRIMINATOR: [u8; 8] = [67, 65, 67, 72, 69, 77, 71, 82]; // "CACHEMGR"

/// Cache manager state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CacheManager {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Cache version
    pub version: u32,
    
    /// Total cache hits
    pub cache_hits: u64,
    
    /// Total cache misses
    pub cache_misses: u64,
    
    /// Last cleanup slot
    pub last_cleanup: u64,
    
    /// Active entries count
    pub active_entries: u32,
}

impl CacheManager {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 4 + 8 + 8 + 8 + 4;
    
    /// Create new cache manager
    pub fn new() -> Self {
        Self {
            discriminator: CACHE_DISCRIMINATOR,
            version: 1,
            cache_hits: 0,
            cache_misses: 0,
            last_cleanup: 0,
            active_entries: 0,
        }
    }
    
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            (self.cache_hits as f64) / (total as f64)
        }
    }
}

/// Cached computation result
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CachedResult<T: BorshSerialize + BorshDeserialize> {
    /// Result data
    pub data: T,
    
    /// Computation slot
    pub computed_at: u64,
    
    /// Expiry slot
    pub expires_at: u64,
    
    /// Access count
    pub access_count: u32,
}

impl<T: BorshSerialize + BorshDeserialize> CachedResult<T> {
    /// Check if expired
    pub fn is_expired(&self, current_slot: u64) -> bool {
        current_slot > self.expires_at
    }
    
    /// Update access count
    pub fn access(&mut self) {
        self.access_count = self.access_count.saturating_add(1);
    }
}

/// Price cache for market data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceCache {
    /// Market ID -> Price mapping
    pub prices: HashMap<[u8; 32], CachedPrice>,
    
    /// Last update slot
    pub last_update: u64,
}

impl PriceCache {
    /// Get cached price
    pub fn get(&mut self, market_id: &[u8; 32], current_slot: u64) -> Option<&CachedPrice> {
        if let Some(price) = self.prices.get_mut(market_id) {
            if !price.is_expired(current_slot) {
                price.access_count += 1;
                return Some(price);
            }
        }
        None
    }
    
    /// Update price
    pub fn update(&mut self, market_id: [u8; 32], price: u64, current_slot: u64) {
        let cached = CachedPrice {
            price,
            computed_at: current_slot,
            expires_at: current_slot + CACHE_TTL_SLOTS,
            access_count: 0,
        };
        self.prices.insert(market_id, cached);
        self.last_update = current_slot;
    }
    
    /// Clean expired entries
    pub fn cleanup(&mut self, current_slot: u64) {
        self.prices.retain(|_, price| !price.is_expired(current_slot));
    }
}

/// Cached price data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CachedPrice {
    pub price: u64,
    pub computed_at: u64,
    pub expires_at: u64,
    pub access_count: u32,
}

impl CachedPrice {
    pub fn is_expired(&self, current_slot: u64) -> bool {
        current_slot > self.expires_at
    }
}

/// AMM computation cache
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AMMCache {
    /// Cached AMM states
    pub amm_states: HashMap<[u8; 32], CachedAMMState>,
    
    /// Cached price impacts
    pub price_impacts: HashMap<PriceImpactKey, CachedPriceImpact>,
}

/// Cached AMM state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CachedAMMState {
    pub liquidity: u64,
    pub total_shares: u64,
    pub outcome_shares: Vec<u64>,
    pub computed_at: u64,
    pub expires_at: u64,
}

/// Price impact cache key
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct PriceImpactKey {
    pub market_id: [u8; 32],
    pub outcome: u8,
    pub size: u64,
    pub is_buy: bool,
}

/// Cached price impact
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CachedPriceImpact {
    pub impact_bps: u16,
    pub new_price: u64,
    pub computed_at: u64,
    pub expires_at: u64,
}

/// Computation cache for expensive operations
pub struct ComputationCache {
    /// Newton-Raphson results
    pub newton_results: HashMap<NewtonKey, CachedNewtonResult>,
    
    /// Integration results
    pub integration_results: HashMap<IntegrationKey, CachedIntegrationResult>,
    
    /// Stats
    pub stats: CacheStats,
}

impl ComputationCache {
    pub fn new() -> Self {
        Self {
            newton_results: HashMap::new(),
            integration_results: HashMap::new(),
            stats: CacheStats::default(),
        }
    }
    
    /// Get or compute Newton-Raphson result
    pub fn get_or_compute_newton<F>(
        &mut self,
        key: NewtonKey,
        current_slot: u64,
        compute_fn: F,
    ) -> Result<u64, ProgramError>
    where
        F: FnOnce() -> Result<u64, ProgramError>,
    {
        // Check cache
        if let Some(cached) = self.newton_results.get_mut(&key) {
            if !cached.is_expired(current_slot) {
                self.stats.hits += 1;
                cached.access();
                return Ok(cached.result);
            }
        }
        
        // Cache miss - compute
        self.stats.misses += 1;
        let result = compute_fn()?;
        
        // Store in cache
        let cached = CachedNewtonResult {
            result,
            iterations: 0, // Would be set by compute_fn
            computed_at: current_slot,
            expires_at: current_slot + CACHE_TTL_SLOTS,
            access_count: 1,
        };
        self.newton_results.insert(key, cached);
        
        Ok(result)
    }
}

/// Newton-Raphson cache key
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct NewtonKey {
    pub function_type: u8,
    pub initial_value: u64,
    pub target: u64,
}

/// Cached Newton-Raphson result
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CachedNewtonResult {
    pub result: u64,
    pub iterations: u8,
    pub computed_at: u64,
    pub expires_at: u64,
    pub access_count: u32,
}

impl CachedNewtonResult {
    pub fn is_expired(&self, current_slot: u64) -> bool {
        current_slot > self.expires_at
    }
    
    pub fn access(&mut self) {
        self.access_count = self.access_count.saturating_add(1);
    }
}

/// Integration cache key
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct IntegrationKey {
    pub function_type: u8,
    pub lower_bound: i64,
    pub upper_bound: i64,
    pub precision: u8,
}

/// Cached integration result
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CachedIntegrationResult {
    pub result: u64,
    pub computed_at: u64,
    pub expires_at: u64,
}

/// Cache statistics
#[derive(Default, Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_compute_saved: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64) / (total as f64)
        }
    }
}

/// LRU eviction policy
pub fn evict_lru_entries<K, V>(
    cache: &mut HashMap<K, V>,
    max_size: usize,
) where
    K: Eq + std::hash::Hash + Clone,
    V: BorshSerialize + BorshDeserialize,
{
    if cache.len() <= max_size {
        return;
    }
    
    // Simple eviction: remove first N entries
    // In production, would track access times
    let to_remove = cache.len() - max_size;
    let keys: Vec<K> = cache.keys().take(to_remove).cloned().collect();
    
    for key in keys {
        cache.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cache_expiry() {
        let current_slot = 1000;
        let cached = CachedPrice {
            price: 5000,
            computed_at: current_slot,
            expires_at: current_slot + CACHE_TTL_SLOTS,
            access_count: 0,
        };
        
        assert!(!cached.is_expired(current_slot));
        assert!(!cached.is_expired(current_slot + CACHE_TTL_SLOTS - 1));
        assert!(cached.is_expired(current_slot + CACHE_TTL_SLOTS + 1));
    }
    
    #[test]
    fn test_hit_rate() {
        let mut stats = CacheStats::default();
        stats.hits = 80;
        stats.misses = 20;
        
        assert_eq!(stats.hit_rate(), 0.8);
    }
}