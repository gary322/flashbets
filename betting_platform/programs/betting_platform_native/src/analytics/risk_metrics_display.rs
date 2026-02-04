//! Risk Metrics Display Module
//! 
//! Provides UI components and display functionality for risk metrics
//! including win rate progress, drawdown visualization, and performance indicators

use solana_program::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    analytics::performance_metrics::UserPerformanceMetrics,
    constants::{TARGET_WIN_RATE_BPS, HIGH_RISK_THRESHOLD, WIN_LOSS_RATIO_TARGET},
};

/// Risk metrics display structure
pub struct RiskMetricsDisplay {
    pub win_rate_display: WinRateDisplay,
    pub drawdown_display: DrawdownDisplay,
    pub ratio_display: WinLossRatioDisplay,
    pub risk_indicator: RiskIndicator,
}

/// Win rate progress display
pub struct WinRateDisplay {
    pub current_rate: f64,
    pub target_rate: f64,
    pub progress_percentage: f64,
    pub status: WinRateStatus,
}

/// Drawdown visualization
pub struct DrawdownDisplay {
    pub current_drawdown_pct: f64,
    pub max_drawdown_pct: f64,
    pub current_drawdown_usd: f64,
    pub severity: DrawdownSeverity,
}

/// Win/Loss ratio display
pub struct WinLossRatioDisplay {
    pub ratio: f64,
    pub target_ratio: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub status: RatioStatus,
}

/// Risk indicator
pub struct RiskIndicator {
    pub risk_score: u8,
    pub risk_level: RiskLevel,
    pub warnings: Vec<String>,
}

/// Win rate status categories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WinRateStatus {
    ExceedingTarget,    // > 78%
    MeetingTarget,      // 75-78%
    ApproachingTarget,  // 70-75%
    BelowTarget,        // 60-70%
    FarBelowTarget,     // < 60%
}

/// Drawdown severity levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawdownSeverity {
    None,       // 0%
    Minor,      // 0-10%
    Moderate,   // 10-25%
    Severe,     // 25-50%
    Critical,   // 50-100%
    Extreme,    // > 100% (including -297% scenario)
}

/// Win/Loss ratio status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RatioStatus {
    Excellent,  // > 2:1
    Good,       // 1.5-2:1
    Acceptable, // 1-1.5:1
    Poor,       // < 1:1
}

/// Risk levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low,        // 0-30
    Moderate,   // 30-50
    High,       // 50-70
    VeryHigh,   // 70-100
}

impl RiskMetricsDisplay {
    /// Create risk metrics display from user performance metrics
    pub fn from_metrics(metrics: &UserPerformanceMetrics) -> Self {
        Self {
            win_rate_display: WinRateDisplay::from_metrics(metrics),
            drawdown_display: DrawdownDisplay::from_metrics(metrics),
            ratio_display: WinLossRatioDisplay::from_metrics(metrics),
            risk_indicator: RiskIndicator::from_metrics(metrics),
        }
    }

    /// Get formatted display string
    pub fn format_display(&self) -> String {
        let mut display = String::from("\n🎯 Risk Metrics Dashboard\n");
        display.push_str("══════════════════════════\n\n");

        // Win Rate Section
        display.push_str(&self.win_rate_display.format_section());
        display.push_str("\n");

        // Drawdown Section
        display.push_str(&self.drawdown_display.format_section());
        display.push_str("\n");

        // Win/Loss Ratio Section
        display.push_str(&self.ratio_display.format_section());
        display.push_str("\n");

        // Risk Indicator Section
        display.push_str(&self.risk_indicator.format_section());

        display
    }
}

impl WinRateDisplay {
    fn from_metrics(metrics: &UserPerformanceMetrics) -> Self {
        let current_rate = metrics.win_rate_bps as f64 / 100.0;
        let target_rate = TARGET_WIN_RATE_BPS as f64 / 100.0;
        let progress_percentage = (current_rate / target_rate) * 100.0;

        let status = match metrics.win_rate_bps {
            rate if rate > TARGET_WIN_RATE_BPS => WinRateStatus::ExceedingTarget,
            rate if rate >= 7500 => WinRateStatus::MeetingTarget,
            rate if rate >= 7000 => WinRateStatus::ApproachingTarget,
            rate if rate >= 6000 => WinRateStatus::BelowTarget,
            _ => WinRateStatus::FarBelowTarget,
        };

        Self {
            current_rate,
            target_rate,
            progress_percentage,
            status,
        }
    }

    fn format_section(&self) -> String {
        let mut section = String::from("📊 Win Rate Progress\n");
        section.push_str(&format!("Current: {:.1}% | Target: {:.1}%\n", self.current_rate, self.target_rate));
        
        // Progress bar
        let bar_length = 20;
        let filled = ((self.progress_percentage.min(100.0) / 100.0) * bar_length as f64) as usize;
        let empty = bar_length - filled;
        section.push_str("[");
        section.push_str(&"█".repeat(filled));
        section.push_str(&"░".repeat(empty));
        section.push_str("] ");
        section.push_str(&format!("{:.0}%\n", self.progress_percentage));

        // Status indicator
        section.push_str("Status: ");
        match self.status {
            WinRateStatus::ExceedingTarget => section.push_str("✨ Exceeding Target!"),
            WinRateStatus::MeetingTarget => section.push_str("✅ Meeting Target"),
            WinRateStatus::ApproachingTarget => section.push_str("📈 Approaching Target"),
            WinRateStatus::BelowTarget => section.push_str("⚡ Below Target"),
            WinRateStatus::FarBelowTarget => section.push_str("⚠️ Far Below Target"),
        }
        section.push_str("\n");

        section
    }
}

impl DrawdownDisplay {
    fn from_metrics(metrics: &UserPerformanceMetrics) -> Self {
        let current_drawdown_pct = metrics.current_drawdown_bps.abs() as f64 / 100.0;
        let max_drawdown_pct = metrics.max_drawdown_bps.abs() as f64 / 100.0;
        let current_drawdown_usd = metrics.current_drawdown as f64 / 1_000_000.0;

        let severity = match metrics.current_drawdown_bps.abs() {
            0 => DrawdownSeverity::None,
            1..=1000 => DrawdownSeverity::Minor,
            1001..=2500 => DrawdownSeverity::Moderate,
            2501..=5000 => DrawdownSeverity::Severe,
            5001..=10000 => DrawdownSeverity::Critical,
            _ => DrawdownSeverity::Extreme,
        };

        Self {
            current_drawdown_pct,
            max_drawdown_pct,
            current_drawdown_usd,
            severity,
        }
    }

    fn format_section(&self) -> String {
        let mut section = String::from("📉 Drawdown Analysis\n");
        section.push_str(&format!(
            "Current: -{:.1}% (${:.2}) | Max: -{:.1}%\n",
            self.current_drawdown_pct,
            self.current_drawdown_usd,
            self.max_drawdown_pct
        ));

        // Severity indicator
        section.push_str("Severity: ");
        match self.severity {
            DrawdownSeverity::None => section.push_str("✅ No Drawdown"),
            DrawdownSeverity::Minor => section.push_str("💚 Minor"),
            DrawdownSeverity::Moderate => section.push_str("💛 Moderate"),
            DrawdownSeverity::Severe => section.push_str("🟠 Severe"),
            DrawdownSeverity::Critical => section.push_str("🔴 Critical"),
            DrawdownSeverity::Extreme => section.push_str("💀 EXTREME (-297% SCENARIO)"),
        }
        section.push_str("\n");

        // Warning for extreme drawdowns
        if self.current_drawdown_pct >= 100.0 {
            section.push_str("⚠️ WARNING: Position at risk of liquidation!\n");
        }

        section
    }
}

impl WinLossRatioDisplay {
    fn from_metrics(metrics: &UserPerformanceMetrics) -> Self {
        let ratio = metrics.win_loss_ratio as f64 / 100.0;
        let target_ratio = WIN_LOSS_RATIO_TARGET as f64 / 100.0;
        let avg_win = metrics.avg_win_size as f64 / 1_000_000.0;
        let avg_loss = metrics.avg_loss_size as f64 / 1_000_000.0;

        let status = match metrics.win_loss_ratio {
            r if r >= 200 => RatioStatus::Excellent,
            r if r >= 150 => RatioStatus::Good,
            r if r >= 100 => RatioStatus::Acceptable,
            _ => RatioStatus::Poor,
        };

        Self {
            ratio,
            target_ratio,
            avg_win,
            avg_loss,
            status,
        }
    }

    fn format_section(&self) -> String {
        let mut section = String::from("💰 Win/Loss Ratio\n");
        section.push_str(&format!(
            "Ratio: {:.2}:1 | Target: {:.2}:1\n",
            self.ratio,
            self.target_ratio
        ));
        section.push_str(&format!(
            "Avg Win: ${:.2} | Avg Loss: ${:.2}\n",
            self.avg_win,
            self.avg_loss
        ));

        // Status
        section.push_str("Performance: ");
        match self.status {
            RatioStatus::Excellent => section.push_str("🌟 Excellent"),
            RatioStatus::Good => section.push_str("✅ Good"),
            RatioStatus::Acceptable => section.push_str("👍 Acceptable"),
            RatioStatus::Poor => section.push_str("⚠️ Needs Improvement"),
        }
        section.push_str("\n");

        section
    }
}

impl RiskIndicator {
    fn from_metrics(metrics: &UserPerformanceMetrics) -> Self {
        let risk_level = match metrics.risk_score {
            0..=30 => RiskLevel::Low,
            31..=50 => RiskLevel::Moderate,
            51..=70 => RiskLevel::High,
            _ => RiskLevel::VeryHigh,
        };

        let mut warnings = Vec::new();

        // Check various risk factors
        if metrics.risk_score > HIGH_RISK_THRESHOLD {
            warnings.push("High risk score detected".to_string());
        }

        if metrics.liquidated_positions > metrics.total_positions / 10 {
            warnings.push("High liquidation rate".to_string());
        }

        if metrics.current_drawdown_bps < -5000 {
            warnings.push("Significant drawdown active".to_string());
        }

        if metrics.worst_loss_streak > 10 {
            warnings.push("Extended loss streak history".to_string());
        }

        if metrics.win_rate_bps < 4000 {
            warnings.push("Win rate below 40%".to_string());
        }

        Self {
            risk_score: metrics.risk_score,
            risk_level,
            warnings,
        }
    }

    fn format_section(&self) -> String {
        let mut section = String::from("⚡ Risk Assessment\n");
        section.push_str(&format!("Risk Score: {}/100\n", self.risk_score));
        
        section.push_str("Risk Level: ");
        match self.risk_level {
            RiskLevel::Low => section.push_str("🟢 Low Risk"),
            RiskLevel::Moderate => section.push_str("🟡 Moderate Risk"),
            RiskLevel::High => section.push_str("🟠 High Risk"),
            RiskLevel::VeryHigh => section.push_str("🔴 Very High Risk"),
        }
        section.push_str("\n");

        // Display warnings
        if !self.warnings.is_empty() {
            section.push_str("\n⚠️ Risk Warnings:\n");
            for warning in &self.warnings {
                section.push_str(&format!("  • {}\n", warning));
            }
        }

        section
    }
}

/// Display risk metrics for a user
pub fn display_user_risk_metrics(
    user_metrics: &UserPerformanceMetrics,
) -> Result<(), ProgramError> {
    let display = RiskMetricsDisplay::from_metrics(user_metrics);
    let formatted = display.format_display();
    
    msg!("{}", formatted);
    
    // Log specific alerts for extreme conditions
    if user_metrics.current_drawdown_bps <= -10000 {
        msg!("🚨 ALERT: User experiencing extreme drawdown of {}%", 
             user_metrics.current_drawdown_bps.abs() as f64 / 100.0);
    }
    
    if user_metrics.risk_score >= HIGH_RISK_THRESHOLD {
        msg!("⚠️ WARNING: User risk score {} exceeds high risk threshold", 
             user_metrics.risk_score);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::accounts::discriminators;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_win_rate_display() {
        let mut metrics = UserPerformanceMetrics::new(Pubkey::new_unique());
        metrics.win_rate_bps = 7500; // 75%
        metrics.win_rate_vs_target_bps = -300; // 3% below target

        let display = WinRateDisplay::from_metrics(&metrics);
        assert_eq!(display.current_rate, 75.0);
        assert_eq!(display.status, WinRateStatus::MeetingTarget);
    }

    #[test]
    fn test_drawdown_severity() {
        let mut metrics = UserPerformanceMetrics::new(Pubkey::new_unique());
        
        // Test extreme drawdown
        metrics.current_drawdown_bps = -29700; // -297%
        let display = DrawdownDisplay::from_metrics(&metrics);
        assert_eq!(display.severity, DrawdownSeverity::Extreme);

        // Test moderate drawdown
        metrics.current_drawdown_bps = -2000; // -20%
        let display = DrawdownDisplay::from_metrics(&metrics);
        assert_eq!(display.severity, DrawdownSeverity::Moderate);
    }

    #[test]
    fn test_risk_warnings() {
        let mut metrics = UserPerformanceMetrics::new(Pubkey::new_unique());
        metrics.risk_score = 75;
        metrics.win_rate_bps = 3500; // 35%
        metrics.worst_loss_streak = 15;
        metrics.total_positions = 100;
        metrics.liquidated_positions = 15;

        let indicator = RiskIndicator::from_metrics(&metrics);
        assert_eq!(indicator.risk_level, RiskLevel::VeryHigh);
        assert!(indicator.warnings.len() >= 3);
    }
}