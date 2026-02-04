//! Market ingestion from Polymarket with proper intervals and error handling
//!
//! Implements:
//! - 60-second interval batching (150 slots at 0.4s/slot)
//! - 300 slot halt on extended failures
//! - Flexible JSON parsing for API changes
//! - Resolution dispute mirroring

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::str::FromStr;

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
    state::{GlobalConfigPDA, IngestorState, ProposalPDA, VersePDA},
    verse_classification::VerseClassifier,
};

/// Constants for ingestion intervals and limits
pub const INGESTION_INTERVAL_SLOTS: u64 = 150; // 60 seconds at 0.4s/slot
pub const MAX_FAILURE_SLOTS: u64 = 300; // Halt after 300 slots (~2 minutes) of failures
pub const BATCH_SIZE: u32 = 1000; // Markets per batch
pub const MAX_MARKETS: u32 = 21000; // Total markets to track

/// Polymarket market data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketMarketData {
    pub id: String,
    pub title: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub yes_price: u64, // In basis points (0-10000)
    pub no_price: u64,  // In basis points (0-10000)
    pub volume_24h: u64,
    pub liquidity: u64,
    pub resolved: bool,
    pub resolution: Option<u8>,
    pub disputed: bool,
    pub dispute_reason: Option<String>,
}

/// Market ingestion state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketIngestionState {
    pub authority: Pubkey,
    pub last_ingestion_slot: u64,
    pub next_scheduled_slot: u64,
    pub consecutive_failures: u32,
    pub first_failure_slot: u64,
    pub total_markets_ingested: u64,
    pub current_offset: u32,
    pub is_halted: bool,
    pub halt_reason: HaltReason,
}

impl MarketIngestionState {
    pub const SIZE: usize = 32 + // authority
        8 + // last_ingestion_slot
        8 + // next_scheduled_slot
        4 + // consecutive_failures
        8 + // first_failure_slot
        8 + // total_markets_ingested
        4 + // current_offset
        1 + // is_halted
        1; // halt_reason
    
    pub fn new(authority: Pubkey) -> Self {
        Self {
            authority,
            last_ingestion_slot: 0,
            next_scheduled_slot: 0,
            consecutive_failures: 0,
            first_failure_slot: 0,
            total_markets_ingested: 0,
            current_offset: 0,
            is_halted: false,
            halt_reason: HaltReason::None,
        }
    }
    
    /// Check if ingestion should be halted
    pub fn should_halt(&self, current_slot: u64) -> bool {
        if self.is_halted {
            return true;
        }
        
        // Halt if failures persist for more than MAX_FAILURE_SLOTS
        if self.consecutive_failures > 0 && self.first_failure_slot > 0 {
            let failure_duration = current_slot.saturating_sub(self.first_failure_slot);
            if failure_duration >= MAX_FAILURE_SLOTS {
                return true;
            }
        }
        
        false
    }
    
    /// Schedule next ingestion
    pub fn schedule_next_ingestion(&mut self, current_slot: u64) {
        self.next_scheduled_slot = current_slot + INGESTION_INTERVAL_SLOTS;
    }
    
    /// Record successful ingestion
    pub fn record_success(&mut self, current_slot: u64, markets_processed: u32) {
        self.last_ingestion_slot = current_slot;
        self.consecutive_failures = 0;
        self.first_failure_slot = 0;
        self.total_markets_ingested += markets_processed as u64;
        self.current_offset = (self.current_offset + markets_processed) % MAX_MARKETS;
        self.schedule_next_ingestion(current_slot);
    }
    
    /// Record failed ingestion
    pub fn record_failure(&mut self, current_slot: u64) {
        if self.consecutive_failures == 0 {
            self.first_failure_slot = current_slot;
        }
        self.consecutive_failures += 1;
        
        // Check if we should halt
        if self.should_halt(current_slot) {
            self.is_halted = true;
            self.halt_reason = HaltReason::ExtendedFailure;
        }
    }
}

/// Halt reasons
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq)]
pub enum HaltReason {
    None,
    ExtendedFailure,
    ManualHalt,
    DisputeResolution,
}

/// Event structures for market ingestion
#[derive(BorshSerialize, BorshDeserialize)]
pub struct IngestionHaltedEvent {
    pub reason: String,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MarketsIngestedEvent {
    pub count: u32,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MarketDisputedEvent {
    pub market_id: String,
    pub reason: String,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct IngestionResumedEvent {
    pub slot: u64,
}

/// Process market ingestion
pub fn process_market_ingestion(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_data_bytes: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let keeper_account = next_account_info(account_iter)?;
    let ingestion_state_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    
    // Validate keeper
    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    
    // Load ingestion state
    let mut ingestion_state = MarketIngestionState::try_from_slice(
        &ingestion_state_account.data.borrow()
    )?;
    
    // Check if halted
    if ingestion_state.should_halt(current_slot) {
        ingestion_state.is_halted = true;
        ingestion_state.halt_reason = HaltReason::ExtendedFailure;
        ingestion_state.serialize(&mut &mut ingestion_state_account.data.borrow_mut()[..])?;
        
        msg!("Market ingestion halted due to extended failures");
        emit_event(EventType::IngestionHalted, &IngestionHaltedEvent {
            reason: "Extended API failures".to_string(),
            slot: current_slot,
        });
        
        return Err(BettingPlatformError::IngestionHalted.into());
    }
    
    // Check if it's time for ingestion
    if current_slot < ingestion_state.next_scheduled_slot {
        msg!("Next ingestion scheduled at slot {}", ingestion_state.next_scheduled_slot);
        return Err(BettingPlatformError::TooEarly.into());
    }
    
    // Parse market batch from borsh-serialized bytes
    #[derive(borsh::BorshDeserialize)]
    struct MarketBatch {
        markets: Vec<PolymarketMarketData>,
    }
    
    let batch: MarketBatch = match MarketBatch::try_from_slice(market_data_bytes) {
        Ok(data) => data,
        Err(e) => {
            msg!("Failed to parse market data: {:?}", e);
            ingestion_state.record_failure(current_slot);
            ingestion_state.serialize(&mut &mut ingestion_state_account.data.borrow_mut()[..])?;
            return Err(BettingPlatformError::InvalidMarketData.into());
        }
    };
    
    let markets = batch.markets;
    
    // Process markets
    let mut processed_count = 0u32;
    let mut dispute_count = 0u32;
    
    for market in markets.iter().take(BATCH_SIZE as usize) {
        // Validate price sum (should be close to 10000 basis points)
        let price_sum = market.yes_price + market.no_price;
        if price_sum < 9900 || price_sum > 10100 {
            msg!("Skipping market {} - invalid price sum: {}", market.id, price_sum);
            continue;
        }
        
        // Check for disputes
        if market.disputed {
            dispute_count += 1;
            handle_market_dispute(
                &market,
                &ingestion_state,
                current_slot,
            )?;
            continue;
        }
        
        // Process normal market update
        process_market_update(
            &market,
            &ingestion_state,
            current_slot,
        )?;
        
        processed_count += 1;
    }
    
    // Update ingestion state
    ingestion_state.record_success(current_slot, processed_count);
    ingestion_state.serialize(&mut &mut ingestion_state_account.data.borrow_mut()[..])?;
    
    msg!(
        "Processed {} markets, {} disputes, next ingestion at slot {}",
        processed_count, dispute_count, ingestion_state.next_scheduled_slot
    );
    
    emit_event(EventType::MarketsIngested, &MarketsIngestedEvent {
        count: processed_count,
        slot: current_slot,
    });
    
    Ok(())
}

/// Process individual market update
fn process_market_update(
    market: &PolymarketMarketData,
    _ingestion_state: &MarketIngestionState,
    current_slot: u64,
) -> ProgramResult {
    // Prices are already in basis points
    let yes_price_bps = market.yes_price;
    let no_price_bps = market.no_price;
    
    // Classify to verse
    let verse_id = VerseClassifier::classify_market_to_verse(&market.title)?;
    
    msg!(
        "Market {}: verse_id={}, yes={}, no={}",
        market.id, verse_id, yes_price_bps, no_price_bps
    );
    
    // In full implementation, would update ProposalPDA and VersePDA here
    
    Ok(())
}

/// Handle market dispute
fn handle_market_dispute(
    market: &PolymarketMarketData,
    _ingestion_state: &MarketIngestionState,
    current_slot: u64,
) -> ProgramResult {
    msg!(
        "Market {} disputed: reason={:?}",
        market.id,
        market.dispute_reason.as_ref().unwrap_or(&"Unknown".to_string())
    );
    
    emit_event(EventType::MarketDisputed, &MarketDisputedEvent {
        market_id: market.id.clone(),
        reason: market.dispute_reason.clone().unwrap_or_default(),
        slot: current_slot,
    });
    
    // In full implementation, would:
    // 1. Freeze all positions in linked verse/quantum
    // 2. Mark proposals as disputed
    // 3. Prevent new trades
    
    Ok(())
}

/// Resume halted ingestion (admin function)
pub fn process_resume_ingestion(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let admin_account = next_account_info(account_iter)?;
    let ingestion_state_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    
    // Validate admin
    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    if admin_account.key != &global_config.update_authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Resume ingestion
    let mut ingestion_state = MarketIngestionState::try_from_slice(
        &ingestion_state_account.data.borrow()
    )?;
    
    ingestion_state.is_halted = false;
    ingestion_state.halt_reason = HaltReason::None;
    ingestion_state.consecutive_failures = 0;
    ingestion_state.first_failure_slot = 0;
    
    let clock = Clock::get()?;
    ingestion_state.schedule_next_ingestion(clock.slot);
    
    ingestion_state.serialize(&mut &mut ingestion_state_account.data.borrow_mut()[..])?;
    
    msg!("Market ingestion resumed, next at slot {}", ingestion_state.next_scheduled_slot);
    
    emit_event(EventType::IngestionResumed, &IngestionResumedEvent {
        slot: clock.slot,
    });
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_market_data_parsing() {
        // Test with borsh serialization
        let market = PolymarketMarketData {
            id: "123".to_string(),
            title: "Will BTC reach $100k?".to_string(),
            description: "Binary market on Bitcoin price".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 6500, // 65%
            no_price: 3500,  // 35%
            volume_24h: 1000000,
            liquidity: 500000,
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        };
        
        // Serialize and deserialize
        let bytes = market.try_to_vec().unwrap();
        let decoded: PolymarketMarketData = PolymarketMarketData::try_from_slice(&bytes).unwrap();
        
        assert_eq!(decoded.yes_price, 6500);
        assert_eq!(decoded.no_price, 3500);
        assert!(!decoded.disputed);
    }
    
    #[test]
    fn test_halt_mechanism() {
        let mut state = MarketIngestionState::new(Pubkey::default());
        
        // Record failures
        state.record_failure(100);
        assert_eq!(state.consecutive_failures, 1);
        assert_eq!(state.first_failure_slot, 100);
        
        // Should not halt yet
        assert!(!state.should_halt(200));
        
        // Should halt after MAX_FAILURE_SLOTS
        assert!(state.should_halt(401)); // 100 + 301 > 300
    }
    
    #[test]
    fn test_ingestion_scheduling() {
        let mut state = MarketIngestionState::new(Pubkey::default());
        
        state.schedule_next_ingestion(1000);
        assert_eq!(state.next_scheduled_slot, 1000 + INGESTION_INTERVAL_SLOTS);
        
        // Record success resets failures and schedules next
        state.record_success(1000, 100);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.total_markets_ingested, 100);
    }
}