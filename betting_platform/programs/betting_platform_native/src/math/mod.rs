//! Math module for betting platform
//!
//! Contains mathematical functions, constants, and types

// Public modules
pub mod fixed_point;
pub mod leverage;
pub mod special_functions;
pub mod table_lookup;
pub mod tables;
pub mod dynamic_leverage;
pub mod u256;

// Re-export commonly used items
pub use fixed_point::{U64F64, U128F128};
pub use leverage::*;
pub use special_functions::*;
pub use table_lookup::*;
pub use dynamic_leverage::*;
pub use u256::U256;

#[cfg(test)]
mod test_fixed_point;

// Helper functions module
pub mod helpers {
    use super::*;
    use solana_program::program_error::ProgramError;
    use crate::error::BettingPlatformError;
    
    /// Calculate percentage of a value
    pub fn calculate_percentage(value: u64, bps: u16) -> Result<u64, ProgramError> {
        let result = (value as u128)
            .checked_mul(bps as u128)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        if result > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(result as u64)
    }
    
    /// Apply leverage to a size
    pub fn apply_leverage(size: u64, leverage: u64) -> Result<u64, ProgramError> {
        let result = (size as u128)
            .checked_mul(leverage as u128)
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        if result > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(result as u64)
    }
}

/// Calculate profit and loss for a position
pub fn calculate_pnl(
    entry_price: u64,
    exit_price: u64,
    size: u64,
    leverage: u64,
    is_long: bool,
) -> Result<i64, solana_program::program_error::ProgramError> {
    use crate::constants::LEVERAGE_PRECISION;
    use crate::error::BettingPlatformError;
    
    // Calculate price difference
    let price_diff = if is_long {
        exit_price as i128 - entry_price as i128
    } else {
        entry_price as i128 - exit_price as i128
    };
    
    // Apply leverage to PnL
    // PnL = (price_diff / entry_price) * size * leverage
    let pnl = price_diff
        .checked_mul(size as i128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_mul(leverage as i128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(entry_price as i128)
        .ok_or(BettingPlatformError::DivisionByZero)?
        .checked_div(LEVERAGE_PRECISION as i128)
        .ok_or(BettingPlatformError::DivisionByZero)?;
    
    // Check bounds
    if pnl > i64::MAX as i128 || pnl < i64::MIN as i128 {
        return Err(BettingPlatformError::MathOverflow.into());
    }
    
    Ok(pnl as i64)
}