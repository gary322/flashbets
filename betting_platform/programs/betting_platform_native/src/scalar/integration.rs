//! Module Integration Layer
//!
//! Integrates scalar calculations with Oracle, CDP, Perpetual, Synthetics, and Vault modules

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::{
    oracle::OraclePDA,
    synthetics::state::SyntheticToken,
    cdp::state::CDPAccount,
    perpetual::state::PerpetualMarket,
    vault::state::Vault,
    error::BettingPlatformError,
};
use super::{
    state::{UnifiedScalar, RiskParameters, PlatformMetrics},
    calculation::{FeeCalculator, UnifiedPricer, RiskCalculator, LeverageCalculator},
};

/// Integration with Oracle module
pub struct OracleIntegration;

impl OracleIntegration {
    /// Update scalar from oracle data
    pub fn update_from_oracle(
        scalar: &mut UnifiedScalar,
        oracle: &OraclePDA,
    ) -> Result<(), ProgramError> {
        // Update base price
        scalar.oracle_price = oracle.price;
        
        // Calculate confidence from oracle accuracy
        scalar.oracle_confidence = if oracle.confidence < 100 {
            100 // Minimum 1% confidence interval
        } else {
            oracle.confidence.min(2000) // Max 20% confidence
        };
        
        // Update last update time
        scalar.last_update = Clock::get()?.unix_timestamp;
        
        msg!("Updated scalar from oracle: price={}, confidence={}", 
             scalar.oracle_price, scalar.oracle_confidence);
        
        Ok(())
    }
    
    /// Get aggregated price from multiple oracles
    pub fn aggregate_oracle_prices(
        oracles: &[OraclePDA],
    ) -> Result<(u64, u16), ProgramError> {
        if oracles.is_empty() {
            return Err(BettingPlatformError::NoOracleData.into());
        }
        
        // Calculate weighted average based on confidence
        let mut weighted_sum = 0u128;
        let mut weight_sum = 0u128;
        
        for oracle in oracles {
            let weight = 10000u128.saturating_sub(oracle.confidence as u128);
            weighted_sum += oracle.price as u128 * weight;
            weight_sum += weight;
        }
        
        if weight_sum == 0 {
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        
        let avg_price = (weighted_sum / weight_sum) as u64;
        
        // Calculate combined confidence
        let avg_confidence = oracles.iter()
            .map(|o| o.confidence as u32)
            .sum::<u32>() / oracles.len() as u32;
        
        Ok((avg_price, avg_confidence as u16))
    }
}

/// Integration with CDP module
pub struct CDPIntegration;

impl CDPIntegration {
    /// Update scalar from CDP state
    pub fn update_from_cdp(
        scalar: &mut UnifiedScalar,
        cdp: &CDPAccount,
    ) -> Result<(), ProgramError> {
        // Update collateral factor based on CDP health
        let health_ratio = if cdp.debt == 0 {
            20000 // 200% if no debt
        } else {
            ((cdp.collateral as u128 * 10000) / cdp.debt as u128)
                .min(20000) as u16
        };
        
        // Adjust collateral factor based on health
        scalar.cdp_collateral_factor = if health_ratio > 15000 {
            9000 // 90% for very healthy
        } else if health_ratio > 12000 {
            8000 // 80% for healthy
        } else if health_ratio > 11000 {
            7000 // 70% for marginal
        } else {
            6000 // 60% for risky
        };
        
        msg!("Updated CDP collateral factor: {}", scalar.cdp_collateral_factor);
        
        Ok(())
    }
    
    /// Calculate CDP borrowing power
    pub fn calculate_borrowing_power(
        scalar: &UnifiedScalar,
        collateral_amount: u64,
    ) -> Result<u64, ProgramError> {
        let collateral_value = scalar.calculate_cdp_value(collateral_amount);
        
        // Borrowing power = collateral_value * collateral_factor
        let borrowing_power = (collateral_value as u128 * scalar.cdp_collateral_factor as u128) / 10000;
        
        if borrowing_power > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(borrowing_power as u64)
    }
}

/// Integration with Perpetual module
pub struct PerpetualIntegration;

impl PerpetualIntegration {
    /// Update scalar from perpetual market
    pub fn update_from_perpetual(
        scalar: &mut UnifiedScalar,
        perp: &PerpetualMarket,
    ) -> Result<(), ProgramError> {
        // Update funding rate
        scalar.perp_funding_rate = perp.funding_rate;
        
        // Update liquidity from open interest
        scalar.liquidity_depth = scalar.liquidity_depth
            .saturating_add(perp.open_interest_long)
            .saturating_add(perp.open_interest_short);
        
        // Adjust risk based on skew
        let skew = if perp.open_interest_long > perp.open_interest_short {
            perp.open_interest_long - perp.open_interest_short
        } else {
            perp.open_interest_short - perp.open_interest_long
        };
        
        if perp.open_interest_long + perp.open_interest_short > 0 {
            let skew_ratio = (skew as u128 * 10000) / 
                (perp.open_interest_long + perp.open_interest_short) as u128;
            
            // High skew increases risk
            if skew_ratio > 3000 { // >30% skew
                scalar.risk_score = scalar.risk_score.saturating_add((skew_ratio / 100) as u16);
            }
        }
        
        msg!("Updated from perpetual: funding_rate={}, liquidity={}", 
             scalar.perp_funding_rate, scalar.liquidity_depth);
        
        Ok(())
    }
    
    /// Calculate perpetual mark price
    pub fn calculate_mark_price(
        scalar: &UnifiedScalar,
        perp: &PerpetualMarket,
    ) -> Result<u64, ProgramError> {
        let fair_value = scalar.calculate_adjusted_price()?;
        
        // Apply funding rate adjustment
        let funding_adjustment = (fair_value as u128 * perp.funding_rate.abs() as u128) / 10000;
        
        let mark_price = if perp.funding_rate > 0 {
            (fair_value as u128).saturating_add(funding_adjustment)
        } else {
            (fair_value as u128).saturating_sub(funding_adjustment)
        };
        
        if mark_price > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(mark_price as u64)
    }
}

/// Integration with Synthetics module
pub struct SyntheticsIntegration;

impl SyntheticsIntegration {
    /// Update scalar from synthetic token state
    pub fn update_from_synthetic(
        scalar: &mut UnifiedScalar,
        synthetic: &SyntheticToken,
    ) -> Result<(), ProgramError> {
        // Calculate price adjustment based on supply/demand
        let total_supply = synthetic.total_supply;
        let target_supply = 1_000_000 * 10_u128.pow(synthetic.decimals as u32); // 1M tokens target
        
        // Adjust price based on supply deviation
        let supply_ratio = if target_supply > 0 {
            (total_supply * 10000) / target_supply
        } else {
            10000
        };
        
        // Higher supply = lower price adjustment
        scalar.synthetic_adjustment = if supply_ratio > 10000 {
            -((supply_ratio - 10000).min(2000) as i16) // Max -20% adjustment
        } else {
            ((10000 - supply_ratio).min(2000) as i16) // Max +20% adjustment
        };
        
        msg!("Updated synthetic adjustment: {}", scalar.synthetic_adjustment);
        
        Ok(())
    }
    
    /// Calculate synthetic token mint price
    pub fn calculate_mint_price(
        scalar: &UnifiedScalar,
        amount: u64,
    ) -> Result<u64, ProgramError> {
        let base_price = scalar.calculate_adjusted_price()?;
        
        // Apply premium for minting (to discourage excessive minting)
        let mint_premium = 50; // 0.5% premium
        let price_with_premium = (base_price as u128 * (10000 + mint_premium)) / 10000;
        
        let total_cost = (price_with_premium * amount as u128) / 10_u128.pow(8);
        
        if total_cost > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(total_cost as u64)
    }
}

/// Integration with Vault module
pub struct VaultIntegration;

impl VaultIntegration {
    /// Update scalar from vault state
    pub fn update_from_vault(
        scalar: &mut UnifiedScalar,
        vault: &Vault,
    ) -> Result<(), ProgramError> {
        // Calculate yield impact based on vault performance
        let performance = if vault.total_deposits > 0 {
            ((vault.total_value_locked as i128 - vault.total_deposits as i128) * 10000) / 
            vault.total_deposits as i128
        } else {
            0
        };
        
        // Positive performance slightly increases prices (wealth effect)
        // Negative performance slightly decreases prices (risk aversion)
        scalar.vault_yield_impact = (performance / 10).max(-500).min(500) as i16;
        
        // Add vault TVL to platform liquidity
        scalar.liquidity_depth = scalar.liquidity_depth
            .saturating_add(vault.total_value_locked);
        
        msg!("Updated vault yield impact: {}", scalar.vault_yield_impact);
        
        Ok(())
    }
    
    /// Calculate vault share price with scalar adjustments
    pub fn calculate_share_price(
        scalar: &UnifiedScalar,
        vault: &Vault,
    ) -> Result<u64, ProgramError> {
        let base_share_price = vault.share_price;
        
        // Apply scalar adjustments
        let risk_adjustment = (10000 - scalar.risk_score / 2) as u128; // Lower risk = higher price
        let adjusted_price = (base_share_price as u128 * risk_adjustment) / 10000;
        
        if adjusted_price > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(adjusted_price as u64)
    }
}

/// Main integration coordinator
pub struct ScalarIntegrator;

impl ScalarIntegrator {
    /// Update scalar from all modules
    pub fn update_scalar_from_all(
        scalar: &mut UnifiedScalar,
        oracle: Option<&OraclePDA>,
        cdp: Option<&CDPAccount>,
        perp: Option<&PerpetualMarket>,
        synthetic: Option<&SyntheticToken>,
        vault: Option<&Vault>,
    ) -> Result<(), ProgramError> {
        // Update from each module if data is available
        if let Some(oracle_data) = oracle {
            OracleIntegration::update_from_oracle(scalar, oracle_data)?;
        }
        
        if let Some(cdp_data) = cdp {
            CDPIntegration::update_from_cdp(scalar, cdp_data)?;
        }
        
        if let Some(perp_data) = perp {
            PerpetualIntegration::update_from_perpetual(scalar, perp_data)?;
        }
        
        if let Some(synthetic_data) = synthetic {
            SyntheticsIntegration::update_from_synthetic(scalar, synthetic_data)?;
        }
        
        if let Some(vault_data) = vault {
            VaultIntegration::update_from_vault(scalar, vault_data)?;
        }
        
        // Recalculate overall risk score
        scalar.risk_score = Self::calculate_composite_risk(scalar);
        
        // Check if should halt
        if scalar.should_halt() {
            scalar.is_halted = true;
            msg!("Market halted due to risk conditions");
        }
        
        Ok(())
    }
    
    /// Calculate composite risk score
    fn calculate_composite_risk(scalar: &UnifiedScalar) -> u16 {
        let mut risk = 5000u32; // Base risk
        
        // Add volatility risk
        risk += scalar.volatility as u32;
        
        // Add confidence risk
        risk += scalar.oracle_confidence as u32;
        
        // Add liquidity risk
        if scalar.liquidity_depth < 100_000 * 10_u128.pow(6) {
            risk += 2000; // Low liquidity penalty
        }
        
        // Add funding rate risk
        risk += scalar.perp_funding_rate.abs() as u32;
        
        (risk.min(10000) as u16)
    }
    
    /// Update platform-wide metrics
    pub fn update_platform_metrics(
        metrics: &mut PlatformMetrics,
        scalars: &[UnifiedScalar],
        vaults: &[Vault],
        cdps: &[CDPAccount],
        perps: &[PerpetualMarket],
    ) -> Result<(), ProgramError> {
        // Reset metrics
        metrics.total_tvl = 0;
        metrics.total_cdp_collateral = 0;
        metrics.total_perp_open_interest = 0;
        metrics.total_volume_24h = 0;
        metrics.active_markets = 0;
        
        // Aggregate from vaults
        for vault in vaults {
            metrics.total_tvl += vault.total_value_locked;
        }
        
        // Aggregate from CDPs
        for cdp in cdps {
            metrics.total_cdp_collateral += cdp.collateral as u128;
        }
        
        // Aggregate from perpetuals
        for perp in perps {
            metrics.total_perp_open_interest += perp.open_interest_long;
            metrics.total_perp_open_interest += perp.open_interest_short;
        }
        
        // Aggregate from scalars
        for scalar in scalars {
            if !scalar.is_halted {
                metrics.active_markets += 1;
            }
            metrics.total_volume_24h += scalar.volume_24h;
        }
        
        // Calculate platform risk
        let avg_risk: u32 = scalars.iter()
            .map(|s| s.risk_score as u32)
            .sum::<u32>() / scalars.len().max(1) as u32;
        metrics.platform_risk_score = avg_risk as u16;
        
        metrics.last_update = Clock::get()?.unix_timestamp;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_oracle_integration() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        let oracle = OraclePDA {
            price: 150_000_000,
            confidence: 150,
            last_update: 0,
            source: 0,
            ..Default::default()
        };
        
        OracleIntegration::update_from_oracle(&mut scalar, &oracle).unwrap();
        assert_eq!(scalar.oracle_price, 150_000_000);
        assert_eq!(scalar.oracle_confidence, 150);
    }
    
    #[test]
    fn test_cdp_integration() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        let cdp = CDPAccount {
            collateral: 150_000,
            debt: 100_000,
            ..Default::default()
        };
        
        CDPIntegration::update_from_cdp(&mut scalar, &cdp).unwrap();
        assert_eq!(scalar.cdp_collateral_factor, 8000); // Healthy CDP
    }
    
    #[test]
    fn test_composite_risk_calculation() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        scalar.volatility = 2000;
        scalar.oracle_confidence = 500;
        scalar.liquidity_depth = 50_000 * 10_u128.pow(6);
        scalar.perp_funding_rate = -100;
        
        let risk = ScalarIntegrator::calculate_composite_risk(&scalar);
        assert!(risk > 5000 && risk < 10000);
    }
}