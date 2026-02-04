use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::{self, Token};
pub use spl_token;
use spl_token::instruction::AuthorityType;

declare_id!("Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4");

// Re-export all modules first
pub mod account_structs;
pub mod advanced_orders;
pub mod amm;
pub mod amm_verification;
pub mod attack_detection;
pub mod chain_execution;
pub mod chain_safety;
pub mod chain_state;
pub mod chain_unwind;
pub mod circuit_breaker;
pub mod contexts;
pub mod dark_pool;
pub mod deployment;
pub mod errors;
pub mod events;
pub mod fees;
pub mod fixed_math;
pub mod fixed_types;
pub mod hybrid_amm;
pub mod iceberg_orders;
pub mod instructions;
pub mod keeper_health;
pub mod keeper_network;
pub mod l2_amm;
pub mod liquidation;
pub mod liquidation_priority;
pub mod lmsr_amm;
pub mod math;
pub mod merkle;
pub mod performance;
pub mod pm_amm;
pub mod price_cache;
pub mod quantum;
pub mod resolution;
pub mod safety;
pub mod sharding;
pub mod state;
pub mod state_compression;
pub mod state_pruning;
pub mod state_traversal;
pub mod trading;
pub mod twap_orders;
pub mod validation;
pub mod verification;
pub mod verse_classifier;

#[cfg(test)]
pub mod test_runner;

#[cfg(test)]
pub mod tests;

// Import types before program module
use crate::account_structs::*;
use crate::contexts::*;
use crate::errors::ErrorCode;
use crate::events::*;

// Re-exports for external use
pub use account_structs::*;
pub use advanced_orders::OrderSide;
pub use chain_execution::AutoChain;
pub use chain_state::ChainStepType;
pub use chain_unwind::UnwindChain;
pub use contexts::*;
pub use dark_pool::{InitializeDarkPool, PlaceDarkOrder, MatchDarkPool, TimeInForce};
// pub use errors::ErrorCode; // Already imported above
pub use events::*;
pub use fees::DistributeFees;
pub use hybrid_amm::{InitializeHybridAMM, HybridTrade, AMMType};
pub use iceberg_orders::{PlaceIcebergOrder, ExecuteIcebergFill};
pub use instructions::attack_detection_instructions::{
    InitializeAttackDetector, ProcessTrade, UpdateVolumeBaseline, ResetDetector
};
pub use instructions::circuit_breaker_instructions::{
    InitializeCircuitBreaker, CheckBreakers, EmergencyShutdown, UpdateBreakerConfig
};
pub use instructions::liquidation_priority_instructions::{
    InitializeLiquidationQueue, UpdateAtRiskPosition, ProcessLiquidation, ClaimKeeperRewards
};
pub use keeper_health::*;
pub use l2_amm::{InitializeL2AMM, L2AMMTrade, DistributionType};
pub use liquidation::PartialLiquidate;
pub use lmsr_amm::{InitializeLSMR, LSMRTrade};
pub use pm_amm::{InitializePMAMM, PMAMMTrade};
pub use price_cache::*;
pub use resolution::*;
pub use safety::{CheckCircuitBreakers, MonitorHealth};
pub use trading::{OpenPositionParams, OpenPosition, ClosePosition};
pub use twap_orders::{PlaceTWAPOrder, ExecuteTWAPInterval};

#[program]
pub mod betting_platform {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, _seed: u128) -> Result<()> {
        let global_config = &mut ctx.accounts.global_config;
        global_config.epoch = 1;
        global_config.coverage = u128::MAX; // Start with infinite coverage
        global_config.vault = 0; // $0 bootstrap
        global_config.total_oi = 0;
        global_config.halt_flag = false;
        global_config.fee_base = 300; // 3bp in basis points (0.03%)
        global_config.fee_slope = 2500; // 25bp
        Ok(())
    }

    pub fn initialize_genesis(ctx: Context<InitializeGenesis>) -> Result<()> {
        let global_config = &mut ctx.accounts.global_config;
        let clock = Clock::get()?;

        // Set genesis parameters
        global_config.epoch = 1;
        global_config.season = 1;
        global_config.vault = 0; // $0 bootstrap
        global_config.total_oi = 0;
        global_config.coverage = u128::MAX; // Infinite coverage initially
        global_config.fee_base = 300; // 3bp in basis points (0.03%)
        global_config.fee_slope = 2500; // 25bp
        global_config.halt_flag = false;
        global_config.genesis_slot = clock.slot;
        global_config.season_start_slot = clock.slot;
        global_config.season_end_slot = clock.slot + 38_880_000; // ~6 months

        // MMT configuration
        global_config.mmt_total_supply = 100_000_000 * 10u64.pow(9); // 100M with 9 decimals
        global_config.mmt_current_season = 10_000_000 * 10u64.pow(9); // 10M for current season
        global_config.mmt_emission_rate = global_config.mmt_current_season / 38_880_000; // Per slot

        emit!(GenesisEvent {
            slot: clock.slot,
            epoch: global_config.epoch,
            season: global_config.season,
        });

        Ok(())
    }

    pub fn initialize_mmt(ctx: Context<InitializeMmt>) -> Result<()> {
        // Initialize mint
        token::initialize_mint(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeMint {
                    mint: ctx.accounts.mmt_mint.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            9, // decimals
            &ctx.accounts.mint_authority.key(),
            Some(&ctx.accounts.mint_authority.key()),
        )?;

        // Initialize treasury token account
        token::initialize_account(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeAccount {
                    account: ctx.accounts.treasury.to_account_info(),
                    mint: ctx.accounts.mmt_mint.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
        )?;

        // Mint total supply to treasury
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mmt_mint.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                &[&[b"mint_authority", &[ctx.bumps.mint_authority]]],
            ),
            100_000_000 * 10u64.pow(9), // 100M MMT
        )?;

        // Burn mint authority
        token::set_authority(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::SetAuthority {
                    account_or_mint: ctx.accounts.mmt_mint.to_account_info(),
                    current_authority: ctx.accounts.mint_authority.to_account_info(),
                },
                &[&[b"mint_authority", &[ctx.bumps.mint_authority]]],
            ),
            AuthorityType::MintTokens,
            None, // Burn authority
        )?;

        Ok(())
    }

    pub fn genesis_atomic(ctx: Context<GenesisAtomic>) -> Result<()> {
        // Initialize genesis parameters directly
        let global_config = &mut ctx.accounts.global_config;
        let clock = Clock::get()?;

        // Set genesis parameters
        global_config.epoch = 1;
        global_config.season = 1;
        global_config.vault = 0;
        global_config.total_oi = 0;
        global_config.coverage = u128::MAX;
        global_config.fee_base = 300;
        global_config.fee_slope = 2500;
        global_config.halt_flag = false;
        global_config.genesis_slot = clock.slot;
        global_config.season_start_slot = clock.slot;
        global_config.season_end_slot = clock.slot + 38_880_000;

        // MMT configuration
        global_config.mmt_total_supply = 100_000_000 * 10u64.pow(9);
        global_config.mmt_current_season = 10_000_000 * 10u64.pow(9);
        global_config.mmt_emission_rate = global_config.mmt_current_season / 38_880_000;

        emit!(GenesisEvent {
            slot: clock.slot,
            epoch: global_config.epoch,
            season: global_config.season,
        });

        // Initialize MMT mint
        token::initialize_mint(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeMint {
                    mint: ctx.accounts.mmt_mint.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            9, // decimals
            &ctx.accounts.mint_authority.key(),
            Some(&ctx.accounts.mint_authority.key()),
        )?;

        // Initialize treasury token account
        token::initialize_account(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeAccount {
                    account: ctx.accounts.treasury.to_account_info(),
                    mint: ctx.accounts.mmt_mint.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
        )?;

        // Mint total supply to treasury
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mmt_mint.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                &[&[b"mint_authority", &[ctx.bumps.mint_authority]]],
            ),
            100_000_000 * 10u64.pow(9),
        )?;

        // Burn mint authority
        token::set_authority(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::SetAuthority {
                    account_or_mint: ctx.accounts.mmt_mint.to_account_info(),
                    current_authority: ctx.accounts.mint_authority.to_account_info(),
                },
                &[&[b"mint_authority", &[ctx.bumps.mint_authority]]],
            ),
            AuthorityType::MintTokens,
            None,
        )?;

        // Initialize first epoch parameters
        let global = &mut ctx.accounts.global_config;
        global.leverage_tiers = vec![
            LeverageTier { n: 1, max: 100 },
            LeverageTier { n: 2, max: 70 },
            LeverageTier { n: 4, max: 25 },
            LeverageTier { n: 8, max: 15 },
            LeverageTier { n: 16, max: 12 },
            LeverageTier { n: 64, max: 10 },
            LeverageTier { n: u32::MAX, max: 5 },
        ];

        Ok(())
    }

    pub fn emergency_halt(ctx: Context<EmergencyHalt>) -> Result<()> {
        let global = &mut ctx.accounts.global_config;
        let clock = Clock::get()?;

        // Only allowed within first 100 slots of genesis
        require!(
            clock.slot < global.genesis_slot + 100,
            ErrorCode::EmergencyHaltExpired
        );

        global.halt_flag = true;

        emit!(EmergencyHaltEvent {
            slot: clock.slot,
            reason: "Genesis configuration issue".to_string(),
        });

        Ok(())
    }

    pub fn initialize_price_cache(
        ctx: Context<InitializePriceCache>,
        verse_id: u128,
    ) -> Result<()> {
        super::price_cache::initialize_price_cache(ctx, verse_id)
    }

    pub fn update_price_cache(
        ctx: Context<UpdatePriceCache>,
        verse_id: u128,
        new_price: u64,
    ) -> Result<()> {
        super::price_cache::update_price_cache(ctx, verse_id, new_price)
    }

    pub fn process_resolution(
        ctx: Context<ProcessResolution>,
        verse_id: u128,
        market_id: String,
        resolution_outcome: String,
    ) -> Result<()> {
        super::resolution::process_resolution(ctx, verse_id, market_id, resolution_outcome)
    }

    pub fn initiate_dispute(
        ctx: Context<InitiateDispute>,
        verse_id: u128,
        market_id: String,
    ) -> Result<()> {
        super::resolution::initiate_dispute(ctx, verse_id, market_id)
    }

    pub fn resolve_dispute(
        ctx: Context<ResolveDispute>,
        verse_id: u128,
        market_id: String,
        final_resolution: String,
    ) -> Result<()> {
        super::resolution::resolve_dispute(ctx, verse_id, market_id, final_resolution)
    }

    pub fn mirror_dispute(
        ctx: Context<InitiateDispute>,
        market_id: String,
        disputed: bool,
    ) -> Result<()> {
        super::resolution::mirror_dispute(ctx, market_id, disputed)
    }

    pub fn initialize_keeper_health(
        ctx: Context<InitializeKeeperHealth>,
    ) -> Result<()> {
        super::keeper_health::initialize_keeper_health(ctx)
    }

    pub fn report_keeper_metrics(
        ctx: Context<UpdateKeeperHealth>,
        markets_processed: u64,
        errors: u64,
        avg_latency: u64,
    ) -> Result<()> {
        super::keeper_health::report_keeper_metrics(ctx, markets_processed, errors, avg_latency)
    }

    pub fn initialize_performance_metrics(
        ctx: Context<InitializePerformanceMetrics>,
    ) -> Result<()> {
        super::keeper_health::initialize_performance_metrics(ctx)
    }

    pub fn update_performance_metrics(
        ctx: Context<UpdateMetrics>,
        request_count: u64,
        success_count: u64,
        fail_count: u64,
        latencies: Vec<u64>,
    ) -> Result<()> {
        super::keeper_health::update_performance_metrics(ctx, request_count, success_count, fail_count, latencies)
    }

    // Trading Engine Instructions
    pub fn open_position(
        ctx: Context<OpenPosition>,
        params: OpenPositionParams,
    ) -> Result<()> {
        super::trading::open_position(ctx, params)
    }

    pub fn close_position<'info>(
        ctx: Context<'_, '_, '_, 'info, OpenPosition<'info>>,
        position_index: u8,
    ) -> Result<()> {
        super::trading::close_position(ctx, position_index)
    }

    // Fee Management Instructions
    pub fn distribute_fees(
        ctx: Context<DistributeFees>,
        fee_amount: u64,
    ) -> Result<()> {
        super::fees::distribute_fees(ctx, fee_amount)
    }

    // Liquidation Instructions
    pub fn partial_liquidate(
        ctx: Context<PartialLiquidate>,
        position_index: u8,
    ) -> Result<()> {
        super::liquidation::partial_liquidate(ctx, position_index)
    }

    // Chaining Engine Instructions
    pub fn auto_chain(
        ctx: Context<AutoChain>,
        verse_id: u128,
        deposit: u64,
        steps: Vec<ChainStepType>,
    ) -> Result<()> {
        super::chain_execution::auto_chain(ctx, verse_id, deposit, steps)
    }

    pub fn unwind_chain(
        ctx: Context<UnwindChain>,
        chain_id: u128,
    ) -> Result<()> {
        super::chain_unwind::unwind_chain(ctx, chain_id)
    }

    // Safety Instructions
    pub fn check_circuit_breakers(
        ctx: Context<CheckCircuitBreakers>,
        price_movement: i64,
    ) -> Result<()> {
        super::safety::check_circuit_breakers(ctx, price_movement)
    }

    pub fn monitor_position_health(
        ctx: Context<MonitorHealth>,
    ) -> Result<()> {
        super::safety::monitor_position_health(ctx)
    }

    // AMM Instructions
    pub fn initialize_lmsr_market(
        ctx: Context<InitializeLSMR>,
        market_id: u128,
        b_parameter: u64,
        num_outcomes: u8,
    ) -> Result<()> {
        super::lmsr_amm::initialize_lmsr_market(ctx, market_id, b_parameter, num_outcomes)
    }

    pub fn execute_lmsr_trade(
        ctx: Context<LSMRTrade>,
        outcome: u8,
        amount: u64,
        is_buy: bool,
    ) -> Result<()> {
        super::lmsr_amm::execute_lmsr_trade(ctx, outcome, amount, is_buy)
    }

    pub fn initialize_pmamm_market(
        ctx: Context<InitializePMAMM>,
        market_id: u128,
        l_parameter: u64,
        expiry_time: i64,
        initial_price: u64,
    ) -> Result<()> {
        super::pm_amm::initialize_pmamm_market(ctx, market_id, l_parameter, expiry_time, initial_price)
    }

    pub fn execute_pmamm_trade(
        ctx: Context<PMAMMTrade>,
        outcome: u8,
        amount: u64,
        is_buy: bool,
    ) -> Result<()> {
        super::pm_amm::execute_pmamm_trade(ctx, outcome, amount, is_buy)
    }

    pub fn initialize_l2_amm_market(
        ctx: Context<InitializeL2AMM>,
        market_id: u128,
        k_parameter: u64,
        b_bound: u64,
        distribution_type: super::l2_amm::DistributionType,
        discretization_points: u16,
        range_min: u64,
        range_max: u64,
    ) -> Result<()> {
        super::l2_amm::initialize_l2_amm_market(
            ctx,
            market_id,
            k_parameter,
            b_bound,
            distribution_type,
            discretization_points,
            range_min,
            range_max,
        )
    }

    pub fn execute_l2_trade(
        ctx: Context<L2AMMTrade>,
        outcome: u8,
        amount: u64,
        is_buy: bool,
    ) -> Result<()> {
        super::l2_amm::execute_l2_trade(ctx, outcome, amount, is_buy)
    }

    pub fn initialize_hybrid_amm(
        ctx: Context<InitializeHybridAMM>,
        market_id: u128,
        amm_type: super::hybrid_amm::AMMType,
        num_outcomes: u8,
        expiry_time: i64,
        is_continuous: bool,
        amm_specific_data: Vec<u8>,
    ) -> Result<()> {
        super::hybrid_amm::initialize_hybrid_amm(
            ctx,
            market_id,
            amm_type,
            num_outcomes,
            expiry_time,
            is_continuous,
            amm_specific_data,
        )
    }

    pub fn execute_hybrid_trade(
        ctx: Context<HybridTrade>,
        outcome: u8,
        amount: u64,
        is_buy: bool,
    ) -> Result<()> {
        super::hybrid_amm::execute_hybrid_trade(ctx, outcome, amount, is_buy)
    }

    // Advanced Trading Instructions
    pub fn place_iceberg_order(
        ctx: Context<PlaceIcebergOrder>,
        market_id: u128,
        outcome: u8,
        visible_size: u64,
        total_size: u64,
        side: OrderSide,
    ) -> Result<()> {
        super::iceberg_orders::place_iceberg_order(ctx, market_id, outcome, visible_size, total_size, side)
    }

    pub fn execute_iceberg_fill(
        ctx: Context<ExecuteIcebergFill>,
        fill_size: u64,
    ) -> Result<()> {
        super::iceberg_orders::execute_iceberg_fill(ctx, fill_size)
    }

    pub fn place_twap_order(
        ctx: Context<PlaceTWAPOrder>,
        market_id: u128,
        outcome: u8,
        total_size: u64,
        duration: u64,
        intervals: u8,
        side: OrderSide,
    ) -> Result<()> {
        super::twap_orders::place_twap_order(ctx, market_id, outcome, total_size, duration, intervals, side)
    }

    pub fn execute_twap_interval(
        ctx: Context<ExecuteTWAPInterval>,
    ) -> Result<()> {
        super::twap_orders::execute_twap_interval(ctx)
    }

    pub fn initialize_dark_pool(
        ctx: Context<InitializeDarkPool>,
        market_id: u128,
        minimum_size: u64,
        price_improvement_bps: u16,
    ) -> Result<()> {
        super::dark_pool::initialize_dark_pool(ctx, market_id, minimum_size, price_improvement_bps)
    }

    pub fn place_dark_order(
        ctx: Context<PlaceDarkOrder>,
        side: OrderSide,
        outcome: u8,
        size: u64,
        min_price: Option<u64>,
        max_price: Option<u64>,
        time_in_force: TimeInForce,
    ) -> Result<()> {
        super::dark_pool::place_dark_order(ctx, side, outcome, size, min_price, max_price, time_in_force)
    }

    // Attack detection and circuit breaker instructions
    
    pub fn initialize_attack_detector(ctx: Context<InitializeAttackDetector>) -> Result<()> {
        super::instructions::attack_detection_instructions::initialize_attack_detector(ctx)
    }
    
    pub fn process_trade_security(
        ctx: Context<ProcessTrade>,
        market_id: [u8; 32],
        size: u64,
        price: u64,
        leverage: u64,
        is_buy: bool,
    ) -> Result<()> {
        super::instructions::attack_detection_instructions::process_trade(ctx, market_id, size, price, leverage, is_buy)
    }
    
    pub fn update_volume_baseline(
        ctx: Context<UpdateVolumeBaseline>,
        new_avg_volume: u64,
        new_std_dev: u64,
    ) -> Result<()> {
        super::instructions::attack_detection_instructions::update_volume_baseline(ctx, new_avg_volume, new_std_dev)
    }
    
    pub fn reset_attack_detector(ctx: Context<ResetDetector>) -> Result<()> {
        super::instructions::attack_detection_instructions::reset_detector(ctx)
    }
    
    pub fn initialize_circuit_breaker(ctx: Context<InitializeCircuitBreaker>) -> Result<()> {
        super::instructions::circuit_breaker_instructions::initialize_circuit_breaker(ctx)
    }
    
    pub fn check_advanced_breakers(
        ctx: Context<CheckBreakers>,
        coverage: u64,
        liquidation_count: u64,
        liquidation_volume: u64,
        total_oi: u64,
        failed_tx: u64,
    ) -> Result<()> {
        super::instructions::circuit_breaker_instructions::check_breakers(ctx, coverage, liquidation_count, liquidation_volume, total_oi, failed_tx)
    }
    
    pub fn emergency_shutdown(ctx: Context<EmergencyShutdown>) -> Result<()> {
        super::instructions::circuit_breaker_instructions::emergency_shutdown(ctx)
    }
    
    pub fn update_breaker_config(
        ctx: Context<UpdateBreakerConfig>,
        new_cooldown_period: Option<u64>,
        new_coverage_halt_duration: Option<u64>,
        new_price_halt_duration: Option<u64>,
        new_volume_halt_duration: Option<u64>,
        new_liquidation_halt_duration: Option<u64>,
        new_congestion_halt_duration: Option<u64>,
    ) -> Result<()> {
        super::instructions::circuit_breaker_instructions::update_breaker_config(
            ctx,
            new_cooldown_period,
            new_coverage_halt_duration,
            new_price_halt_duration,
            new_volume_halt_duration,
            new_liquidation_halt_duration,
            new_congestion_halt_duration,
        )
    }
    
    pub fn initialize_liquidation_queue(ctx: Context<InitializeLiquidationQueue>) -> Result<()> {
        super::instructions::liquidation_priority_instructions::initialize_liquidation_queue(ctx)
    }
    
    pub fn update_at_risk_position(
        ctx: Context<UpdateAtRiskPosition>,
        mark_price: u64,
    ) -> Result<()> {
        super::instructions::liquidation_priority_instructions::update_at_risk_position(ctx, mark_price)
    }
    
    pub fn process_priority_liquidation(
        ctx: Context<ProcessLiquidation>,
        max_liquidations: u64,
    ) -> Result<()> {
        super::instructions::liquidation_priority_instructions::process_liquidation(ctx, max_liquidations)
    }
    
    pub fn claim_keeper_rewards(ctx: Context<ClaimKeeperRewards>) -> Result<()> {
        super::instructions::liquidation_priority_instructions::claim_keeper_rewards(ctx)
    }
    }

    // Account structures
    #[derive(Accounts)]
    pub struct InitializeMmt<'info> {
    /// CHECK: MMT token mint account - initialized manually
    #[account(
        init,
        payer = authority,
        seeds = [b"mmt_mint"],
        bump,
        space = 82,
        owner = anchor_spl::token::ID
    )]
    pub mmt_mint: AccountInfo<'info>,
    
    /// CHECK: Treasury token account - initialized manually
    #[account(
        init,
        payer = authority,
        seeds = [b"treasury"],
        bump,
        space = 165,
        owner = anchor_spl::token::ID
    )]
    pub treasury: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// CHECK: PDA used as mint authority
    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct GenesisAtomic<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalConfigPDA::LEN,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    /// CHECK: MMT token mint account - initialized manually
    #[account(
        init,
        payer = authority,
        seeds = [b"mmt_mint"],
        bump,
        space = 82,
        owner = anchor_spl::token::ID
    )]
    pub mmt_mint: AccountInfo<'info>,
    
    /// CHECK: Treasury token account - initialized manually
    #[account(
        init,
        payer = authority,
        seeds = [b"treasury"],
        bump,
        space = 165,
        owner = anchor_spl::token::ID
    )]
    pub treasury: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// CHECK: PDA used as mint authority
    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct EmergencyHalt<'info> {
    #[account(mut, seeds = [b"global_config"], bump)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}