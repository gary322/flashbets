//! Unified Risk Model
//!
//! Comprehensive risk assessment and management

use solana_program::{
    program_error::ProgramError,
    msg,
};
use crate::{
    error::BettingPlatformError,
    math::U64F64,
};
use super::state::{UnifiedScalar, RiskParameters};

/// Risk categories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskCategory {
    Minimal,    // 0-2000
    Low,        // 2000-4000
    Medium,     // 4000-6000
    High,       // 6000-8000
    Critical,   // 8000-10000
}

impl RiskCategory {
    pub fn from_score(score: u16) -> Self {
        match score {
            0..=2000 => RiskCategory::Minimal,
            2001..=4000 => RiskCategory::Low,
            4001..=6000 => RiskCategory::Medium,
            6001..=8000 => RiskCategory::High,
            _ => RiskCategory::Critical,
        }
    }
    
    pub fn get_margin_multiplier(&self) -> u16 {
        match self {
            RiskCategory::Minimal => 10000,  // 1x (no extra margin)
            RiskCategory::Low => 11000,      // 1.1x
            RiskCategory::Medium => 12500,   // 1.25x
            RiskCategory::High => 15000,     // 1.5x
            RiskCategory::Critical => 20000, // 2x
        }
    }
    
    pub fn get_max_leverage_multiplier(&self) -> u16 {
        match self {
            RiskCategory::Minimal => 10000,  // 100% of max
            RiskCategory::Low => 8000,       // 80% of max
            RiskCategory::Medium => 6000,    // 60% of max
            RiskCategory::High => 4000,      // 40% of max
            RiskCategory::Critical => 2000,  // 20% of max
        }
    }
}

/// Value at Risk (VaR) calculator
pub struct VaRCalculator;

impl VaRCalculator {
    /// Calculate Value at Risk for a position
    pub fn calculate_var(
        position_value: u64,
        volatility_bps: u16,
        confidence_level: f64,
        time_horizon_days: u32,
    ) -> Result<u64, ProgramError> {
        // Z-scores for common confidence levels
        let z_score = match (confidence_level * 100.0) as u32 {
            99 => 2.33,
            95 => 1.65,
            90 => 1.28,
            _ => 1.65, // Default to 95%
        };
        
        // VaR = position_value * volatility * z_score * sqrt(time)
        let daily_vol = volatility_bps as f64 / 10000.0;
        let time_factor = (time_horizon_days as f64).sqrt();
        
        let var_percentage = daily_vol * z_score * time_factor;
        let var_amount = (position_value as f64 * var_percentage) as u64;
        
        Ok(var_amount)
    }
    
    /// Calculate Conditional VaR (CVaR/Expected Shortfall)
    pub fn calculate_cvar(
        position_value: u64,
        volatility_bps: u16,
        confidence_level: f64,
    ) -> Result<u64, ProgramError> {
        // CVaR is approximately 1.25x VaR for normal distribution
        let var = Self::calculate_var(position_value, volatility_bps, confidence_level, 1)?;
        let cvar = (var as u128 * 125) / 100;
        
        if cvar > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }
        
        Ok(cvar as u64)
    }
}

/// Stress testing framework
pub struct StressTester;

impl StressTester {
    /// Run stress test on a position
    pub fn stress_test_position(
        scalar: &UnifiedScalar,
        position_value: u64,
        leverage: u64,
        scenarios: &[StressScenario],
    ) -> Result<StressTestResult, ProgramError> {
        let mut worst_loss = 0u64;
        let mut worst_scenario = StressScenario::default();
        let mut failures = 0u32;
        
        for scenario in scenarios {
            let loss = Self::calculate_scenario_loss(
                scalar,
                position_value,
                leverage,
                scenario,
            )?;
            
            if loss > worst_loss {
                worst_loss = loss;
                worst_scenario = scenario.clone();
            }
            
            // Check if position would be liquidated
            if loss > position_value / (leverage / 10000) {
                failures += 1;
            }
        }
        
        Ok(StressTestResult {
            worst_loss,
            worst_scenario,
            failure_rate: (failures * 100) / scenarios.len() as u32,
            tested_scenarios: scenarios.len() as u32,
        })
    }
    
    /// Calculate loss for a specific scenario
    fn calculate_scenario_loss(
        scalar: &UnifiedScalar,
        position_value: u64,
        leverage: u64,
        scenario: &StressScenario,
    ) -> Result<u64, ProgramError> {
        // Apply price shock
        let price_impact = (position_value as u128 * scenario.price_shock_bps as u128) / 10000;
        
        // Apply volatility spike impact
        let vol_impact = (position_value as u128 * scenario.volatility_spike_bps as u128) / 10000;
        
        // Apply liquidity crunch impact
        let liquidity_impact = if scenario.liquidity_drop_pct > 50 {
            (position_value as u128 * 1000) / 10000 // 10% additional loss
        } else {
            0
        };
        
        // Total loss amplified by leverage
        let base_loss = price_impact + vol_impact + liquidity_impact;
        let leveraged_loss = (base_loss * leverage as u128) / 10000;
        
        if leveraged_loss > u64::MAX as u128 {
            return Ok(u64::MAX);
        }
        
        Ok(leveraged_loss as u64)
    }
}

/// Stress test scenario
#[derive(Clone, Debug, Default)]
pub struct StressScenario {
    pub name: String,
    pub price_shock_bps: u16,      // Price movement in basis points
    pub volatility_spike_bps: u16, // Volatility increase
    pub liquidity_drop_pct: u8,    // Liquidity reduction percentage
    pub correlation_break: bool,   // Correlation breakdown
}

impl StressScenario {
    /// Create standard stress scenarios
    pub fn standard_scenarios() -> Vec<Self> {
        vec![
            StressScenario {
                name: "Market Crash".to_string(),
                price_shock_bps: 2000,     // 20% drop
                volatility_spike_bps: 5000, // 50% vol spike
                liquidity_drop_pct: 70,
                correlation_break: true,
            },
            StressScenario {
                name: "Flash Crash".to_string(),
                price_shock_bps: 1000,     // 10% drop
                volatility_spike_bps: 10000, // 100% vol spike
                liquidity_drop_pct: 90,
                correlation_break: true,
            },
            StressScenario {
                name: "Liquidity Crisis".to_string(),
                price_shock_bps: 500,      // 5% drop
                volatility_spike_bps: 2000, // 20% vol spike
                liquidity_drop_pct: 80,
                correlation_break: false,
            },
            StressScenario {
                name: "Black Swan".to_string(),
                price_shock_bps: 5000,     // 50% drop
                volatility_spike_bps: 20000, // 200% vol spike
                liquidity_drop_pct: 95,
                correlation_break: true,
            },
        ]
    }
}

/// Stress test result
#[derive(Debug)]
pub struct StressTestResult {
    pub worst_loss: u64,
    pub worst_scenario: StressScenario,
    pub failure_rate: u32, // Percentage of scenarios causing liquidation
    pub tested_scenarios: u32,
}

/// Portfolio risk analyzer
pub struct PortfolioRiskAnalyzer;

impl PortfolioRiskAnalyzer {
    /// Calculate portfolio beta
    pub fn calculate_portfolio_beta(
        position_values: &[u64],
        position_betas: &[f64],
    ) -> Result<f64, ProgramError> {
        if position_values.len() != position_betas.len() {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        let total_value: u64 = position_values.iter().sum();
        if total_value == 0 {
            return Ok(0.0);
        }
        
        let mut weighted_beta = 0.0;
        for (value, beta) in position_values.iter().zip(position_betas.iter()) {
            let weight = *value as f64 / total_value as f64;
            weighted_beta += weight * beta;
        }
        
        Ok(weighted_beta)
    }
    
    /// Calculate Sharpe ratio
    pub fn calculate_sharpe_ratio(
        returns: &[f64],
        risk_free_rate: f64,
    ) -> Result<f64, ProgramError> {
        if returns.is_empty() {
            return Ok(0.0);
        }
        
        // Calculate average return
        let avg_return: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        
        // Calculate standard deviation
        let variance: f64 = returns.iter()
            .map(|r| (r - avg_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev == 0.0 {
            return Ok(0.0);
        }
        
        // Sharpe ratio = (return - risk_free_rate) / std_dev
        Ok((avg_return - risk_free_rate) / std_dev)
    }
    
    /// Calculate maximum drawdown
    pub fn calculate_max_drawdown(
        values: &[u64],
    ) -> Result<u16, ProgramError> {
        if values.is_empty() {
            return Ok(0);
        }
        
        let mut max_value = values[0];
        let mut max_drawdown_bps = 0u16;
        
        for &value in values {
            if value > max_value {
                max_value = value;
            } else {
                let drawdown = ((max_value - value) as u128 * 10000) / max_value as u128;
                if drawdown > max_drawdown_bps as u128 {
                    max_drawdown_bps = drawdown.min(10000) as u16;
                }
            }
        }
        
        Ok(max_drawdown_bps)
    }
}

/// Dynamic risk adjustment
pub struct DynamicRiskAdjuster;

impl DynamicRiskAdjuster {
    /// Adjust risk parameters based on market conditions
    pub fn adjust_risk_parameters(
        params: &mut RiskParameters,
        scalar: &UnifiedScalar,
    ) -> Result<(), ProgramError> {
        let risk_category = RiskCategory::from_score(scalar.risk_score);
        
        // Adjust max leverage based on risk
        let leverage_multiplier = risk_category.get_max_leverage_multiplier();
        let base_max_leverage = 1000000u64; // 100x base
        params.max_leverage = ((base_max_leverage * leverage_multiplier as u64) / 10000) as u16;
        
        // Adjust collateral requirements
        let margin_multiplier = risk_category.get_margin_multiplier();
        params.min_collateral_ratio = ((11000u32 * margin_multiplier as u32) / 10000) as u16;
        
        // Adjust liquidation threshold
        params.liquidation_threshold = params.min_collateral_ratio.saturating_sub(500);
        
        // Adjust circuit breaker based on volatility
        if scalar.volatility > 3000 { // >30% volatility
            params.circuit_breaker_threshold = 1000; // Tighten to 10%
            params.circuit_breaker_cooldown = 600; // 10 minutes
        } else if scalar.volatility > 2000 { // >20% volatility
            params.circuit_breaker_threshold = 1500; // 15%
            params.circuit_breaker_cooldown = 300; // 5 minutes
        } else {
            params.circuit_breaker_threshold = 2000; // 20%
            params.circuit_breaker_cooldown = 180; // 3 minutes
        }
        
        msg!("Adjusted risk parameters: max_leverage={}, min_collateral={}", 
             params.max_leverage, params.min_collateral_ratio);
        
        Ok(())
    }
    
    /// Calculate dynamic position limit
    pub fn calculate_position_limit(
        scalar: &UnifiedScalar,
        user_history_score: u16, // 0-10000, higher is better
    ) -> Result<u64, ProgramError> {
        // Base limit from liquidity
        let base_limit = scalar.liquidity_depth / 10; // 10% of liquidity
        
        // Adjust for risk
        let risk_multiplier = match RiskCategory::from_score(scalar.risk_score) {
            RiskCategory::Minimal => 10000,
            RiskCategory::Low => 8000,
            RiskCategory::Medium => 6000,
            RiskCategory::High => 4000,
            RiskCategory::Critical => 2000,
        };
        
        // Adjust for user history
        let history_multiplier = 5000 + (user_history_score / 2); // 50-100% based on history
        
        let adjusted_limit = (base_limit as u128 * risk_multiplier as u128 * history_multiplier as u128) 
            / (10000 * 10000);
        
        if adjusted_limit > u64::MAX as u128 {
            return Ok(u64::MAX);
        }
        
        Ok(adjusted_limit as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;
    
    #[test]
    fn test_risk_categories() {
        assert_eq!(RiskCategory::from_score(1000), RiskCategory::Minimal);
        assert_eq!(RiskCategory::from_score(3000), RiskCategory::Low);
        assert_eq!(RiskCategory::from_score(5000), RiskCategory::Medium);
        assert_eq!(RiskCategory::from_score(7000), RiskCategory::High);
        assert_eq!(RiskCategory::from_score(9000), RiskCategory::Critical);
        
        assert_eq!(RiskCategory::Medium.get_margin_multiplier(), 12500);
        assert_eq!(RiskCategory::High.get_max_leverage_multiplier(), 4000);
    }
    
    #[test]
    fn test_var_calculation() {
        let position_value = 100_000 * 10_u64.pow(6);
        let volatility = 2000; // 20%
        let var = VaRCalculator::calculate_var(position_value, volatility, 0.95, 1).unwrap();
        
        // VaR should be approximately 3.3% of position value (20% * 1.65)
        assert!(var > position_value * 3 / 100 && var < position_value * 4 / 100);
        
        let cvar = VaRCalculator::calculate_cvar(position_value, volatility, 0.95).unwrap();
        assert!(cvar > var); // CVaR should be higher than VaR
    }
    
    #[test]
    fn test_stress_testing() {
        let scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        let scenarios = StressScenario::standard_scenarios();
        
        let result = StressTester::stress_test_position(
            &scalar,
            100_000 * 10_u64.pow(6),
            100_000, // 10x leverage
            &scenarios,
        ).unwrap();
        
        assert!(result.worst_loss > 0);
        assert_eq!(result.tested_scenarios, scenarios.len() as u32);
    }
    
    #[test]
    fn test_portfolio_metrics() {
        let position_values = vec![100_000, 200_000, 150_000];
        let position_betas = vec![1.2, 0.8, 1.5];
        
        let portfolio_beta = PortfolioRiskAnalyzer::calculate_portfolio_beta(
            &position_values,
            &position_betas,
        ).unwrap();
        
        assert!(portfolio_beta > 0.0 && portfolio_beta < 2.0);
        
        let returns = vec![0.05, -0.02, 0.08, 0.03, -0.01];
        let sharpe = PortfolioRiskAnalyzer::calculate_sharpe_ratio(&returns, 0.02).unwrap();
        assert!(sharpe > -1.0 && sharpe < 3.0); // Reasonable Sharpe ratio range
        
        let values = vec![100, 110, 105, 95, 100, 120, 115];
        let max_dd = PortfolioRiskAnalyzer::calculate_max_drawdown(&values).unwrap();
        assert!(max_dd > 0 && max_dd < 2000); // Should detect the drawdown from 110 to 95
    }
    
    #[test]
    fn test_dynamic_risk_adjustment() {
        let mut params = RiskParameters::default(Pubkey::new_unique());
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        scalar.risk_score = 7500; // High risk
        scalar.volatility = 3500; // High volatility
        
        DynamicRiskAdjuster::adjust_risk_parameters(&mut params, &scalar).unwrap();
        
        assert!(params.max_leverage < 1000000); // Should reduce max leverage
        assert!(params.min_collateral_ratio > 11000); // Should increase collateral requirement
        assert_eq!(params.circuit_breaker_threshold, 1000); // Should tighten circuit breaker
    }
}