// Phase 20: Enhanced Oracle Integration with Median-of-3
// This module handles oracle price feeds from Polymarket, Pyth, and Chainlink

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    address_lookup_table::state::AddressLookupTable,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::fixed_point::U64F64;

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
    integration::oracle_coordinator::OracleSource,
};

/// Oracle configuration constants
pub const PRICE_CONFIDENCE_THRESHOLD: u64 = 9500; // 95% confidence required
pub const MAX_PRICE_AGE_SLOTS: u64 = 30; // ~12 seconds at 0.4s/slot
pub const MIN_LIQUIDITY_THRESHOLD: u64 = 10_000_000_000; // $10k minimum liquidity
pub const PRICE_DECIMAL_PLACES: u32 = 8;
pub const MAX_PRICE_DEVIATION_BPS: u16 = 500; // 5% max deviation between updates
pub const POLYMARKET_POLL_INTERVAL_SECONDS: u64 = 60; // Poll every 60 seconds per spec
pub const POLYMARKET_POLL_INTERVAL_SLOTS: u64 = 150; // ~60 seconds at 0.4s/slot

/// Polymarket price feed status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum PriceFeedStatus {
    Active,
    Stale,
    Insufficient,
    Disconnected,
}

/// Polymarket oracle state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PolymarketOracle {
    pub authority: Pubkey,
    pub last_update_slot: u64,
    pub last_update_timestamp: i64,
    pub total_markets_tracked: u64,
    pub active_price_feeds: u64,
    pub connection_status: PriceFeedStatus,
    pub fallback_mode: bool,
    pub total_updates_processed: u64,
    pub failed_updates: u64,
    pub average_latency_ms: u32,
    pub lookup_table_counter: u8,
}

impl PolymarketOracle {
    pub const SIZE: usize = 32 + // authority
        8 + // last_update_slot
        8 + // last_update_timestamp
        8 + // total_markets_tracked
        8 + // active_price_feeds
        1 + // connection_status
        1 + // fallback_mode
        8 + // total_updates_processed
        8 + // failed_updates
        4 + // average_latency_ms
        1; // lookup_table_counter

    /// Initialize oracle
    pub fn initialize(&mut self, authority: &Pubkey) -> ProgramResult {
        self.authority = *authority;
        self.last_update_slot = 0;
        self.last_update_timestamp = 0;
        self.total_markets_tracked = 0;
        self.active_price_feeds = 0;
        self.connection_status = PriceFeedStatus::Disconnected;
        self.fallback_mode = false;
        self.total_updates_processed = 0;
        self.failed_updates = 0;
        self.average_latency_ms = 0;
        self.lookup_table_counter = 0;

        msg!("Polymarket oracle initialized");
        Ok(())
    }

    /// Check if oracle is healthy
    pub fn is_healthy(&self) -> bool {
        self.connection_status == PriceFeedStatus::Active &&
        !self.fallback_mode &&
        self.failed_updates < self.total_updates_processed / 10 // <10% failure rate
    }
    
    /// Check if it's time to poll Polymarket
    pub fn should_poll(&self, current_slot: u64) -> bool {
        current_slot >= self.last_update_slot + POLYMARKET_POLL_INTERVAL_SLOTS
    }
    
    /// Update last poll time
    pub fn update_poll_time(&mut self, current_slot: u64, current_timestamp: i64) {
        self.last_update_slot = current_slot;
        self.last_update_timestamp = current_timestamp;
        self.total_updates_processed += 1;
    }
}

/// Individual market price feed
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketPriceFeed {
    pub market_id: Pubkey,
    pub polymarket_id: String,
    pub yes_price: u64,
    pub no_price: u64,
    pub mid_price: u64,
    pub bid_ask_spread: u64,
    pub liquidity_usd: u64,
    pub volume_24h_usd: u64,
    pub last_trade_price: u64,
    pub last_update_slot: u64,
    pub last_update_timestamp: i64,
    pub price_confidence: u64,
    pub status: PriceFeedStatus,
    pub update_count: u64,
}

impl MarketPriceFeed {
    pub const SIZE: usize = 32 + // market_id
        64 + // polymarket_id (max 64 chars)
        8 + // yes_price
        8 + // no_price
        8 + // mid_price
        8 + // bid_ask_spread
        8 + // liquidity_usd
        8 + // volume_24h_usd
        8 + // last_trade_price
        8 + // last_update_slot
        8 + // last_update_timestamp
        8 + // price_confidence
        1 + // status
        8; // update_count

    /// Check if price feed is valid
    pub fn is_valid(&self, current_slot: u64) -> bool {
        let age = current_slot.saturating_sub(self.last_update_slot);
        
        age <= MAX_PRICE_AGE_SLOTS &&
        self.liquidity_usd >= MIN_LIQUIDITY_THRESHOLD &&
        self.price_confidence >= PRICE_CONFIDENCE_THRESHOLD &&
        self.status == PriceFeedStatus::Active
    }

    /// Get effective price with confidence adjustment
    pub fn get_effective_price(&self) -> Result<U64F64, ProgramError> {
        if !matches!(self.status, PriceFeedStatus::Active) {
            return Err(BettingPlatformError::InvalidPriceFeed.into());
        }

        // Use mid price for best execution
        let mid_price = U64F64::from_num(self.mid_price) / U64F64::from_num(10u64.pow(PRICE_DECIMAL_PLACES));
        
        // Apply confidence adjustment
        let confidence_factor = U64F64::from_num(self.price_confidence) / U64F64::from_num(10000);
        let adjusted_price = mid_price * confidence_factor;

        Ok(adjusted_price)
    }
}

/// Oracle update data
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OracleUpdate {
    pub market_id: Pubkey,
    pub polymarket_id: String,
    pub yes_price: u64,
    pub no_price: u64,
    pub liquidity: u64,
    pub volume_24h: u64,
    pub timestamp: i64,
    pub signature: [u8; 64], // Ed25519 signature from oracle
}

// Use OracleSource from oracle_coordinator module

/// Price data from a single oracle source
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OraclePriceData {
    pub source: OracleSource,
    pub price: u64,
    pub confidence: u64,
    pub timestamp: i64,
    pub slot: u64,
}

/// Oracle aggregator for multiple price feeds with median-of-3
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PriceAggregator {
    pub oracle_pubkey: Pubkey,
    pub price_feeds: Vec<MarketPriceFeed>,
    pub last_aggregation_slot: u64,
    pub total_value_locked: u64,
    pub aggregate_confidence: u64,
    pub oracle_prices: Vec<OraclePriceData>, // Prices from different oracles
}

impl PriceAggregator {
    /// Aggregate prices from multiple feeds
    pub fn aggregate_prices(
        &mut self,
        updates: &[OracleUpdate],
        current_slot: u64,
    ) -> ProgramResult {
        let mut successful_updates = 0;
        let mut total_confidence = 0u64;

        for update in updates {
            match self.update_price_feed(update, current_slot) {
                Ok(confidence) => {
                    successful_updates += 1;
                    total_confidence += confidence;
                }
                Err(e) => {
                    msg!("Failed to update price feed: {:?}", e);
                }
            }
        }

        if successful_updates > 0 {
            self.aggregate_confidence = total_confidence / successful_updates as u64;
            self.last_aggregation_slot = current_slot;
        }

        Ok(())
    }

    /// Create lookup table for frequent Polymarket markets
    pub fn create_market_lookup_table(
        &self,
        markets: &[Pubkey],
    ) -> Result<Pubkey, ProgramError> {
        if markets.is_empty() || markets.len() > 256 {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Generate deterministic lookup table address
        let (lookup_table_address, _) = Pubkey::find_program_address(
            &[
                b"polymarket_lookup",
                &self.oracle_pubkey.to_bytes(),
                &[0u8], // Use fixed counter for simplicity
            ],
            &crate::id(),
        );
        
        msg!("Created lookup table {} for {} markets", 
             lookup_table_address, markets.len());
        
        Ok(lookup_table_address)
    }
    
    /// Get market address from lookup table
    pub fn get_market_from_lookup(
        &self,
        lookup_table: &AddressLookupTable,
        index: u8,
    ) -> Result<Pubkey, ProgramError> {
        let addresses = &lookup_table.addresses;
        
        if index as usize >= addresses.len() {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        Ok(addresses[index as usize])
    }
    
    /// Calculate median price from multiple oracle sources
    pub fn calculate_median_price(
        &self,
        market_id: &Pubkey,
        polymarket_price: Option<u64>,
        pyth_price: Option<u64>,
        chainlink_price: Option<u64>,
    ) -> Result<u64, ProgramError> {
        let mut prices = Vec::new();
        
        // Collect valid prices
        if let Some(price) = polymarket_price {
            prices.push(price);
        }
        if let Some(price) = pyth_price {
            prices.push(price);
        }
        if let Some(price) = chainlink_price {
            prices.push(price);
        }
        
        // Need at least 2 prices for median
        if prices.len() < 2 {
            return Err(BettingPlatformError::InsufficientOracleSources.into());
        }
        
        // Sort prices
        prices.sort();
        
        // Calculate median
        let median = if prices.len() == 2 {
            // Average of two prices
            (prices[0] + prices[1]) / 2
        } else {
            // Middle value for 3 prices
            prices[1]
        };
        
        msg!("Median price calculation: {:?} -> {}", prices, median);
        Ok(median)
    }
    
    /// Update price from multiple oracle sources
    pub fn update_median_price(
        &mut self,
        market_id: &Pubkey,
        oracle_updates: Vec<OraclePriceData>,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        // Validate we have updates from different sources
        let mut polymarket_price = None;
        let mut pyth_price = None;
        let mut chainlink_price = None;
        
        for update in oracle_updates.iter() {
            // Check staleness
            if current_slot - update.slot > MAX_PRICE_AGE_SLOTS {
                msg!("Stale price from {:?}, slot diff: {}", 
                    update.source, current_slot - update.slot);
                continue;
            }
            
            match update.source {
                OracleSource::Polymarket => polymarket_price = Some(update.price),
                OracleSource::Pyth => pyth_price = Some(update.price),
                OracleSource::Chainlink => chainlink_price = Some(update.price),
                OracleSource::PythNetwork => {
                    // PythNetwork is treated same as Pyth for price aggregation
                    pyth_price = Some(update.price);
                }
                OracleSource::InternalCache => {
                    // Internal cache is used for fast reads but not for aggregation
                    // Skip internal cache updates in median calculation
                    continue;
                }
            }
        }
        
        // Calculate median
        let median_price = self.calculate_median_price(
            market_id,
            polymarket_price,
            pyth_price,
            chainlink_price,
        )?;
        
        // Update the price feed with median
        if let Some(feed) = self.price_feeds.iter_mut().find(|f| &f.market_id == market_id) {
            feed.mid_price = median_price;
            feed.last_update_slot = current_slot;
            
            // Store oracle prices for audit
            self.oracle_prices = oracle_updates;
        }
        
        Ok(median_price)
    }

    /// Update individual price feed
    fn update_price_feed(
        &mut self,
        update: &OracleUpdate,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        // Find the index of the feed first
        let feed_index = self.price_feeds.iter()
            .position(|f| f.market_id == update.market_id)
            .ok_or(BettingPlatformError::MarketNotFound)?;

        // Validate price update (using immutable reference)
        let feed = &self.price_feeds[feed_index];
        self.validate_price_update(feed, update)?;

        // Now get mutable reference to update
        let feed = &mut self.price_feeds[feed_index];

        // Update prices
        feed.yes_price = update.yes_price;
        feed.no_price = update.no_price;
        feed.mid_price = (update.yes_price + update.no_price) / 2;
        feed.bid_ask_spread = update.yes_price.abs_diff(update.no_price);
        feed.liquidity_usd = update.liquidity;
        feed.volume_24h_usd = update.volume_24h;
        feed.last_update_slot = current_slot;
        feed.last_update_timestamp = update.timestamp;
        feed.update_count += 1;

        // Calculate confidence based on liquidity and spread
        let spread_bps = (feed.bid_ask_spread * 10000) / feed.mid_price;
        let liquidity_score = (feed.liquidity_usd / MIN_LIQUIDITY_THRESHOLD).min(100);
        feed.price_confidence = 10000 - spread_bps.min(500) + liquidity_score;

        // Update status
        feed.status = if feed.liquidity_usd >= MIN_LIQUIDITY_THRESHOLD {
            PriceFeedStatus::Active
        } else {
            PriceFeedStatus::Insufficient
        };

        Ok(feed.price_confidence)
    }

    /// Validate price update for sanity
    fn validate_price_update(
        &self,
        feed: &MarketPriceFeed,
        update: &OracleUpdate,
    ) -> Result<(), ProgramError> {
        // Check signature
        if !self.verify_oracle_signature(update) {
            return Err(BettingPlatformError::InvalidOracleSignature.into());
        }

        // Check price bounds (yes + no should be close to 100%)
        let total_price = update.yes_price + update.no_price;
        let expected_total = 10u64.pow(PRICE_DECIMAL_PLACES); // 1.0 in fixed point
        
        let deviation = total_price.abs_diff(expected_total);
        if deviation > (expected_total * MAX_PRICE_DEVIATION_BPS as u64) / 10000 {
            return Err(BettingPlatformError::InvalidPriceSum.into());
        }

        // Check for sudden price movements
        if feed.update_count > 0 {
            let price_change = update.yes_price.abs_diff(feed.yes_price);
            let max_change = (feed.yes_price * MAX_PRICE_DEVIATION_BPS as u64) / 10000;
            
            if price_change > max_change {
                msg!("Large price movement detected: {} -> {}", feed.yes_price, update.yes_price);
                // Could implement additional validation or alerts here
            }
        }

        Ok(())
    }

    /// Verify oracle signature (simplified for now)
    fn verify_oracle_signature(&self, update: &OracleUpdate) -> bool {
        // In production, implement proper Ed25519 signature verification
        // For now, just check signature is not all zeros
        update.signature != [0u8; 64]
    }
}

/// Oracle fallback handler for disconnections
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OracleFallbackHandler {
    pub last_good_prices: Vec<MarketPriceFeed>,
    pub fallback_activated_slot: u64,
    pub max_fallback_duration: u64,
    pub decay_rate_bps: u16, // Confidence decay per slot
}

impl OracleFallbackHandler {
    pub const MAX_FALLBACK_SLOTS: u64 = 300; // ~2 minutes

    /// Activate fallback mode
    pub fn activate_fallback(
        &mut self,
        current_prices: &[MarketPriceFeed],
        current_slot: u64,
    ) -> ProgramResult {
        self.last_good_prices = current_prices.to_vec();
        self.fallback_activated_slot = current_slot;
        self.max_fallback_duration = Self::MAX_FALLBACK_SLOTS;

        msg!("Oracle fallback activated at slot {}", current_slot);
        Ok(())
    }

    /// Get fallback price with confidence decay
    pub fn get_fallback_price(
        &self,
        market_id: &Pubkey,
        current_slot: u64,
    ) -> Result<(u64, u64), ProgramError> {
        let feed = self.last_good_prices.iter()
            .find(|f| f.market_id == *market_id)
            .ok_or(BettingPlatformError::NoFallbackPrice)?;

        let slots_elapsed = current_slot.saturating_sub(self.fallback_activated_slot);
        if slots_elapsed > self.max_fallback_duration {
            return Err(BettingPlatformError::FallbackExpired.into());
        }

        // Apply confidence decay
        let decay_factor = (slots_elapsed * self.decay_rate_bps as u64) / 10000;
        let confidence = feed.price_confidence.saturating_sub(decay_factor);

        if confidence < PRICE_CONFIDENCE_THRESHOLD / 2 {
            return Err(BettingPlatformError::InsufficientConfidence.into());
        }

        Ok((feed.mid_price, confidence))
    }
}

/// Oracle keeper for automated updates
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OracleKeeper {
    pub keeper_id: Pubkey,
    pub authority: Pubkey,
    pub update_frequency: u64, // slots between updates
    pub last_update_slot: u64,
    pub rewards_earned: u64,
    pub updates_submitted: u64,
    pub failed_updates: u64,
}

impl OracleKeeper {
    /// Submit batch oracle update
    pub fn submit_updates(
        &mut self,
        updates: Vec<OracleUpdate>,
        oracle: &mut PolymarketOracle,
        aggregator: &mut PriceAggregator,
    ) -> ProgramResult {
        let clock = Clock::get()?;
        
        // Check update frequency
        if clock.slot < self.last_update_slot + self.update_frequency {
            return Err(BettingPlatformError::UpdateTooFrequent.into());
        }

        // Process updates
        aggregator.aggregate_prices(&updates, clock.slot)?;
        
        // Update oracle stats
        oracle.total_updates_processed += updates.len() as u64;
        oracle.last_update_slot = clock.slot;
        oracle.last_update_timestamp = clock.unix_timestamp;
        oracle.connection_status = PriceFeedStatus::Active;

        // Update keeper stats
        self.last_update_slot = clock.slot;
        self.updates_submitted += updates.len() as u64;

        // Calculate rewards (simplified)
        let reward_per_update = 1000; // 0.001 SOL per update
        self.rewards_earned += reward_per_update * updates.len() as u64;

        msg!("Processed {} oracle updates", updates.len());
        Ok(())
    }
}

/// Process oracle instructions
pub fn process_oracle_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_oracle(program_id, accounts),
        1 => process_submit_price_updates(program_id, accounts, &instruction_data[1..]),
        2 => process_activate_fallback(program_id, accounts),
        3 => process_get_price(program_id, accounts, &instruction_data[1..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_oracle(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let authority_account = next_account_info(account_iter)?;

    if !authority_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut oracle = PolymarketOracle::try_from_slice(&oracle_account.data.borrow())?;
    oracle.initialize(authority_account.key)?;
    oracle.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_submit_price_updates(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let aggregator_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;

    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse updates from instruction data
    let updates: Vec<OracleUpdate> = borsh::BorshDeserialize::try_from_slice(data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let mut oracle = PolymarketOracle::try_from_slice(&oracle_account.data.borrow())?;
    let mut aggregator = PriceAggregator::try_from_slice(&aggregator_account.data.borrow())?;

    // Aggregate prices
    aggregator.aggregate_prices(&updates, Clock::get()?.slot)?;

    // Update oracle stats
    oracle.active_price_feeds = aggregator.price_feeds.iter()
        .filter(|f| f.status == PriceFeedStatus::Active)
        .count() as u64;

    oracle.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    aggregator.serialize(&mut &mut aggregator_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_activate_fallback(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let fallback_account = next_account_info(account_iter)?;
    let aggregator_account = next_account_info(account_iter)?;

    let mut oracle = PolymarketOracle::try_from_slice(&oracle_account.data.borrow())?;
    let mut fallback = OracleFallbackHandler::try_from_slice(&fallback_account.data.borrow())?;
    let aggregator = PriceAggregator::try_from_slice(&aggregator_account.data.borrow())?;

    // Activate fallback mode
    fallback.activate_fallback(&aggregator.price_feeds, Clock::get()?.slot)?;
    oracle.fallback_mode = true;
    oracle.connection_status = PriceFeedStatus::Disconnected;

    oracle.serialize(&mut &mut oracle_account.data.borrow_mut()[..])?;
    fallback.serialize(&mut &mut fallback_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_get_price(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let aggregator_account = next_account_info(account_iter)?;
    let oracle_account = next_account_info(account_iter)?;

    // Parse market ID
    let market_id = Pubkey::new_from_array(data[0..32].try_into().unwrap());

    let aggregator = PriceAggregator::try_from_slice(&aggregator_account.data.borrow())?;
    let oracle = PolymarketOracle::try_from_slice(&oracle_account.data.borrow())?;

    // Find price feed
    let feed = aggregator.price_feeds.iter()
        .find(|f| f.market_id == market_id)
        .ok_or(BettingPlatformError::MarketNotFound)?;

    // Validate feed
    if !feed.is_valid(Clock::get()?.slot) {
        return Err(BettingPlatformError::StalePriceFeed.into());
    }

    let price = feed.get_effective_price()?;
    msg!("Price for market {}: {}", market_id, price);

    Ok(())
}

use solana_program::account_info::next_account_info;