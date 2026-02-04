//! Polymarket Sole Oracle Implementation
//! 
//! This module implements Polymarket as the ONLY oracle source for the platform.
//! No median-of-3, no other oracles - just Polymarket direct mirroring.
//!
//! Key features:
//! - 60-second polling interval
//! - 10% spread detection with automatic halt
//! - Stale price detection after 5 minutes
//! - Direct price mirroring (yes_price as truth)

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
    events::{emit_event, EventType},
};

/// Oracle configuration constants
pub const POLYMARKET_POLL_INTERVAL_SLOTS: u64 = 150; // 60 seconds at 0.4s/slot
pub const STALE_PRICE_THRESHOLD_SLOTS: u64 = 750; // 5 minutes
pub const SPREAD_HALT_THRESHOLD_BPS: u16 = 1000; // 10% spread triggers halt
pub const PRICE_DECIMAL_PLACES: u32 = 4; // Polymarket uses 4 decimals (basis points)

/// Polymarket price data
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct PolymarketPriceData {
    pub market_id: [u8; 16],
    pub yes_price: u64, // In basis points (10000 = 100%)
    pub no_price: u64,  // In basis points
    pub last_update_slot: u64,
    pub last_update_timestamp: i64,
    pub volume_24h: u64,
    pub liquidity: u64,
    pub is_halted: bool,
    pub halt_reason: HaltReason,
}

/// Reasons for halting a market
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum HaltReason {
    None,
    SpreadTooHigh,
    StalePrice,
    InternalError,
    ManualHalt,
    LowCoverage,
}

/// Polymarket sole oracle state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PolymarketSoleOracle {
    pub authority: Pubkey,
    pub is_initialized: bool,
    pub last_poll_slot: u64,
    pub total_markets_tracked: u64,
    pub halted_markets_count: u64,
    pub total_updates_processed: u64,
    pub total_spread_halts: u64,
    pub total_stale_halts: u64,
}

impl PolymarketSoleOracle {
    pub const SIZE: usize = 32 + // authority
        1 +   // is_initialized
        8 +   // last_poll_slot
        8 +   // total_markets_tracked
        8 +   // halted_markets_count
        8 +   // total_updates_processed
        8 +   // total_spread_halts
        8;    // total_stale_halts

    /// Initialize the Polymarket sole oracle
    pub fn initialize(&mut self, authority: &Pubkey) -> ProgramResult {
        if self.is_initialized {
            return Err(BettingPlatformError::AlreadyInitialized.into());
        }

        self.authority = *authority;
        self.is_initialized = true;
        self.last_poll_slot = 0;
        self.total_markets_tracked = 0;
        self.halted_markets_count = 0;
        self.total_updates_processed = 0;
        self.total_spread_halts = 0;
        self.total_stale_halts = 0;

        msg!("Polymarket sole oracle initialized");
        Ok(())
    }

    /// Check if it's time to poll Polymarket (every 60 seconds)
    pub fn should_poll(&self, current_slot: u64) -> bool {
        current_slot >= self.last_poll_slot + POLYMARKET_POLL_INTERVAL_SLOTS
    }

    /// Update oracle poll timestamp
    pub fn update_poll_time(&mut self, current_slot: u64) {
        self.last_poll_slot = current_slot;
    }

    /// Process a price update from Polymarket
    pub fn process_price_update(
        &mut self,
        price_data: &mut PolymarketPriceData,
        current_slot: u64,
    ) -> ProgramResult {
        // Check if price is stale
        if self.is_price_stale(&price_data, current_slot) {
            price_data.is_halted = true;
            price_data.halt_reason = HaltReason::StalePrice;
            self.total_stale_halts += 1;
            msg!("Market halted due to stale price");
            return Ok(());
        }

        // Check spread (should be ~100% total, but allow some deviation)
        let total_prob = price_data.yes_price + price_data.no_price;
        let spread = if total_prob > 10000 {
            total_prob - 10000
        } else {
            10000 - total_prob
        };

        if spread > SPREAD_HALT_THRESHOLD_BPS as u64 {
            price_data.is_halted = true;
            price_data.halt_reason = HaltReason::SpreadTooHigh;
            self.total_spread_halts += 1;
            msg!("Market halted: spread {} bps exceeds 10% threshold", spread);
            return Ok(());
        }

        // Price is valid, update timestamps
        price_data.last_update_slot = current_slot;
        price_data.is_halted = false;
        price_data.halt_reason = HaltReason::None;
        self.total_updates_processed += 1;

        Ok(())
    }

    /// Check if a price is stale (older than 5 minutes)
    pub fn is_price_stale(&self, price_data: &PolymarketPriceData, current_slot: u64) -> bool {
        current_slot > price_data.last_update_slot + STALE_PRICE_THRESHOLD_SLOTS
    }

    /// Get the current price for a market (yes_price as truth)
    pub fn get_price(&self, price_data: &PolymarketPriceData) -> Result<u64, ProgramError> {
        if price_data.is_halted {
            return Err(BettingPlatformError::MarketHalted.into());
        }

        // Return yes_price as the source of truth
        Ok(price_data.yes_price)
    }

    /// Manual halt/unhalt for emergency situations
    pub fn set_halt_status(
        &mut self,
        price_data: &mut PolymarketPriceData,
        halt: bool,
        reason: HaltReason,
    ) -> ProgramResult {
        price_data.is_halted = halt;
        price_data.halt_reason = reason;

        if halt {
            self.halted_markets_count += 1;
        } else if self.halted_markets_count > 0 {
            self.halted_markets_count -= 1;
        }

        Ok(())
    }
}

/// Money-making opportunity detection
impl PolymarketSoleOracle {
    /// Check for arbitrage opportunities during halts
    pub fn get_halt_arbitrage_opportunity(&self, price_data: &PolymarketPriceData) -> Option<f64> {
        if !price_data.is_halted {
            return None;
        }

        match price_data.halt_reason {
            HaltReason::SpreadTooHigh => {
                // When spread is high, there's often a 5%+ arbitrage opportunity post-resume
                Some(0.05)
            }
            HaltReason::StalePrice => {
                // Stale prices often lead to 3%+ moves on refresh
                Some(0.03)
            }
            _ => None,
        }
    }

    /// Calculate expected edge from being early on price updates
    pub fn calculate_polling_edge(&self, last_user_check: u64, current_slot: u64) -> f64 {
        let slots_behind = current_slot.saturating_sub(last_user_check);
        let seconds_behind = (slots_behind as f64) * 0.4;
        
        // Each second of delay = ~0.1% edge for fast movers
        (seconds_behind * 0.001).min(0.05) // Cap at 5% edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_detection() {
        let mut oracle = PolymarketSoleOracle {
            authority: Pubkey::default(),
            is_initialized: true,
            last_poll_slot: 0,
            total_markets_tracked: 0,
            halted_markets_count: 0,
            total_updates_processed: 0,
            total_spread_halts: 0,
            total_stale_halts: 0,
        };

        // Test normal spread (should pass)
        let mut normal_price = PolymarketPriceData {
            market_id: [0u8; 16],
            yes_price: 6000, // 60%
            no_price: 4000,  // 40%
            last_update_slot: 100,
            last_update_timestamp: 1234567890,
            volume_24h: 1_000_000,
            liquidity: 500_000,
            is_halted: false,
            halt_reason: HaltReason::None,
        };

        oracle.process_price_update(&mut normal_price, 200).unwrap();
        assert!(!normal_price.is_halted);

        // Test high spread (should halt)
        let mut high_spread = PolymarketPriceData {
            market_id: [1u8; 16],
            yes_price: 7000, // 70%
            no_price: 4500,  // 45% - Total 115% (15% spread)
            last_update_slot: 100,
            last_update_timestamp: 1234567890,
            volume_24h: 1_000_000,
            liquidity: 500_000,
            is_halted: false,
            halt_reason: HaltReason::None,
        };

        oracle.process_price_update(&mut high_spread, 200).unwrap();
        assert!(high_spread.is_halted);
        assert_eq!(high_spread.halt_reason, HaltReason::SpreadTooHigh);
        assert_eq!(oracle.total_spread_halts, 1);
    }

    #[test]
    fn test_stale_price_detection() {
        let oracle = PolymarketSoleOracle {
            authority: Pubkey::default(),
            is_initialized: true,
            last_poll_slot: 0,
            total_markets_tracked: 0,
            halted_markets_count: 0,
            total_updates_processed: 0,
            total_spread_halts: 0,
            total_stale_halts: 0,
        };

        let price_data = PolymarketPriceData {
            market_id: [0u8; 16],
            yes_price: 5000,
            no_price: 5000,
            last_update_slot: 1000,
            last_update_timestamp: 1234567890,
            volume_24h: 1_000_000,
            liquidity: 500_000,
            is_halted: false,
            halt_reason: HaltReason::None,
        };

        // Fresh price
        assert!(!oracle.is_price_stale(&price_data, 1500));

        // Stale price (>750 slots)
        assert!(oracle.is_price_stale(&price_data, 1751));
    }

    #[test]
    fn test_polling_interval() {
        let oracle = PolymarketSoleOracle {
            authority: Pubkey::default(),
            is_initialized: true,
            last_poll_slot: 1000,
            total_markets_tracked: 0,
            halted_markets_count: 0,
            total_updates_processed: 0,
            total_spread_halts: 0,
            total_stale_halts: 0,
        };

        // Should not poll yet
        assert!(!oracle.should_poll(1149));

        // Should poll now (150 slots = 60 seconds)
        assert!(oracle.should_poll(1150));
    }
}