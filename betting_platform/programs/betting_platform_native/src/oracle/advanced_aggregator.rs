//! Advanced Oracle Aggregation
//!
//! Implements weighted aggregation, outlier detection, and reliability scoring

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
    math::U64F64,
};

/// Maximum number of oracle sources
pub const MAX_ORACLE_SOURCES: usize = 7;

/// Minimum sources required for aggregation
pub const MIN_ORACLE_SOURCES: usize = 3;

/// Outlier detection threshold (standard deviations)
pub const OUTLIER_THRESHOLD: f64 = 2.5;

/// Oracle source with metadata
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OracleSource {
    /// Oracle public key
    pub oracle_id: Pubkey,
    
    /// Oracle type (Pyth, Chainlink, Custom)
    pub oracle_type: OracleType,
    
    /// Current price from this oracle
    pub price: u64,
    
    /// Timestamp of price update
    pub timestamp: i64,
    
    /// Confidence interval (basis points)
    pub confidence_bps: u16,
    
    /// Historical reliability score (0-100)
    pub reliability_score: u8,
    
    /// Number of successful updates
    pub success_count: u64,
    
    /// Number of failed updates
    pub failure_count: u64,
    
    /// Average response time (ms)
    pub avg_response_time: u32,
}

/// Oracle type enumeration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OracleType {
    Pyth,
    Chainlink,
    Switchboard,
    Custom,
}

/// Advanced oracle aggregator
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AdvancedOracleAggregator {
    /// Market ID being tracked
    pub market_id: [u8; 32],
    
    /// Active oracle sources
    pub sources: Vec<OracleSource>,
    
    /// Current aggregated price
    pub aggregated_price: u64,
    
    /// Aggregation timestamp
    pub last_update: i64,
    
    /// Aggregation method
    pub method: AggregationMethod,
    
    /// Price history for TWAP
    pub price_history: Vec<PricePoint>,
    
    /// Outlier detection enabled
    pub outlier_detection: bool,
    
    /// Minimum confidence threshold (basis points)
    pub min_confidence_bps: u16,
}

/// Aggregation methods
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum AggregationMethod {
    /// Simple median
    Median,
    
    /// Weighted average by reliability
    WeightedAverage,
    
    /// Time-weighted average price
    TWAP { window_slots: u64 },
    
    /// Volume-weighted average price
    VWAP { window_slots: u64 },
    
    /// Trimmed mean (remove outliers)
    TrimmedMean { trim_percent: u8 },
}

/// Historical price point
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct PricePoint {
    pub slot: u64,
    pub price: u64,
    pub volume: u64,
}

impl AdvancedOracleAggregator {
    /// Create new aggregator
    pub fn new(market_id: [u8; 32], method: AggregationMethod) -> Self {
        Self {
            market_id,
            sources: Vec::with_capacity(MAX_ORACLE_SOURCES),
            aggregated_price: 0,
            last_update: 0,
            method,
            price_history: Vec::with_capacity(100),
            outlier_detection: true,
            min_confidence_bps: 100, // 1% minimum confidence
        }
    }
    
    /// Add or update oracle source
    pub fn add_source(&mut self, source: OracleSource) -> Result<(), ProgramError> {
        if self.sources.len() >= MAX_ORACLE_SOURCES {
            return Err(BettingPlatformError::TooManyOracleSources.into());
        }
        
        // Update if exists, otherwise add
        if let Some(existing) = self.sources.iter_mut().find(|s| s.oracle_id == source.oracle_id) {
            *existing = source;
        } else {
            self.sources.push(source);
        }
        
        Ok(())
    }
    
    /// Update prices from all sources and aggregate
    pub fn update_and_aggregate(&mut self) -> Result<AggregationResult, ProgramError> {
        let clock = Clock::get()?;
        
        // Get valid source indices
        let valid_indices = self.get_valid_source_indices(clock.unix_timestamp)?;
        
        if valid_indices.len() < MIN_ORACLE_SOURCES {
            return Err(BettingPlatformError::InsufficientOracleSources.into());
        }
        
        // Collect valid sources based on indices
        let valid_sources: Vec<OracleSource> = valid_indices.iter()
            .map(|&i| self.sources[i].clone())
            .collect();
        
        // Detect and filter outliers if enabled
        let filtered_indices = if self.outlier_detection {
            self.filter_outliers_indices(&valid_sources)?
        } else {
            (0..valid_sources.len()).collect()
        };
        
        // Collect filtered sources
        let filtered_sources: Vec<OracleSource> = filtered_indices.iter()
            .map(|&i| valid_sources[i].clone())
            .collect();
        
        // Convert to references for existing methods
        let filtered_source_refs: Vec<&OracleSource> = filtered_sources.iter().collect();
        
        // Calculate aggregate confidence before mutation
        let aggregate_confidence = self.calculate_aggregate_confidence(&filtered_source_refs);
        
        // Aggregate based on method
        let aggregated_price = match self.method {
            AggregationMethod::Median => {
                self.aggregate_median(&filtered_source_refs)
            }
            AggregationMethod::WeightedAverage => {
                self.aggregate_weighted(&filtered_source_refs)?
            }
            AggregationMethod::TWAP { window_slots } => {
                self.aggregate_twap(window_slots, clock.slot)?
            }
            AggregationMethod::VWAP { window_slots } => {
                self.aggregate_vwap(window_slots, clock.slot)?
            }
            AggregationMethod::TrimmedMean { trim_percent } => {
                self.aggregate_trimmed_mean(&filtered_source_refs, trim_percent)?
            }
        };
        
        // Update state
        self.aggregated_price = aggregated_price;
        self.last_update = clock.unix_timestamp;
        
        // Add to price history
        self.add_to_history(clock.slot, aggregated_price, 0)?;
        
        Ok(AggregationResult {
            price: aggregated_price,
            confidence_bps: aggregate_confidence,
            sources_used: filtered_sources.len() as u8,
            outliers_removed: valid_sources.len() - filtered_sources.len(),
            timestamp: clock.unix_timestamp,
        })
    }
    
    /// Get valid sources (fresh and reliable) - returns indices instead of references
    fn get_valid_source_indices(&self, current_time: i64) -> Result<Vec<usize>, ProgramError> {
        let mut valid_indices = Vec::new();
        
        for (index, source) in self.sources.iter().enumerate() {
            // Check staleness (max 30 seconds old)
            if current_time - source.timestamp > 30 {
                continue;
            }
            
            // Check confidence
            if source.confidence_bps > self.min_confidence_bps * 10 {
                continue;
            }
            
            // Check reliability score
            if source.reliability_score < 50 {
                continue;
            }
            
            valid_indices.push(index);
        }
        
        Ok(valid_indices)
    }
    
    /// Filter outliers using statistical methods - returns indices
    fn filter_outliers_indices(&self, sources: &[OracleSource]) -> Result<Vec<usize>, ProgramError> {
        if sources.len() < 3 {
            return Ok((0..sources.len()).collect());
        }
        
        // Calculate mean and standard deviation
        let prices: Vec<f64> = sources.iter().map(|s| s.price as f64).collect();
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        
        let variance = prices.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
        let std_dev = variance.sqrt();
        
        // Filter outliers
        let mut filtered_indices = Vec::new();
        for (i, _) in sources.iter().enumerate() {
            let z_score = (prices[i] - mean).abs() / std_dev;
            if z_score <= OUTLIER_THRESHOLD {
                filtered_indices.push(i);
            }
        }
        
        // Ensure minimum sources remain
        if filtered_indices.len() < MIN_ORACLE_SOURCES && sources.len() >= MIN_ORACLE_SOURCES {
            // Return sources sorted by distance from mean
            let mut indexed: Vec<(usize, f64)> = sources.iter()
                .enumerate()
                .map(|(i, _)| (i, (prices[i] - mean).abs()))
                .collect();
            indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            filtered_indices = indexed.iter()
                .take(MIN_ORACLE_SOURCES)
                .map(|(i, _)| *i)
                .collect();
        }
        
        Ok(filtered_indices)
    }
    
    /// Filter outliers using statistical methods - returns filtered sources
    pub fn filter_outliers<'a>(&self, sources: &[&'a OracleSource]) -> Result<Vec<&'a OracleSource>, ProgramError> {
        if sources.len() < 3 {
            return Ok(sources.to_vec());
        }
        
        // Clone sources for processing
        let owned_sources: Vec<OracleSource> = sources.iter().map(|s| (*s).clone()).collect();
        
        let indices = self.filter_outliers_indices(&owned_sources)?;
        Ok(indices.into_iter().map(|i| sources[i]).collect())
    }
    
    /// Aggregate using median
    fn aggregate_median(&self, sources: &[&OracleSource]) -> u64 {
        let mut prices: Vec<u64> = sources.iter().map(|s| s.price).collect();
        prices.sort_unstable();
        
        let mid = prices.len() / 2;
        if prices.len() % 2 == 0 {
            (prices[mid - 1] + prices[mid]) / 2
        } else {
            prices[mid]
        }
    }
    
    /// Aggregate using weighted average
    fn aggregate_weighted(&self, sources: &[&OracleSource]) -> Result<u64, ProgramError> {
        let mut weighted_sum = 0u128;
        let mut weight_sum = 0u128;
        
        for source in sources {
            // Weight = reliability_score * (1 / (1 + avg_response_time/1000))
            let time_factor = 1000u128 / (1000 + source.avg_response_time as u128);
            let weight = source.reliability_score as u128 * time_factor;
            
            weighted_sum += source.price as u128 * weight;
            weight_sum += weight;
        }
        
        if weight_sum == 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok((weighted_sum / weight_sum) as u64)
    }
    
    /// Aggregate using TWAP
    fn aggregate_twap(&self, window_slots: u64, current_slot: u64) -> Result<u64, ProgramError> {
        let cutoff_slot = current_slot.saturating_sub(window_slots);
        
        let relevant_points: Vec<&PricePoint> = self.price_history
            .iter()
            .filter(|p| p.slot >= cutoff_slot)
            .collect();
        
        if relevant_points.is_empty() {
            return Ok(self.aggregated_price);
        }
        
        // Calculate time-weighted average
        let mut weighted_sum = 0u128;
        let mut time_sum = 0u64;
        
        for i in 0..relevant_points.len() {
            let duration = if i + 1 < relevant_points.len() {
                relevant_points[i + 1].slot - relevant_points[i].slot
            } else {
                current_slot - relevant_points[i].slot
            };
            
            weighted_sum += relevant_points[i].price as u128 * duration as u128;
            time_sum += duration;
        }
        
        if time_sum == 0 {
            return Ok(self.aggregated_price);
        }
        
        Ok((weighted_sum / time_sum as u128) as u64)
    }
    
    /// Aggregate using VWAP
    fn aggregate_vwap(&self, window_slots: u64, current_slot: u64) -> Result<u64, ProgramError> {
        let cutoff_slot = current_slot.saturating_sub(window_slots);
        
        let relevant_points: Vec<&PricePoint> = self.price_history
            .iter()
            .filter(|p| p.slot >= cutoff_slot && p.volume > 0)
            .collect();
        
        if relevant_points.is_empty() {
            return Ok(self.aggregated_price);
        }
        
        // Calculate volume-weighted average
        let mut weighted_sum = 0u128;
        let mut volume_sum = 0u128;
        
        for point in relevant_points {
            weighted_sum += point.price as u128 * point.volume as u128;
            volume_sum += point.volume as u128;
        }
        
        if volume_sum == 0 {
            return Ok(self.aggregated_price);
        }
        
        Ok((weighted_sum / volume_sum) as u64)
    }
    
    /// Aggregate using trimmed mean
    fn aggregate_trimmed_mean(&self, sources: &[&OracleSource], trim_percent: u8) -> Result<u64, ProgramError> {
        if trim_percent >= 50 {
            return Err(ProgramError::InvalidArgument);
        }
        
        let mut prices: Vec<u64> = sources.iter().map(|s| s.price).collect();
        prices.sort_unstable();
        
        let trim_count = (prices.len() as f64 * trim_percent as f64 / 100.0).round() as usize;
        let start = trim_count;
        let end = prices.len() - trim_count;
        
        if start >= end {
            return Ok(self.aggregate_median(sources));
        }
        
        let sum: u64 = prices[start..end].iter().sum();
        Ok(sum / (end - start) as u64)
    }
    
    /// Calculate aggregate confidence
    fn calculate_aggregate_confidence(&self, sources: &[&OracleSource]) -> u16 {
        if sources.is_empty() {
            return 0;
        }
        
        // Average confidence weighted by reliability
        let mut weighted_confidence = 0u32;
        let mut weight_sum = 0u32;
        
        for source in sources {
            let weight = source.reliability_score as u32;
            weighted_confidence += source.confidence_bps as u32 * weight;
            weight_sum += weight;
        }
        
        if weight_sum == 0 {
            return 10000; // 100% if no weights
        }
        
        (weighted_confidence / weight_sum) as u16
    }
    
    /// Add price point to history
    fn add_to_history(&mut self, slot: u64, price: u64, volume: u64) -> Result<(), ProgramError> {
        // Maintain rolling window of 100 points
        if self.price_history.len() >= 100 {
            self.price_history.remove(0);
        }
        
        self.price_history.push(PricePoint {
            slot,
            price,
            volume,
        });
        
        Ok(())
    }
    
    /// Update oracle reliability scores based on performance
    pub fn update_reliability_scores(&mut self) -> Result<(), ProgramError> {
        for source in &mut self.sources {
            let total = source.success_count + source.failure_count;
            if total == 0 {
                continue;
            }
            
            // Base score from success rate
            let success_rate = (source.success_count * 100 / total) as u8;
            
            // Penalty for slow response times
            let time_penalty = (source.avg_response_time / 100).min(20) as u8;
            
            // Bonus for low volatility in confidence
            let confidence_bonus = if source.confidence_bps < 50 { 10 } else { 0 };
            
            source.reliability_score = success_rate
                .saturating_sub(time_penalty)
                .saturating_add(confidence_bonus)
                .min(100);
        }
        
        Ok(())
    }
}

/// Aggregation result
#[derive(Debug)]
pub struct AggregationResult {
    pub price: u64,
    pub confidence_bps: u16,
    pub sources_used: u8,
    pub outliers_removed: usize,
    pub timestamp: i64,
}

// From trait already implemented in error.rs, removing duplicate

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_outlier_detection() {
        let mut aggregator = AdvancedOracleAggregator::new([0; 32], AggregationMethod::Median);
        
        // Add sources with one outlier
        let sources = vec![
            OracleSource {
                oracle_id: Pubkey::new_unique(),
                oracle_type: OracleType::Pyth,
                price: 50000,
                timestamp: 100,
                confidence_bps: 50,
                reliability_score: 90,
                success_count: 1000,
                failure_count: 10,
                avg_response_time: 50,
            },
            OracleSource {
                oracle_id: Pubkey::new_unique(),
                oracle_type: OracleType::Chainlink,
                price: 50100,
                timestamp: 100,
                confidence_bps: 60,
                reliability_score: 85,
                success_count: 950,
                failure_count: 50,
                avg_response_time: 75,
            },
            OracleSource {
                oracle_id: Pubkey::new_unique(),
                oracle_type: OracleType::Custom,
                price: 49900,
                timestamp: 100,
                confidence_bps: 40,
                reliability_score: 95,
                success_count: 2000,
                failure_count: 5,
                avg_response_time: 25,
            },
            OracleSource {
                oracle_id: Pubkey::new_unique(),
                oracle_type: OracleType::Custom,
                price: 60000, // Outlier
                timestamp: 100,
                confidence_bps: 100,
                reliability_score: 80,
                success_count: 800,
                failure_count: 200,
                avg_response_time: 100,
            },
        ];
        
        for source in sources {
            aggregator.add_source(source).unwrap();
        }
        
        let valid_sources: Vec<&OracleSource> = aggregator.sources.iter().collect();
        let filtered = aggregator.filter_outliers(&valid_sources).unwrap();
        
        assert_eq!(filtered.len(), 3); // Outlier removed
        assert!(filtered.iter().all(|s| s.price < 55000)); // Outlier was 60000
    }
    
    #[test]
    fn test_weighted_aggregation() {
        let aggregator = AdvancedOracleAggregator::new([0; 32], AggregationMethod::WeightedAverage);
        
        let source1 = OracleSource {
            oracle_id: Pubkey::new_unique(),
            oracle_type: OracleType::Pyth,
            price: 50000,
            timestamp: 100,
            confidence_bps: 50,
            reliability_score: 100, // High reliability
            success_count: 1000,
            failure_count: 0,
            avg_response_time: 10, // Fast
        };
        
        let source2 = OracleSource {
            oracle_id: Pubkey::new_unique(),
            oracle_type: OracleType::Chainlink,
            price: 51000,
            timestamp: 100,
            confidence_bps: 60,
            reliability_score: 50, // Low reliability
            success_count: 500,
            failure_count: 500,
            avg_response_time: 1000, // Slow
        };
        
        let sources = vec![&source1, &source2];
        
        let weighted_price = aggregator.aggregate_weighted(&sources).unwrap();
        
        // Should be closer to 50000 due to higher weight
        assert!(weighted_price < 50500);
    }
}