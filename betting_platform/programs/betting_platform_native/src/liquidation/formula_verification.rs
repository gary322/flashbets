//! Liquidation Formula Verification Module
//!
//! Implements and verifies the specification-compliant liquidation formula:
//! liq_price = entry_price * (1 - (margin_ratio / lev_eff))

use solana_program::{
    msg,
    program_error::ProgramError,
};

use crate::{
    error::BettingPlatformError,
    math::U64F64,
};

/// Calculate liquidation price using the specification formula
/// liq_price = entry_price * (1 - (margin_ratio / lev_eff))
/// 
/// For long positions: liquidation when price drops
/// For short positions: formula is adjusted to liq_price = entry_price * (1 + (margin_ratio / lev_eff))
pub fn calculate_liquidation_price_spec(
    entry_price: u64,
    margin_ratio: u64, // In basis points (10000 = 1.0)
    effective_leverage: u64,
    is_long: bool,
) -> Result<u64, ProgramError> {
    if effective_leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    // Convert to fixed point
    let entry_fp = U64F64::from_num(entry_price);
    let margin_ratio_fp = U64F64::from_num(margin_ratio) / U64F64::from_num(10000);
    let lev_eff_fp = U64F64::from_num(effective_leverage);
    
    // Calculate margin_ratio / lev_eff
    let ratio = margin_ratio_fp
        .checked_div(lev_eff_fp)
        .unwrap_or(U64F64::from_num(0));
    
    if is_long {
        // Long: liq_price = entry_price * (1 - (margin_ratio / lev_eff))
        let factor = U64F64::from_num(1)
            .checked_sub(ratio)
            .unwrap_or(U64F64::from_num(0)); // Clamp to 0 if negative
        
        let liq_price = entry_fp
            .checked_mul(factor)
            .unwrap_or(U64F64::from_num(0));
        
        Ok(liq_price.to_num())
    } else {
        // Short: liq_price = entry_price * (1 + (margin_ratio / lev_eff))
        let factor = U64F64::from_num(1)
            .checked_add(ratio)
            .unwrap_or(U64F64::from_num(1));
        
        let liq_price = entry_fp
            .checked_mul(factor)
            .unwrap_or(U64F64::from_num(0));
        
        Ok(liq_price.to_num())
    }
}

/// Calculate margin ratio using the existing formula
/// MR = 1/lev + sigma * sqrt(lev) * f(n)
/// Returns margin ratio in basis points
pub fn calculate_margin_ratio_spec(
    base_leverage: u64,
    sigma: u64, // In basis points
    num_positions: u64,
) -> Result<u64, ProgramError> {
    if base_leverage == 0 {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    // Calculate 1/leverage in basis points
    let base_margin_bps = 10000u64 / base_leverage;
    
    // Calculate f(n) = 1 + 0.1 * (n-1)
    let f_n = 10000u64 + 1000u64 * num_positions.saturating_sub(1);
    
    // Calculate sqrt(leverage) using integer approximation
    let sqrt_lev = integer_sqrt(base_leverage);
    
    // sigma * sqrt(lev) * f(n) / 10000 (to normalize f(n))
    let volatility_component = (sigma * sqrt_lev * f_n) / (10000 * 10000);
    
    Ok(base_margin_bps + volatility_component)
}

/// Calculate effective leverage for positions
/// Formula: effective_leverage = position_leverage × (1 - unrealized_pnl_pct)
/// For chain positions: also apply chain_multiplier
pub fn calculate_effective_leverage(
    base_leverage: u64,
    chain_multiplier: Option<u64>, // In basis points (10000 = 1.0x)
    unrealized_pnl_pct: Option<i64>, // In basis points (10000 = 100%)
) -> Result<u64, ProgramError> {
    let mut effective = base_leverage;
    
    // First apply PnL adjustment if provided
    if let Some(pnl_pct) = unrealized_pnl_pct {
        // Calculate (1 - unrealized_pnl_pct) in basis points
        let adjustment_factor = 10000i64 - pnl_pct;
        
        // Ensure adjustment factor doesn't go below 10% (minimum 0.1x multiplier)
        let safe_adjustment = adjustment_factor.max(1000);
        
        // Apply PnL adjustment
        effective = ((effective as i64 * safe_adjustment) / 10000).max(1) as u64;
    }
    
    // Then apply chain multiplier if provided
    if let Some(multiplier) = chain_multiplier {
        effective = (effective as u128)
            .checked_mul(multiplier as u128)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::DivisionByZero)? as u64;
    }
    
    // Cap at 500x as per specification
    Ok(effective.min(500))
}

/// Verify liquidation price calculation matches specification
pub fn verify_liquidation_calculation(
    entry_price: u64,
    base_leverage: u64,
    effective_leverage: u64,
    sigma: u64,
    num_positions: u64,
    is_long: bool,
) -> Result<LiquidationVerification, ProgramError> {
    // Calculate margin ratio
    let margin_ratio = calculate_margin_ratio_spec(base_leverage, sigma, num_positions)?;
    
    // Calculate liquidation price using spec formula
    let liq_price_spec = calculate_liquidation_price_spec(
        entry_price,
        margin_ratio,
        effective_leverage,
        is_long,
    )?;
    
    // Calculate liquidation percentage (how much price needs to move)
    let liq_percentage = if is_long {
        // Long: percentage drop to liquidation
        let drop = entry_price.saturating_sub(liq_price_spec);
        (drop * 10000) / entry_price
    } else {
        // Short: percentage rise to liquidation
        let rise = liq_price_spec.saturating_sub(entry_price);
        (rise * 10000) / entry_price
    };
    
    Ok(LiquidationVerification {
        entry_price,
        liquidation_price: liq_price_spec,
        margin_ratio,
        base_leverage,
        effective_leverage,
        liquidation_percentage: liq_percentage,
        is_long,
    })
}

/// Liquidation verification result
#[derive(Debug)]
pub struct LiquidationVerification {
    pub entry_price: u64,
    pub liquidation_price: u64,
    pub margin_ratio: u64,
    pub base_leverage: u64,
    pub effective_leverage: u64,
    pub liquidation_percentage: u64, // In basis points
    pub is_long: bool,
}

impl LiquidationVerification {
    pub fn log_details(&self) {
        msg!("=== Liquidation Calculation Verification ===");
        msg!("Entry Price: ${}", self.entry_price / 1_000_000);
        msg!("Base Leverage: {}x", self.base_leverage);
        msg!("Effective Leverage: {}x", self.effective_leverage);
        msg!("Margin Ratio: {}bps ({:.2}%)", 
            self.margin_ratio, 
            self.margin_ratio as f64 / 100.0
        );
        msg!("Position Type: {}", if self.is_long { "LONG" } else { "SHORT" });
        msg!("Liquidation Price: ${}", self.liquidation_price / 1_000_000);
        msg!("Liquidation Buffer: {}bps ({:.2}%)", 
            self.liquidation_percentage,
            self.liquidation_percentage as f64 / 100.0
        );
        msg!("Formula: liq_price = entry_price * (1 {} (margin_ratio / lev_eff))",
            if self.is_long { "-" } else { "+" }
        );
    }
}

/// Integer square root approximation
fn integer_sqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_liquidation_price_spec() {
        // Test case: 10x leverage, entry at $1.00
        let entry_price = 1_000_000; // $1.00 with 6 decimals
        let base_leverage = 10;
        let effective_leverage = 10;
        let sigma = 150; // 1.5% in basis points
        let num_positions = 1;
        
        // Calculate margin ratio
        let margin_ratio = calculate_margin_ratio_spec(base_leverage, sigma, num_positions).unwrap();
        
        // Base margin: 1/10 = 10%
        // Volatility: 1.5% * sqrt(10) * 1 ≈ 4.74%
        // Total margin ratio ≈ 14.74%
        
        // Long position liquidation price
        let liq_price_long = calculate_liquidation_price_spec(
            entry_price,
            margin_ratio,
            effective_leverage,
            true,
        ).unwrap();
        
        // Expected: $1.00 * (1 - 0.1474/10) ≈ $1.00 * 0.985 ≈ $0.985
        assert!(liq_price_long < entry_price);
        assert!(liq_price_long > 980_000); // Should be above $0.98
        
        // Short position liquidation price
        let liq_price_short = calculate_liquidation_price_spec(
            entry_price,
            margin_ratio,
            effective_leverage,
            false,
        ).unwrap();
        
        // Expected: $1.00 * (1 + 0.1474/10) ≈ $1.00 * 1.015 ≈ $1.015
        assert!(liq_price_short > entry_price);
        assert!(liq_price_short < 1_020_000); // Should be below $1.02
    }
    
    #[test]
    fn test_effective_leverage_calculation() {
        // Test without any adjustments
        let base_leverage = 10;
        let effective = calculate_effective_leverage(base_leverage, None, None).unwrap();
        assert_eq!(effective, 10);
        
        // Test with 2x chain multiplier
        let effective_2x = calculate_effective_leverage(base_leverage, Some(20000), None).unwrap();
        assert_eq!(effective_2x, 20);
        
        // Test with positive PnL (20% profit reduces leverage)
        // effective = 10 * (1 - 0.2) = 8x
        let effective_profit = calculate_effective_leverage(base_leverage, None, Some(2000)).unwrap();
        assert_eq!(effective_profit, 8);
        
        // Test with negative PnL (-10% loss increases leverage)
        // effective = 10 * (1 - (-0.1)) = 10 * 1.1 = 11x
        let effective_loss = calculate_effective_leverage(base_leverage, None, Some(-1000)).unwrap();
        assert_eq!(effective_loss, 11);
        
        // Test with both PnL and chain multiplier
        // effective = 10 * (1 - 0.2) * 2 = 16x
        let effective_combined = calculate_effective_leverage(base_leverage, Some(20000), Some(2000)).unwrap();
        assert_eq!(effective_combined, 16);
        
        // Test with cap at 500x
        let effective_capped = calculate_effective_leverage(100, Some(100000), None).unwrap();
        assert_eq!(effective_capped, 500); // Capped at 500x
        
        // Test extreme profit scenario (90% profit)
        // effective = 10 * (1 - 0.9) = 1x (minimum)
        let effective_extreme = calculate_effective_leverage(base_leverage, None, Some(9000)).unwrap();
        assert_eq!(effective_extreme, 1); // Minimum 1x leverage
    }
    
    #[test]
    fn test_liquidation_verification() {
        let entry_price = 5_000_000_000; // $5000
        let base_leverage = 20;
        let effective_leverage = 40; // 2x chain multiplier
        let sigma = 150;
        let num_positions = 1;
        
        let verification = verify_liquidation_calculation(
            entry_price,
            base_leverage,
            effective_leverage,
            sigma,
            num_positions,
            true,
        ).unwrap();
        
        // With 40x effective leverage, liquidation should be very close
        assert!(verification.liquidation_percentage < 500); // Less than 5% buffer
        
        // Log details for inspection
        verification.log_details();
    }
    
    #[test]
    fn test_pnl_adjusted_liquidation() {
        let entry_price = 1_000_000; // $1.00
        let base_leverage = 10;
        
        // Test 1: Position with 20% profit should have lower liquidation risk
        let profit_pnl_pct = 2000; // 20% profit
        let effective_with_profit = calculate_effective_leverage(base_leverage, None, Some(profit_pnl_pct)).unwrap();
        assert_eq!(effective_with_profit, 8); // 10 * (1 - 0.2) = 8x
        
        // Calculate liquidation price with profit
        let margin_ratio = calculate_margin_ratio_spec(base_leverage, 150, 1).unwrap();
        let liq_price_profit = calculate_liquidation_price_spec(
            entry_price,
            margin_ratio,
            effective_with_profit,
            true, // long position
        ).unwrap();
        
        // With lower effective leverage, liquidation price should be further away
        // Original: 10x leverage = 90% of entry price = $0.90
        // With profit: 8x leverage = 87.5% of entry price = $0.875
        assert!(liq_price_profit < 900_000); // Less than $0.90
        assert!(liq_price_profit > 870_000); // Greater than $0.87
        
        // Test 2: Position with 10% loss should have higher liquidation risk
        let loss_pnl_pct = -1000; // -10% loss
        let effective_with_loss = calculate_effective_leverage(base_leverage, None, Some(loss_pnl_pct)).unwrap();
        assert_eq!(effective_with_loss, 11); // 10 * (1 - (-0.1)) = 11x
        
        let liq_price_loss = calculate_liquidation_price_spec(
            entry_price,
            margin_ratio,
            effective_with_loss,
            true, // long position
        ).unwrap();
        
        // With higher effective leverage, liquidation price should be closer
        // 11x leverage = ~90.9% of entry price = $0.909
        assert!(liq_price_loss > 900_000); // Greater than $0.90
        assert!(liq_price_loss < 920_000); // Less than $0.92
        
        // Verify that profitable positions are safer than losing positions
        assert!(liq_price_profit < liq_price_loss);
    }
}