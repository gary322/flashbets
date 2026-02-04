use anchor_lang::prelude::*;
use crate::advanced_orders::*;
use crate::errors::ErrorCode;

#[derive(Accounts)]
#[instruction(market_id: u128)]
pub struct PlaceTWAPOrder<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 16 + 32 + 16 + 200 + 1 + 1 + 8 + 8 + 8 + 1 + 8 + 8 + 100,
        seeds = [b"advanced_order", user.key().as_ref(), &generate_order_id().to_le_bytes()],
        bump
    )]
    pub advanced_order: Account<'info, AdvancedOrderPDA>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[event]
pub struct TWAPOrderPlacedEvent {
    pub order_id: u128,
    pub user: Pubkey,
    pub market_id: u128,
    pub total_size: u64,
    pub duration: u64,
    pub intervals: u8,
    pub side: OrderSide,
}

#[event]
pub struct TWAPIntervalExecutedEvent {
    pub order_id: u128,
    pub interval: u8,
    pub executed_size: u64,
    pub average_price: u64,
}

pub fn place_twap_order(
    ctx: Context<PlaceTWAPOrder>,
    market_id: u128,
    outcome: u8,
    total_size: u64,
    duration: u64,  // in slots
    intervals: u8,
    side: OrderSide,
) -> Result<()> {
    require!(intervals > 0 && intervals <= 100, ErrorCode::InvalidIntervals);
    require!(duration >= intervals as u64 * 10, ErrorCode::DurationTooShort);

    let size_per_interval = total_size / intervals as u64;
    require!(size_per_interval > 0, ErrorCode::SizeTooSmall);

    let order = &mut ctx.accounts.advanced_order;
    let order_id = generate_order_id();
    let current_slot = Clock::get()?.slot;

    order.order_id = order_id;
    order.user = ctx.accounts.user.key();
    order.market_id = market_id;
    order.order_type = OrderType::TWAP { duration, intervals };
    order.side = side.clone();
    order.outcome = outcome;
    order.remaining_size = total_size;
    order.executed_size = 0;
    order.average_price = 0;
    order.status = OrderStatus::Active;
    order.created_at = Clock::get()?.unix_timestamp;
    order.expires_at = Some(Clock::get()?.unix_timestamp + (duration as i64 / 2)); // slots to seconds approximation

    let interval_duration = duration / intervals as u64;

    order.execution_metadata = OrderExecutionMetadata {
        last_execution_slot: current_slot,
        num_fills: 0,
        twap_progress: Some(TWAPProgress {
            intervals_completed: 0,
            next_execution_slot: current_slot + interval_duration,
            size_per_interval,
        }),
        iceberg_revealed: 0,
    };

    emit!(TWAPOrderPlacedEvent {
        order_id,
        user: ctx.accounts.user.key(),
        market_id,
        total_size,
        duration,
        intervals,
        side,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteTWAPInterval<'info> {
    #[account(mut)]
    pub advanced_order: Account<'info, AdvancedOrderPDA>,
    
    #[account(mut)]
    pub keeper: Signer<'info>,
}

pub fn execute_twap_interval(
    ctx: Context<ExecuteTWAPInterval>,
) -> Result<()> {
    let current_slot = Clock::get()?.slot;
    
    // First read all needed values
    let order = &ctx.accounts.advanced_order;
    let (intervals_copy, duration_copy) = match &order.order_type {
        OrderType::TWAP { duration, intervals } => (*intervals, *duration),
        _ => return Err(ErrorCode::InvalidOrderType.into()),
    };
    
    let twap_progress = order.execution_metadata.twap_progress
        .as_ref()
        .ok_or(ErrorCode::InvalidTWAPState)?;
    
    let next_execution_slot = twap_progress.next_execution_slot;
    require!(
        current_slot >= next_execution_slot,
        ErrorCode::TooEarlyForTWAP
    );
    
    let size_per_interval = twap_progress.size_per_interval;
    let market_id = order.market_id;
    let outcome = order.outcome;
    let side = order.side.clone();
    let prev_executed_size = order.executed_size;
    let prev_average_price = order.average_price;

    // Execute interval order
    let execution_result = execute_market_order(
        &ctx.accounts.to_account_infos(),
        market_id,
        outcome,
        size_per_interval,
        side,
    )?;
    
    // Now update the order
    let order = &mut ctx.accounts.advanced_order;

    // Update order state
    order.executed_size = order.executed_size
        .checked_add(execution_result.executed_size)
        .ok_or(ErrorCode::MathOverflow)?;

    order.remaining_size = order.remaining_size
        .checked_sub(execution_result.executed_size)
        .ok_or(ErrorCode::MathOverflow)?;

    // Update average price
    if order.executed_size > execution_result.executed_size {
        let total_value = order.average_price
            .checked_mul(order.executed_size - execution_result.executed_size)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(
                execution_result.average_price
                    .checked_mul(execution_result.executed_size)
                    .ok_or(ErrorCode::MathOverflow)?
            )
            .ok_or(ErrorCode::MathOverflow)?;

        order.average_price = total_value / order.executed_size;
    } else {
        order.average_price = execution_result.average_price;
    }

    // Update TWAP progress
    let twap_progress = order.execution_metadata.twap_progress
        .as_mut()
        .ok_or(ErrorCode::InvalidTWAPState)?;
    
    twap_progress.intervals_completed += 1;
    let intervals_completed = twap_progress.intervals_completed;

    if intervals_completed < intervals_copy {
        let interval_duration = duration_copy / intervals_copy as u64;
        twap_progress.next_execution_slot = current_slot + interval_duration;
    }
    
    // Update order metadata after mutable borrow of twap_progress is done
    order.execution_metadata.last_execution_slot = current_slot;
    order.execution_metadata.num_fills += 1;
    
    if intervals_completed >= intervals_copy {
        order.status = OrderStatus::Filled;
    }

    let order_id = order.order_id;

    emit!(TWAPIntervalExecutedEvent {
        order_id,
        interval: intervals_completed,
        executed_size: execution_result.executed_size,
        average_price: execution_result.average_price,
    });

    Ok(())
}