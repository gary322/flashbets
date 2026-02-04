use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer};
use crate::chain_state::*;
use crate::errors::ErrorCode;
use crate::events::*;

// Chain Unwinding

#[derive(Accounts)]
#[instruction(chain_id: u128)]
pub struct UnwindChain<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(
        mut,
        has_one = user,
        constraint = chain_state.chain_id == chain_id @ ErrorCode::InvalidChainStatus
    )]
    pub chain_state: Account<'info, ChainStatePDA>,
    
    #[account(mut)]
    pub user_token_account: Account<'info, token::TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

pub fn unwind_chain(
    ctx: Context<UnwindChain>,
    chain_id: u128,
) -> Result<()> {
    let chain_state = &mut ctx.accounts.chain_state;

    // Validate ownership
    require!(
        chain_state.user == ctx.accounts.user.key(),
        ErrorCode::Unauthorized
    );

    // Validate chain_id
    require!(
        chain_state.chain_id == chain_id,
        ErrorCode::InvalidChainStatus
    );

    // Check chain can be unwound
    require!(
        chain_state.status == ChainStatus::Active,
        ErrorCode::InvalidChainStatus
    );

    // Update status
    chain_state.status = ChainStatus::Unwinding;
    chain_state.last_update_slot = Clock::get()?.slot;

    // Unwind in reverse order
    let steps_to_unwind = chain_state.steps_completed;
    
    // Extract step information first to avoid borrowing conflicts
    let mut steps_info = Vec::new();
    for i in (0..steps_to_unwind).rev() {
        if chain_state.step_states[i as usize].status != StepStatus::Reverted {
            steps_info.push((i, chain_state.step_states[i as usize].clone()));
        }
    }
    
    // Process unwinding and collect results
    let mut unwind_results = Vec::new();
    for (i, step_state) in steps_info.iter() {
        // Unwind based on step type
        let unwind_result = match step_state.step_type {
            ChainStepType::Borrow => unwind_borrow(ctx.accounts, step_state)?,
            ChainStepType::Liquidity => unwind_liquidity(ctx.accounts, step_state)?,
            ChainStepType::Stake => unwind_stake(ctx.accounts, step_state)?,
            ChainStepType::Arbitrage => unwind_arbitrage(ctx.accounts, step_state)?,
        };
        unwind_results.push((*i, unwind_result));
    }
    
    // Now update chain_state with results
    let chain_state = &mut ctx.accounts.chain_state;
    for (i, unwind_result) in unwind_results {
        chain_state.step_states[i as usize].status = StepStatus::Reverted;
        chain_state.current_value = unwind_result.recovered_amount;
    }

    // Final status update
    chain_state.status = ChainStatus::Completed;

    // Extract values before transfer
    let recovered = chain_state.current_value;
    let initial_deposit = chain_state.initial_deposit;
    let loss_amount = if initial_deposit > recovered {
        initial_deposit - recovered
    } else {
        0
    };
    
    // Transfer recovered funds to user
    transfer_to_user(ctx.accounts, recovered)?;

    emit!(ChainUnwoundEvent {
        chain_id,
        user: ctx.accounts.user.key(),
        initial_deposit,
        recovered_amount: recovered,
        loss_amount,
    });

    Ok(())
}

fn unwind_borrow(
    _accounts: &UnwindChain,
    step: &ChainStepState,
) -> Result<UnwindResult> {
    // Calculate repayment amount with interest
    let borrowed_amount = step.output_amount.saturating_sub(step.input_amount);
    let interest = borrowed_amount
        .checked_mul(5) // 5% interest
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let repayment = borrowed_amount.saturating_add(interest);
    let recovered = step.output_amount.saturating_sub(repayment);

    Ok(UnwindResult {
        recovered_amount: recovered,
    })
}

fn unwind_liquidity(
    _accounts: &UnwindChain,
    step: &ChainStepState,
) -> Result<UnwindResult> {
    // Remove liquidity with small penalty
    let penalty = step.output_amount
        .checked_mul(2) // 2% penalty
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let recovered = step.output_amount.saturating_sub(penalty);

    Ok(UnwindResult {
        recovered_amount: recovered,
    })
}

fn unwind_stake(
    _accounts: &UnwindChain,
    step: &ChainStepState,
) -> Result<UnwindResult> {
    // Unstake with small penalty for early withdrawal
    let penalty = step.output_amount
        .checked_mul(3) // 3% penalty
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let recovered = step.output_amount.saturating_sub(penalty);

    Ok(UnwindResult {
        recovered_amount: recovered,
    })
}

fn unwind_arbitrage(
    _accounts: &UnwindChain,
    step: &ChainStepState,
) -> Result<UnwindResult> {
    // Arbitrage positions can be unwound with minimal loss
    let penalty = step.output_amount
        .checked_mul(1) // 1% penalty
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let recovered = step.output_amount.saturating_sub(penalty);

    Ok(UnwindResult {
        recovered_amount: recovered,
    })
}

fn transfer_to_user(
    accounts: &UnwindChain,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    let cpi_accounts = Transfer {
        from: accounts.user_token_account.to_account_info(),
        to: accounts.user_token_account.to_account_info(),
        authority: accounts.user.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::transfer(cpi_ctx, amount)?;

    Ok(())
}

// Emergency unwind function for liquidations
pub fn emergency_unwind_chain(
    ctx: Context<UnwindChain>,
) -> Result<()> {
    let chain_state = &mut ctx.accounts.chain_state;

    // Update status
    chain_state.status = ChainStatus::Failed;
    chain_state.last_update_slot = Clock::get()?.slot;

    // Calculate total recovery with higher penalties
    let mut total_recovered = 0u64;
    
    for step in chain_state.step_states.iter() {
        if step.status == StepStatus::Completed {
            // 10% emergency penalty
            let penalty = step.output_amount
                .checked_mul(10)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(100)
                .ok_or(ErrorCode::MathOverflow)?;
            
            let recovered = step.output_amount.saturating_sub(penalty);
            total_recovered = total_recovered.saturating_add(recovered);
        }
    }

    chain_state.current_value = total_recovered;
    
    // Extract values before transfer
    let chain_id = chain_state.chain_id;
    let initial_deposit = chain_state.initial_deposit;

    // Transfer recovered funds
    transfer_to_user(ctx.accounts, total_recovered)?;

    emit!(ChainUnwoundEvent {
        chain_id,
        user: ctx.accounts.user.key(),
        initial_deposit,
        recovered_amount: total_recovered,
        loss_amount: initial_deposit.saturating_sub(total_recovered),
    });

    Ok(())
}