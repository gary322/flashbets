//! Polymarket Oracle Integration
//!
//! Sole oracle source for the betting platform

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
    state::ProposalPDA,
    constants::*,
};

/// Polymarket price feed state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketPriceFeed {
    /// Market identifier
    pub market_id: String,
    
    /// Current prices for each outcome
    pub prices: Vec<u64>,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Total volume
    pub total_volume: u64,
    
    /// Oracle status
    pub status: OracleStatus,
}

/// Oracle status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OracleStatus {
    Active,
    Stale,
    Halted,
    Disputed,
}

/// Update Polymarket price
pub fn update_polymarket_price(
    accounts: &[AccountInfo],
    market_id: String,
    prices: Vec<u64>,
    volume: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Deserialize oracle state
    let mut oracle_state = PolymarketPriceFeed::try_from_slice(&oracle_account.data.borrow())?;
    
    // Validate price movement (2% clamp per slot)
    if !oracle_state.prices.is_empty() {
        let slots_elapsed = clock.slot.saturating_sub(oracle_state.last_update as u64 / SLOT_DURATION);
        let max_change_bps = PRICE_CLAMP_PER_SLOT_BPS * slots_elapsed;
        
        for (i, &new_price) in prices.iter().enumerate() {
            if i < oracle_state.prices.len() {
                let old_price = oracle_state.prices[i];
                let change_bps = if new_price > old_price {
                    ((new_price - old_price) * 10000) / old_price
                } else {
                    ((old_price - new_price) * 10000) / old_price
                };
                
                if change_bps > max_change_bps {
                    msg!("Price movement exceeds clamp: {}bps > {}bps", change_bps, max_change_bps);
                    return Err(BettingPlatformError::PriceManipulation.into());
                }
            }
        }
    }
    
    // Update oracle state
    oracle_state.prices = prices;
    oracle_state.last_update = clock.unix_timestamp;
    oracle_state.total_volume = oracle_state.total_volume.saturating_add(volume);
    oracle_state.status = OracleStatus::Active;
    
    // Serialize back
    oracle_state.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    
    msg!("Polymarket price updated for market: {}", market_id);
    
    Ok(())
}

/// Get Polymarket price
pub fn get_polymarket_price(
    oracle_account: &AccountInfo,
    outcome: u8,
) -> Result<u64, ProgramError> {
    let oracle_state = PolymarketPriceFeed::try_from_slice(&oracle_account.data.borrow())?;
    
    // Check if oracle is active
    if oracle_state.status != OracleStatus::Active {
        return Err(BettingPlatformError::OracleNotActive.into());
    }
    
    // Check staleness (max 5 minutes)
    let clock = Clock::get()?;
    let time_since_update = clock.unix_timestamp - oracle_state.last_update;
    if time_since_update > MAX_ORACLE_STALENESS {
        return Err(BettingPlatformError::StaleOracle.into());
    }
    
    // Get price for outcome
    oracle_state.prices.get(outcome as usize)
        .copied()
        .ok_or(BettingPlatformError::InvalidOutcome.into())
}

/// Halt oracle for emergency
pub fn halt_oracle(
    oracle_account: &AccountInfo,
    authority: &AccountInfo,
) -> ProgramResult {
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut oracle_state = PolymarketPriceFeed::try_from_slice(&oracle_account.data.borrow())?;
    oracle_state.status = OracleStatus::Halted;
    oracle_state.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    
    msg!("Oracle halted for market: {}", oracle_state.market_id);
    
    Ok(())
}

/// Resume oracle after halt
pub fn resume_oracle(
    oracle_account: &AccountInfo,
    authority: &AccountInfo,
) -> ProgramResult {
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut oracle_state = PolymarketPriceFeed::try_from_slice(&oracle_account.data.borrow())?;
    
    if oracle_state.status != OracleStatus::Halted {
        return Err(BettingPlatformError::InvalidOracleState.into());
    }
    
    oracle_state.status = OracleStatus::Active;
    oracle_state.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    
    msg!("Oracle resumed for market: {}", oracle_state.market_id);
    
    Ok(())
}

/// Constants
const SLOT_DURATION: u64 = 400; // milliseconds per slot
const PRICE_CLAMP_PER_SLOT_BPS: u64 = 200; // 2% per slot
const MAX_ORACLE_STALENESS: i64 = 300; // 5 minutes

// Helper function
fn next_account_info<'a, 'b>(
    iter: &mut std::slice::Iter<'a, AccountInfo<'b>>,
) -> Result<&'a AccountInfo<'b>, ProgramError> {
    iter.next().ok_or(ProgramError::NotEnoughAccountKeys)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_price_clamp_validation() {
        // Test that 2% change per slot is allowed
        let old_price = 1000;
        let new_price = 1020; // 2% increase
        let change_bps = ((new_price - old_price) * 10000) / old_price;
        assert_eq!(change_bps, 200);
        assert!(change_bps <= PRICE_CLAMP_PER_SLOT_BPS);
        
        // Test that 3% change per slot is rejected
        let new_price = 1030; // 3% increase
        let change_bps = ((new_price - old_price) * 10000) / old_price;
        assert_eq!(change_bps, 300);
        assert!(change_bps > PRICE_CLAMP_PER_SLOT_BPS);
    }
}

/// Polymarket oracle wrapper
pub struct PolymarketOracle;

impl PolymarketOracle {
    /// Get market prices from Polymarket oracle account
    pub fn get_market_prices(
        oracle_account: &AccountInfo,
    ) -> Result<Vec<u64>, ProgramError> {
        let oracle_state = PolymarketPriceFeed::try_from_slice(&oracle_account.data.borrow())?;
        
        if oracle_state.status != OracleStatus::Active {
            return Err(BettingPlatformError::OracleNotActive.into());
        }
        
        Ok(oracle_state.prices)
    }
}

/// Get market prices helper function
pub fn get_market_prices(
    oracle_account: &AccountInfo,
) -> Result<Vec<u64>, ProgramError> {
    PolymarketOracle::get_market_prices(oracle_account)
}

/// Oracle price struct
#[derive(Debug, Clone)]
pub struct OraclePrice {
    pub source: String,
    pub outcome: u8,
    pub price: u64,
    pub timestamp: i64,
    pub confidence: u8,
}

/// Rate limiter for oracle updates
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct RateLimiter {
    pub requests_per_window: u32,
    pub window_duration: i64,
    pub current_requests: u32,
    pub window_start: i64,
}

impl RateLimiter {
    pub fn new(requests_per_window: u32, window_duration: i64) -> Self {
        Self {
            requests_per_window,
            window_duration,
            current_requests: 0,
            window_start: 0,
        }
    }
    
    pub fn check_rate_limit(&mut self, current_time: i64) -> Result<(), ProgramError> {
        // Reset window if expired
        if current_time - self.window_start >= self.window_duration {
            self.current_requests = 0;
            self.window_start = current_time;
        }
        
        // Check if under limit
        if self.current_requests >= self.requests_per_window {
            return Err(BettingPlatformError::RateLimitExceeded.into());
        }
        
        self.current_requests += 1;
        Ok(())
    }
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub markets_per_window: u32,
    pub orders_per_window: u32,
    pub window_duration: i64,
}