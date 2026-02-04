//! Compute Unit (CU) tracking for compression operations
//!
//! Tracks and optimizes CU usage for state compression/decompression

use solana_program::{
    clock::Clock,
    sysvar::Sysvar,
    msg,
};
use std::collections::HashMap;

/// CU tracking for compression operations
pub struct CompressionCUTracker {
    /// Operation -> CU cost mapping
    operation_costs: HashMap<String, u32>,
    
    /// Total CU used in current transaction
    total_cu_used: u32,
    
    /// CU budget remaining
    cu_budget: u32,
}

impl CompressionCUTracker {
    /// Create new CU tracker
    pub fn new(cu_budget: u32) -> Self {
        let mut operation_costs = HashMap::new();
        
        // Initialize with known costs from spec
        operation_costs.insert("compression".to_string(), 5000);
        operation_costs.insert("decompression".to_string(), 2000);
        operation_costs.insert("zk_proof_generation".to_string(), 5000);
        operation_costs.insert("zk_proof_verification".to_string(), 2000);
        operation_costs.insert("merkle_path_build".to_string(), 500);
        operation_costs.insert("merkle_path_verify".to_string(), 300);
        operation_costs.insert("pedersen_commit".to_string(), 1000);
        
        Self {
            operation_costs,
            total_cu_used: 0,
            cu_budget,
        }
    }
    
    /// Track CU for an operation
    pub fn track_operation(&mut self, operation: &str) -> Result<(), &'static str> {
        let cost = self.operation_costs
            .get(operation)
            .copied()
            .unwrap_or(100); // Default cost
        
        if self.total_cu_used + cost > self.cu_budget {
            return Err("CU budget exceeded");
        }
        
        self.total_cu_used += cost;
        msg!("Operation {} used {} CU, total: {}", operation, cost, self.total_cu_used);
        
        Ok(())
    }
    
    /// Get remaining CU budget
    pub fn remaining_cu(&self) -> u32 {
        self.cu_budget.saturating_sub(self.total_cu_used)
    }
    
    /// Get CU usage percentage
    pub fn usage_percentage(&self) -> u8 {
        ((self.total_cu_used as f64 / self.cu_budget as f64) * 100.0) as u8
    }
}

/// Hot data cache for frequently accessed compressed data
pub struct HotDataCache {
    /// Cache entries: proposal_id -> (decompressed_data, last_access_slot)
    cache: HashMap<[u8; 32], (Vec<u8>, u64)>,
    
    /// Maximum cache size
    max_entries: usize,
    
    /// Cache hit/miss statistics
    hits: u64,
    misses: u64,
}

impl HotDataCache {
    /// Create new hot data cache
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }
    
    /// Get from cache
    pub fn get(&mut self, proposal_id: &[u8; 32]) -> Option<&Vec<u8>> {
        let current_slot = Clock::get().ok()?.slot;
        
        if let Some((data, last_access)) = self.cache.get_mut(proposal_id) {
            *last_access = current_slot;
            self.hits += 1;
            Some(data)
        } else {
            self.misses += 1;
            None
        }
    }
    
    /// Add to cache
    pub fn put(&mut self, proposal_id: [u8; 32], data: Vec<u8>) {
        let current_slot = Clock::get().ok().map(|c| c.slot).unwrap_or(0);
        
        // Evict oldest if at capacity
        if self.cache.len() >= self.max_entries {
            if let Some(oldest_key) = self.find_oldest_entry() {
                self.cache.remove(&oldest_key);
            }
        }
        
        self.cache.insert(proposal_id, (data, current_slot));
    }
    
    /// Find oldest cache entry
    fn find_oldest_entry(&self) -> Option<[u8; 32]> {
        self.cache
            .iter()
            .min_by_key(|(_, (_, slot))| *slot)
            .map(|(key, _)| *key)
    }
    
    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let hit_rate = if self.hits + self.misses > 0 {
            (self.hits as f64 / (self.hits + self.misses) as f64) * 100.0
        } else {
            0.0
        };
        
        CacheStats {
            entries: self.cache.len(),
            hits: self.hits,
            misses: self.misses,
            hit_rate,
        }
    }
}

/// Cache statistics
pub struct CacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

/// Batch compression optimizer
pub struct BatchOptimizer {
    /// Pending items for batch compression
    pending: Vec<([u8; 32], Vec<u8>)>,
    
    /// Optimal batch size based on CU constraints
    optimal_batch_size: usize,
}

impl BatchOptimizer {
    /// Create new batch optimizer
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            optimal_batch_size: 100, // From spec
        }
    }
    
    /// Add item to batch
    pub fn add_to_batch(&mut self, proposal_id: [u8; 32], data: Vec<u8>) {
        self.pending.push((proposal_id, data));
    }
    
    /// Check if batch is ready
    pub fn is_batch_ready(&self) -> bool {
        self.pending.len() >= self.optimal_batch_size
    }
    
    /// Get batch for compression
    pub fn take_batch(&mut self) -> Vec<([u8; 32], Vec<u8>)> {
        let batch_size = self.optimal_batch_size.min(self.pending.len());
        self.pending.drain(..batch_size).collect()
    }
    
    /// Calculate optimal batch size based on CU budget
    pub fn optimize_batch_size(&mut self, cu_budget: u32, cu_per_item: u32) {
        // Reserve 10% for overhead
        let available_cu = (cu_budget as f64 * 0.9) as u32;
        self.optimal_batch_size = (available_cu / cu_per_item) as usize;
        self.optimal_batch_size = self.optimal_batch_size.clamp(10, 1000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cu_tracker() {
        let mut tracker = CompressionCUTracker::new(10000);
        
        assert!(tracker.track_operation("compression").is_ok());
        assert_eq!(tracker.total_cu_used, 5000);
        assert_eq!(tracker.remaining_cu(), 5000);
        
        assert!(tracker.track_operation("decompression").is_ok());
        assert_eq!(tracker.total_cu_used, 7000);
        
        // Test budget exceeded
        assert!(tracker.track_operation("compression").is_err());
    }
    
    #[test]
    fn test_hot_cache() {
        let mut cache = HotDataCache::new(2);
        
        let id1 = [1u8; 32];
        let id2 = [2u8; 32];
        let id3 = [3u8; 32];
        
        cache.put(id1, vec![1, 2, 3]);
        cache.put(id2, vec![4, 5, 6]);
        
        assert!(cache.get(&id1).is_some());
        assert_eq!(cache.hits, 1);
        
        // Test eviction
        cache.put(id3, vec![7, 8, 9]);
        assert_eq!(cache.cache.len(), 2); // Still only 2 entries
    }
}