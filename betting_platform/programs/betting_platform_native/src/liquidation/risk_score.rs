//! Risk score calculation for positions
//!
//! Calculates risk scores to determine liquidation eligibility

use solana_program::program_error::ProgramError;

use crate::{
    math::U64F64,
    state::Position,
};

/// Calculate risk score for a position (0-100)
pub fn calculate_risk_score(position: &Position) -> Result<u8, ProgramError> {
    // Risk score based on how close price is to liquidation price
    let current_price = position.entry_price; // Simplified, would use oracle price
    let risk_score = calculate_risk_score_with_price(position, U64F64::from_num(current_price))?;
    Ok(risk_score)
}

/// Calculate risk score with specific price
pub fn calculate_risk_score_with_price(
    position: &Position,
    current_price: U64F64,
) -> Result<u8, ProgramError> {
    let entry_price = U64F64::from_num(position.entry_price);
    let liq_price = U64F64::from_num(position.liquidation_price);
    
    // Calculate distance to liquidation
    let distance = if position.is_long {
        // Long: risk increases as price approaches liquidation price from above
        if current_price <= liq_price {
            return Ok(100); // Already past liquidation
        }
        
        entry_price
            .checked_sub(liq_price)?
    } else {
        // Short: risk increases as price approaches liquidation price from below
        if current_price >= liq_price {
            return Ok(100); // Already past liquidation
        }
        
        liq_price
            .checked_sub(entry_price)?
    };
    
    // Calculate how close we are to liquidation
    let current_distance = if position.is_long {
        current_price
            .checked_sub(liq_price)?
    } else {
        liq_price
            .checked_sub(current_price)?
    };
    
    // Risk score = (1 - current_distance/total_distance) * 100
    if distance > U64F64::from_num(0) {
        let ratio = current_distance
            .checked_div(distance)?;
            
        let risk_ratio = U64F64::from_num(1u64)
            .checked_sub(ratio)
            .unwrap_or(U64F64::from_num(0));
            
        let risk_score = risk_ratio
            .checked_mul(U64F64::from_num(100))?
            .to_num() as u8;
            
        Ok(risk_score.min(100))
    } else {
        Ok(100)
    }
}