use anchor_lang::prelude::*;
use crate::advanced_orders::*;
use crate::errors::ErrorCode;

#[derive(Accounts)]
#[instruction(market_id: u128)]
pub struct PlaceIcebergOrder<'info> {
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
pub struct IcebergOrderPlacedEvent {
    pub order_id: u128,
    pub user: Pubkey,
    pub market_id: u128,
    pub visible_size: u64,
    pub total_size: u64,
    pub side: OrderSide,
}

pub fn place_iceberg_order(
    ctx: Context<PlaceIcebergOrder>,
    market_id: u128,
    outcome: u8,
    visible_size: u64,
    total_size: u64,
    side: OrderSide,
) -> Result<()> {
    require!(visible_size > 0, ErrorCode::InvalidVisibleSize);
    require!(total_size >= visible_size, ErrorCode::InvalidTotalSize);
    require!(visible_size <= total_size / 10, ErrorCode::VisibleSizeTooLarge);

    let order = &mut ctx.accounts.advanced_order;
    let order_id = generate_order_id();

    order.order_id = order_id;
    order.user = ctx.accounts.user.key();
    order.market_id = market_id;
    order.order_type = OrderType::Iceberg { visible_size, total_size };
    order.side = side.clone();
    order.outcome = outcome;
    order.remaining_size = total_size;
    order.executed_size = 0;
    order.average_price = 0;
    order.status = OrderStatus::Active;
    order.created_at = Clock::get()?.unix_timestamp;
    order.expires_at = None;
    order.execution_metadata = OrderExecutionMetadata {
        last_execution_slot: Clock::get()?.slot,
        num_fills: 0,
        twap_progress: None,
        iceberg_revealed: visible_size,
    };

    // Add visible portion to orderbook
    add_to_orderbook(&ctx.accounts.to_account_infos(), order_id, visible_size)?;

    emit!(IcebergOrderPlacedEvent {
        order_id,
        user: ctx.accounts.user.key(),
        market_id,
        visible_size,
        total_size,
        side,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteIcebergFill<'info> {
    #[account(mut)]
    pub advanced_order: Account<'info, AdvancedOrderPDA>,
    
    #[account(mut)]
    pub executor: Signer<'info>,
}

pub fn execute_iceberg_fill(
    ctx: Context<ExecuteIcebergFill>,
    fill_size: u64,
) -> Result<()> {
    // First, extract all needed values
    let order = &ctx.accounts.advanced_order;
    let visible_size = match &order.order_type {
        OrderType::Iceberg { visible_size, total_size: _ } => *visible_size,
        _ => return Err(ErrorCode::InvalidOrderType.into()),
    };
    let order_id = order.order_id;
    let average_price = order.average_price;
    
    require!(fill_size <= order.execution_metadata.iceberg_revealed,
             ErrorCode::ExceedsVisibleSize);

    // Calculate if we need to reveal more
    let current_revealed = order.execution_metadata.iceberg_revealed;
    let remaining_after_fill = order.remaining_size.checked_sub(fill_size)
        .ok_or(ErrorCode::MathOverflow)?;
    let revealed_after_fill = current_revealed.checked_sub(fill_size)
        .ok_or(ErrorCode::MathOverflow)?;
    let needs_reveal = revealed_after_fill == 0 && remaining_after_fill > 0;
    let new_reveal = if needs_reveal {
        Some(visible_size.min(remaining_after_fill))
    } else {
        None
    };

    // Call external function if needed
    if let Some(reveal_size) = new_reveal {
        add_to_orderbook(&ctx.accounts.to_account_infos(), order_id, reveal_size)?;
    }

    // Now update the order
    let order = &mut ctx.accounts.advanced_order;
    
    order.executed_size = order.executed_size
        .checked_add(fill_size)
        .ok_or(ErrorCode::MathOverflow)?;

    order.remaining_size = remaining_after_fill;
    order.execution_metadata.iceberg_revealed = if let Some(reveal_size) = new_reveal {
        reveal_size
    } else {
        revealed_after_fill
    };

    // Update status if fully executed
    if order.remaining_size == 0 {
        order.status = OrderStatus::Filled;
    }

    order.execution_metadata.num_fills += 1;
    order.execution_metadata.last_execution_slot = Clock::get()?.slot;

    emit!(OrderFilledEvent {
        order_id,
        executed_size: fill_size,
        average_price,
    });

    Ok(())
}