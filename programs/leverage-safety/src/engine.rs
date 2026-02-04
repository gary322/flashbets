use solana_program::{
    program_error::ProgramError,
    msg,
    clock::Clock,
    pubkey::Pubkey,
};
use crate::{
    error::LeverageSafetyError,
    state::{
        LeverageSafetyConfig, PositionHealth, WarningLevel,
        ChainStep, ChainStepType,
    },
};

/// Fixed point constants (6 decimals)
pub const ONE: u64 = 1_000_000;
pub const HALF: u64 = 500_000;

/// Main leverage safety engine
pub struct LeverageSafetyEngine;

impl LeverageSafetyEngine {
    /// Calculate safe leverage with all constraints
    /// Formula: lev_max = min(100 × (1 + 0.1 × depth), coverage × 100/√N, tier_cap(N))
    pub fn calculate_safe_leverage(
        config: &LeverageSafetyConfig,
        coverage: u64, // Fixed point 6 decimals
        depth: u8,
        outcome_count: u8,
        correlation_factor: u64, // Fixed point 6 decimals, 0-1
        volatility: u64, // Fixed point 6 decimals, percentage
    ) -> Result<u64, ProgramError> {
        // Validate inputs
        if outcome_count == 0 {
            return Err(LeverageSafetyError::InvalidOutcomeCount.into());
        }
        
        if correlation_factor > ONE {
            return Err(LeverageSafetyError::InvalidCorrelationFactor.into());
        }
        
        // 1. Depth boost: 100 × (1 + 0.1 × depth)
        let depth_multiplier = ONE + (config.chain_depth_multiplier * depth as u64) / ONE;
        let depth_adjusted = (config.max_base_leverage * depth_multiplier) / ONE;
        
        msg!("Depth adjusted leverage: {}", depth_adjusted);
        
        // 2. Coverage constraint: coverage × 100/√N
        let sqrt_n = Self::sqrt(outcome_count as u64 * ONE)?;
        let coverage_adjusted = (coverage * 100) / sqrt_n;
        
        msg!("Coverage adjusted leverage: {}", coverage_adjusted);
        
        // 3. Tier cap based on N
        let tier_cap = config.get_tier_cap(outcome_count)?;
        
        msg!("Tier cap: {}", tier_cap);
        
        // Take minimum of all three
        let mut base_leverage = depth_adjusted
            .min(coverage_adjusted)
            .min(tier_cap);
        
        // Apply correlation penalty (reduces leverage for correlated markets)
        if config.correlation_penalty > 0 && correlation_factor > 0 {
            let correlation_penalty = ONE - (correlation_factor * config.correlation_penalty) / ONE;
            base_leverage = (base_leverage * correlation_penalty) / ONE;
            msg!("After correlation penalty: {}", base_leverage);
        }
        
        // Apply volatility adjustment (reduces leverage in volatile markets)
        if config.volatility_adjustment && volatility > 0 {
            // Reduce leverage by 1% for each 1% of volatility above 10%
            let volatility_threshold = 10 * ONE; // 10%
            if volatility > volatility_threshold {
                let excess_volatility = volatility - volatility_threshold;
                let vol_penalty = ONE - (excess_volatility / 100); // 1% reduction per 1% excess
                base_leverage = (base_leverage * vol_penalty.max(HALF)) / ONE; // Min 50% of original
                msg!("After volatility adjustment: {}", base_leverage);
            }
        }
        
        // Ensure within bounds
        if base_leverage > config.max_base_leverage {
            Ok(config.max_base_leverage)
        } else if base_leverage == 0 {
            Ok(1) // Minimum 1x leverage
        } else {
            Ok(base_leverage)
        }
    }
    
    /// Monitor position health with high leverage
    pub fn monitor_high_leverage_position(
        config: &LeverageSafetyConfig,
        position: &mut PositionHealth,
        current_price: u64,
        clock: &Clock,
        price_staleness_threshold: i64, // seconds
    ) -> Result<MonitoringResult, ProgramError> {
        // Check if price is stale
        let price_age = clock.unix_timestamp - position.last_check_timestamp;
        if price_age > price_staleness_threshold {
            return Err(LeverageSafetyError::PriceDataStale.into());
        }
        
        // Update current price
        position.current_price = current_price;
        position.last_check_slot = clock.slot;
        position.last_check_timestamp = clock.unix_timestamp;
        
        // Recalculate effective leverage if needed
        position.recalculate_effective_leverage()?;
        
        // Calculate health metrics
        position.calculate_health_ratio()?;
        position.calculate_liquidation_price()?;
        
        // At 500x, 0.2% move = 100% loss
        let liquidation_threshold = ONE / position.effective_leverage;
        
        // Calculate time to liquidation based on recent volatility
        let time_to_liq = Self::estimate_time_to_liquidation(
            position,
            current_price,
            20 * ONE, // Assume 20% daily volatility for now
        )?;
        position.time_to_liquidation = time_to_liq;
        
        // Check if approaching liquidation
        let mut result = MonitoringResult {
            needs_liquidation: false,
            add_to_queue: false,
            warning_issued: false,
            health_ratio: position.health_ratio,
            effective_leverage: position.effective_leverage,
            liquidation_price: position.liquidation_price,
            time_to_liquidation: time_to_liq,
        };
        
        // Check liquidation conditions
        if position.should_liquidate() {
            result.needs_liquidation = true;
            msg!("Position {:?} needs immediate liquidation", position.position_id);
        } else if position.should_queue_for_liquidation() {
            result.add_to_queue = true;
            position.in_liquidation_queue = true;
            msg!("Position {:?} added to liquidation queue", position.position_id);
        } else if position.warning_level != WarningLevel::None {
            result.warning_issued = true;
            msg!("Warning issued for position {:?}: {:?}", 
                position.position_id,
                position.warning_level
            );
        }
        
        Ok(result)
    }
    
    /// Calculate effective leverage including chains
    pub fn calculate_effective_leverage(
        base_leverage: u64,
        chain_steps: &[ChainStep],
        max_effective: u64,
    ) -> Result<u64, ProgramError> {
        let mut effective = base_leverage as u128;
        
        for step in chain_steps {
            effective = effective
                .checked_mul(step.multiplier as u128)
                .ok_or(LeverageSafetyError::ArithmeticOverflow)?
                / ONE as u128;
            
            // Check against max
            if effective > max_effective as u128 {
                return Ok(max_effective);
            }
        }
        
        Ok(effective as u64)
    }
    
    /// Estimate time to liquidation based on volatility
    fn estimate_time_to_liquidation(
        position: &PositionHealth,
        current_price: u64,
        daily_volatility: u64, // Fixed point percentage
    ) -> Result<i64, ProgramError> {
        if position.health_ratio > 2 * ONE {
            // Very healthy, return max
            return Ok(i64::MAX);
        }
        
        // Calculate price distance to liquidation
        let price_distance = if position.side {
            // Long position
            if current_price > position.liquidation_price {
                current_price - position.liquidation_price
            } else {
                return Ok(0); // Already past liquidation
            }
        } else {
            // Short position
            if position.liquidation_price > current_price {
                position.liquidation_price - current_price
            } else {
                return Ok(0); // Already past liquidation
            }
        };
        
        // Calculate as percentage of current price
        let distance_percent = (price_distance as u128 * ONE as u128) / current_price as u128;
        
        // Estimate based on volatility
        // Assume price follows random walk: expected time = (distance / volatility)^2
        if daily_volatility == 0 {
            return Ok(i64::MAX);
        }
        
        let hourly_vol = daily_volatility / 24; // Rough approximation
        let time_hours = (distance_percent * distance_percent) / (hourly_vol as u128 * hourly_vol as u128);
        let time_seconds = time_hours * 3600 / (ONE as u128 * ONE as u128);
        
        Ok(time_seconds.min(i64::MAX as u128) as i64)
    }
    
    /// Calculate partial liquidation amount
    pub fn calculate_partial_liquidation_amount(
        config: &LeverageSafetyConfig,
        position_size: u64,
        open_interest: u64,
    ) -> Result<u64, ProgramError> {
        // Maximum 8% of OI per slot
        let max_from_oi = (open_interest as u128 * config.liquidation_params.partial_liq_percent as u128) 
            / 10_000;
        
        // Also limit by position size
        let max_from_position = position_size;
        
        // Take minimum
        let liquidation_amount = max_from_oi.min(max_from_position as u128);
        
        if liquidation_amount > u64::MAX as u128 {
            Err(LeverageSafetyError::ArithmeticOverflow.into())
        } else {
            Ok(liquidation_amount as u64)
        }
    }
    
    /// Simple integer square root for coverage calculations
    fn sqrt(n: u64) -> Result<u64, ProgramError> {
        if n == 0 {
            return Ok(0);
        }
        
        // Newton's method
        let mut x = n;
        let mut y = (x + 1) / 2;
        
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        
        Ok(x)
    }
}

/// Result of position monitoring
#[derive(Debug)]
pub struct MonitoringResult {
    pub needs_liquidation: bool,
    pub add_to_queue: bool,
    pub warning_issued: bool,
    pub health_ratio: u64,
    pub effective_leverage: u64,
    pub liquidation_price: u64,
    pub time_to_liquidation: i64,
}