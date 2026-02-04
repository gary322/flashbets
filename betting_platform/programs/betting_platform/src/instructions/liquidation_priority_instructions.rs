use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::*;
use crate::liquidation_priority::*;
use crate::fixed_types::U64F64;
use crate::errors::*;

#[derive(Accounts)]
pub struct InitializeLiquidationQueue<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 1000 * 128 + 100 * 96 + 64 + 40 + 8 + 8,
        seeds = [b"liquidation_queue", verse.key().as_ref()],
        bump
    )]
    pub liquidation_queue: Account<'info, LiquidationQueue>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdateAtRiskPosition<'info> {
    #[account(
        mut,
        seeds = [b"liquidation_queue", verse.key().as_ref()],
        bump
    )]
    pub liquidation_queue: Account<'info, LiquidationQueue>,
    
    pub position: Account<'info, Position>,
    pub verse: Account<'info, Verse>,
    pub keeper: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ProcessLiquidation<'info> {
    #[account(
        mut,
        seeds = [b"liquidation_queue", verse.key().as_ref()],
        bump
    )]
    pub liquidation_queue: Account<'info, LiquidationQueue>,
    
    #[account(mut)]
    pub position: Account<'info, Position>,
    
    #[account(mut)]
    pub position_owner: Account<'info, User>,
    
    #[account(mut)]
    pub keeper: Signer<'info>,
    
    #[account(mut)]
    pub keeper_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    
    pub verse: Account<'info, Verse>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ClaimKeeperRewards<'info> {
    #[account(
        mut,
        seeds = [b"liquidation_queue", verse.key().as_ref()],
        bump
    )]
    pub liquidation_queue: Account<'info, LiquidationQueue>,
    
    #[account(mut)]
    pub keeper: Signer<'info>,
    
    #[account(mut)]
    pub keeper_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub rewards_vault: Account<'info, TokenAccount>,
    
    pub verse: Account<'info, Verse>,
    pub token_program: Program<'info, Token>,
}

// Mock structs for compilation - these should be replaced with actual implementations
#[account]
pub struct Position {
    pub position_id: [u8; 32],
    pub owner: Pubkey,
    pub market_id: [u8; 32],
    pub verse_id: [u8; 32],
    pub size: u64,
    pub entry_price: u64,
    pub leverage: u64,
    pub is_long: bool,
    pub owner_mmt_staked: u64,
    pub bootstrap_tier: u8,
    pub last_update_slot: u64,
    pub is_chained: bool,
    pub chain_depth: u8,
}

#[account]
pub struct User {
    pub pubkey: Pubkey,
    pub mmt_staked: u64,
}

// Instruction implementations
pub fn initialize_liquidation_queue(ctx: Context<InitializeLiquidationQueue>) -> Result<()> {
    let queue = &mut ctx.accounts.liquidation_queue;
    
    queue.queue_id = Pubkey::new_unique().to_bytes();
    queue.at_risk_positions = Vec::new();
    queue.active_liquidations = Vec::new();
    
    // Initialize config with CLAUDE.md parameters
    queue.config = LiquidationConfig {
        min_liquidation_size: 10 * 10u64.pow(6), // $10 minimum
        max_liquidation_per_slot: U64F64::from_num(0.08), // 8% max
        liquidation_penalty_bps: 50, // 5bp to keeper
        grace_period: 180, // ~1 minute grace
        staking_tier_boost: [0, 10, 20, 30, 50], // Priority boost per tier
        bootstrap_protection_multiplier: U64F64::from_num(1.5), // 50% more time
    };
    
    queue.metrics = LiquidationMetrics::default();
    queue.keeper_rewards_pool = 0;
    queue.last_update_slot = ctx.accounts.clock.slot;
    
    msg!("Liquidation queue initialized");
    msg!("Max liquidation per slot: 8%");
    msg!("Keeper reward: 5bp");
    
    Ok(())
}

pub fn update_at_risk_position(
    ctx: Context<UpdateAtRiskPosition>,
    mark_price: u64,
) -> Result<()> {
    let queue = &mut ctx.accounts.liquidation_queue;
    let position = &ctx.accounts.position;
    let clock = &ctx.accounts.clock;
    
    // Calculate risk metrics
    let mark_price_fp = U64F64::from_num(mark_price) / U64F64::from_num(10u64.pow(6));
    let entry_price_fp = U64F64::from_num(position.entry_price) / U64F64::from_num(10u64.pow(6));
    
    // Calculate distance to liquidation
    let liquidation_price = if position.is_long {
        entry_price_fp * (U64F64::one() - U64F64::one() / U64F64::from_num(position.leverage))
    } else {
        entry_price_fp * (U64F64::one() + U64F64::one() / U64F64::from_num(position.leverage))
    };
    
    let distance = if position.is_long {
        (mark_price_fp - liquidation_price) / mark_price_fp
    } else {
        (liquidation_price - mark_price_fp) / mark_price_fp
    };
    
    // Only track if within 20% of liquidation
    if distance < U64F64::from_num(0.2) {
        let risk_score = LiquidationEngine::calculate_risk_score(
            mark_price_fp,
            entry_price_fp,
            U64F64::from_num(position.leverage),
            position.is_long,
        );
        
        let at_risk = AtRiskPosition {
            position_id: position.position_id,
            owner: position.owner,
            market_id: position.market_id,
            size: position.size,
            entry_price: entry_price_fp,
            mark_price: mark_price_fp,
            effective_leverage: U64F64::from_num(position.leverage),
            distance_to_liquidation: distance,
            risk_score,
            staking_tier: get_staking_tier(position.owner_mmt_staked),
            bootstrap_priority: position.bootstrap_tier,
            time_at_risk: clock.slot - position.last_update_slot,
            is_chained: position.is_chained,
            chain_depth: position.chain_depth,
        };
        
        LiquidationEngine::add_at_risk_position(queue, at_risk)?;
        
        msg!("Position added to at-risk queue");
        msg!("Risk score: {}", risk_score);
        msg!("Distance to liquidation: {}%", (distance * U64F64::from_num(100)).to_num::<u16>());
    } else {
        // Remove from queue if no longer at risk
        LiquidationEngine::remove_position(queue, position.position_id)?;
    }
    
    Ok(())
}

pub fn process_liquidation(
    ctx: Context<ProcessLiquidation>,
    max_liquidations: u64,
) -> Result<()> {
    let queue = &mut ctx.accounts.liquidation_queue;
    let clock = &ctx.accounts.clock;
    
    // Get liquidation orders
    let orders = LiquidationEngine::process_queue(
        queue,
        max_liquidations,
        clock.slot,
    )?;
    
    msg!("Processing {} liquidations", orders.len());
    
    // Execute liquidations
    for order in orders {
        // Verify this position matches
        if ctx.accounts.position.position_id != order.position_id {
            continue;
        }
        
        // Process the liquidation
        LiquidationEngine::process_keeper_liquidation(
            queue,
            &order,
            ctx.accounts.keeper.key(),
            clock.slot,
        )?;
        
        // Update position
        let position = &mut ctx.accounts.position;
        position.size = position.size.saturating_sub(order.liquidation_amount);
        
        // Transfer keeper reward
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.keeper_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        
        token::transfer(cpi_ctx, order.keeper_reward)?;
        
        msg!("Liquidated {} at price {}", order.liquidation_amount, order.liquidation_price);
        msg!("Keeper reward: {}", order.keeper_reward);
        
        // Emit event
        emit!(LiquidationEvent {
            position_id: order.position_id,
            owner: order.owner,
            liquidation_amount: order.liquidation_amount,
            liquidation_price: order.liquidation_price,
            keeper: ctx.accounts.keeper.key(),
            keeper_reward: order.keeper_reward,
            slot: clock.slot,
        });
    }
    
    Ok(())
}

pub fn claim_keeper_rewards(ctx: Context<ClaimKeeperRewards>) -> Result<()> {
    let queue = &ctx.accounts.liquidation_queue;
    let keeper = ctx.accounts.keeper.key();
    
    // Calculate total rewards for this keeper
    let mut total_rewards = 0u64;
    for liquidation in &queue.active_liquidations {
        if liquidation.keeper == keeper && liquidation.status == LiquidationStatus::Completed {
            // Add reward calculation based on completed liquidations
            total_rewards += liquidation.amount_liquidated * 5 / 10_000; // 5bp
        }
    }
    
    require!(
        total_rewards > 0,
        crate::errors::ErrorCode::NoRewardsToClaim
    );
    
    // Transfer rewards
    let cpi_accounts = Transfer {
        from: ctx.accounts.rewards_vault.to_account_info(),
        to: ctx.accounts.keeper_token_account.to_account_info(),
        authority: ctx.accounts.rewards_vault.to_account_info(),
    };
    
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    
    token::transfer(cpi_ctx, total_rewards)?;
    
    msg!("Keeper rewards claimed: {}", total_rewards);
    
    Ok(())
}

// Events
#[event]
pub struct LiquidationEvent {
    pub position_id: [u8; 32],
    pub owner: Pubkey,
    pub liquidation_amount: u64,
    pub liquidation_price: U64F64,
    pub keeper: Pubkey,
    pub keeper_reward: u64,
    pub slot: u64,
}