// Pyth Oracle Integration for Median-of-3 Price Aggregation
// This module handles Pyth Network price feeds

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
use crate::math::fixed_point::U64F64;

use crate::{
    error::BettingPlatformError,
    integration::{
        oracle_coordinator::OracleSource,
        polymarket_oracle::{OraclePriceData, PRICE_DECIMAL_PLACES, MAX_PRICE_AGE_SLOTS},
    },
};

/// Pyth Price Status
#[repr(u8)]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum PythPriceStatus {
    Unknown = 0,
    Trading = 1,
    Halted = 2,
    Auction = 3,
}

/// Pyth Price Type
#[repr(u8)]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum PythPriceType {
    Unknown = 0,
    Price = 1,
    TWAP = 2,
    Volatility = 3,
}

/// Pyth Price Account Structure (simplified)
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PythPriceAccount {
    pub magic: u32,           // Pyth magic number (0xa1b2c3d4)
    pub version: u32,         // Version
    pub account_type: u32,    // Account type (3 for price)
    pub size: u32,           // Size of price account
    pub price_type: PythPriceType,
    pub exponent: i32,        // Price exponent
    pub num_components: u32,  // Number of price components
    pub num_quoters: u32,     // Number of quoters
    pub last_slot: u64,       // Last slot updated
    pub valid_slot: u64,      // Valid until slot
    pub prod_price: i64,      // Product price
    pub next_price: i64,      // Next price
    pub prev_slot: u64,       // Previous slot
    pub prev_price: i64,      // Previous price
    pub prev_conf: u64,       // Previous confidence
    pub price: i64,           // Current price
    pub conf: u64,            // Price confidence interval
    pub status: PythPriceStatus,
    pub corporate_action: u32,
    pub publish_slot: u64,    // Publish slot
}

impl PythPriceAccount {
    pub const MAGIC: u32 = 0xa1b2c3d4;
    pub const VERSION: u32 = 2;
    pub const PRICE_ACCOUNT_TYPE: u32 = 3;
    
    /// Validate Pyth account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.magic != Self::MAGIC {
            return Err(BettingPlatformError::InvalidPythAccount.into());
        }
        
        if self.account_type != Self::PRICE_ACCOUNT_TYPE {
            return Err(BettingPlatformError::InvalidPythAccountType.into());
        }
        
        if self.status != PythPriceStatus::Trading {
            return Err(BettingPlatformError::PythPriceNotTrading.into());
        }
        
        Ok(())
    }
    
    /// Get price in standard format (8 decimals)
    pub fn get_price_unchecked(&self) -> Result<u64, ProgramError> {
        if self.price <= 0 {
            return Err(BettingPlatformError::InvalidPythPrice.into());
        }
        
        // Convert from Pyth's exponent to our standard 8 decimals
        let price_f64 = self.price as f64 * 10f64.powi(-self.exponent);
        let standard_price = (price_f64 * 10f64.powi(PRICE_DECIMAL_PLACES as i32)) as u64;
        
        Ok(standard_price)
    }
    
    /// Get confidence interval in standard format
    pub fn get_confidence(&self) -> u64 {
        // Convert confidence to basis points (10000 = 100%)
        let price_abs = self.price.abs() as u64;
        if price_abs == 0 {
            return 0;
        }
        
        // Calculate confidence as (1 - conf/price) * 10000
        let conf_ratio = (self.conf * 10000) / price_abs;
        10000u64.saturating_sub(conf_ratio)
    }
    
    /// Check if price is fresh
    pub fn is_fresh(&self, current_slot: u64) -> bool {
        current_slot.saturating_sub(self.publish_slot) <= MAX_PRICE_AGE_SLOTS
    }
}

/// Pyth Oracle Configuration
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PythOracleConfig {
    pub authority: Pubkey,
    pub pyth_program_id: Pubkey,
    pub price_feeds: Vec<PythPriceFeedMapping>,
    pub last_update_slot: u64,
    pub total_feeds: u32,
    pub active_feeds: u32,
}

impl PythOracleConfig {
    pub const SIZE: usize = 32 + // authority
        32 + // pyth_program_id
        4 + // vec length prefix
        (100 * PythPriceFeedMapping::SIZE) + // up to 100 feeds
        8 + // last_update_slot
        4 + // total_feeds
        4; // active_feeds
    
    /// Initialize Pyth oracle config
    pub fn initialize(&mut self, authority: &Pubkey, pyth_program_id: &Pubkey) -> ProgramResult {
        self.authority = *authority;
        self.pyth_program_id = *pyth_program_id;
        self.price_feeds = Vec::new();
        self.last_update_slot = 0;
        self.total_feeds = 0;
        self.active_feeds = 0;
        
        msg!("Pyth oracle config initialized");
        Ok(())
    }
    
    /// Add price feed mapping
    pub fn add_price_feed(
        &mut self,
        market_id: Pubkey,
        pyth_price_account: Pubkey,
        symbol: String,
    ) -> ProgramResult {
        if self.price_feeds.iter().any(|f| f.market_id == market_id) {
            return Err(BettingPlatformError::PriceFeedAlreadyExists.into());
        }
        
        self.price_feeds.push(PythPriceFeedMapping {
            market_id,
            pyth_price_account,
            symbol,
            last_update_slot: 0,
            last_price: 0,
        });
        
        self.total_feeds += 1;
        msg!("Added Pyth price feed for market {}", market_id);
        Ok(())
    }
}

/// Mapping between our market and Pyth price account
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PythPriceFeedMapping {
    pub market_id: Pubkey,
    pub pyth_price_account: Pubkey,
    pub symbol: String,
    pub last_update_slot: u64,
    pub last_price: u64,
}

impl PythPriceFeedMapping {
    pub const SIZE: usize = 32 + // market_id
        32 + // pyth_price_account
        32 + // symbol (max 32 chars)
        8 + // last_update_slot
        8; // last_price
}

/// Pyth Oracle Handler
pub struct PythOracleHandler;

impl PythOracleHandler {
    /// Fetch price from Pyth
    pub fn fetch_price(
        pyth_price_account: &AccountInfo,
        current_slot: u64,
    ) -> Result<OraclePriceData, ProgramError> {
        // Deserialize Pyth price account
        let pyth_price = PythPriceAccount::try_from_slice(&pyth_price_account.data.borrow())?;
        
        // Validate account
        pyth_price.validate()?;
        
        // Check freshness
        if !pyth_price.is_fresh(current_slot) {
            return Err(BettingPlatformError::StalePythPrice.into());
        }
        
        // Get price and confidence
        let price = pyth_price.get_price_unchecked()?;
        let confidence = pyth_price.get_confidence();
        
        Ok(OraclePriceData {
            source: OracleSource::Pyth,
            price,
            confidence,
            timestamp: Clock::get()?.unix_timestamp,
            slot: current_slot,
        })
    }
    
    /// Update price from Pyth for a market
    pub fn update_price(
        config: &mut PythOracleConfig,
        market_id: &Pubkey,
        pyth_price_account: &AccountInfo,
        current_slot: u64,
    ) -> Result<OraclePriceData, ProgramError> {
        // Find mapping
        let mapping = config.price_feeds.iter_mut()
            .find(|f| &f.market_id == market_id)
            .ok_or(BettingPlatformError::PythMappingNotFound)?;
        
        // Verify account matches mapping
        if mapping.pyth_price_account != *pyth_price_account.key {
            return Err(BettingPlatformError::InvalidPythAccount.into());
        }
        
        // Fetch price
        let price_data = Self::fetch_price(pyth_price_account, current_slot)?;
        
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
        config: &PythOracleConfig,
        current_slot: u64,
    ) -> Vec<(Pubkey, u64)> {
        config.price_feeds.iter()
            .filter(|f| current_slot.saturating_sub(f.last_update_slot) <= MAX_PRICE_AGE_SLOTS)
            .map(|f| (f.market_id, f.last_price))
            .collect()
    }
}

/// Pyth test utilities
#[cfg(test)]
pub mod test_utils {
    use super::*;
    
    /// Create mock Pyth price account
    pub fn create_mock_pyth_price(
        price: i64,
        confidence: u64,
        slot: u64,
    ) -> PythPriceAccount {
        PythPriceAccount {
            magic: PythPriceAccount::MAGIC,
            version: PythPriceAccount::VERSION,
            account_type: PythPriceAccount::PRICE_ACCOUNT_TYPE,
            size: 256,
            price_type: PythPriceType::Price,
            exponent: -8,
            num_components: 1,
            num_quoters: 3,
            last_slot: slot,
            valid_slot: slot + 25,
            prod_price: price,
            next_price: price,
            prev_slot: slot - 1,
            prev_price: price,
            prev_conf: confidence,
            price,
            conf: confidence,
            status: PythPriceStatus::Trading,
            corporate_action: 0,
            publish_slot: slot,
        }
    }
}