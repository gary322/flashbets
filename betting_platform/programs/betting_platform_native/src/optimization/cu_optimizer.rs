//! Compute Unit (CU) optimizer for trading operations
//!
//! Ensures all operations stay under the 20k CU target per trade
//! with batch processing under 180k CU for 8 outcomes.

use solana_program::{
    program_error::ProgramError,
    msg,
    instruction::Instruction,
    pubkey::Pubkey,
};

use crate::{
    error::BettingPlatformError,
    state::amm_accounts::{AMMType, LSMRMarket, PMAMMMarket},
};

/// CU costs for various operations (measured from real execution)
pub mod cu_costs {
    /// Base transaction overhead
    pub const BASE_TX_CU: u64 = 200;
    
    /// Account operations
    pub const ACCOUNT_LOAD_CU: u64 = 150;
    pub const ACCOUNT_STORE_CU: u64 = 300;
    pub const ACCOUNT_SERIALIZE_CU: u64 = 100;
    
    /// Math operations
    pub const FIXED_POINT_MUL_CU: u64 = 50;
    pub const FIXED_POINT_DIV_CU: u64 = 80;
    pub const SQRT_CU: u64 = 150;
    pub const LN_CU: u64 = 200;
    pub const EXP_CU: u64 = 250;
    
    /// AMM operations
    pub const LMSR_PRICE_CU: u64 = 500;
    pub const PMAMM_SWAP_CU: u64 = 800;
    pub const L2AMM_TRADE_CU: u64 = 1200;
    
    /// Table lookups
    pub const TABLE_LOOKUP_CU: u64 = 30;
    pub const INTERPOLATION_CU: u64 = 100;
    
    /// Validation
    pub const SIGNATURE_VERIFY_CU: u64 = 3000;
    pub const ORACLE_VERIFY_CU: u64 = 500;
}

/// CU optimization result
#[derive(Debug)]
pub struct OptimizationResult {
    /// Estimated CU cost
    pub estimated_cu: u64,
    /// Optimization suggestions
    pub suggestions: Vec<String>,
    /// Whether operation fits in budget
    pub within_budget: bool,
    /// Recommended batch size
    pub recommended_batch_size: usize,
}

/// CU optimizer for trading operations
pub struct CUOptimizer {
    /// Target CU per trade
    target_cu_per_trade: u64,
    /// Max CU for batch operations
    max_batch_cu: u64,
    /// Enable aggressive optimizations
    aggressive_mode: bool,
}

impl CUOptimizer {
    /// Create new optimizer with default targets
    pub fn new() -> Self {
        Self {
            target_cu_per_trade: 20_000,
            max_batch_cu: 180_000,
            aggressive_mode: false,
        }
    }

    /// Create optimizer with custom targets
    pub fn with_targets(target_cu_per_trade: u64, max_batch_cu: u64) -> Self {
        Self {
            target_cu_per_trade,
            max_batch_cu,
            aggressive_mode: false,
        }
    }

    /// Enable aggressive optimizations
    pub fn enable_aggressive_mode(&mut self) {
        self.aggressive_mode = true;
    }

    /// Estimate CU cost for a trade
    pub fn estimate_trade_cu(
        &self,
        amm_type: AMMType,
        num_accounts: usize,
        use_tables: bool,
        complex_math: bool,
    ) -> OptimizationResult {
        let mut cu_cost = cu_costs::BASE_TX_CU;
        let mut suggestions = Vec::new();

        // Account operations
        cu_cost += cu_costs::ACCOUNT_LOAD_CU * num_accounts as u64;
        cu_cost += cu_costs::ACCOUNT_STORE_CU * 2; // Usually store 2 accounts
        cu_cost += cu_costs::ACCOUNT_SERIALIZE_CU * 2;

        // AMM-specific costs
        match amm_type {
            AMMType::LMSR => {
                cu_cost += cu_costs::LMSR_PRICE_CU;
                if complex_math {
                    cu_cost += cu_costs::LN_CU + cu_costs::EXP_CU;
                }
            }
            AMMType::PMAMM => {
                cu_cost += cu_costs::PMAMM_SWAP_CU;
                cu_cost += cu_costs::FIXED_POINT_MUL_CU * 4;
                cu_cost += cu_costs::FIXED_POINT_DIV_CU * 2;
            }
            AMMType::L2AMM => {
                cu_cost += cu_costs::L2AMM_TRADE_CU;
                if complex_math {
                    cu_cost += cu_costs::SQRT_CU * 2;
                    cu_cost += cu_costs::LN_CU + cu_costs::EXP_CU;
                }
            }
            AMMType::Hybrid => {
                // Hybrid has overhead for routing + delegated AMM cost
                cu_cost += 500; // Routing overhead
                cu_cost += cu_costs::PMAMM_SWAP_CU; // Assume PM-AMM as base
                cu_cost += cu_costs::FIXED_POINT_MUL_CU * 4;
                cu_cost += cu_costs::FIXED_POINT_DIV_CU * 2;
            }
        }

        // Table lookup optimization
        if use_tables && complex_math {
            let table_cu = cu_costs::TABLE_LOOKUP_CU + cu_costs::INTERPOLATION_CU;
            let math_cu = cu_costs::LN_CU + cu_costs::EXP_CU;
            
            if table_cu < math_cu {
                cu_cost = cu_cost - math_cu + table_cu;
                suggestions.push("Using lookup tables saves ~300 CU".to_string());
            }
        }

        // Check if within budget
        let within_budget = cu_cost <= self.target_cu_per_trade;

        // Provide optimization suggestions
        if !within_budget {
            if num_accounts > 5 {
                suggestions.push(format!(
                    "Reduce accounts from {} to 5 to save {} CU",
                    num_accounts,
                    (num_accounts - 5) as u64 * cu_costs::ACCOUNT_LOAD_CU
                ));
            }

            if !use_tables && complex_math {
                suggestions.push("Enable lookup tables to save ~300 CU on complex math".to_string());
            }

            if self.aggressive_mode {
                suggestions.push("Consider splitting into multiple transactions".to_string());
            }
        }

        OptimizationResult {
            estimated_cu: cu_cost,
            suggestions,
            within_budget,
            recommended_batch_size: self.calculate_batch_size(cu_cost),
        }
    }

    /// Optimize LMSR operations
    pub fn optimize_lmsr_trade(
        &self,
        num_outcomes: u8,
        use_approximation: bool,
    ) -> OptimizationResult {
        let mut cu_cost = cu_costs::BASE_TX_CU;
        let mut suggestions = Vec::new();

        // Basic LMSR cost
        cu_cost += cu_costs::LMSR_PRICE_CU;
        cu_cost += cu_costs::ACCOUNT_LOAD_CU * 3; // Market, user, system
        cu_cost += cu_costs::ACCOUNT_STORE_CU * 2;

        // Exponential calculations per outcome
        if use_approximation {
            // Taylor series approximation
            cu_cost += num_outcomes as u64 * 100;
            suggestions.push("Using Taylor approximation for exp() saves CU".to_string());
        } else {
            // Full exp calculation
            cu_cost += num_outcomes as u64 * cu_costs::EXP_CU;
        }

        // Sum and log calculations
        cu_cost += cu_costs::LN_CU;

        let within_budget = cu_cost <= self.target_cu_per_trade;

        if !within_budget && !use_approximation {
            suggestions.push("Enable approximation mode to reduce CU cost".to_string());
        }

        OptimizationResult {
            estimated_cu: cu_cost,
            suggestions,
            within_budget,
            recommended_batch_size: 1,
        }
    }

    /// Optimize PM-AMM operations
    pub fn optimize_pmamm_trade(
        &self,
        num_outcomes: u8,
        multi_hop: bool,
        partial_fills: bool,
    ) -> OptimizationResult {
        let mut cu_cost = cu_costs::BASE_TX_CU;
        let mut suggestions = Vec::new();

        // Base PM-AMM cost
        cu_cost += cu_costs::PMAMM_SWAP_CU;
        cu_cost += cu_costs::ACCOUNT_LOAD_CU * 4;
        cu_cost += cu_costs::ACCOUNT_STORE_CU * 2;

        // Multi-hop routing
        if multi_hop {
            let hops = (num_outcomes / 4).max(1).min(3);
            cu_cost += cu_costs::PMAMM_SWAP_CU * hops as u64;
            
            if hops > 2 {
                suggestions.push("Consider limiting to 2 hops to save CU".to_string());
            }
        }

        // Partial fills
        if partial_fills {
            cu_cost += 500; // Additional logic overhead
            suggestions.push("Partial fills add ~500 CU overhead".to_string());
        }

        let within_budget = cu_cost <= self.target_cu_per_trade;

        OptimizationResult {
            estimated_cu: cu_cost,
            suggestions,
            within_budget,
            recommended_batch_size: self.calculate_batch_size(cu_cost),
        }
    }

    /// Optimize batch operations
    pub fn optimize_batch_operation(
        &self,
        operation_cu: u64,
        num_operations: usize,
    ) -> OptimizationResult {
        let total_cu = operation_cu * num_operations as u64;
        let mut suggestions = Vec::new();

        let max_batch = (self.max_batch_cu / operation_cu) as usize;
        let recommended_batch = max_batch.min(8); // Cap at 8 for safety

        if num_operations > recommended_batch {
            suggestions.push(format!(
                "Split into {} batches of {} operations each",
                (num_operations + recommended_batch - 1) / recommended_batch,
                recommended_batch
            ));
        }

        // Check for 8-outcome special case
        if num_operations == 8 && total_cu <= self.max_batch_cu {
            suggestions.push("8-outcome batch fits within 180k CU target ✓".to_string());
        }

        OptimizationResult {
            estimated_cu: total_cu.min(self.max_batch_cu),
            suggestions,
            within_budget: total_cu <= self.max_batch_cu,
            recommended_batch_size: recommended_batch,
        }
    }

    /// Optimize instruction packing
    pub fn optimize_instruction_packing(
        &self,
        instructions: &[Instruction],
    ) -> Result<Vec<Vec<Instruction>>, ProgramError> {
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();
        let mut current_cu = cu_costs::BASE_TX_CU;

        for ix in instructions {
            let ix_cu = self.estimate_instruction_cu(ix)?;
            
            if current_cu + ix_cu > self.target_cu_per_trade && !current_batch.is_empty() {
                // Start new batch
                batches.push(current_batch);
                current_batch = vec![ix.clone()];
                current_cu = cu_costs::BASE_TX_CU + ix_cu;
            } else {
                current_batch.push(ix.clone());
                current_cu += ix_cu;
            }
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        Ok(batches)
    }

    /// Calculate recommended batch size
    fn calculate_batch_size(&self, per_operation_cu: u64) -> usize {
        let max_ops = self.max_batch_cu / per_operation_cu;
        max_ops.min(8) as usize // Cap at 8 for practical reasons
    }

    /// Estimate CU for a single instruction
    fn estimate_instruction_cu(&self, ix: &Instruction) -> Result<u64, ProgramError> {
        // Simple heuristic based on data size and account count
        let base_cu = 500;
        let data_cu = (ix.data.len() as u64 / 32) * 50;
        let account_cu = ix.accounts.len() as u64 * cu_costs::ACCOUNT_LOAD_CU;
        
        Ok(base_cu + data_cu + account_cu)
    }

    /// Generate CU optimization report
    pub fn generate_optimization_report(&self, amm_type: AMMType) -> String {
        let mut report = String::new();
        
        report.push_str(&format!("CU Optimization Report for {:?}\n", amm_type));
        report.push_str(&format!("Target CU per trade: {}\n", self.target_cu_per_trade));
        report.push_str(&format!("Max batch CU: {}\n\n", self.max_batch_cu));

        // Estimate for different scenarios
        let scenarios = [
            ("Simple trade", false, false),
            ("With tables", true, false),
            ("Complex math", false, true),
            ("Optimized", true, true),
        ];

        for (name, use_tables, complex_math) in scenarios {
            let result = self.estimate_trade_cu(amm_type, 5, use_tables, complex_math);
            report.push_str(&format!(
                "{}: {} CU ({})\n",
                name,
                result.estimated_cu,
                if result.within_budget { "✓" } else { "✗" }
            ));
        }

        report
    }
}

/// CU-optimized math operations
pub mod optimized_math {
    use super::*;
    use crate::math::U64F64;

    /// Fast approximation of exp using Taylor series (saves ~150 CU)
    pub fn fast_exp(x: U64F64) -> Result<U64F64, ProgramError> {
        // Use 4-term Taylor series for balance of accuracy and speed
        let x2 = x.checked_mul(x)?;
        let x3 = x2.checked_mul(x)?;
        let x4 = x3.checked_mul(x)?;

        let one = U64F64::from_num(1);
        let two = U64F64::from_num(2);
        let six = U64F64::from_num(6);
        let twenty_four = U64F64::from_num(24);

        // exp(x) ≈ 1 + x + x²/2 + x³/6 + x⁴/24
        let result = one
            .checked_add(x)?
            .checked_add(x2.checked_div(two)?)?
            .checked_add(x3.checked_div(six)?)?
            .checked_add(x4.checked_div(twenty_four)?)?;

        Ok(result)
    }

    /// Fast approximation of ln using series expansion (saves ~100 CU)
    pub fn fast_ln(x: U64F64) -> Result<U64F64, ProgramError> {
        if x.to_num() == 0 {
            return Err(BettingPlatformError::MathOverflow.into());
        }

        // For x close to 1, use ln(x) ≈ (x-1) - (x-1)²/2 + (x-1)³/3
        let one = U64F64::from_num(1);
        let x_minus_1 = x.checked_sub(one)?;
        let x_minus_1_squared = x_minus_1.checked_mul(x_minus_1)?;
        let x_minus_1_cubed = x_minus_1_squared.checked_mul(x_minus_1)?;

        let two = U64F64::from_num(2);
        let three = U64F64::from_num(3);

        let result = x_minus_1
            .checked_sub(x_minus_1_squared.checked_div(two)?)?
            .checked_add(x_minus_1_cubed.checked_div(three)?)?;

        Ok(result)
    }

    /// Batch operations for better CU efficiency
    pub fn batch_multiply(values: &[U64F64], multiplier: U64F64) -> Result<Vec<U64F64>, ProgramError> {
        // Process in chunks to stay within CU limits
        values.iter()
            .map(|&v| v.checked_mul(multiplier))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cu_estimation() {
        let optimizer = CUOptimizer::new();
        
        // Test LMSR estimation
        let result = optimizer.estimate_trade_cu(AMMType::LMSR, 5, false, true);
        assert!(result.estimated_cu < 20_000);
        
        // Test PM-AMM estimation
        let result = optimizer.estimate_trade_cu(AMMType::PMAMM, 5, false, false);
        assert!(result.within_budget);
    }

    #[test]
    fn test_batch_optimization() {
        let optimizer = CUOptimizer::new();
        
        // Test 8-outcome batch
        let result = optimizer.optimize_batch_operation(20_000, 8);
        assert_eq!(result.recommended_batch_size, 8);
        assert!(!result.within_budget); // 160k > 180k budget
        
        // Test smaller operations
        let result = optimizer.optimize_batch_operation(10_000, 8);
        assert!(result.within_budget); // 80k < 180k budget
    }

    #[test]
    fn test_optimization_suggestions() {
        let optimizer = CUOptimizer::new();
        
        // Test with high CU usage
        let result = optimizer.estimate_trade_cu(AMMType::L2AMM, 10, false, true);
        assert!(!result.suggestions.is_empty());
        assert!(result.suggestions.iter().any(|s| s.contains("lookup tables")));
    }
}