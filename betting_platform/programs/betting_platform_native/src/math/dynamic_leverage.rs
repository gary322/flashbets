//! Dynamic Leverage System
//!
//! Implements advanced leverage calculations with:
//! - Time-based decay functions
//! - Risk profile integration
//! - Market volatility adjustments
//! - User history considerations
//!
//! Per specification: Production-grade dynamic leverage

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;

use crate::{
    error::BettingPlatformError,
    math::leverage::{calculate_max_leverage, calculate_bootstrap_leverage},
};

/// Dynamic leverage constants
pub const TIME_DECAY_HALF_LIFE_DAYS: i64 = 30; // Leverage decays by half every 30 days
pub const MIN_LEVERAGE_MULTIPLIER: u64 = 5000; // 0.5x in basis points
pub const MAX_LEVERAGE_MULTIPLIER: u64 = 20000; // 2.0x in basis points
pub const VOLATILITY_THRESHOLD_HIGH: u64 = 5000; // 50% volatility
pub const VOLATILITY_THRESHOLD_LOW: u64 = 1000; // 10% volatility

/// Risk profile types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum RiskProfile {
    Conservative { max_leverage_override: u64 },
    Moderate { leverage_cap: u64 },
    Aggressive { leverage_boost: u64 },
    Custom { parameters: RiskParameters },
}

impl RiskProfile {
    /// Get leverage adjustment factor
    pub fn get_leverage_factor(&self) -> u64 {
        match self {
            RiskProfile::Conservative { .. } => 7500, // 0.75x
            RiskProfile::Moderate { .. } => 10000, // 1.0x
            RiskProfile::Aggressive { leverage_boost } => 10000 + leverage_boost,
            RiskProfile::Custom { parameters } => parameters.base_factor,
        }
    }

    /// Apply profile-specific caps
    pub fn apply_cap(&self, leverage: u64) -> u64 {
        match self {
            RiskProfile::Conservative { max_leverage_override } => {
                leverage.min(*max_leverage_override)
            }
            RiskProfile::Moderate { leverage_cap } => {
                leverage.min(*leverage_cap)
            }
            RiskProfile::Aggressive { .. } => leverage, // No cap for aggressive
            RiskProfile::Custom { parameters } => {
                leverage.min(parameters.max_leverage)
            }
        }
    }
}

/// Custom risk parameters
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub struct RiskParameters {
    pub base_factor: u64,
    pub max_leverage: u64,
    pub time_decay_enabled: bool,
    pub volatility_adjustment: bool,
    pub history_weight: u64,
}

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            base_factor: 10000,
            max_leverage: 100,
            time_decay_enabled: true,
            volatility_adjustment: true,
            history_weight: 2000, // 20% weight
        }
    }
}

/// Dynamic leverage calculator
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct DynamicLeverageCalculator {
    pub user_profiles: HashMap<Pubkey, UserLeverageProfile>,
    pub market_volatility: HashMap<[u8; 16], MarketVolatility>,
    pub global_risk_factor: u64,
    pub last_update: i64,
}

impl DynamicLeverageCalculator {
    pub const SIZE: usize = 1024 * 64; // 64KB

    pub fn new() -> Self {
        Self {
            user_profiles: HashMap::new(),
            market_volatility: HashMap::new(),
            global_risk_factor: 10000, // 1.0x default
            last_update: 0,
        }
    }

    /// Calculate dynamic leverage for user
    pub fn calculate_dynamic_leverage(
        &mut self,
        user: &Pubkey,
        market_id: [u8; 16],
        base_leverage: u64,
        coverage: u64,
        current_timestamp: i64,
    ) -> Result<DynamicLeverageResult, ProgramError> {
        // Extract needed values before mutable borrow
        let profile_data = {
            let profile = self.user_profiles
                .entry(*user)
                .or_insert_with(|| UserLeverageProfile::new(*user));
            
            // Clone the needed data
            let last_trade_timestamp = profile.last_trade_timestamp;
            let risk_profile = profile.risk_profile.clone();
            let risk_factor = profile.risk_profile.get_leverage_factor();
            
            (last_trade_timestamp, risk_profile, risk_factor)
        };

        // Start with base leverage
        let mut adjusted_leverage = base_leverage;

        // Apply time decay if enabled
        if profile_data.2 > 0 {
            adjusted_leverage = self.apply_time_decay(
                adjusted_leverage,
                profile_data.0,
                current_timestamp,
            );
        }

        // Apply risk profile adjustments
        adjusted_leverage = {
            let factor = profile_data.1.get_leverage_factor();
            let adjusted = (adjusted_leverage * factor) / 10000;
            profile_data.1.apply_cap(adjusted)
        };

        // Apply market volatility adjustments
        if let Some(volatility) = self.market_volatility.get(&market_id) {
            adjusted_leverage = self.apply_volatility_adjustment(
                adjusted_leverage,
                volatility,
            );
        }

        // Apply user history adjustments
        adjusted_leverage = {
            // Get profile again for history adjustment
            let profile = self.user_profiles.get(user).unwrap();
            self.apply_history_adjustment(adjusted_leverage, profile)
        };

        // Apply global risk factor
        adjusted_leverage = (adjusted_leverage * self.global_risk_factor) / 10000;

        // Ensure within bounds
        let min_leverage = 1;
        // Use default values for depth (0) and outcome_count (2) when not available
        let max_leverage = calculate_max_leverage(0, coverage, 2);
        adjusted_leverage = adjusted_leverage.clamp(min_leverage, max_leverage);

        // Calculate final values for result
        let time_decay_factor = self.calculate_time_decay_factor(
            profile_data.0,
            current_timestamp,
        );
        let volatility_adjustment = self.get_volatility_factor(&market_id);
        let history_adjustment = {
            let profile = self.user_profiles.get(user).unwrap();
            self.calculate_history_factor(profile)
        };

        // Update profile
        let profile = self.user_profiles.get_mut(user).unwrap();
        profile.update_trade(adjusted_leverage, current_timestamp);

        Ok(DynamicLeverageResult {
            base_leverage,
            adjusted_leverage,
            time_decay_factor,
            risk_adjustment: profile_data.2,
            volatility_adjustment,
            history_adjustment,
            final_leverage: adjusted_leverage,
        })
    }

    /// Apply time decay to leverage
    fn apply_time_decay(
        &self,
        leverage: u64,
        last_trade: i64,
        current_time: i64,
    ) -> u64 {
        if last_trade == 0 {
            return leverage;
        }

        let _time_elapsed_days = (current_time - last_trade) / 86400;
        let decay_factor = self.calculate_time_decay_factor(last_trade, current_time);
        
        (leverage * decay_factor) / 10000
    }

    /// Calculate time decay factor
    fn calculate_time_decay_factor(
        &self,
        last_trade: i64,
        current_time: i64,
    ) -> u64 {
        if last_trade == 0 {
            return 10000; // No decay
        }

        let time_elapsed_days = (current_time - last_trade) / 86400;
        
        // Exponential decay: factor = 0.5^(days/half_life)
        // Approximated using integer math
        if time_elapsed_days >= TIME_DECAY_HALF_LIFE_DAYS * 4 {
            MIN_LEVERAGE_MULTIPLIER // Minimum after 4 half-lives
        } else if time_elapsed_days >= TIME_DECAY_HALF_LIFE_DAYS * 2 {
            7500 // 0.75x after 2 half-lives
        } else if time_elapsed_days >= TIME_DECAY_HALF_LIFE_DAYS {
            8500 // 0.85x after 1 half-life
        } else if time_elapsed_days >= TIME_DECAY_HALF_LIFE_DAYS / 2 {
            9200 // 0.92x after 0.5 half-life
        } else {
            10000 // No significant decay
        }
    }

    /// Apply risk profile adjustments
    fn apply_risk_profile(
        &self,
        leverage: u64,
        risk_profile: &RiskProfile,
        _profile: &UserLeverageProfile,
    ) -> u64 {
        let factor = risk_profile.get_leverage_factor();
        let adjusted = (leverage * factor) / 10000;
        
        // Apply profile-specific caps
        risk_profile.apply_cap(adjusted)
    }

    /// Apply volatility adjustments
    fn apply_volatility_adjustment(
        &self,
        leverage: u64,
        volatility: &MarketVolatility,
    ) -> u64 {
        let vol_factor = if volatility.current_volatility > VOLATILITY_THRESHOLD_HIGH {
            7000 // 0.7x for high volatility
        } else if volatility.current_volatility < VOLATILITY_THRESHOLD_LOW {
            11000 // 1.1x for low volatility
        } else {
            // Linear interpolation between thresholds
            let range = VOLATILITY_THRESHOLD_HIGH - VOLATILITY_THRESHOLD_LOW;
            let position = volatility.current_volatility - VOLATILITY_THRESHOLD_LOW;
            let factor_range = 11000 - 7000;
            
            11000 - (position * factor_range / range)
        };

        (leverage * vol_factor) / 10000
    }

    /// Apply user history adjustments
    fn apply_history_adjustment(
        &self,
        leverage: u64,
        profile: &UserLeverageProfile,
    ) -> u64 {
        let history_factor = self.calculate_history_factor(profile);
        (leverage * history_factor) / 10000
    }

    /// Calculate history factor based on user performance
    fn calculate_history_factor(&self, profile: &UserLeverageProfile) -> u64 {
        if profile.total_trades < 10 {
            return 10000; // No adjustment for new users
        }

        let win_rate = if profile.total_trades > 0 {
            (profile.winning_trades * 10000) / profile.total_trades
        } else {
            5000 // 50% default
        };

        let profit_factor = if profile.total_volume > 0 {
            let profit_rate = profile.total_profit.abs() as u64 * 10000 / profile.total_volume;
            if profile.total_profit >= 0 {
                10000 + profit_rate.min(5000) // Up to 1.5x for profitable users
            } else {
                10000 - profit_rate.min(3000) // Down to 0.7x for losing users
            }
        } else {
            10000
        };

        // Combine win rate and profit factor
        (win_rate + profit_factor) / 2
    }

    /// Get volatility factor for market
    fn get_volatility_factor(&self, market_id: &[u8; 16]) -> u64 {
        self.market_volatility
            .get(market_id)
            .map(|v| {
                if v.current_volatility > VOLATILITY_THRESHOLD_HIGH {
                    7000
                } else if v.current_volatility < VOLATILITY_THRESHOLD_LOW {
                    11000
                } else {
                    10000
                }
            })
            .unwrap_or(10000)
    }

    /// Update market volatility
    pub fn update_market_volatility(
        &mut self,
        market_id: [u8; 16],
        volatility: u64,
        timestamp: i64,
    ) {
        let market_vol = self.market_volatility
            .entry(market_id)
            .or_insert_with(|| MarketVolatility::new(market_id));
        
        market_vol.update(volatility, timestamp);
    }

    /// Update global risk factor
    pub fn update_global_risk_factor(
        &mut self,
        new_factor: u64,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        // Validate factor is within reasonable bounds
        if new_factor < 5000 || new_factor > 15000 {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        self.global_risk_factor = new_factor;
        self.last_update = timestamp;

        msg!("Global risk factor updated to: {}", new_factor);
        Ok(())
    }

    /// Get user leverage profile
    pub fn get_user_profile(&self, user: &Pubkey) -> Option<&UserLeverageProfile> {
        self.user_profiles.get(user)
    }

    /// Update user risk profile
    pub fn update_user_risk_profile(
        &mut self,
        user: &Pubkey,
        risk_profile: RiskProfile,
    ) -> Result<(), ProgramError> {
        let profile = self.user_profiles
            .entry(*user)
            .or_insert_with(|| UserLeverageProfile::new(*user));
        
        profile.risk_profile = risk_profile;
        profile.profile_updated = Clock::get()?.unix_timestamp;

        Ok(())
    }
}

/// User leverage profile
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct UserLeverageProfile {
    pub user: Pubkey,
    pub risk_profile: RiskProfile,
    pub last_trade_timestamp: i64,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub total_volume: u64,
    pub total_profit: i64,
    pub average_leverage: u64,
    pub max_leverage_used: u64,
    pub profile_created: i64,
    pub profile_updated: i64,
}

impl UserLeverageProfile {
    pub fn new(user: Pubkey) -> Self {
        let timestamp = Clock::get().unwrap().unix_timestamp;
        
        Self {
            user,
            risk_profile: RiskProfile::Moderate { leverage_cap: 50 },
            last_trade_timestamp: 0,
            total_trades: 0,
            winning_trades: 0,
            total_volume: 0,
            total_profit: 0,
            average_leverage: 0,
            max_leverage_used: 0,
            profile_created: timestamp,
            profile_updated: timestamp,
        }
    }

    /// Update profile with new trade
    pub fn update_trade(&mut self, leverage: u64, timestamp: i64) {
        self.last_trade_timestamp = timestamp;
        self.total_trades += 1;
        
        // Update average leverage
        if self.average_leverage == 0 {
            self.average_leverage = leverage;
        } else {
            self.average_leverage = (self.average_leverage * (self.total_trades - 1) + leverage) / self.total_trades;
        }
        
        // Update max leverage
        if leverage > self.max_leverage_used {
            self.max_leverage_used = leverage;
        }
    }

    /// Record trade outcome
    pub fn record_outcome(&mut self, profit: i64, volume: u64, won: bool) {
        self.total_profit += profit;
        self.total_volume += volume;
        
        if won {
            self.winning_trades += 1;
        }
    }
}

/// Market volatility tracking
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketVolatility {
    pub market_id: [u8; 16],
    pub current_volatility: u64,
    pub avg_volatility_24h: u64,
    pub avg_volatility_7d: u64,
    pub last_update: i64,
    pub samples: VecDeque<(u64, i64)>, // (volatility, timestamp)
}

impl MarketVolatility {
    pub const MAX_SAMPLES: usize = 168; // 7 days of hourly samples

    pub fn new(market_id: [u8; 16]) -> Self {
        Self {
            market_id,
            current_volatility: 0,
            avg_volatility_24h: 0,
            avg_volatility_7d: 0,
            last_update: 0,
            samples: VecDeque::new(),
        }
    }

    /// Update volatility
    pub fn update(&mut self, volatility: u64, timestamp: i64) {
        self.current_volatility = volatility;
        self.last_update = timestamp;
        
        // Add sample
        self.samples.push_back((volatility, timestamp));
        
        // Maintain size limit
        while self.samples.len() > Self::MAX_SAMPLES {
            self.samples.pop_front();
        }
        
        // Update averages
        self.update_averages(timestamp);
    }

    /// Update time-weighted averages
    fn update_averages(&mut self, current_time: i64) {
        let cutoff_24h = current_time - 86400;
        let cutoff_7d = current_time - (7 * 86400);
        
        let mut sum_24h = 0u64;
        let mut count_24h = 0u64;
        let mut sum_7d = 0u64;
        let mut count_7d = 0u64;
        
        for (vol, ts) in &self.samples {
            if *ts >= cutoff_24h {
                sum_24h += vol;
                count_24h += 1;
            }
            if *ts >= cutoff_7d {
                sum_7d += vol;
                count_7d += 1;
            }
        }
        
        self.avg_volatility_24h = if count_24h > 0 { sum_24h / count_24h } else { 0 };
        self.avg_volatility_7d = if count_7d > 0 { sum_7d / count_7d } else { 0 };
    }
}

/// Dynamic leverage calculation result
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct DynamicLeverageResult {
    pub base_leverage: u64,
    pub adjusted_leverage: u64,
    pub time_decay_factor: u64,
    pub risk_adjustment: u64,
    pub volatility_adjustment: u64,
    pub history_adjustment: u64,
    pub final_leverage: u64,
}

/// Leverage decay curve types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum DecayCurve {
    Exponential { half_life: i64 },
    Linear { days_to_zero: i64 },
    Stepped { steps: Vec<(i64, u64)> }, // (days, multiplier)
    None,
}

impl DecayCurve {
    /// Calculate decay factor
    pub fn calculate_factor(&self, days_elapsed: i64) -> u64 {
        match self {
            DecayCurve::Exponential { half_life } => {
                // Simplified exponential decay
                if days_elapsed >= half_life * 4 {
                    5000 // 0.5x minimum
                } else if days_elapsed >= half_life * 2 {
                    7500 // 0.75x
                } else if days_elapsed >= *half_life {
                    8500 // 0.85x
                } else {
                    10000 // No decay
                }
            }
            DecayCurve::Linear { days_to_zero } => {
                if days_elapsed >= *days_to_zero {
                    5000 // Minimum
                } else {
                    10000u64.saturating_sub((5000u64 * days_elapsed as u64) / *days_to_zero as u64)
                }
            }
            DecayCurve::Stepped { steps } => {
                for (day_threshold, multiplier) in steps {
                    if days_elapsed >= *day_threshold {
                        return *multiplier;
                    }
                }
                10000 // Default no decay
            }
            DecayCurve::None => 10000,
        }
    }
}

use std::collections::VecDeque;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_decay() {
        let calc = DynamicLeverageCalculator::new();
        
        // Test no decay
        let factor = calc.calculate_time_decay_factor(100, 100);
        assert_eq!(factor, 10000);
        
        // Test half-life decay
        let factor = calc.calculate_time_decay_factor(0, TIME_DECAY_HALF_LIFE_DAYS * 86400);
        assert_eq!(factor, 8500); // 0.85x after 1 half-life
        
        // Test maximum decay
        let factor = calc.calculate_time_decay_factor(0, TIME_DECAY_HALF_LIFE_DAYS * 86400 * 5);
        assert_eq!(factor, MIN_LEVERAGE_MULTIPLIER);
    }

    #[test]
    fn test_risk_profiles() {
        let conservative = RiskProfile::Conservative { max_leverage_override: 20 };
        assert_eq!(conservative.get_leverage_factor(), 7500);
        assert_eq!(conservative.apply_cap(100), 20);

        let aggressive = RiskProfile::Aggressive { leverage_boost: 5000 };
        assert_eq!(aggressive.get_leverage_factor(), 15000);
        assert_eq!(aggressive.apply_cap(100), 100); // No cap
    }

    #[test]
    fn test_volatility_adjustment() {
        let mut calc = DynamicLeverageCalculator::new();
        let market_id = [0u8; 16];
        
        // High volatility
        calc.update_market_volatility(market_id, 6000, 100);
        
        if let Some(vol) = calc.market_volatility.get(&market_id) {
            let adjusted = calc.apply_volatility_adjustment(100, vol);
            assert_eq!(adjusted, 70); // 0.7x for high volatility
        }
        
        // Low volatility
        calc.update_market_volatility(market_id, 500, 200);
        
        if let Some(vol) = calc.market_volatility.get(&market_id) {
            let adjusted = calc.apply_volatility_adjustment(100, vol);
            assert_eq!(adjusted, 110); // 1.1x for low volatility
        }
    }

    #[test]
    fn test_dynamic_leverage_calculation() {
        let mut calc = DynamicLeverageCalculator::new();
        let user = Pubkey::new_unique();
        let market_id = [0u8; 16];
        
        // Set up user profile
        calc.update_user_risk_profile(&user, RiskProfile::Moderate { leverage_cap: 50 }).unwrap();
        
        // Calculate dynamic leverage
        let result = calc.calculate_dynamic_leverage(
            &user,
            market_id,
            20, // base leverage
            150, // coverage
            100, // timestamp
        ).unwrap();
        
        assert_eq!(result.base_leverage, 20);
        assert!(result.adjusted_leverage <= 50); // Respects cap
        assert_eq!(result.risk_adjustment, 10000); // 1.0x for moderate
    }
}