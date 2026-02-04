//! Perpetual Position Management
//!
//! Handles creation, modification, and closure of perpetual positions

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
    cdp::{CDPAccount, BorrowRequest},
};

use super::{
    state::{PerpetualPosition, PerpetualMarket, PositionType, PositionStatus},
    funding::calculate_funding_payment,
};

/// Open a new perpetual position
pub fn open_position(
    program_id: &Pubkey,
    owner: &Pubkey,
    market: &mut PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
    position_type: PositionType,
    size: u128,
    leverage: u16,
    collateral: u128,
) -> Result<PerpetualPosition, ProgramError> {
    // Validate leverage
    if leverage < market.min_leverage || leverage > market.max_leverage {
        msg!("Invalid leverage: {} (min: {}, max: {})", 
             leverage, market.min_leverage, market.max_leverage);
        return Err(BettingPlatformError::InvalidLeverage.into());
    }
    
    // Check market status
    if market.status != super::state::MarketStatus::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }
    
    // Calculate required margin
    let required_margin = (size as f64) * market.initial_margin_ratio;
    if (collateral as f64) < required_margin {
        msg!("Insufficient collateral: {} < {}", collateral, required_margin);
        return Err(BettingPlatformError::InsufficientCollateral.into());
    }
    
    // Get current price from oracle
    let entry_price = oracle.current_prob;
    
    // Generate position ID
    let position_id = generate_position_id(owner, market.market_id);
    
    // Create position
    let mut position = PerpetualPosition::new(
        position_id,
        *owner,
        market.market_id,
        cdp.owner,
        position_type.clone(),
        entry_price,
        size,
        leverage,
        collateral,
    );
    
    // Set timestamps
    let clock = Clock::get()?;
    position.created_at = clock.unix_timestamp;
    position.last_updated = clock.unix_timestamp;
    position.last_funding_payment = clock.unix_timestamp;
    
    // Set oracle scalar
    position.entry_oracle_scalar = calculate_oracle_scalar(oracle);
    position.entry_funding_rate = market.funding_rate;
    
    // Update market open interest
    market.update_open_interest(&position_type, size, true);
    market.total_collateral += collateral;
    
    // Update CDP
    cdp.deposit_collateral(collateral)?;
    
    // Apply leverage through CDP borrowing
    let borrow_amount = (size as f64 * (leverage as f64 - 1.0)) as u128;
    if borrow_amount > 0 {
        let borrow_request = BorrowRequest {
            amount: borrow_amount,
            leverage,
            use_oracle_scalar: true,
        };
        
        // This would normally execute the actual borrow
        cdp.debt_amount += borrow_amount;
        cdp.leverage = leverage;
    }
    
    msg!("Opened perpetual position {} with {}x leverage", 
         position_id, leverage);
    
    Ok(position)
}

/// Close an existing perpetual position
pub fn close_position(
    program_id: &Pubkey,
    position: &mut PerpetualPosition,
    market: &mut PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
) -> Result<i128, ProgramError> {
    // Validate position status
    if position.status != PositionStatus::Active {
        return Err(BettingPlatformError::InvalidPositionStatus.into());
    }
    
    // Update to current mark price
    position.update_mark_price(oracle.current_prob);
    
    // Calculate final funding payment
    let funding_payment = calculate_funding_payment(
        position,
        market.funding_rate,
        Clock::get()?.unix_timestamp,
    );
    position.apply_funding(funding_payment);
    
    // Calculate total PnL
    let total_pnl = position.close(oracle.current_prob)?;
    
    // Update market open interest
    market.update_open_interest(&position.position_type, position.size, false);
    market.total_collateral = market.total_collateral
        .saturating_sub(position.collateral);
    
    // Return collateral and PnL to CDP
    let return_amount = (position.collateral as i128 + total_pnl).max(0) as u128;
    cdp.withdraw_collateral(return_amount, oracle.current_prob)?;
    
    // Repay CDP debt if any
    if cdp.debt_amount > 0 {
        let repay_amount = cdp.debt_amount.min(return_amount);
        cdp.debt_amount = cdp.debt_amount.saturating_sub(repay_amount);
    }
    
    msg!("Closed position {} with PnL: {}", position.position_id, total_pnl);
    
    Ok(total_pnl)
}

/// Modify an existing position
pub fn modify_position(
    position: &mut PerpetualPosition,
    market: &mut PerpetualMarket,
    cdp: &mut CDPAccount,
    oracle: &OraclePDA,
    new_size: Option<u128>,
    new_leverage: Option<u16>,
    add_collateral: Option<u128>,
    remove_collateral: Option<u128>,
) -> Result<(), ProgramError> {
    // Validate position is active
    if position.status != PositionStatus::Active {
        return Err(BettingPlatformError::InvalidPositionStatus.into());
    }
    
    // Update mark price
    position.update_mark_price(oracle.current_prob);
    
    // Handle size change
    if let Some(size) = new_size {
        let size_diff = size as i128 - position.size as i128;
        
        if size_diff > 0 {
            // Increasing position
            market.update_open_interest(
                &position.position_type,
                size_diff.abs() as u128,
                true,
            );
        } else if size_diff < 0 {
            // Decreasing position
            market.update_open_interest(
                &position.position_type,
                size_diff.abs() as u128,
                false,
            );
            
            // Realize partial PnL
            let partial_ratio = (size_diff.abs() as f64) / (position.size as f64);
            let partial_pnl = (position.unrealized_pnl as f64 * partial_ratio) as i128;
            position.realized_pnl += partial_pnl;
            position.unrealized_pnl -= partial_pnl;
        }
        
        position.size = size;
    }
    
    // Handle leverage change
    if let Some(leverage) = new_leverage {
        if leverage < market.min_leverage || leverage > market.max_leverage {
            return Err(BettingPlatformError::InvalidLeverage.into());
        }
        
        position.leverage = leverage;
        position.margin_ratio = 1.0 / leverage as f64;
        position.initial_margin = position.margin_ratio;
        
        // Recalculate liquidation price
        position.liquidation_price = PerpetualPosition::calculate_liquidation_price(
            position.entry_price,
            leverage,
            &position.position_type,
        );
    }
    
    // Handle collateral changes
    if let Some(amount) = add_collateral {
        position.collateral += amount;
        cdp.deposit_collateral(amount)?;
        market.total_collateral += amount;
    }
    
    if let Some(amount) = remove_collateral {
        // Check margin requirements after removal
        let new_collateral = position.collateral.saturating_sub(amount);
        let required_margin = (position.size as f64) * market.maintenance_margin_ratio;
        
        if (new_collateral as f64) < required_margin {
            return Err(BettingPlatformError::InsufficientMargin.into());
        }
        
        position.collateral = new_collateral;
        cdp.withdraw_collateral(amount, oracle.current_prob)?;
        market.total_collateral = market.total_collateral.saturating_sub(amount);
    }
    
    // Update timestamp
    position.last_updated = Clock::get()?.unix_timestamp;
    
    msg!("Modified position {}", position.position_id);
    
    Ok(())
}

/// Add stop loss to position
pub fn add_stop_loss(
    position: &mut PerpetualPosition,
    stop_price: f64,
) -> Result<(), ProgramError> {
    // Validate position is active
    if position.status != PositionStatus::Active {
        return Err(BettingPlatformError::InvalidPositionStatus.into());
    }
    
    // Validate stop price
    match position.position_type {
        PositionType::Long => {
            if stop_price >= position.mark_price {
                return Err(BettingPlatformError::InvalidStopPrice.into());
            }
        },
        PositionType::Short => {
            if stop_price <= position.mark_price {
                return Err(BettingPlatformError::InvalidStopPrice.into());
            }
        }
    }
    
    position.stop_loss = Some(stop_price);
    position.last_updated = Clock::get()?.unix_timestamp;
    
    msg!("Added stop loss at {} to position {}", stop_price, position.position_id);
    
    Ok(())
}

/// Add take profit to position
pub fn add_take_profit(
    position: &mut PerpetualPosition,
    target_price: f64,
) -> Result<(), ProgramError> {
    // Validate position is active
    if position.status != PositionStatus::Active {
        return Err(BettingPlatformError::InvalidPositionStatus.into());
    }
    
    // Validate target price
    match position.position_type {
        PositionType::Long => {
            if target_price <= position.mark_price {
                return Err(BettingPlatformError::InvalidTargetPrice.into());
            }
        },
        PositionType::Short => {
            if target_price >= position.mark_price {
                return Err(BettingPlatformError::InvalidTargetPrice.into());
            }
        }
    }
    
    position.take_profit = Some(target_price);
    position.last_updated = Clock::get()?.unix_timestamp;
    
    msg!("Added take profit at {} to position {}", 
         target_price, position.position_id);
    
    Ok(())
}

/// Check and trigger stop loss/take profit
pub fn check_triggers(
    position: &mut PerpetualPosition,
    current_price: f64,
) -> Result<bool, ProgramError> {
    // Check stop loss
    if let Some(stop_price) = position.stop_loss {
        let should_trigger = match position.position_type {
            PositionType::Long => current_price <= stop_price,
            PositionType::Short => current_price >= stop_price,
        };
        
        if should_trigger {
            msg!("Stop loss triggered for position {} at {}", 
                 position.position_id, current_price);
            return Ok(true);
        }
    }
    
    // Check take profit
    if let Some(target_price) = position.take_profit {
        let should_trigger = match position.position_type {
            PositionType::Long => current_price >= target_price,
            PositionType::Short => current_price <= target_price,
        };
        
        if should_trigger {
            msg!("Take profit triggered for position {} at {}", 
                 position.position_id, current_price);
            return Ok(true);
        }
    }
    
    Ok(false)
}

/// Calculate oracle scalar for leverage boost
fn calculate_oracle_scalar(oracle: &OraclePDA) -> f64 {
    // Use the oracle's unified scalar
    let sigma = oracle.current_sigma.max(0.01);
    let cap_fused = 20.0;
    let cap_vault = 30.0;
    let base_risk = 0.25;
    
    let risk = oracle.current_prob * (1.0 - oracle.current_prob);
    let unified_scalar = (1.0 / sigma) * cap_fused;
    let premium_factor = (risk / base_risk) * cap_vault;
    
    (unified_scalar * premium_factor).min(10.0) // Cap at 10x for perpetuals
}

/// Generate unique position ID
fn generate_position_id(owner: &Pubkey, market_id: u128) -> u128 {
    let clock = Clock::get().unwrap();
    let timestamp = clock.unix_timestamp as u128;
    let owner_bytes = owner.to_bytes();
    let owner_part = u128::from_le_bytes(owner_bytes[0..16].try_into().unwrap());
    
    // Combine: market_id (high 64 bits) + timestamp (middle 32 bits) + owner (low 32 bits)
    (market_id << 64) | (timestamp << 32) | (owner_part & 0xFFFFFFFF)
}

/// Validate position health
pub fn validate_position_health(
    position: &PerpetualPosition,
    market: &PerpetualMarket,
) -> Result<f64, ProgramError> {
    // Calculate current margin ratio
    let position_value = (position.size as f64) * position.mark_price;
    let current_equity = position.collateral as f64 + position.unrealized_pnl as f64;
    
    if position_value == 0.0 {
        return Ok(f64::MAX);
    }
    
    let margin_ratio = current_equity / position_value;
    
    // Check if below maintenance margin
    if margin_ratio < market.maintenance_margin_ratio {
        msg!("Position {} below maintenance margin: {:.4} < {:.4}", 
             position.position_id, margin_ratio, market.maintenance_margin_ratio);
        return Err(BettingPlatformError::BelowMaintenanceMargin.into());
    }
    
    Ok(margin_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_position_creation() {
        let owner = Pubkey::new_unique();
        let market_id = 1;
        let position_id = generate_position_id(&owner, market_id);
        
        let position = PerpetualPosition::new(
            position_id,
            owner,
            market_id,
            Pubkey::new_unique(),
            PositionType::Long,
            100.0,
            10000,
            10,
            1000,
        );
        
        assert_eq!(position.leverage, 10);
        assert_eq!(position.size, 10000);
        assert_eq!(position.collateral, 1000);
        assert!(position.liquidation_price < 100.0);
    }
    
    #[test]
    fn test_pnl_calculation() {
        let mut position = PerpetualPosition::new(
            1,
            Pubkey::new_unique(),
            1,
            Pubkey::new_unique(),
            PositionType::Long,
            100.0,
            10000,
            10,
            1000,
        );
        
        // Price increases to 110
        position.update_mark_price(110.0);
        assert!(position.unrealized_pnl > 0);
        
        // Price decreases to 90
        position.update_mark_price(90.0);
        assert!(position.unrealized_pnl < 0);
    }
}