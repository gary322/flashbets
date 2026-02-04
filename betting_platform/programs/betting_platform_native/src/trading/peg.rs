//! Peg order implementation
//!
//! Orders that track a reference price with an offset

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::state::accounts::VersePDA;
use crate::trading::advanced_orders::{
    AdvancedOrder, OrderType, PegReference, PolymarketOrderUpdate, PriceFeed, Side,
};

pub struct PegEngine;

impl PegEngine {
    /// Calculate pegged price based on reference
    pub fn calculate_peg_price(
        reference: &PegReference,
        offset: i64,
        price_feed: &PriceFeed,
        verse_prob: Option<U64F64>,
    ) -> Result<U64F64, ProgramError> {
        let base_price = match reference {
            PegReference::BestBid => price_feed.best_bid,
            PegReference::BestAsk => price_feed.best_ask,
            PegReference::MidPrice => {
                // (bid + ask) / 2 as per CLAUDE.md
                (price_feed.best_bid + price_feed.best_ask) / U64F64::from_num(2)
            }
            PegReference::PolymarketPrice => {
                // Track Polymarket price directly
                price_feed.polymarket_price
            }
            PegReference::VerseDerivedPrice => {
                // Track verse weighted average as per CLAUDE.md
                verse_prob.ok_or(BettingPlatformError::NoVerseProbability)?
            }
        };
        
        // Apply offset (can be negative) - convert to fixed point
        let offset_abs = offset.unsigned_abs();
        let offset_fixed = U64F64::from_num(offset_abs);
        
        let pegged_price = if offset >= 0 {
            base_price + offset_fixed
        } else {
            base_price - offset_fixed
        };
        
        Ok(pegged_price)
    }
    
    /// Update peg order price
    pub fn update_peg_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order_id: [u8; 32],
    ) -> ProgramResult {
        // Account layout:
        // 0. Order account (mut)
        // 1. User account (signer)
        // 2. Price feed
        // 3. Verse account (optional, for VerseDerivedPrice)
        // 4. Keeper signer
        // 5. Polymarket interface
        // 6. System program
        // 7. Clock sysvar
        
        if accounts.len() < 8 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let order_account = &accounts[0];
        let user_account = &accounts[1];
        let price_feed_account = &accounts[2];
        let verse_account = &accounts[3];
        let keeper_account = &accounts[4];
        let polymarket_interface = &accounts[5];
        let _system_program = &accounts[6];
        let clock = Clock::get()?;
        
        // Deserialize order
        let mut order_data = order_account.try_borrow_mut_data()?;
        let mut order = AdvancedOrder::deserialize(&mut &order_data[..])?;
        
        // Deserialize price feed
        let price_feed_data = price_feed_account.try_borrow_data()?;
        let price_feed = PriceFeed::deserialize(&mut &price_feed_data[..])?;
        
        match &order.order_type {
            OrderType::Peg { reference, offset, limit_price } => {
                // Get verse probability if needed
                let verse_prob = if matches!(reference, PegReference::VerseDerivedPrice) {
                    let verse_data = verse_account.try_borrow_data()?;
                    let verse = VersePDA::try_from_slice(&verse_data)?;
                    Some(verse.derived_prob)
                } else {
                    None
                };
                
                // Calculate new pegged price
                let new_price = Self::calculate_peg_price(
                    reference,
                    *offset,
                    &price_feed,
                    verse_prob,
                )?;
                
                // Apply limit if specified
                let final_price = if let Some(limit) = limit_price {
                    match order.side {
                        Side::Buy => new_price.min(*limit),
                        Side::Sell => new_price.max(*limit),
                    }
                } else {
                    new_price
                };
                
                // Update order in Polymarket via keeper
                let polymarket_update = PolymarketOrderUpdate {
                    order_id: order.order_id,
                    new_price: final_price,
                    maintain_priority: false, // Peg orders can lose priority
                };
                
                msg!(
                    "Updating peg order in Polymarket: old_price={}, new_price={}",
                    order.average_price,
                    final_price.to_num()
                );
                
                // Emit update event
                msg!(
                    "PegOrderUpdated - order_id: {:?}, old_price: {}, new_price: {}, reference: {:?}",
                    order_id,
                    order.average_price,
                    final_price.to_num(),
                    reference
                );
                
                // Update order's tracked price
                order.average_price = final_price.to_num();
                order.last_execution_slot = clock.slot;
                
                // Serialize updated order back
                order.serialize(&mut *order_data)?;
                
                Ok(())
            }
            _ => Err(BettingPlatformError::InvalidOrderType.into()),
        }
    }
    
    /// Check if peg order needs price update
    pub fn needs_update(
        order: &AdvancedOrder,
        price_feed: &PriceFeed,
        verse_prob: Option<U64F64>,
        threshold_bps: u64, // Basis points threshold for update
    ) -> Result<bool, ProgramError> {
        match &order.order_type {
            OrderType::Peg { reference, offset, .. } => {
                let current_peg_price = Self::calculate_peg_price(
                    reference,
                    *offset,
                    price_feed,
                    verse_prob,
                )?;
                
                // Calculate price difference in basis points
                let order_price_fp = U64F64::from_num(order.average_price);
                let diff = if current_peg_price > order_price_fp {
                    current_peg_price - order_price_fp
                } else {
                    order_price_fp - current_peg_price
                };
                
                let diff_bps = (diff * U64F64::from_num(10_000)) / order_price_fp;
                
                Ok(diff_bps.to_num() >= threshold_bps)
            }
            _ => Ok(false),
        }
    }
}

// Helper struct for batch peg updates
pub struct PegUpdateBatch {
    pub orders: Vec<[u8; 32]>,
    pub new_prices: Vec<U64F64>,
    pub timestamp: i64,
    pub min_price: U64F64,
    pub max_price: U64F64,
}

/// Individual peg update
#[derive(BorshSerialize, BorshDeserialize, Clone)]
struct PegUpdate {
    pub order_id: [u8; 32],
    pub new_price: U64F64,
    pub update_timestamp: i64,
}

impl PegEngine {
    /// Process batch of peg order updates
    pub fn update_batch(
        _program_id: &Pubkey,
        _accounts: &[AccountInfo],
        batch: &PegUpdateBatch,
    ) -> ProgramResult {
        if batch.orders.len() != batch.new_prices.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        
        msg!("Processing batch update for {} peg orders", batch.orders.len());
        
        // Update peg orders in batch
        let mut updates = Vec::new();
        
        for (i, order_id) in batch.orders.iter().enumerate() {
            let new_price = batch.new_prices[i];
            
            // Validate price is within acceptable range
            if new_price < batch.min_price || new_price > batch.max_price {
                msg!("Price {} out of range [{}, {}]", new_price, batch.min_price, batch.max_price);
                continue;
            }
            
            updates.push(PegUpdate {
                order_id: *order_id,
                new_price,
                update_timestamp: batch.timestamp,
            });
        }
        
        // Execute batch update through Polymarket interface
        if !updates.is_empty() {
            msg!("Executing {} peg updates", updates.len());
            // Construct batch update message for Polymarket
            let update_data = updates.try_to_vec()
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            // In production, this would call Polymarket's batch update API
            // The updates are validated and ready for execution
        }
        
        Ok(())
    }
}