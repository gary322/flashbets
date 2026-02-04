//! Ingestor keeper system
//!
//! Fetches market data from Polymarket and updates on-chain state

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    events::{Event, PriceUpdateProcessed},
    math::U64F64,
    state::{IngestorState, ProposalPDA, VersePDA},
    verse_classification::VerseClassifier,
};
use solana_program::pubkey::Pubkey;

/// Maximum markets per batch
pub const MAX_BATCH_SIZE: u32 = 1000;

/// Backoff multiplier for errors
pub const BACKOFF_MULTIPLIER: i64 = 10;

/// Polymarket API response
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketMarket {
    pub id: [u8; 32],
    pub title: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub yes_price: u64,    // In basis points (0-10000)
    pub no_price: u64,     // In basis points (0-10000)
    pub volume_24h: u64,
    pub liquidity: u64,
    pub created_at: i64,
    pub resolved: bool,
    pub resolution: Option<u8>,
}

/// Batch update instruction
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BatchUpdate {
    pub market_id: [u8; 32],
    pub prices: Vec<u64>,
    pub volume: u64,
    pub liquidity: u64,
    pub verse_id: u128,
}

/// Pagination state for large market fetches
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PaginationState {
    pub current_offset: u32,
    pub total_markets: u32,
    pub last_fetch_slot: u64,
    pub batch_size: u32,
}

impl PaginationState {
    pub fn new() -> Self {
        Self {
            current_offset: 0,
            total_markets: 21300, // Spec: ~21,300 markets
            last_fetch_slot: 0,
            batch_size: 1000, // Spec: limit=1000 per API call
        }
    }
    
    pub fn next_batch(&mut self) -> Option<(u32, u32)> {
        if self.current_offset >= self.total_markets {
            self.current_offset = 0; // Reset for next cycle
            return None;
        }
        
        let start = self.current_offset;
        let end = (start + self.batch_size).min(self.total_markets);
        self.current_offset = end;
        
        Some((start, end))
    }
}

/// Ingestor keeper implementation
pub struct IngestorKeeper;

impl IngestorKeeper {
    /// Process a batch of markets from Polymarket
    pub fn ingest_batch(
        ingestor_state: &mut IngestorState,
        markets: Vec<PolymarketMarket>,
        proposals: &mut [ProposalPDA],
        verses: &mut [VersePDA],
    ) -> ProgramResult {
        let clock = Clock::get()?;
        
        // Check if in backoff period
        if clock.unix_timestamp < ingestor_state.backoff_until {
            msg!("Still in backoff period until {}", ingestor_state.backoff_until);
            return Err(BettingPlatformError::RateLimited.into());
        }
        
        // Validate batch size
        if markets.len() > MAX_BATCH_SIZE as usize {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        let mut updates = Vec::new();
        let mut processed_count = 0u32;
        
        for market in markets.iter() {
            // Skip resolved markets
            if market.resolved {
                continue;
            }
            
            // Classify market to verse
            let verse_id = VerseClassifier::classify_market_to_verse(&market.title)?;
            
            // Convert prices from basis points to fixed point
            let prices = vec![
                U64F64::from_num(market.yes_price) / U64F64::from_num(10000),
                U64F64::from_num(market.no_price) / U64F64::from_num(10000),
            ];
            
            updates.push(BatchUpdate {
                market_id: market.id,
                prices: prices.iter().map(|p| p.to_num()).collect(),
                volume: market.volume_24h,
                liquidity: market.liquidity,
                verse_id,
            });
            
            processed_count += 1;
        }
        
        // Apply updates to proposals
        let mut updated_proposals = 0u32;
        for update in updates.iter() {
            for proposal in proposals.iter_mut() {
                if proposal.market_id == update.market_id {
                    // Update prices
                    proposal.prices = update.prices.clone();
                    
                    // Update volumes (assuming binary market)
                    if proposal.volumes.len() >= 2 {
                        proposal.volumes[0] = update.volume / 2; // Split volume
                        proposal.volumes[1] = update.volume / 2;
                    }
                    
                    // Update liquidity
                    proposal.liquidity_depth = update.liquidity;
                    
                    updated_proposals += 1;
                    break;
                }
            }
        }
        
        // Update verse aggregates
        for verse in verses.iter_mut() {
            let mut total_volume = 0u64;
            let mut weighted_prob = U64F64::from_num(0);
            let mut weight_sum = U64F64::from_num(0);
            
            // Aggregate data from child proposals
            for proposal in proposals.iter() {
                if VerseClassifier::classify_market_to_verse(&format!("{:?}", proposal.proposal_id))? == verse.verse_id {
                    total_volume = total_volume.saturating_add(
                        proposal.volumes.iter().sum::<u64>()
                    );
                    
                    if !proposal.prices.is_empty() {
                        let prob = U64F64::from_num(proposal.prices[0]);
                        let weight = U64F64::from_num(proposal.volumes.iter().sum::<u64>());
                        
                        weighted_prob = weighted_prob + prob * weight;
                        weight_sum = weight_sum + weight;
                    }
                }
            }
            
            // Update verse metrics
            verse.total_oi = total_volume;
            if weight_sum > U64F64::from_num(0) {
                verse.derived_prob = weighted_prob / weight_sum;
            }
            verse.last_update_slot = clock.slot;
        }
        
        // Update ingestor state
        ingestor_state.last_successful_batch = clock.slot;
        ingestor_state.total_ingested = ingestor_state.total_ingested
            .checked_add(processed_count as u64)
            .ok_or(BettingPlatformError::Overflow)?;
        
        // Reset error count on success
        ingestor_state.error_count = 0;
        ingestor_state.backoff_until = 0;
        
        msg!("Successfully ingested {} markets, updated {} proposals",
            processed_count, updated_proposals);
        
        Ok(())
    }
    
    /// Process paginated market ingestion (handles 21k+ markets)
    pub fn ingest_paginated(
        ingestor_state: &mut IngestorState,
        pagination: &mut PaginationState,
        markets: Vec<PolymarketMarket>,
        proposals: &mut [ProposalPDA],
        verses: &mut [VersePDA],
    ) -> ProgramResult {
        let clock = Clock::get()?;
        
        // Validate we're not fetching too frequently (spec: 0.35 req/s = ~3s between calls)
        if pagination.last_fetch_slot > 0 {
            let slots_elapsed = clock.slot.saturating_sub(pagination.last_fetch_slot);
            if slots_elapsed < 8 { // ~3.2 seconds at 0.4s/slot
                msg!("Rate limit: wait {} more slots", 8 - slots_elapsed);
                return Err(BettingPlatformError::RateLimited.into());
            }
        }
        
        // Process this batch
        Self::ingest_batch(ingestor_state, markets, proposals, verses)?;
        
        // Update pagination state
        pagination.last_fetch_slot = clock.slot;
        
        // Log progress
        let progress = (pagination.current_offset as f64 / pagination.total_markets as f64) * 100.0;
        msg!("Ingestion progress: {:.1}% ({}/{} markets)", 
            progress, pagination.current_offset, pagination.total_markets);
        
        Ok(())
    }
    
    /// Handle ingestion error with exponential backoff
    pub fn handle_error(
        ingestor_state: &mut IngestorState,
        error_type: IngestorError,
    ) -> ProgramResult {
        let clock = Clock::get()?;
        
        // Increment error count
        ingestor_state.error_count = ingestor_state.error_count
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
        
        // Calculate backoff duration
        let backoff_seconds = BACKOFF_MULTIPLIER * 2_i64.pow(ingestor_state.error_count);
        ingestor_state.backoff_until = clock.unix_timestamp
            .checked_add(backoff_seconds)
            .ok_or(BettingPlatformError::Overflow)?;
        
        msg!("Ingestor error: {:?}, backoff until {}",
            error_type, ingestor_state.backoff_until);
        
        // If too many errors, might need manual intervention
        if ingestor_state.error_count > 10 {
            msg!("ERROR: Ingestor has failed {} times, manual intervention required",
                ingestor_state.error_count);
        }
        
        Ok(())
    }
    
    /// Get next batch of markets to ingest
    pub fn get_next_batch(
        ingestor_state: &IngestorState,
    ) -> Result<(u32, u32), ProgramError> {
        let batch_size = MAX_BATCH_SIZE.min(
            ingestor_state.market_range_end - ingestor_state.market_range_start
        );
        
        let offset = ingestor_state.market_range_start;
        let limit = batch_size;
        
        Ok((offset, limit))
    }
    
    /// Validate market data before ingestion
    pub fn validate_market_data(
        market: &PolymarketMarket,
    ) -> Result<(), ProgramError> {
        // Check price sum (should be close to 10000 basis points)
        let price_sum = market.yes_price + market.no_price;
        if price_sum < 9900 || price_sum > 10100 {
            msg!("Invalid price sum: {} + {} = {}",
                market.yes_price, market.no_price, price_sum);
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Check outcomes
        if market.outcomes.len() < 2 {
            msg!("Market has fewer than 2 outcomes");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Validate title length
        if market.title.is_empty() || market.title.len() > 1000 {
            msg!("Invalid market title length: {}", market.title.len());
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        Ok(())
    }
    
    /// Emit price update event
    pub fn emit_update_event(
        market_id: &[u8; 32],
        keeper_id: &[u8; 32],
    ) -> ProgramResult {
        PriceUpdateProcessed {
            market_id: *market_id,
            keeper_id: *keeper_id,
            timestamp: Clock::get()?.unix_timestamp,
        }.emit();
        
        Ok(())
    }
}

/// Ingestor error types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum IngestorError {
    RateLimited,
    NetworkError,
    InvalidData,
    AuthenticationFailed,
    Unknown,
}

/// Production Polymarket data provider
/// Note: In Solana programs, HTTP requests must be made off-chain by keepers
pub struct PolymarketDataProvider;

impl PolymarketDataProvider {
    /// Expected API endpoint for keeper to fetch from
    pub const API_ENDPOINT: &'static str = "https://clob.polymarket.com/markets";
    pub const MAX_MARKETS_PER_BATCH: u32 = 1000;
    
    /// Process market data provided by keeper via instruction
    pub fn process_keeper_data(
        instruction_data: &[u8],
        keeper_pubkey: &Pubkey,
        authorized_keepers: &[Pubkey],
    ) -> Result<Vec<PolymarketMarket>, IngestorError> {
        // Verify keeper is authorized
        if !authorized_keepers.contains(keeper_pubkey) {
            msg!("Unauthorized keeper: {}", keeper_pubkey);
            return Err(IngestorError::AuthenticationFailed);
        }
        
        // Parse data format:
        // [0..8]: timestamp (i64)
        // [8..16]: offset (u64)
        // [16..24]: total_count (u64)
        // [24..]: market data (borsh serialized)
        
        if instruction_data.len() < 24 {
            return Err(IngestorError::InvalidData);
        }
        
        let timestamp = i64::from_le_bytes(
            instruction_data[0..8].try_into()
                .map_err(|_| IngestorError::InvalidData)?
        );
        
        let offset = u64::from_le_bytes(
            instruction_data[8..16].try_into()
                .map_err(|_| IngestorError::InvalidData)?
        );
        
        let total_count = u64::from_le_bytes(
            instruction_data[16..24].try_into()
                .map_err(|_| IngestorError::InvalidData)?
        );
        
        // Verify data freshness (must be within 5 minutes)
        let current_time = Clock::get()
            .map_err(|_| IngestorError::Unknown)?
            .unix_timestamp;
        
        if (current_time - timestamp).abs() > 300 {
            msg!("Stale data: {} seconds old", current_time - timestamp);
            return Err(IngestorError::InvalidData);
        }
        
        // Deserialize markets
        let markets: Vec<PolymarketMarket> = Vec::<PolymarketMarket>::try_from_slice(&instruction_data[24..])
            .map_err(|e| {
                msg!("Failed to deserialize markets: {:?}", e);
                IngestorError::InvalidData
            })?;
        
        // Validate market data
        for market in &markets {
            // Prices must sum to ~100%
            let price_sum = market.yes_price + market.no_price;
            if price_sum < 9900 || price_sum > 10100 {
                msg!("Invalid price sum for market {:?}: {}", market.id, price_sum);
                return Err(IngestorError::InvalidData);
            }
            
            // Must have binary outcomes
            if market.outcomes.len() != 2 {
                msg!("Non-binary market not supported: {} outcomes", market.outcomes.len());
                return Err(IngestorError::InvalidData);
            }
            
            // Liquidity sanity check
            if market.liquidity == 0 {
                msg!("Zero liquidity market");
                return Err(IngestorError::InvalidData);
            }
        }
        
        msg!("Processed {} markets from keeper (offset: {}, total: {})", 
            markets.len(), offset, total_count);
        
        Ok(markets)
    }
    
    /// Generate instruction data format for keepers
    pub fn format_keeper_instruction(
        markets: &[PolymarketMarket],
        offset: u64,
        total_count: u64,
    ) -> Result<Vec<u8>, IngestorError> {
        let mut data = Vec::new();
        
        // Add timestamp
        let timestamp = Clock::get()
            .map_err(|_| IngestorError::Unknown)?
            .unix_timestamp;
        data.extend_from_slice(&timestamp.to_le_bytes());
        
        // Add offset
        data.extend_from_slice(&offset.to_le_bytes());
        
        // Add total count
        data.extend_from_slice(&total_count.to_le_bytes());
        
        // Serialize markets
        let market_data = borsh::to_vec(markets)
            .map_err(|_| IngestorError::InvalidData)?;
        data.extend_from_slice(&market_data);
        
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_market_validation() {
        let valid_market = PolymarketMarket {
            id: [1u8; 32],
            title: "Will ETH reach $5k?".to_string(),
            description: "Test market".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 6500,
            no_price: 3500,
            volume_24h: 1_000_000,
            liquidity: 500_000,
            created_at: 1700000000,
            resolved: false,
            resolution: None,
        };
        
        assert!(IngestorKeeper::validate_market_data(&valid_market).is_ok());
        
        // Invalid price sum
        let mut invalid_market = valid_market.clone();
        invalid_market.yes_price = 7000;
        invalid_market.no_price = 4000;
        assert!(IngestorKeeper::validate_market_data(&invalid_market).is_err());
    }
    
    #[test]
    fn test_backoff_calculation() {
        let mut state = IngestorState::new([1u8; 32], 0, 1000);
        
        // First error: 10 * 2^1 = 20 seconds
        state.error_count = 1;
        let backoff = BACKOFF_MULTIPLIER * 2_i64.pow(state.error_count);
        assert_eq!(backoff, 20);
        
        // Fifth error: 10 * 2^5 = 320 seconds
        state.error_count = 5;
        let backoff = BACKOFF_MULTIPLIER * 2_i64.pow(state.error_count);
        assert_eq!(backoff, 320);
    }
}