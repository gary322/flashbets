// Polymarket Sole Oracle Implementation
// This module uses only Polymarket as the sole oracle source per specification

use solana_program::{
    account_info::{AccountInfo, next_account_info},
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
        polymarket_oracle::{
            OraclePriceData, MarketPriceFeed, 
            MAX_PRICE_AGE_SLOTS, PRICE_CONFIDENCE_THRESHOLD,
        },
    },
};

/// Type alias for backward compatibility
pub type MedianOracleState = PolymarketOracleState;

/// Polymarket Price Result
#[derive(Debug, Clone)]
pub struct MedianPriceResult {
    pub price: u64,
    pub confidence: u64,
    pub timestamp: i64,
}

/// Polymarket Oracle State - manages Polymarket as sole oracle source
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PolymarketOracleState {
    pub authority: Pubkey,
    pub polymarket_oracle: Pubkey,
    pub last_update_slot: u64,
    pub total_markets: u32,
    pub active_markets: u32,
    pub price_updates: u64,
    pub failed_updates: u64,
    pub halted_markets: u32,
    pub stale_price_flags: u32,
    pub polling_interval_slots: u64, // 60 second intervals in slots
}

impl PolymarketOracleState {
    pub const SIZE: usize = 32 + // authority
        32 + // polymarket_oracle
        8 + // last_update_slot
        4 + // total_markets
        4 + // active_markets
        8 + // price_updates
        8 + // failed_updates
        4 + // halted_markets
        4 + // stale_price_flags
        8; // polling_interval_slots
    
    pub const POLLING_INTERVAL_SECONDS: u64 = 60; // Poll every 60 seconds
    pub const POLLING_INTERVAL_SLOTS: u64 = 150; // ~60 seconds at 400ms/slot
    
    /// Initialize Polymarket oracle state
    pub fn initialize(
        &mut self,
        authority: &Pubkey,
        polymarket_oracle: &Pubkey,
    ) -> ProgramResult {
        self.authority = *authority;
        self.polymarket_oracle = *polymarket_oracle;
        self.last_update_slot = 0;
        self.total_markets = 0;
        self.active_markets = 0;
        self.price_updates = 0;
        self.failed_updates = 0;
        self.halted_markets = 0;
        self.stale_price_flags = 0;
        self.polling_interval_slots = Self::POLLING_INTERVAL_SLOTS;
        
        msg!("Polymarket oracle state initialized as sole oracle source");
        Ok(())
    }
    
    /// Check if polling is due
    pub fn should_poll(&self, current_slot: u64) -> bool {
        current_slot.saturating_sub(self.last_update_slot) >= self.polling_interval_slots
    }
}

/// Polymarket price result with metadata
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct PolymarketPriceResult {
    pub market_id: Pubkey,
    pub price: u64,
    pub yes_price: u64,
    pub no_price: u64,
    pub confidence: u64,
    pub timestamp: i64,
    pub slot: u64,
    pub is_stale: bool,
    pub spread_basis_points: u16, // Deviation from 100% sum
    pub is_halted: bool,
}

impl PolymarketPriceResult {
    pub const SIZE: usize = 32 + // market_id
        8 + // price
        8 + // yes_price
        8 + // no_price
        8 + // confidence
        8 + // timestamp
        8 + // slot
        1 + // is_stale
        2 + // spread_basis_points
        1; // is_halted
        
    pub const MAX_SPREAD_BASIS_POINTS: u16 = 1000; // 10% maximum spread

}

/// Polymarket Oracle Handler
pub struct PolymarketOracleHandler;

impl PolymarketOracleHandler {
    /// Calculate median price from multiple oracle sources
    /// Since Polymarket is the sole oracle, this returns the Polymarket price directly
    pub fn calculate_median_price(
        polymarket_price: Option<u64>,
        _pyth_price: Option<u64>,
        _chainlink_price: Option<u64>,
    ) -> Result<MedianPriceResult, ProgramError> {
        // Polymarket is the sole oracle per specification
        match polymarket_price {
            Some(price) => Ok(MedianPriceResult {
                price,
                confidence: PRICE_CONFIDENCE_THRESHOLD,
                timestamp: 0, // Would be set by caller with actual timestamp
            }),
            None => Err(BettingPlatformError::InsufficientOracleSources.into()),
        }
    }
    /// Get price from Polymarket as sole oracle
    pub fn get_price(
        feed: &MarketPriceFeed,
        current_slot: u64,
    ) -> Result<PolymarketPriceResult, ProgramError> {
        // Check staleness
        let age_slots = current_slot.saturating_sub(feed.last_update_slot);
        let is_stale = age_slots > MAX_PRICE_AGE_SLOTS;
        
        if is_stale {
            msg!("WARNING: Polymarket price stale by {} slots", age_slots);
        }
        
        // Ensure minimum confidence threshold
        if feed.price_confidence < PRICE_CONFIDENCE_THRESHOLD {
            return Err(BettingPlatformError::InsufficientConfidence.into());
        }
        
        // Calculate spread from 100%
        let total = feed.yes_price + feed.no_price;
        let expected_total = 10000; // 100% in basis points
        let spread_basis_points = if total > expected_total {
            ((total - expected_total) * 10000 / expected_total) as u16
        } else {
            ((expected_total - total) * 10000 / expected_total) as u16
        };
        
        // Check if should halt due to excessive spread
        let is_halted = spread_basis_points > PolymarketPriceResult::MAX_SPREAD_BASIS_POINTS;
        
        if is_halted {
            msg!("CRITICAL: Market halted due to {}bp spread", spread_basis_points);
            return Err(BettingPlatformError::ExcessivePriceMovement.into());
        }
        
        Ok(PolymarketPriceResult {
            market_id: feed.market_id,
            price: feed.mid_price,
            yes_price: feed.yes_price,
            no_price: feed.no_price,
            confidence: feed.price_confidence,
            timestamp: feed.last_update_timestamp,
            slot: feed.last_update_slot,
            is_stale,
            spread_basis_points,
            is_halted,
        })
    }
    
    /// Fetch price from Polymarket feed account
    pub fn fetch_price(
        market_id: &Pubkey,
        polymarket_feed_account: &AccountInfo,
        current_slot: u64,
    ) -> Result<PolymarketPriceResult, ProgramError> {
        // Deserialize the feed data
        let feed = MarketPriceFeed::try_from_slice(&polymarket_feed_account.data.borrow())?;
        
        // Validate market ID matches
        if feed.market_id != *market_id {
            return Err(BettingPlatformError::InvalidMarket.into());
        }
        
        // Get price with all validation checks
        Self::get_price(&feed, current_slot)
    }
    
    /// Validate that price is safe to use
    pub fn validate_price(result: &PolymarketPriceResult) -> Result<(), ProgramError> {
        // Check if market is halted
        if result.is_halted {
            return Err(BettingPlatformError::MarketHalted.into());
        }
        
        // Warn if stale but still allow (with flag)
        if result.is_stale {
            msg!("WARNING: Using stale Polymarket price for market {}", result.market_id);
        }
        
        // Validate spread is within bounds
        if result.spread_basis_points > PolymarketPriceResult::MAX_SPREAD_BASIS_POINTS {
            return Err(BettingPlatformError::ExcessivePriceMovement.into());
        }
        
        Ok(())
    }
    
    /// Process price update from Polymarket
    pub fn process_price_update(
        oracle_state: &mut PolymarketOracleState,
        result: &PolymarketPriceResult,
        current_slot: u64,
    ) -> ProgramResult {
        // Update state
        oracle_state.price_updates += 1;
        oracle_state.last_update_slot = current_slot;
        
        if result.is_stale {
            oracle_state.stale_price_flags += 1;
        }
        
        if result.is_halted {
            oracle_state.halted_markets += 1;
        }
        
        msg!("Polymarket price update: market={}, price={}, spread={}bp", 
            result.market_id, result.price, result.spread_basis_points);
        
        Ok(())
    }
}

/// Process Polymarket oracle instructions
pub fn process_polymarket_oracle_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_polymarket_oracle(program_id, accounts),
        1 => process_get_polymarket_price(program_id, accounts, &instruction_data[1..]),
        2 => process_update_oracle_config(program_id, accounts, &instruction_data[1..]),
        3 => process_poll_markets(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_polymarket_oracle(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_state_account = next_account_info(account_iter)?;
    let authority_account = next_account_info(account_iter)?;
    let polymarket_oracle_account = next_account_info(account_iter)?;
    
    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut oracle_state = PolymarketOracleState::try_from_slice(&oracle_state_account.data.borrow())?;
    oracle_state.initialize(
        authority_account.key,
        polymarket_oracle_account.key,
    )?;
    
    oracle_state.serialize(&mut &mut oracle_state_account.data.borrow_mut()[..])?;
    
    Ok(())
}

fn process_get_median_price(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let median_oracle_account = next_account_info(account_iter)?;
    let polymarket_aggregator_account = next_account_info(account_iter)?;
    let pyth_price_account = next_account_info(account_iter)?;
    let chainlink_feed_account = next_account_info(account_iter)?;
    
    // Parse market ID
    let market_id = Pubkey::new_from_array(data[0..32].try_into().unwrap());
    
    let median_oracle = MedianOracleState::try_from_slice(&median_oracle_account.data.borrow())?;
    let current_slot = Clock::get()?.slot;
    
    // Fetch price from Polymarket (sole oracle per spec)
    let polymarket_result = PolymarketOracleHandler::fetch_price(
        &market_id,
        polymarket_aggregator_account,
        current_slot,
    )?;
    
    // Since Polymarket is sole oracle, use its price directly
    let median_result = PolymarketOracleHandler::calculate_median_price(
        Some(polymarket_result.price),
        None, // Pyth not used per spec
        None, // Chainlink not used per spec
    )?;
    
    msg!("Polymarket price for market {}: {} (confidence: {})", 
        market_id, median_result.price, median_result.confidence);
    
    // Update stats
    let mut median_oracle = MedianOracleState::try_from_slice(&median_oracle_account.data.borrow())?;
    median_oracle.price_updates += 1;
    median_oracle.last_update_slot = current_slot;
    median_oracle.serialize(&mut &mut median_oracle_account.data.borrow_mut()[..])?;
    
    Ok(())
}

fn process_get_polymarket_price(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_state_account = next_account_info(account_iter)?;
    let polymarket_feed_account = next_account_info(account_iter)?;
    
    // Parse market ID
    let market_id = Pubkey::new_from_array(data[0..32].try_into().unwrap());
    
    let mut oracle_state = PolymarketOracleState::try_from_slice(&oracle_state_account.data.borrow())?;
    let current_slot = Clock::get()?.slot;
    
    // Fetch price from Polymarket
    let result = PolymarketOracleHandler::fetch_price(
        &market_id,
        polymarket_feed_account,
        current_slot,
    )?;
    
    // Validate price
    PolymarketOracleHandler::validate_price(&result)?;
    
    // Process the update
    PolymarketOracleHandler::process_price_update(
        &mut oracle_state,
        &result,
        current_slot,
    )?;
    
    msg!("Polymarket price for market {}: {} (yes: {}, no: {}, spread: {}bp)", 
        market_id, result.price, result.yes_price, result.no_price, result.spread_basis_points);
    
    // Save state
    oracle_state.serialize(&mut &mut oracle_state_account.data.borrow_mut()[..])?;
    
    Ok(())
}

fn process_poll_markets(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_state_account = next_account_info(account_iter)?;
    
    let mut oracle_state = PolymarketOracleState::try_from_slice(&oracle_state_account.data.borrow())?;
    let current_slot = Clock::get()?.slot;
    
    // Check if polling is due
    if !oracle_state.should_poll(current_slot) {
        msg!("Polling not due yet, {} slots remaining", 
            oracle_state.polling_interval_slots.saturating_sub(current_slot.saturating_sub(oracle_state.last_update_slot)));
        return Ok(());
    }
    
    msg!("Polling markets at slot {}", current_slot);
    oracle_state.last_update_slot = current_slot;
    
    // In production, this would trigger off-chain polling of Polymarket API
    // For now, just update the state
    oracle_state.serialize(&mut &mut oracle_state_account.data.borrow_mut()[..])?;
    
    Ok(())
}

fn process_update_oracle_config(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let authority_account = next_account_info(account_iter)?;
    let median_oracle_account = next_account_info(account_iter)?;
    
    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut median_oracle = MedianOracleState::try_from_slice(&median_oracle_account.data.borrow())?;
    
    if median_oracle.authority != *authority_account.key {
        return Err(BettingPlatformError::UnauthorizedOracleUpdate.into());
    }
    
    // Parse update type
    match data[0] {
        0 => {
            // Update Polymarket oracle (sole oracle per spec)
            let new_oracle = Pubkey::new_from_array(data[1..33].try_into().unwrap());
            median_oracle.polymarket_oracle = new_oracle;
        },
        1 => {
            // Update polling interval
            let new_interval = u64::from_le_bytes(data[1..9].try_into().unwrap());
            median_oracle.polling_interval_slots = new_interval;
        },
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    
    median_oracle.serialize(&mut &mut median_oracle_account.data.borrow_mut()[..])?;
    
    Ok(())
}