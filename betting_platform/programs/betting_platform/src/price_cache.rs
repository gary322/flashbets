use anchor_lang::prelude::*;
use crate::errors::*;
use crate::events::*;

#[account]
pub struct PriceCachePDA {
    pub verse_id: u128,
    pub last_price: u64, // Fixed point with 8 decimals
    pub last_update_slot: u64,
    pub update_count: u64,
    pub is_stale: bool,
}

impl PriceCachePDA {
    pub const LEN: usize = 8 + 16 + 8 + 8 + 8 + 1;

    pub fn is_stale(&self, current_slot: u64) -> bool {
        // Stale if not updated for 150 slots (~1 minute)
        current_slot > self.last_update_slot + 150
    }
}

#[derive(Accounts)]
#[instruction(verse_id: u128)]
pub struct UpdatePriceCache<'info> {
    #[account(
        mut,
        seeds = [b"price_cache", verse_id.to_le_bytes().as_ref()],
        bump
    )]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn update_price_cache(
    ctx: Context<UpdatePriceCache>,
    verse_id: u128,
    new_price: u64,
) -> Result<()> {
    let cache = &mut ctx.accounts.price_cache;
    let clock = &ctx.accounts.clock;

    // Validate price change is within bounds (max 5% per update)
    if cache.last_price > 0 {
        let price_change = if new_price > cache.last_price {
            ((new_price - cache.last_price) * 10000) / cache.last_price
        } else {
            ((cache.last_price - new_price) * 10000) / cache.last_price
        };

        require!(
            price_change <= 500, // 5%
            BettingPlatformError::ExcessivePriceMovement
        );
    }

    cache.verse_id = verse_id;
    cache.last_price = new_price;
    cache.last_update_slot = clock.slot;
    cache.update_count += 1;
    cache.is_stale = false;

    emit!(PriceUpdateEvent {
        verse_id,
        price: new_price,
        slot: clock.slot,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(verse_id: u128)]
pub struct InitializePriceCache<'info> {
    #[account(
        init,
        payer = authority,
        space = PriceCachePDA::LEN,
        seeds = [b"price_cache", verse_id.to_le_bytes().as_ref()],
        bump
    )]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize_price_cache(
    ctx: Context<InitializePriceCache>,
    verse_id: u128,
) -> Result<()> {
    let cache = &mut ctx.accounts.price_cache;
    
    cache.verse_id = verse_id;
    cache.last_price = 0;
    cache.last_update_slot = 0;
    cache.update_count = 0;
    cache.is_stale = true;
    
    Ok(())
}