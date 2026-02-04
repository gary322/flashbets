use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use crate::types::U64F64;
use crate::state::*;
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::bootstrap::{BootstrapIncentiveEngine, MilestoneManager};

#[derive(Accounts)]
pub struct InitializeBootstrap<'info> {
    #[account(
        init,
        payer = admin,
        space = BootstrapState::LEN,
        seeds = [BOOTSTRAP_STATE_SEED],
        bump
    )]
    pub bootstrap_state: Account<'info, BootstrapState>,
    
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct RegisterBootstrapTrader<'info> {
    #[account(
        init,
        payer = trader,
        space = BootstrapTrader::LEN,
        seeds = [BOOTSTRAP_TRADER_SEED, trader.key().as_ref()],
        bump
    )]
    pub trader_state: Account<'info, BootstrapTrader>,
    
    #[account(mut)]
    pub bootstrap_state: Account<'info, BootstrapState>,
    
    #[account(mut)]
    pub trader: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProcessBootstrapTrade<'info> {
    #[account(mut)]
    pub bootstrap_state: Account<'info, BootstrapState>,
    
    #[account(mut)]
    pub trader_state: Account<'info, BootstrapTrader>,
    
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub trader: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ClaimBootstrapRewards<'info> {
    #[account(mut)]
    pub bootstrap_state: Account<'info, BootstrapState>,
    
    #[account(mut)]
    pub trader_state: Account<'info, BootstrapTrader>,
    
    #[account(mut)]
    pub mmt_mint: Account<'info, Mint>,
    
    #[account(
        init_if_needed,
        payer = trader,
        associated_token::mint = mmt_mint,
        associated_token::authority = trader,
    )]
    pub trader_mmt_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub mmt_treasury: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub trader: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProcessBootstrapMilestone<'info> {
    #[account(mut)]
    pub bootstrap_state: Account<'info, BootstrapState>,
    
    #[account(mut)]
    pub milestone: Account<'info, BootstrapMilestone>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ProcessReferralBonus<'info> {
    #[account(mut)]
    pub referrer_state: Account<'info, BootstrapTrader>,
    
    pub bootstrap_state: Account<'info, BootstrapState>,
    
    pub referrer: Signer<'info>,
}

// Bootstrap instructions implementation
pub fn initialize_bootstrap(ctx: Context<InitializeBootstrap>) -> Result<()> {
    let bootstrap_state = &mut ctx.accounts.bootstrap_state;
    let clock = &ctx.accounts.clock;
    
    bootstrap_state.init(clock)?;
    
    // Update global state to indicate bootstrap is active
    ctx.accounts.global_state.bootstrap_active = true;
    ctx.accounts.global_state.bootstrap_start_slot = clock.slot;
    
    msg!("Bootstrap initialized with 2M MMT allocation");
    msg!("Early trader bonus: 2x for first 100 traders");
    msg!("Target coverage: 100% to end bootstrap");
    
    Ok(())
}

pub fn register_bootstrap_trader(ctx: Context<RegisterBootstrapTrader>) -> Result<()> {
    let trader_state = &mut ctx.accounts.trader_state;
    let trader = ctx.accounts.trader.key();
    
    trader_state.trader = trader;
    trader_state.volume_traded = 0;
    trader_state.mmt_earned = 0;
    trader_state.trade_count = 0;
    trader_state.is_early_trader = false;
    trader_state.first_trade_slot = 0;
    trader_state.avg_leverage = U64F64::zero();
    trader_state.vault_contribution = 0;
    trader_state.referral_bonus = 0;
    trader_state.referred_count = 0;
    
    msg!("Bootstrap trader registered: {}", trader);
    
    Ok(())
}

pub fn process_bootstrap_trade(
    ctx: Context<ProcessBootstrapTrade>,
    trade_volume: u64,
    leverage_used: u64,
) -> Result<()> {
    let bootstrap_state = &mut ctx.accounts.bootstrap_state;
    let trader_state = &mut ctx.accounts.trader_state;
    let global_state = &ctx.accounts.global_state;
    let clock = &ctx.accounts.clock;
    
    // Verify bootstrap is active
    require!(
        bootstrap_state.status == BootstrapStatus::Active,
        ErrorCode::BootstrapNotActive
    );
    
    // Verify minimum trade size
    require!(
        trade_volume >= bootstrap_state.min_trade_size,
        ErrorCode::TradeTooSmall
    );
    
    // Calculate fee
    let fee_bps = bootstrap_state.calculate_bootstrap_fee();
    let fee_amount = (trade_volume as u128 * fee_bps as u128) / 10_000;
    
    // Process the trade
    let result = BootstrapIncentiveEngine::process_bootstrap_trade(
        bootstrap_state,
        trader_state,
        trade_volume,
        fee_amount as u64,
        U64F64::from_num(leverage_used),
        clock,
    )?;
    
    // Update coverage
    let total_oi = global_state.total_open_interest;
    bootstrap_state.current_coverage = BootstrapIncentiveEngine::calculate_bootstrap_coverage(
        bootstrap_state.current_vault_balance,
        total_oi,
        true,
    );
    
    // Check if bootstrap should end
    if bootstrap_state.should_end_bootstrap(clock) {
        bootstrap_state.status = BootstrapStatus::Completed;
        msg!("Bootstrap phase completed!");
    }
    
    msg!("Bootstrap trade processed:");
    msg!("  MMT reward: {}", result.mmt_reward);
    msg!("  Fee rebate: {}", result.fee_rebate);
    msg!("  New coverage: {}", result.new_coverage);
    msg!("  Is early trader: {}", result.is_early_trader);
    
    Ok(())
}

pub fn claim_bootstrap_rewards(ctx: Context<ClaimBootstrapRewards>) -> Result<()> {
    let trader_state = &ctx.accounts.trader_state;
    let _bootstrap_state = &mut ctx.accounts.bootstrap_state;
    
    // Calculate total claimable rewards
    let total_rewards = trader_state.mmt_earned + trader_state.referral_bonus;
    
    require!(
        total_rewards > 0,
        ErrorCode::NoRewardsToClaim
    );
    
    // Transfer MMT rewards
    let cpi_accounts = Transfer {
        from: ctx.accounts.mmt_treasury.to_account_info(),
        to: ctx.accounts.trader_mmt_account.to_account_info(),
        authority: ctx.accounts.mmt_treasury.to_account_info(),
    };
    
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    
    token::transfer(cpi_ctx, total_rewards)?;
    
    msg!("Bootstrap rewards claimed: {} MMT", total_rewards);
    
    Ok(())
}

pub fn process_referral_bonus(
    ctx: Context<ProcessReferralBonus>,
    _referred_trader: Pubkey,
    referred_volume: u64,
) -> Result<()> {
    let referrer_state = &mut ctx.accounts.referrer_state;
    let bootstrap_state = &ctx.accounts.bootstrap_state;
    
    let bonus = BootstrapIncentiveEngine::process_referral(
        referrer_state,
        referred_volume,
        bootstrap_state,
    )?;
    
    msg!("Referral bonus processed: {} MMT", bonus);
    msg!("Total referred traders: {}", referrer_state.referred_count);
    
    Ok(())
}

pub fn process_bootstrap_milestone(
    ctx: Context<ProcessBootstrapMilestone>,
    _milestone_index: u64,
    top_traders: Vec<(Pubkey, u64)>,
) -> Result<()> {
    let bootstrap_state = &mut ctx.accounts.bootstrap_state;
    let milestone = &mut ctx.accounts.milestone;
    let clock = &ctx.accounts.clock;
    
    let achieved = MilestoneManager::check_and_process_milestone(
        bootstrap_state,
        milestone,
        top_traders,
        clock,
    )?;
    
    if achieved {
        msg!("Milestone {} achieved!", milestone.index);
        msg!("Vault target: {}", milestone.vault_target);
        msg!("Coverage target: {}", milestone.coverage_target);
        msg!("Traders target: {}", milestone.traders_target);
    }
    
    Ok(())
}