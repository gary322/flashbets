//! Real-time performance display module
//!
//! Provides live updates of user performance metrics, win rates, and P&L
//! Designed for dashboard integration with low latency updates

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
    state::accounts::{UserMap, Position},
    analytics::{UserLTVMetrics, performance_metrics::{UserPerformanceMetrics, PerformanceRating}},
    events::{Event, EventType},
    define_event,
};

/// Display format for performance data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum DisplayFormat {
    Compact,    // Minimal data for mobile
    Standard,   // Regular dashboard view
    Detailed,   // Full analytics view
    Export,     // CSV/JSON export format
}

/// Performance snapshot for real-time display
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PerformanceSnapshot {
    /// User public key
    pub user: Pubkey,
    
    /// Snapshot timestamp
    pub timestamp: i64,
    
    /// Current P&L (scaled by 1e6)
    pub current_pnl: i64,
    
    /// Win rate percentage (0-100)
    pub win_rate: u8,
    
    /// Average ROI percentage (scaled by 100)
    pub avg_roi: i64,
    
    /// Current streak
    pub current_streak: i16,
    
    /// Best streak
    pub best_streak: i16,
    
    /// Total volume traded (USDC)
    pub total_volume: u64,
    
    /// Active positions count
    pub active_positions: u8,
    
    /// Risk score (0-100)
    pub risk_score: u8,
    
    /// Performance rating
    pub rating: PerformanceRating,
}


/// Real-time dashboard data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DashboardData {
    /// User performance snapshot
    pub performance: PerformanceSnapshot,
    
    /// Recent position outcomes (last 10)
    pub recent_outcomes: Vec<PositionOutcome>,
    
    /// Top performing markets
    pub top_markets: Vec<MarketPerformance>,
    
    /// Risk alerts
    pub risk_alerts: Vec<RiskAlert>,
    
    /// Next milestone progress
    pub milestone_progress: MilestoneProgress,
}

/// Position outcome for display
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionOutcome {
    pub market_id: [u8; 32],
    pub is_win: bool,
    pub pnl: i64,
    pub roi: i64,
    pub leverage: u64,
    pub duration: u64,
    pub closed_at: i64,
}

/// Market performance summary
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketPerformance {
    pub market_id: [u8; 32],
    pub total_trades: u32,
    pub win_rate: u8,
    pub total_pnl: i64,
    pub avg_roi: i64,
}

/// Risk alert types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum RiskAlert {
    HighLeverage { current: u64, recommended: u64 },
    LowWinRate { current: u8, threshold: u8 },
    DrawdownWarning { percentage: u8 },
    ConcentrationRisk { market_id: [u8; 32], percentage: u8 },
    StreakAlert { losing_streak: u8 },
}

/// Milestone progress tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MilestoneProgress {
    pub next_milestone: String,
    pub current_value: u64,
    pub target_value: u64,
    pub percentage: u8,
    pub reward_amount: u64,
}

/// Get performance snapshot for real-time display
pub fn process_get_performance_snapshot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    display_format: DisplayFormat,
) -> ProgramResult {
    msg!("Getting performance snapshot");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let user = next_account_info(account_info_iter)?;
    let user_pda_account = next_account_info(account_info_iter)?;
    let performance_metrics_account = next_account_info(account_info_iter)?;
    let ltv_metrics_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify PDAs
    let (user_pda, _) = Pubkey::find_program_address(
        &[b"user", user.key.as_ref()],
        program_id,
    );
    
    if user_pda != *user_pda_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    let (performance_pda, _) = Pubkey::find_program_address(
        &[b"performance", user.key.as_ref()],
        program_id,
    );
    
    if performance_pda != *performance_metrics_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load accounts
    let user_data = UserMap::try_from_slice(&user_pda_account.data.borrow())?;
    let performance = UserPerformanceMetrics::try_from_slice(&performance_metrics_account.data.borrow())?;
    let ltv = UserLTVMetrics::try_from_slice(&ltv_metrics_account.data.borrow())?;
    
    let clock_data = Clock::from_account_info(clock)?;
    
    // Calculate current P&L from active positions
    let mut current_pnl = performance.net_pnl;
    let active_positions = user_data.positions.len().min(255) as u8;
    
    // Calculate win rate
    let win_rate = if performance.total_positions > 0 {
        ((performance.winning_positions * 100) / performance.total_positions).min(100) as u8
    } else {
        0
    };
    
    // Calculate average ROI
    let avg_roi = if performance.total_positions > 0 {
        performance.net_pnl.checked_div(performance.total_positions as i64)
            .unwrap_or(0)
    } else {
        0
    };
    
    // Determine performance rating
    let rating = calculate_performance_rating(&performance, &ltv);
    
    // Create snapshot
    let snapshot = PerformanceSnapshot {
        user: *user.key,
        timestamp: clock_data.unix_timestamp,
        current_pnl,
        win_rate,
        avg_roi,
        current_streak: performance.current_streak,
        best_streak: performance.best_win_streak as i16,
        total_volume: performance.total_positions * 1_000_000, // Approximation
        active_positions,
        risk_score: performance.risk_score,
        rating,
    };
    
    // Log based on display format
    match display_format {
        DisplayFormat::Compact => {
            msg!("=== PERFORMANCE SNAPSHOT (COMPACT) ===");
            msg!("P&L: ${}", current_pnl as f64 / 1_000_000.0);
            msg!("Win Rate: {}%", win_rate);
            msg!("Active: {}", active_positions);
        }
        DisplayFormat::Standard => {
            msg!("=== PERFORMANCE SNAPSHOT ===");
            msg!("User: {}", user.key);
            msg!("P&L: ${}", current_pnl as f64 / 1_000_000.0);
            msg!("Win Rate: {}%", win_rate);
            msg!("Avg ROI: {}%", avg_roi as f64 / 100.0);
            msg!("Streak: {} (Best: {})", performance.current_streak, performance.best_win_streak);
            msg!("Volume: ${}", (performance.total_positions * 1_000_000) as f64 / 1_000_000.0);
            msg!("Active Positions: {}", active_positions);
            msg!("Risk Score: {}/100", performance.risk_score);
            msg!("Rating: {:?}", rating);
        }
        DisplayFormat::Detailed => {
            msg!("=== DETAILED PERFORMANCE SNAPSHOT ===");
            msg!("User: {}", user.key);
            msg!("Timestamp: {}", clock_data.unix_timestamp);
            msg!("");
            msg!("TRADING METRICS:");
            msg!("  Total P&L: ${}", current_pnl as f64 / 1_000_000.0);
            msg!("  Win Rate: {}% ({}/{})", win_rate, performance.winning_positions, performance.total_positions);
            msg!("  Average ROI: {}%", avg_roi as f64 / 100.0);
            msg!("  Profit Factor: {}", performance.profit_factor as f64 / 100.0);
            msg!("  Sharpe Ratio: {}", performance.sharpe_ratio as f64 / 100.0);
            msg!("");
            msg!("ACTIVITY:");
            msg!("  Total Volume: ${}", (performance.total_positions * 1_000_000) as f64 / 1_000_000.0);
            msg!("  Active Positions: {}", active_positions);
            msg!("  Current Streak: {}", performance.current_streak);
            msg!("  Best Streak: {}", performance.best_win_streak);
            msg!("  Consistency Score: {}/100", performance.consistency_score);
            msg!("");
            msg!("RISK PROFILE:");
            msg!("  Risk Score: {}/100", performance.risk_score);
            msg!("  Max Drawdown: {}%", performance.max_drawdown as f64 / 100.0);
            msg!("  LTV: ${}", ltv.current_ltv_usd as f64 / 1_000_000.0);
            msg!("  Retention Score: {}/100", ltv.retention_score);
            msg!("");
            msg!("RATING: {:?}", rating);
        }
        DisplayFormat::Export => {
            // JSON-like format for export
            msg!("{{");
            msg!("  \"user\": \"{}\",", user.key);
            msg!("  \"timestamp\": {},", clock_data.unix_timestamp);
            msg!("  \"pnl\": {},", current_pnl);
            msg!("  \"win_rate\": {},", win_rate);
            msg!("  \"avg_roi\": {},", avg_roi);
            msg!("  \"current_streak\": {},", performance.current_streak);
            msg!("  \"best_streak\": {},", performance.best_win_streak);
            msg!("  \"total_volume\": {},", performance.total_positions * 1_000_000);
            msg!("  \"active_positions\": {},", active_positions);
            msg!("  \"risk_score\": {},", performance.risk_score);
            msg!("  \"rating\": \"{:?}\"", rating);
            msg!("}}");
        }
    }
    
    // Emit display event
    let event = PerformanceSnapshotEvent {
        user: *user.key,
        timestamp: clock_data.unix_timestamp,
        pnl: current_pnl,
        win_rate,
        rating,
    };
    event.emit();
    
    Ok(())
}

/// Get full dashboard data including recent trades and alerts
pub fn process_get_dashboard_data(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Getting dashboard data");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let user = next_account_info(account_info_iter)?;
    let user_pda_account = next_account_info(account_info_iter)?;
    let performance_metrics_account = next_account_info(account_info_iter)?;
    let ltv_metrics_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // First get the performance snapshot
    let snapshot = get_performance_snapshot_internal(
        program_id,
        user,
        user_pda_account,
        performance_metrics_account,
        ltv_metrics_account,
        clock,
    )?;
    
    // Load performance metrics for additional data
    let performance = UserPerformanceMetrics::try_from_slice(&performance_metrics_account.data.borrow())?;
    
    // Generate risk alerts
    let risk_alerts = generate_risk_alerts(&snapshot, &performance);
    
    // Calculate milestone progress
    let milestone_progress = calculate_milestone_progress(&performance);
    
    // Log dashboard data
    msg!("=== DASHBOARD DATA ===");
    msg!("User: {}", user.key);
    msg!("");
    msg!("PERFORMANCE:");
    msg!("  P&L: ${}", snapshot.current_pnl as f64 / 1_000_000.0);
    msg!("  Win Rate: {}%", snapshot.win_rate);
    msg!("  Rating: {:?}", snapshot.rating);
    msg!("");
    
    if !risk_alerts.is_empty() {
        msg!("RISK ALERTS:");
        for alert in &risk_alerts {
            match alert {
                RiskAlert::HighLeverage { current, recommended } => {
                    msg!("  ⚠️ High Leverage: {}x (recommended: {}x)", current, recommended);
                }
                RiskAlert::LowWinRate { current, threshold } => {
                    msg!("  ⚠️ Low Win Rate: {}% (threshold: {}%)", current, threshold);
                }
                RiskAlert::DrawdownWarning { percentage } => {
                    msg!("  ⚠️ Drawdown Warning: {}%", percentage);
                }
                RiskAlert::ConcentrationRisk { market_id: _, percentage } => {
                    msg!("  ⚠️ Concentration Risk: {}% in single market", percentage);
                }
                RiskAlert::StreakAlert { losing_streak } => {
                    msg!("  ⚠️ Losing Streak: {} consecutive losses", losing_streak);
                }
            }
        }
        msg!("");
    }
    
    msg!("NEXT MILESTONE:");
    msg!("  {}: {}/{} ({}%)", 
        milestone_progress.next_milestone,
        milestone_progress.current_value,
        milestone_progress.target_value,
        milestone_progress.percentage
    );
    if milestone_progress.reward_amount > 0 {
        msg!("  Reward: {} MMT", milestone_progress.reward_amount / 1_000_000);
    }
    
    Ok(())
}

/// Internal function to get performance snapshot
fn get_performance_snapshot_internal(
    program_id: &Pubkey,
    user: &AccountInfo,
    user_pda_account: &AccountInfo,
    performance_metrics_account: &AccountInfo,
    ltv_metrics_account: &AccountInfo,
    clock: &AccountInfo,
) -> Result<PerformanceSnapshot, ProgramError> {
    // Load accounts
    let user_data = UserMap::try_from_slice(&user_pda_account.data.borrow())?;
    let performance = UserPerformanceMetrics::try_from_slice(&performance_metrics_account.data.borrow())?;
    let ltv = UserLTVMetrics::try_from_slice(&ltv_metrics_account.data.borrow())?;
    let clock_data = Clock::from_account_info(clock)?;
    
    // Calculate metrics
    let current_pnl = performance.net_pnl;
    let active_positions = user_data.positions.len().min(255) as u8;
    
    let win_rate = if performance.total_positions > 0 {
        ((performance.winning_positions * 100) / performance.total_positions).min(100) as u8
    } else {
        0
    };
    
    let avg_roi = if performance.total_positions > 0 {
        performance.net_pnl.checked_div(performance.total_positions as i64).unwrap_or(0)
    } else {
        0
    };
    
    let rating = calculate_performance_rating(&performance, &ltv);
    
    Ok(PerformanceSnapshot {
        user: *user.key,
        timestamp: clock_data.unix_timestamp,
        current_pnl,
        win_rate,
        avg_roi,
        current_streak: performance.current_streak,
        best_streak: performance.best_win_streak as i16,
        total_volume: performance.total_positions * 1_000_000, // Approximation
        active_positions,
        risk_score: performance.risk_score,
        rating,
    })
}

/// Calculate performance rating based on multiple factors
fn calculate_performance_rating(
    performance: &UserPerformanceMetrics,
    ltv: &UserLTVMetrics,
) -> PerformanceRating {
    let mut score = 0u32;
    
    // Win rate component (0-30 points)
    let win_rate = if performance.total_positions > 0 {
        ((performance.winning_positions * 100) / performance.total_positions).min(100) as u32
    } else {
        0
    };
    score += (win_rate * 30) / 100;
    
    // Profit factor component (0-25 points)
    if performance.profit_factor >= 200 {
        score += 25;
    } else if performance.profit_factor >= 150 {
        score += 20;
    } else if performance.profit_factor >= 120 {
        score += 15;
    } else if performance.profit_factor >= 100 {
        score += 10;
    }
    
    // Volume component (0-20 points)
    let total_volume = performance.total_positions * 1_000_000;
    if total_volume >= 10_000_000_000_000 { // $10M
        score += 20;
    } else if total_volume >= 1_000_000_000_000 { // $1M
        score += 15;
    } else if total_volume >= 100_000_000_000 { // $100k
        score += 10;
    } else if total_volume >= 10_000_000_000 { // $10k
        score += 5;
    }
    
    // Consistency component (0-15 points)
    score += (performance.consistency_score as u32 * 15) / 100;
    
    // LTV component (0-10 points)
    if ltv.current_ltv_usd >= 500_000_000 { // $500 target
        score += 10;
    } else {
        score += (ltv.current_ltv_usd as u32 * 10) / 500_000_000;
    }
    
    // Determine rating based on profit factor and consistency
    if performance.total_positions < 10 {
        PerformanceRating::Unrated
    } else if performance.profit_factor < 80 {
        PerformanceRating::Learning
    } else if performance.profit_factor < 100 {
        PerformanceRating::BreakEven
    } else if performance.profit_factor < 120 {
        PerformanceRating::Profitable
    } else if performance.profit_factor < 150 {
        PerformanceRating::Professional
    } else if performance.profit_factor >= 150 && performance.consistency_score >= 70 {
        PerformanceRating::Elite
    } else {
        PerformanceRating::Professional
    }
}

/// Generate risk alerts based on current performance
fn generate_risk_alerts(
    snapshot: &PerformanceSnapshot,
    performance: &UserPerformanceMetrics,
) -> Vec<RiskAlert> {
    let mut alerts = Vec::new();
    
    // Check for low win rate
    if snapshot.win_rate < 40 && performance.total_positions >= 10 {
        alerts.push(RiskAlert::LowWinRate {
            current: snapshot.win_rate,
            threshold: 40,
        });
    }
    
    // Check for losing streak
    if performance.current_streak < -3 {
        alerts.push(RiskAlert::StreakAlert {
            losing_streak: (-performance.current_streak) as u8,
        });
    }
    
    // Check for high drawdown
    if performance.max_drawdown > 2000 { // 20%
        alerts.push(RiskAlert::DrawdownWarning {
            percentage: (performance.max_drawdown / 100) as u8,
        });
    }
    
    alerts
}

/// Calculate progress towards next milestone
fn calculate_milestone_progress(performance: &UserPerformanceMetrics) -> MilestoneProgress {
    // Define milestones
    let milestones = [
        (10, "First 10 Trades", 100_000_000), // 100 MMT
        (50, "50 Trade Veteran", 500_000_000), // 500 MMT
        (100, "Century Club", 1_000_000_000), // 1000 MMT
        (500, "Half Thousand", 5_000_000_000), // 5000 MMT
        (1000, "Thousand Trade Master", 10_000_000_000), // 10000 MMT
    ];
    
    // Find next milestone
    for (target, name, reward) in milestones.iter() {
        if performance.total_positions < *target as u64 {
            let percentage = ((performance.total_positions * 100) / (*target as u64)).min(100) as u8;
            
            return MilestoneProgress {
                next_milestone: name.to_string(),
                current_value: performance.total_positions,
                target_value: *target as u64,
                percentage,
                reward_amount: *reward,
            };
        }
    }
    
    // All milestones completed
    MilestoneProgress {
        next_milestone: "All Milestones Completed!".to_string(),
        current_value: performance.total_positions,
        target_value: performance.total_positions,
        percentage: 100,
        reward_amount: 0,
    }
}

// Event definitions
define_event!(PerformanceSnapshotEvent, EventType::PerformanceSnapshot, {
    user: Pubkey,
    timestamp: i64,
    pnl: i64,
    win_rate: u8,
    rating: PerformanceRating,
});