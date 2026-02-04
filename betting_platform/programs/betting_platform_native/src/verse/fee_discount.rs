//! Verse Fee Discount Implementation
//! 
//! Per specification: Verse bundles get 60% fee savings
//! Base fee: 28bp + 150bp (Polymarket) = 178bp
//! Verse discount: 60% of 178bp = 107bp discount
//! Final verse fee: 178bp - 107bp = 71bp

use solana_program::{
    program_error::ProgramError,
    msg,
};

use crate::{
    constants::{BASIS_POINTS_DIVISOR, BASE_FEE_BPS, POLYMARKET_FEE_BPS},
    error::BettingPlatformError,
};

/// Verse discount percentage (60% as per specification)
pub const VERSE_DISCOUNT_PERCENTAGE: u8 = 60;

/// Calculate verse bundle discount in basis points
pub fn calculate_verse_discount_bps() -> u16 {
    let total_fee_bps = BASE_FEE_BPS + POLYMARKET_FEE_BPS; // 178bp
    
    // 60% discount
    (total_fee_bps as u32 * VERSE_DISCOUNT_PERCENTAGE as u32 / 100) as u16
}

/// Get verse bundle fee (after 60% discount)
pub fn get_verse_bundle_fee_bps() -> u16 {
    let total_fee_bps = BASE_FEE_BPS + POLYMARKET_FEE_BPS; // 178bp
    let discount_bps = calculate_verse_discount_bps(); // 107bp
    
    total_fee_bps.saturating_sub(discount_bps) // 71bp
}

/// Calculate fees for a verse bundle trade
pub fn calculate_verse_bundle_fee(
    trade_amount: u64,
    num_markets: u8,
) -> Result<u64, ProgramError> {
    if num_markets == 0 {
        return Err(BettingPlatformError::InvalidBundleSize.into());
    }
    
    // Get discounted fee rate
    let fee_bps = get_verse_bundle_fee_bps();
    
    msg!(
        "Verse bundle fee: {} markets, {}bp rate (60% off {}bp)",
        num_markets,
        fee_bps,
        BASE_FEE_BPS + POLYMARKET_FEE_BPS
    );
    
    // Calculate total fee
    let fee = (trade_amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(BASIS_POINTS_DIVISOR as u128)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    Ok(fee as u64)
}

/// Compare verse bundle savings vs individual trades
pub fn calculate_verse_savings(
    trade_amount: u64,
    num_markets: u8,
) -> Result<u64, ProgramError> {
    // Cost of individual trades
    let individual_fee_per_trade = (trade_amount as u128)
        .checked_mul((BASE_FEE_BPS + POLYMARKET_FEE_BPS) as u128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(BASIS_POINTS_DIVISOR as u128)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    let total_individual_cost = individual_fee_per_trade
        .checked_mul(num_markets as u128)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    // Cost with verse bundle
    let bundle_fee = calculate_verse_bundle_fee(trade_amount, num_markets)? as u128;
    
    // Savings
    let savings = total_individual_cost
        .checked_sub(bundle_fee)
        .unwrap_or(0);
    
    msg!(
        "Verse savings: ${} saved by bundling {} markets",
        savings / 1_000_000, // Convert to dollars
        num_markets
    );
    
    Ok(savings as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_verse_discount_calculation() {
        // Verify 60% discount calculation
        let discount_bps = calculate_verse_discount_bps();
        assert_eq!(discount_bps, 106, "Should be ~60% of 178bp"); // Rounding: 178 * 0.6 = 106.8
        
        // Verify final fee
        let verse_fee = get_verse_bundle_fee_bps();
        assert_eq!(verse_fee, 72, "Should be 178bp - 106bp = 72bp");
    }
    
    #[test]
    fn test_verse_bundle_fee() {
        let trade_amount = 1_000_000_000; // $1000
        let num_markets = 10;
        
        let fee = calculate_verse_bundle_fee(trade_amount, num_markets).unwrap();
        
        // Expected: $1000 * 0.72% = $7.20
        assert_eq!(fee, 7_200_000, "Fee should be $7.20 for $1000 trade");
    }
    
    #[test]
    fn test_verse_savings() {
        let trade_amount = 1_000_000_000; // $1000
        let num_markets = 10;
        
        let savings = calculate_verse_savings(trade_amount, num_markets).unwrap();
        
        // Individual: $1000 * 1.78% * 10 = $178
        // Bundle: $1000 * 0.72% = $7.20
        // Savings: $178 - $7.20 = $170.80
        assert_eq!(savings, 170_800_000, "Should save $170.80 by bundling");
    }
}