use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
};

/// Constants for price history management
pub const MAX_HISTORY_DAYS: usize = 7;
pub const SLOTS_PER_DAY: u64 = 216_000; // ~0.4s per slot
pub const SLOTS_PER_HOUR: u64 = 9_000;
pub const MAX_HOURLY_POINTS: usize = 24;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PricePoint {
    pub price: u64,          // Fixed point representation
    pub timestamp: i64,
    pub slot: u64,
    pub volume: u64,         // 24h volume at this point
}

/// Price history for a single market
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MarketPriceHistory {
    pub is_initialized: bool,
    pub market_id: [u8; 16],
    pub daily_prices: Vec<PricePoint>,    // 7-day rolling window
    pub hourly_prices: Vec<PricePoint>,   // Last 24 hours
    pub last_update_slot: u64,
    pub bump: u8,
}

impl MarketPriceHistory {
    pub const BASE_LEN: usize = 1 + 16 + 4 + 4 + 8 + 1;
    
    pub fn new(market_id: [u8; 16], bump: u8) -> Self {
        Self {
            is_initialized: true,
            market_id,
            daily_prices: Vec::with_capacity(MAX_HISTORY_DAYS),
            hourly_prices: Vec::with_capacity(MAX_HOURLY_POINTS),
            last_update_slot: 0,
            bump,
        }
    }
    
    /// Add a new price point to the history
    pub fn add_price_point(
        &mut self,
        price: u64,
        timestamp: i64,
        slot: u64,
        volume: u64,
    ) -> Result<(), ProgramError> {
        let point = PricePoint {
            price,
            timestamp,
            slot,
            volume,
        };
        
        // Add to hourly prices
        self.hourly_prices.push(point.clone());
        
        // Remove old hourly prices (keep last 24 hours)
        let cutoff_slot = slot.saturating_sub(SLOTS_PER_HOUR * 24);
        self.hourly_prices.retain(|p| p.slot >= cutoff_slot);
        
        // Update daily price if new day
        let current_day = slot / SLOTS_PER_DAY;
        let last_day = self.last_update_slot / SLOTS_PER_DAY;
        
        if current_day > last_day || self.daily_prices.is_empty() {
            // Calculate daily average from hourly prices
            let daily_avg = if !self.hourly_prices.is_empty() {
                let sum: u128 = self.hourly_prices.iter()
                    .map(|p| p.price as u128)
                    .sum();
                (sum / self.hourly_prices.len() as u128) as u64
            } else {
                price
            };
            
            self.daily_prices.push(PricePoint {
                price: daily_avg,
                timestamp,
                slot,
                volume,
            });
            
            // Keep only last 7 days
            if self.daily_prices.len() > MAX_HISTORY_DAYS {
                self.daily_prices.remove(0);
            }
        }
        
        self.last_update_slot = slot;
        Ok(())
    }
    
    /// Get daily prices for correlation calculation
    pub fn get_daily_prices(&self) -> Vec<u64> {
        self.daily_prices.iter()
            .map(|p| p.price)
            .collect()
    }
    
    /// Check if we have enough data for correlation calculation
    pub fn has_sufficient_data(&self) -> bool {
        self.daily_prices.len() >= MAX_HISTORY_DAYS
    }
    
    /// Calculate the size needed for this account
    pub fn calculate_size() -> usize {
        Self::BASE_LEN 
            + (MAX_HISTORY_DAYS * std::mem::size_of::<PricePoint>())
            + (MAX_HOURLY_POINTS * std::mem::size_of::<PricePoint>())
            + 100 // Buffer for vec overhead
    }
}

/// Aggregated price history for a verse
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VersePriceHistory {
    pub is_initialized: bool,
    pub verse_id: [u8; 16],
    pub market_histories: Vec<[u8; 16]>,  // References to child market histories
    pub weighted_daily_prices: Vec<PricePoint>,  // Weighted average prices
    pub last_update_slot: u64,
    pub bump: u8,
}

impl VersePriceHistory {
    pub fn new(verse_id: [u8; 16], bump: u8) -> Self {
        Self {
            is_initialized: true,
            verse_id,
            market_histories: Vec::new(),
            weighted_daily_prices: Vec::with_capacity(MAX_HISTORY_DAYS),
            last_update_slot: 0,
            bump,
        }
    }
    
    pub fn add_market(&mut self, market_id: [u8; 16]) -> Result<(), ProgramError> {
        if !self.market_histories.contains(&market_id) {
            self.market_histories.push(market_id);
        }
        Ok(())
    }
    
    pub fn remove_market(&mut self, market_id: &[u8; 16]) -> Result<(), ProgramError> {
        self.market_histories.retain(|id| id != market_id);
        Ok(())
    }
}