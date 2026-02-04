use solana_program::{
    msg,
    clock::Clock,
    program_error::ProgramError,
    pubkey::Pubkey,
    account_info::AccountInfo,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::CompressionError,
    state::{
        CompressionConfig,
        CompressedStateProof,
        DecompressionCache,
        CacheEntry,
        MarketEssentials,
        MarketData,
    },
    compression::engine::StateCompressionEngine,
};

/// Handles efficient access to compressed state with caching
pub struct CompressedStateAccess;

impl CompressedStateAccess {
    /// Read compressed market with caching
    pub fn read_compressed_market(
        market_id: &[u8; 32],
        cache: &mut DecompressionCache,
        proof: &CompressedStateProof,
        config: &CompressionConfig,
        clock: &Clock,
    ) -> Result<MarketData, ProgramError> {
        // Check cache first
        if let Ok(cached_data) = Self::get_from_cache(market_id, cache, clock.unix_timestamp) {
            cache.record_hit();
            msg!("Cache hit for market {:?}", market_id);
            return Ok(MarketData::from_essentials(&cached_data));
        }
        
        cache.record_miss();
        msg!("Cache miss for market {:?}, decompressing...", market_id);
        
        // Decompress from proof (costs ~2k CU)
        let essentials = StateCompressionEngine::decompress_and_verify(
            proof,
            market_id,
            config,
        )?;
        
        // Add to cache
        Self::add_to_cache(market_id, &essentials, cache, clock.unix_timestamp)?;
        
        // Convert to full market data
        Ok(MarketData::from_essentials(&essentials))
    }
    
    /// Batch read for efficiency
    pub fn batch_read_compressed(
        market_ids: &[[u8; 32]],
        cache: &mut DecompressionCache,
        proofs: &[&CompressedStateProof],
        config: &CompressionConfig,
        clock: &Clock,
    ) -> Result<Vec<MarketData>, ProgramError> {
        let mut results = Vec::new();
        let mut cache_hits = 0;
        let mut cache_misses = 0;
        
        for market_id in market_ids {
            // Try cache first
            if let Ok(cached_data) = Self::get_from_cache(market_id, cache, clock.unix_timestamp) {
                results.push(MarketData::from_essentials(&cached_data));
                cache_hits += 1;
                continue;
            }
            
            cache_misses += 1;
            
            // Find proof containing this market
            let proof = Self::find_proof_for_market(market_id, proofs)?
                .ok_or(CompressionError::MarketNotInCompressedState)?;
            
            // Decompress
            let essentials = StateCompressionEngine::decompress_and_verify(
                proof,
                market_id,
                config,
            )?;
            
            // Add to cache
            Self::add_to_cache(market_id, &essentials, cache, clock.unix_timestamp)?;
            
            results.push(MarketData::from_essentials(&essentials));
        }
        
        // Update cache statistics
        cache.total_hits += cache_hits;
        cache.total_misses += cache_misses;
        cache.update_hit_rate();
        
        msg!("Batch read: {} hits, {} misses", cache_hits, cache_misses);
        
        Ok(results)
    }
    
    /// Update compressed market
    pub fn update_compressed_market(
        market_id: &[u8; 32],
        update_fn: impl FnOnce(&mut MarketEssentials) -> Result<(), ProgramError>,
        cache: &mut DecompressionCache,
        proof: &CompressedStateProof,
        config: &CompressionConfig,
        clock: &Clock,
    ) -> Result<MarketEssentials, ProgramError> {
        // Decompress current state
        let mut essentials = StateCompressionEngine::decompress_and_verify(
            proof,
            market_id,
            config,
        )?;
        
        // Apply update
        update_fn(&mut essentials)?;
        
        // Update timestamp
        essentials.last_update = clock.unix_timestamp;
        
        // Invalidate cache entry
        Self::invalidate_cache_entry(market_id, cache)?;
        
        msg!("Updated compressed market {:?}", market_id);
        
        Ok(essentials)
    }
    
    /// Get market from cache
    fn get_from_cache(
        market_id: &[u8; 32],
        cache: &DecompressionCache,
        current_time: i64,
    ) -> Result<MarketEssentials, ProgramError> {
        // In production, would look up in separate cache entry PDA
        // For now, return not found
        Err(CompressionError::StaleCacheEntry.into())
    }
    
    /// Add market to cache
    fn add_to_cache(
        market_id: &[u8; 32],
        essentials: &MarketEssentials,
        cache: &mut DecompressionCache,
        current_time: i64,
    ) -> Result<(), ProgramError> {
        // Check cache capacity
        if cache.entry_count >= cache.max_entries {
            // Need to evict old entries
            msg!("Cache full, need to evict entries");
            return Err(CompressionError::DecompressionCacheFull.into());
        }
        
        // In production, would create cache entry PDA
        cache.entry_count += 1;
        cache.cache_size += CacheEntry::SIZE as u32;
        
        Ok(())
    }
    
    /// Invalidate cache entry
    fn invalidate_cache_entry(
        market_id: &[u8; 32],
        cache: &mut DecompressionCache,
    ) -> Result<(), ProgramError> {
        // In production, would mark cache entry as stale
        msg!("Invalidated cache entry for market {:?}", market_id);
        Ok(())
    }
    
    /// Find proof containing a specific market
    fn find_proof_for_market<'a>(
        market_id: &[u8; 32],
        proofs: &[&'a CompressedStateProof],
    ) -> Result<Option<&'a CompressedStateProof>, ProgramError> {
        for proof in proofs {
            if proof.contains_market_sample(market_id) {
                return Ok(Some(proof));
            }
        }
        
        // In production, would use proof index
        Ok(None)
    }
    
    /// Clean up stale cache entries
    pub fn cleanup_cache(
        cache: &mut DecompressionCache,
        current_time: i64,
    ) -> Result<u32, ProgramError> {
        if !cache.needs_cleanup(current_time) {
            return Ok(0);
        }
        
        // In production, would iterate through cache entries and remove stale ones
        let cleaned = 0u32;
        
        cache.last_cleanup = current_time;
        
        msg!("Cleaned {} stale cache entries", cleaned);
        Ok(cleaned)
    }
    
    /// Get cache statistics
    pub fn get_cache_stats(cache: &DecompressionCache) -> CacheStats {
        let hit_rate_percent = if cache.hit_rate > 0 {
            ((cache.hit_rate as f64 / 1_000_000.0) * 100.0) as u8
        } else {
            0
        };
        
        let utilization_percent = if cache.max_entries > 0 {
            ((cache.entry_count as f64 / cache.max_entries as f64) * 100.0) as u8
        } else {
            0
        };
        
        CacheStats {
            hit_rate_percent,
            utilization_percent,
            total_hits: cache.total_hits,
            total_misses: cache.total_misses,
            entry_count: cache.entry_count,
            cache_size_bytes: cache.cache_size,
        }
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub hit_rate_percent: u8,
    pub utilization_percent: u8,
    pub total_hits: u64,
    pub total_misses: u64,
    pub entry_count: u32,
    pub cache_size_bytes: u32,
}

/// Market update operations
pub enum MarketUpdate {
    Price(u64),
    Volume(u64),
    Status(crate::state::MarketStatus),
}

impl MarketUpdate {
    /// Apply update to market essentials
    pub fn apply(&self, essentials: &mut MarketEssentials) -> Result<(), ProgramError> {
        match self {
            MarketUpdate::Price(new_price) => {
                essentials.current_price = *new_price;
            }
            MarketUpdate::Volume(additional) => {
                essentials.total_volume = essentials.total_volume
                    .checked_add(*additional)
                    .ok_or(CompressionError::ArithmeticOverflow)?;
            }
            MarketUpdate::Status(new_status) => {
                essentials.status = *new_status;
            }
        }
        Ok(())
    }
}

/// Recompression queue entry
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct RecompressionEntry {
    pub market_id: [u8; 32],
    pub updated_data: MarketEssentials,
    pub priority: Priority,
    pub queued_at: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum Priority {
    High,
    Normal,
    Low,
}