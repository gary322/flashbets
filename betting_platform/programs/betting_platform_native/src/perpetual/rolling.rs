//! Auto-Rolling Mechanism for Perpetual Positions
//!
//! Handles automatic position rolling before expiry

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    cdp::CDPAccount,
};

use super::{
    state::{PerpetualPosition, PerpetualMarket, PositionStatus, RollParameters},
    position::{close_position, open_position},
    funding::calculate_funding_payment,
};

/// Roll strategy
#[derive(Debug, Clone, PartialEq)]
pub enum RollStrategy {
    /// Roll to next available expiry
    NextExpiry,
    /// Roll to specific duration
    FixedDuration(u64),
    /// Roll to perpetual (no expiry)
    Perpetual,
    /// Custom roll logic
    Custom(RollConfig),
}

/// Custom roll configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RollConfig {
    /// Target expiry offset (slots)
    pub target_offset: u64,
    
    /// Acceptable range (+/- slots)
    pub acceptable_range: u64,
    
    /// Prefer liquid contracts
    pub prefer_liquid: bool,
    
    /// Maximum premium to pay
    pub max_premium: f64,
}

/// Execute auto-roll for position
pub fn execute_auto_roll(
    program_id: &Pubkey,
    position: &mut PerpetualPosition,
    market: &mut PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
    strategy: RollStrategy,
) -> Result<(), ProgramError> {
    // Validate position can be rolled
    if !position.auto_roll_enabled {
        return Err(BettingPlatformError::AutoRollDisabled.into());
    }
    
    if position.status != PositionStatus::Active {
        return Err(BettingPlatformError::InvalidPositionStatus.into());
    }
    
    if position.roll_count >= position.roll_params.max_rolls {
        msg!("Position {} reached max rolls: {}", 
             position.position_id, position.roll_count);
        return Err(BettingPlatformError::MaxRollsExceeded.into());
    }
    
    // Check if roll is needed
    let current_slot = Clock::get()?.slot;
    if !position.should_roll(current_slot) {
        return Ok(());
    }
    
    // Set status to rolling
    position.status = PositionStatus::RollingOver;
    
    // Calculate current position value
    let current_value = calculate_position_value(position, oracle);
    
    // Determine target expiry based on strategy
    let target_expiry = determine_target_expiry(&strategy, current_slot);
    
    // Calculate roll cost
    let roll_cost = calculate_roll_cost(
        position,
        market,
        oracle,
        target_expiry,
    )?;
    
    // Check roll cost against limit
    if roll_cost > position.roll_params.max_roll_fee {
        msg!("Roll cost {} exceeds max fee {}", 
             roll_cost, position.roll_params.max_roll_fee);
        position.status = PositionStatus::Active;
        return Err(BettingPlatformError::RollCostTooHigh.into());
    }
    
    // Apply funding before roll
    let funding_payment = calculate_funding_payment(
        position,
        market.funding_rate,
        Clock::get()?.unix_timestamp,
    );
    position.apply_funding(funding_payment);
    
    // Store old position details
    let old_entry_price = position.entry_price;
    let old_size = position.size;
    let old_leverage = position.leverage;
    let old_collateral = position.collateral;
    let old_pnl = position.unrealized_pnl + position.realized_pnl;
    
    // Calculate slippage
    let slippage = calculate_slippage(oracle.current_prob, old_entry_price);
    if slippage > position.roll_params.max_slippage {
        msg!("Slippage {:.4} exceeds max {:.4}", 
             slippage, position.roll_params.max_slippage);
        position.status = PositionStatus::Active;
        return Err(BettingPlatformError::ExcessiveSlippage.into());
    }
    
    // Update position for new contract
    position.entry_price = oracle.current_prob;
    position.mark_price = oracle.current_prob;
    position.expiry = Some(target_expiry as i64);
    position.roll_count += 1;
    position.roll_params.last_roll_slot = current_slot;
    position.total_fees_paid += roll_cost;
    
    // Reset funding
    position.accumulated_funding = 0;
    position.last_funding_payment = Clock::get()?.unix_timestamp;
    position.entry_funding_rate = market.funding_rate;
    
    // Carry over PnL
    position.realized_pnl = old_pnl;
    position.unrealized_pnl = 0;
    
    // Adjust position size for roll
    if strategy == RollStrategy::Perpetual {
        // For perpetual rolls, maintain exposure
        position.size = adjust_size_for_price(old_size, old_entry_price, oracle.current_prob);
    }
    
    // Return to active status
    position.status = PositionStatus::Active;
    
    msg!("Rolled position {} to new expiry: {} (roll #{}/{})", 
         position.position_id, 
         target_expiry,
         position.roll_count,
         position.roll_params.max_rolls);
    
    Ok(())
}

/// Batch roll multiple positions
pub fn batch_roll_positions(
    program_id: &Pubkey,
    positions: &mut [PerpetualPosition],
    market: &mut PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
    strategy: RollStrategy,
) -> Result<u32, ProgramError> {
    let mut rolled_count = 0;
    let current_slot = Clock::get()?.slot;
    
    for position in positions.iter_mut() {
        // Skip if not eligible
        if !position.auto_roll_enabled || !position.should_roll(current_slot) {
            continue;
        }
        
        // Attempt roll
        match execute_auto_roll(
            program_id,
            position,
            market,
            cdp,
            oracle,
            strategy.clone(),
        ) {
            Ok(_) => {
                rolled_count += 1;
                msg!("Successfully rolled position {}", position.position_id);
            },
            Err(e) => {
                msg!("Failed to roll position {}: {:?}", position.position_id, e);
                // Continue with other positions
            }
        }
    }
    
    msg!("Batch rolled {} positions", rolled_count);
    Ok(rolled_count)
}

/// Calculate position value including PnL
fn calculate_position_value(
    position: &PerpetualPosition,
    oracle: &OraclePDA,
) -> u128 {
    let base_value = position.collateral;
    let pnl = position.unrealized_pnl + position.accumulated_funding;
    
    if pnl >= 0 {
        base_value + (pnl as u128)
    } else {
        base_value.saturating_sub(pnl.abs() as u128)
    }
}

/// Determine target expiry based on strategy
fn determine_target_expiry(
    strategy: &RollStrategy,
    current_slot: u64,
) -> u64 {
    match strategy {
        RollStrategy::NextExpiry => {
            // Next monthly expiry (30 days)
            current_slot + 2_592_000
        },
        RollStrategy::FixedDuration(duration) => {
            current_slot + duration
        },
        RollStrategy::Perpetual => {
            // No expiry for perpetual
            u64::MAX
        },
        RollStrategy::Custom(config) => {
            current_slot + config.target_offset
        }
    }
}

/// Calculate cost of rolling position
fn calculate_roll_cost(
    position: &PerpetualPosition,
    market: &PerpetualMarket,
    oracle: &OraclePDA,
    target_expiry: u64,
) -> Result<u128, ProgramError> {
    // Base fee
    let base_fee = (position.size as f64 * market.trading_fee) as u128;
    
    // Time value adjustment (longer expiry = higher cost)
    let current_slot = Clock::get()?.slot;
    let time_to_expiry = target_expiry.saturating_sub(current_slot);
    let time_factor = 1.0 + (time_to_expiry as f64 / 2_592_000.0) * 0.1; // +10% per month
    
    // Volatility adjustment
    let vol_factor = 1.0 + oracle.current_sigma * 2.0; // Higher vol = higher cost
    
    // Calculate total cost
    let total_cost = (base_fee as f64 * time_factor * vol_factor) as u128;
    
    Ok(total_cost)
}

/// Calculate slippage between prices
fn calculate_slippage(new_price: f64, old_price: f64) -> f64 {
    ((new_price - old_price).abs() / old_price)
}

/// Adjust position size for new price
fn adjust_size_for_price(old_size: u128, old_price: f64, new_price: f64) -> u128 {
    // Maintain same notional value
    ((old_size as f64) * old_price / new_price) as u128
}

/// Configure auto-roll parameters
pub fn configure_auto_roll(
    position: &mut PerpetualPosition,
    enabled: bool,
    params: Option<RollParameters>,
) -> Result<(), ProgramError> {
    position.auto_roll_enabled = enabled;
    
    if let Some(p) = params {
        // Validate parameters
        if p.max_rolls == 0 || p.max_rolls > 365 {
            return Err(BettingPlatformError::InvalidRollParameters.into());
        }
        
        if p.max_slippage > 0.1 { // 10% max
            return Err(BettingPlatformError::InvalidRollParameters.into());
        }
        
        position.roll_params = p;
    }
    
    msg!("Configured auto-roll for position {}: enabled={}", 
         position.position_id, enabled);
    
    Ok(())
}

/// Check positions needing roll
pub fn check_positions_for_roll(
    positions: &[PerpetualPosition],
    current_slot: u64,
) -> Vec<u128> {
    positions.iter()
        .filter(|p| p.auto_roll_enabled && p.should_roll(current_slot))
        .map(|p| p.position_id)
        .collect()
}

/// Emergency stop all rolls
pub fn emergency_stop_rolls(
    positions: &mut [PerpetualPosition],
) -> Result<u32, ProgramError> {
    let mut stopped_count = 0;
    
    for position in positions.iter_mut() {
        if position.status == PositionStatus::RollingOver {
            position.status = PositionStatus::Active;
            position.auto_roll_enabled = false;
            stopped_count += 1;
        }
    }
    
    msg!("Emergency stopped {} rolling positions", stopped_count);
    Ok(stopped_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_roll_strategy() {
        let current_slot = 1000;
        
        // Test next expiry
        let target = determine_target_expiry(&RollStrategy::NextExpiry, current_slot);
        assert_eq!(target, current_slot + 2_592_000);
        
        // Test fixed duration
        let target = determine_target_expiry(&RollStrategy::FixedDuration(1000), current_slot);
        assert_eq!(target, current_slot + 1000);
        
        // Test perpetual
        let target = determine_target_expiry(&RollStrategy::Perpetual, current_slot);
        assert_eq!(target, u64::MAX);
    }
    
    #[test]
    fn test_slippage_calculation() {
        let slippage = calculate_slippage(102.0, 100.0);
        assert_eq!(slippage, 0.02);
        
        let slippage = calculate_slippage(98.0, 100.0);
        assert_eq!(slippage, 0.02);
    }
    
    #[test]
    fn test_size_adjustment() {
        let new_size = adjust_size_for_price(1000, 100.0, 110.0);
        assert_eq!(new_size, 909); // Maintains notional value
        
        let new_size = adjust_size_for_price(1000, 100.0, 90.0);
        assert_eq!(new_size, 1111); // Maintains notional value
    }
}