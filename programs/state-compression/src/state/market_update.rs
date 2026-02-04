use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

use crate::state::{MarketEssentials, MarketStatus};

/// Market update variants for state compression
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum MarketUpdate {
    /// Update market price
    Price(u64),
    /// Update volume (additive)
    Volume(u64),
    /// Update market status
    Status(MarketStatus),
    /// Update liquidity
    Liquidity(u64),
    /// Update multiple fields at once
    Batch {
        price: Option<u64>,
        volume: Option<u64>,
        liquidity: Option<u64>,
        status: Option<MarketStatus>,
    },
}

impl MarketUpdate {
    /// Apply update to market essentials
    pub fn apply(&self, market: &mut MarketEssentials) -> Result<(), ProgramError> {
        match self {
            MarketUpdate::Price(price) => {
                market.current_price = *price;
            }
            MarketUpdate::Volume(volume) => {
                market.total_volume = market.total_volume
                    .checked_add(*volume)
                    .ok_or(ProgramError::ArithmeticOverflow)?;
            }
            MarketUpdate::Status(status) => {
                market.status = *status;
            }
            MarketUpdate::Liquidity(liquidity) => {
                // MarketEssentials doesn't have liquidity field, so we ignore for now
                // In production, would extend MarketEssentials or handle separately
                let _ = liquidity;
            }
            MarketUpdate::Batch { price, volume, liquidity, status } => {
                if let Some(p) = price {
                    market.current_price = *p;
                }
                if let Some(v) = volume {
                    market.total_volume = market.total_volume
                        .checked_add(*v)
                        .ok_or(ProgramError::ArithmeticOverflow)?;
                }
                if let Some(s) = status {
                    market.status = *s;
                }
                // Ignore liquidity for now
                let _ = liquidity;
            }
        }
        
        Ok(())
    }

    /// Get the size of this update variant
    pub fn size(&self) -> usize {
        match self {
            MarketUpdate::Price(_) => 1 + 8,
            MarketUpdate::Volume(_) => 1 + 8,
            MarketUpdate::Status(_) => 1 + 1,
            MarketUpdate::Liquidity(_) => 1 + 8,
            MarketUpdate::Batch { .. } => 1 + 1 + 8 + 1 + 8 + 1 + 8 + 1 + 1,
        }
    }
}