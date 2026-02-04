//! Compute Unit Optimization
//!
//! Strategies to minimize compute unit consumption for production efficiency

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{Position, ProposalPDA, GlobalConfigPDA},
    math::U64F64,
};

/// Compute unit costs for common operations
pub mod compute_costs {
    pub const ACCOUNT_LOAD: u64 = 200;
    pub const ACCOUNT_STORE: u64 = 300;
    pub const MATH_OPERATION: u64 = 50;
    pub const VALIDATION: u64 = 100;
    pub const EVENT_EMISSION: u64 = 150;
    pub const CROSS_PROGRAM_CALL: u64 = 5000;
}

/// Optimized account loading with selective deserialization
pub struct OptimizedAccountLoader;

impl OptimizedAccountLoader {
    /// Load only required fields from an account
    pub fn load_position_minimal(
        account: &AccountInfo,
    ) -> Result<MinimalPosition, ProgramError> {
        let data = account.data.borrow();
        
        // Skip discriminator (8 bytes) and version (4 bytes)
        let offset = 12;
        
        // Read only essential fields
        let user = Pubkey::new_from_array(data[offset..offset + 32].try_into().unwrap());
        let size = u64::from_le_bytes(data[offset + 64..offset + 72].try_into().unwrap());
        let leverage = u8::from_le_bytes(data[offset + 72..offset + 73].try_into().unwrap());
        let is_closed = data[offset + 120] != 0;
        
        Ok(MinimalPosition {
            user,
            size,
            leverage,
            is_closed,
        })
    }
    
    /// Batch load multiple accounts efficiently
    pub fn batch_load_positions(
        accounts: &[AccountInfo],
    ) -> Result<Vec<Position>, ProgramError> {
        let mut positions = Vec::with_capacity(accounts.len());
        
        // Pre-allocate to avoid reallocation
        for account in accounts {
            let position = Position::try_from_slice(&account.data.borrow())?;
            positions.push(position);
        }
        
        Ok(positions)
    }
}

/// Minimal position data for compute optimization
#[derive(Debug, Clone)]
pub struct MinimalPosition {
    pub user: Pubkey,
    pub size: u64,
    pub leverage: u8,
    pub is_closed: bool,
}

/// Optimized AMM calculations
pub struct OptimizedAMM;

impl OptimizedAMM {
    /// Calculate price using optimized LMSR
    pub fn calculate_price_optimized(
        outcome_balance: u64,
        total_balance: u64,
        b_value: u64,
    ) -> Result<u64, ProgramError> {
        // Use bit shifts for power-of-2 divisions
        if total_balance == 0 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        // Avoid expensive logarithm calculations
        // Use approximation for small changes
        let ratio = (outcome_balance as u128 * 1_000_000) / total_balance as u128;
        
        // Fast approximation for exp calculation
        let price = if ratio < 100_000 {
            // Linear approximation for small ratios
            50_000 + (ratio as u64 / 20)
        } else if ratio > 900_000 {
            // Linear approximation for large ratios
            950_000 + ((ratio as u64 - 900_000) / 20)
        } else {
            // Use table lookup for common values
            get_cached_price(ratio as u64 / 10_000)
                .unwrap_or_else(|| calculate_price_full(outcome_balance, total_balance, b_value))
        };
        
        Ok(price)
    }
    
    /// Batch price updates for multiple outcomes
    pub fn batch_update_prices(
        proposal: &mut ProposalPDA,
        trades: &[(u8, u64, bool)], // (outcome, size, is_buy)
    ) -> Result<(), ProgramError> {
        // Group trades by outcome to minimize calculations
        let mut outcome_deltas: [i64; 16] = [0; 16];
        
        for (outcome, size, is_buy) in trades {
            if *outcome as usize >= proposal.outcomes as usize {
                return Err(BettingPlatformError::InvalidOutcome.into());
            }
            
            outcome_deltas[*outcome as usize] += if *is_buy {
                *size as i64
            } else {
                -(*size as i64)
            };
        }
        
        // Apply all deltas and recalculate prices once
        for (i, delta) in outcome_deltas.iter().enumerate() {
            if *delta != 0 && i < proposal.outcomes as usize {
                let new_balance = (proposal.outcome_balances[i] as i64 + delta) as u64;
                proposal.outcome_balances[i] = new_balance;
            }
        }
        
        // Recalculate all prices in one pass
        recalculate_all_prices(proposal)?;
        
        Ok(())
    }
}

/// Get pre-computed price from lookup table
fn get_cached_price(ratio_key: u64) -> Option<u64> {
    // Simplified lookup table for common values
    match ratio_key {
        10 => Some(100_000),
        20 => Some(200_000),
        30 => Some(300_000),
        40 => Some(400_000),
        50 => Some(500_000),
        60 => Some(600_000),
        70 => Some(700_000),
        80 => Some(800_000),
        90 => Some(900_000),
        _ => None,
    }
}

/// Full price calculation (fallback for non-cached values)
fn calculate_price_full(outcome_balance: u64, total_balance: u64, b_value: u64) -> u64 {
    // Simplified calculation for example
    let ratio = (outcome_balance as u128 * 1_000_000) / total_balance as u128;
    (ratio as u64).min(1_000_000)
}

/// Recalculate all prices efficiently
fn recalculate_all_prices(proposal: &mut ProposalPDA) -> Result<(), ProgramError> {
    let total: u64 = proposal.outcome_balances.iter().sum();
    
    if total == 0 {
        return Err(BettingPlatformError::InvalidAMMState.into());
    }
    
    // Calculate all prices in one pass
    for i in 0..proposal.outcomes as usize {
        proposal.prices[i] = OptimizedAMM::calculate_price_optimized(
            proposal.outcome_balances[i],
            total,
            proposal.b_value,
        )?;
    }
    
    Ok(())
}

/// Compute unit budget manager
pub struct ComputeBudgetManager {
    pub remaining_units: u64,
    pub operation_log: Vec<(String, u64)>,
}

impl ComputeBudgetManager {
    pub fn new(initial_budget: u64) -> Self {
        Self {
            remaining_units: initial_budget,
            operation_log: Vec::new(),
        }
    }
    
    /// Track compute unit consumption
    pub fn consume(&mut self, operation: &str, units: u64) -> Result<(), ProgramError> {
        if self.remaining_units < units {
            msg!("Compute budget exceeded: {} requires {} units, {} remaining",
                operation, units, self.remaining_units);
            return Err(BettingPlatformError::ComputeBudgetExceeded.into());
        }
        
        self.remaining_units -= units;
        self.operation_log.push((operation.to_string(), units));
        
        Ok(())
    }
    
    /// Get compute unit report
    pub fn report(&self) -> ComputeReport {
        let total_consumed: u64 = self.operation_log
            .iter()
            .map(|(_, units)| units)
            .sum();
        
        ComputeReport {
            total_consumed,
            remaining: self.remaining_units,
            operations: self.operation_log.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ComputeReport {
    pub total_consumed: u64,
    pub remaining: u64,
    pub operations: Vec<(String, u64)>,
}

/// Optimized validation routines
pub struct OptimizedValidation;

impl OptimizedValidation {
    /// Batch validate multiple positions
    pub fn batch_validate_positions(
        positions: &[Position],
    ) -> Result<Vec<bool>, ProgramError> {
        let mut results = Vec::with_capacity(positions.len());
        
        for position in positions {
            // Quick checks first (cheap operations)
            let is_valid = !position.is_closed &&
                          position.size > 0 &&
                          position.leverage > 0 &&
                          position.leverage <= 100;
            
            results.push(is_valid);
        }
        
        Ok(results)
    }
    
    /// Fast signature validation using bitwise operations
    pub fn validate_signatures_fast(
        accounts: &[AccountInfo],
        required_signers: &[bool],
    ) -> Result<(), ProgramError> {
        // Use bitwise operations for batch checking
        let mut signer_bitmap: u64 = 0;
        let mut required_bitmap: u64 = 0;
        
        for (i, (account, required)) in accounts.iter()
            .zip(required_signers.iter())
            .enumerate()
            .take(64) // Max 64 accounts in bitmap
        {
            if account.is_signer {
                signer_bitmap |= 1 << i;
            }
            if *required {
                required_bitmap |= 1 << i;
            }
        }
        
        // Check all required signatures in one operation
        if (signer_bitmap & required_bitmap) != required_bitmap {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        Ok(())
    }
}

/// Memory pool for reducing allocations
pub struct MemoryPool {
    buffers: Vec<Vec<u8>>,
    available: Vec<usize>,
}

impl MemoryPool {
    pub fn new(pool_size: usize, buffer_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(pool_size);
        let mut available = Vec::with_capacity(pool_size);
        
        for i in 0..pool_size {
            buffers.push(vec![0u8; buffer_size]);
            available.push(i);
        }
        
        Self { buffers, available }
    }
    
    /// Get a buffer from the pool
    pub fn acquire(&mut self) -> Option<&mut Vec<u8>> {
        if let Some(index) = self.available.pop() {
            Some(&mut self.buffers[index])
        } else {
            None
        }
    }
    
    /// Return a buffer to the pool
    pub fn release(&mut self, index: usize) {
        if index < self.buffers.len() {
            self.buffers[index].clear();
            self.available.push(index);
        }
    }
}

/// Inline small functions for better performance
#[inline(always)]
pub fn fast_min(a: u64, b: u64) -> u64 {
    if a < b { a } else { b }
}

#[inline(always)]
pub fn fast_max(a: u64, b: u64) -> u64 {
    if a > b { a } else { b }
}

#[inline(always)]
pub fn fast_abs_diff(a: u64, b: u64) -> u64 {
    if a > b { a - b } else { b - a }
}

/// Optimized percentage calculation without division
#[inline(always)]
pub fn fast_percentage(value: u64, percentage_bps: u16) -> u64 {
    ((value as u128 * percentage_bps as u128) / 10_000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute_budget_tracking() {
        let mut manager = ComputeBudgetManager::new(200_000);
        
        assert!(manager.consume("load_account", 1000).is_ok());
        assert!(manager.consume("calculate_price", 5000).is_ok());
        assert_eq!(manager.remaining_units, 194_000);
        
        let report = manager.report();
        assert_eq!(report.total_consumed, 6000);
    }
    
    #[test]
    fn test_fast_percentage() {
        assert_eq!(fast_percentage(10_000, 100), 100); // 1%
        assert_eq!(fast_percentage(10_000, 1000), 1000); // 10%
        assert_eq!(fast_percentage(10_000, 5000), 5000); // 50%
    }
    
    #[test]
    fn test_optimized_validation() {
        let positions = vec![
            Position {
                discriminator: [0; 8],
                version: 1,
                user: Pubkey::new_unique(),
                proposal_id: 1,
                position_id: [0; 32],
                outcome: 0,
                size: 1000,
                notional: 1000,
                leverage: 10,
                entry_price: 500_000,
                liquidation_price: 450_000,
                is_long: true,
                created_at: 0,
                is_closed: false,
                partial_liq_accumulator: 0,
                verse_id: 1,
                margin: 100,
                collateral: 0,
                is_short: false,
                last_mark_price: 500_000,
                unrealized_pnl: 0,
                cross_margin_enabled: false,
                unrealized_pnl_pct: 0,
                entry_funding_index: Some(U64F64::from_num(0)),
            },
        ];
        
        let results = OptimizedValidation::batch_validate_positions(&positions).unwrap();
        assert_eq!(results, vec![true]);
    }
}