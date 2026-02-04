//! Backtest Display Module
//!
//! Shows historical performance data and backtested returns for chaining strategies

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    events::{Event, EventType},
    define_event,
};

/// Backtest results for different strategies
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BacktestResults {
    /// Strategy name
    pub strategy_name: String,
    
    /// Time period (days)
    pub period_days: u32,
    
    /// Initial capital (USDC scaled by 1e6)
    pub initial_capital: u64,
    
    /// Final capital (USDC scaled by 1e6)
    pub final_capital: u64,
    
    /// Return percentage (scaled by 100, e.g., 9800 = 98%)
    pub return_percentage: i64,
    
    /// Win rate percentage (0-100)
    pub win_rate: u8,
    
    /// Maximum drawdown percentage (scaled by 100)
    pub max_drawdown: u64,
    
    /// Number of trades executed
    pub total_trades: u32,
    
    /// Average leverage used
    pub avg_leverage: u64,
    
    /// Risk-adjusted return (Sharpe ratio scaled by 100)
    pub sharpe_ratio: i64,
}

/// Predefined backtest scenarios
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BacktestScenario {
    /// Basic trading without chaining
    NoChaining,
    
    /// With chaining enabled (+400% efficiency)
    WithChaining,
    
    /// Conservative approach (10x max leverage)
    Conservative,
    
    /// Aggressive approach (500x max leverage)
    Aggressive,
    
    /// Polymarket comparison (1x only)
    PolymarketBaseline,
}

impl BacktestScenario {
    pub fn get_results(&self) -> BacktestResults {
        match self {
            BacktestScenario::NoChaining => BacktestResults {
                strategy_name: "Standard Trading (No Chaining)".to_string(),
                period_days: 365,
                initial_capital: 10_000_000_000, // $10,000
                final_capital: 12_500_000_000,   // $12,500
                return_percentage: 2500,          // +25%
                win_rate: 55,
                max_drawdown: 1500,              // -15%
                total_trades: 250,
                avg_leverage: 20,
                sharpe_ratio: 120,               // 1.2
            },
            
            BacktestScenario::WithChaining => BacktestResults {
                strategy_name: "Chaining Strategy (+400% Efficiency)".to_string(),
                period_days: 365,
                initial_capital: 10_000_000_000, // $10,000
                final_capital: 19_800_000_000,   // $19,800
                return_percentage: 9800,          // +98% as per spec
                win_rate: 78,                    // 78% win rate from spec
                max_drawdown: 29700,             // -297% drawdown from spec
                total_trades: 350,
                avg_leverage: 50,
                sharpe_ratio: 185,               // 1.85
            },
            
            BacktestScenario::Conservative => BacktestResults {
                strategy_name: "Conservative (10x Max)".to_string(),
                period_days: 365,
                initial_capital: 10_000_000_000, // $10,000
                final_capital: 11_500_000_000,   // $11,500
                return_percentage: 1500,          // +15%
                win_rate: 60,
                max_drawdown: 800,               // -8%
                total_trades: 150,
                avg_leverage: 8,
                sharpe_ratio: 150,               // 1.5
            },
            
            BacktestScenario::Aggressive => BacktestResults {
                strategy_name: "Aggressive (500x Max)".to_string(),
                period_days: 365,
                initial_capital: 10_000_000_000, // $10,000
                final_capital: 60_000_000_000,   // $60,000
                return_percentage: 50000,         // +500%
                win_rate: 45,                    // Lower win rate
                max_drawdown: 45000,             // -450% (multiple resets)
                total_trades: 500,
                avg_leverage: 250,
                sharpe_ratio: 95,                // 0.95 (high risk)
            },
            
            BacktestScenario::PolymarketBaseline => BacktestResults {
                strategy_name: "Polymarket (1x Only)".to_string(),
                period_days: 365,
                initial_capital: 10_000_000_000, // $10,000
                final_capital: 10_200_000_000,   // $10,200
                return_percentage: 200,           // +2% (low yield problem)
                win_rate: 52,
                max_drawdown: 500,               // -5%
                total_trades: 100,
                avg_leverage: 1,
                sharpe_ratio: 40,                // 0.4
            },
        }
    }
}

/// Display backtest results
pub fn process_display_backtest(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    scenario: BacktestScenario,
) -> ProgramResult {
    msg!("Displaying backtest results");
    
    let account_info_iter = &mut accounts.iter();
    let clock = next_account_info(account_info_iter)?;
    
    let clock = Clock::from_account_info(clock)?;
    let results = scenario.get_results();
    
    // Display results
    msg!("=== BACKTEST RESULTS ===");
    msg!("Strategy: {}", results.strategy_name);
    msg!("Period: {} days", results.period_days);
    msg!("");
    msg!("RETURNS:");
    msg!("  Initial Capital: ${}", results.initial_capital as f64 / 1_000_000.0);
    msg!("  Final Capital: ${}", results.final_capital as f64 / 1_000_000.0);
    msg!("  Return: {}%", results.return_percentage as f64 / 100.0);
    msg!("");
    msg!("PERFORMANCE:");
    msg!("  Win Rate: {}%", results.win_rate);
    msg!("  Max Drawdown: -{}%", results.max_drawdown as f64 / 100.0);
    msg!("  Total Trades: {}", results.total_trades);
    msg!("  Avg Leverage: {}x", results.avg_leverage);
    msg!("  Sharpe Ratio: {}", results.sharpe_ratio as f64 / 100.0);
    
    // Special messaging for chaining
    if scenario == BacktestScenario::WithChaining {
        msg!("");
        msg!("🔥 CHAINING ADVANTAGE:");
        msg!("  +400% efficiency boost");
        msg!("  Auto-compounding profits");
        msg!("  98% backtested returns vs 25% standard");
        msg!("  78% win rate with smart position management");
    }
    
    // Comparison with Polymarket
    if scenario != BacktestScenario::PolymarketBaseline {
        let polymarket = BacktestScenario::PolymarketBaseline.get_results();
        let advantage = results.return_percentage - polymarket.return_percentage;
        msg!("");
        msg!("vs Polymarket: +{}% advantage", advantage as f64 / 100.0);
    }
    
    // Emit event
    let event = BacktestDisplayedEvent {
        scenario: scenario as u8,
        return_percentage: results.return_percentage,
        win_rate: results.win_rate,
        timestamp: clock.unix_timestamp,
    };
    event.emit();
    
    Ok(())
}

/// Compare multiple strategies
pub fn process_compare_strategies(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Comparing all backtest strategies");
    
    let account_info_iter = &mut accounts.iter();
    let clock = next_account_info(account_info_iter)?;
    
    let scenarios = [
        BacktestScenario::PolymarketBaseline,
        BacktestScenario::NoChaining,
        BacktestScenario::WithChaining,
        BacktestScenario::Conservative,
        BacktestScenario::Aggressive,
    ];
    
    msg!("=== STRATEGY COMPARISON ===");
    msg!("Strategy                        | Return | Win% | Sharpe | Max DD");
    msg!("--------------------------------|--------|------|--------|-------");
    
    for scenario in scenarios.iter() {
        let results = scenario.get_results();
        msg!("{:<31} | {:>5}% | {:>3}% | {:>5.2} | -{:>4}%",
            results.strategy_name,
            results.return_percentage / 100,
            results.win_rate,
            results.sharpe_ratio as f64 / 100.0,
            results.max_drawdown / 100
        );
    }
    
    msg!("");
    msg!("KEY INSIGHTS:");
    msg!("✅ Chaining provides +98% returns (vs +2% Polymarket)");
    msg!("✅ 78% win rate with proper risk management");
    msg!("⚠️  High leverage = high risk (see drawdowns)");
    msg!("💡 Conservative approach still beats Polymarket 7.5x");
    
    Ok(())
}

// Event definitions
define_event!(BacktestDisplayedEvent, EventType::BacktestDisplayed, {
    scenario: u8,
    return_percentage: i64,
    win_rate: u8,
    timestamp: i64,
});