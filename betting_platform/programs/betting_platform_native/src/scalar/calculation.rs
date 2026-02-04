//! Unified Calculation Engine
//!
//! Core calculations for pricing, fees, and risk assessment

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::{
    error::BettingPlatformError,
    math::{U64F64, U128F128},
};
use super::state::{UnifiedScalar, RiskParameters};

/// Fee calculation based on risk and volume
pub struct FeeCalculator;

impl FeeCalculator {
    /// Calculate dynamic trading fee based on risk and volume
    pub fn calculate_trading_fee(
        scalar: &UnifiedScalar,
        base_fee_bps: u16,
        volume: u64,
        is_maker: bool,
    ) -> Result<u64, ProgramError> {
        // Start with base fee
        let mut fee_bps = base_fee_bps as u32;
        
        // Maker discount
        if is_maker {
            fee_bps = fee_bps.saturating_sub(fee_bps / 4); // 25% discount for makers
        }
        
        // Risk adjustment (higher risk = higher fees)
        let risk_multiplier = 10000 + (scalar.risk_score as u32 / 2); // 0.5x risk score
        fee_bps = (fee_bps * risk_multiplier) / 10000;
        
        // Volume discount (higher volume = lower fees)
        let volume_tier = match scalar.volume_24h {
            v if v > 10_000_000 * 10_u128.pow(6) => 8000, // 20% discount
            v if v > 1_000_000 * 10_u128.pow(6) => 9000,  // 10% discount
            v if v > 100_000 * 10_u128.pow(6) => 9500,    // 5% discount
            _ => 10000, // No discount
        };
        fee_bps = (fee_bps * volume_tier) / 10000;
        
        // Volatility adjustment
        if scalar.volatility > 2000 { // >20% volatility
            let vol_multiplier = 10000 + ((scalar.volatility - 2000) / 10) as u32;
            fee_bps = (fee_bps * vol_multiplier) / 10000;
        }
        
        // Calculate final fee
        let fee = (volume as u128 * fee_bps as u128) / 10000;
        
        if fee > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(fee as u64)
    }
    
    /// Calculate liquidation fee
    pub fn calculate_liquidation_fee(
        position_value: u64,
        is_partial: bool,
    ) -> Result<u64, ProgramError> {
        let base_fee_bps = if is_partial { 50 } else { 100 }; // 0.5% or 1%
        let fee = (position_value as u128 * base_fee_bps) / 10000;
        
        if fee > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(fee as u64)
    }
}

/// Slippage and price impact calculations
pub struct SlippageCalculator;

impl SlippageCalculator {
    /// Calculate price impact for a trade
    pub fn calculate_price_impact(
        scalar: &UnifiedScalar,
        trade_size: u64,
    ) -> Result<u16, ProgramError> {
        if scalar.liquidity_depth == 0 {
            return Ok(10000); // 100% impact if no liquidity
        }
        
        // Impact = trade_size / liquidity * volatility_factor
        let size_ratio = (trade_size as u128 * 10000) / scalar.liquidity_depth;
        let volatility_factor = 10000 + scalar.volatility as u128;
        
        let impact = (size_ratio * volatility_factor) / 10000;
        
        if impact > u16::MAX as u128 {
            return Ok(u16::MAX); // Cap at max impact
        }
        
        Ok(impact as u16)
    }
    
    /// Calculate expected slippage
    pub fn calculate_slippage(
        scalar: &UnifiedScalar,
        trade_size: u64,
        is_buy: bool,
    ) -> Result<u64, ProgramError> {
        let price_impact = Self::calculate_price_impact(scalar, trade_size)?;
        let adjusted_price = scalar.calculate_adjusted_price()?;
        
        // Slippage = price * impact
        let slippage = (adjusted_price as u128 * price_impact as u128) / 10000;
        
        // Apply directional adjustment
        let final_price = if is_buy {
            (adjusted_price as u128).saturating_add(slippage)
        } else {
            (adjusted_price as u128).saturating_sub(slippage)
        };
        
        if final_price > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(final_price as u64)
    }
}

/// Leverage calculations
pub struct LeverageCalculator;

impl LeverageCalculator {
    /// Calculate maximum safe leverage based on volatility and liquidity
    pub fn calculate_max_safe_leverage(
        scalar: &UnifiedScalar,
        risk_params: &RiskParameters,
    ) -> u64 {
        // Base max leverage
        let mut max_leverage = risk_params.max_leverage as u64;
        
        // Reduce for high volatility
        if scalar.volatility > 1000 { // >10% volatility
            let reduction_factor = 10000_u64.saturating_sub(scalar.volatility as u64);
            max_leverage = (max_leverage * reduction_factor) / 10000;
        }
        
        // Reduce for low liquidity
        if scalar.liquidity_depth < 100_000 * 10_u128.pow(6) { // <$100k liquidity
            max_leverage = max_leverage / 2; // Halve max leverage
        }
        
        // Reduce for high risk score
        if scalar.risk_score > 7500 {
            let risk_reduction = 10000_u64.saturating_sub(scalar.risk_score as u64 / 2);
            max_leverage = (max_leverage * risk_reduction) / 10000;
        }
        
        max_leverage.max(10000) // Minimum 1x
    }
    
    /// Calculate required margin for a leveraged position
    pub fn calculate_required_margin(
        position_size: u64,
        leverage: u64,
        risk_params: &RiskParameters,
    ) -> Result<u64, ProgramError> {
        if leverage == 0 {
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        
        // Base margin = size / leverage
        let base_margin = (position_size as u128 * 10000) / leverage as u128;
        
        // Apply minimum collateral ratio
        let required_margin = (base_margin * risk_params.min_collateral_ratio as u128) / 10000;
        
        if required_margin > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(required_margin as u64)
    }
}

/// Yield and APY calculations
pub struct YieldCalculator;

impl YieldCalculator {
    /// Calculate compounded yield
    pub fn calculate_compound_yield(
        principal: u64,
        apy_bps: u16,
        days: u32,
    ) -> Result<u64, ProgramError> {
        // Convert APY to daily rate
        let daily_rate = U64F64::from_num(apy_bps) / U64F64::from_num(365 * 10000);
        
        // Compound formula: P * (1 + r)^n
        let one = U64F64::from_num(1);
        let compound_factor = (one + daily_rate).pow(days);
        
        let final_amount = U64F64::from_num(principal) * compound_factor;
        
        Ok(final_amount.to_num::<u64>())
    }
    
    /// Calculate vault performance fee
    pub fn calculate_performance_fee(
        profit: u64,
        performance_fee_bps: u16,
    ) -> Result<u64, ProgramError> {
        let fee = (profit as u128 * performance_fee_bps as u128) / 10000;
        
        if fee > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(fee as u64)
    }
}

/// Unified pricing model
pub struct UnifiedPricer;

impl UnifiedPricer {
    /// Calculate fair value price incorporating all factors
    pub fn calculate_fair_value(
        scalar: &UnifiedScalar,
        include_confidence: bool,
    ) -> Result<(u64, u64, u64), ProgramError> {
        // Get base adjusted price
        let adjusted_price = scalar.calculate_adjusted_price()?;
        
        // Calculate confidence bounds if requested
        let (lower_bound, upper_bound) = if include_confidence {
            let confidence_adjustment = (adjusted_price as u128 * scalar.oracle_confidence as u128) / 10000;
            
            let lower = (adjusted_price as u128).saturating_sub(confidence_adjustment);
            let upper = (adjusted_price as u128).saturating_add(confidence_adjustment);
            
            (lower.min(u64::MAX as u128) as u64, upper.min(u64::MAX as u128) as u64)
        } else {
            (adjusted_price, adjusted_price)
        };
        
        Ok((lower_bound, adjusted_price, upper_bound))
    }
    
    /// Calculate execution price with slippage
    pub fn calculate_execution_price(
        scalar: &UnifiedScalar,
        trade_size: u64,
        is_buy: bool,
    ) -> Result<u64, ProgramError> {
        SlippageCalculator::calculate_slippage(scalar, trade_size, is_buy)
    }
    
    /// Calculate price for CDP collateral
    pub fn calculate_cdp_price(
        scalar: &UnifiedScalar,
        amount: u64,
    ) -> Result<u64, ProgramError> {
        Ok(scalar.calculate_cdp_value(amount))
    }
}

/// Risk-based calculations
pub struct RiskCalculator;

impl RiskCalculator {
    /// Calculate position risk score
    pub fn calculate_position_risk(
        scalar: &UnifiedScalar,
        leverage: u64,
        position_size: u64,
    ) -> u16 {
        // Base risk from market
        let mut risk_score = scalar.risk_score;
        
        // Add leverage risk (higher leverage = higher risk)
        let leverage_risk = (leverage.saturating_sub(10000) / 100).min(2500) as u16;
        risk_score = risk_score.saturating_add(leverage_risk);
        
        // Add size risk (larger positions relative to liquidity = higher risk)
        if scalar.liquidity_depth > 0 {
            let size_ratio = (position_size as u128 * 10000) / scalar.liquidity_depth;
            let size_risk = (size_ratio as u16).min(2500);
            risk_score = risk_score.saturating_add(size_risk);
        }
        
        risk_score.min(10000) // Cap at max risk
    }
    
    /// Calculate liquidation price
    pub fn calculate_liquidation_price(
        entry_price: u64,
        leverage: u64,
        is_long: bool,
        risk_params: &RiskParameters,
    ) -> Result<u64, ProgramError> {
        // Liquidation distance = 1 / leverage * liquidation_threshold
        let distance_bps = (10000 * 10000) / leverage;
        let adjusted_distance = (distance_bps * risk_params.liquidation_threshold as u64) / 10000;
        
        let liquidation_price = if is_long {
            // Long liquidates when price falls
            let decrease = (entry_price as u128 * adjusted_distance as u128) / 10000;
            (entry_price as u128).saturating_sub(decrease)
        } else {
            // Short liquidates when price rises
            let increase = (entry_price as u128 * adjusted_distance as u128) / 10000;
            (entry_price as u128).saturating_add(increase)
        };
        
        if liquidation_price > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(liquidation_price as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;
    
    #[test]
    fn test_fee_calculation() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        scalar.risk_score = 6000;
        scalar.volume_24h = 2_000_000 * 10_u128.pow(6);
        scalar.volatility = 1500;
        
        let fee = FeeCalculator::calculate_trading_fee(&scalar, 30, 100_000, false).unwrap();
        assert!(fee > 0 && fee < 1000); // Reasonable fee range
        
        let maker_fee = FeeCalculator::calculate_trading_fee(&scalar, 30, 100_000, true).unwrap();
        assert!(maker_fee < fee); // Maker fee should be lower
    }
    
    #[test]
    fn test_slippage_calculation() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        scalar.oracle_price = 100_000_000; // $100
        scalar.liquidity_depth = 1_000_000 * 10_u128.pow(6); // $1M liquidity
        scalar.volatility = 500; // 5%
        
        let impact = SlippageCalculator::calculate_price_impact(&scalar, 50_000 * 10_u64.pow(6)).unwrap();
        assert!(impact > 0 && impact < 1000); // <10% impact for 5% of liquidity
        
        let buy_price = SlippageCalculator::calculate_slippage(&scalar, 50_000 * 10_u64.pow(6), true).unwrap();
        assert!(buy_price > scalar.oracle_price); // Buy price should be higher
        
        let sell_price = SlippageCalculator::calculate_slippage(&scalar, 50_000 * 10_u64.pow(6), false).unwrap();
        assert!(sell_price < scalar.oracle_price); // Sell price should be lower
    }
    
    #[test]
    fn test_leverage_calculations() {
        let scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        let risk_params = RiskParameters::default(Pubkey::new_unique());
        
        let max_leverage = LeverageCalculator::calculate_max_safe_leverage(&scalar, &risk_params);
        assert!(max_leverage >= 10000 && max_leverage <= 1_000_000);
        
        let margin = LeverageCalculator::calculate_required_margin(
            100_000 * 10_u64.pow(6),
            100_000, // 10x leverage
            &risk_params
        ).unwrap();
        assert!(margin >= 10_000 * 10_u64.pow(6)); // At least 10% margin
    }
    
    #[test]
    fn test_risk_calculation() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        scalar.risk_score = 5000;
        scalar.liquidity_depth = 1_000_000 * 10_u128.pow(6);
        
        let position_risk = RiskCalculator::calculate_position_risk(
            &scalar,
            200_000, // 20x leverage
            100_000 * 10_u64.pow(6), // $100k position
        );
        assert!(position_risk > 5000); // Higher than base risk due to leverage
        
        let risk_params = RiskParameters::default(Pubkey::new_unique());
        let liq_price = RiskCalculator::calculate_liquidation_price(
            100_000_000, // $100 entry
            100_000, // 10x leverage
            true,
            &risk_params,
        ).unwrap();
        assert!(liq_price < 100_000_000); // Long liquidates below entry
    }
}