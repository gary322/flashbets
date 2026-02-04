use anchor_lang::prelude::*;
use crate::chain_state::*;
use crate::fixed_math::*;
use crate::trading::*;
use crate::account_structs::*;
use crate::errors::ErrorCode;
use crate::events::*;

// Chain Execution Engine

// Helper struct removed - will pass accounts directly to avoid lifetime issues

#[derive(Accounts)]
#[instruction(verse_id: u128)]
pub struct AutoChain<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub verse_pda: Account<'info, VersePDA>,
    
    #[account(
        init,
        payer = user,
        space = ChainStatePDA::LEN,
        seeds = [b"chain_state", user.key().as_ref(), verse_id.to_le_bytes().as_ref()],
        bump
    )]
    pub chain_state: Account<'info, ChainStatePDA>,
    
    #[account(mut)]
    pub verse_liquidity_pool: Account<'info, VerseLiquidityPool>,
    
    #[account(mut)]
    pub verse_staking_pool: Account<'info, VerseStakingPool>,
    
    pub system_program: Program<'info, System>,
}

// Constants for CU limits
pub const CU_PER_CHAIN_STEP: u64 = 10_000; // Base CU per step
pub const MAX_CU_CHAIN_BUNDLE: u64 = 30_000; // Spec: Bundle 10 children=30k CU

pub fn auto_chain(
    ctx: Context<AutoChain>,
    verse_id: u128,
    deposit: u64,
    steps: Vec<ChainStepType>,
) -> Result<()> {
    // Validate inputs
    require!(deposit > 0, ErrorCode::InvalidDeposit);
    require!(steps.len() <= 5, ErrorCode::TooManySteps);
    require!(steps.len() > 0, ErrorCode::NoSteps);
    
    // Validate CU budget (spec: 30k CU limit for chain bundle)
    let estimated_cu = steps.len() as u64 * CU_PER_CHAIN_STEP;
    require!(estimated_cu <= MAX_CU_CHAIN_BUNDLE, ErrorCode::ExceedsCULimit);

    // Check verse exists and is active
    let verse = &ctx.accounts.verse_pda;
    require!(verse.status == VerseStatus::Active, ErrorCode::InactiveVerse);

    // Check coverage allows leverage
    let coverage = calculate_coverage_fixed(&ctx.accounts.global_config);
    require!(coverage > FixedPoint::from_u64(0), ErrorCode::InsufficientCoverage);

    // Initialize chain state
    let chain_state = &mut ctx.accounts.chain_state;
    chain_state.verse_id = verse_id;
    chain_state.user = ctx.accounts.user.key();
    chain_state.chain_id = generate_chain_id();
    chain_state.steps_completed = 0;
    chain_state.max_steps = steps.len() as u8;
    chain_state.initial_deposit = deposit;
    chain_state.current_value = deposit;
    chain_state.effective_leverage = FixedPoint::from_u64(1);
    chain_state.status = ChainStatus::Active;
    chain_state.created_slot = Clock::get()?.slot;
    chain_state.last_update_slot = Clock::get()?.slot;

    // Execute each step
    let mut current_amount = deposit;
    
    for (i, step_type) in steps.iter().enumerate() {
        let step_result = execute_chain_step(
            &ctx.accounts.global_config,
            &ctx.accounts.verse_pda,
            &ctx.accounts.verse_liquidity_pool,
            &ctx.accounts.verse_staking_pool,
            verse_id,
            current_amount,
            step_type.clone(),
            i as u8,
        )?;

        current_amount = step_result.output_amount;
        chain_state.step_states.push(step_result.step_state);
        chain_state.steps_completed += 1;
        chain_state.current_value = current_amount;

        // Update effective leverage
        chain_state.effective_leverage = chain_state.effective_leverage
            .mul(&step_result.leverage_multiplier)?;
    }

    // Emit event
    emit!(ChainCreatedEvent {
        chain_id: chain_state.chain_id,
        user: ctx.accounts.user.key(),
        verse_id,
        initial_deposit: deposit,
        final_value: current_amount,
        effective_leverage: chain_state.effective_leverage,
        steps: steps.len() as u8,
    });

    Ok(())
}

fn execute_chain_step<'info>(
    global_config: &Account<'info, GlobalConfigPDA>,
    verse_pda: &Account<'info, VersePDA>,
    verse_liquidity_pool: &Account<'info, VerseLiquidityPool>,
    verse_staking_pool: &Account<'info, VerseStakingPool>,
    verse_id: u128,
    input_amount: u64,
    step_type: ChainStepType,
    _step_index: u8,
) -> Result<ChainStepResult> {
    match step_type {
        ChainStepType::Borrow => execute_borrow_step(global_config, verse_pda, verse_liquidity_pool, verse_staking_pool, verse_id, input_amount),
        ChainStepType::Liquidity => execute_liquidity_step(global_config, verse_pda, verse_liquidity_pool, verse_staking_pool, verse_id, input_amount),
        ChainStepType::Stake => execute_stake_step(global_config, verse_pda, verse_liquidity_pool, verse_staking_pool, verse_id, input_amount),
        ChainStepType::Arbitrage => execute_arbitrage_step(global_config, verse_pda, verse_liquidity_pool, verse_staking_pool, verse_id, input_amount),
    }
}

// Individual step implementations

pub fn execute_borrow_step<'info>(
    global_config: &Account<'info, GlobalConfigPDA>,
    verse_pda: &Account<'info, VersePDA>,
    _verse_liquidity_pool: &Account<'info, VerseLiquidityPool>,
    _verse_staking_pool: &Account<'info, VerseStakingPool>,
    _verse_id: u128,
    input_amount: u64,
) -> Result<ChainStepResult> {
    // Calculate borrowing parameters
    let coverage = calculate_coverage_fixed(global_config);
    let n_outcomes = verse_pda.num_outcomes();
    let borrow_multiplier = calculate_borrow_multiplier(coverage, n_outcomes)?;

    // Maximum safe borrow amount
    let max_borrow = input_amount
        .checked_mul(borrow_multiplier.to_u64_truncate())
        .ok_or(ErrorCode::MathOverflow)?;

    // Apply safety factor (80% of max)
    let borrow_amount = max_borrow
        .checked_mul(80)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::MathOverflow)?;

    let output_amount = input_amount
        .checked_add(borrow_amount)
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(ChainStepResult {
        step_state: ChainStepState {
            step_type: ChainStepType::Borrow,
            input_amount,
            output_amount,
            leverage_multiplier: FixedPoint::from_u64(1).add(
                &FixedPoint::from_u64(borrow_amount).div(&FixedPoint::from_u64(input_amount))?
            )?,
            position_id: Some(generate_position_id()),
            status: StepStatus::Completed,
            error_code: None,
        },
        output_amount,
        leverage_multiplier: FixedPoint::from_float(1.5), // Typical 1.5x from borrow
    })
}

pub fn execute_liquidity_step<'info>(
    _global_config: &Account<'info, GlobalConfigPDA>,
    _verse_pda: &Account<'info, VersePDA>,
    verse_liquidity_pool: &Account<'info, VerseLiquidityPool>,
    _verse_staking_pool: &Account<'info, VerseStakingPool>,
    verse_id: u128,
    input_amount: u64,
) -> Result<ChainStepResult> {
    // Add liquidity to verse pool
    let pool = verse_liquidity_pool;
    require!(pool.verse_id == verse_id, ErrorCode::WrongVerse);

    // Calculate LP tokens received
    let lp_tokens = calculate_lp_tokens(&pool, input_amount)?;

    // Calculate yield from LVR
    let lvr_yield = calculate_lvr_yield(&pool, input_amount)?;
    let output_amount = input_amount
        .checked_add(lvr_yield)
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(ChainStepResult {
        step_state: ChainStepState {
            step_type: ChainStepType::Liquidity,
            input_amount,
            output_amount,
            leverage_multiplier: FixedPoint::from_u64(output_amount).div(
                &FixedPoint::from_u64(input_amount)
            )?,
            position_id: Some(generate_position_id()),
            status: StepStatus::Completed,
            error_code: None,
        },
        output_amount,
        leverage_multiplier: FixedPoint::from_float(1.2), // Typical 1.2x from liquidity
    })
}

pub fn execute_stake_step<'info>(
    _global_config: &Account<'info, GlobalConfigPDA>,
    verse_pda: &Account<'info, VersePDA>,
    _verse_liquidity_pool: &Account<'info, VerseLiquidityPool>,
    verse_staking_pool: &Account<'info, VerseStakingPool>,
    verse_id: u128,
    input_amount: u64,
) -> Result<ChainStepResult> {
    // Stake in verse-specific staking pool
    let staking_pool = verse_staking_pool;
    require!(staking_pool.verse_id == verse_id, ErrorCode::WrongVerse);

    // Calculate staking rewards based on depth
    let depth_bonus = FixedPoint::from_u64(verse_pda.depth as u64)
        .div(&FixedPoint::from_u64(32))?; // Max depth 32

    let stake_multiplier = FixedPoint::from_float(1.1)
        .add(&depth_bonus.mul(&FixedPoint::from_float(0.1))?)?;

    let output_amount = FixedPoint::from_u64(input_amount)
        .mul(&stake_multiplier)?
        .to_u64_truncate();

    // Note: In a real implementation, staking pool state would be updated via CPI

    Ok(ChainStepResult {
        step_state: ChainStepState {
            step_type: ChainStepType::Stake,
            input_amount,
            output_amount,
            leverage_multiplier: stake_multiplier,
            position_id: Some(generate_position_id()),
            status: StepStatus::Completed,
            error_code: None,
        },
        output_amount,
        leverage_multiplier: FixedPoint::from_float(1.1), // Typical 1.1x from staking
    })
}

pub fn execute_arbitrage_step<'info>(
    _global_config: &Account<'info, GlobalConfigPDA>,
    _verse_pda: &Account<'info, VersePDA>,
    _verse_liquidity_pool: &Account<'info, VerseLiquidityPool>,
    _verse_staking_pool: &Account<'info, VerseStakingPool>,
    _verse_id: u128,
    input_amount: u64,
) -> Result<ChainStepResult> {
    // Simplified arbitrage step - would check price differentials
    // between markets in a real implementation
    
    let arbitrage_profit = input_amount
        .checked_mul(5)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::MathOverflow)?; // 5% profit

    let output_amount = input_amount
        .checked_add(arbitrage_profit)
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(ChainStepResult {
        step_state: ChainStepState {
            step_type: ChainStepType::Arbitrage,
            input_amount,
            output_amount,
            leverage_multiplier: FixedPoint::from_float(1.05),
            position_id: Some(generate_position_id()),
            status: StepStatus::Completed,
            error_code: None,
        },
        output_amount,
        leverage_multiplier: FixedPoint::from_float(1.05),
    })
}

// Helper functions

fn calculate_coverage_fixed(global_config: &GlobalConfigPDA) -> FixedPoint {
    FixedPoint {
        value: global_config.coverage,
    }
}

fn calculate_borrow_multiplier(coverage: FixedPoint, n_outcomes: u8) -> Result<FixedPoint> {
    // Base multiplier adjusted by coverage and outcomes
    let base = FixedPoint::from_float(1.5);
    let outcome_adj = FixedPoint::from_u64(1).div(
        &FixedPoint::from_u64(n_outcomes as u64).sqrt()?
    )?;
    
    base.mul(&outcome_adj)
}

fn calculate_lp_tokens(pool: &VerseLiquidityPool, amount: u64) -> Result<u64> {
    if pool.total_liquidity == 0 {
        Ok(amount) // First LP gets 1:1
    } else {
        // LP tokens = (amount / total_liquidity) * lp_token_supply
        let ratio = (amount as u128 * pool.lp_token_supply as u128) / pool.total_liquidity as u128;
        Ok(ratio as u64)
    }
}

fn calculate_lvr_yield(pool: &VerseLiquidityPool, amount: u64) -> Result<u64> {
    // Simplified LVR yield calculation
    let fee_income = amount
        .checked_mul(pool.fee_rate)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::MathOverflow)?;
    
    Ok(fee_income / 2) // Half of fees as LVR yield
}

pub fn generate_chain_id() -> u128 {
    // Simple chain ID generation using timestamp and randomness
    let clock = Clock::get().unwrap();
    let timestamp = clock.unix_timestamp as u128;
    let slot = clock.slot as u128;
    
    (timestamp << 64) | (slot & 0xFFFFFFFFFFFFFFFF)
}

pub fn generate_position_id() -> u128 {
    // Similar to chain ID but with additional entropy
    let clock = Clock::get().unwrap();
    let timestamp = clock.unix_timestamp as u128;
    let slot = clock.slot as u128;
    
    ((timestamp << 64) | (slot & 0xFFFFFFFFFFFFFFFF)).wrapping_add(1)
}