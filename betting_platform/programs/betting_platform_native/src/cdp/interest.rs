//! Interest Rate Management for CDPs
//!
//! Handles interest calculation and accrual

use solana_program::{
    clock::Clock,
    msg,
    program_error::ProgramError,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::error::BettingPlatformError;

/// Interest rate model types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum InterestModel {
    /// Fixed rate
    Fixed { rate: f64 },
    
    /// Variable rate based on utilization
    Variable { 
        base: f64,
        slope1: f64,
        slope2: f64,
        kink: f64,
    },
    
    /// Dynamic rate based on oracle
    Dynamic {
        base: f64,
        oracle_multiplier: f64,
        max_rate: f64,
    },
}

/// Interest rate structure
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct InterestRate {
    /// Current rate (annual)
    pub current_rate: f64,
    
    /// Model type
    pub model: InterestModel,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Total interest accrued
    pub total_accrued: u128,
    
    /// Compound frequency (seconds)
    pub compound_frequency: i64,
}

impl InterestRate {
    pub fn new(model: InterestModel) -> Self {
        Self {
            current_rate: 0.05, // 5% default
            model,
            last_update: 0,
            total_accrued: 0,
            compound_frequency: 3600, // Hourly compounding
        }
    }
    
    /// Update interest rate based on model
    pub fn update_rate(
        &mut self,
        utilization: f64,
        oracle_value: Option<f64>,
    ) -> Result<(), ProgramError> {
        self.current_rate = match &self.model {
            InterestModel::Fixed { rate } => *rate,
            
            InterestModel::Variable { base, slope1, slope2, kink } => {
                if utilization <= *kink {
                    base + slope1 * utilization
                } else {
                    base + slope1 * kink + slope2 * (utilization - kink)
                }
            }
            
            InterestModel::Dynamic { base, oracle_multiplier, max_rate } => {
                let oracle_val = oracle_value.unwrap_or(1.0);
                let rate = base * (1.0 + oracle_multiplier * oracle_val);
                rate.min(*max_rate)
            }
        };
        
        self.last_update = Clock::get()?.unix_timestamp;
        
        Ok(())
    }
}

/// Calculate interest for a period
pub fn calculate_interest(
    principal: u128,
    rate: f64,
    time_seconds: i64,
) -> Result<u128, ProgramError> {
    if principal == 0 || time_seconds <= 0 {
        return Ok(0);
    }
    
    let seconds_per_year = 365.25 * 24.0 * 60.0 * 60.0;
    let time_fraction = (time_seconds as f64) / seconds_per_year;
    
    // Simple interest for short periods
    let interest = (principal as f64) * rate * time_fraction;
    
    Ok(interest as u128)
}

/// Accrue interest on a position
pub fn accrue_interest(
    principal: u128,
    rate: &mut InterestRate,
    current_time: i64,
) -> Result<u128, ProgramError> {
    if rate.last_update == 0 || current_time <= rate.last_update {
        rate.last_update = current_time;
        return Ok(0);
    }
    
    let time_elapsed = current_time - rate.last_update;
    let interest = calculate_interest(principal, rate.current_rate, time_elapsed)?;
    
    rate.total_accrued = rate.total_accrued
        .checked_add(interest)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    
    rate.last_update = current_time;
    
    msg!("Accrued {} interest on {} principal", interest, principal);
    
    Ok(interest)
}

/// Compound interest calculation
pub fn compound_interest(
    principal: u128,
    rate: f64,
    time_seconds: i64,
    compound_frequency: i64,
) -> Result<u128, ProgramError> {
    if principal == 0 || time_seconds <= 0 {
        return Ok(0);
    }
    
    let seconds_per_year = 365.25 * 24.0 * 60.0 * 60.0;
    let periods_per_year = seconds_per_year / (compound_frequency as f64);
    let total_periods = (time_seconds / compound_frequency) as f64;
    
    // A = P(1 + r/n)^(nt)
    let rate_per_period = rate / periods_per_year;
    let compound_factor = (1.0 + rate_per_period).powf(total_periods);
    let final_amount = (principal as f64) * compound_factor;
    
    let interest = (final_amount - principal as f64) as u128;
    
    Ok(interest)
}

/// Get current interest rate based on market conditions
pub fn get_current_rate(
    model: &InterestModel,
    utilization: f64,
    volatility: f64,
) -> f64 {
    match model {
        InterestModel::Fixed { rate } => *rate,
        
        InterestModel::Variable { base, slope1, slope2, kink } => {
            // Add volatility premium
            let vol_premium = volatility * 0.1;
            let base_rate = if utilization <= *kink {
                base + slope1 * utilization
            } else {
                base + slope1 * kink + slope2 * (utilization - *kink)
            };
            base_rate + vol_premium
        }
        
        InterestModel::Dynamic { base, oracle_multiplier, max_rate } => {
            // Use volatility as oracle input
            let rate = base * (1.0 + oracle_multiplier * volatility);
            rate.min(*max_rate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_interest() {
        let principal = 10000;
        let rate = 0.1; // 10% annual
        let time = 365 * 24 * 60 * 60; // 1 year in seconds
        
        let interest = calculate_interest(principal, rate, time).unwrap();
        assert_eq!(interest, 1000); // 10% of 10000
    }
    
    #[test]
    fn test_compound_interest() {
        let principal = 10000;
        let rate = 0.1;
        let time = 365 * 24 * 60 * 60;
        let frequency = 30 * 24 * 60 * 60; // Monthly
        
        let interest = compound_interest(principal, rate, time, frequency).unwrap();
        assert!(interest > 1000); // Should be more than simple interest
    }
    
    #[test]
    fn test_variable_rate() {
        let model = InterestModel::Variable {
            base: 0.02,
            slope1: 0.1,
            slope2: 0.5,
            kink: 0.8,
        };
        
        // Low utilization
        let rate = get_current_rate(&model, 0.3, 0.1);
        assert!(rate < 0.1);
        
        // High utilization
        let rate = get_current_rate(&model, 0.9, 0.1);
        assert!(rate > 0.1);
    }
}