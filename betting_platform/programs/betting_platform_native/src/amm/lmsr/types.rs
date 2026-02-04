//! LMSR AMM types and context
//!
//! Type definitions for LMSR AMM operations

use solana_program::program_error::ProgramError;

use crate::{
    error::BettingPlatformError,
    state::ProposalPDA,
    math::U64F64,
};

/// LMSR AMM context for calculations
pub struct LMSRAMMContext {
    pub b_value: U64F64,
    pub outcome_balances: Vec<U64F64>,
    pub total_shares: U64F64,
}

impl LMSRAMMContext {
    /// Create context from proposal
    pub fn from_proposal(proposal: &ProposalPDA) -> Result<Self, ProgramError> {
        if proposal.outcomes != 2 {
            return Err(BettingPlatformError::InvalidOutcomeCount.into());
        }
        
        let b_value = U64F64::from_num(proposal.b_value) / U64F64::from_num(1_000_000);
        
        let outcome_balances = proposal.outcome_balances[..2]
            .iter()
            .map(|&b| U64F64::from_num(b) / U64F64::from_num(1_000_000))
            .collect();
            
        let total_shares = U64F64::from_num(proposal.total_liquidity) / U64F64::from_num(1_000_000);
        
        Ok(Self {
            b_value,
            outcome_balances,
            total_shares,
        })
    }
    
    /// Calculate price for an outcome
    pub fn price(&self, outcome: u8) -> Result<u64, ProgramError> {
        if outcome >= 2 {
            return Err(BettingPlatformError::InvalidOutcome.into());
        }
        
        // LMSR price formula: exp(q_i/b) / sum(exp(q_j/b))
        let exp_values: Vec<U64F64> = self.outcome_balances
            .iter()
            .map(|&q| {
                let ratio = q / self.b_value;
                // Simplified exponential approximation
                U64F64::from_num(1) + ratio + (ratio * ratio) / U64F64::from_num(2)
            })
            .collect();
            
        let sum_exp: U64F64 = exp_values.iter()
            .fold(U64F64::from_num(0), |acc, &val| acc + val);
        
        let price_fp = exp_values[outcome as usize] / sum_exp;
        
        // Convert back to basis points (10000 = 100%)
        Ok((price_fp * U64F64::from_num(10000)).to_num())
    }
    
    /// Calculate cost for buying shares
    pub fn cost(&self, outcome: u8, shares: u64) -> Result<u64, ProgramError> {
        if outcome >= 2 {
            return Err(BettingPlatformError::InvalidOutcome.into());
        }
        
        let shares_fp = U64F64::from_num(shares) / U64F64::from_num(1_000_000);
        
        // C(q) = b * ln(sum(exp(q_i/b)))
        // Cost = C(q + shares) - C(q)
        
        let current_cost = self.calculate_cost_function()?;
        
        // Update balances
        let mut new_balances = self.outcome_balances.clone();
        new_balances[outcome as usize] = new_balances[outcome as usize] + shares_fp;
        
        let new_context = Self {
            b_value: self.b_value,
            outcome_balances: new_balances,
            total_shares: self.total_shares,
        };
        
        let new_cost = new_context.calculate_cost_function()?;
        
        let cost_fp = new_cost - current_cost;
        
        // Convert to amount with 6 decimals
        Ok((cost_fp * U64F64::from_num(1_000_000)).to_num())
    }
    
    fn calculate_cost_function(&self) -> Result<U64F64, ProgramError> {
        // C(q) = b * ln(sum(exp(q_i/b)))
        // Using approximation: ln(sum) ≈ max(q_i/b) + ln(1 + sum(exp((q_j - max)/b)))
        
        let max_ratio = self.outcome_balances
            .iter()
            .map(|&q| q / self.b_value)
            .max()
            .unwrap_or(U64F64::from_num(0));
            
        let sum_exp_adjusted: U64F64 = self.outcome_balances
            .iter()
            .map(|&q| {
                let ratio = (q / self.b_value) - max_ratio;
                // exp approximation for small values
                U64F64::from_num(1) + ratio + (ratio * ratio) / U64F64::from_num(2)
            })
            .fold(U64F64::from_num(0), |acc, val| acc + val);
            
        // ln approximation: ln(1 + x) ≈ x - x²/2 for small x
        let ln_sum = sum_exp_adjusted - U64F64::from_num(1);
        let ln_approx = ln_sum - (ln_sum * ln_sum) / U64F64::from_num(2);
        
        Ok(self.b_value * (max_ratio + ln_approx))
    }
}