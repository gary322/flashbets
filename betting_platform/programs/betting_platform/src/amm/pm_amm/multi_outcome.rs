use super::core::*;
use super::newton_raphson::*;
use fixed::types::{U64F64, I64F64};
use anchor_lang::prelude::*;

#[derive(Debug, Clone)]
pub enum PricingError {
    ZeroSum,
    InvalidOutcome,
    MathOverflow,
    InvalidPrice,
}

impl From<PricingError> for ProgramError {
    fn from(e: PricingError) -> Self {
        match e {
            PricingError::ZeroSum => ProgramError::Custom(200),
            PricingError::InvalidOutcome => ProgramError::Custom(201),
            PricingError::MathOverflow => ProgramError::Custom(202),
            PricingError::InvalidPrice => ProgramError::Custom(203),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrossImpact {
    pub outcome_id: u8,
    pub price_change: U64F64,
    pub is_negative: bool,
    pub liquidity_share: U64F64,
}

pub struct MultiOutcomePricing {
    pub outcome_count: u8,
    pub price_sum_constraint: U64F64, // Must sum to 1
    pub min_price: U64F64, // 0.001 to prevent zero prices
    pub max_price: U64F64, // 0.999 to prevent certainty
}

impl MultiOutcomePricing {
    pub fn new() -> Self {
        Self {
            outcome_count: 0, // Will be set when used
            price_sum_constraint: U64F64::from_num(1),
            min_price: U64F64::from_num(0.001),
            max_price: U64F64::from_num(0.999),
        }
    }

    pub fn update_all_prices(
        &self,
        state: &mut PMAMMState,
        outcome_traded: u8,
        new_price: U64F64,
        _solver: &NewtonRaphsonSolver,
    ) -> std::result::Result<(), PricingError> {
        if outcome_traded >= state.outcome_count {
            return Err(PricingError::InvalidOutcome);
        }

        let old_price = state.prices[outcome_traded as usize];
        let price_delta = new_price - old_price;

        // Update traded outcome price
        state.prices[outcome_traded as usize] = new_price;

        // Redistribute delta to maintain sum = 1
        let remaining_outcomes = state.outcome_count - 1;
        if remaining_outcomes == 0 {
            return Err(PricingError::InvalidOutcome);
        }

        let redistribution = price_delta / U64F64::from_num(remaining_outcomes);

        for i in 0..state.outcome_count as usize {
            if i != outcome_traded as usize {
                let adjusted_price = state.prices[i].saturating_sub(redistribution);
                state.prices[i] = adjusted_price
                    .max(self.min_price)
                    .min(self.max_price);
            }
        }

        // Normalize to ensure exact sum = 1
        self.normalize_prices(&mut state.prices)?;

        Ok(())
    }

    fn normalize_prices(&self, prices: &mut Vec<U64F64>) -> std::result::Result<(), PricingError> {
        let sum: U64F64 = prices.iter().copied().sum();

        if sum == U64F64::from_num(0) {
            return Err(PricingError::ZeroSum);
        }

        for price in prices.iter_mut() {
            *price = *price / sum;
        }

        Ok(())
    }

    pub fn calculate_cross_impact(
        &self,
        state: &PMAMMState,
        primary_outcome: u8,
        primary_impact: U64F64,
    ) -> Vec<CrossImpact> {
        let mut impacts = Vec::new();

        // Calculate total volume for normalization
        let total_volume: U64F64 = state.volumes.iter().copied().sum();
        let volume_safe = if total_volume > U64F64::from_num(0) {
            total_volume
        } else {
            U64F64::from_num(1) // Prevent division by zero
        };

        for i in 0..state.outcome_count {
            if i != primary_outcome {
                let impact = CrossImpact {
                    outcome_id: i,
                    price_change: primary_impact / U64F64::from_num(state.outcome_count - 1),
                    is_negative: true,
                    liquidity_share: state.volumes[i as usize] / volume_safe,
                };
                impacts.push(impact);
            }
        }

        impacts
    }
}