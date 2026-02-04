use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Market status enum
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MarketStatus {
    Active,
    Paused,
    Settled,
    Cancelled,
}

/// Essential market data for compression
/// This contains only the most critical fields needed for verification
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketEssentials {
    /// Market ID (32 bytes)
    pub market_id: [u8; 32],
    
    /// Current price (fixed point 6 decimals)
    pub current_price: u64,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Number of outcomes
    pub outcome_count: u8,
    
    /// Market status
    pub status: MarketStatus,
    
    /// Last update timestamp
    pub last_update: i64,
}

impl MarketEssentials {
    /// Size in bytes
    pub const SIZE: usize = 32 + // market_id
        8 + // current_price
        8 + // total_volume
        1 + // outcome_count
        1 + // status
        8; // last_update
    
    /// Create from full market data
    pub fn from_market_data(
        market_id: [u8; 32],
        current_price: u64,
        total_volume: u64,
        outcome_count: u8,
        status: MarketStatus,
        last_update: i64,
    ) -> Self {
        Self {
            market_id,
            current_price,
            total_volume,
            outcome_count,
            status,
            last_update,
        }
    }
    
    /// Validate essential data
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check outcome count
        if self.outcome_count == 0 {
            return Err(crate::error::CompressionError::InvalidMarketData.into());
        }
        
        // Check price is reasonable (0-100% with 6 decimals)
        if self.current_price > 100_000_000 {
            return Err(crate::error::CompressionError::InvalidMarketData.into());
        }
        
        Ok(())
    }
    
    /// Convert to bytes for hashing
    pub fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap_or_default()
    }
}

/// Market data for reconstruction after decompression
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketData {
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Current price
    pub current_price: u64,
    
    /// Total volume
    pub total_volume: u64,
    
    /// Outcome count
    pub outcome_count: u8,
    
    /// Market status
    pub status: MarketStatus,
    
    /// Last update
    pub last_update: i64,
    
    /// Additional fields reconstructed from other sources
    pub liquidity: u64,
    pub open_interest: u64,
    pub creation_time: i64,
    pub settlement_time: Option<i64>,
}

impl MarketData {
    /// Create from essentials with defaults for additional fields
    pub fn from_essentials(essentials: &MarketEssentials) -> Self {
        Self {
            market_id: essentials.market_id,
            current_price: essentials.current_price,
            total_volume: essentials.total_volume,
            outcome_count: essentials.outcome_count,
            status: essentials.status,
            last_update: essentials.last_update,
            liquidity: 0,
            open_interest: 0,
            creation_time: 0,
            settlement_time: None,
        }
    }
}