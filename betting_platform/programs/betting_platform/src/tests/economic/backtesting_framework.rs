use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub initial_capital: f64,
    pub max_leverage: f64,
    pub chain_steps: Vec<ChainStep>,
    pub risk_limits: RiskLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    pub step_type: StepType,
    pub multiplier: f64,
    pub target_allocation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    Borrow,
    Liquidity,
    Stake,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_drawdown: f64,
    pub position_limit: f64,
    pub var_limit: f64,
}

pub struct BacktestEngine {
    config: BacktestConfig,
    price_data: Vec<PricePoint>,
    results: BacktestResults,
}

#[derive(Debug, Default, Clone)]
pub struct BacktestResults {
    pub total_return: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub total_fees: f64,
    pub liquidation_events: u32,
    pub daily_returns: Vec<f64>,
    pub positions: Vec<PositionResult>,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            config,
            price_data: Vec::new(),
            results: BacktestResults::default(),
        }
    }

    pub fn load_price_data(&mut self, data: Vec<PricePoint>) {
        self.price_data = data;
    }

    pub fn run_backtest(&mut self) -> BacktestResults {
        let mut capital = self.config.initial_capital;
        let mut positions = Vec::new();
        let mut daily_pnl = Vec::new();
        let mut high_water_mark = capital;

        // Simulate each time period
        for i in 1..self.price_data.len() {
            let current_price = self.price_data[i].price;
            let prev_price = self.price_data[i - 1].price;
            let price_change = (current_price - prev_price) / prev_price;

            // Apply strategy logic
            let signal = self.generate_signal(i);

            if signal != 0.0 {
                // Calculate effective leverage with chaining
                let base_leverage = signal.abs() * self.config.max_leverage;
                let effective_leverage = self.apply_chain_multipliers(base_leverage);

                // Open position
                let position = Position {
                    entry_price: current_price,
                    size: capital * effective_leverage,
                    leverage: effective_leverage,
                    direction: signal > 0.0,
                    entry_time: self.price_data[i].timestamp,
                    unrealized_pnl: 0.0,
                    realized_pnl: 0.0,
                    fees_paid: 0.0,
                };

                positions.push(position);
            }

            // Update existing positions
            let mut period_pnl = 0.0;
            let mut liquidated_positions = Vec::new();

            for (idx, position) in positions.iter_mut().enumerate() {
                // Calculate P&L
                let position_pnl = if position.direction {
                    position.size * price_change
                } else {
                    -position.size * price_change
                };

                period_pnl += position_pnl;

                // Check liquidation
                let liquidation_price = self.calculate_liquidation_price(
                    position.entry_price,
                    position.leverage,
                    position.direction
                );

                if (position.direction && current_price <= liquidation_price) ||
                   (!position.direction && current_price >= liquidation_price) {
                    liquidated_positions.push(idx);
                    self.results.liquidation_events += 1;
                    period_pnl = -position.size / position.leverage; // Lose entire margin
                }

                // Apply fees
                let fee = self.calculate_fees(position.size, position.leverage);
                period_pnl -= fee;
                self.results.total_fees += fee;
            }

            // Remove liquidated positions
            for idx in liquidated_positions.iter().rev() {
                positions.remove(*idx);
            }

            // Update capital
            capital += period_pnl;
            daily_pnl.push(period_pnl);

            // Track drawdown
            if capital > high_water_mark {
                high_water_mark = capital;
            }
            let drawdown = (high_water_mark - capital) / high_water_mark;
            self.results.max_drawdown = self.results.max_drawdown.max(drawdown);

            // Risk limits check
            if drawdown > self.config.risk_limits.max_drawdown {
                // Close all positions
                positions.clear();
            }
        }

        // Calculate final statistics
        self.calculate_statistics(daily_pnl, capital);

        self.results.clone()
    }

    fn apply_chain_multipliers(&self, base_leverage: f64) -> f64 {
        let mut effective = base_leverage;

        for step in &self.config.chain_steps {
            effective *= step.multiplier;
        }

        // Cap at 500x
        effective.min(500.0)
    }

    fn calculate_fees(&self, size: f64, leverage: f64) -> f64 {
        // Dynamic fee based on coverage
        let base_fee = 0.0003; // 3 basis points
        let coverage_factor = (leverage / 100.0).min(1.0);
        let fee_multiplier = (-3.0 * coverage_factor).exp();

        size * (base_fee + 0.0025 * fee_multiplier)
    }

    fn calculate_statistics(&mut self, daily_pnl: Vec<f64>, final_capital: f64) {
        let initial = self.config.initial_capital;
        self.results.total_return = (final_capital - initial) / initial;

        // Calculate daily returns
        let mut cumulative = initial;
        for pnl in &daily_pnl {
            let daily_return = pnl / cumulative;
            self.results.daily_returns.push(daily_return);
            cumulative += pnl;
        }

        // Sharpe ratio (annualized)
        if !self.results.daily_returns.is_empty() {
            let mean_return = self.results.daily_returns.iter().sum::<f64>() 
                / self.results.daily_returns.len() as f64;
            let variance = self.results.daily_returns.iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>() / self.results.daily_returns.len() as f64;
            let std_dev = variance.sqrt();

            if std_dev > 0.0 {
                self.results.sharpe_ratio = (mean_return * 252.0_f64.sqrt()) / std_dev;
            }
        }

        // Win rate
        let wins = daily_pnl.iter().filter(|&&pnl| pnl > 0.0).count();
        if !daily_pnl.is_empty() {
            self.results.win_rate = wins as f64 / daily_pnl.len() as f64;
        }

        // Average win/loss
        let winning_pnls: Vec<f64> = daily_pnl.iter()
            .filter(|&&pnl| pnl > 0.0)
            .cloned()
            .collect();
        let losing_pnls: Vec<f64> = daily_pnl.iter()
            .filter(|&&pnl| pnl < 0.0)
            .cloned()
            .collect();

        self.results.avg_win = if !winning_pnls.is_empty() {
            winning_pnls.iter().sum::<f64>() / winning_pnls.len() as f64
        } else {
            0.0
        };

        self.results.avg_loss = if !losing_pnls.is_empty() {
            losing_pnls.iter().sum::<f64>() / losing_pnls.len() as f64
        } else {
            0.0
        };
    }

    fn generate_signal(&self, index: usize) -> f64 {
        // Simplified momentum strategy for testing
        if index < 20 {
            return 0.0;
        }

        let recent_prices = &self.price_data[(index - 20)..index];
        let avg_20 = recent_prices.iter().map(|p| p.price).sum::<f64>() / 20.0;
        let current = self.price_data[index].price;

        if current > avg_20 * 1.02 {
            1.0 // Long signal
        } else if current < avg_20 * 0.98 {
            -1.0 // Short signal
        } else {
            0.0 // No signal
        }
    }

    fn calculate_liquidation_price(&self, entry: f64, leverage: f64, is_long: bool) -> f64 {
        let margin_ratio = 0.01; // 1% for simplicity
        if is_long {
            entry * (1.0 - margin_ratio / leverage)
        } else {
            entry * (1.0 + margin_ratio / leverage)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Position {
    pub entry_price: f64,
    pub size: f64,
    pub leverage: f64,
    pub direction: bool,
    pub entry_time: DateTime<Utc>,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub fees_paid: f64,
}

#[derive(Debug, Clone)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
}

#[derive(Debug, Clone)]
pub struct PositionResult {
    pub position: Position,
    pub exit_price: f64,
    pub pnl: f64,
    pub holding_period: u64,
    pub max_drawdown: f64,
}

#[cfg(test)]
mod backtest_tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_backtest_with_chaining() {
        // Generate sample data
        let mut data = Vec::new();
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        for i in 0..1000 {
            let timestamp = start + chrono::Duration::hours(i);
            let price = 0.5 + 0.1 * (i as f64 / 100.0).sin();

            data.push(PricePoint {
                timestamp,
                price,
                volume: 100000.0,
            });
        }

        // Run backtest with chaining
        let config = BacktestConfig {
            initial_capital: 10000.0,
            start_date: start,
            end_date: start + chrono::Duration::days(30),
            max_leverage: 100.0,
            chain_steps: vec![
                ChainStep { step_type: StepType::Borrow, multiplier: 1.5, target_allocation: 0.3 },
                ChainStep { step_type: StepType::Liquidity, multiplier: 1.2, target_allocation: 0.4 },
                ChainStep { step_type: StepType::Stake, multiplier: 1.1, target_allocation: 0.3 },
            ],
            risk_limits: RiskLimits {
                max_drawdown: 0.2,
                position_limit: 50000.0,
                var_limit: 0.1,
            },
        };

        let mut backtester = BacktestEngine::new(config);
        backtester.load_price_data(data.clone());
        let results_with_chain = backtester.run_backtest();

        // Run without chaining
        let config_no_chain = BacktestConfig {
            initial_capital: 10000.0,
            start_date: start,
            end_date: start + chrono::Duration::days(30),
            max_leverage: 100.0,
            chain_steps: vec![],
            risk_limits: RiskLimits {
                max_drawdown: 0.2,
                position_limit: 50000.0,
                var_limit: 0.1,
            },
        };

        let mut backtester_no_chain = BacktestEngine::new(config_no_chain);
        backtester_no_chain.load_price_data(data);
        let results_no_chain = backtester_no_chain.run_backtest();

        // Chaining should increase returns (and risk)
        println!("With chaining: return={:.2}%, max_dd={:.2}%, sharpe={:.2}",
            results_with_chain.total_return * 100.0,
            results_with_chain.max_drawdown * 100.0,
            results_with_chain.sharpe_ratio
        );

        println!("Without chaining: return={:.2}%, max_dd={:.2}%, sharpe={:.2}",
            results_no_chain.total_return * 100.0,
            results_no_chain.max_drawdown * 100.0,
            results_no_chain.sharpe_ratio
        );

        // Verify metrics are calculated
        assert!(results_with_chain.daily_returns.len() > 0);
        assert!(results_no_chain.daily_returns.len() > 0);
    }
}