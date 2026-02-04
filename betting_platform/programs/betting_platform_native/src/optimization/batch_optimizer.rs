//! Batch operation optimizer for 8-outcome processing
//!
//! Ensures batch processing of 8 outcomes stays under 180k CU target
//! as specified in Part 7 requirements.

use solana_program::{
    program_error::ProgramError,
    msg,
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta},
};

use crate::{
    error::BettingPlatformError,
    state::amm_accounts::AMMType,
    optimization::cu_optimizer::{CUOptimizer, cu_costs},
};

/// Batch operation configuration
#[derive(Clone)]
pub struct BatchConfig {
    /// Maximum outcomes per batch
    pub max_outcomes_per_batch: u8,
    /// Target CU for batch operations
    pub target_batch_cu: u64,
    /// Enable parallel processing
    pub enable_parallel: bool,
    /// Use lookup tables for optimization
    pub use_lookup_tables: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_outcomes_per_batch: 8,
            target_batch_cu: 180_000,
            enable_parallel: true,
            use_lookup_tables: true,
        }
    }
}

/// Batch operation result
#[derive(Debug)]
pub struct BatchOperationResult {
    /// Number of batches required
    pub num_batches: usize,
    /// Outcomes per batch
    pub outcomes_per_batch: Vec<u8>,
    /// Estimated CU per batch
    pub cu_per_batch: Vec<u64>,
    /// Total estimated CU
    pub total_cu: u64,
    /// Whether operation fits in single batch
    pub single_batch_possible: bool,
}

/// Batch operation optimizer
pub struct BatchOptimizer {
    config: BatchConfig,
    cu_optimizer: CUOptimizer,
}

impl BatchOptimizer {
    /// Create new batch optimizer
    pub fn new() -> Self {
        Self {
            config: BatchConfig::default(),
            cu_optimizer: CUOptimizer::new(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: BatchConfig) -> Self {
        let target_cu = config.target_batch_cu;
        Self {
            config,
            cu_optimizer: CUOptimizer::with_targets(20_000, target_cu),
        }
    }

    /// Optimize 8-outcome batch operation
    pub fn optimize_8_outcome_batch(
        &self,
        amm_type: AMMType,
        operation_type: BatchOperationType,
    ) -> Result<BatchOperationResult, ProgramError> {
        msg!("Optimizing 8-outcome batch for {:?} {:?}", amm_type, operation_type);

        // Calculate base CU cost
        let base_cu = self.calculate_base_batch_cu(&operation_type);
        
        // Calculate per-outcome CU cost
        let per_outcome_cu = self.calculate_per_outcome_cu(amm_type, &operation_type)?;
        
        // Check if 8 outcomes fit in single batch
        let total_cu_8_outcomes = base_cu + (per_outcome_cu * 8);
        let single_batch_possible = total_cu_8_outcomes <= self.config.target_batch_cu;

        if single_batch_possible {
            msg!("✓ 8 outcomes fit in single batch: {} CU", total_cu_8_outcomes);
            
            Ok(BatchOperationResult {
                num_batches: 1,
                outcomes_per_batch: vec![8],
                cu_per_batch: vec![total_cu_8_outcomes],
                total_cu: total_cu_8_outcomes,
                single_batch_possible: true,
            })
        } else {
            // Split into multiple batches
            self.split_into_batches(8, base_cu, per_outcome_cu)
        }
    }

    /// Optimize arbitrary outcome count batch
    pub fn optimize_batch_operation(
        &self,
        amm_type: AMMType,
        num_outcomes: u8,
        operation_type: BatchOperationType,
    ) -> Result<BatchOperationResult, ProgramError> {
        if num_outcomes == 0 {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        let base_cu = self.calculate_base_batch_cu(&operation_type);
        let per_outcome_cu = self.calculate_per_outcome_cu(amm_type, &operation_type)?;

        // Special optimization for 8 outcomes
        if num_outcomes == 8 {
            return self.optimize_8_outcome_batch(amm_type, operation_type);
        }

        // General case
        let total_cu = base_cu + (per_outcome_cu * num_outcomes as u64);
        
        if total_cu <= self.config.target_batch_cu {
            Ok(BatchOperationResult {
                num_batches: 1,
                outcomes_per_batch: vec![num_outcomes],
                cu_per_batch: vec![total_cu],
                total_cu,
                single_batch_possible: true,
            })
        } else {
            self.split_into_batches(num_outcomes, base_cu, per_outcome_cu)
        }
    }

    /// Create optimized batch instructions
    pub fn create_batch_instructions(
        &self,
        accounts: Vec<AccountMeta>,
        data: Vec<u8>,
        program_id: Pubkey,
        outcomes_per_batch: &[u8],
    ) -> Result<Vec<Instruction>, ProgramError> {
        let mut instructions = Vec::new();

        for &batch_size in outcomes_per_batch {
            // Create instruction for this batch
            let batch_data = self.create_batch_data(&data, batch_size)?;
            
            instructions.push(Instruction {
                program_id,
                accounts: accounts.clone(),
                data: batch_data,
            });
        }

        Ok(instructions)
    }

    /// Calculate base CU cost for batch operation
    fn calculate_base_batch_cu(&self, operation_type: &BatchOperationType) -> u64 {
        let mut cu = cu_costs::BASE_TX_CU;
        
        // Add fixed costs based on operation type
        match operation_type {
            BatchOperationType::PriceUpdate => {
                cu += cu_costs::ACCOUNT_LOAD_CU * 3; // Market, oracle, config
                cu += cu_costs::ACCOUNT_STORE_CU; // Update market
            }
            BatchOperationType::TradeExecution => {
                cu += cu_costs::ACCOUNT_LOAD_CU * 5; // Market, user, positions, etc.
                cu += cu_costs::ACCOUNT_STORE_CU * 3; // Update multiple accounts
                cu += cu_costs::SIGNATURE_VERIFY_CU; // User signature
            }
            BatchOperationType::LiquidityUpdate => {
                cu += cu_costs::ACCOUNT_LOAD_CU * 4;
                cu += cu_costs::ACCOUNT_STORE_CU * 2;
            }
            BatchOperationType::SettlementBatch => {
                cu += cu_costs::ACCOUNT_LOAD_CU * 6;
                cu += cu_costs::ACCOUNT_STORE_CU * 4;
                cu += cu_costs::ORACLE_VERIFY_CU;
            }
        }

        cu
    }

    /// Calculate per-outcome CU cost
    fn calculate_per_outcome_cu(
        &self,
        amm_type: AMMType,
        operation_type: &BatchOperationType,
    ) -> Result<u64, ProgramError> {
        let mut cu = 0u64;

        // AMM-specific costs
        match amm_type {
            AMMType::LMSR => {
                match operation_type {
                    BatchOperationType::PriceUpdate => {
                        cu += cu_costs::LMSR_PRICE_CU;
                        if self.config.use_lookup_tables {
                            cu += cu_costs::TABLE_LOOKUP_CU * 2;
                        } else {
                            cu += cu_costs::EXP_CU + cu_costs::LN_CU;
                        }
                    }
                    BatchOperationType::TradeExecution => {
                        cu += cu_costs::LMSR_PRICE_CU * 2; // Entry and exit prices
                        cu += cu_costs::FIXED_POINT_MUL_CU * 3;
                    }
                    _ => cu += 1000, // Default per-outcome cost
                }
            }
            AMMType::PMAMM => {
                match operation_type {
                    BatchOperationType::PriceUpdate => {
                        cu += cu_costs::PMAMM_SWAP_CU / 2; // Just price calc
                    }
                    BatchOperationType::TradeExecution => {
                        cu += cu_costs::PMAMM_SWAP_CU;
                        cu += cu_costs::FIXED_POINT_MUL_CU * 4;
                        cu += cu_costs::FIXED_POINT_DIV_CU * 2;
                    }
                    _ => cu += 800,
                }
            }
            AMMType::L2AMM => {
                cu += cu_costs::L2AMM_TRADE_CU;
                if matches!(operation_type, BatchOperationType::TradeExecution) {
                    cu += cu_costs::SQRT_CU * 2;
                }
            }
            AMMType::Hybrid => {
                // Hybrid AMM has overhead for routing logic
                match operation_type {
                    BatchOperationType::PriceUpdate => {
                        cu += cu_costs::PMAMM_SWAP_CU / 2 + 500; // Price calc + routing overhead
                    }
                    BatchOperationType::TradeExecution => {
                        cu += cu_costs::PMAMM_SWAP_CU + 1000; // Trade + routing overhead
                        cu += cu_costs::FIXED_POINT_MUL_CU * 4;
                        cu += cu_costs::FIXED_POINT_DIV_CU * 2;
                    }
                    _ => cu += 900, // Default with slight overhead
                }
            }
        }

        // Add parallel processing overhead if enabled
        if self.config.enable_parallel {
            cu = (cu * 85) / 100; // 15% reduction from parallelization
        }

        Ok(cu)
    }

    /// Split operation into multiple batches
    fn split_into_batches(
        &self,
        total_outcomes: u8,
        base_cu: u64,
        per_outcome_cu: u64,
    ) -> Result<BatchOperationResult, ProgramError> {
        let mut batches = Vec::new();
        let mut cu_per_batch = Vec::new();
        let mut remaining_outcomes = total_outcomes;

        // Calculate optimal batch size
        let max_outcomes_per_batch = ((self.config.target_batch_cu - base_cu) / per_outcome_cu) as u8;
        let optimal_batch_size = max_outcomes_per_batch.min(self.config.max_outcomes_per_batch);

        while remaining_outcomes > 0 {
            let batch_size = remaining_outcomes.min(optimal_batch_size);
            batches.push(batch_size);
            
            let batch_cu = base_cu + (per_outcome_cu * batch_size as u64);
            cu_per_batch.push(batch_cu);
            
            remaining_outcomes -= batch_size;
        }

        let total_cu: u64 = cu_per_batch.iter().sum();

        msg!(
            "Split {} outcomes into {} batches: {:?}",
            total_outcomes,
            batches.len(),
            batches
        );

        Ok(BatchOperationResult {
            num_batches: batches.len(),
            outcomes_per_batch: batches,
            cu_per_batch,
            total_cu,
            single_batch_possible: false,
        })
    }

    /// Create batch data for instruction
    fn create_batch_data(&self, base_data: &[u8], batch_size: u8) -> Result<Vec<u8>, ProgramError> {
        let mut batch_data = base_data.to_vec();
        
        // Add batch metadata
        batch_data.push(batch_size);
        
        // Add optimization flags
        let flags = self.create_optimization_flags();
        batch_data.push(flags);

        Ok(batch_data)
    }

    /// Create optimization flags byte
    fn create_optimization_flags(&self) -> u8 {
        let mut flags = 0u8;
        
        if self.config.enable_parallel {
            flags |= 0b00000001;
        }
        if self.config.use_lookup_tables {
            flags |= 0b00000010;
        }
        
        flags
    }

    /// Generate batch optimization report
    pub fn generate_batch_report(&self, num_outcomes: u8) -> String {
        let mut report = String::new();
        
        report.push_str(&format!("Batch Optimization Report for {} outcomes\n", num_outcomes));
        report.push_str(&format!("Target batch CU: {}\n", self.config.target_batch_cu));
        report.push_str(&format!("Max outcomes per batch: {}\n\n", self.config.max_outcomes_per_batch));

        // Test different AMM types
        for amm_type in &[AMMType::LMSR, AMMType::PMAMM, AMMType::L2AMM, AMMType::Hybrid] {
            report.push_str(&format!("{:?}:\n", amm_type));
            
            for op_type in &[
                BatchOperationType::PriceUpdate,
                BatchOperationType::TradeExecution,
                BatchOperationType::SettlementBatch,
            ] {
                if let Ok(result) = self.optimize_batch_operation(*amm_type, num_outcomes, *op_type) {
                    report.push_str(&format!(
                        "  {:?}: {} batches, {} total CU {}\n",
                        op_type,
                        result.num_batches,
                        result.total_cu,
                        if result.single_batch_possible { "✓" } else { "⚠" }
                    ));
                }
            }
            report.push_str("\n");
        }

        report
    }
}

/// Types of batch operations
#[derive(Debug, Clone, Copy)]
pub enum BatchOperationType {
    /// Price updates across multiple outcomes
    PriceUpdate,
    /// Trade execution batch
    TradeExecution,
    /// Liquidity updates
    LiquidityUpdate,
    /// Settlement batch processing
    SettlementBatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_8_outcome_optimization() {
        let optimizer = BatchOptimizer::new();
        
        // Test LMSR 8-outcome batch
        let result = optimizer.optimize_8_outcome_batch(
            AMMType::LMSR,
            BatchOperationType::PriceUpdate,
        ).unwrap();
        
        // Should fit in single batch for price updates
        assert!(result.single_batch_possible);
        assert!(result.total_cu <= 180_000);
    }

    #[test]
    fn test_batch_splitting() {
        let config = BatchConfig {
            max_outcomes_per_batch: 4,
            target_batch_cu: 50_000,
            enable_parallel: false,
            use_lookup_tables: false,
        };
        
        let optimizer = BatchOptimizer::with_config(config);
        
        // Force splitting with low CU limit
        let result = optimizer.optimize_batch_operation(
            AMMType::PMAMM,
            8,
            BatchOperationType::TradeExecution,
        ).unwrap();
        
        assert!(!result.single_batch_possible);
        assert!(result.num_batches >= 2);
        assert_eq!(result.outcomes_per_batch.iter().sum::<u8>(), 8);
    }

    #[test]
    fn test_cu_reduction_with_tables() {
        let mut config = BatchConfig::default();
        config.use_lookup_tables = false;
        let optimizer_no_tables = BatchOptimizer::with_config(config.clone());
        
        config.use_lookup_tables = true;
        let optimizer_with_tables = BatchOptimizer::with_config(config);
        
        let result_no_tables = optimizer_no_tables.optimize_8_outcome_batch(
            AMMType::LMSR,
            BatchOperationType::TradeExecution,
        ).unwrap();
        
        let result_with_tables = optimizer_with_tables.optimize_8_outcome_batch(
            AMMType::LMSR,
            BatchOperationType::TradeExecution,
        ).unwrap();
        
        // Tables should reduce CU usage
        assert!(result_with_tables.total_cu < result_no_tables.total_cu);
    }
}