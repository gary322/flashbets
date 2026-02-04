//! L2AMM types and context
//!
//! Type definitions for L2AMM operations

use solana_program::program_error::ProgramError;

use crate::{
    error::BettingPlatformError,
    state::ProposalPDA,
    math::U64F64,
};

/// L2AMM context for calculations
pub struct L2AMMContext {
    pub liquidity: U64F64,
    pub distribution_params: Vec<U64F64>,
    pub integration_points: usize,
}

impl L2AMMContext {
    /// Create context from proposal
    pub fn from_proposal(proposal: &ProposalPDA) -> Result<Self, ProgramError> {
        let liquidity = U64F64::from_num(proposal.total_liquidity) / U64F64::from_num(1_000_000);
        
        // Use outcome balances as distribution parameters
        let distribution_params = proposal.outcome_balances
            .iter()
            .map(|&b| U64F64::from_num(b) / U64F64::from_num(1_000_000))
            .collect();
            
        Ok(Self {
            liquidity,
            distribution_params,
            integration_points: 10, // Default integration points
        })
    }
    
    /// Calculate price for an outcome using L2 norm
    pub fn calculate_price(&self, outcome: u8) -> Result<u64, ProgramError> {
        if outcome as usize >= self.distribution_params.len() {
            return Err(BettingPlatformError::InvalidOutcome.into());
        }
        
        // L2 norm price: p_i = w_i² / sum(w_j²)
        let squares: Vec<U64F64> = self.distribution_params
            .iter()
            .map(|&w| w * w)
            .collect();
            
        let sum_squares: U64F64 = squares.iter()
            .fold(U64F64::from_num(0), |acc, &val| acc + val);
        
        if sum_squares == U64F64::from_num(0) {
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        
        let price_fp = squares[outcome as usize] / sum_squares;
        
        // Convert to basis points
        Ok((price_fp * U64F64::from_num(10000)).to_num())
    }
    
    /// Calculate cost for a trade
    pub fn calculate_cost(&self, outcome: u8, amount: u64) -> Result<u64, ProgramError> {
        if outcome as usize >= self.distribution_params.len() {
            return Err(BettingPlatformError::InvalidOutcome.into());
        }
        
        let amount_fp = U64F64::from_num(amount) / U64F64::from_num(1_000_000);
        
        // Cost function: C = L * sqrt(sum((w_i + δ_i*a)²)) - L * sqrt(sum(w_i²))
        // where δ_i = 1 for outcome i, 0 otherwise
        
        let current_norm = self.calculate_l2_norm()?;
        
        // Update weight for the outcome
        let mut new_params = self.distribution_params.clone();
        new_params[outcome as usize] = new_params[outcome as usize] + amount_fp;
        
        let new_norm = Self::calculate_l2_norm_static(&new_params)?;
        
        let cost_fp = self.liquidity * (new_norm - current_norm);
        
        // Convert to amount with 6 decimals
        Ok((cost_fp * U64F64::from_num(1_000_000)).to_num())
    }
    
    fn calculate_l2_norm(&self) -> Result<U64F64, ProgramError> {
        Self::calculate_l2_norm_static(&self.distribution_params)
    }
    
    fn calculate_l2_norm_static(params: &[U64F64]) -> Result<U64F64, ProgramError> {
        let sum_squares: U64F64 = params
            .iter()
            .map(|&w| w * w)
            .fold(U64F64::from_num(0), |acc, val| acc + val);
            
        // Square root approximation using Newton's method
        if sum_squares == U64F64::from_num(0) {
            return Ok(U64F64::from_num(0));
        }
        
        let mut x = sum_squares;
        for _ in 0..5 {
            x = (x + sum_squares / x) / U64F64::from_num(2);
        }
        
        Ok(x)
    }
}

/// L2AMM trade parameters
#[derive(Debug, Clone)]
pub struct L2TradeParams {
    pub outcome: u8,
    pub amount: u64,
    pub is_buy: bool,
    pub max_slippage_bps: u16,
}