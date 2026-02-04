//! Performance Metrics Tracking
//! 
//! Implements comprehensive performance tracking for users and markets
//! including win rates, liquidation rates, and profitability metrics.

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    constants::{BASIS_POINTS_DIVISOR, TARGET_WIN_RATE_BPS},
    error::BettingPlatformError,
    state::accounts::{Position, ProposalPDA, discriminators},
    events::{EventType, Event},
    define_event,
    math::calculate_pnl,
};

/// Performance tracking for individual users
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UserPerformanceMetrics {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Total positions opened
    pub total_positions: u64,
    
    /// Winning positions
    pub winning_positions: u64,
    
    /// Losing positions
    pub losing_positions: u64,
    
    /// Positions liquidated
    pub liquidated_positions: u64,
    
    /// Total profit in USD (scaled by 1e6)
    pub total_profit_usd: i64,
    
    /// Total loss in USD (scaled by 1e6)
    pub total_loss_usd: i64,
    
    /// Net PnL
    pub net_pnl: i64,
    
    /// Win rate (basis points)
    pub win_rate_bps: u16,
    
    /// Average win size
    pub avg_win_size: u64,
    
    /// Average loss size
    pub avg_loss_size: u64,
    
    /// Profit factor (total_profit / total_loss)
    pub profit_factor: u64, // Scaled by 100 (100 = 1.0)
    
    /// Sharpe ratio approximation
    pub sharpe_ratio: i64, // Scaled by 100
    
    /// Max drawdown in USD
    pub max_drawdown: u64,
    
    /// Max drawdown in basis points
    pub max_drawdown_bps: i32,
    
    /// Current drawdown in USD
    pub current_drawdown: u64,
    
    /// Current drawdown in basis points
    pub current_drawdown_bps: i32,
    
    /// Peak equity value (for drawdown calculation)
    pub peak_equity: u64,
    
    /// Win/loss ratio (avg win / avg loss, scaled by 100)
    pub win_loss_ratio: u64,
    
    /// Distance from target win rate (can be negative)
    pub win_rate_vs_target_bps: i16,
    
    /// Current streak (positive = win streak, negative = loss streak)
    pub current_streak: i16,
    
    /// Best win streak
    pub best_win_streak: u16,
    
    /// Worst loss streak
    pub worst_loss_streak: u16,
    
    /// Average holding time in slots
    pub avg_holding_time: u64,
    
    /// Most traded market
    pub favorite_market: [u8; 32],
    
    /// Performance by leverage tier
    pub performance_by_leverage: LeveragePerformance,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Risk score (0-100, higher = riskier)
    pub risk_score: u8,
    
    /// Consistency score (0-100, higher = more consistent)
    pub consistency_score: u8,
}

impl UserPerformanceMetrics {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        8 + // total_positions
        8 + // winning_positions
        8 + // losing_positions
        8 + // liquidated_positions
        8 + // total_profit_usd
        8 + // total_loss_usd
        8 + // net_pnl
        2 + // win_rate_bps
        8 + // avg_win_size
        8 + // avg_loss_size
        8 + // profit_factor
        8 + // sharpe_ratio
        8 + // max_drawdown
        4 + // max_drawdown_bps
        8 + // current_drawdown
        4 + // current_drawdown_bps
        8 + // peak_equity
        8 + // win_loss_ratio
        2 + // win_rate_vs_target_bps
        2 + // current_streak
        2 + // best_win_streak
        2 + // worst_loss_streak
        8 + // avg_holding_time
        32 + // favorite_market
        56 + // performance_by_leverage
        8 + // last_update
        1 + // risk_score
        1; // consistency_score

    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_STATS,
            user,
            total_positions: 0,
            winning_positions: 0,
            losing_positions: 0,
            liquidated_positions: 0,
            total_profit_usd: 0,
            total_loss_usd: 0,
            net_pnl: 0,
            win_rate_bps: 0,
            avg_win_size: 0,
            avg_loss_size: 0,
            profit_factor: 0,
            sharpe_ratio: 0,
            max_drawdown: 0,
            max_drawdown_bps: 0,
            current_drawdown: 0,
            current_drawdown_bps: 0,
            peak_equity: 0,
            win_loss_ratio: 0,
            win_rate_vs_target_bps: 0,
            current_streak: 0,
            best_win_streak: 0,
            worst_loss_streak: 0,
            avg_holding_time: 0,
            favorite_market: [0u8; 32],
            performance_by_leverage: LeveragePerformance::default(),
            last_update: Clock::get().unwrap_or_default().unix_timestamp,
            risk_score: 50,
            consistency_score: 50,
        }
    }

    /// Update metrics after position close
    pub fn update_position_close(
        &mut self,
        position: &Position,
        exit_price: u64,
        holding_time: u64,
    ) -> Result<(), ProgramError> {
        self.total_positions += 1;
        
        // Calculate PnL
        let pnl = calculate_pnl(
            position.entry_price,
            exit_price,
            position.size,
            position.leverage,
            position.is_long,
        )?;
        
        // Update win/loss stats
        if pnl > 0 {
            self.winning_positions += 1;
            self.total_profit_usd = self.total_profit_usd
                .checked_add(pnl)
                .ok_or(BettingPlatformError::MathOverflow)?;
            
            // Update win streak
            if self.current_streak >= 0 {
                self.current_streak += 1;
                if self.current_streak as u16 > self.best_win_streak {
                    self.best_win_streak = self.current_streak as u16;
                }
            } else {
                self.current_streak = 1;
            }
            
            // Update average win size
            self.avg_win_size = (self.total_profit_usd / self.winning_positions as i64) as u64;
        } else if pnl < 0 {
            self.losing_positions += 1;
            self.total_loss_usd = self.total_loss_usd
                .checked_add(pnl.abs())
                .ok_or(BettingPlatformError::MathOverflow)?;
            
            // Update loss streak
            if self.current_streak <= 0 {
                self.current_streak -= 1;
                if self.current_streak.abs() as u16 > self.worst_loss_streak {
                    self.worst_loss_streak = self.current_streak.abs() as u16;
                }
            } else {
                self.current_streak = -1;
            }
            
            // Update average loss size
            self.avg_loss_size = (self.total_loss_usd / self.losing_positions as i64) as u64;
        }
        
        // Update net PnL
        self.net_pnl = self.total_profit_usd - self.total_loss_usd;
        
        // Update win rate
        self.win_rate_bps = ((self.winning_positions * BASIS_POINTS_DIVISOR) / self.total_positions) as u16;
        
        // Calculate distance from target win rate
        self.win_rate_vs_target_bps = self.win_rate_bps as i16 - TARGET_WIN_RATE_BPS as i16;
        
        // Update profit factor
        if self.total_loss_usd > 0 {
            self.profit_factor = ((self.total_profit_usd * 100) / self.total_loss_usd) as u64;
        }
        
        // Update win/loss ratio
        if self.avg_win_size > 0 && self.avg_loss_size > 0 {
            self.win_loss_ratio = (self.avg_win_size * 100) / self.avg_loss_size;
        }
        
        // Update drawdown tracking
        self.update_drawdown_metrics()?;
        
        // Update average holding time
        let total_holding = self.avg_holding_time * (self.total_positions - 1) + holding_time;
        self.avg_holding_time = total_holding / self.total_positions;
        
        // Update leverage performance
        self.update_leverage_performance(position.leverage, pnl)?;
        
        // Update risk and consistency scores
        self.update_risk_score()?;
        self.update_consistency_score()?;
        
        self.last_update = Clock::get()?.unix_timestamp;
        
        Ok(())
    }

    /// Update metrics for liquidation
    pub fn update_liquidation(&mut self, position: &Position) -> Result<(), ProgramError> {
        self.liquidated_positions += 1;
        self.losing_positions += 1;
        self.total_positions += 1;
        
        // Full loss on liquidation
        let loss = position.size as i64;
        self.total_loss_usd = self.total_loss_usd
            .checked_add(loss)
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Update streaks
        if self.current_streak <= 0 {
            self.current_streak -= 1;
            if self.current_streak.abs() as u16 > self.worst_loss_streak {
                self.worst_loss_streak = self.current_streak.abs() as u16;
            }
        } else {
            self.current_streak = -1;
        }
        
        // Update metrics
        self.net_pnl = self.total_profit_usd - self.total_loss_usd;
        self.win_rate_bps = ((self.winning_positions * BASIS_POINTS_DIVISOR) / self.total_positions) as u16;
        
        // Calculate distance from target win rate
        self.win_rate_vs_target_bps = self.win_rate_bps as i16 - TARGET_WIN_RATE_BPS as i16;
        
        // Update win/loss ratio after liquidation
        if self.avg_loss_size > 0 {
            self.avg_loss_size = (self.total_loss_usd / self.losing_positions as i64) as u64;
            if self.avg_win_size > 0 {
                self.win_loss_ratio = (self.avg_win_size * 100) / self.avg_loss_size;
            }
        }
        
        // Update drawdown tracking
        self.update_drawdown_metrics()?;
        
        // Liquidations heavily impact risk score
        self.risk_score = (self.risk_score + 20).min(100);
        
        self.last_update = Clock::get()?.unix_timestamp;
        
        Ok(())
    }

    /// Update leverage-based performance tracking
    fn update_leverage_performance(&mut self, leverage: u64, pnl: i64) -> Result<(), ProgramError> {
        if leverage <= 10_000_000 { // 10x
            self.performance_by_leverage.low_leverage_trades += 1;
            if pnl > 0 {
                self.performance_by_leverage.low_leverage_wins += 1;
            }
        } else if leverage <= 50_000_000 { // 50x
            self.performance_by_leverage.medium_leverage_trades += 1;
            if pnl > 0 {
                self.performance_by_leverage.medium_leverage_wins += 1;
            }
        } else if leverage <= 100_000_000 { // 100x
            self.performance_by_leverage.high_leverage_trades += 1;
            if pnl > 0 {
                self.performance_by_leverage.high_leverage_wins += 1;
            }
        } else { // >100x
            self.performance_by_leverage.extreme_leverage_trades += 1;
            if pnl > 0 {
                self.performance_by_leverage.extreme_leverage_wins += 1;
            }
        }
        
        Ok(())
    }

    /// Calculate risk score based on trading behavior
    fn update_risk_score(&mut self) -> Result<(), ProgramError> {
        let mut risk_score = 50u8; // Base score
        
        // High liquidation rate increases risk
        if self.total_positions > 0 {
            let liquidation_rate = (self.liquidated_positions * 100) / self.total_positions;
            risk_score = risk_score.saturating_add((liquidation_rate * 2) as u8);
        }
        
        // High leverage usage increases risk
        let extreme_leverage_pct = if self.total_positions > 0 {
            (self.performance_by_leverage.extreme_leverage_trades * 100) / self.total_positions
        } else { 0 };
        risk_score = risk_score.saturating_add((extreme_leverage_pct * 3) as u8);
        
        // Poor win rate increases risk
        if self.win_rate_bps < 4000 { // < 40%
            risk_score = risk_score.saturating_add(10);
        }
        
        // Long loss streaks increase risk
        if self.worst_loss_streak > 5 {
            risk_score = risk_score.saturating_add(15);
        }
        
        self.risk_score = risk_score.min(100);
        Ok(())
    }

    /// Calculate consistency score
    fn update_consistency_score(&mut self) -> Result<(), ProgramError> {
        let mut consistency_score = 50u8; // Base score
        
        // Stable win rate improves consistency
        if self.win_rate_bps >= 4500 && self.win_rate_bps <= 5500 { // 45-55%
            consistency_score = consistency_score.saturating_add(20);
        }
        
        // Profit factor > 1 improves consistency
        if self.profit_factor > 100 { // > 1.0
            consistency_score = consistency_score.saturating_add(15);
        }
        
        // Low variance in win/loss sizes improves consistency
        if self.avg_win_size > 0 && self.avg_loss_size > 0 {
            let ratio = (self.avg_win_size * 100) / self.avg_loss_size;
            if ratio >= 80 && ratio <= 120 { // Similar sized wins/losses
                consistency_score = consistency_score.saturating_add(15);
            }
        }
        
        self.consistency_score = consistency_score.min(100);
        Ok(())
    }

    /// Update drawdown metrics
    fn update_drawdown_metrics(&mut self) -> Result<(), ProgramError> {
        // Calculate current equity (initial capital + net PnL)
        let initial_capital = 1_000_000_000u64; // Assume $1000 starting capital
        let current_equity = if self.net_pnl >= 0 {
            initial_capital.saturating_add(self.net_pnl as u64)
        } else {
            initial_capital.saturating_sub(self.net_pnl.abs() as u64)
        };
        
        // Update peak equity
        if current_equity > self.peak_equity || self.peak_equity == 0 {
            self.peak_equity = current_equity;
            self.current_drawdown = 0;
            self.current_drawdown_bps = 0;
        } else {
            // Calculate drawdown from peak
            self.current_drawdown = self.peak_equity.saturating_sub(current_equity);
            self.current_drawdown_bps = -((self.current_drawdown as i128 * 10000 / self.peak_equity as i128) as i32);
            
            // Update max drawdown if necessary
            if self.current_drawdown > self.max_drawdown {
                self.max_drawdown = self.current_drawdown;
                self.max_drawdown_bps = self.current_drawdown_bps;
            }
        }
        
        Ok(())
    }

    /// Get performance rating
    pub fn get_performance_rating(&self) -> PerformanceRating {
        if self.total_positions < 10 {
            return PerformanceRating::Unrated;
        }
        
        // Based on profit factor and consistency
        if self.profit_factor >= 150 && self.consistency_score >= 70 {
            PerformanceRating::Elite
        } else if self.profit_factor >= 120 && self.consistency_score >= 60 {
            PerformanceRating::Professional
        } else if self.profit_factor >= 100 && self.win_rate_bps >= 4500 {
            PerformanceRating::Profitable
        } else if self.profit_factor >= 80 {
            PerformanceRating::BreakEven
        } else {
            PerformanceRating::Learning
        }
    }
}

/// Performance breakdown by leverage tier
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Default)]
pub struct LeveragePerformance {
    pub low_leverage_trades: u64,      // <= 10x
    pub low_leverage_wins: u64,
    pub medium_leverage_trades: u64,   // 10-50x
    pub medium_leverage_wins: u64,
    pub high_leverage_trades: u64,     // 50-100x
    pub high_leverage_wins: u64,
    pub extreme_leverage_trades: u64,  // > 100x
    pub extreme_leverage_wins: u64,
}

/// Performance rating categories
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum PerformanceRating {
    Unrated,      // < 10 trades
    Learning,     // Negative profit factor
    BreakEven,    // 0.8-1.0 profit factor
    Profitable,   // 1.0-1.2 profit factor
    Professional, // 1.2-1.5 profit factor
    Elite,        // > 1.5 profit factor with consistency
}

/// Market performance metrics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketPerformanceMetrics {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Market ID (proposal or verse)
    pub market_id: [u8; 32],
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Number of unique traders
    pub unique_traders: u32,
    
    /// Average position size
    pub avg_position_size: u64,
    
    /// Liquidation rate (basis points)
    pub liquidation_rate_bps: u16,
    
    /// Average leverage used
    pub avg_leverage: u64,
    
    /// Market volatility score (0-100)
    pub volatility_score: u8,
    
    /// Liquidity score (0-100)
    pub liquidity_score: u8,
    
    /// Most active hour (0-23)
    pub peak_hour: u8,
    
    /// Chain position percentage
    pub chain_position_pct: u8,
    
    /// Last update
    pub last_update: i64,
}

/// Process performance update after position close
pub fn process_update_performance_metrics(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    exit_price: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let performance_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Load position
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    
    // Verify ownership
    if position.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Load or create performance metrics
    let mut metrics = if performance_account.data_is_empty() {
        UserPerformanceMetrics::new(*user.key)
    } else {
        UserPerformanceMetrics::try_from_slice(&performance_account.data.borrow())?
    };
    
    // Calculate holding time
    let clock = Clock::from_account_info(clock_sysvar)?;
    let holding_time = (clock.unix_timestamp - position.created_at).max(0) as u64;
    
    // Update metrics
    // Check if this is a liquidation (exit_price would be 0 for liquidation)
    if exit_price == 0 || position.is_closed {
        metrics.update_liquidation(&position)?;
    } else {
        metrics.update_position_close(&position, exit_price, holding_time)?;
    }
    
    // Get performance rating
    let rating = metrics.get_performance_rating();
    
    msg!(
        "Updated performance for {}: Win rate {}%, Profit factor {}, Rating: {:?}",
        user.key,
        metrics.win_rate_bps / 100,
        metrics.profit_factor,
        rating
    );
    
    // Emit event for significant milestones
    if metrics.total_positions == 100 || metrics.total_positions == 1000 {
        let event = PerformanceMilestone {
            user: *user.key,
            milestone_type: if metrics.total_positions == 100 { 
                MilestoneType::Trades100 
            } else { 
                MilestoneType::Trades1000 
            },
            win_rate_bps: metrics.win_rate_bps,
            profit_factor: metrics.profit_factor,
            rating,
            timestamp: clock.unix_timestamp,
        };
        event.emit();
    }
    
    // Save updated metrics
    metrics.serialize(&mut &mut performance_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Get performance summary for display
pub fn get_performance_summary(metrics: &UserPerformanceMetrics) -> String {
    let mut summary = String::from("📈 Performance Summary\n");
    summary.push_str("═══════════════════════\n\n");
    
    // Overall stats
    summary.push_str(&format!(
        "Total Positions: {}\n",
        metrics.total_positions
    ));
    summary.push_str(&format!(
        "Win Rate: {:.1}% ({} W / {} L)\n",
        metrics.win_rate_bps as f64 / 100.0,
        metrics.winning_positions,
        metrics.losing_positions
    ));
    summary.push_str(&format!(
        "Target Win Rate: 78% ({:+.1}% from target)\n",
        metrics.win_rate_vs_target_bps as f64 / 100.0
    ));
    summary.push_str(&format!(
        "Win/Loss Ratio: {:.2}:1\n",
        metrics.win_loss_ratio as f64 / 100.0
    ));
    
    // PnL
    summary.push_str(&format!(
        "\nNet PnL: ${:.2}\n",
        metrics.net_pnl as f64 / 1_000_000.0
    ));
    summary.push_str(&format!(
        "Profit Factor: {:.2}\n",
        metrics.profit_factor as f64 / 100.0
    ));
    
    // Drawdown
    summary.push_str(&format!(
        "\nCurrent Drawdown: {:.1}% (${:.2})\n",
        metrics.current_drawdown_bps as f64 / 100.0,
        metrics.current_drawdown as f64 / 1_000_000.0
    ));
    summary.push_str(&format!(
        "Max Drawdown: {:.1}% (${:.2})\n",
        metrics.max_drawdown_bps as f64 / 100.0,
        metrics.max_drawdown as f64 / 1_000_000.0
    ));
    
    // Streaks
    summary.push_str(&format!(
        "\nCurrent Streak: {}\n",
        if metrics.current_streak > 0 {
            format!("{}W 🔥", metrics.current_streak)
        } else if metrics.current_streak < 0 {
            format!("{}L 💔", metrics.current_streak.abs())
        } else {
            "None".to_string()
        }
    ));
    
    // Risk metrics
    summary.push_str(&format!(
        "\nRisk Score: {}/100 ",
        metrics.risk_score
    ));
    if metrics.risk_score > 70 {
        summary.push_str("⚠️ HIGH RISK\n");
    } else if metrics.risk_score > 40 {
        summary.push_str("⚡ MODERATE\n");
    } else {
        summary.push_str("✅ LOW RISK\n");
    }
    
    // Consistency
    summary.push_str(&format!(
        "Consistency: {}/100\n",
        metrics.consistency_score
    ));
    
    // Rating
    summary.push_str(&format!(
        "\nPerformance Rating: {:?}\n",
        metrics.get_performance_rating()
    ));
    
    // Leverage breakdown
    summary.push_str("\nLeverage Usage:\n");
    let perf = &metrics.performance_by_leverage;
    if perf.low_leverage_trades > 0 {
        summary.push_str(&format!(
            "  Low (≤10x): {} trades, {:.1}% win rate\n",
            perf.low_leverage_trades,
            (perf.low_leverage_wins * 100) as f64 / perf.low_leverage_trades as f64
        ));
    }
    if perf.extreme_leverage_trades > 0 {
        summary.push_str(&format!(
            "  Extreme (>100x): {} trades, {:.1}% win rate ⚠️\n",
            perf.extreme_leverage_trades,
            (perf.extreme_leverage_wins * 100) as f64 / perf.extreme_leverage_trades as f64
        ));
    }
    
    summary
}

// Event definitions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum MilestoneType {
    Trades100,
    Trades1000,
    ProfitFactor2x,
    WinStreak10,
}

define_event!(PerformanceMilestone, EventType::UserMetricsUpdate, {
    user: Pubkey,
    milestone_type: MilestoneType,
    win_rate_bps: u16,
    profit_factor: u64,
    rating: PerformanceRating,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_tracking() {
        let user = Pubkey::new_unique();
        let mut metrics = UserPerformanceMetrics::new(user);
        
        // Create test position
        let position = Position::new(
            user,
            12345u128,
            67890u128,
            0,
            1_000_000_000, // $1000
            10_000_000, // 10x
            100_000_000, // $100
            true,
            0,
        );
        
        // Simulate winning trade
        metrics.update_position_close(&position, 110_000_000, 1000).unwrap(); // $110 exit
        
        assert_eq!(metrics.winning_positions, 1);
        assert_eq!(metrics.total_positions, 1);
        assert_eq!(metrics.win_rate_bps, 10000); // 100%
        assert!(metrics.net_pnl > 0);
    }

    #[test]
    fn test_streak_tracking() {
        let user = Pubkey::new_unique();
        let mut metrics = UserPerformanceMetrics::new(user);
        
        let position = Position::new(
            user,
            12345u128,
            67890u128,
            0,
            1_000_000_000,
            10_000_000,
            100_000_000,
            true,
            0,
        );
        
        // Win streak
        for _ in 0..3 {
            metrics.update_position_close(&position, 110_000_000, 1000).unwrap();
        }
        assert_eq!(metrics.current_streak, 3);
        assert_eq!(metrics.best_win_streak, 3);
        
        // Break streak with loss
        metrics.update_position_close(&position, 90_000_000, 1000).unwrap();
        assert_eq!(metrics.current_streak, -1);
    }

    #[test]
    fn test_performance_rating() {
        let user = Pubkey::new_unique();
        let mut metrics = UserPerformanceMetrics::new(user);
        
        // Not enough trades
        assert_eq!(metrics.get_performance_rating(), PerformanceRating::Unrated);
        
        // Simulate profitable trader
        metrics.total_positions = 100;
        metrics.winning_positions = 55;
        metrics.profit_factor = 130; // 1.3
        metrics.consistency_score = 65;
        
        assert_eq!(metrics.get_performance_rating(), PerformanceRating::Professional);
    }
}