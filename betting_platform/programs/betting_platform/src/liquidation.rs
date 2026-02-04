use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer};
use crate::account_structs::*;
use crate::errors::ErrorCode;
use crate::events::*;
use crate::trading::calculate_coverage;
use crate::math::{calculate_volatility, PriceHistory as MathPriceHistory};

// Liquidation System

#[derive(Accounts)]
pub struct PartialLiquidate<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,
    
    /// CHECK: User account being liquidated
    pub user: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub user_map: Account<'info, MapEntryPDA>,
    
    #[account(mut)]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    #[account(mut)]
    pub price_history: Account<'info, PriceHistory>,
    
    #[account(mut)]
    pub vault_token_account: Account<'info, token::TokenAccount>,
    
    #[account(mut)]
    pub keeper_token_account: Account<'info, token::TokenAccount>,
    
    /// CHECK: This is the vault authority PDA
    pub vault_authority: UncheckedAccount<'info>,
    
    pub token_program: Program<'info, Token>,
}

fn calculate_liquidation_amounts(
    position: &Position,
    current_price: u64,
    keeper_reward_bps: u16,
    insurance_fund_bps: u16,
) -> Result<(i64, u64, u64)> {
    // Calculate PnL
    let pnl = calculate_position_pnl(position, current_price)?;
    
    // Calculate total liquidation value
    let liquidation_value = position.size;
    
    // Calculate keeper reward (e.g., 2% of position size)
    let keeper_reward = liquidation_value
        .checked_mul(keeper_reward_bps as u64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::MathOverflow)?;
    
    // Calculate insurance fund amount (e.g., 3% of position size)
    let insurance_fund_amount = liquidation_value
        .checked_mul(insurance_fund_bps as u64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::MathOverflow)?;
    
    Ok((pnl, keeper_reward, insurance_fund_amount))
}

pub fn partial_liquidate(
    ctx: Context<PartialLiquidate>,
    position_index: u8,
) -> Result<()> {
    let global = &ctx.accounts.global_config;
    let price_cache = &ctx.accounts.price_cache;
    let user_map = &mut ctx.accounts.user_map;
    let clock = Clock::get()?;

    // Validate position exists
    require!(
        (position_index as usize) < user_map.positions.len(),
        ErrorCode::InvalidPosition
    );

    // Clone position data to avoid borrowing conflicts
    let position_data = user_map.positions[position_index as usize].clone();

    // Check if liquidatable
    let current_price = price_cache.last_price;
    let is_liquidatable = if position_data.is_long {
        current_price <= position_data.liquidation_price
    } else {
        current_price >= position_data.liquidation_price
    };

    require!(
        is_liquidatable,
        ErrorCode::PositionHealthy
    );

    // Calculate liquidation amount (2-8% based on volatility)
    let price_history_math = MathPriceHistory {
        movements: ctx.accounts.price_history.movements.clone(),
        last_update_slot: 0, // Not used in volatility calculation
    };
    let volatility = calculate_volatility(&price_history_math);
    let liq_percentage = calculate_liquidation_percentage(volatility, global.coverage);

    let liq_amount = position_data.size
        .saturating_mul(liq_percentage)
        .checked_div(10000)
        .unwrap_or(0)
        .min(position_data.size);

    // Execute liquidation
    let remaining_size = position_data.size.saturating_sub(liq_amount);

    if remaining_size == 0 {
        // Full liquidation
        user_map.positions.remove(position_index as usize);
    } else {
        // Partial liquidation
        let position = &mut user_map.positions[position_index as usize];
        position.size = remaining_size;
    }

    // Calculate liquidation penalty and rewards
    let penalty = liq_amount
        .saturating_mul(500) // 5% penalty
        .checked_div(10000)
        .unwrap_or(0);

    let keeper_reward = penalty
        .saturating_mul(2000) // 20% of penalty to keeper
        .checked_div(10000)
        .unwrap_or(0);

    // Transfer keeper reward
    let cpi_accounts = Transfer {
        from: ctx.accounts.vault_token_account.to_account_info(),
        to: ctx.accounts.keeper_token_account.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::transfer(cpi_ctx, keeper_reward)?;

    // Update global state
    let global = &mut ctx.accounts.global_config;
    global.total_oi = global.total_oi.saturating_sub(liq_amount);
    global.vault = global.vault.saturating_add(penalty.saturating_sub(keeper_reward));

    // Recalculate coverage
    global.coverage = calculate_coverage(
        global.vault,
        global.total_oi,
        1, // Simplified for single position
    );

    // Calculate PnL for the event
    let pnl = calculate_position_pnl(&position_data, current_price)?;
    
    emit!(LiquidationEvent {
        user: ctx.accounts.user.key(),
        keeper: ctx.accounts.keeper.key(),
        position_index,
        liquidation_price: current_price,
        pnl,
        keeper_reward,
        insurance_fund_amount: penalty,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

pub fn calculate_liquidation_percentage(volatility: u64, coverage: u128) -> u64 {
    // Base: 2-8% per slot
    let base_percentage = 200u64; // 2%

    // Volatility adjustment (0-4% additional)
    let volatility_adjustment = volatility
        .saturating_mul(400)
        .checked_div(VOLATILITY_PRECISION)
        .unwrap_or(0)
        .min(400);

    // Coverage adjustment (lower coverage = higher liquidation %)
    let coverage_multiplier = if coverage > PRECISION {
        10000 // 1.0x
    } else {
        // 1.0x - 2.0x based on coverage
        20000u128
            .saturating_sub((coverage * 10000) / PRECISION)
            .max(10000) as u64
    };

    let adjusted_percentage = base_percentage
        .saturating_add(volatility_adjustment)
        .saturating_mul(coverage_multiplier)
        .checked_div(10000)
        .unwrap_or(base_percentage);

    // Cap at 8%
    adjusted_percentage.min(800)
}

// Removed - using calculate_volatility from math module

// Auto-liquidation system for multiple positions
pub fn auto_liquidate_unhealthy_positions<'info>(
    ctx: &mut Context<'_, '_, '_, 'info, PartialLiquidate<'info>>,
    max_positions: u8,
) -> Result<()> {
    let current_price = ctx.accounts.price_cache.last_price;
    
    let mut liquidated_count = 0u8;
    let mut indices_to_liquidate = Vec::new();
    
    // Find liquidatable positions
    for (i, position) in ctx.accounts.user_map.positions.iter().enumerate() {
        if liquidated_count >= max_positions {
            break;
        }
        
        let is_liquidatable = if position.is_long {
            current_price <= position.liquidation_price
        } else {
            current_price >= position.liquidation_price
        };
        
        if is_liquidatable {
            indices_to_liquidate.push(i as u8);
            liquidated_count += 1;
        }
    }
    
    // Liquidate positions in reverse order to maintain indices
    for &index in indices_to_liquidate.iter().rev() {
        partial_liquidate_internal(ctx, index)?;
    }
    
    Ok(())
}

// Internal function to liquidate without consuming context
fn partial_liquidate_internal<'info>(
    ctx: &mut Context<'_, '_, '_, 'info, PartialLiquidate<'info>>,
    position_index: u8,
) -> Result<()> {
    let global = &ctx.accounts.global_config;
    let price_cache = &ctx.accounts.price_cache;
    let user_map = &mut ctx.accounts.user_map;
    let clock = Clock::get()?;
    
    // Validate position index
    require!(
        position_index < user_map.positions.len() as u8,
        ErrorCode::InvalidPositionIndex
    );
    
    let position = &user_map.positions[position_index as usize];
    
    // Check if position is liquidatable
    let current_price = price_cache.last_price;
    let is_liquidatable = if position.is_long {
        current_price <= position.liquidation_price
    } else {
        current_price >= position.liquidation_price
    };
    
    require!(is_liquidatable, ErrorCode::PositionHealthy);
    
    // Calculate liquidation amounts
    let (liquidation_pnl, keeper_reward, insurance_fund_amount) = calculate_liquidation_amounts(
        position,
        current_price,
        global.keeper_reward_bps,
        global.insurance_fund_bps,
    )?;
    
    // Update global state
    let global = &mut ctx.accounts.global_config;
    global.total_oi = global.total_oi.saturating_sub(position.size);
    
    // Update vault balance
    global.vault = if liquidation_pnl < 0 {
        global.vault.saturating_sub(liquidation_pnl.abs() as u64)
    } else {
        global.vault.saturating_add(liquidation_pnl as u64)
    };
    
    // Transfer keeper reward
    if keeper_reward > 0 {
        // Find vault authority bump
        let (vault_auth_pda, vault_auth_bump) = Pubkey::find_program_address(
            &[b"vault_authority"],
            ctx.program_id,
        );
        
        let seeds: &[&[u8]] = &[
            b"vault_authority",
            &[vault_auth_bump],
        ];
        let signer = &[seeds];
        
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.keeper_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        
        token::transfer(cpi_ctx, keeper_reward)?;
    }
    
    // Remove position
    user_map.positions.remove(position_index as usize);
    
    // Emit liquidation event
    emit!(LiquidationEvent {
        user: ctx.accounts.user.key(),
        keeper: ctx.accounts.keeper.key(),
        position_index,
        liquidation_price: current_price,
        pnl: liquidation_pnl,
        keeper_reward,
        insurance_fund_amount,
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}

// Calculate position PnL
pub fn calculate_position_pnl(position: &Position, current_price: u64) -> Result<i64> {
    let pnl = if position.is_long {
        (current_price as i64 - position.entry_price as i64) * position.size as i64 / 1_000_000
    } else {
        (position.entry_price as i64 - current_price as i64) * position.size as i64 / 1_000_000
    };
    Ok(pnl)
}

// Calculate health factor for position prioritization
pub fn calculate_position_health_factor(
    position: &Position,
    current_price: u64,
) -> u64 {
    let price_distance = if position.is_long {
        if current_price > position.liquidation_price {
            current_price - position.liquidation_price
        } else {
            0
        }
    } else {
        if position.liquidation_price > current_price {
            position.liquidation_price - current_price
        } else {
            0
        }
    };
    
    if price_distance == 0 {
        return 0; // Already liquidatable
    }
    
    // Health factor = price distance / entry price * 10000 (basis points)
    (price_distance * 10000) / position.entry_price
}

// Insurance fund mechanics
pub fn process_insurance_claim(
    ctx: Context<ProcessInsuranceClaim>,
    loss_amount: u64,
) -> Result<()> {
    let global = &mut ctx.accounts.global_config;
    
    // Check if vault can cover the loss
    require!(
        global.vault >= loss_amount,
        ErrorCode::InsufficientVaultBalance
    );
    
    // Deduct from vault (insurance fund)
    global.vault = global.vault.saturating_sub(loss_amount);
    
    // Recalculate coverage
    global.coverage = calculate_coverage(
        global.vault,
        global.total_oi,
        1,
    );
    
    // If coverage drops below minimum, trigger halt
    if global.coverage < MINIMUM_COVERAGE {
        global.halt_flag = true;
        global.halt_until = Clock::get()?.slot + 450; // 30 minutes
        
        emit!(CircuitBreakerEvent {
            reason: "Insurance fund depleted".to_string(),
            total_movement: 0,
            halt_until: global.halt_until,
            coverage: global.coverage,
        });
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct ProcessInsuranceClaim<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidation_percentage() {
        // Normal volatility, good coverage
        let normal_vol = 5000; // 50%
        let good_coverage = PRECISION * 2;
        let liq_pct = calculate_liquidation_percentage(normal_vol, good_coverage);
        assert!(liq_pct >= 200 && liq_pct <= 400); // 2-4%
        
        // High volatility, low coverage
        let high_vol = 9000; // 90%
        let low_coverage = PRECISION / 2;
        let liq_pct = calculate_liquidation_percentage(high_vol, low_coverage);
        assert!(liq_pct >= 400 && liq_pct <= 800); // 4-8%
    }
    
    #[test]
    fn test_health_factor() {
        let position = Position {
            proposal_id: 1,
            outcome: 0,
            size: 1000,
            leverage: 10,
            entry_price: 1000,
            liquidation_price: 950,
            is_long: true,
            created_at: 0,
        };
        
        // Healthy position
        let health = calculate_position_health_factor(&position, 1050);
        assert!(health > 0);
        
        // Unhealthy position
        let health = calculate_position_health_factor(&position, 940);
        assert_eq!(health, 0);
    }
}