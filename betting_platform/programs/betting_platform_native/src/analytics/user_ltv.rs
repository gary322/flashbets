//! User Lifetime Value (LTV) Tracking
//! 
//! Implements comprehensive LTV metrics targeting $500 per user as specified
//! in sections 45-50 of the specification.

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
    constants::BASIS_POINTS_DIVISOR,
    error::BettingPlatformError,
    state::accounts::{UserStatsPDA, Position, discriminators},
    events::{EventType, Event},
    define_event,
};

/// Target LTV per user in USD (scaled by 1e6)
pub const TARGET_LTV_USD: u64 = 550_000_000; // $550 (mid-point of $500-600 target)

/// User LTV tracking structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UserLTVMetrics {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Total revenue generated (fees + liquidations)
    pub total_revenue_generated: u64,
    
    /// Total MMT rewards earned
    pub total_mmt_earned: u64,
    
    /// Total referral rewards earned
    pub total_referral_rewards: u64,
    
    /// First activity timestamp
    pub first_activity: i64,
    
    /// Days active (unique days with activity)
    pub days_active: u32,
    
    /// Average position size
    pub avg_position_size: u64,
    
    /// Average leverage used
    pub avg_leverage: u64,
    
    /// Total deposits
    pub total_deposits: u64,
    
    /// Total withdrawals
    pub total_withdrawals: u64,
    
    /// Net deposits (deposits - withdrawals)
    pub net_deposits: i64,
    
    /// Current LTV estimate in USD
    pub current_ltv_usd: u64,
    
    /// LTV growth rate (basis points per day)
    pub ltv_growth_rate_bps: u16,
    
    /// User segment
    pub user_segment: UserSegment,
    
    /// Retention score (0-100)
    pub retention_score: u8,
    
    /// Churn risk (0-100, higher = more likely to churn)
    pub churn_risk: u8,
    
    /// Last LTV calculation timestamp
    pub last_calculation: i64,
    
    /// Number of unique markets traded
    pub unique_markets_traded: u32,
    
    /// Preferred trading hours (0-23)
    pub preferred_trading_hour: u8,
    
    /// Chain usage count
    pub chain_positions_count: u32,
    
    /// Migration bonus received
    pub migration_bonus_received: u64,
}

impl UserLTVMetrics {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        8 + // total_revenue_generated
        8 + // total_mmt_earned
        8 + // total_referral_rewards
        8 + // first_activity
        4 + // days_active
        8 + // avg_position_size
        8 + // avg_leverage
        8 + // total_deposits
        8 + // total_withdrawals
        8 + // net_deposits
        8 + // current_ltv_usd
        2 + // ltv_growth_rate_bps
        1 + // user_segment
        1 + // retention_score
        1 + // churn_risk
        8 + // last_calculation
        4 + // unique_markets_traded
        1 + // preferred_trading_hour
        4 + // chain_positions_count
        8; // migration_bonus_received

    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_STATS, // Reuse existing discriminator
            user,
            total_revenue_generated: 0,
            total_mmt_earned: 0,
            total_referral_rewards: 0,
            first_activity: Clock::get().unwrap_or_default().unix_timestamp,
            days_active: 1,
            avg_position_size: 0,
            avg_leverage: 0,
            total_deposits: 0,
            total_withdrawals: 0,
            net_deposits: 0,
            current_ltv_usd: 0,
            ltv_growth_rate_bps: 0,
            user_segment: UserSegment::New,
            retention_score: 50,
            churn_risk: 50,
            last_calculation: Clock::get().unwrap_or_default().unix_timestamp,
            unique_markets_traded: 0,
            preferred_trading_hour: 0,
            chain_positions_count: 0,
            migration_bonus_received: 0,
        }
    }

    /// Calculate current LTV based on historical data and projections
    pub fn calculate_ltv(&mut self, user_stats: &UserStatsPDA) -> Result<u64, ProgramError> {
        let clock = Clock::get()?;
        let days_since_start = ((clock.unix_timestamp - self.first_activity) / 86400).max(1) as u64;
        
        // Revenue components
        let fee_revenue = user_stats.total_fees;
        let liquidation_revenue = (user_stats.liquidation_count as u64)
            .checked_mul(1_000_000) // Avg $1 per liquidation
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Current realized revenue
        let realized_revenue = fee_revenue
            .checked_add(liquidation_revenue)
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Calculate daily revenue rate
        let daily_revenue = realized_revenue
            .checked_div(days_since_start)
            .unwrap_or(0);
        
        // Project future revenue based on retention curve
        let projected_days = self.estimate_remaining_lifetime()?;
        let decay_factor = self.calculate_revenue_decay_factor()?;
        
        // Future revenue = daily_revenue * projected_days * decay_factor
        let future_revenue = daily_revenue
            .checked_mul(projected_days)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_mul(decay_factor)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(100) // decay_factor is in percentage
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Total LTV = realized + projected future
        let total_ltv = realized_revenue
            .checked_add(future_revenue)
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Update metrics
        self.total_revenue_generated = realized_revenue;
        self.current_ltv_usd = total_ltv;
        self.last_calculation = clock.unix_timestamp;
        
        // Calculate growth rate
        self.update_ltv_growth_rate(total_ltv, days_since_start)?;
        
        // Update user segment
        self.update_user_segment(total_ltv)?;
        
        Ok(total_ltv)
    }

    /// Estimate remaining lifetime in days based on user behavior
    fn estimate_remaining_lifetime(&self) -> Result<u64, ProgramError> {
        // Base lifetime estimate
        let mut lifetime_days = match self.user_segment {
            UserSegment::New => 30,
            UserSegment::Active => 90,
            UserSegment::Power => 365,
            UserSegment::VIP => 730,
            UserSegment::Whale => 1095,
            UserSegment::Dormant => 7,
            UserSegment::Churned => 0,
        };
        
        // Adjust based on retention score
        lifetime_days = (lifetime_days as u128)
            .checked_mul(self.retention_score as u128)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(50) // Normalize around 50
            .ok_or(BettingPlatformError::MathOverflow)? as u64;
        
        // Adjust based on chain usage (chains increase retention)
        if self.chain_positions_count > 0 {
            lifetime_days = lifetime_days
                .checked_mul(150)
                .ok_or(BettingPlatformError::MathOverflow)?
                .checked_div(100)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }
        
        Ok(lifetime_days)
    }

    /// Calculate revenue decay factor (how revenue decreases over time)
    fn calculate_revenue_decay_factor(&self) -> Result<u64, ProgramError> {
        // Start with base decay based on segment
        let base_decay: u64 = match self.user_segment {
            UserSegment::Whale => 90, // 90% revenue retention
            UserSegment::VIP => 80,
            UserSegment::Power => 70,
            UserSegment::Active => 60,
            UserSegment::New => 50,
            UserSegment::Dormant => 20,
            UserSegment::Churned => 0,
        };
        
        // Adjust for churn risk
        let adjusted_decay = base_decay
            .saturating_sub((self.churn_risk / 2) as u64);
        
        Ok(adjusted_decay)
    }

    /// Update LTV growth rate
    fn update_ltv_growth_rate(&mut self, current_ltv: u64, days: u64) -> Result<(), ProgramError> {
        if days == 0 {
            return Ok(());
        }
        
        // Daily LTV
        let daily_ltv = current_ltv
            .checked_div(days)
            .unwrap_or(0);
        
        // Growth rate in basis points (1 bp = 0.01%)
        // If daily LTV is $1, and target is $500 over 500 days, that's 100% = 10000 bps
        let growth_rate = daily_ltv
            .checked_mul(10000) // Convert to bps
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(TARGET_LTV_USD / 500) // Daily target
            .unwrap_or(0);
        
        self.ltv_growth_rate_bps = growth_rate.min(u16::MAX as u64) as u16;
        Ok(())
    }

    /// Update user segment based on LTV and behavior
    fn update_user_segment(&mut self, ltv: u64) -> Result<(), ProgramError> {
        self.user_segment = if ltv >= TARGET_LTV_USD * 10 {
            UserSegment::Whale // $5000+ LTV
        } else if ltv >= TARGET_LTV_USD * 4 {
            UserSegment::VIP // $2000+ LTV
        } else if ltv >= TARGET_LTV_USD * 2 {
            UserSegment::Power // $1000+ LTV
        } else if ltv >= TARGET_LTV_USD / 2 {
            UserSegment::Active // $250+ LTV
        } else if self.days_active == 1 {
            UserSegment::New
        } else if self.churn_risk > 80 {
            UserSegment::Dormant
        } else {
            UserSegment::Active
        };
        
        Ok(())
    }

    /// Update retention and churn scores
    pub fn update_retention_metrics(
        &mut self,
        days_since_last_activity: i64,
        activity_frequency: f32,
    ) -> Result<(), ProgramError> {
        // Retention score calculation
        let base_retention = 50u8;
        
        // Boost for recent activity
        let recency_boost = match days_since_last_activity {
            0..=1 => 30,
            2..=7 => 20,
            8..=30 => 10,
            _ => 0,
        };
        
        // Boost for high frequency
        let frequency_boost = (activity_frequency * 20.0) as u8;
        
        // Boost for chain usage
        let chain_boost = if self.chain_positions_count > 0 { 10 } else { 0 };
        
        self.retention_score = (base_retention + recency_boost + frequency_boost + chain_boost).min(100);
        
        // Churn risk calculation (inverse of retention)
        self.churn_risk = 100u8.saturating_sub(self.retention_score);
        
        // Additional churn risk factors
        if self.net_deposits < 0 {
            self.churn_risk = self.churn_risk.saturating_add(20);
        }
        
        if days_since_last_activity > 30 {
            self.churn_risk = self.churn_risk.saturating_add(30);
        }
        
        self.churn_risk = self.churn_risk.min(100);
        
        Ok(())
    }

    /// Check if user is approaching LTV target
    pub fn is_approaching_target(&self) -> bool {
        self.current_ltv_usd >= (TARGET_LTV_USD * 80 / 100) // 80% of target
    }

    /// Get personalized incentives based on LTV
    pub fn get_ltv_incentives(&self) -> LTVIncentives {
        if self.current_ltv_usd >= TARGET_LTV_USD {
            // Achieved target - retention incentives
            LTVIncentives {
                bonus_mmt_multiplier: 150, // 1.5x MMT
                fee_discount_bps: 10, // 10bp discount
                chain_bonus_bps: 20, // Extra 20bp for chains
                vip_perks: true,
            }
        } else if self.is_approaching_target() {
            // Close to target - growth incentives
            LTVIncentives {
                bonus_mmt_multiplier: 125, // 1.25x MMT
                fee_discount_bps: 5,
                chain_bonus_bps: 15,
                vip_perks: false,
            }
        } else {
            // Growth phase - acquisition incentives
            LTVIncentives {
                bonus_mmt_multiplier: 110, // 1.1x MMT
                fee_discount_bps: 0,
                chain_bonus_bps: 10,
                vip_perks: false,
            }
        }
    }
}

/// User segments for LTV analysis
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum UserSegment {
    New,       // < 7 days
    Active,    // Regular trader
    Power,     // High volume/frequency
    VIP,       // High LTV achieved
    Whale,     // Ultra high value
    Dormant,   // Inactive but not churned
    Churned,   // Lost user
}

/// LTV-based incentives
#[derive(Debug, Clone)]
pub struct LTVIncentives {
    pub bonus_mmt_multiplier: u16, // 100 = 1x, 150 = 1.5x
    pub fee_discount_bps: u16,
    pub chain_bonus_bps: u16,
    pub vip_perks: bool,
}

/// Process LTV update for a user
pub fn process_update_user_ltv(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let ltv_metrics_account = next_account_info(account_info_iter)?;
    let user_stats_account = next_account_info(account_info_iter)?;
    
    // Load or create LTV metrics
    let mut ltv_metrics = if ltv_metrics_account.data_is_empty() {
        UserLTVMetrics::new(*user.key)
    } else {
        UserLTVMetrics::try_from_slice(&ltv_metrics_account.data.borrow())?
    };
    
    // Load user stats
    let user_stats = UserStatsPDA::try_from_slice(&user_stats_account.data.borrow())?;
    
    // Verify ownership
    if user_stats.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Calculate updated LTV
    let new_ltv = ltv_metrics.calculate_ltv(&user_stats)?;
    
    msg!(
        "Updated LTV for user {}: ${} ({}% of target)",
        user.key,
        new_ltv / 1_000_000,
        (new_ltv * 100) / TARGET_LTV_USD
    );
    
    // Check if approaching or exceeded target
    if ltv_metrics.is_approaching_target() {
        msg!("User approaching LTV target! Applying retention incentives");
        
        // Emit event for approaching target
        let event = UserApproachingLTVTarget {
            user: *user.key,
            current_ltv: new_ltv,
            target_ltv: TARGET_LTV_USD,
            percentage: ((new_ltv * 100) / TARGET_LTV_USD) as u8,
            timestamp: Clock::get()?.unix_timestamp,
        };
        event.emit();
    }
    
    // Save updated metrics
    ltv_metrics.serialize(&mut &mut ltv_metrics_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Get LTV summary for display
pub fn get_ltv_summary(metrics: &UserLTVMetrics) -> String {
    let mut summary = String::from("📊 Lifetime Value Summary\n");
    summary.push_str("══════════════════════════\n\n");
    
    // Current LTV
    summary.push_str(&format!(
        "Current LTV: ${:.2} ({:.1}% of target)\n",
        metrics.current_ltv_usd as f64 / 1_000_000.0,
        (metrics.current_ltv_usd as f64 * 100.0) / TARGET_LTV_USD as f64
    ));
    
    // Growth rate
    summary.push_str(&format!(
        "Growth Rate: {:.2}% daily\n",
        metrics.ltv_growth_rate_bps as f64 / 100.0
    ));
    
    // User segment
    summary.push_str(&format!(
        "Segment: {:?}\n",
        metrics.user_segment
    ));
    
    // Retention metrics
    summary.push_str(&format!(
        "\nRetention Score: {}/100\n",
        metrics.retention_score
    ));
    summary.push_str(&format!(
        "Churn Risk: {}% ",
        metrics.churn_risk
    ));
    
    if metrics.churn_risk > 70 {
        summary.push_str("⚠️ HIGH RISK\n");
    } else if metrics.churn_risk > 40 {
        summary.push_str("⚡ MEDIUM RISK\n");
    } else {
        summary.push_str("✅ LOW RISK\n");
    }
    
    // Revenue breakdown
    summary.push_str(&format!(
        "\nTotal Revenue Generated: ${:.2}\n",
        metrics.total_revenue_generated as f64 / 1_000_000.0
    ));
    
    // Days active
    summary.push_str(&format!(
        "Days Active: {}\n",
        metrics.days_active
    ));
    
    // Incentives
    let incentives = metrics.get_ltv_incentives();
    if incentives.vip_perks {
        summary.push_str("\n🌟 VIP Status Unlocked!\n");
    }
    summary.push_str(&format!(
        "MMT Bonus: {}%\n",
        incentives.bonus_mmt_multiplier - 100
    ));
    if incentives.fee_discount_bps > 0 {
        summary.push_str(&format!(
            "Fee Discount: -{} bps\n",
            incentives.fee_discount_bps
        ));
    }
    
    summary
}

// Event definitions
define_event!(UserApproachingLTVTarget, EventType::UserMetricsUpdate, {
    user: Pubkey,
    current_ltv: u64,
    target_ltv: u64,
    percentage: u8,
    timestamp: i64,
});

define_event!(UserLTVMilestone, EventType::UserMetricsUpdate, {
    user: Pubkey,
    milestone: u64, // $100, $250, $500, $1000
    segment: UserSegment,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ltv_calculation() {
        let user = Pubkey::new_unique();
        let mut ltv_metrics = UserLTVMetrics::new(user);
        
        // Create test user stats
        let mut user_stats = UserStatsPDA::new(user);
        user_stats.total_fees = 50_000_000; // $50 in fees
        user_stats.liquidation_count = 2; // 2 liquidations
        
        // Calculate LTV
        let ltv = ltv_metrics.calculate_ltv(&user_stats).unwrap();
        
        // Should be > $50 due to future projections
        assert!(ltv > 50_000_000);
        assert_eq!(ltv_metrics.user_segment, UserSegment::New);
    }

    #[test]
    fn test_retention_scoring() {
        let user = Pubkey::new_unique();
        let mut ltv_metrics = UserLTVMetrics::new(user);
        
        // Test high retention scenario
        ltv_metrics.update_retention_metrics(1, 0.8).unwrap();
        assert!(ltv_metrics.retention_score > 70);
        assert!(ltv_metrics.churn_risk < 30);
        
        // Test high churn risk scenario
        ltv_metrics.update_retention_metrics(45, 0.1).unwrap();
        assert!(ltv_metrics.retention_score < 30);
        assert!(ltv_metrics.churn_risk > 70);
    }

    #[test]
    fn test_incentive_tiers() {
        let user = Pubkey::new_unique();
        let mut ltv_metrics = UserLTVMetrics::new(user);
        
        // Test growth phase
        ltv_metrics.current_ltv_usd = 100_000_000; // $100
        let incentives = ltv_metrics.get_ltv_incentives();
        assert_eq!(incentives.bonus_mmt_multiplier, 110);
        assert!(!incentives.vip_perks);
        
        // Test approaching target
        ltv_metrics.current_ltv_usd = 450_000_000; // $450
        let incentives = ltv_metrics.get_ltv_incentives();
        assert_eq!(incentives.bonus_mmt_multiplier, 125);
        
        // Test target achieved
        ltv_metrics.current_ltv_usd = 600_000_000; // $600
        let incentives = ltv_metrics.get_ltv_incentives();
        assert_eq!(incentives.bonus_mmt_multiplier, 150);
        assert!(incentives.vip_perks);
    }
}