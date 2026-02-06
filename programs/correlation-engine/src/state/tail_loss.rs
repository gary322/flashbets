use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
};
use crate::math::fixed_point::U64F64;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TailLossParameters {
    pub base_tail_loss: u64,      // 1 - 1/N
    pub correlation_factor: u64,  // Average correlation from matrix
    pub enhanced_tail_loss: u64,  // Final calculated tail loss
    pub last_updated: i64,
}

/// Tail loss state for a verse
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VerseTailLoss {
    pub is_initialized: bool,
    pub verse_id: [u8; 16],
    pub parameters: TailLossParameters,
    pub coverage_impact: u64,  // How this affects leverage coverage
    pub outcome_count: u32,
    pub bump: u8,
}

impl VerseTailLoss {
    pub const LEN: usize = 1 + 16 + (8 * 4) + 8 + 4 + 1;
    
    pub fn new(verse_id: [u8; 16], bump: u8) -> Self {
        Self {
            is_initialized: true,
            verse_id,
            parameters: TailLossParameters {
                base_tail_loss: 0,
                correlation_factor: 0,
                enhanced_tail_loss: 0,
                last_updated: 0,
            },
            coverage_impact: U64F64::ONE,
            outcome_count: 0,
            bump,
        }
    }
    
    /// Calculate enhanced tail loss with correlation
    /// Formula: tail_loss = 1 - (1/N) × (1 - correlation_factor)
    pub fn calculate_enhanced_tail_loss(
        n: u32,
        correlation_factor: u64,
    ) -> Result<u64, ProgramError> {
        if n == 0 {
            return Ok(0);
        }
        
        // Base tail loss = 1 - 1/N
        let one = U64F64::ONE;
        let n_fixed = U64F64::from_num(n as u64);
        let one_over_n = U64F64::checked_div(one, n_fixed)?;
        let _base = U64F64::checked_sub(one, one_over_n)?;
        
        // Enhanced formula: tail_loss = 1 - (1/N) × (1 - correlation_factor)
        let one_minus_corr = U64F64::checked_sub(one, correlation_factor)?;
        let adjustment = U64F64::checked_mul(one_over_n, one_minus_corr)?;
        let enhanced = U64F64::checked_sub(one, adjustment)?;
        
        Ok(enhanced)
    }
    
    /// Update tail loss parameters
    pub fn update(
        &mut self,
        outcome_count: u32,
        correlation_factor: u64,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        self.outcome_count = outcome_count;
        
        // Calculate base tail loss
        if outcome_count > 0 {
            let one = U64F64::ONE;
            let n_fixed = U64F64::from_num(outcome_count as u64);
            let one_over_n = U64F64::checked_div(one, n_fixed)?;
            self.parameters.base_tail_loss = U64F64::checked_sub(one, one_over_n)?;
        } else {
            self.parameters.base_tail_loss = 0;
        }
        
        // Store correlation factor
        self.parameters.correlation_factor = correlation_factor;
        
        // Calculate enhanced tail loss
        self.parameters.enhanced_tail_loss = Self::calculate_enhanced_tail_loss(
            outcome_count,
            correlation_factor,
        )?;
        
        // Calculate coverage impact (enhanced / base)
        if self.parameters.base_tail_loss > 0 {
            self.coverage_impact = U64F64::checked_div(
                self.parameters.enhanced_tail_loss,
                self.parameters.base_tail_loss,
            )?;
        } else {
            self.coverage_impact = U64F64::ONE;
        }
        
        self.parameters.last_updated = timestamp;
        
        Ok(())
    }
    
    /// Get the tail loss to use for coverage calculations
    pub fn get_effective_tail_loss(&self) -> u64 {
        self.parameters.enhanced_tail_loss
    }
}

/// Coverage calculation with tail loss
pub struct CoverageCalculator;

impl CoverageCalculator {
    /// Calculate coverage with correlation-adjusted tail loss
    /// Coverage = Vault / (tail_loss × OI)
    pub fn calculate_coverage(
        vault_balance: u64,
        open_interest: u64,
        tail_loss: &VerseTailLoss,
    ) -> Result<u64, ProgramError> {
        let effective_tail_loss = tail_loss.get_effective_tail_loss();
        
        if open_interest == 0 || effective_tail_loss == 0 {
            // Infinite coverage
            return Ok(u64::MAX);
        }
        
        let denominator = U64F64::checked_mul(effective_tail_loss, open_interest)?;
        let coverage = U64F64::checked_div(vault_balance, denominator)?;
        
        Ok(coverage)
    }
    
    /// Calculate maximum leverage based on coverage
    /// leverage = min(100 × (1 + 0.1 × depth), coverage × 100/√N, tier_cap)
    pub fn calculate_max_leverage(
        coverage: u64,
        outcome_count: u32,
        verse_depth: u32,
    ) -> Result<u64, ProgramError> {
        // Depth multiplier: 1 + 0.1 × depth
        let depth_bonus = U64F64::from_num(verse_depth as u64) / 10;
        let depth_multiplier = U64F64::ONE + depth_bonus;
        let depth_adjusted = U64F64::checked_mul(
            U64F64::from_num(100),
            depth_multiplier,
        )?;
        
        // Coverage adjustment: coverage × 100/√N
        let sqrt_n = (outcome_count as f64).sqrt();
        let sqrt_n_fixed = U64F64::from_f64(sqrt_n)?;
        let hundred = U64F64::from_num(100);
        let coverage_factor = U64F64::checked_div(hundred, sqrt_n_fixed)?;
        let coverage_adjusted = U64F64::checked_mul(coverage, coverage_factor)?;
        
        // Get tier cap
        let tier_cap = Self::get_tier_cap(outcome_count);
        
        // Take minimum
        let max_leverage = depth_adjusted
            .min(coverage_adjusted)
            .min(tier_cap);
        
        Ok(max_leverage)
    }
    
    fn get_tier_cap(n: u32) -> u64 {
        match n {
            1 => U64F64::from_num(100),
            2 => U64F64::from_num(70),
            3..=4 => U64F64::from_num(25),
            5..=8 => U64F64::from_num(15),
            9..=16 => U64F64::from_num(12),
            17..=64 => U64F64::from_num(10),
            _ => U64F64::from_num(5),
        }
    }
}
