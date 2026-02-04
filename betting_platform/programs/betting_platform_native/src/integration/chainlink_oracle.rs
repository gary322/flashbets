// Chainlink Oracle Integration for Median-of-3 Price Aggregation
// This module handles Chainlink price feeds

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

use crate::{
    error::BettingPlatformError,
    integration::{
        oracle_coordinator::OracleSource,
        polymarket_oracle::{OraclePriceData, PRICE_DECIMAL_PLACES, MAX_PRICE_AGE_SLOTS},
    },
};

/// Chainlink Round Data
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ChainlinkRound {
    pub round_id: u128,
    pub answer: i128,           // Price with decimals
    pub started_at: u64,        // Timestamp
    pub updated_at: u64,        // Timestamp
    pub answered_in_round: u128,
}

impl ChainlinkRound {
    pub const SIZE: usize = 16 + 16 + 8 + 8 + 16; // 64 bytes
}

/// Chainlink Aggregator Account
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ChainlinkAggregator {
    pub version: u8,
    pub decimals: u8,
    pub description: String,
    pub latest_round_id: u128,
    pub latest_answer: i128,
    pub latest_timestamp: u64,
    pub latest_started_at: u64,
    pub updated_at_slot: u64,
    pub min_answer: i128,
    pub max_answer: i128,
    pub answered_in_round: u128,
}

impl ChainlinkAggregator {
    pub const VERSION: u8 = 3;
    pub const MAX_DECIMALS: u8 = 18;
    
    /// Validate Chainlink aggregator
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.version != Self::VERSION {
            return Err(BettingPlatformError::InvalidChainlinkVersion.into());
        }
        
        if self.decimals > Self::MAX_DECIMALS {
            return Err(BettingPlatformError::InvalidChainlinkDecimals.into());
        }
        
        if self.latest_answer <= 0 {
            return Err(BettingPlatformError::InvalidChainlinkPrice.into());
        }
        
        // Check price bounds
        if self.latest_answer < self.min_answer || self.latest_answer > self.max_answer {
            return Err(BettingPlatformError::ChainlinkPriceOutOfBounds.into());
        }
        
        Ok(())
    }
    
    /// Get price in standard format (8 decimals)
    pub fn get_price(&self) -> Result<u64, ProgramError> {
        if self.latest_answer <= 0 {
            return Err(BettingPlatformError::InvalidChainlinkPrice.into());
        }
        
        // Convert from Chainlink's decimals to our standard 8 decimals
        let price = if self.decimals > PRICE_DECIMAL_PLACES as u8 {
            // Scale down
            let scale = 10u128.pow((self.decimals - PRICE_DECIMAL_PLACES as u8) as u32);
            (self.latest_answer as u128 / scale) as u64
        } else {
            // Scale up
            let scale = 10u128.pow((PRICE_DECIMAL_PLACES as u8 - self.decimals) as u32);
            (self.latest_answer as u128 * scale) as u64
        };
        
        Ok(price)
    }
    
    /// Calculate confidence based on update frequency and price stability
    pub fn get_confidence(&self, current_timestamp: i64) -> u64 {
        // Base confidence of 95%
        let mut confidence = 9500u64;
        
        // Reduce confidence for stale data (1% per minute)
        let age_seconds = (current_timestamp - self.latest_timestamp as i64).abs() as u64;
        let age_penalty = (age_seconds / 60).min(50) * 100; // Max 50% penalty
        confidence = confidence.saturating_sub(age_penalty);
        
        // Additional confidence based on round completion
        if self.latest_round_id == self.answered_in_round {
            confidence += 500; // 5% bonus for fresh round
        }
        
        confidence.min(10000) // Cap at 100%
    }
    
    /// Check if price is fresh
    pub fn is_fresh(&self, current_slot: u64) -> bool {
        current_slot.saturating_sub(self.updated_at_slot) <= MAX_PRICE_AGE_SLOTS
    }
}

/// Chainlink Oracle Configuration
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ChainlinkOracleConfig {
    pub authority: Pubkey,
    pub chainlink_program_id: Pubkey,
    pub price_feeds: Vec<ChainlinkPriceFeedMapping>,
    pub last_update_slot: u64,
    pub total_feeds: u32,
    pub active_feeds: u32,
}

impl ChainlinkOracleConfig {
    pub const SIZE: usize = 32 + // authority
        32 + // chainlink_program_id
        4 + // vec length prefix
        (100 * ChainlinkPriceFeedMapping::SIZE) + // up to 100 feeds
        8 + // last_update_slot
        4 + // total_feeds
        4; // active_feeds
    
    /// Initialize Chainlink oracle config
    pub fn initialize(&mut self, authority: &Pubkey, chainlink_program_id: &Pubkey) -> ProgramResult {
        self.authority = *authority;
        self.chainlink_program_id = *chainlink_program_id;
        self.price_feeds = Vec::new();
        self.last_update_slot = 0;
        self.total_feeds = 0;
        self.active_feeds = 0;
        
        msg!("Chainlink oracle config initialized");
        Ok(())
    }
    
    /// Add price feed mapping
    pub fn add_price_feed(
        &mut self,
        market_id: Pubkey,
        feed_account: Pubkey,
        base_symbol: String,
        quote_symbol: String,
    ) -> ProgramResult {
        if self.price_feeds.iter().any(|f| f.market_id == market_id) {
            return Err(BettingPlatformError::PriceFeedAlreadyExists.into());
        }
        
        self.price_feeds.push(ChainlinkPriceFeedMapping {
            market_id,
            feed_account,
            base_symbol,
            quote_symbol,
            last_update_slot: 0,
            last_price: 0,
            last_round_id: 0,
        });
        
        self.total_feeds += 1;
        msg!("Added Chainlink price feed for market {}", market_id);
        Ok(())
    }
}

/// Mapping between our market and Chainlink feed
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ChainlinkPriceFeedMapping {
    pub market_id: Pubkey,
    pub feed_account: Pubkey,
    pub base_symbol: String,    // e.g., "BTC"
    pub quote_symbol: String,   // e.g., "USD"
    pub last_update_slot: u64,
    pub last_price: u64,
    pub last_round_id: u128,
}

impl ChainlinkPriceFeedMapping {
    pub const SIZE: usize = 32 + // market_id
        32 + // feed_account
        16 + // base_symbol (max 16 chars)
        16 + // quote_symbol (max 16 chars)
        8 + // last_update_slot
        8 + // last_price
        16; // last_round_id
}

/// Chainlink Oracle Handler
pub struct ChainlinkOracleHandler;

impl ChainlinkOracleHandler {
    /// Fetch price from Chainlink
    pub fn fetch_price(
        feed_account: &AccountInfo,
        current_slot: u64,
    ) -> Result<OraclePriceData, ProgramError> {
        // Deserialize Chainlink aggregator
        let aggregator = ChainlinkAggregator::try_from_slice(&feed_account.data.borrow())?;
        
        // Validate account
        aggregator.validate()?;
        
        // Check freshness
        if !aggregator.is_fresh(current_slot) {
            return Err(BettingPlatformError::StaleChainlinkPrice.into());
        }
        
        // Get price and confidence
        let price = aggregator.get_price()?;
        let confidence = aggregator.get_confidence(Clock::get()?.unix_timestamp);
        
        Ok(OraclePriceData {
            source: OracleSource::Chainlink,
            price,
            confidence,
            timestamp: aggregator.latest_timestamp as i64,
            slot: current_slot,
        })
    }
    
    /// Update price from Chainlink for a market
    pub fn update_price(
        config: &mut ChainlinkOracleConfig,
        market_id: &Pubkey,
        feed_account: &AccountInfo,
        current_slot: u64,
    ) -> Result<OraclePriceData, ProgramError> {
        // Find mapping
        let mapping = config.price_feeds.iter_mut()
            .find(|f| &f.market_id == market_id)
            .ok_or(BettingPlatformError::ChainlinkMappingNotFound)?;
        
        // Verify account matches mapping
        if mapping.feed_account != *feed_account.key {
            return Err(BettingPlatformError::InvalidChainlinkFeed.into());
        }
        
        // Fetch price
        let price_data = Self::fetch_price(feed_account, current_slot)?;
        
        // Update mapping
        mapping.last_update_slot = current_slot;
        mapping.last_price = price_data.price;
        
        // Update config stats
        config.last_update_slot = current_slot;
        config.active_feeds = config.price_feeds.iter()
            .filter(|f| current_slot.saturating_sub(f.last_update_slot) <= MAX_PRICE_AGE_SLOTS)
            .count() as u32;
        
        Ok(price_data)
    }
    
    /// Get all active prices for median calculation
    pub fn get_active_prices(
        config: &ChainlinkOracleConfig,
        current_slot: u64,
    ) -> Vec<(Pubkey, u64)> {
        config.price_feeds.iter()
            .filter(|f| current_slot.saturating_sub(f.last_update_slot) <= MAX_PRICE_AGE_SLOTS)
            .map(|f| (f.market_id, f.last_price))
            .collect()
    }
    
    /// Convert Chainlink feed data for prediction markets
    pub fn convert_to_probability(
        price: u64,
        market_type: &str,
    ) -> Result<(u64, u64), ProgramError> {
        // For binary prediction markets, convert price to probability
        match market_type {
            "BINARY" => {
                // Assume price is already a probability (0-1 scaled to decimals)
                let yes_prob = price;
                let no_prob = 10u64.pow(PRICE_DECIMAL_PLACES) - yes_prob;
                Ok((yes_prob, no_prob))
            },
            "PRICE_LEVEL" => {
                // For "Will BTC be above $X" type markets
                // This would need market-specific logic
                Ok((price, 10u64.pow(PRICE_DECIMAL_PLACES) - price))
            },
            _ => Err(BettingPlatformError::UnsupportedMarketType.into()),
        }
    }
}

/// Chainlink test utilities
#[cfg(test)]
pub mod test_utils {
    use super::*;
    
    /// Create mock Chainlink aggregator
    pub fn create_mock_chainlink_aggregator(
        price: i128,
        decimals: u8,
        slot: u64,
    ) -> ChainlinkAggregator {
        ChainlinkAggregator {
            version: ChainlinkAggregator::VERSION,
            decimals,
            description: "TEST/USD".to_string(),
            latest_round_id: 1000,
            latest_answer: price,
            latest_timestamp: Clock::get().unwrap().unix_timestamp as u64,
            latest_started_at: Clock::get().unwrap().unix_timestamp as u64 - 10,
            updated_at_slot: slot,
            min_answer: price / 2,
            max_answer: price * 2,
            answered_in_round: 1000,
        }
    }
}