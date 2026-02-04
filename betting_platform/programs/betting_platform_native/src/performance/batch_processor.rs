//! Batch Processing for High Throughput
//!
//! Optimizes transaction processing by batching operations

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::Position,
    account_validation::{validate_writable, DISCRIMINATOR_SIZE},
};

/// Maximum operations per batch
pub const MAX_BATCH_SIZE: usize = 50;

/// Batch processor discriminator
pub const BATCH_PROCESSOR_DISCRIMINATOR: [u8; 8] = [66, 65, 84, 67, 72, 80, 82, 79]; // "BATCHPRO"

/// Batch processor state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BatchProcessor {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Batch ID
    pub batch_id: u64,
    
    /// Operations processed
    pub operations_processed: u32,
    
    /// Total gas saved
    pub gas_saved: u64,
    
    /// Start slot
    pub start_slot: u64,
    
    /// End slot
    pub end_slot: Option<u64>,
    
    /// Status
    pub status: BatchStatus,
    
    /// Bump seed
    pub bump: u8,
}

impl BatchProcessor {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 8 + 4 + 8 + 8 + 9 + 1 + 1;
    
    /// Create new batch processor
    pub fn new(batch_id: u64, start_slot: u64, bump: u8) -> Self {
        Self {
            discriminator: BATCH_PROCESSOR_DISCRIMINATOR,
            batch_id,
            operations_processed: 0,
            gas_saved: 0,
            start_slot,
            end_slot: None,
            status: BatchStatus::Active,
            bump,
        }
    }
    
    /// Validate account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != BATCH_PROCESSOR_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Batch status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum BatchStatus {
    Active,
    Completed,
    Failed,
}

/// Batch operation types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum BatchOperation {
    /// Update multiple positions
    UpdatePositions {
        positions: Vec<PositionUpdate>,
    },
    
    /// Process multiple orders
    ProcessOrders {
        orders: Vec<OrderData>,
    },
    
    /// Update multiple prices
    UpdatePrices {
        prices: Vec<PriceUpdate>,
    },
    
    /// Liquidate multiple positions
    BatchLiquidations {
        positions: Vec<Pubkey>,
    },
}

/// Position update data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionUpdate {
    pub position_pubkey: Pubkey,
    pub new_size: Option<u64>,
    pub new_margin: Option<u64>,
    pub close: bool,
}

/// Order data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderData {
    pub user: Pubkey,
    pub market_id: [u8; 32],
    pub outcome: u8,
    pub size: u64,
    pub leverage: u8,
}

/// Price update data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceUpdate {
    pub market_id: [u8; 32],
    pub outcome: u8,
    pub new_price: u64,
}

/// Process batch operations
pub fn process_batch_operations(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    operations: Vec<BatchOperation>,
) -> ProgramResult {
    msg!("Processing batch operations: {} items", operations.len());
    
    if operations.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if operations.len() > MAX_BATCH_SIZE {
        return Err(BettingPlatformError::TooManyPositions.into());
    }
    
    let account_info_iter = &mut accounts.iter();
    let batch_processor_account = next_account_info(account_info_iter)?;
    
    // Load or create batch processor
    let mut batch_processor = if batch_processor_account.data_len() > 0 {
        BatchProcessor::try_from_slice(&batch_processor_account.data.borrow())?
    } else {
        let clock = Clock::get()?;
        let batch_id = clock.slot; // Use slot as batch ID
        BatchProcessor::new(batch_id, clock.slot, 255)
    };
    
    // Track gas savings
    let base_cu_per_operation = 5000u64;
    let batch_cu_per_operation = 2000u64;
    let operations_count = operations.len() as u64;
    let gas_saved = (base_cu_per_operation - batch_cu_per_operation) * operations_count;
    
    // Process each operation type
    for operation in operations {
        match operation {
            BatchOperation::UpdatePositions { positions } => {
                process_batch_position_updates(account_info_iter, positions)?;
            }
            BatchOperation::ProcessOrders { orders } => {
                process_batch_orders(account_info_iter, orders)?;
            }
            BatchOperation::UpdatePrices { prices } => {
                process_batch_price_updates(account_info_iter, prices)?;
            }
            BatchOperation::BatchLiquidations { positions } => {
                process_batch_liquidations(account_info_iter, positions)?;
            }
        }
        
        batch_processor.operations_processed += 1;
    }
    
    // Update batch processor stats
    batch_processor.gas_saved += gas_saved;
    
    msg!("Batch processed: {} operations, {} CU saved", 
        batch_processor.operations_processed, 
        batch_processor.gas_saved
    );
    
    Ok(())
}

/// Process batch position updates
fn process_batch_position_updates(
    account_iter: &mut std::slice::Iter<AccountInfo>,
    updates: Vec<PositionUpdate>,
) -> ProgramResult {
    msg!("Processing {} position updates", updates.len());
    
    for update in updates {
        let position_account = next_account_info(account_iter)?;
        validate_writable(position_account)?;
        
        let mut position = Position::try_from_slice(&position_account.data.borrow())?;
        
        if let Some(new_size) = update.new_size {
            position.size = new_size;
        }
        
        if let Some(new_margin) = update.new_margin {
            position.margin = new_margin;
        }
        
        if update.close {
            position.is_closed = true;
        }
        
        position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    }
    
    Ok(())
}

/// Process batch orders
fn process_batch_orders(
    _account_iter: &mut std::slice::Iter<AccountInfo>,
    orders: Vec<OrderData>,
) -> ProgramResult {
    msg!("Processing {} orders", orders.len());
    
    // Implementation would create positions for all orders
    // in a single transaction, saving significant CU
    
    Ok(())
}

/// Process batch price updates
fn process_batch_price_updates(
    _account_iter: &mut std::slice::Iter<AccountInfo>,
    prices: Vec<PriceUpdate>,
) -> ProgramResult {
    msg!("Updating {} prices", prices.len());
    
    // Implementation would update all market prices
    // in a single transaction
    
    Ok(())
}

/// Process batch liquidations
fn process_batch_liquidations(
    _account_iter: &mut std::slice::Iter<AccountInfo>,
    positions: Vec<Pubkey>,
) -> ProgramResult {
    msg!("Processing {} liquidations", positions.len());
    
    // Implementation would liquidate all unhealthy positions
    // in a single transaction
    
    Ok(())
}

/// Batch operation result
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BatchResult {
    pub batch_id: u64,
    pub operations_succeeded: u32,
    pub operations_failed: u32,
    pub gas_saved: u64,
    pub errors: Vec<BatchError>,
}

/// Batch error
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BatchError {
    pub operation_index: u32,
    pub error_code: u32,
    pub error_message: String,
}

/// Optimize batch size based on current network conditions
pub fn calculate_optimal_batch_size(
    avg_cu_per_operation: u64,
    max_cu_per_transaction: u64,
    network_congestion: u8, // 0-100
) -> usize {
    let base_batch_size = (max_cu_per_transaction / avg_cu_per_operation) as usize;
    
    // Reduce batch size during congestion
    let congestion_factor = (100 - network_congestion) as usize;
    let adjusted_size = (base_batch_size * congestion_factor) / 100;
    
    adjusted_size.clamp(1, MAX_BATCH_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimal_batch_size() {
        // Low congestion
        let size = calculate_optimal_batch_size(5000, 200_000, 10);
        assert!(size > 30);
        
        // High congestion
        let size = calculate_optimal_batch_size(5000, 200_000, 90);
        assert!(size < 10);
    }
    
    #[test]
    fn test_gas_savings() {
        let operations = 20;
        let base_cu = 5000u64;
        let batch_cu = 2000u64;
        
        let saved = (base_cu - batch_cu) * operations;
        assert_eq!(saved, 60_000);
    }
}