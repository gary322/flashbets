use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub mod types;
pub mod state;
pub mod instructions;
pub mod errors;
pub mod constants;
pub mod math;
pub mod bootstrap;
pub mod amm;
pub mod router;

pub use types::{U64F64, I64F64};

pub use state::*;
pub use instructions::*;
pub use errors::*;
pub use constants::*;
pub use math::*;
pub use bootstrap::*;
pub use amm::*;
pub use router::*;

#[program]
pub mod phase10_betting {
    use super::*;

    // Bootstrap Instructions
    pub fn initialize_bootstrap(ctx: Context<InitializeBootstrap>) -> Result<()> {
        instructions::bootstrap::initialize_bootstrap(ctx)
    }

    pub fn register_bootstrap_trader(ctx: Context<RegisterBootstrapTrader>) -> Result<()> {
        instructions::bootstrap::register_bootstrap_trader(ctx)
    }

    pub fn process_bootstrap_trade(
        ctx: Context<ProcessBootstrapTrade>,
        trade_volume: u64,
        leverage_used: u64,
    ) -> Result<()> {
        instructions::bootstrap::process_bootstrap_trade(ctx, trade_volume, leverage_used)
    }

    pub fn claim_bootstrap_rewards(ctx: Context<ClaimBootstrapRewards>) -> Result<()> {
        instructions::bootstrap::claim_bootstrap_rewards(ctx)
    }

    pub fn process_referral_bonus(
        ctx: Context<ProcessReferralBonus>,
        referred_trader: Pubkey,
        referred_volume: u64,
    ) -> Result<()> {
        instructions::bootstrap::process_referral_bonus(ctx, referred_trader, referred_volume)
    }

    pub fn process_bootstrap_milestone(
        ctx: Context<ProcessBootstrapMilestone>,
        milestone_index: u64,
        top_traders: Vec<(Pubkey, u64)>,
    ) -> Result<()> {
        instructions::bootstrap::process_bootstrap_milestone(ctx, milestone_index, top_traders)
    }

    // AMM Selector Instructions
    pub fn initialize_amm_selector(
        ctx: Context<InitializeAMMSelector>,
        market_type: MarketType,
        time_to_expiry: u64,
    ) -> Result<()> {
        instructions::amm::initialize_amm_selector(ctx, market_type, time_to_expiry)
    }

    pub fn update_amm_metrics(
        ctx: Context<UpdateAMMMetrics>,
        trade_volume: u64,
        slippage_bps: u16,
        lvr_amount: u64,
    ) -> Result<()> {
        instructions::amm::update_amm_metrics(ctx, trade_volume, slippage_bps, lvr_amount)
    }

    pub fn switch_amm_type(
        ctx: Context<SwitchAMMType>,
        new_amm_type: AMMType,
        current_liquidity: u64,
    ) -> Result<()> {
        instructions::amm::switch_amm_type(ctx, new_amm_type, current_liquidity)
    }

    // Synthetic Router Instructions
    pub fn initialize_synthetic_router(
        ctx: Context<InitializeSyntheticRouter>,
        verse_id: [u8; 32],
        routing_strategy: RoutingStrategy,
    ) -> Result<()> {
        instructions::router::initialize_synthetic_router(ctx, verse_id, routing_strategy)
    }

    pub fn add_child_market_to_router(
        ctx: Context<AddChildMarket>,
        market_id: String,
        initial_probability: u64,
        volume_7d: u64,
        liquidity_depth: u64,
    ) -> Result<()> {
        instructions::router::add_child_market_to_router(
            ctx,
            market_id,
            initial_probability,
            volume_7d,
            liquidity_depth,
        )
    }

    pub fn execute_synthetic_route(
        ctx: Context<ExecuteSyntheticRoute>,
        trade_size: u64,
        is_buy: bool,
        max_slippage_bps: u16,
    ) -> Result<()> {
        instructions::router::execute_synthetic_route(ctx, trade_size, is_buy, max_slippage_bps)
    }

    pub fn update_router_weights(ctx: Context<UpdateRouterWeights>) -> Result<()> {
        instructions::router::update_router_weights(ctx)
    }
}