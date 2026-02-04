//! Cache Layer Optimization
//!
//! Production-grade caching to minimize repeated computations and account loads

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, VecDeque};

use crate::{
    error::BettingPlatformError,
    state::{ProposalPDA, Position, GlobalConfigPDA},
    math::U64F64,
};

/// Cache configuration
pub const PRICE_CACHE_SIZE: usize = 1024;
pub const POSITION_CACHE_SIZE: usize = 512;
pub const COMPUTATION_CACHE_SIZE: usize = 256;
pub const CACHE_TTL_SLOTS: u64 = 10; // Cache validity in slots

/// Cache entry with timestamp
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub slot: u64,
    pub hits: u32,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T, slot: u64) -> Self {
        Self {
            value,
            slot,
            hits: 0,
        }
    }
    
    pub fn is_valid(&self, current_slot: u64) -> bool {
        current_slot.saturating_sub(self.slot) <= CACHE_TTL_SLOTS
    }
    
    pub fn hit(&mut self) {
        self.hits = self.hits.saturating_add(1);
    }
}

/// LRU cache implementation
pub struct LRUCache<K: Eq + std::hash::Hash + Clone, V: Clone> {
    capacity: usize,
    map: HashMap<K, CacheEntry<V>>,
    order: VecDeque<K>,
    hits: u64,
    misses: u64,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> LRUCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            hits: 0,
            misses: 0,
        }
    }
    
    /// Get value from cache
    pub fn get(&mut self, key: &K, current_slot: u64) -> Option<&V> {
        // Check if entry exists and is valid
        if let Some(entry) = self.map.get(key) {
            if entry.is_valid(current_slot) {
                // Entry is valid, update hit count and LRU order
                if let Some(entry) = self.map.get_mut(key) {
                    entry.hit();
                    self.hits += 1;
                }
                
                // Move to front
                if let Some(pos) = self.order.iter().position(|k| k == key) {
                    self.order.remove(pos);
                    self.order.push_front(key.clone());
                }
                
                // Return reference to the value
                return self.map.get(key).map(|e| &e.value);
            } else {
                // Entry is expired, remove it
                self.remove(key);
            }
        }
        
        self.misses += 1;
        None
    }
    
    /// Put value in cache
    pub fn put(&mut self, key: K, value: V, current_slot: u64) {
        // Remove if exists
        if self.map.contains_key(&key) {
            self.remove(&key);
        }
        
        // Evict if at capacity
        if self.map.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_back() {
                self.map.remove(&oldest);
            }
        }
        
        // Insert new entry
        self.map.insert(key.clone(), CacheEntry::new(value, current_slot));
        self.order.push_front(key);
    }
    
    /// Remove entry
    fn remove(&mut self, key: &K) {
        self.map.remove(key);
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
    }
    
    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let hit_rate = if self.hits + self.misses > 0 {
            (self.hits as f64) / ((self.hits + self.misses) as f64)
        } else {
            0.0
        };
        
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            hit_rate,
            size: self.map.len(),
            capacity: self.capacity,
        }
    }
    
    /// Clear expired entries
    pub fn evict_expired(&mut self, current_slot: u64) {
        let expired_keys: Vec<K> = self.map
            .iter()
            .filter(|(_, entry)| !entry.is_valid(current_slot))
            .map(|(k, _)| k.clone())
            .collect();
        
        for key in expired_keys {
            self.remove(&key);
        }
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub size: usize,
    pub capacity: usize,
}

/// Price cache for AMM calculations
pub struct PriceCache {
    cache: LRUCache<PriceCacheKey, CachedPrice>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PriceCacheKey {
    pub proposal_id: u64,
    pub outcome: u8,
}

#[derive(Debug, Clone)]
pub struct CachedPrice {
    pub price: u64,
    pub total_liquidity: u64,
    pub outcome_balance: u64,
}

impl PriceCache {
    pub fn new() -> Self {
        Self {
            cache: LRUCache::new(PRICE_CACHE_SIZE),
        }
    }
    
    /// Get cached price
    pub fn get_price(
        &mut self,
        proposal_id: u64,
        outcome: u8,
        current_slot: u64,
    ) -> Option<&CachedPrice> {
        let key = PriceCacheKey { proposal_id, outcome };
        self.cache.get(&key, current_slot)
    }
    
    /// Cache price calculation
    pub fn cache_price(
        &mut self,
        proposal_id: u64,
        outcome: u8,
        price: u64,
        total_liquidity: u64,
        outcome_balance: u64,
        current_slot: u64,
    ) {
        let key = PriceCacheKey { proposal_id, outcome };
        let cached = CachedPrice {
            price,
            total_liquidity,
            outcome_balance,
        };
        self.cache.put(key, cached, current_slot);
    }
    
    /// Invalidate prices for a proposal
    pub fn invalidate_proposal(&mut self, proposal_id: u64) {
        // Remove all outcomes for this proposal
        for outcome in 0..16 {
            let key = PriceCacheKey { proposal_id, outcome };
            self.cache.remove(&key);
        }
    }
}

/// Position cache for fast lookups
pub struct PositionCache {
    cache: LRUCache<[u8; 32], CachedPosition>,
}

#[derive(Debug, Clone)]
pub struct CachedPosition {
    pub user: Pubkey,
    pub size: u64,
    pub leverage: u8,
    pub is_closed: bool,
    pub liquidation_price: u64,
    pub unrealized_pnl: i64,
}

impl PositionCache {
    pub fn new() -> Self {
        Self {
            cache: LRUCache::new(POSITION_CACHE_SIZE),
        }
    }
    
    /// Cache position data
    pub fn cache_position(&mut self, position: &Position, current_slot: u64) {
        let cached = CachedPosition {
            user: position.user,
            size: position.size,
            leverage: position.leverage as u8,
            is_closed: position.is_closed,
            liquidation_price: position.liquidation_price,
            unrealized_pnl: position.unrealized_pnl,
        };
        self.cache.put(position.position_id, cached, current_slot);
    }
    
    /// Get cached position
    pub fn get_position(
        &mut self,
        position_id: &[u8; 32],
        current_slot: u64,
    ) -> Option<&CachedPosition> {
        self.cache.get(position_id, current_slot)
    }
    
    /// Batch cache positions
    pub fn batch_cache(&mut self, positions: &[Position], current_slot: u64) {
        for position in positions {
            self.cache_position(position, current_slot);
        }
    }
}

/// Computation cache for expensive calculations
pub struct ComputationCache {
    cache: LRUCache<String, ComputationResult>,
}

#[derive(Debug, Clone)]
pub struct ComputationResult {
    pub value: Vec<u8>,
    pub compute_units: u64,
}

impl ComputationCache {
    pub fn new() -> Self {
        Self {
            cache: LRUCache::new(COMPUTATION_CACHE_SIZE),
        }
    }
    
    /// Get cached computation
    pub fn get(&mut self, key: &str, current_slot: u64) -> Option<&ComputationResult> {
        self.cache.get(&key.to_string(), current_slot)
    }
    
    /// Cache computation result
    pub fn put(
        &mut self,
        key: String,
        value: Vec<u8>,
        compute_units: u64,
        current_slot: u64,
    ) {
        let result = ComputationResult {
            value,
            compute_units,
        };
        self.cache.put(key, result, current_slot);
    }
    
    /// Generate cache key for liquidation check
    pub fn liquidation_key(position_id: &[u8; 32], price: u64) -> String {
        // Convert first 8 bytes to u64 for simpler key
        let id_prefix = u64::from_le_bytes(position_id[..8].try_into().unwrap());
        format!("liq_{}_{}", id_prefix, price)
    }
    
    /// Generate cache key for price calculation
    pub fn price_calc_key(balances: &[u64], b_value: u64) -> String {
        let balance_str = balances.iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join("_");
        format!("price_{}_{}", balance_str, b_value)
    }
}

/// Unified cache manager
pub struct CacheManager {
    pub price_cache: PriceCache,
    pub position_cache: PositionCache,
    pub computation_cache: ComputationCache,
    pub global_config_cache: Option<(GlobalConfigPDA, u64)>,
    pub last_maintenance_slot: u64,
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            price_cache: PriceCache::new(),
            position_cache: PositionCache::new(),
            computation_cache: ComputationCache::new(),
            global_config_cache: None,
            last_maintenance_slot: 0,
        }
    }
    
    /// Get or load global config with caching
    pub fn get_global_config(
        &mut self,
        account: &AccountInfo,
        current_slot: u64,
    ) -> Result<GlobalConfigPDA, ProgramError> {
        // Check cache
        if let Some((ref config, cached_slot)) = self.global_config_cache {
            if current_slot.saturating_sub(cached_slot) <= CACHE_TTL_SLOTS {
                return Ok(config.clone());
            }
        }
        
        // Load from account
        let config = GlobalConfigPDA::try_from_slice(&account.data.borrow())?;
        self.global_config_cache = Some((config.clone(), current_slot));
        
        Ok(config)
    }
    
    /// Perform cache maintenance
    pub fn maintenance(&mut self, current_slot: u64) {
        // Run maintenance every 100 slots
        if current_slot.saturating_sub(self.last_maintenance_slot) < 100 {
            return;
        }
        
        self.price_cache.cache.evict_expired(current_slot);
        self.position_cache.cache.evict_expired(current_slot);
        self.computation_cache.cache.evict_expired(current_slot);
        
        self.last_maintenance_slot = current_slot;
        
        msg!("Cache maintenance completed at slot {}", current_slot);
    }
    
    /// Get cache statistics
    pub fn get_stats(&self) -> CacheManagerStats {
        CacheManagerStats {
            price_cache_stats: self.price_cache.cache.stats(),
            position_cache_stats: self.position_cache.cache.stats(),
            computation_cache_stats: self.computation_cache.cache.stats(),
            global_config_cached: self.global_config_cache.is_some(),
        }
    }
    
    /// Clear all caches
    pub fn clear_all(&mut self) {
        self.price_cache = PriceCache::new();
        self.position_cache = PositionCache::new();
        self.computation_cache = ComputationCache::new();
        self.global_config_cache = None;
    }
}

#[derive(Debug)]
pub struct CacheManagerStats {
    pub price_cache_stats: CacheStats,
    pub position_cache_stats: CacheStats,
    pub computation_cache_stats: CacheStats,
    pub global_config_cached: bool,
}

/// Cache-aware AMM calculator
pub struct CachedAMMCalculator<'a> {
    cache_manager: &'a mut CacheManager,
}

impl<'a> CachedAMMCalculator<'a> {
    pub fn new(cache_manager: &'a mut CacheManager) -> Self {
        Self { cache_manager }
    }
    
    /// Calculate price with caching
    pub fn calculate_price(
        &mut self,
        proposal: &ProposalPDA,
        outcome: u8,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        // Check cache first
        if let Some(cached) = self.cache_manager.price_cache.get_price(
            u64::from_le_bytes(proposal.proposal_id[..8].try_into().unwrap()),
            outcome,
            current_slot,
        ) {
            return Ok(cached.price);
        }
        
        // Calculate if not cached
        let total_balance: u64 = proposal.outcome_balances.iter().sum();
        let outcome_balance = proposal.outcome_balances[outcome as usize];
        
        // Check computation cache for complex calculation
        let calc_key = ComputationCache::price_calc_key(
            &proposal.outcome_balances,
            proposal.b_value,
        );
        
        let price = if let Some(cached_result) = self.cache_manager.computation_cache.get(
            &calc_key,
            current_slot,
        ) {
            // Deserialize cached price
            u64::from_le_bytes(cached_result.value[..8].try_into().unwrap())
        } else {
            // Perform calculation
            let price = calculate_lmsr_price(outcome_balance, total_balance, proposal.b_value)?;
            
            // Cache computation
            self.cache_manager.computation_cache.put(
                calc_key,
                price.to_le_bytes().to_vec(),
                1000, // Estimated compute units
                current_slot,
            );
            
            price
        };
        
        // Cache the result
        self.cache_manager.price_cache.cache_price(
            u64::from_le_bytes(proposal.proposal_id[..8].try_into().unwrap()),
            outcome,
            price,
            total_balance,
            outcome_balance,
            current_slot,
        );
        
        Ok(price)
    }
}

/// Simplified LMSR price calculation
fn calculate_lmsr_price(
    outcome_balance: u64,
    total_balance: u64,
    b_value: u64,
) -> Result<u64, ProgramError> {
    if total_balance == 0 {
        return Err(BettingPlatformError::InvalidAMMState.into());
    }
    
    // Simplified calculation
    let ratio = (outcome_balance as u128 * 1_000_000) / total_balance as u128;
    Ok((ratio as u64).min(1_000_000))
}

/// Module for cache initialization
pub mod cache_init {
    use super::*;
    
    /// Create a new cache manager instance
    pub fn create_cache_manager() -> CacheManager {
        CacheManager::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lru_cache() {
        let mut cache: LRUCache<String, u64> = LRUCache::new(3);
        let current_slot = 100;
        
        cache.put("a".to_string(), 1, current_slot);
        cache.put("b".to_string(), 2, current_slot);
        cache.put("c".to_string(), 3, current_slot);
        
        assert_eq!(cache.get(&"a".to_string(), current_slot), Some(&1));
        
        // This should evict "b"
        cache.put("d".to_string(), 4, current_slot);
        
        assert_eq!(cache.get(&"b".to_string(), current_slot), None);
        assert_eq!(cache.get(&"d".to_string(), current_slot), Some(&4));
        
        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
    }
    
    #[test]
    fn test_cache_expiration() {
        let mut cache: LRUCache<String, u64> = LRUCache::new(10);
        
        cache.put("test".to_string(), 42, 100);
        
        // Valid within TTL
        assert_eq!(cache.get(&"test".to_string(), 105), Some(&42));
        
        // Expired after TTL
        assert_eq!(cache.get(&"test".to_string(), 111), None);
    }
    
    #[test]
    fn test_price_cache() {
        let mut price_cache = PriceCache::new();
        let current_slot = 1000;
        
        price_cache.cache_price(1, 0, 525_000, 100_000_000, 50_000_000, current_slot);
        
        let cached = price_cache.get_price(1, 0, current_slot).unwrap();
        assert_eq!(cached.price, 525_000);
        assert_eq!(cached.total_liquidity, 100_000_000);
    }
}