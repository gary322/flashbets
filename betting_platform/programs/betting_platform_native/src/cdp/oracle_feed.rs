//! Oracle Feed Integration for CDPs
//!
//! Connects CDP system to oracle price feeds

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    oracle::{OraclePDA, PythClient, FallbackHandler, MAX_PROB_LATENCY_SLOTS},
    constants::*,
};

use super::state::CollateralType;

/// CDP Oracle feed
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CDPOracleFeed {
    /// Oracle account
    pub oracle_account: Pubkey,
    
    /// Current price
    pub current_price: f64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Price confidence
    pub confidence: f64,
    
    /// Is valid
    pub is_valid: bool,
    
    /// Price history (for TWAP)
    pub price_history: Vec<PricePoint>,
    
    /// TWAP price
    pub twap_price: f64,
    
    /// Volatility (sigma)
    pub volatility: f64,
}

/// Price point in history
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PricePoint {
    pub price: f64,
    pub slot: u64,
    pub timestamp: i64,
}

/// Price feed for collateral
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PriceFeed {
    /// Collateral type
    pub collateral_type: CollateralType,
    
    /// Price in USD
    pub price_usd: f64,
    
    /// Last update
    pub last_update: i64,
    
    /// Is stale
    pub is_stale: bool,
    
    /// Confidence interval
    pub confidence: f64,
}

impl CDPOracleFeed {
    pub fn new(oracle_account: Pubkey) -> Self {
        Self {
            oracle_account,
            current_price: 0.0,
            last_update_slot: 0,
            confidence: 0.0,
            is_valid: false,
            price_history: Vec::new(),
            twap_price: 0.0,
            volatility: 0.0,
        }
    }
    
    /// Update from oracle
    pub fn update_from_oracle(
        &mut self,
        oracle_pda: &OraclePDA,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check freshness
        let slots_elapsed = current_slot.saturating_sub(oracle_pda.last_update_slot);
        if slots_elapsed > MAX_PROB_LATENCY_SLOTS {
            self.is_valid = false;
            msg!("Oracle data stale: {} slots old", slots_elapsed);
            return Err(BettingPlatformError::StaleOracle.into());
        }
        
        // Update price
        self.current_price = oracle_pda.current_prob;
        self.last_update_slot = current_slot;
        self.confidence = 1.0 - oracle_pda.current_sigma; // Higher sigma = lower confidence
        self.is_valid = true;
        self.volatility = oracle_pda.current_sigma;
        
        // Add to history
        let price_point = PricePoint {
            price: self.current_price,
            slot: current_slot,
            timestamp: Clock::get()?.unix_timestamp,
        };
        
        self.price_history.push(price_point);
        
        // Keep only last 100 points
        if self.price_history.len() > 100 {
            self.price_history.remove(0);
        }
        
        // Calculate TWAP
        self.calculate_twap();
        
        Ok(())
    }
    
    /// Calculate time-weighted average price
    fn calculate_twap(&mut self) {
        if self.price_history.len() < 2 {
            self.twap_price = self.current_price;
            return;
        }
        
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        
        for i in 1..self.price_history.len() {
            let time_diff = self.price_history[i].slot - self.price_history[i-1].slot;
            let avg_price = (self.price_history[i].price + self.price_history[i-1].price) / 2.0;
            
            weighted_sum += avg_price * (time_diff as f64);
            total_weight += time_diff as f64;
        }
        
        if total_weight > 0.0 {
            self.twap_price = weighted_sum / total_weight;
        } else {
            self.twap_price = self.current_price;
        }
    }
}

/// Validate oracle price
pub fn validate_oracle_price(
    oracle_account: &AccountInfo,
    expected_oracle: &Pubkey,
    max_staleness: u64,
) -> Result<f64, ProgramError> {
    // Check oracle account
    if oracle_account.key != expected_oracle {
        msg!("Invalid oracle account");
        return Err(BettingPlatformError::InvalidOracle.into());
    }
    
    // Fetch price from oracle
    let (prob, sigma) = PythClient::fetch_prob_sigma(oracle_account)?;
    
    // Check staleness
    let clock = Clock::get()?;
    if clock.slot > max_staleness {
        msg!("Oracle price too stale");
        return Err(BettingPlatformError::StaleOracle.into());
    }
    
    Ok(prob)
}

/// Get collateral value in USD
pub fn get_collateral_value(
    collateral_amount: u128,
    collateral_type: &CollateralType,
    oracle_price: f64,
) -> u128 {
    // Apply collateral-specific price adjustments
    let price_multiplier = match collateral_type {
        CollateralType::USDC => 1.0, // Stable at $1
        CollateralType::SOL => oracle_price,
        CollateralType::BTC => oracle_price * 50000.0, // Example BTC price
        CollateralType::ETH => oracle_price * 3000.0,  // Example ETH price
        CollateralType::SyntheticToken => oracle_price,
    };
    
    ((collateral_amount as f64) * price_multiplier) as u128
}

/// Calculate loan-to-value ratio
pub fn calculate_ltv_ratio(
    debt_amount: u128,
    collateral_value: u128,
) -> f64 {
    if collateral_value == 0 {
        return f64::MAX;
    }
    
    (debt_amount as f64) / (collateral_value as f64)
}

/// Get price feed for collateral type
pub fn get_price_feed(
    collateral_type: &CollateralType,
    oracle_pda: &OraclePDA,
) -> PriceFeed {
    let price_usd = match collateral_type {
        CollateralType::USDC => 1.0,
        CollateralType::SOL => oracle_pda.current_prob * 100.0, // Example SOL price
        CollateralType::BTC => oracle_pda.current_prob * 50000.0,
        CollateralType::ETH => oracle_pda.current_prob * 3000.0,
        CollateralType::SyntheticToken => oracle_pda.current_prob,
    };
    
    PriceFeed {
        collateral_type: collateral_type.clone(),
        price_usd,
        last_update: Clock::get().unwrap().unix_timestamp,
        is_stale: false,
        confidence: 1.0 - oracle_pda.current_sigma,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collateral_value() {
        // USDC should be 1:1
        let value = get_collateral_value(
            1000,
            &CollateralType::USDC,
            1.0,
        );
        assert_eq!(value, 1000);
        
        // SOL with price
        let value = get_collateral_value(
            10,
            &CollateralType::SOL,
            100.0,
        );
        assert_eq!(value, 1000);
    }
    
    #[test]
    fn test_ltv_calculation() {
        let ltv = calculate_ltv_ratio(500, 1000);
        assert_eq!(ltv, 0.5);
        
        let ltv = calculate_ltv_ratio(800, 1000);
        assert_eq!(ltv, 0.8);
        
        // Edge case: no collateral
        let ltv = calculate_ltv_ratio(100, 0);
        assert_eq!(ltv, f64::MAX);
    }
    
    #[test]
    fn test_twap_calculation() {
        let mut feed = CDPOracleFeed::new(Pubkey::default());
        
        // Add price points
        feed.price_history.push(PricePoint {
            price: 100.0,
            slot: 100,
            timestamp: 1000,
        });
        
        feed.price_history.push(PricePoint {
            price: 110.0,
            slot: 110,
            timestamp: 1010,
        });
        
        feed.price_history.push(PricePoint {
            price: 105.0,
            slot: 120,
            timestamp: 1020,
        });
        
        feed.calculate_twap();
        
        // TWAP should be between min and max
        assert!(feed.twap_price >= 100.0 && feed.twap_price <= 110.0);
    }
}