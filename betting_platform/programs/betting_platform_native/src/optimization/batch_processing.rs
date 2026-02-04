//! Batch Processing Optimization
//!
//! Efficient batch operations to minimize compute units and improve throughput

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
    state::{Position, ProposalPDA, GlobalConfigPDA, UserMap, Resolution},
    liquidation::{should_liquidate_coverage_based, calculate_liquidation_amount},
    math::U64F64,
    events::{Event, EventType},
};

/// Maximum items per batch to stay within compute limits
pub const MAX_BATCH_SIZE: usize = 32;
pub const MAX_LIQUIDATION_BATCH: usize = 16;
pub const MAX_SETTLEMENT_BATCH: usize = 64;

/// Batch operation types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BatchOperationType {
    Liquidation,
    Settlement,
    PriceUpdate,
    PositionUpdate,
    Withdrawal,
}

/// Batch processor for efficient operations
pub struct BatchProcessor {
    /// Operation type
    operation_type: BatchOperationType,
    /// Batch size limit
    max_batch_size: usize,
    /// Compute unit tracker
    compute_used: u64,
    /// Success counter
    successful_ops: u32,
    /// Failed operations
    failed_ops: Vec<(usize, ProgramError)>,
}

impl BatchProcessor {
    pub fn new(operation_type: BatchOperationType) -> Self {
        let max_batch_size = match operation_type {
            BatchOperationType::Liquidation => MAX_LIQUIDATION_BATCH,
            BatchOperationType::Settlement => MAX_SETTLEMENT_BATCH,
            _ => MAX_BATCH_SIZE,
        };
        
        Self {
            operation_type,
            max_batch_size,
            compute_used: 0,
            successful_ops: 0,
            failed_ops: Vec::new(),
        }
    }
    
    /// Process batch liquidations efficiently
    pub fn process_batch_liquidations(
        &mut self,
        positions: &mut [Position],
        current_price: u64,
        coverage: U64F64,
        _keeper: &Pubkey,
    ) -> Result<BatchLiquidationResult, ProgramError> {
        let batch_size = positions.len().min(self.max_batch_size);
        let mut total_liquidated = 0u64;
        let mut keeper_rewards = 0u64;
        let mut liquidated_positions = Vec::new();
        
        // Pre-calculate shared values
        let liquidation_params = LiquidationParams {
            coverage,
            keeper_reward_bps: 100, // 1%
            current_slot: Clock::get()?.slot,
        };
        
        // Process in chunks to avoid stack overflow
        for (i, position) in positions[..batch_size].iter_mut().enumerate() {
            // Skip if already closed
            if position.is_closed {
                continue;
            }
            
            // Quick health check
            let should_liquidate = should_liquidate_coverage_based(
                position,
                current_price,
                coverage,
            )?;
            
            if should_liquidate {
                // Calculate liquidation amount
                let liq_amount = calculate_liquidation_amount(
                    position.size,
                    coverage,
                )?;
                
                // Update position
                position.size = position.size.saturating_sub(liq_amount);
                if position.size == 0 {
                    position.is_closed = true;
                }
                position.partial_liq_accumulator += liq_amount;
                
                // Calculate keeper reward
                let reward = (liq_amount as u128 * liquidation_params.keeper_reward_bps as u128 / 10_000) as u64;
                
                total_liquidated += liq_amount;
                keeper_rewards += reward;
                liquidated_positions.push(i);
                
                self.successful_ops += 1;
                self.compute_used += 2000; // Approximate CU cost
            }
        }
        
        Ok(BatchLiquidationResult {
            positions_checked: batch_size,
            positions_liquidated: liquidated_positions.len(),
            total_liquidated,
            keeper_rewards,
            liquidated_indices: liquidated_positions,
        })
    }
    
    /// Process batch settlements
    pub fn process_batch_settlements(
        &mut self,
        proposals: &mut [ProposalPDA],
        resolution_prices: &[u64],
    ) -> Result<BatchSettlementResult, ProgramError> {
        let batch_size = proposals.len().min(self.max_batch_size);
        let mut settled_count = 0u32;
        let mut total_volume_settled = 0u64;
        
        for (i, proposal) in proposals[..batch_size].iter_mut().enumerate() {
            // Skip if already settled
            if proposal.settle_slot > 0 {
                continue;
            }
            
            // Get resolution price
            let resolution_price = resolution_prices.get(i)
                .copied()
                .ok_or(BettingPlatformError::InvalidIndex)?;
            
            // Update proposal state
            proposal.settle_slot = Clock::get()?.slot;
            proposal.resolution = Some(Resolution {
                outcome: if resolution_price >= 500_000 { 0 } else { 1 },
                timestamp: Clock::get()?.unix_timestamp,
                oracle_signature: [0u8; 64], // Placeholder - should be from oracle
            });
            proposal.status = crate::state::ProposalState::Resolved;
            
            settled_count += 1;
            total_volume_settled += proposal.total_volume;
            self.successful_ops += 1;
            self.compute_used += 1500; // Approximate CU cost
        }
        
        Ok(BatchSettlementResult {
            proposals_processed: batch_size,
            proposals_settled: settled_count as usize,
            total_volume_settled,
        })
    }
    
    /// Process batch price updates
    pub fn process_batch_price_updates(
        &mut self,
        proposals: &mut [ProposalPDA],
        price_updates: &[(u8, u64)], // (outcome, new_price)
    ) -> Result<BatchPriceUpdateResult, ProgramError> {
        let batch_size = proposals.len().min(self.max_batch_size);
        let mut updated_count = 0u32;
        
        // Group updates by proposal for efficiency
        let mut updates_by_proposal: std::collections::HashMap<usize, Vec<(u8, u64)>> = 
            std::collections::HashMap::new();
        
        for &(outcome, price) in price_updates {
            // Find proposal containing this outcome
            for (i, proposal) in proposals[..batch_size].iter().enumerate() {
                if outcome < proposal.outcomes {
                    updates_by_proposal.entry(i).or_insert_with(Vec::new).push((outcome, price));
                    break;
                }
            }
        }
        
        // Apply updates efficiently
        for (proposal_idx, updates) in updates_by_proposal {
            let proposal = &mut proposals[proposal_idx];
            
            for (outcome, new_price) in updates {
                if outcome as usize >= proposal.prices.len() {
                    self.failed_ops.push((proposal_idx, BettingPlatformError::InvalidOutcome.into()));
                    continue;
                }
                
                // Update price with bounds checking
                let old_price = proposal.prices[outcome as usize];
                let max_change = old_price / 10; // 10% max change
                let clamped_price = new_price.max(old_price - max_change).min(old_price + max_change);
                
                proposal.prices[outcome as usize] = clamped_price;
                updated_count += 1;
                self.compute_used += 500; // Approximate CU cost
            }
            
            // Normalize prices to sum to 1
            normalize_prices(&mut proposal.prices);
        }
        
        Ok(BatchPriceUpdateResult {
            proposals_updated: updated_count as usize,
            failed_updates: self.failed_ops.len(),
        })
    }
    
    /// Process batch position updates
    pub fn process_batch_position_updates(
        &mut self,
        positions: &mut [Position],
        updates: &[PositionUpdate],
    ) -> Result<BatchPositionUpdateResult, ProgramError> {
        let batch_size = positions.len().min(self.max_batch_size);
        let mut updated_count = 0u32;
        
        for (i, update) in updates[..batch_size.min(updates.len())].iter().enumerate() {
            let position = &mut positions[i];
            
            // Validate update
            if position.position_id != update.position_id {
                self.failed_ops.push((i, BettingPlatformError::InvalidPosition.into()));
                continue;
            }
            
            // Apply update based on type
            match update.update_type {
                PositionUpdateType::MarkPrice(price) => {
                    position.last_mark_price = price;
                    position.update_unrealized_pnl(price);
                }
                PositionUpdateType::PartialClose(amount) => {
                    position.size = position.size.saturating_sub(amount);
                    if position.size == 0 {
                        position.is_closed = true;
                    }
                }
                PositionUpdateType::AddMargin(amount) => {
                    position.margin += amount;
                    // Recalculate liquidation price
                    position.recalculate_liquidation_price();
                }
            }
            
            updated_count += 1;
            self.successful_ops += 1;
            self.compute_used += 800; // Approximate CU cost
        }
        
        Ok(BatchPositionUpdateResult {
            positions_processed: batch_size,
            positions_updated: updated_count as usize,
            failed_updates: self.failed_ops.len(),
        })
    }
    
    /// Get batch processing report
    pub fn get_report(&self) -> BatchProcessingReport {
        BatchProcessingReport {
            operation_type: self.operation_type,
            successful_operations: self.successful_ops,
            failed_operations: self.failed_ops.len() as u32,
            compute_units_used: self.compute_used,
            efficiency_score: calculate_efficiency_score(
                self.successful_ops,
                self.failed_ops.len() as u32,
                self.compute_used,
            ),
        }
    }
}

/// Batch liquidation result
#[derive(Debug)]
pub struct BatchLiquidationResult {
    pub positions_checked: usize,
    pub positions_liquidated: usize,
    pub total_liquidated: u64,
    pub keeper_rewards: u64,
    pub liquidated_indices: Vec<usize>,
}

/// Batch settlement result
#[derive(Debug)]
pub struct BatchSettlementResult {
    pub proposals_processed: usize,
    pub proposals_settled: usize,
    pub total_volume_settled: u64,
}

/// Batch price update result
#[derive(Debug)]
pub struct BatchPriceUpdateResult {
    pub proposals_updated: usize,
    pub failed_updates: usize,
}

/// Batch position update result
#[derive(Debug)]
pub struct BatchPositionUpdateResult {
    pub positions_processed: usize,
    pub positions_updated: usize,
    pub failed_updates: usize,
}

/// Position update specification
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PositionUpdate {
    pub position_id: [u8; 32],
    pub update_type: PositionUpdateType,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum PositionUpdateType {
    MarkPrice(u64),
    PartialClose(u64),
    AddMargin(u64),
}

/// Liquidation parameters
struct LiquidationParams {
    coverage: U64F64,
    keeper_reward_bps: u16,
    current_slot: u64,
}

/// Batch processing report
#[derive(Debug)]
pub struct BatchProcessingReport {
    pub operation_type: BatchOperationType,
    pub successful_operations: u32,
    pub failed_operations: u32,
    pub compute_units_used: u64,
    pub efficiency_score: f64,
}

/// Parallel batch executor for independent operations
pub struct ParallelBatchExecutor {
    /// Worker threads (simulated as operation queues)
    queues: Vec<OperationQueue>,
    /// Maximum parallelism
    max_parallel: usize,
}

impl ParallelBatchExecutor {
    pub fn new(max_parallel: usize) -> Self {
        let queues = (0..max_parallel)
            .map(|_| OperationQueue::new())
            .collect();
        
        Self {
            queues,
            max_parallel,
        }
    }
    
    /// Execute operations in parallel
    pub fn execute_parallel<T, F>(
        &mut self,
        items: Vec<T>,
        operation: F,
    ) -> Vec<Result<(), ProgramError>>
    where
        F: Fn(&T) -> Result<(), ProgramError> + Copy,
    {
        let chunk_size = (items.len() + self.max_parallel - 1) / self.max_parallel;
        let mut results = Vec::with_capacity(items.len());
        
        // Distribute work across queues
        for (_i, chunk) in items.chunks(chunk_size).enumerate() {
            for item in chunk {
                // Simulate parallel execution
                let result = operation(item);
                results.push(result);
            }
        }
        
        results
    }
}

/// Operation queue for batch processing
struct OperationQueue {
    operations: Vec<Box<dyn FnOnce() -> Result<(), ProgramError>>>,
}

impl OperationQueue {
    fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}

/// Normalize prices to sum to 1
fn normalize_prices(prices: &mut [u64]) {
    let sum: u64 = prices.iter().sum();
    if sum == 0 {
        return;
    }
    
    for price in prices.iter_mut() {
        *price = (*price as u128 * 1_000_000 / sum as u128) as u64;
    }
}

/// Calculate efficiency score
fn calculate_efficiency_score(successful: u32, failed: u32, compute_used: u64) -> f64 {
    let total_ops = successful + failed;
    if total_ops == 0 {
        return 0.0;
    }
    
    let success_rate = successful as f64 / total_ops as f64;
    let compute_per_op = compute_used as f64 / total_ops as f64;
    let efficiency = success_rate * 1000.0 / compute_per_op;
    
    efficiency
}

/// Position trait extension for batch operations
trait PositionBatchExt {
    fn update_unrealized_pnl(&mut self, current_price: u64);
    fn recalculate_liquidation_price(&mut self);
}

impl PositionBatchExt for Position {
    fn update_unrealized_pnl(&mut self, current_price: u64) {
        let price_diff = if self.is_long {
            current_price as i64 - self.entry_price as i64
        } else {
            self.entry_price as i64 - current_price as i64
        };
        
        self.unrealized_pnl = (self.size as i128 * price_diff as i128 / self.entry_price as i128) as i64;
        self.unrealized_pnl_pct = if self.margin > 0 {
            (self.unrealized_pnl * 10000 / self.margin as i64)
        } else {
            0
        };
    }
    
    fn recalculate_liquidation_price(&mut self) {
        // Simplified calculation
        let maintenance_margin = self.size / (self.leverage as u64 * 2);
        
        if self.is_long {
            self.liquidation_price = self.entry_price.saturating_sub(
                (self.margin - maintenance_margin) * self.entry_price / self.size
            );
        } else {
            self.liquidation_price = self.entry_price.saturating_add(
                (self.margin - maintenance_margin) * self.entry_price / self.size
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_batch_liquidation_processing() {
        let mut processor = BatchProcessor::new(BatchOperationType::Liquidation);
        
        let mut positions = vec![
            create_test_position(100_000_000, 10, false), // Healthy
            create_test_position(50_000_000, 50, false),  // Unhealthy
            create_test_position(75_000_000, 40, false),  // Unhealthy
        ];
        
        let result = processor.process_batch_liquidations(
            &mut positions,
            480_000, // Current price below liquidation
            U64F64::from_num(800_000) / U64F64::from_num(1_000_000), // 0.8
            &Pubkey::new_unique(),
        ).unwrap();
        
        assert_eq!(result.positions_checked, 3);
        assert_eq!(result.positions_liquidated, 2);
        assert!(result.total_liquidated > 0);
    }
    
    #[test]
    fn test_parallel_batch_executor() {
        let mut executor = ParallelBatchExecutor::new(4);
        
        let items = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let results = executor.execute_parallel(items, |&x| {
            if x % 2 == 0 {
                Ok(())
            } else {
                Err(ProgramError::Custom(x as u32))
            }
        });
        
        assert_eq!(results.len(), 8);
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 4);
    }
    
    fn create_test_position(size: u64, leverage: u8, is_closed: bool) -> Position {
        Position {
            discriminator: [0; 8],
            version: 1,
            user: Pubkey::new_unique(),
            proposal_id: 1,
            position_id: [0; 32],
            outcome: 0,
            size,
            notional: size,
            leverage: leverage as u64,
            entry_price: 500_000,
            liquidation_price: 490_000,
            is_long: true,
            created_at: 0,
            is_closed,
            partial_liq_accumulator: 0,
            verse_id: 1,
            margin: size / leverage as u64,
            collateral: 0,
            is_short: false,
            last_mark_price: 500_000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
            entry_funding_index: Some(U64F64::from_num(0)),
        }
    }
}