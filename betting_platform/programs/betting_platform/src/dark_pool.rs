use anchor_lang::prelude::*;
use crate::advanced_orders::{OrderSide, OrderStatus};
use crate::errors::ErrorCode;

#[account]
pub struct DarkPoolPDA {
    pub pool_id: u128,
    pub market_id: u128,
    pub minimum_size: u64,
    pub price_improvement_bps: u16,  // Basis points
    pub total_volume: u64,
    pub num_trades: u64,
    pub last_match_slot: u64,
}

#[account]
pub struct DarkOrderPDA {
    pub order_id: u128,
    pub user: Pubkey,
    pub pool_id: u128,
    pub side: OrderSide,
    pub outcome: u8,
    pub size: u64,
    pub min_price: Option<u64>,  // For buyers
    pub max_price: Option<u64>,  // For sellers
    pub time_in_force: TimeInForce,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum TimeInForce {
    IOC,  // Immediate or cancel
    FOK,  // Fill or kill
    GTT { expires_at: i64 },  // Good till time
}

pub struct MatchedOrder {
    pub buy_order_id: u128,
    pub sell_order_id: u128,
    pub size: u64,
    pub price: u64,
}

#[derive(Accounts)]
#[instruction(market_id: u128)]
pub struct InitializeDarkPool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 16 + 16 + 8 + 2 + 8 + 8 + 8,
        seeds = [b"dark_pool", market_id.to_le_bytes().as_ref()],
        bump
    )]
    pub dark_pool: Account<'info, DarkPoolPDA>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlaceDarkOrder<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 16 + 32 + 16 + 1 + 1 + 8 + 8 + 8 + 32 + 8 + 8,
        seeds = [b"dark_order", user.key().as_ref(), &Clock::get()?.slot.to_le_bytes()],
        bump
    )]
    pub dark_order: Account<'info, DarkOrderPDA>,
    
    #[account(mut)]
    pub dark_pool: Account<'info, DarkPoolPDA>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MatchDarkPool<'info> {
    #[account(mut)]
    pub dark_pool: Account<'info, DarkPoolPDA>,
    
    /// CHECK: Market state for reference price
    pub market_state: AccountInfo<'info>,
    
    #[account(mut)]
    pub keeper: Signer<'info>,
}

pub fn initialize_dark_pool(
    ctx: Context<InitializeDarkPool>,
    market_id: u128,
    minimum_size: u64,
    price_improvement_bps: u16,
) -> Result<()> {
    let dark_pool = &mut ctx.accounts.dark_pool;
    
    dark_pool.pool_id = market_id; // Using market_id as pool_id for simplicity
    dark_pool.market_id = market_id;
    dark_pool.minimum_size = minimum_size;
    dark_pool.price_improvement_bps = price_improvement_bps;
    dark_pool.total_volume = 0;
    dark_pool.num_trades = 0;
    dark_pool.last_match_slot = 0;
    
    Ok(())
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
    let dark_order = &mut ctx.accounts.dark_order;
    let dark_pool = &ctx.accounts.dark_pool;
    
    require!(size >= dark_pool.minimum_size, ErrorCode::SizeTooSmall);
    
    let order_id = ((Clock::get()?.slot as u128) << 64) | (Clock::get()?.unix_timestamp as u128);
    
    dark_order.order_id = order_id;
    dark_order.user = ctx.accounts.user.key();
    dark_order.pool_id = dark_pool.pool_id;
    dark_order.side = side;
    dark_order.outcome = outcome;
    dark_order.size = size;
    dark_order.min_price = min_price;
    dark_order.max_price = max_price;
    dark_order.time_in_force = time_in_force.clone();
    dark_order.created_at = Clock::get()?.unix_timestamp;
    
    dark_order.expires_at = match time_in_force {
        TimeInForce::IOC => Clock::get()?.unix_timestamp + 1, // 1 second
        TimeInForce::FOK => Clock::get()?.unix_timestamp + 1, // 1 second
        TimeInForce::GTT { expires_at } => expires_at,
    };
    
    Ok(())
}

pub fn match_dark_pool_orders(
    ctx: Context<MatchDarkPool>,
    buy_orders: Vec<Account<'_, DarkOrderPDA>>,
    sell_orders: Vec<Account<'_, DarkOrderPDA>>,
) -> Result<()> {
    // Extract values before mutable borrow
    let dark_pool = &ctx.accounts.dark_pool;
    let minimum_size = dark_pool.minimum_size;
    let price_improvement_bps = dark_pool.price_improvement_bps;

    // Sort orders by price priority
    let mut matched_pairs = Vec::new();

    for buy_order in buy_orders.iter() {
        for sell_order in sell_orders.iter() {
            // Check size compatibility
            if buy_order.size < minimum_size ||
               sell_order.size < minimum_size {
                continue;
            }

            // Check price overlap
            let price_match = match (buy_order.min_price, sell_order.max_price) {
                (Some(min), Some(max)) => min >= max,
                _ => true,  // No price limits
            };

            if !price_match {
                continue;
            }

            // Calculate match size
            let match_size = buy_order.size.min(sell_order.size);

            // Get reference price from lit market
            let reference_price = get_reference_price(
                &ctx.accounts.market_state,
                buy_order.outcome,
            )?;

            // Apply price improvement
            let execution_price = calculate_improved_price(
                reference_price,
                price_improvement_bps,
                &buy_order.side,
            )?;

            matched_pairs.push(MatchedOrder {
                buy_order_id: buy_order.order_id,
                sell_order_id: sell_order.order_id,
                size: match_size,
                price: execution_price,
            });
        }
    }

    // Execute matches and collect results
    let mut total_volume_added = 0u64;
    let mut trades_executed = 0u64;
    
    for matched in matched_pairs {
        execute_dark_pool_trade(&ctx.accounts.to_account_infos(), &matched)?;
        
        total_volume_added = total_volume_added
            .checked_add(matched.size)
            .ok_or(ErrorCode::MathOverflow)?;
        trades_executed += 1;
    }
    
    // Update dark pool state after all trades are executed
    let dark_pool = &mut ctx.accounts.dark_pool;
    dark_pool.total_volume = dark_pool.total_volume
        .checked_add(total_volume_added)
        .ok_or(ErrorCode::MathOverflow)?;
    dark_pool.num_trades += trades_executed;
    dark_pool.last_match_slot = Clock::get()?.slot;

    Ok(())
}

fn calculate_improved_price(
    reference_price: u64,
    improvement_bps: u16,
    side: &OrderSide,
) -> Result<u64> {
    let improvement = reference_price
        .checked_mul(improvement_bps as u64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(10000)
        .ok_or(ErrorCode::MathOverflow)?;

    match side {
        OrderSide::Buy => reference_price
            .checked_sub(improvement)
            .ok_or(ErrorCode::MathOverflow.into()),
        OrderSide::Sell => reference_price
            .checked_add(improvement)
            .ok_or(ErrorCode::MathOverflow.into()),
    }
}

// Placeholder functions
fn get_reference_price(
    _market_state: &AccountInfo,
    _outcome: u8,
) -> Result<u64> {
    // In production, this would read from the market state
    Ok(500_000_000_000_000_000) // 0.5 in fixed point
}

fn execute_dark_pool_trade(
    _accounts: &[AccountInfo],
    _matched: &MatchedOrder,
) -> Result<()> {
    // In production, this would execute the trade
    msg!("Executing dark pool trade: buy {} sell {} size {} price {}", 
         _matched.buy_order_id, 
         _matched.sell_order_id, 
         _matched.size, 
         _matched.price);
    Ok(())
}

#[event]
pub struct DarkPoolMatchEvent {
    pub buy_order_id: u128,
    pub sell_order_id: u128,
    pub size: u64,
    pub execution_price: u64,
}