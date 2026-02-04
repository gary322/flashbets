//! Funding Rate Calculation and Management
//!
//! Handles funding rate calculations and payments for perpetual positions

use solana_program::{
    clock::{Clock, UnixTimestamp},
    msg,
    program_error::ProgramError,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
};

use super::{
    state::{PerpetualPosition, PerpetualMarket, PositionType},
};

/// Funding rate configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct FundingConfig {
    /// Funding interval in seconds
    pub interval: u64,
    
    /// Maximum funding rate per period (e.g., 0.75%)
    pub max_rate: f64,
    
    /// Minimum funding rate per period (e.g., -0.75%)
    pub min_rate: f64,
    
    /// Premium index smoothing factor (0-1)
    pub smoothing_factor: f64,
    
    /// Clamp threshold for extreme rates
    pub clamp_threshold: f64,
    
    /// Use oracle TWAP for index price
    pub use_twap: bool,
    
    /// Damping factor for large imbalances
    pub damping_factor: f64,
}

/// Funding rate history
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct FundingHistory {
    /// Historical funding rates
    pub rates: Vec<FundingRateEntry>,
    
    /// Maximum history entries
    pub max_entries: usize,
    
    /// Total funding collected
    pub total_collected: i128,
    
    /// Total funding paid
    pub total_paid: i128,
}

/// Single funding rate entry
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct FundingRateEntry {
    pub timestamp: UnixTimestamp,
    pub rate: f64,
    pub premium_index: f64,
    pub mark_price: f64,
    pub index_price: f64,
    pub open_interest_long: u128,
    pub open_interest_short: u128,
}

impl Default for FundingConfig {
    fn default() -> Self {
        Self {
            interval: 3600, // 1 hour
            max_rate: 0.0075, // 0.75%
            min_rate: -0.0075, // -0.75%
            smoothing_factor: 0.3,
            clamp_threshold: 0.02, // 2%
            use_twap: true,
            damping_factor: 0.5,
        }
    }
}

/// Calculate funding rate for market
pub fn calculate_funding_rate(
    market: &PerpetualMarket,
    oracle: &OraclePDA,
    config: &FundingConfig,
) -> f64 {
    // Get index price (from oracle)
    let index_price = if config.use_twap {
        oracle.twap_prob // Use TWAP for stability
    } else {
        oracle.current_prob
    };
    
    // Calculate premium index
    let premium_index = calculate_premium_index(
        market.mark_price,
        index_price,
        config.smoothing_factor,
    );
    
    // Calculate base funding rate
    let base_rate = premium_index * (config.interval as f64) / 86400.0;
    
    // Apply imbalance adjustment
    let imbalance_rate = calculate_imbalance_adjustment(
        market.open_interest_long,
        market.open_interest_short,
        config.damping_factor,
    );
    
    // Combine rates
    let mut funding_rate = base_rate + imbalance_rate;
    
    // Apply volatility adjustment
    let vol_adjustment = oracle.current_sigma * 0.1; // 10% of volatility
    funding_rate *= 1.0 + vol_adjustment;
    
    // Clamp to limits
    funding_rate = funding_rate.max(config.min_rate).min(config.max_rate);
    
    // Apply extreme rate clamping
    if funding_rate.abs() > config.clamp_threshold {
        funding_rate = funding_rate.signum() * config.clamp_threshold;
    }
    
    funding_rate
}

/// Calculate premium index
fn calculate_premium_index(
    mark_price: f64,
    index_price: f64,
    smoothing_factor: f64,
) -> f64 {
    if index_price == 0.0 {
        return 0.0;
    }
    
    let raw_premium = (mark_price - index_price) / index_price;
    
    // Apply smoothing
    raw_premium * smoothing_factor
}

/// Calculate imbalance adjustment
fn calculate_imbalance_adjustment(
    oi_long: u128,
    oi_short: u128,
    damping_factor: f64,
) -> f64 {
    let total_oi = oi_long + oi_short;
    if total_oi == 0 {
        return 0.0;
    }
    
    let imbalance = (oi_long as f64 - oi_short as f64) / total_oi as f64;
    
    // Apply damping to prevent extreme adjustments
    imbalance * damping_factor * 0.001 // 0.1% max adjustment
}

/// Calculate funding payment for a position
pub fn calculate_funding_payment(
    position: &PerpetualPosition,
    funding_rate: f64,
    current_time: UnixTimestamp,
) -> i128 {
    // Calculate time elapsed since last payment
    let time_elapsed = current_time.saturating_sub(position.last_funding_payment);
    if time_elapsed == 0 {
        return 0;
    }
    
    // Calculate funding periods elapsed
    let periods = time_elapsed as f64 / 3600.0; // Hourly funding
    
    // Calculate payment
    let position_value = (position.size as f64) * position.mark_price;
    let payment = position_value * funding_rate * periods;
    
    // Long positions pay when rate is positive, receive when negative
    // Short positions receive when rate is positive, pay when negative
    match position.position_type {
        PositionType::Long => -(payment as i128),
        PositionType::Short => payment as i128,
    }
}

/// Process funding for all positions in market
pub fn process_market_funding(
    market: &mut PerpetualMarket,
    positions: &mut [PerpetualPosition],
    oracle: &OraclePDA,
    config: &FundingConfig,
) -> Result<(), ProgramError> {
    let current_time = Clock::get()?.unix_timestamp;
    
    // Check if funding interval has passed
    if current_time < market.next_funding_time {
        return Ok(());
    }
    
    // Calculate new funding rate
    let funding_rate = calculate_funding_rate(market, oracle, config);
    market.funding_rate = funding_rate;
    market.next_funding_time = current_time + config.interval as i64;
    
    // Process each position
    let mut total_funding_collected = 0i128;
    let mut total_funding_paid = 0i128;
    
    for position in positions.iter_mut() {
        if position.status != super::state::PositionStatus::Active {
            continue;
        }
        
        let payment = calculate_funding_payment(position, funding_rate, current_time);
        position.apply_funding(payment);
        
        if payment > 0 {
            total_funding_collected += payment;
        } else {
            total_funding_paid += payment.abs();
        }
    }
    
    // Update market statistics
    market.insurance_fund += (total_funding_collected - total_funding_paid).max(0) as u128;
    
    msg!("Processed funding: rate={:.6}, collected={}, paid={}", 
         funding_rate, total_funding_collected, total_funding_paid);
    
    Ok(())
}

/// Update funding history
pub fn update_funding_history(
    history: &mut FundingHistory,
    market: &PerpetualMarket,
    oracle: &OraclePDA,
) {
    let entry = FundingRateEntry {
        timestamp: Clock::get().unwrap().unix_timestamp,
        rate: market.funding_rate,
        premium_index: calculate_premium_index(
            market.mark_price,
            oracle.current_prob,
            0.3,
        ),
        mark_price: market.mark_price,
        index_price: oracle.current_prob,
        open_interest_long: market.open_interest_long,
        open_interest_short: market.open_interest_short,
    };
    
    history.rates.push(entry);
    
    // Limit history size
    if history.rates.len() > history.max_entries {
        history.rates.remove(0);
    }
}

/// Calculate average funding rate over period
pub fn calculate_average_funding_rate(
    history: &FundingHistory,
    periods: usize,
) -> f64 {
    if history.rates.is_empty() {
        return 0.0;
    }
    
    let start_idx = history.rates.len().saturating_sub(periods);
    let recent_rates: Vec<f64> = history.rates[start_idx..]
        .iter()
        .map(|e| e.rate)
        .collect();
    
    if recent_rates.is_empty() {
        return 0.0;
    }
    
    recent_rates.iter().sum::<f64>() / recent_rates.len() as f64
}

/// Predict next funding rate
pub fn predict_next_funding_rate(
    market: &PerpetualMarket,
    oracle: &OraclePDA,
    config: &FundingConfig,
) -> f64 {
    // Simple prediction based on current conditions
    let predicted_rate = calculate_funding_rate(market, oracle, config);
    
    // Apply momentum factor
    let momentum = (market.mark_price - oracle.current_prob) / oracle.current_prob;
    let momentum_adjustment = momentum * 0.2; // 20% weight
    
    (predicted_rate + momentum_adjustment)
        .max(config.min_rate)
        .min(config.max_rate)
}

/// Calculate funding APR
pub fn calculate_funding_apr(funding_rate: f64, interval: u64) -> f64 {
    let periods_per_year = 31536000.0 / interval as f64;
    funding_rate * periods_per_year
}

/// Estimate funding cost for position
pub fn estimate_funding_cost(
    position: &PerpetualPosition,
    funding_rate: f64,
    hold_duration: u64, // in seconds
) -> i128 {
    let periods = hold_duration as f64 / 3600.0; // Hourly funding
    let position_value = (position.size as f64) * position.mark_price;
    let total_cost = position_value * funding_rate * periods;
    
    match position.position_type {
        PositionType::Long => -(total_cost as i128),
        PositionType::Short => total_cost as i128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_premium_index_calculation() {
        let premium = calculate_premium_index(101.0, 100.0, 0.3);
        assert_eq!(premium, 0.003); // 0.3% premium
        
        let premium = calculate_premium_index(99.0, 100.0, 0.3);
        assert_eq!(premium, -0.003); // -0.3% discount
    }
    
    #[test]
    fn test_imbalance_adjustment() {
        let adjustment = calculate_imbalance_adjustment(1000, 1000, 0.5);
        assert_eq!(adjustment, 0.0); // Balanced
        
        let adjustment = calculate_imbalance_adjustment(1500, 500, 0.5);
        assert_eq!(adjustment, 0.00025); // Long heavy
        
        let adjustment = calculate_imbalance_adjustment(500, 1500, 0.5);
        assert_eq!(adjustment, -0.00025); // Short heavy
    }
    
    #[test]
    fn test_funding_payment() {
        let mut position = PerpetualPosition::new(
            1,
            solana_program::pubkey::Pubkey::new_unique(),
            1,
            solana_program::pubkey::Pubkey::new_unique(),
            PositionType::Long,
            100.0,
            10000,
            10,
            1000,
        );
        
        position.last_funding_payment = 0;
        let payment = calculate_funding_payment(&position, 0.001, 3600);
        assert!(payment < 0); // Long pays positive rate
        
        position.position_type = PositionType::Short;
        let payment = calculate_funding_payment(&position, 0.001, 3600);
        assert!(payment > 0); // Short receives positive rate
    }
    
    #[test]
    fn test_funding_apr() {
        let apr = calculate_funding_apr(0.0001, 3600); // 0.01% per hour
        assert!((apr - 0.876).abs() < 0.001); // ~87.6% APR
    }
}