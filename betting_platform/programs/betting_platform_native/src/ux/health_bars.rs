//! Real-time Position Health Monitoring
//! 
//! Provides visual health bars and liquidation alerts for positions,
//! helping users understand their risk exposure in real-time.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    constants::{LEVERAGE_PRECISION, BASIS_POINTS_DIVISOR},
    error::BettingPlatformError,
    math::U64F64,
    state::accounts::{Position, GlobalConfigPDA, ProposalPDA},
    events::{EventType, Event},
    define_event,
};

/// Health status levels
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// > 50% margin ratio (very safe)
    Excellent,
    /// 20-50% margin ratio (safe)
    Good,
    /// 10-20% margin ratio (caution)
    Fair,
    /// 5-10% margin ratio (warning)
    Poor,
    /// 2-5% margin ratio (critical)
    Critical,
    /// < 2% margin ratio (imminent liquidation)
    Danger,
}

impl HealthStatus {
    pub fn from_margin_ratio(ratio_bps: u64) -> Self {
        match ratio_bps {
            5000..=u64::MAX => HealthStatus::Excellent, // > 50%
            2000..=4999 => HealthStatus::Good,         // 20-50%
            1000..=1999 => HealthStatus::Fair,         // 10-20%
            500..=999 => HealthStatus::Poor,           // 5-10%
            200..=499 => HealthStatus::Critical,       // 2-5%
            _ => HealthStatus::Danger,                  // < 2%
        }
    }

    pub fn to_emoji(&self) -> &'static str {
        match self {
            HealthStatus::Excellent => "💚",
            HealthStatus::Good => "🟢",
            HealthStatus::Fair => "🟡",
            HealthStatus::Poor => "🟠",
            HealthStatus::Critical => "🔴",
            HealthStatus::Danger => "💀",
        }
    }

    pub fn to_color(&self) -> &'static str {
        match self {
            HealthStatus::Excellent => "#00FF00",
            HealthStatus::Good => "#7FFF00",
            HealthStatus::Fair => "#FFFF00",
            HealthStatus::Poor => "#FFA500",
            HealthStatus::Critical => "#FF0000",
            HealthStatus::Danger => "#8B0000",
        }
    }

    pub fn get_alert_message(&self) -> Option<&'static str> {
        match self {
            HealthStatus::Poor => Some("⚠️ Warning: Position health is poor. Consider reducing leverage."),
            HealthStatus::Critical => Some("🚨 CRITICAL: Position at risk of liquidation! Add collateral NOW!"),
            HealthStatus::Danger => Some("💀 DANGER: Liq in 10s! Emergency action required!"),
            _ => None,
        }
    }
}

/// Position health data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionHealth {
    /// Position ID
    pub position_id: [u8; 32],
    /// Current health status
    pub status: HealthStatus,
    /// Margin ratio in basis points
    pub margin_ratio_bps: u64,
    /// Distance to liquidation in basis points
    pub distance_to_liquidation_bps: u64,
    /// Time to liquidation at current funding rate (slots)
    pub time_to_liquidation: Option<u64>,
    /// Current collateral
    pub current_collateral: u64,
    /// Required collateral for safety (10% margin)
    pub safe_collateral_required: u64,
    /// Unrealized PnL
    pub unrealized_pnl: i64,
    /// Health score (0-100)
    pub health_score: u8,
    /// Last update timestamp
    pub last_update: i64,
}

impl PositionHealth {
    /// Calculate health metrics for a position
    pub fn calculate(
        position: &Position,
        current_price: u64,
        funding_rate: i64,
    ) -> Result<Self, ProgramError> {
        // Calculate margin ratio
        let margin_ratio = position.get_margin_ratio(current_price)?;
        let margin_ratio_bps = (margin_ratio * U64F64::from_num(BASIS_POINTS_DIVISOR)).to_num();

        // Calculate distance to liquidation
        let distance_to_liquidation_bps = calculate_distance_to_liquidation(
            position,
            current_price,
        )?;

        // Calculate time to liquidation if funding rate is negative
        let time_to_liquidation = if funding_rate < 0 {
            calculate_time_to_liquidation(
                position.collateral,
                funding_rate.abs() as u64,
                position.leverage,
            )
        } else {
            None
        };

        // Calculate safe collateral (10% margin ratio)
        let safe_collateral_required = position.size / 10;

        // Calculate health score (0-100)
        let health_score = calculate_health_score(margin_ratio_bps);

        // Determine status
        let status = HealthStatus::from_margin_ratio(margin_ratio_bps);

        Ok(PositionHealth {
            position_id: position.position_id,
            status,
            margin_ratio_bps,
            distance_to_liquidation_bps,
            time_to_liquidation,
            current_collateral: position.collateral,
            safe_collateral_required,
            unrealized_pnl: position.unrealized_pnl,
            health_score,
            last_update: Clock::get()?.unix_timestamp,
        })
    }

    /// Get health bar representation (for UI)
    pub fn get_health_bar(&self) -> String {
        let filled = (self.health_score / 10) as usize;
        let empty = 10 - filled;
        
        // Use red bars for critical/danger status
        let bar_char = match self.status {
            HealthStatus::Critical | HealthStatus::Danger => "🟥",
            _ => "█",
        };
        
        let alert_suffix = match self.status {
            HealthStatus::Danger => " Liq in 10s!",
            HealthStatus::Critical => " Add collateral!",
            _ => "",
        };
        
        format!(
            "{} [{}{}] {}%{}",
            self.status.to_emoji(),
            bar_char.repeat(filled),
            "░".repeat(empty),
            self.health_score,
            alert_suffix
        )
    }

    /// Check if position needs alert
    pub fn needs_alert(&self) -> bool {
        matches!(
            self.status,
            HealthStatus::Poor | HealthStatus::Critical | HealthStatus::Danger
        )
    }

    /// Get recommended action based on health
    pub fn get_recommendation(&self) -> &'static str {
        match self.status {
            HealthStatus::Excellent | HealthStatus::Good => "Position healthy. No action needed.",
            HealthStatus::Fair => "Monitor position closely.",
            HealthStatus::Poor => "Consider adding collateral or reducing position size.",
            HealthStatus::Critical => "Add collateral immediately or close position.",
            HealthStatus::Danger => "IMMEDIATE ACTION REQUIRED: Add collateral or position will be liquidated!",
        }
    }
    
    /// Get time to liquidation message
    pub fn get_time_to_liquidation_message(&self) -> String {
        match self.time_to_liquidation {
            Some(slots) if slots < 20 => format!("🚨 Liq in {}s!", slots / 2), // ~2 slots per second
            Some(slots) if slots < 120 => format!("⚠️ Liquidation in {} seconds", slots / 2),
            Some(slots) => format!("Time to liquidation: {} minutes", slots / 120),
            None => "No liquidation risk from funding".to_string(),
        }
    }
}

/// Calculate distance to liquidation in basis points
fn calculate_distance_to_liquidation(
    position: &Position,
    current_price: u64,
) -> Result<u64, ProgramError> {
    let liquidation_price = position.liquidation_price;
    
    let distance = if position.is_long {
        // For long: (current - liquidation) / current
        if current_price > liquidation_price {
            ((current_price - liquidation_price) as u128 * BASIS_POINTS_DIVISOR as u128
                / current_price as u128) as u64
        } else {
            0 // Already past liquidation
        }
    } else {
        // For short: (liquidation - current) / current
        if liquidation_price > current_price {
            ((liquidation_price - current_price) as u128 * BASIS_POINTS_DIVISOR as u128
                / current_price as u128) as u64
        } else {
            0 // Already past liquidation
        }
    };

    Ok(distance)
}

/// Calculate time to liquidation based on funding rate
fn calculate_time_to_liquidation(
    collateral: u64,
    funding_rate_per_slot: u64,
    leverage: u64,
) -> Option<u64> {
    // Funding cost per slot = position_size * funding_rate * leverage
    let funding_cost_per_slot = (collateral as u128 * leverage as u128 * funding_rate_per_slot as u128
        / LEVERAGE_PRECISION as u128 / BASIS_POINTS_DIVISOR as u128) as u64;

    if funding_cost_per_slot == 0 {
        return None;
    }

    // Time to liquidation = collateral / funding_cost_per_slot
    Some(collateral / funding_cost_per_slot)
}

/// Calculate health score (0-100) based on margin ratio
fn calculate_health_score(margin_ratio_bps: u64) -> u8 {
    match margin_ratio_bps {
        0..=199 => 0,       // < 2%: 0
        200..=499 => 10,    // 2-5%: 10
        500..=999 => 25,    // 5-10%: 25
        1000..=1999 => 50,  // 10-20%: 50
        2000..=4999 => 75,  // 20-50%: 75
        _ => 100,           // > 50%: 100
    }
}

/// Health monitoring configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct HealthMonitoringConfig {
    /// Enable automatic alerts
    pub auto_alerts_enabled: bool,
    /// Alert threshold (health score)
    pub alert_threshold: u8,
    /// Cooldown between alerts (slots)
    pub alert_cooldown: u64,
    /// Last alert slot
    pub last_alert_slot: u64,
}

impl Default for HealthMonitoringConfig {
    fn default() -> Self {
        Self {
            auto_alerts_enabled: true,
            alert_threshold: 25, // Alert when health score < 25
            alert_cooldown: 3600, // 1 hour between alerts
            last_alert_slot: 0,
        }
    }
}

/// Process health check for a position
pub fn process_health_check(
    position_account: &AccountInfo,
    proposal_account: &AccountInfo,
    user_account: &AccountInfo,
) -> ProgramResult {
    // Deserialize accounts
    let position = Position::try_from_slice(&position_account.data.borrow())?;
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;

    // Validate position ownership
    if position.user != *user_account.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }

    // Get current price from proposal
    let outcome_idx = position.outcome as usize;
    if outcome_idx >= proposal.prices.len() {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    let current_price = proposal.prices[outcome_idx];

    // Calculate health
    let health = PositionHealth::calculate(
        &position,
        current_price,
        proposal.funding_state.current_funding_rate_bps,
    )?;

    // Log health status
    msg!(
        "Position Health: {} | Score: {} | Margin: {}% | Distance to Liq: {}%",
        health.get_health_bar(),
        health.health_score,
        health.margin_ratio_bps / 100,
        health.distance_to_liquidation_bps / 100
    );

    // Show time to liquidation if applicable
    msg!("{}", health.get_time_to_liquidation_message());

    // Show alert if needed
    if let Some(alert) = health.status.get_alert_message() {
        msg!("{}", alert);
    }

    // Show recommendation
    msg!("Recommendation: {}", health.get_recommendation());

    // Emit event if health is poor
    if health.needs_alert() {
        let event = PositionHealthAlert {
            user: *user_account.key,
            position_id: position.position_id,
            health_status: health.status,
            margin_ratio_bps: health.margin_ratio_bps,
            distance_to_liquidation_bps: health.distance_to_liquidation_bps,
            timestamp: health.last_update,
        };
        event.emit();
    }

    Ok(())
}

/// Batch health check for multiple positions
pub fn batch_health_check(
    positions: &[Position],
    proposals: &[ProposalPDA],
) -> Result<Vec<PositionHealth>, ProgramError> {
    let mut health_results = Vec::new();

    for (i, position) in positions.iter().enumerate() {
        if i >= proposals.len() {
            break;
        }

        let proposal = &proposals[i];
        let outcome_idx = position.outcome as usize;
        
        if outcome_idx < proposal.prices.len() {
            let current_price = proposal.prices[outcome_idx];
            let health = PositionHealth::calculate(
                position,
                current_price,
                proposal.funding_state.current_funding_rate_bps,
            )?;
            
            health_results.push(health);
        }
    }

    Ok(health_results)
}

/// Format health summary for display
pub fn format_health_summary(healths: &[PositionHealth]) -> String {
    let mut summary = String::from("📊 Portfolio Health Summary\n");
    summary.push_str("─────────────────────────\n");

    let mut danger_count = 0;
    let mut critical_count = 0;
    let mut poor_count = 0;

    for health in healths {
        match health.status {
            HealthStatus::Danger => danger_count += 1,
            HealthStatus::Critical => critical_count += 1,
            HealthStatus::Poor => poor_count += 1,
            _ => {}
        }
    }

    if danger_count > 0 {
        summary.push_str(&format!("💀 {} positions in DANGER\n", danger_count));
    }
    if critical_count > 0 {
        summary.push_str(&format!("🔴 {} positions CRITICAL\n", critical_count));
    }
    if poor_count > 0 {
        summary.push_str(&format!("🟠 {} positions need attention\n", poor_count));
    }

    let avg_health = healths.iter()
        .map(|h| h.health_score as u32)
        .sum::<u32>() / healths.len() as u32;

    summary.push_str(&format!("\nAverage Health Score: {}/100", avg_health));

    summary
}

// Event definitions
define_event!(PositionHealthAlert, EventType::HealthAlert, {
    user: Pubkey,
    position_id: [u8; 32],
    health_status: HealthStatus,
    margin_ratio_bps: u64,
    distance_to_liquidation_bps: u64,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_levels() {
        assert_eq!(HealthStatus::from_margin_ratio(6000), HealthStatus::Excellent);
        assert_eq!(HealthStatus::from_margin_ratio(3000), HealthStatus::Good);
        assert_eq!(HealthStatus::from_margin_ratio(1500), HealthStatus::Fair);
        assert_eq!(HealthStatus::from_margin_ratio(700), HealthStatus::Poor);
        assert_eq!(HealthStatus::from_margin_ratio(300), HealthStatus::Critical);
        assert_eq!(HealthStatus::from_margin_ratio(100), HealthStatus::Danger);
    }

    #[test]
    fn test_health_score_calculation() {
        assert_eq!(calculate_health_score(100), 0);    // 1% margin
        assert_eq!(calculate_health_score(300), 10);   // 3% margin
        assert_eq!(calculate_health_score(700), 25);   // 7% margin
        assert_eq!(calculate_health_score(1500), 50);  // 15% margin
        assert_eq!(calculate_health_score(3000), 75);  // 30% margin
        assert_eq!(calculate_health_score(6000), 100); // 60% margin
    }

    #[test]
    fn test_health_bar_generation() {
        let health = PositionHealth {
            position_id: [0u8; 32],
            status: HealthStatus::Good,
            margin_ratio_bps: 3000,
            distance_to_liquidation_bps: 2500,
            time_to_liquidation: None,
            current_collateral: 1000,
            safe_collateral_required: 2000,
            unrealized_pnl: 100,
            health_score: 75,
            last_update: 0,
        };

        let bar = health.get_health_bar();
        assert!(bar.contains("🟢"));
        assert!(bar.contains("75%"));
        assert!(bar.contains("███████"));
        
        // Test danger status with red bars
        let danger_health = PositionHealth {
            position_id: [0u8; 32],
            status: HealthStatus::Danger,
            margin_ratio_bps: 100,
            distance_to_liquidation_bps: 50,
            time_to_liquidation: Some(20),
            current_collateral: 100,
            safe_collateral_required: 1000,
            unrealized_pnl: -900,
            health_score: 0,
            last_update: 0,
        };
        
        let danger_bar = danger_health.get_health_bar();
        assert!(danger_bar.contains("💀"));
        assert!(danger_bar.contains("Liq in 10s!"));
        assert!(danger_bar.contains("0%"));
    }

    #[test]
    fn test_alert_messages() {
        assert!(HealthStatus::Excellent.get_alert_message().is_none());
        assert!(HealthStatus::Good.get_alert_message().is_none());
        assert!(HealthStatus::Fair.get_alert_message().is_none());
        assert!(HealthStatus::Poor.get_alert_message().is_some());
        assert!(HealthStatus::Critical.get_alert_message().is_some());
        assert!(HealthStatus::Danger.get_alert_message().is_some());
    }
}