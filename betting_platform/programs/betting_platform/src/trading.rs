use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer, Burn};
use crate::account_structs::*;
use crate::errors::ErrorCode;
use crate::events::*;
use crate::fixed_math::{FixedPoint, PRECISION as FIXED_PRECISION};
use crate::math::*;

// Trading Engine Implementation

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub verse: Account<'info, VersePDA>,
    
    #[account(mut)]
    pub proposal: Account<'info, ProposalPDA>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = MapEntryPDA::space(50),
        seeds = [b"map_entry", user.key().as_ref(), verse.verse_id_as_u128().to_le_bytes().as_ref()],
        bump
    )]
    pub user_map: Account<'info, MapEntryPDA>,
    
    #[account(mut)]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    #[account(mut)]
    pub user_token_account: Account<'info, token::TokenAccount>,
    
    #[account(mut)]
    pub vault_token_account: Account<'info, token::TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub verse: Account<'info, VersePDA>,
    
    #[account(mut)]
    pub proposal: Account<'info, ProposalPDA>,
    
    #[account(mut)]
    pub user_map: Account<'info, MapEntryPDA>,
    
    #[account(mut)]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    #[account(mut)]
    pub user_token_account: Account<'info, token::TokenAccount>,
    
    #[account(mut)]
    pub vault_token_account: Account<'info, token::TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OpenPositionParams {
    pub amount: u64,
    pub leverage: u64,
    pub outcome: u8,
    pub is_long: bool,
}

pub fn open_position(ctx: Context<OpenPosition>, params: OpenPositionParams) -> Result<()> {
    let global = &ctx.accounts.global_config;
    let verse = &ctx.accounts.verse;
    let proposal = &ctx.accounts.proposal;
    let user_map = &mut ctx.accounts.user_map;
    let price_cache = &ctx.accounts.price_cache;

    // Validate trading conditions
    verse.can_trade()?;
    require!(!global.halt_flag, ErrorCode::SystemHalted);

    // Validate price staleness
    let clock = Clock::get()?;
    require!(
        !price_cache.is_stale(clock.slot),
        ErrorCode::StalePrice
    );

    // Calculate maximum leverage
    let max_leverage = calculate_max_leverage(
        global.coverage,
        verse.depth,
        proposal.outcome_count() as u32,
    );

    require!(
        params.leverage <= max_leverage,
        ErrorCode::ExcessiveLeverage
    );

    // Calculate required collateral
    let position_size = params.amount
        .checked_mul(params.leverage)
        .ok_or(ErrorCode::MathOverflow)?;

    let required_collateral = calculate_required_collateral(
        position_size,
        params.leverage,
        global.coverage,
    );

    // Transfer collateral from user
    let cpi_accounts = Transfer {
        from: ctx.accounts.user_token_account.to_account_info(),
        to: ctx.accounts.vault_token_account.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::transfer(cpi_ctx, required_collateral)?;

    // Calculate entry price based on AMM
    let entry_price = match proposal.amm_type {
        AMMType::LMSR => calculate_lmsr_price(
            &proposal.q_values(),
            proposal.liquidity_parameter(),
            params.outcome as usize,
        ),
        AMMType::PMAMM => calculate_pmamm_price(
            &proposal.prices,
            params.amount,
            proposal.liquidity_parameter(),
            clock.slot - proposal.created_at() as u64,
            proposal.expires_at() as u64 - proposal.created_at() as u64,
        ),
        AMMType::L2Norm => calculate_l2_price(
            &proposal.prices,
            params.outcome as usize,
            params.amount,
        ),
    }?;

    // Calculate liquidation price
    let liquidation_price = calculate_liquidation_price(
        entry_price,
        params.leverage,
        params.is_long,
        global.coverage,
    );

    // Create position
    let position = Position {
        proposal_id: proposal.id(),
        outcome: params.outcome,
        size: position_size,
        leverage: params.leverage,
        entry_price,
        liquidation_price,
        is_long: params.is_long,
        created_at: clock.unix_timestamp,
    };

    // Update user map
    user_map.positions.push(position.clone());
    user_map.total_collateral = user_map.total_collateral
        .checked_add(required_collateral)
        .ok_or(ErrorCode::MathOverflow)?;
    user_map.last_update = clock.unix_timestamp;
    user_map.health_factor = user_map.calculate_health(&proposal.prices);

    // Update global state
    let global = &mut ctx.accounts.global_config;
    global.total_oi = global.total_oi
        .checked_add(position_size)
        .ok_or(ErrorCode::MathOverflow)?;

    // Update coverage
    global.coverage = calculate_coverage(
        global.vault,
        global.total_oi,
        proposal.outcome_count() as u32,
    );

    // Emit event
    emit!(PositionOpenedEvent {
        user: ctx.accounts.user.key(),
        verse_id: verse.verse_id_as_u128(),
        proposal_id: proposal.id(),
        position: position.clone(),
        collateral: required_collateral,
    });

    Ok(())
}

pub fn calculate_max_leverage(
    coverage: u128,
    depth: u8,
    outcome_count: u32,
) -> u64 {
    // Base formula: min(100 × (1 + 0.1 × depth), coverage × 100/√N, tier_cap(N))

    let depth_boost = 100u64
        .saturating_mul(100 + (depth as u64 * 10))
        .checked_div(100)
        .unwrap_or(100);

    let coverage_cap = if outcome_count == 0 {
        0
    } else {
        let sqrt_n = (outcome_count as f64).sqrt() as u64;
        coverage
            .saturating_mul(100)
            .checked_div(sqrt_n as u128)
            .unwrap_or(0)
            .min(u64::MAX as u128) as u64
    };

    let tier_cap = match outcome_count {
        1 => 100,
        2 => 70,
        3..=4 => 25,
        5..=8 => 15,
        9..=16 => 12,
        17..=64 => 10,
        _ => 5,
    };

    depth_boost.min(coverage_cap).min(tier_cap)
}

pub fn calculate_required_collateral(
    position_size: u64,
    leverage: u64,
    coverage: u128,
) -> u64 {
    // Base collateral = position_size / leverage
    let base_collateral = position_size
        .checked_div(leverage)
        .unwrap_or(position_size);

    // Add safety margin based on coverage
    let safety_factor = if coverage > FIXED_PRECISION {
        10000 // 1.0 in basis points
    } else {
        // Lower coverage = higher safety margin
        let coverage_ratio = (coverage * 10000) / FIXED_PRECISION;
        20000u128
            .saturating_sub(coverage_ratio)
            .max(10000) as u64
    };

    base_collateral
        .saturating_mul(safety_factor)
        .checked_div(10000)
        .unwrap_or(base_collateral)
}

pub fn calculate_coverage(
    vault: u64,
    total_oi: u64,
    outcome_count: u32,
) -> u128 {
    if total_oi == 0 {
        return u128::MAX;
    }

    // Simplified tail loss calculation
    let tail_loss = calculate_tail_loss(outcome_count);
    
    (vault as u128 * FIXED_PRECISION) / (tail_loss * total_oi as u128)
}

pub fn calculate_tail_loss(outcome_count: u32) -> u128 {
    // Simplified tail loss calculation based on outcome count
    match outcome_count {
        1 => PRECISION,        // Binary: 100% tail loss
        2..=4 => PRECISION * 2,  // 200% tail loss
        5..=8 => PRECISION * 3,  // 300% tail loss
        _ => PRECISION * 4,      // 400% tail loss
    }
}

// AMM Price Calculations

pub fn calculate_lmsr_price(
    q_values: &[i64],
    b: u64,
    outcome: usize,
) -> Result<u64> {
    require!(outcome < q_values.len(), ErrorCode::InvalidOutcome);
    
    // LMSR price formula: p_i = exp(q_i/b) / Σ(exp(q_j/b))
    let mut sum_exp = FixedPoint::from_u64(0);
    
    for q in q_values.iter() {
        let exp_q = FixedPoint::from_i64(*q).div(&FixedPoint::from_u64(b))?;
        sum_exp = sum_exp.add(&exp_q.exp()?)?;
    }
    
    let exp_outcome = FixedPoint::from_i64(q_values[outcome])
        .div(&FixedPoint::from_u64(b))?
        .exp()?;
    
    let price = exp_outcome.div(&sum_exp)?;
    Ok((price.to_u64_truncate() * PRICE_PRECISION) / FIXED_POINT_PRECISION)
}

pub fn calculate_pmamm_price(
    current_prices: &[u64],
    amount: u64,
    liquidity: u64,
    time_elapsed: u64,
    total_time: u64,
) -> Result<u64> {
    // PM-AMM with time decay
    require!(current_prices.len() > 0, ErrorCode::InvalidInput);
    
    let avg_price = current_prices.iter().sum::<u64>() / current_prices.len() as u64;
    
    // Apply time decay factor
    let time_factor = if total_time > 0 {
        let remaining_time = total_time.saturating_sub(time_elapsed);
        (remaining_time * 10000) / total_time
    } else {
        10000
    };
    
    // Price impact based on amount and liquidity
    let impact = (amount * 10000) / liquidity.max(1);
    
    let adjusted_price = avg_price
        .saturating_mul(time_factor)
        .saturating_add(impact)
        .checked_div(10000)
        .unwrap_or(avg_price);
    
    Ok(adjusted_price)
}

pub fn calculate_l2_price(
    prices: &[u64],
    outcome: usize,
    amount: u64,
) -> Result<u64> {
    require!(outcome < prices.len(), ErrorCode::InvalidOutcome);
    
    // L2 norm-based pricing for continuous distributions
    let base_price = prices[outcome];
    
    // Apply L2 norm adjustment
    let mut norm_squared = 0u128;
    for price in prices {
        norm_squared = norm_squared.saturating_add((*price as u128).pow(2));
    }
    
    let norm = (norm_squared as f64).sqrt() as u64;
    
    if norm > 0 {
        let normalized_price = (base_price * PRICE_PRECISION) / norm;
        Ok(normalized_price)
    } else {
        Ok(base_price)
    }
}

pub fn calculate_liquidation_price(
    entry_price: u64,
    leverage: u64,
    is_long: bool,
    coverage: u128,
) -> u64 {
    // Calculate maintenance margin based on coverage
    let maintenance_margin = if coverage > PRECISION {
        500 // 5% for high coverage
    } else {
        1000 // 10% for low coverage
    };
    
    let price_move = (entry_price * maintenance_margin) / (leverage * 10000);
    
    if is_long {
        entry_price.saturating_sub(price_move)
    } else {
        entry_price.saturating_add(price_move)
    }
}

// Close position function
pub fn close_position<'info>(
    ctx: Context<'_, '_, '_, 'info, OpenPosition<'info>>,
    position_index: u8,
) -> Result<()> {
    let user_map = &mut ctx.accounts.user_map;
    let price_cache = &ctx.accounts.price_cache;
    let clock = Clock::get()?;
    
    // Validate position exists
    require!(
        (position_index as usize) < user_map.positions.len(),
        ErrorCode::InvalidPosition
    );
    
    let position = user_map.positions.remove(position_index as usize);
    let current_price = price_cache.last_price;
    
    // Calculate P&L
    let price_diff = if position.is_long {
        current_price as i64 - position.entry_price as i64
    } else {
        position.entry_price as i64 - current_price as i64
    };
    
    let pnl = (price_diff * position.size as i64 * position.leverage as i64) / PRICE_PRECISION as i64;
    
    // Calculate return amount (collateral + pnl)
    let collateral = position.size / position.leverage;
    let return_amount = if pnl >= 0 {
        collateral.saturating_add(pnl as u64)
    } else {
        collateral.saturating_sub(pnl.abs() as u64)
    };
    
    // Transfer funds back to user
    if return_amount > 0 {
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.global_config.to_account_info(),
        };
        
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );
        
        token::transfer(cpi_ctx, return_amount)?;
    }
    
    // Update user map
    user_map.total_collateral = user_map.total_collateral.saturating_sub(collateral);
    user_map.realized_pnl = user_map.realized_pnl.saturating_add(pnl);
    user_map.last_update = clock.unix_timestamp;
    
    // Update global OI
    let global = &mut ctx.accounts.global_config;
    global.total_oi = global.total_oi.saturating_sub(position.size);
    
    // Emit event
    emit!(PositionClosedEvent {
        user: ctx.accounts.user.key(),
        verse_id: ctx.accounts.verse.verse_id_as_u128(),
        amount: position.size,
        exit_price: current_price,
        pnl,
    });
    
    Ok(())
}

// Helper constant
const FIXED_POINT_PRECISION: u64 = 1_000_000_000;