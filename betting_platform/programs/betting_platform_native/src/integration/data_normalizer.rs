use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
    market_ingestion::PolymarketMarketData,
    synthetics::MarketData,
    math::U64F64,
};
use std::collections::HashMap;

/// Data normalization constants
pub const MAX_BATCH_SIZE: usize = 1000; // Process 1k items per batch
pub const NORMALIZATION_VERSION: u8 = 1;
pub const PRICE_PRECISION: u64 = 1_000_000; // 6 decimal places
pub const VOLUME_PRECISION: u64 = 1_000_000_000; // 9 decimal places

/// Data source types
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Debug)]
pub enum DataSource {
    Polymarket,
    Pyth,
    Chainlink,
    Switchboard,
    Internal,
    Custom(String),
}

/// Normalized market data structure
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct NormalizedMarketData {
    pub market_id: Pubkey,
    pub source: DataSource,
    pub external_id: String,
    pub title: String,
    pub description: String,
    pub outcomes: Vec<NormalizedOutcome>,
    pub prices: NormalizedPrices,
    pub volume: NormalizedVolume,
    pub liquidity: u64,
    pub status: MarketStatus,
    pub metadata: MarketMetadata,
    pub timestamp: i64,
    pub version: u8,
}

/// Normalized outcome data
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct NormalizedOutcome {
    pub index: u8,
    pub name: String,
    pub price: u64,         // In PRICE_PRECISION units
    pub probability: u16,   // Basis points (0-10000)
    pub volume: u64,        // In VOLUME_PRECISION units
}

/// Normalized price data
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct NormalizedPrices {
    pub bid: u64,           // Best bid in PRICE_PRECISION
    pub ask: u64,           // Best ask in PRICE_PRECISION
    pub mid: u64,           // Mid price
    pub last: u64,          // Last traded price
    pub change_24h: i64,    // 24h change in basis points
    pub high_24h: u64,      // 24h high
    pub low_24h: u64,       // 24h low
}

/// Normalized volume data
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct NormalizedVolume {
    pub total_24h: u64,     // In VOLUME_PRECISION units
    pub buy_24h: u64,       // Buy volume
    pub sell_24h: u64,      // Sell volume
    pub trades_24h: u32,    // Number of trades
    pub unique_traders: u32, // Unique trader count
}

/// Market status
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq)]
pub enum MarketStatus {
    Active,
    Paused,
    Resolved,
    Disputed,
    Cancelled,
}

/// Market metadata
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketMetadata {
    pub category: String,
    pub tags: Vec<String>,
    pub resolution_time: Option<i64>,
    pub create_time: i64,
    pub update_time: i64,
    pub dispute_info: Option<DisputeInfo>,
}

/// Dispute information
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct DisputeInfo {
    pub reason: String,
    pub raised_at: i64,
    pub raised_by: Option<Pubkey>,
    pub evidence_url: Option<String>,
}

/// Data validation rules
#[derive(BorshSerialize, BorshDeserialize)]
pub struct ValidationRules {
    pub require_price_sum: bool,        // Prices must sum to ~100%
    pub max_price_deviation: u16,       // Max deviation in basis points
    pub min_liquidity: u64,             // Minimum liquidity required
    pub max_outcome_count: u8,          // Maximum outcomes allowed
    pub require_description: bool,       // Description required
    pub max_title_length: u16,          // Max title length
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            require_price_sum: true,
            max_price_deviation: 200,       // 2% deviation allowed
            min_liquidity: 10_000_000_000,  // $10k minimum
            max_outcome_count: 10,
            require_description: true,
            max_title_length: 200,
        }
    }
}

/// Data normalizer state
#[derive(BorshSerialize, BorshDeserialize)]
pub struct DataNormalizer {
    pub total_processed: u64,
    pub total_errors: u64,
    pub validation_rules: ValidationRules,
    pub source_mappings: HashMap<String, DataSource>,
    pub last_update: i64,
}

impl DataNormalizer {
    pub const SIZE: usize = 8 + // total_processed
        8 + // total_errors
        200 + // validation_rules
        4 + (100 * 50) + // source_mappings (approx)
        8; // last_update

    /// Initialize new normalizer
    pub fn new() -> Self {
        let mut source_mappings = HashMap::new();
        source_mappings.insert("polymarket".to_string(), DataSource::Polymarket);
        source_mappings.insert("pyth".to_string(), DataSource::Pyth);
        source_mappings.insert("chainlink".to_string(), DataSource::Chainlink);
        
        Self {
            total_processed: 0,
            total_errors: 0,
            validation_rules: ValidationRules::default(),
            source_mappings,
            last_update: Clock::get().unwrap().unix_timestamp,
        }
    }

    /// Normalize Polymarket data
    pub fn normalize_polymarket(
        &mut self,
        data: &PolymarketMarketData,
    ) -> Result<NormalizedMarketData, ProgramError> {
        // Validate input
        self.validate_polymarket_data(data)?;
        
        // Create normalized outcomes
        let mut outcomes = Vec::new();
        if data.outcomes.len() >= 2 {
            // Binary market (Yes/No)
            outcomes.push(NormalizedOutcome {
                index: 0,
                name: data.outcomes[0].clone(),
                price: data.yes_price * PRICE_PRECISION / 10000, // Convert from basis points
                probability: data.yes_price as u16,  // Already in basis points, cast to u16
                volume: data.volume_24h / 2, // Split volume estimate
            });
            
            outcomes.push(NormalizedOutcome {
                index: 1,
                name: data.outcomes[1].clone(),
                price: data.no_price * PRICE_PRECISION / 10000,
                probability: data.no_price as u16,  // Already in basis points, cast to u16
                volume: data.volume_24h / 2,
            });
        }
        
        // Calculate normalized prices
        let mid_price = (data.yes_price + data.no_price) / 2;
        let prices = NormalizedPrices {
            bid: (mid_price - 50) * PRICE_PRECISION / 10000, // Estimate spread
            ask: (mid_price + 50) * PRICE_PRECISION / 10000,
            mid: mid_price * PRICE_PRECISION / 10000,
            last: mid_price * PRICE_PRECISION / 10000,
            change_24h: 0, // Would need historical data
            high_24h: (mid_price + 100) * PRICE_PRECISION / 10000,
            low_24h: (mid_price - 100) * PRICE_PRECISION / 10000,
        };
        
        // Create volume data
        let volume = NormalizedVolume {
            total_24h: data.volume_24h,
            buy_24h: data.volume_24h * 45 / 100, // Estimate
            sell_24h: data.volume_24h * 55 / 100,
            trades_24h: (data.volume_24h / 100_000_000) as u32, // Estimate trades
            unique_traders: (data.volume_24h / 1_000_000_000) as u32, // Estimate
        };
        
        // Determine status
        let status = if data.resolved {
            MarketStatus::Resolved
        } else if data.disputed {
            MarketStatus::Disputed
        } else {
            MarketStatus::Active
        };
        
        // Create metadata
        let metadata = MarketMetadata {
            category: self.extract_category(&data.title),
            tags: self.extract_tags(&data.title),
            resolution_time: if data.resolved { 
                Some(Clock::get().unwrap().unix_timestamp) 
            } else { 
                None 
            },
            create_time: Clock::get().unwrap().unix_timestamp - 86400, // Estimate
            update_time: Clock::get().unwrap().unix_timestamp,
            dispute_info: if data.disputed {
                Some(DisputeInfo {
                    reason: data.dispute_reason.clone().unwrap_or_default(),
                    raised_at: Clock::get().unwrap().unix_timestamp,
                    raised_by: None,
                    evidence_url: None,
                })
            } else {
                None
            },
        };
        
        // Create normalized data
        let normalized = NormalizedMarketData {
            market_id: self.generate_market_id(&data.id),
            source: DataSource::Polymarket,
            external_id: data.id.clone(),
            title: data.title.clone(),
            description: data.description.clone(),
            outcomes,
            prices,
            volume,
            liquidity: data.liquidity,
            status,
            metadata,
            timestamp: Clock::get().unwrap().unix_timestamp,
            version: NORMALIZATION_VERSION,
        };
        
        self.total_processed += 1;
        self.last_update = Clock::get().unwrap().unix_timestamp;
        
        Ok(normalized)
    }

    /// Validate Polymarket data
    fn validate_polymarket_data(&self, data: &PolymarketMarketData) -> Result<(), ProgramError> {
        let rules = &self.validation_rules;
        
        // Check price sum
        if rules.require_price_sum {
            let price_sum = data.yes_price + data.no_price;
            let deviation = (price_sum as i32 - 10000).abs() as u16;
            if deviation > rules.max_price_deviation {
                return Err(BettingPlatformError::InvalidPriceSum.into());
            }
        }
        
        // Check liquidity
        if data.liquidity < rules.min_liquidity {
            return Err(BettingPlatformError::InsufficientLiquidity.into());
        }
        
        // Check outcomes
        if data.outcomes.len() > rules.max_outcome_count as usize {
            return Err(BettingPlatformError::TooManyOutcomes.into());
        }
        
        // Check title
        if data.title.len() > rules.max_title_length as usize {
            return Err(BettingPlatformError::TitleTooLong.into());
        }
        
        // Check description
        if rules.require_description && data.description.is_empty() {
            return Err(BettingPlatformError::MissingDescription.into());
        }
        
        Ok(())
    }

    /// Extract category from title
    fn extract_category(&self, title: &str) -> String {
        // Simple category extraction based on keywords
        let title_lower = title.to_lowercase();
        
        if title_lower.contains("election") || title_lower.contains("president") {
            "Politics".to_string()
        } else if title_lower.contains("bitcoin") || title_lower.contains("crypto") {
            "Crypto".to_string()
        } else if title_lower.contains("sports") || title_lower.contains("game") {
            "Sports".to_string()
        } else if title_lower.contains("stock") || title_lower.contains("market") {
            "Finance".to_string()
        } else {
            "General".to_string()
        }
    }

    /// Extract tags from title
    fn extract_tags(&self, title: &str) -> Vec<String> {
        let mut tags = Vec::new();
        let words: Vec<&str> = title.split_whitespace().collect();
        
        for word in words {
            // Extract significant words as tags
            if word.len() > 4 && !Self::is_common_word(word) {
                tags.push(word.to_lowercase());
            }
        }
        
        tags.truncate(5); // Limit to 5 tags
        tags
    }

    /// Check if word is common (should not be a tag)
    fn is_common_word(word: &str) -> bool {
        matches!(word.to_lowercase().as_str(), 
            "will" | "does" | "when" | "what" | "where" | "which" | "there" | "their" | "these" | "those"
        )
    }

    /// Generate deterministic market ID from external ID
    fn generate_market_id(&self, external_id: &str) -> Pubkey {
        use solana_program::hash::hash;
        
        let seed = format!("market:{}", external_id);
        let hash_result = hash(seed.as_bytes());
        
        Pubkey::new_from_array(hash_result.to_bytes())
    }

    /// Batch normalize multiple markets
    pub fn batch_normalize(
        &mut self,
        markets: Vec<PolymarketMarketData>,
    ) -> Result<Vec<NormalizedMarketData>, ProgramError> {
        let mut normalized = Vec::new();
        let mut errors = 0;
        
        for market in markets.iter().take(MAX_BATCH_SIZE) {
            match self.normalize_polymarket(market) {
                Ok(data) => normalized.push(data),
                Err(e) => {
                    msg!("Failed to normalize market {}: {:?}", market.id, e);
                    errors += 1;
                    self.total_errors += 1;
                }
            }
        }
        
        if errors > MAX_BATCH_SIZE / 10 {
            // More than 10% errors
            return Err(BettingPlatformError::TooManyNormalizationErrors.into());
        }
        
        Ok(normalized)
    }

    /// Update validation rules
    pub fn update_rules(&mut self, new_rules: ValidationRules) -> ProgramResult {
        self.validation_rules = new_rules;
        msg!("Updated validation rules");
        Ok(())
    }

    /// Get normalizer statistics
    pub fn get_stats(&self) -> NormalizerStats {
        let error_rate = if self.total_processed > 0 {
            (self.total_errors as f64 / self.total_processed as f64) * 100.0
        } else {
            0.0
        };
        
        NormalizerStats {
            total_processed: self.total_processed,
            total_errors: self.total_errors,
            error_rate,
            last_update: self.last_update,
            sources_configured: self.source_mappings.len() as u32,
        }
    }
}

/// Normalizer statistics
#[derive(Debug)]
pub struct NormalizerStats {
    pub total_processed: u64,
    pub total_errors: u64,
    pub error_rate: f64,
    pub last_update: i64,
    pub sources_configured: u32,
}

/// Transform normalized data for specific use cases
pub struct DataTransformer;

impl DataTransformer {
    /// Convert to internal market format
    pub fn to_internal_market(data: &NormalizedMarketData) -> MarketData {
        MarketData {
            market_id: data.market_id,
            title: data.title.clone(),
            probability: U64F64::from_num(data.outcomes[0].probability as u64), // Convert u16 to u64
            yes_price: data.outcomes[0].price,
            volume_7d: data.volume.total_24h * 7, // Estimate 7d from 24h
            volume_24h: data.volume.total_24h,
            liquidity_depth: data.liquidity,
            liquidity: data.liquidity,
            last_trade_time: data.timestamp,
            category: data.metadata.category.clone(),
            created_at: data.metadata.create_time,
        }
    }
    
    /// Convert to oracle price feed
    pub fn to_price_feed(data: &NormalizedMarketData) -> PriceFeed {
        PriceFeed {
            market_id: data.market_id,
            price: data.prices.mid,
            confidence: calculate_confidence(&data.volume, data.liquidity),
            timestamp: data.timestamp,
            source: data.source.clone(),
        }
    }
    
    /// Convert to analytics event
    pub fn to_analytics_event(data: &NormalizedMarketData) -> AnalyticsEvent {
        AnalyticsEvent {
            market_id: data.market_id,
            event_type: AnalyticsEventType::MarketUpdate,
            volume_24h: data.volume.total_24h,
            trades_24h: data.volume.trades_24h,
            unique_traders: data.volume.unique_traders,
            price_change_24h: data.prices.change_24h,
            timestamp: data.timestamp,
        }
    }
}

/// Price feed structure
#[derive(Debug)]
pub struct PriceFeed {
    pub market_id: Pubkey,
    pub price: u64,
    pub confidence: u16, // Basis points
    pub timestamp: i64,
    pub source: DataSource,
}

/// Analytics event
#[derive(Debug)]
pub struct AnalyticsEvent {
    pub market_id: Pubkey,
    pub event_type: AnalyticsEventType,
    pub volume_24h: u64,
    pub trades_24h: u32,
    pub unique_traders: u32,
    pub price_change_24h: i64,
    pub timestamp: i64,
}

#[derive(Debug)]
pub enum AnalyticsEventType {
    MarketUpdate,
    PriceChange,
    VolumeSpike,
    LiquidityChange,
}

/// Calculate confidence based on volume and liquidity
fn calculate_confidence(volume: &NormalizedVolume, liquidity: u64) -> u16 {
    // Higher volume and liquidity = higher confidence
    let volume_score = (volume.total_24h / 1_000_000_000).min(100) as u16;
    let liquidity_score = (liquidity / 10_000_000_000).min(100) as u16;
    let trade_score = (volume.trades_24h / 100).min(100) as u16;
    
    // Weighted average
    let confidence = (volume_score * 4 + liquidity_score * 4 + trade_score * 2) / 10;
    
    // Convert to basis points (0-10000)
    confidence * 100
}

/// Initialize data normalizer
pub fn initialize_data_normalizer(
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing data normalizer");
    msg!("Normalization version: {}", NORMALIZATION_VERSION);
    msg!("Price precision: {}", PRICE_PRECISION);
    msg!("Volume precision: {}", VOLUME_PRECISION);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polymarket_normalization() {
        let mut normalizer = DataNormalizer::new();
        
        let polymarket_data = PolymarketMarketData {
            id: "test-123".to_string(),
            title: "Will Bitcoin reach $100k by end of 2024?".to_string(),
            description: "This market resolves to YES if Bitcoin...".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 6500, // 65%
            no_price: 3500,  // 35%
            volume_24h: 1_000_000_000_000, // $1M
            liquidity: 500_000_000_000,    // $500k
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        };
        
        let normalized = normalizer.normalize_polymarket(&polymarket_data).unwrap();
        
        assert_eq!(normalized.source, DataSource::Polymarket);
        assert_eq!(normalized.external_id, "test-123");
        assert_eq!(normalized.outcomes.len(), 2);
        assert_eq!(normalized.outcomes[0].probability, 6500);
        assert_eq!(normalized.outcomes[1].probability, 3500);
        assert_eq!(normalized.metadata.category, "Crypto");
    }

    #[test]
    fn test_validation_rules() {
        let normalizer = DataNormalizer::new();
        
        // Test price sum validation
        let mut data = PolymarketMarketData {
            id: "test".to_string(),
            title: "Test Market".to_string(),
            description: "Description".to_string(),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 7000,
            no_price: 2000, // Sum = 9000, too far from 10000
            volume_24h: 1_000_000_000_000,
            liquidity: 100_000_000_000,
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        };
        
        let result = normalizer.validate_polymarket_data(&data);
        assert!(result.is_err());
        
        // Fix price sum
        data.no_price = 3000; // Sum = 10000
        let result = normalizer.validate_polymarket_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_batch_normalization() {
        let mut normalizer = DataNormalizer::new();
        
        let markets = vec![
            PolymarketMarketData {
                id: "market1".to_string(),
                title: "Election Market".to_string(),
                description: "Will candidate X win?".to_string(),
                outcomes: vec!["Yes".to_string(), "No".to_string()],
                yes_price: 5500,
                no_price: 4500,
                volume_24h: 2_000_000_000_000,
                liquidity: 1_000_000_000_000,
                resolved: false,
                resolution: None,
                disputed: false,
                dispute_reason: None,
            },
            PolymarketMarketData {
                id: "market2".to_string(),
                title: "Sports Game Outcome".to_string(),
                description: "Will team Y win?".to_string(),
                outcomes: vec!["Yes".to_string(), "No".to_string()],
                yes_price: 3000,
                no_price: 7000,
                volume_24h: 500_000_000_000,
                liquidity: 200_000_000_000,
                resolved: false,
                resolution: None,
                disputed: false,
                dispute_reason: None,
            },
        ];
        
        let normalized = normalizer.batch_normalize(markets).unwrap();
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].metadata.category, "Politics");
        assert_eq!(normalized[1].metadata.category, "Sports");
    }

    #[test]
    fn test_data_transformer() {
        let normalized = NormalizedMarketData {
            market_id: Pubkey::new_unique(),
            source: DataSource::Polymarket,
            external_id: "test".to_string(),
            title: "Test Market".to_string(),
            description: "Description".to_string(),
            outcomes: vec![
                NormalizedOutcome {
                    index: 0,
                    name: "Yes".to_string(),
                    price: 650_000, // 0.65
                    probability: 6500,
                    volume: 500_000_000_000,
                },
                NormalizedOutcome {
                    index: 1,
                    name: "No".to_string(),
                    price: 350_000, // 0.35
                    probability: 3500,
                    volume: 500_000_000_000,
                },
            ],
            prices: NormalizedPrices {
                bid: 640_000,
                ask: 660_000,
                mid: 650_000,
                last: 650_000,
                change_24h: 100,
                high_24h: 700_000,
                low_24h: 600_000,
            },
            volume: NormalizedVolume {
                total_24h: 1_000_000_000_000,
                buy_24h: 450_000_000_000,
                sell_24h: 550_000_000_000,
                trades_24h: 10_000,
                unique_traders: 1_000,
            },
            liquidity: 500_000_000_000,
            status: MarketStatus::Active,
            metadata: MarketMetadata {
                category: "Test".to_string(),
                tags: vec!["test".to_string()],
                resolution_time: None,
                create_time: 0,
                update_time: 0,
                dispute_info: None,
            },
            timestamp: 0,
            version: NORMALIZATION_VERSION,
        };
        
        // Test transformations
        let internal = DataTransformer::to_internal_market(&normalized);
        // MarketData doesn't have outcomes field, check probability instead
        assert_eq!(internal.probability, U64F64::from_num(6500));
        
        let price_feed = DataTransformer::to_price_feed(&normalized);
        assert_eq!(price_feed.price, 650_000);
        assert!(price_feed.confidence > 0);
        
        let analytics = DataTransformer::to_analytics_event(&normalized);
        assert_eq!(analytics.volume_24h, 1_000_000_000_000);
        assert_eq!(analytics.trades_24h, 10_000);
    }
}