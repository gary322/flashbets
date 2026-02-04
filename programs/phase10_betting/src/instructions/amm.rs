use anchor_lang::prelude::*;
use crate::types::U64F64;
use crate::amm::types::*;
use crate::errors::ErrorCode;
use crate::constants::*;

#[derive(Accounts)]
#[instruction(market_type: MarketType)]
pub struct InitializeAMMSelector<'info> {
    #[account(
        init,
        payer = trader,
        space = HybridAMMSelector::LEN,
        seeds = [AMM_SELECTOR_SEED, trader.key().as_ref()],
        bump
    )]
    pub amm_selector: Account<'info, HybridAMMSelector>,
    
    #[account(mut)]
    pub trader: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAMMMetrics<'info> {
    #[account(mut)]
    pub amm_selector: Account<'info, HybridAMMSelector>,
    
    pub trader: Signer<'info>,
}

#[derive(Accounts)]
pub struct SwitchAMMType<'info> {
    #[account(mut)]
    pub amm_selector: Account<'info, HybridAMMSelector>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

pub fn initialize_amm_selector(
    ctx: Context<InitializeAMMSelector>,
    market_type: MarketType,
    time_to_expiry: u64,
) -> Result<()> {
    let selector = &mut ctx.accounts.amm_selector;
    let market_id = Pubkey::new_unique().to_bytes();
    let current_slot = Clock::get()?.slot;
    
    selector.initialize(
        market_id,
        market_type,
        time_to_expiry,
        current_slot,
    )?;
    
    msg!("AMM selector initialized");
    msg!("Market type: {:?}", market_type);
    msg!("Selected AMM: {:?}", selector.amm_type);
    msg!("Time to expiry: {} slots", time_to_expiry);
    
    Ok(())
}

pub fn update_amm_metrics(
    ctx: Context<UpdateAMMMetrics>,
    trade_volume: u64,
    slippage_bps: u16,
    lvr_amount: u64,
) -> Result<()> {
    let selector = &mut ctx.accounts.amm_selector;
    
    selector.update_metrics(
        trade_volume,
        slippage_bps,
        U64F64::from_num(lvr_amount),
    )?;
    
    // Check if we should recommend switching AMM type
    if selector.performance_metrics.efficiency_score < 70 {
        msg!("Warning: AMM efficiency below 70%, consider switching");
        
        let new_amm = HybridAMMSelector::select_amm(
            &selector.market_type,
            selector.time_to_expiry,
            &selector.override_flags,
            &selector.performance_metrics,
        );
        
        if new_amm != selector.amm_type {
            msg!("Recommended AMM switch: {:?} -> {:?}", selector.amm_type, new_amm);
        }
    }
    
    msg!("AMM metrics updated:");
    msg!("  Total volume: {}", selector.performance_metrics.total_volume);
    msg!("  Avg slippage: {} bps", selector.performance_metrics.avg_slippage_bps);
    msg!("  Efficiency score: {}", selector.performance_metrics.efficiency_score);
    
    Ok(())
}

pub fn switch_amm_type(
    ctx: Context<SwitchAMMType>,
    new_amm_type: AMMType,
    current_liquidity: u64,
) -> Result<()> {
    let selector = &mut ctx.accounts.amm_selector;
    let clock = &ctx.accounts.clock;
    
    // Check if transition is already in progress
    if let Some(transition) = &selector.transition_state {
        require!(
            transition.progress == 100,
            ErrorCode::AMMTransitionInProgress
        );
    }
    
    // Calculate switching cost
    let switching_cost = selector.calculate_switching_cost(new_amm_type, current_liquidity)?;
    
    if switching_cost > 0 {
        // Initialize transition
        selector.transition_state = Some(AMMTransition {
            from_amm: selector.amm_type,
            to_amm: new_amm_type,
            start_slot: clock.slot,
            end_slot: clock.slot + 3600, // 1 hour transition period
            progress: 0,
        });
        
        msg!("AMM type transition initiated");
        msg!("From: {:?} to {:?}", selector.amm_type, new_amm_type);
        msg!("Estimated switching cost: {}", switching_cost);
    } else {
        selector.amm_type = new_amm_type;
        msg!("AMM type switched instantly to {:?}", new_amm_type);
    }
    
    Ok(())
}