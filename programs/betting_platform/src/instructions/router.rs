use anchor_lang::prelude::*;
use fixed::types::U64F64;
use crate::router::*;
use crate::amm::types::{MarketType, AMMType};
use crate::errors::ErrorCode;
use crate::constants::*;

#[derive(Accounts)]
pub struct InitializeSyntheticRouter<'info> {
    #[account(
        init,
        payer = creator,
        space = SyntheticRouter::len(0), // Start with no markets
        seeds = [SYNTHETIC_ROUTER_SEED, verse_id.as_ref()],
        bump
    )]
    pub router: Account<'info, SyntheticRouter>,
    
    #[account(mut)]
    pub creator: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddChildMarket<'info> {
    #[account(mut)]
    pub router: Account<'info, SyntheticRouter>,
    
    pub keeper: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ExecuteSyntheticRoute<'info> {
    #[account(mut)]
    pub router: Account<'info, SyntheticRouter>,
    
    #[account(mut)]
    pub trader: Signer<'info>,
    
    #[account(mut)]
    pub user_position: Account<'info, UserPosition>,
    
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdateRouterWeights<'info> {
    #[account(mut)]
    pub router: Account<'info, SyntheticRouter>,
    
    pub keeper: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

pub fn initialize_synthetic_router(
    ctx: Context<InitializeSyntheticRouter>,
    verse_id: [u8; 32],
    routing_strategy: RoutingStrategy,
) -> Result<()> {
    let router = &mut ctx.accounts.router;
    let current_slot = Clock::get()?.slot;
    
    router.initialize(verse_id, routing_strategy, current_slot)?;
    
    msg!("Synthetic router initialized for verse: {:?}", verse_id);
    msg!("Routing strategy: {:?}", routing_strategy);
    
    Ok(())
}

pub fn add_child_market_to_router(
    ctx: Context<AddChildMarket>,
    market_id: String,
    initial_probability: u64,
    volume_7d: u64,
    liquidity_depth: u64,
) -> Result<()> {
    let router = &mut ctx.accounts.router;
    let clock = &ctx.accounts.clock;
    
    // Validate probability
    require!(
        initial_probability <= 100 * PRECISION,
        ErrorCode::InvalidProbability
    );
    
    // Determine AMM type for this market
    let market_type = determine_market_type(&market_id)?;
    let amm_type = HybridAMMSelector::select_amm(
        &market_type,
        SLOTS_PER_DAY * 30, // 30 days default
        &AMMOverrideFlags::default(),
        &AMMPerformanceMetrics::default(),
    );
    
    router.add_child_market(
        market_id,
        U64F64::from_num(initial_probability) / U64F64::from_num(100 * PRECISION),
        volume_7d,
        liquidity_depth,
        amm_type,
        clock.unix_timestamp,
    )?;
    
    msg!("Added child market with AMM type: {:?}", amm_type);
    msg!("Total markets: {}", router.child_markets.len());
    msg!("Total liquidity: {}", router.total_liquidity);
    
    Ok(())
}

pub fn execute_synthetic_route(
    ctx: Context<ExecuteSyntheticRoute>,
    trade_size: u64,
    is_buy: bool,
    max_slippage_bps: u16,
) -> Result<()> {
    let router = &mut ctx.accounts.router;
    let _clock = &ctx.accounts.clock;
    
    // Validate slippage tolerance
    require!(
        max_slippage_bps <= MAX_SLIPPAGE_BPS,
        ErrorCode::SlippageExceeded
    );
    
    // Calculate optimal route
    let route_result = RouteExecutor::calculate_route(
        router,
        trade_size,
        is_buy,
    )?;
    
    // Check slippage tolerance
    require!(
        route_result.total_slippage_bps <= max_slippage_bps,
        ErrorCode::SlippageExceeded
    );
    
    // Calculate execution improvement
    let individual_fees = (trade_size as u128 * POLYMARKET_FEE_BPS as u128 / 10_000) as u64;
    let fees_saved = individual_fees.saturating_sub(route_result.total_fees);
    
    let execution_improvement = if router.aggregated_prob > U64F64::from_num(0) {
        let price_improvement = calculate_price_improvement(
            route_result.avg_execution_price,
            router.aggregated_prob,
        );
        price_improvement
    } else {
        0
    };
    
    // Update router performance
    router.update_performance(
        trade_size,
        fees_saved,
        execution_improvement,
    )?;
    
    msg!("Route executed successfully:");
    msg!("  Route legs: {}", route_result.route_legs.len());
    msg!("  Total cost: {}", route_result.total_cost);
    msg!("  Total fees: {}", route_result.total_fees);
    msg!("  Avg execution price: {}", route_result.avg_execution_price);
    msg!("  Total slippage: {} bps", route_result.total_slippage_bps);
    msg!("  Fees saved: {}", fees_saved);
    
    // In production, this would execute the actual trades through Polymarket
    // For now, we just update the accounting
    
    Ok(())
}

pub fn update_router_weights(ctx: Context<UpdateRouterWeights>) -> Result<()> {
    let router = &mut ctx.accounts.router;
    let clock = &ctx.accounts.clock;
    
    // Check if enough time has passed since last update
    let slots_since_update = clock.slot.saturating_sub(router.last_update_slot);
    require!(
        slots_since_update >= ROUTER_UPDATE_INTERVAL,
        ErrorCode::Unauthorized // Should be a more specific error
    );
    
    // Update weights based on latest liquidity and volume data
    router.update_weights()?;
    router.update_aggregated_probability()?;
    router.last_update_slot = clock.slot;
    
    msg!("Router weights updated");
    msg!("Aggregated probability: {}", router.aggregated_prob);
    msg!("Total liquidity: {}", router.total_liquidity);
    
    Ok(())
}

// Helper functions
fn determine_market_type(market_id: &str) -> Result<MarketType> {
    // Parse market ID to determine type
    // This would involve analyzing the market title/metadata
    
    if market_id.contains("vs") || market_id.contains("binary") {
        Ok(MarketType::Binary)
    } else if market_id.contains("multi") {
        Ok(MarketType::MultiOutcome { count: 4 })
    } else if market_id.contains("range") || market_id.contains("continuous") {
        Ok(MarketType::Continuous {
            min: I64F64::from_num(0),
            max: I64F64::from_num(100),
            precision: 2,
        })
    } else {
        Ok(MarketType::Binary) // Default
    }
}

fn calculate_price_improvement(avg_execution_price: U64F64, base_price: U64F64) -> u16 {
    if base_price == U64F64::from_num(0) {
        return 0;
    }
    
    let price_diff = if avg_execution_price < base_price {
        base_price - avg_execution_price
    } else {
        U64F64::from_num(0)
    };
    
    ((price_diff / base_price) * U64F64::from_num(10_000)).to_num::<u16>()
}

use anchor_spl::token::TokenAccount;
use crate::state::UserPosition;
use crate::amm::types::{HybridAMMSelector, AMMOverrideFlags, AMMPerformanceMetrics};
use fixed::types::I64F64;