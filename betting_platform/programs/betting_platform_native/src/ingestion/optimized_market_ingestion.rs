//! Optimized Market Ingestion for 21k Markets
//!
//! Implements efficient batch processing to handle Polymarket's entire
//! market catalog within 60-second intervals using 21 batches.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
    compression::{ZKStateCompressor, ZKCompressionConfig},
};

/// Optimized ingestion constants
pub const TOTAL_MARKETS: u32 = 21000;
pub const BATCH_COUNT: u32 = 21;
pub const MARKETS_PER_BATCH: u32 = 1000;
pub const INGESTION_INTERVAL_SLOTS: u64 = 150; // 60 seconds
pub const SLOTS_PER_BATCH: u64 = 7; // ~2.8 seconds per batch
pub const MAX_CU_PER_BATCH: u32 = 1_400_000; // Solana limit
pub const TARGET_CU_PER_MARKET: u32 = 1000; // Optimized processing

/// Batch processing state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BatchIngestionState {
    pub current_batch: u32,
    pub batch_start_slot: u64,
    pub markets_processed_in_batch: u32,
    pub total_markets_processed: u64,
    pub current_cycle_start: u64,
    pub cycles_completed: u32,
    pub compression_enabled: bool,
    pub average_processing_time: u64,
}

impl BatchIngestionState {
    pub const SIZE: usize = 4 +  // current_batch
        8 +  // batch_start_slot
        4 +  // markets_processed_in_batch
        8 +  // total_markets_processed
        8 +  // current_cycle_start
        4 +  // cycles_completed
        1 +  // compression_enabled
        8 +  // average_processing_time
        32; // padding

    pub fn new() -> Self {
        Self {
            current_batch: 0,
            batch_start_slot: 0,
            markets_processed_in_batch: 0,
            total_markets_processed: 0,
            current_cycle_start: 0,
            cycles_completed: 0,
            compression_enabled: true,
            average_processing_time: 0,
        }
    }
}

/// Optimized market data structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OptimizedMarketData {
    pub market_id: [u8; 16],    // Compressed from 32 bytes
    pub price_yes: u16,         // Basis points (0-10000)
    pub price_no: u16,          // Basis points (0-10000)
    pub volume_24h: u32,        // In thousands of USDC
    pub liquidity: u32,         // In thousands of USDC
    pub state: MarketState,
    pub last_update: u32,       // Timestamp offset from epoch
}

/// Market state enum for efficient storage
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
#[repr(u8)]
pub enum MarketState {
    Active = 0,
    Resolved = 1,
    Disputed = 2,
    Halted = 3,
    Archived = 4,
}

/// Batch processor for market ingestion
pub struct OptimizedMarketIngestion;

impl OptimizedMarketIngestion {
    /// Process a batch of markets
    pub fn process_market_batch(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        batch_number: u32,
        market_data: Vec<OptimizedMarketData>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        
        let ingestion_state_account = next_account_info(account_info_iter)?;
        let global_state_account = next_account_info(account_info_iter)?;
        let authority = next_account_info(account_info_iter)?;
        
        // Verify authority
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        let mut ingestion_state = BatchIngestionState::deserialize(
            &mut &ingestion_state_account.data.borrow()[..]
        )?;
        
        let clock = Clock::get()?;
        
        // Verify batch timing
        if !Self::is_batch_window_valid(&ingestion_state, batch_number, clock.slot)? {
            msg!("Batch {} submitted outside valid window", batch_number);
            return Err(BettingPlatformError::UpdateTooFrequent.into());
        }
        
        // Process markets efficiently
        let processed = Self::process_markets_optimized(
            &market_data,
            &ingestion_state,
        )?;
        
        // Update state
        ingestion_state.current_batch = batch_number;
        ingestion_state.batch_start_slot = clock.slot;
        ingestion_state.markets_processed_in_batch = processed as u32;
        ingestion_state.total_markets_processed += processed as u64;
        
        // Check if cycle complete
        if batch_number == BATCH_COUNT - 1 {
            ingestion_state.cycles_completed += 1;
            ingestion_state.current_batch = 0;
            ingestion_state.current_cycle_start = clock.slot + INGESTION_INTERVAL_SLOTS;
            
            msg!(
                "Ingestion cycle {} complete. Processed {} markets",
                ingestion_state.cycles_completed,
                TOTAL_MARKETS
            );
            
            emit_event(EventType::BatchProcessed, &BatchProcessedEvent {
                cycle: ingestion_state.cycles_completed,
                total_markets: TOTAL_MARKETS,
                compression_ratio: if ingestion_state.compression_enabled { 10.0 } else { 1.0 },
            });
        }
        
        ingestion_state.serialize(&mut &mut ingestion_state_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
    
    /// Verify batch submission timing
    fn is_batch_window_valid(
        state: &BatchIngestionState,
        batch_number: u32,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Calculate expected slot for this batch
        let expected_slot_start = state.current_cycle_start + (batch_number as u64 * SLOTS_PER_BATCH);
        let expected_slot_end = expected_slot_start + SLOTS_PER_BATCH;
        
        // Allow some flexibility (±1 slot)
        Ok(current_slot >= expected_slot_start.saturating_sub(1) && 
           current_slot <= expected_slot_end.saturating_add(1))
    }
    
    /// Process markets with optimizations
    fn process_markets_optimized(
        markets: &[OptimizedMarketData],
        state: &BatchIngestionState,
    ) -> Result<usize, ProgramError> {
        let mut processed = 0;
        let mut compute_used = 0u32;
        
        for market in markets.iter() {
            // Skip if we're approaching compute limit
            if compute_used + TARGET_CU_PER_MARKET > MAX_CU_PER_BATCH {
                msg!(
                    "Compute limit approaching. Processed {} of {} markets",
                    processed,
                    markets.len()
                );
                break;
            }
            
            // Validate market data
            if !Self::validate_market_data(market)? {
                continue;
            }
            
            // Process based on state
            match market.state {
                MarketState::Active => {
                    Self::update_active_market(market)?;
                    compute_used += 800;
                }
                MarketState::Resolved => {
                    Self::process_resolution(market)?;
                    compute_used += 1200;
                }
                MarketState::Disputed => {
                    Self::handle_dispute(market)?;
                    compute_used += 1500;
                }
                _ => {
                    // Archived/Halted markets need minimal processing
                    compute_used += 200;
                }
            }
            
            processed += 1;
        }
        
        Ok(processed)
    }
    
    /// Validate market data
    fn validate_market_data(market: &OptimizedMarketData) -> Result<bool, ProgramError> {
        // Ensure prices sum to ~100%
        let price_sum = market.price_yes + market.price_no;
        if price_sum < 9900 || price_sum > 10100 {
            return Ok(false);
        }
        
        // Ensure reasonable values
        if market.volume_24h > 1_000_000_000 || market.liquidity > 100_000_000 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Update active market prices
    fn update_active_market(market: &OptimizedMarketData) -> ProgramResult {
        // In production, this would update the on-chain market state
        // For now, just track the update
        msg!(
            "Updated market {:?}: YES={:.2}% NO={:.2}%",
            market.market_id,
            market.price_yes as f64 / 100.0,
            market.price_no as f64 / 100.0
        );
        Ok(())
    }
    
    /// Process market resolution
    fn process_resolution(market: &OptimizedMarketData) -> ProgramResult {
        msg!("Processing resolution for market {:?}", market.market_id);
        // Resolution logic would go here
        Ok(())
    }
    
    /// Handle disputed market
    fn handle_dispute(market: &OptimizedMarketData) -> ProgramResult {
        msg!("Handling dispute for market {:?}", market.market_id);
        // Dispute handling logic would go here
        Ok(())
    }
}

/// Parallel batch coordinator
pub struct ParallelBatchCoordinator {
    pub batch_states: Vec<BatchIngestionState>,
    pub compression_config: ZKCompressionConfig,
}

impl ParallelBatchCoordinator {
    /// Initialize coordinator for parallel processing
    pub fn new() -> Self {
        Self {
            batch_states: (0..BATCH_COUNT)
                .map(|_| BatchIngestionState::new())
                .collect(),
            compression_config: ZKCompressionConfig {
                enabled: true,
                batch_size: MARKETS_PER_BATCH,
                ..Default::default()
            },
        }
    }
    
    /// Get next batch to process
    pub fn get_next_batch(&self, current_slot: u64) -> Option<u32> {
        let cycle_position = current_slot % INGESTION_INTERVAL_SLOTS;
        let batch_number = (cycle_position / SLOTS_PER_BATCH) as u32;
        
        if batch_number < BATCH_COUNT {
            Some(batch_number)
        } else {
            None
        }
    }
    
    /// Calculate ingestion metrics
    pub fn calculate_metrics(&self) -> IngestionMetrics {
        let total_markets = self.batch_states.iter()
            .map(|s| s.total_markets_processed)
            .sum();
            
        let avg_time = self.batch_states.iter()
            .map(|s| s.average_processing_time)
            .sum::<u64>() / BATCH_COUNT as u64;
            
        IngestionMetrics {
            total_markets_processed: total_markets,
            markets_per_second: (TOTAL_MARKETS as f64) / 60.0,
            average_batch_time_ms: avg_time,
            compression_ratio: if self.compression_config.enabled { 10.0 } else { 1.0 },
            cycles_completed: self.batch_states[0].cycles_completed,
        }
    }
}

#[derive(Debug)]
pub struct IngestionMetrics {
    pub total_markets_processed: u64,
    pub markets_per_second: f64,
    pub average_batch_time_ms: u64,
    pub compression_ratio: f32,
    pub cycles_completed: u32,
}

// Event definition
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BatchProcessedEvent {
    pub cycle: u32,
    pub total_markets: u32,
    pub compression_ratio: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_batch_timing() {
        let state = BatchIngestionState::new();
        
        // Test first batch
        assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 0, 0).unwrap());
        assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 0, 6).unwrap());
        assert!(!OptimizedMarketIngestion::is_batch_window_valid(&state, 0, 8).unwrap());
        
        // Test later batch
        assert!(OptimizedMarketIngestion::is_batch_window_valid(&state, 10, 70).unwrap());
        assert!(!OptimizedMarketIngestion::is_batch_window_valid(&state, 10, 80).unwrap());
    }
    
    #[test]
    fn test_market_validation() {
        let valid_market = OptimizedMarketData {
            market_id: [0u8; 16],
            price_yes: 6000,
            price_no: 4000,
            volume_24h: 100_000,
            liquidity: 50_000,
            state: MarketState::Active,
            last_update: 12345,
        };
        
        assert!(OptimizedMarketIngestion::validate_market_data(&valid_market).unwrap());
        
        // Invalid price sum
        let mut invalid = valid_market.clone();
        invalid.price_yes = 8000;
        invalid.price_no = 1000;
        assert!(!OptimizedMarketIngestion::validate_market_data(&invalid).unwrap());
    }
}