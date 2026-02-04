use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};
use crate::state::MarketEssentials;

/// Cache entry for decompressed market data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CacheEntry {
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Decompressed market data
    pub data: MarketEssentials,
    
    /// Timestamp when cached
    pub cached_at: i64,
    
    /// Number of times accessed
    pub access_count: u32,
    
    /// Last access timestamp
    pub last_access: i64,
}

impl CacheEntry {
    pub const SIZE: usize = 32 + // market_id
        MarketEssentials::SIZE + // data
        8 + // cached_at
        4 + // access_count
        8; // last_access
    
    /// Check if cache entry is fresh
    pub fn is_fresh(&self, current_time: i64, max_age: i64) -> bool {
        current_time - self.cached_at < max_age
    }
    
    /// Update access statistics
    pub fn record_access(&mut self, current_time: i64) {
        self.access_count += 1;
        self.last_access = current_time;
    }
}

/// Decompression cache for fast access to frequently used markets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DecompressionCache {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Is initialized
    pub is_initialized: bool,
    
    /// Authority
    pub authority: Pubkey,
    
    /// Maximum number of entries
    pub max_entries: u32,
    
    /// Current cache size
    pub cache_size: u32,
    
    /// Cache hit rate (fixed point 6 decimals)
    pub hit_rate: u64,
    
    /// Total hits
    pub total_hits: u64,
    
    /// Total misses
    pub total_misses: u64,
    
    /// Last cleanup timestamp
    pub last_cleanup: i64,
    
    /// Cache freshness timeout (seconds)
    pub cache_timeout: i64,
    
    /// Cache entries (stored separately in PDA accounts)
    pub entry_count: u32,
}

impl DecompressionCache {
    pub const DISCRIMINATOR: [u8; 8] = [68, 69, 67, 79, 77, 80, 95, 67]; // "DECOMP_C"
    
    pub const LEN: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // authority
        4 + // max_entries
        4 + // cache_size
        8 + // hit_rate
        8 + // total_hits
        8 + // total_misses
        8 + // last_cleanup
        8 + // cache_timeout
        4 + // entry_count
        64; // padding
    
    /// Create default cache configuration
    pub fn default(authority: Pubkey) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            authority,
            max_entries: 1000, // Cache up to 1000 markets
            cache_size: 0,
            hit_rate: 0,
            total_hits: 0,
            total_misses: 0,
            last_cleanup: 0,
            cache_timeout: 60, // 60 seconds cache timeout
            entry_count: 0,
        }
    }
    
    /// Update hit rate statistics
    pub fn update_hit_rate(&mut self) {
        let total = self.total_hits + self.total_misses;
        if total > 0 {
            // Calculate hit rate with 6 decimal precision
            self.hit_rate = (self.total_hits * 1_000_000) / total;
        }
    }
    
    /// Record a cache hit
    pub fn record_hit(&mut self) {
        self.total_hits += 1;
        self.update_hit_rate();
    }
    
    /// Record a cache miss
    pub fn record_miss(&mut self) {
        self.total_misses += 1;
        self.update_hit_rate();
    }
    
    /// Check if cache needs cleanup
    pub fn needs_cleanup(&self, current_time: i64) -> bool {
        current_time - self.last_cleanup > 3600 // Cleanup every hour
    }
    
    /// Validate cache configuration
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check discriminator
        if self.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Check initialized
        if !self.is_initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        
        // Validate max entries
        if self.max_entries == 0 || self.max_entries > 10000 {
            return Err(crate::error::CompressionError::DecompressionCacheFull.into());
        }
        
        Ok(())
    }
}

/// Cache statistics for monitoring
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CacheStats {
    /// Hit rate percentage (0-100)
    pub hit_rate_percent: u8,
    
    /// Average access count per entry
    pub avg_access_count: u32,
    
    /// Number of stale entries
    pub stale_entries: u32,
    
    /// Cache utilization percentage
    pub utilization_percent: u8,
    
    /// Most accessed market IDs (top 5)
    pub hot_markets: [[u8; 32]; 5],
}

impl CacheStats {
    pub const LEN: usize = 1 + // hit_rate_percent
        4 + // avg_access_count
        4 + // stale_entries
        1 + // utilization_percent
        160; // hot_markets (5 * 32)
}