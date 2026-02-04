//! Polymarket Fee Integration
//! 
//! Implements additive fee calculation per Part 7 specification:
//! Total_fee = model_fee (3-28bp) + polymarket_fee (1.5% avg)

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::{
    error::BettingPlatformError,
    math::U64F64,
    fees::elastic_fee::calculate_elastic_fee,
};

/// Polymarket fee configuration
pub struct PolymarketFeeConfig {
    /// Base polymarket fee in basis points (150bp = 1.5%)
    pub base_fee_bps: u16,
    /// Premium tier fee discount (for high volume)
    pub premium_discount_bps: u16,
    /// Volume threshold for premium tier (in USDC)
    pub premium_volume_threshold: u64,
}

impl Default for PolymarketFeeConfig {
    fn default() -> Self {
        Self {
            base_fee_bps: 150,        // 1.5% base fee
            premium_discount_bps: 50,  // 0.5% discount for premium
            premium_volume_threshold: 1_000_000_000, // $1M volume
        }
    }
}

/// Fee breakdown for transparency
#[derive(Debug, Clone, Copy)]
pub struct FeeBreakdown {
    /// Model fee (elastic 3-28bp)
    pub model_fee_bps: u16,
    /// Polymarket routing fee
    pub polymarket_fee_bps: u16,
    /// Total combined fee
    pub total_fee_bps: u16,
    /// Amount saved by bundling
    pub savings_bps: u16,
}

/// Calculate total additive fees for a trade
pub fn calculate_total_fees(
    amount: u64,
    coverage: U64F64,
    user_volume_7d: u64,
    is_bundled: bool,
) -> Result<(u64, FeeBreakdown), ProgramError> {
    // Calculate model fee (3-28bp based on coverage)
    let model_fee_bps = calculate_elastic_fee(coverage)?;
    let model_fee = (amount as u128)
        .checked_mul(model_fee_bps as u128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(10000)
        .ok_or(BettingPlatformError::MathOverflow)? as u64;
    
    // Get polymarket fee config
    let config = PolymarketFeeConfig::default();
    
    // Calculate polymarket fee with volume discount
    let mut polymarket_fee_bps = config.base_fee_bps;
    if user_volume_7d >= config.premium_volume_threshold {
        polymarket_fee_bps = polymarket_fee_bps.saturating_sub(config.premium_discount_bps);
    }
    
    // Apply bundling discount (40% reduction per spec)
    let savings_bps = if is_bundled {
        (polymarket_fee_bps * 40) / 100  // 40% savings
    } else {
        0
    };
    
    let effective_polymarket_fee_bps = polymarket_fee_bps.saturating_sub(savings_bps);
    
    // Calculate polymarket fee amount
    let polymarket_fee = (amount as u128)
        .checked_mul(effective_polymarket_fee_bps as u128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(10000)
        .ok_or(BettingPlatformError::MathOverflow)? as u64;
    
    // Total fee is additive
    let total_fee = model_fee
        .checked_add(polymarket_fee)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    let total_fee_bps = model_fee_bps + effective_polymarket_fee_bps;
    
    let breakdown = FeeBreakdown {
        model_fee_bps,
        polymarket_fee_bps: effective_polymarket_fee_bps,
        total_fee_bps,
        savings_bps,
    };
    
    msg!(
        "Fee breakdown: Model {}bp, Polymarket {}bp (saved {}bp), Total {}bp",
        model_fee_bps, effective_polymarket_fee_bps, savings_bps, total_fee_bps
    );
    
    Ok((total_fee, breakdown))
}

/// Calculate fee savings from bundling multiple trades
pub fn calculate_bundle_savings(
    trades: &[(u64, U64F64)], // (amount, coverage) pairs
    user_volume_7d: u64,
) -> Result<u64, ProgramError> {
    let mut total_individual_fees = 0u64;
    let mut total_bundled_fees = 0u64;
    
    for &(amount, coverage) in trades {
        // Individual trade fees
        let (individual_fee, _) = calculate_total_fees(
            amount,
            coverage,
            user_volume_7d,
            false, // not bundled
        )?;
        total_individual_fees = total_individual_fees
            .checked_add(individual_fee)
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Bundled trade fees
        let (bundled_fee, _) = calculate_total_fees(
            amount,
            coverage,
            user_volume_7d,
            true, // bundled
        )?;
        total_bundled_fees = total_bundled_fees
            .checked_add(bundled_fee)
            .ok_or(BettingPlatformError::MathOverflow)?;
    }
    
    // Calculate savings
    let savings = total_individual_fees.saturating_sub(total_bundled_fees);
    
    msg!(
        "Bundle savings: ${} (individual: ${}, bundled: ${})",
        savings / 1_000_000,
        total_individual_fees / 1_000_000,
        total_bundled_fees / 1_000_000
    );
    
    Ok(savings)
}

/// Format fee breakdown for UX display
pub fn format_fee_breakdown(breakdown: FeeBreakdown, amount: u64) -> String {
    let model_fee_usd = (amount as u128 * breakdown.model_fee_bps as u128 / 10000) as u64;
    let polymarket_fee_usd = (amount as u128 * breakdown.polymarket_fee_bps as u128 / 10000) as u64;
    let total_fee_usd = (amount as u128 * breakdown.total_fee_bps as u128 / 10000) as u64;
    let savings_usd = (amount as u128 * breakdown.savings_bps as u128 / 10000) as u64;
    
    format!(
        "Model: ${:.2}, Routed: ${:.2}, Total: ${:.2} (Saved: ${:.2})",
        model_fee_usd as f64 / 1_000_000.0,
        polymarket_fee_usd as f64 / 1_000_000.0,
        total_fee_usd as f64 / 1_000_000.0,
        savings_usd as f64 / 1_000_000.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_additive_fees() {
        // Test with low coverage (high model fee)
        let coverage = U64F64::from_num(1) / U64F64::from_num(2); // 0.5
        let amount = 10_000_000_000; // $10k
        let user_volume = 0;
        
        let (total_fee, breakdown) = calculate_total_fees(
            amount,
            coverage,
            user_volume,
            false,
        ).unwrap();
        
        // Model fee should be ~8.575bp (from spec formula)
        // Polymarket fee should be 150bp
        // Total should be ~158.575bp
        assert!(breakdown.model_fee_bps > 8 && breakdown.model_fee_bps < 10);
        assert_eq!(breakdown.polymarket_fee_bps, 150);
        assert_eq!(breakdown.total_fee_bps, breakdown.model_fee_bps + 150);
        assert_eq!(breakdown.savings_bps, 0); // No bundling
        
        // Test bundled trade (40% savings on polymarket fee)
        let (bundled_fee, bundled_breakdown) = calculate_total_fees(
            amount,
            coverage,
            user_volume,
            true,
        ).unwrap();
        
        assert_eq!(bundled_breakdown.savings_bps, 60); // 40% of 150bp
        assert_eq!(bundled_breakdown.polymarket_fee_bps, 90); // 150bp - 60bp
        assert!(bundled_fee < total_fee);
    }
    
    #[test]
    fn test_premium_discount() {
        let coverage = U64F64::from_num(1);
        let amount = 10_000_000_000; // $10k
        let premium_volume = 1_000_000_000_000; // $1M
        
        let (_, breakdown) = calculate_total_fees(
            amount,
            coverage,
            premium_volume,
            false,
        ).unwrap();
        
        // Should get 50bp discount for premium tier
        assert_eq!(breakdown.polymarket_fee_bps, 100); // 150bp - 50bp
    }
    
    #[test]
    fn test_bundle_savings() {
        let trades = vec![
            (10_000_000_000, U64F64::from_num(1)), // $10k
            (20_000_000_000, U64F64::from_num(4) / U64F64::from_num(5)), // $20k, 0.8
            (15_000_000_000, U64F64::from_num(6) / U64F64::from_num(5)), // $15k, 1.2
        ];
        
        let savings = calculate_bundle_savings(&trades, 0).unwrap();
        
        // Should save 40% of polymarket fees
        // Total: $45k * 1.5% * 40% = $270
        assert!(savings > 250_000_000 && savings < 300_000_000);
    }
}