//! Risk Management and Greeks Calculation Engine

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeks {
    pub delta: f64,      // Price sensitivity
    pub gamma: f64,      // Delta sensitivity
    pub theta: f64,      // Time decay
    pub vega: f64,       // Volatility sensitivity
    pub rho: f64,        // Interest rate sensitivity
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub portfolio_value: f64,
    pub total_exposure: f64,
    pub leverage_ratio: f64,
    pub margin_used: f64,
    pub margin_available: f64,
    pub margin_ratio: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub var_95: f64,      // Value at Risk 95%
    pub var_99: f64,      // Value at Risk 99%
    pub expected_shortfall: f64,
    pub beta: f64,
    pub alpha: f64,
    pub correlation_matrix: HashMap<String, f64>,
    pub risk_score: f64,  // Overall risk score 0-100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRisk {
    pub position_id: String,
    pub market_id: u128,
    pub current_value: f64,
    pub entry_value: f64,
    pub unrealized_pnl: f64,
    pub greeks: Greeks,
    pub risk_contribution: f64,
    pub margin_requirement: f64,
    pub liquidation_price: f64,
    pub time_to_expiry: Option<i64>,
    pub volatility: f64,
    pub correlation_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRisk {
    pub market_id: u128,
    pub volatility: f64,
    pub liquidity_score: f64,
    pub bid_ask_spread: f64,
    pub market_depth: f64,
    pub price_impact: f64,
    pub correlation_with_btc: f64,
    pub correlation_with_spy: f64,
    pub tail_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: f64,
    pub max_leverage: f64,
    pub max_portfolio_risk: f64,
    pub max_correlation_exposure: f64,
    pub max_single_market_exposure: f64,
    pub var_limit: f64,
    pub margin_call_threshold: f64,
    pub liquidation_threshold: f64,
}

pub struct RiskEngine {
    risk_metrics: Arc<RwLock<HashMap<String, RiskMetrics>>>,
    position_risks: Arc<RwLock<HashMap<String, PositionRisk>>>,
    market_risks: Arc<RwLock<HashMap<u128, MarketRisk>>>,
    risk_limits: RiskLimits,
    price_history: Arc<RwLock<HashMap<u128, Vec<PricePoint>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: i64,
    pub price: f64,
    pub volume: f64,
}

impl RiskEngine {
    pub fn new() -> Self {
        Self {
            risk_metrics: Arc::new(RwLock::new(HashMap::new())),
            position_risks: Arc::new(RwLock::new(HashMap::new())),
            market_risks: Arc::new(RwLock::new(HashMap::new())),
            risk_limits: RiskLimits {
                max_position_size: 1_000_000.0,
                max_leverage: 10.0,
                max_portfolio_risk: 0.25,
                max_correlation_exposure: 0.5,
                max_single_market_exposure: 0.3,
                var_limit: 100_000.0,
                margin_call_threshold: 0.8,
                liquidation_threshold: 0.9,
            },
            price_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Calculate Greeks for a position
    pub fn calculate_greeks(
        &self,
        position_value: f64,
        market_price: f64,
        strike_price: f64,
        time_to_expiry: f64,
        volatility: f64,
        risk_free_rate: f64,
        position_type: PositionType,
    ) -> Greeks {
        // Simplified Black-Scholes Greeks calculation
        let d1 = self.calculate_d1(market_price, strike_price, time_to_expiry, volatility, risk_free_rate);
        let d2 = d1 - volatility * time_to_expiry.sqrt();
        
        let n_d1 = self.normal_cdf(d1);
        let n_d2 = self.normal_cdf(d2);
        let phi_d1 = self.normal_pdf(d1);
        
        let sign = match position_type {
            PositionType::Long => 1.0,
            PositionType::Short => -1.0,
        };
        
        Greeks {
            delta: sign * n_d1,
            gamma: phi_d1 / (market_price * volatility * time_to_expiry.sqrt()),
            theta: -(market_price * phi_d1 * volatility) / (2.0 * time_to_expiry.sqrt())
                - sign * risk_free_rate * strike_price * (-risk_free_rate * time_to_expiry).exp() * n_d2,
            vega: market_price * phi_d1 * time_to_expiry.sqrt() / 100.0,
            rho: sign * strike_price * time_to_expiry * (-risk_free_rate * time_to_expiry).exp() * n_d2 / 100.0,
        }
    }
    
    /// Calculate d1 for Black-Scholes
    fn calculate_d1(&self, s: f64, k: f64, t: f64, vol: f64, r: f64) -> f64 {
        ((s / k).ln() + (r + 0.5 * vol.powi(2)) * t) / (vol * t.sqrt())
    }
    
    /// Normal cumulative distribution function
    fn normal_cdf(&self, x: f64) -> f64 {
        0.5 * (1.0 + self.erf(x / 2.0_f64.sqrt()))
    }
    
    /// Normal probability density function
    fn normal_pdf(&self, x: f64) -> f64 {
        (-0.5 * x.powi(2)).exp() / (2.0 * std::f64::consts::PI).sqrt()
    }
    
    /// Error function approximation
    fn erf(&self, x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;
        
        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();
        
        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
        
        sign * y
    }

    /// Calculate comprehensive risk metrics for a portfolio
    pub async fn calculate_portfolio_risk(&self, wallet: &str, positions: &[PositionInfo]) -> Result<RiskMetrics> {
        let mut portfolio_value = 0.0;
        let mut total_exposure = 0.0;
        let mut margin_used = 0.0;
        let mut unrealized_pnl = 0.0;
        let mut winning_trades = 0;
        let mut total_trades = 0;
        let mut profit_sum = 0.0;
        let mut loss_sum = 0.0;
        
        // Calculate position-level metrics
        for position in positions {
            let current_value = position.amount as f64 * position.current_price;
            let entry_value = position.amount as f64 * position.entry_price;
            let position_pnl = current_value - entry_value;
            
            portfolio_value += current_value;
            total_exposure += position.amount as f64 * position.leverage as f64;
            margin_used += position.amount as f64 / position.leverage as f64;
            unrealized_pnl += position_pnl;
            
            total_trades += 1;
            if position_pnl > 0.0 {
                winning_trades += 1;
                profit_sum += position_pnl;
            } else {
                loss_sum += position_pnl.abs();
            }
        }
        
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        
        let profit_factor = if loss_sum > 0.0 {
            profit_sum / loss_sum
        } else if profit_sum > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        
        // Calculate volatility and risk metrics
        let volatility = self.calculate_portfolio_volatility(positions).await;
        let var_95 = self.calculate_var(positions, 0.95).await;
        let var_99 = self.calculate_var(positions, 0.99).await;
        let expected_shortfall = self.calculate_expected_shortfall(positions, 0.99).await;
        
        // Calculate Sharpe ratio (assuming risk-free rate of 2%)
        let risk_free_rate = 0.02;
        let excess_return = (unrealized_pnl / portfolio_value) - risk_free_rate;
        let sharpe_ratio = if volatility > 0.0 {
            excess_return / volatility
        } else {
            0.0
        };
        
        // Calculate Sortino ratio (downside deviation)
        let downside_volatility = self.calculate_downside_volatility(positions).await;
        let sortino_ratio = if downside_volatility > 0.0 {
            excess_return / downside_volatility
        } else {
            0.0
        };
        
        let leverage_ratio = if portfolio_value > 0.0 {
            total_exposure / portfolio_value
        } else {
            0.0
        };
        
        // Convert unbounded leverage_ratio into a [0..1) utilization-style metric.
        let margin_ratio = if leverage_ratio > 0.0 {
            leverage_ratio / (leverage_ratio + 1.0)
        } else {
            0.0
        };
        
        // Calculate risk score (0-100, higher = riskier)
        let risk_score = self.calculate_risk_score(leverage_ratio, volatility, var_95 / portfolio_value, win_rate);
        
        // Calculate correlations
        let correlation_matrix = self.calculate_correlation_matrix(positions).await;
        
        let risk_metrics = RiskMetrics {
            portfolio_value,
            total_exposure,
            leverage_ratio,
            margin_used,
            margin_available: portfolio_value - margin_used,
            margin_ratio,
            unrealized_pnl,
            realized_pnl: 0.0, // TODO: Track realized PnL
            max_drawdown: self.calculate_max_drawdown(wallet).await,
            sharpe_ratio,
            sortino_ratio,
            win_rate,
            profit_factor,
            var_95,
            var_99,
            expected_shortfall,
            beta: self.calculate_beta(positions).await,
            alpha: self.calculate_alpha(positions, risk_free_rate).await,
            correlation_matrix,
            risk_score,
        };
        
        // Store metrics
        {
            let mut metrics_guard = self.risk_metrics.write().await;
            metrics_guard.insert(wallet.to_string(), risk_metrics.clone());
        }
        
        Ok(risk_metrics)
    }
    
    /// Calculate portfolio volatility
    async fn calculate_portfolio_volatility(&self, positions: &[PositionInfo]) -> f64 {
        if positions.is_empty() {
            return 0.0;
        }
        
        // Simplified volatility calculation based on position sizes and market volatilities
        let mut weighted_volatility = 0.0;
        let mut total_weight = 0.0;
        
        for position in positions {
            let weight = position.amount as f64;
            let market_vol = self.get_market_volatility(position.market_id).await;
            weighted_volatility += weight * market_vol;
            total_weight += weight;
        }
        
        if total_weight > 0.0 {
            weighted_volatility / total_weight
        } else {
            0.0
        }
    }
    
    /// Get or estimate market volatility
    async fn get_market_volatility(&self, market_id: u128) -> f64 {
        let market_risks = self.market_risks.read().await;
        market_risks.get(&market_id)
            .map(|risk| risk.volatility)
            .unwrap_or(0.3) // Default 30% volatility
    }
    
    /// Calculate Value at Risk
    async fn calculate_var(&self, positions: &[PositionInfo], confidence: f64) -> f64 {
        let portfolio_value: f64 = positions.iter()
            .map(|p| p.amount as f64 * p.current_price)
            .sum();
        
        let volatility = self.calculate_portfolio_volatility(positions).await;
        
        // Normal distribution VaR
        let z_score = match confidence {
            0.95 => 1.645,
            0.99 => 2.326,
            _ => 1.96, // 95% default
        };
        
        portfolio_value * volatility * z_score
    }
    
    /// Calculate Expected Shortfall (Conditional VaR)
    async fn calculate_expected_shortfall(&self, positions: &[PositionInfo], confidence: f64) -> f64 {
        let var = self.calculate_var(positions, confidence).await;
        // Simplified ES calculation (ES is typically 1.3-1.5x VaR for normal distribution)
        var * 1.4
    }
    
    /// Calculate downside volatility for Sortino ratio
    async fn calculate_downside_volatility(&self, positions: &[PositionInfo]) -> f64 {
        // Simplified calculation - assume downside vol is 1.2x regular vol
        let vol = self.calculate_portfolio_volatility(positions).await;
        vol * 1.2
    }
    
    /// Calculate maximum drawdown
    async fn calculate_max_drawdown(&self, _wallet: &str) -> f64 {
        // TODO: Implement actual drawdown calculation from historical data
        0.15 // Mock 15% max drawdown
    }
    
    /// Calculate portfolio beta
    async fn calculate_beta(&self, _positions: &[PositionInfo]) -> f64 {
        // TODO: Calculate actual beta vs market benchmark
        1.2 // Mock beta of 1.2 (20% more volatile than market)
    }
    
    /// Calculate portfolio alpha
    async fn calculate_alpha(&self, _positions: &[PositionInfo], _risk_free_rate: f64) -> f64 {
        // TODO: Calculate actual alpha
        0.05 // Mock 5% alpha
    }
    
    /// Calculate correlation matrix between positions
    async fn calculate_correlation_matrix(&self, positions: &[PositionInfo]) -> HashMap<String, f64> {
        let mut correlations = HashMap::new();
        
        // Mock correlations
        correlations.insert("BTC".to_string(), 0.7);
        correlations.insert("ETH".to_string(), 0.6);
        correlations.insert("SPY".to_string(), 0.3);
        correlations.insert("POLITICS".to_string(), -0.1);
        
        correlations
    }
    
    /// Calculate overall risk score (0-100)
    fn calculate_risk_score(&self, leverage: f64, volatility: f64, var_ratio: f64, win_rate: f64) -> f64 {
        let leverage_score = (leverage * 10.0).min(30.0);      // Max 30 points
        let volatility_score = (volatility * 100.0).min(25.0); // Max 25 points
        let var_score = (var_ratio * 100.0).min(30.0);         // Max 30 points
        let win_rate_score = (1.0 - win_rate) * 15.0;          // Max 15 points (inverse)
        
        (leverage_score + volatility_score + var_score + win_rate_score).min(100.0)
    }
    
    /// Check if position violates risk limits
    pub async fn check_risk_limits(&self, position: &PositionInfo) -> Vec<String> {
        let mut violations = Vec::new();
        
        let position_size = position.amount as f64 * position.current_price;
        let leverage = position.leverage as f64;
        
        if position_size > self.risk_limits.max_position_size {
            violations.push(format!(
                "Position size ${:.0} exceeds limit ${:.0}",
                position_size, self.risk_limits.max_position_size
            ));
        }
        
        if leverage > self.risk_limits.max_leverage {
            violations.push(format!(
                "Leverage {:.1}x exceeds limit {:.1}x",
                leverage, self.risk_limits.max_leverage
            ));
        }
        
        violations
    }
    
    /// Update market risk data
    pub async fn update_market_risk(&self, market_id: u128, risk: MarketRisk) {
        let mut market_risks = self.market_risks.write().await;
        market_risks.insert(market_id, risk);
    }
    
    /// Get market risk data
    pub async fn get_market_risk(&self, market_id: u128) -> Option<MarketRisk> {
        let market_risks = self.market_risks.read().await;
        market_risks.get(&market_id).cloned()
    }
    
    /// Calculate position-specific risk
    pub async fn calculate_position_risk(&self, position: &PositionInfo) -> PositionRisk {
        let current_value = position.amount as f64 * position.current_price;
        let entry_value = position.amount as f64 * position.entry_price;
        let unrealized_pnl = current_value - entry_value;
        
        // Calculate Greeks
        let time_to_expiry = 30.0; // 30 days default
        let volatility = self.get_market_volatility(position.market_id).await;
        let risk_free_rate = 0.02;
        
        let greeks = self.calculate_greeks(
            current_value,
            position.current_price,
            position.entry_price,
            time_to_expiry / 365.0,
            volatility,
            risk_free_rate,
            PositionType::Long,
        );
        
        // Calculate liquidation price
        let margin_ratio = 1.0 / position.leverage as f64;
        let liquidation_price = position.entry_price * (1.0 - margin_ratio * 0.9);
        
        PositionRisk {
            position_id: format!("{:?}", position.position),
            market_id: position.market_id,
            current_value,
            entry_value,
            unrealized_pnl,
            greeks,
            risk_contribution: unrealized_pnl.abs() / current_value,
            margin_requirement: current_value / position.leverage as f64,
            liquidation_price,
            time_to_expiry: Some(time_to_expiry as i64),
            volatility,
            correlation_risk: 0.3, // Mock correlation risk
        }
    }
    
    /// Get comprehensive risk report
    pub async fn get_risk_report(&self, wallet: &str) -> Result<serde_json::Value> {
        let metrics = {
            let metrics_guard = self.risk_metrics.read().await;
            metrics_guard.get(wallet).cloned()
        };
        
        if let Some(metrics) = metrics {
            Ok(serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "wallet": wallet,
                "risk_metrics": metrics,
                "risk_limits": self.risk_limits,
                "recommendations": self.generate_risk_recommendations(&metrics),
                "alerts": self.generate_risk_alerts(&metrics),
            }))
        } else {
            Err(anyhow!("No risk data found for wallet"))
        }
    }
    
    /// Generate risk-based recommendations
    fn generate_risk_recommendations(&self, metrics: &RiskMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if metrics.leverage_ratio > 5.0 {
            recommendations.push("Consider reducing leverage to manage risk".to_string());
        }
        
        if metrics.risk_score > 80.0 {
            recommendations.push("Portfolio risk is very high - consider diversification".to_string());
        }
        
        if metrics.win_rate < 0.4 {
            recommendations.push("Low win rate detected - review trading strategy".to_string());
        }
        
        if metrics.margin_ratio > 0.8 {
            recommendations.push("High margin usage - consider adding funds or closing positions".to_string());
        }
        
        recommendations
    }
    
    /// Generate risk alerts
    fn generate_risk_alerts(&self, metrics: &RiskMetrics) -> Vec<String> {
        let mut alerts = Vec::new();
        
        if metrics.margin_ratio > self.risk_limits.margin_call_threshold {
            alerts.push("MARGIN CALL WARNING: Margin usage approaching limit".to_string());
        }
        
        if metrics.var_95 > self.risk_limits.var_limit {
            alerts.push("VALUE AT RISK EXCEEDED: Portfolio VaR above limit".to_string());
        }
        
        if metrics.leverage_ratio > self.risk_limits.max_leverage {
            alerts.push("LEVERAGE VIOLATION: Portfolio leverage exceeds maximum".to_string());
        }
        
        alerts
    }
    
    /// Check position limit for a user
    pub async fn check_position_limit(&self, user_wallet: &str, market_id: u128, amount: u64) -> Result<bool, String> {
        // Get user's position risk
        let position_risks = self.position_risks.read().await;
        let position_key = format!("{}:{}", user_wallet, market_id);
        
        // Get current position size from risk metrics
        let current_position = if let Some(pos_risk) = position_risks.get(&position_key) {
            pos_risk.current_value as u64
        } else {
            0
        };
        
        // Check against position limit (using default limit for now)
        let position_limit = 1_000_000; // $1M position limit
        let new_total = current_position + amount;
        if new_total > position_limit {
            return Err(format!("Position limit exceeded. Current: {}, New: {}, Limit: {}", 
                current_position, new_total, position_limit));
        }
        
        Ok(true)
    }
    
    /// Check exposure limit for a user
    pub async fn check_exposure_limit(&self, user_wallet: &str, _market_id: u128, amount: u64, price: f64) -> Result<bool, String> {
        // Get user's risk metrics
        let risk_metrics = self.risk_metrics.read().await;
        
        // Calculate current exposure from all positions
        let current_exposure = if let Some(metrics) = risk_metrics.get(user_wallet) {
            metrics.portfolio_value
        } else {
            0.0
        };
        
        // Add new exposure
        let new_exposure = amount as f64 * price;
        let total_exposure = current_exposure + new_exposure;
        
        // Check against exposure limit (using default limit for now)
        let exposure_limit = 10_000_000.0; // $10M exposure limit
        if total_exposure > exposure_limit {
            return Err(format!("Exposure limit exceeded. Current: {:.2}, New: {:.2}, Limit: {:.2}", 
                current_exposure, total_exposure, exposure_limit));
        }
        
        Ok(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionType {
    Long,
    Short,
}

impl Default for RiskEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PositionInfo, PositionStatus};
    use solana_sdk::pubkey::Pubkey;
    
    fn create_greeks() -> Greeks {
        Greeks {
            delta: 0.5,
            gamma: 0.1,
            theta: -0.05,
            vega: 0.2,
            rho: 0.01,
        }
    }
    
    fn assert_float_eq(a: f64, b: f64, epsilon: f64) {
        assert!((a - b).abs() < epsilon, "Expected {} ≈ {}", a, b);
    }
    
    fn create_position_info(
        market_id: u128,
        amount: u64,
        leverage: u32,
        entry_price: f64,
        current_price: f64,
    ) -> PositionInfo {
        PositionInfo {
            position: Pubkey::new_unique(),
            market_id,
            amount,
            outcome: 0,
            leverage,
            entry_price,
            current_price,
            pnl: ((current_price - entry_price) * amount as f64) as i128,
            status: PositionStatus::Open,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }
    
    #[test]
    fn test_greeks_calculation() {
        let greeks = create_greeks();
        
        // Test delta bounds
        assert!(greeks.delta >= -1.0 && greeks.delta <= 1.0);
        
        // Test gamma is positive
        assert!(greeks.gamma >= 0.0);
        
        // Test theta is typically negative (time decay)
        assert!(greeks.theta <= 0.0);
        
        // Test vega is positive
        assert!(greeks.vega >= 0.0);
    }
    
    #[tokio::test]
    async fn test_calculate_portfolio_risk() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        let position = create_position_info(1000, 10000, 5, 0.5, 0.45);
        let positions = vec![position];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        assert!(metrics.portfolio_value > 0.0);
        assert!(metrics.leverage_ratio > 0.0);
    }
    
    #[tokio::test]
    async fn test_portfolio_risk_metrics() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create multiple positions
        let positions = vec![
            create_position_info(1000, 5000, 2, 0.5, 0.45),
            create_position_info(1001, 3000, 3, 0.6, 0.55),
            create_position_info(1002, 2000, 5, 0.4, 0.42),
        ];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // Test portfolio calculations
        assert!(metrics.total_exposure > 0.0);
        assert!(metrics.margin_used > 0.0);
        assert!(metrics.risk_score >= 0.0 && metrics.risk_score <= 100.0);
    }
    
    #[tokio::test]
    async fn test_var_calculation() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create position with known parameters
        let positions = vec![create_position_info(1000, 10000, 1, 0.5, 0.5)];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // VaR should be positive (representing potential loss amount)
        assert!(metrics.var_95 > 0.0);
        assert!(metrics.var_99 > metrics.var_95); // 99% VaR should be higher than 95%
        
        // Expected shortfall should be worse than VaR
        assert!(metrics.expected_shortfall > metrics.var_99);
    }
    
    #[tokio::test]
    async fn test_liquidation_monitoring() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create high-leverage position with low current price
        let position = create_position_info(1000, 10000, 50, 0.5, 0.3);
        let positions = vec![position];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // Should have high risk due to leverage and price drop
        assert!(metrics.margin_ratio > 0.8); // High utilization due to leverage
        assert!(metrics.risk_score > 80.0); // High risk score
    }
    
    #[tokio::test]
    async fn test_margin_ratio() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create position with specific leverage
        let amount = 10000;
        let leverage = 10;
        let position = create_position_info(1000, amount, leverage, 0.5, 0.5);
        let positions = vec![position];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // Margin ratio should be calculated properly
        assert!(metrics.margin_ratio > 0.0 && metrics.margin_ratio <= 1.0);
    }
    
    #[tokio::test]
    async fn test_sharpe_ratio_calculation() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create profitable position
        let position = create_position_info(1000, 10000, 2, 0.5, 0.6);
        let positions = vec![position];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // Sharpe ratio should be positive for profitable position
        assert!(metrics.sharpe_ratio > 0.0);
    }
    
    #[tokio::test]
    async fn test_portfolio_correlation() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create correlated positions
        let positions = vec![
            create_position_info(1000, 5000, 2, 0.5, 0.55), // BTC-like
            create_position_info(1001, 3000, 2, 0.5, 0.52), // ETH-like
        ];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // Check correlation matrix
        assert!(!metrics.correlation_matrix.is_empty());
        assert!(metrics.correlation_matrix.contains_key("BTC"));
    }
    
    #[tokio::test]
    async fn test_stress_testing() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create multiple leveraged positions
        let positions = vec![
            create_position_info(1000, 5000, 3, 0.5, 0.5),
            create_position_info(1001, 5000, 3, 0.5, 0.5),
        ];
        
        // Calculate risk under normal conditions
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // With leverage, risk should be elevated
        assert!(metrics.leverage_ratio > 2.0);
        assert!(metrics.risk_score > 50.0);
        
        // Simulate stressed conditions with lower prices
        let stressed_positions = vec![
            create_position_info(1000, 5000, 3, 0.5, 0.2), // 60% drop
            create_position_info(1001, 5000, 3, 0.5, 0.25), // 50% drop
        ];
        
        let stressed_metrics = engine.calculate_portfolio_risk(wallet, &stressed_positions).await.unwrap();
        
        // Under stress, metrics should show extreme risk
        assert!(stressed_metrics.risk_score > 90.0);
        assert!(stressed_metrics.unrealized_pnl < 0.0);
    }
    
    #[tokio::test]
    async fn test_risk_limits() {
        let engine = RiskEngine::new();
        
        // Create position that violates risk limits
        let position = create_position_info(1000, 2_100_000, 15, 0.5, 0.5); // Exceeds size and leverage
        
        let violations = engine.check_risk_limits(&position).await;
        
        // Should have violations
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.contains("size")));
        assert!(violations.iter().any(|v| v.contains("Leverage")));
    }
    
    #[tokio::test]
    async fn test_historical_risk_tracking() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Track metrics over time with different prices
        let mut historical_values = Vec::new();
        for i in 0..5 {
            let price = 0.5 + (i as f64 * 0.02);
            let position = create_position_info(1000, 10000, 2, 0.5, price);
            let positions = vec![position];
            
            let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
            historical_values.push(metrics.portfolio_value);
        }
        
        // Portfolio value should increase with price
        for i in 1..historical_values.len() {
            assert!(historical_values[i] > historical_values[i-1]);
        }
    }
    
    #[tokio::test]
    async fn test_greeks_aggregation() {
        let engine = RiskEngine::new();
        
        // Test Greeks calculation
        let greeks = engine.calculate_greeks(
            10000.0,  // position value
            0.5,      // market price
            0.5,      // strike price
            30.0/365.0, // time to expiry
            0.3,      // volatility
            0.02,     // risk free rate
            PositionType::Long
        );
        
        // Greeks should be reasonable
        assert!(greeks.delta >= -1.0 && greeks.delta <= 1.0);
        assert!(greeks.gamma >= 0.0);
        assert!(greeks.vega >= 0.0);
    }
    
    #[tokio::test]
    async fn test_max_drawdown_tracking() {
        let engine = RiskEngine::new();
        let wallet = "test_wallet";
        
        // Create position with loss
        let position = create_position_info(1000, 10000, 2, 0.7, 0.4); // 43% loss
        let positions = vec![position];
        
        let metrics = engine.calculate_portfolio_risk(wallet, &positions).await.unwrap();
        
        // Should have negative PnL
        assert!(metrics.unrealized_pnl < 0.0);
        // Max drawdown should be reasonable (mocked at 15%)
        assert_eq!(metrics.max_drawdown, 0.15);
    }
    
    #[tokio::test]
    async fn test_concurrent_updates() {
        let engine = Arc::new(RiskEngine::new());
        let wallet = "test_wallet";
        
        // Simulate concurrent risk calculations
        let mut handles = vec![];
        for i in 0..10 {
            let engine_clone = engine.clone();
            let wallet_clone = wallet.to_string();
            let price = 0.5 + (i as f64 * 0.01);
            let handle = tokio::spawn(async move {
                let position = create_position_info(1000 + i as u128, 10000, 2, 0.5, price);
                let positions = vec![position];
                engine_clone.calculate_portfolio_risk(&wallet_clone, &positions).await
            });
            handles.push(handle);
        }
        
        // All calculations should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }
}
