//! Dark pool implementation
//!
//! Private order matching for large trades

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;
use crate::math::{U64F64, U128F128};
use crate::trading::advanced_orders::{
    DarkOrderStatus, PolymarketOrder, PolymarketOrderType, PriceFeed, Side,
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DarkPool {
    pub pool_id: [u8; 32],
    pub market_id: [u8; 32],
    pub status: DarkPoolStatus,
    pub min_size: u64,           // Minimum order size
    pub settlement_frequency: u64, // Slots between settlements
    pub last_settlement: u64,
    
    // Aggregated orders
    pub buy_volume: u64,
    pub sell_volume: u64,
    pub buy_value: U128F128,     // Total value for VWAP
    pub sell_value: U128F128,
    pub participants: u16,
    
    // Settlement price (from last matching)
    pub last_match_price: Option<U64F64>,
    pub total_matched: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum DarkPoolStatus {
    Active,
    Settling,
    Paused,
}

// Dark order account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DarkOrder {
    pub pool_id: [u8; 32],
    pub user: Pubkey,
    pub side: Side,
    pub size: u64,
    pub limit_price: Option<U64F64>,
    pub submitted_slot: u64,
    pub status: DarkOrderStatus,
}

pub struct DarkPoolEngine;

impl DarkPoolEngine {
    /// Submit order to dark pool
    pub fn submit_dark_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        pool_id: [u8; 32],
        side: Side,
        size: u64,
        limit_price: Option<U64F64>,
    ) -> ProgramResult {
        // Account layout:
        // 0. Pool account (mut)
        // 1. Dark order account (mut)
        // 2. User account (signer)
        // 3. System program
        // 4. Clock sysvar
        
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let pool_account = &accounts[0];
        let order_account = &accounts[1];
        let user_account = &accounts[2];
        let _system_program = &accounts[3];
        let clock = Clock::get()?;
        
        // Verify user is signer
        if !user_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize pool
        let mut pool_data = pool_account.try_borrow_mut_data()?;
        let mut pool = DarkPool::try_from_slice(&pool_data)?;
        
        // Verify pool is active
        if pool.status != DarkPoolStatus::Active {
            return Err(BettingPlatformError::DarkPoolNotActive.into());
        }
        
        // Check minimum size
        if size < pool.min_size {
            return Err(BettingPlatformError::BelowMinimumSize.into());
        }
        
        // Store order details in PDA (encrypted/hashed in production)
        let dark_order = DarkOrder {
            pool_id,
            user: *user_account.key,
            side,
            size,
            limit_price,
            submitted_slot: clock.slot,
            status: DarkOrderStatus::Pending,
        };
        
        // Update pool aggregates (without revealing individual orders)
        match side {
            Side::Buy => {
                pool.buy_volume = pool.buy_volume.saturating_add(size);
                if let Some(price) = limit_price {
                    let value = U128F128::from_num(size as u128).saturating_mul(U128F128::from_num(price.to_num() as u128));
                    pool.buy_value = pool.buy_value.saturating_add(value);
                }
            }
            Side::Sell => {
                pool.sell_volume = pool.sell_volume.saturating_add(size);
                if let Some(price) = limit_price {
                    let value = U128F128::from_num(size as u128).saturating_mul(U128F128::from_num(price.to_num() as u128));
                    pool.sell_value = pool.sell_value.saturating_add(value);
                }
            }
        }
        
        pool.participants += 1;
        
        // Emit obfuscated event
        msg!(
            "DarkOrderSubmitted - pool_id: {:?}, side: {:?}, size_bucket: {}, participant_count: {}",
            pool_id,
            side,
            Self::get_size_bucket(size),
            pool.participants
        );
        
        // Serialize updated data
        pool.serialize(&mut *pool_data)?;
        
        // Serialize dark order
        let mut order_data = order_account.try_borrow_mut_data()?;
        dark_order.serialize(&mut *order_data)?;
        
        Ok(())
    }
    
    /// Match dark pool orders (keeper-triggered)
    pub fn match_dark_pool(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        pool_id: [u8; 32],
    ) -> ProgramResult {
        // Account layout:
        // 0. Pool account (mut)
        // 1. Price feed
        // 2. Keeper account (signer)
        // 3. Polymarket interface
        // 4. System program
        // 5. Clock sysvar
        
        if accounts.len() < 6 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let pool_account = &accounts[0];
        let price_feed_account = &accounts[1];
        let keeper_account = &accounts[2];
        let polymarket_interface = &accounts[3];
        let _system_program = &accounts[4];
        let clock = Clock::get()?;
        
        // Verify keeper is signer
        if !keeper_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize pool
        let mut pool_data = pool_account.try_borrow_mut_data()?;
        let mut pool = DarkPool::try_from_slice(&pool_data)?;
        
        // Deserialize price feed
        let price_feed_data = price_feed_account.try_borrow_data()?;
        let price_feed = PriceFeed::try_from_slice(&price_feed_data)?;
        
        // Check if it's time to settle
        if clock.slot < pool.last_settlement.saturating_add(pool.settlement_frequency) {
            return Err(BettingPlatformError::TooEarlyToSettle.into());
        }
        
        pool.status = DarkPoolStatus::Settling;
        
        // Calculate crossing price
        let crossing_price = Self::calculate_crossing_price(
            pool.buy_volume,
            pool.sell_volume,
            pool.buy_value,
            pool.sell_value,
            &price_feed,
        )?;
        
        // Determine matched volume
        let matched_volume = pool.buy_volume.min(pool.sell_volume);
        
        if matched_volume > 0 {
            // Route matched volume to Polymarket at crossing price
            let polymarket_order = PolymarketOrder {
                market_id: pool.market_id,
                side: Side::Buy, // Neutral - just executing crosses
                size: matched_volume,
                order_type: PolymarketOrderType::Limit { price: crossing_price },
                time_priority: false,
                dark_pool: true,
            };
            
            msg!(
                "Settling dark pool: {} units at price {}",
                matched_volume,
                crossing_price.to_num()
            );
            
            pool.last_match_price = Some(crossing_price);
            pool.total_matched = pool.total_matched.saturating_add(matched_volume);
        }
        
        // Reset pool for next round
        pool.buy_volume = pool.buy_volume.saturating_sub(matched_volume);
        pool.sell_volume = pool.sell_volume.saturating_sub(matched_volume);
        pool.buy_value = U128F128::zero();
        pool.sell_value = U128F128::zero();
        pool.participants = 0;
        pool.last_settlement = clock.slot;
        pool.status = DarkPoolStatus::Active;
        
        msg!(
            "DarkPoolSettled - pool_id: {:?}, matched_volume: {}, crossing_price: {}, remaining_buy: {}, remaining_sell: {}",
            pool_id,
            matched_volume,
            crossing_price.to_num(),
            pool.buy_volume,
            pool.sell_volume
        );
        
        // Serialize updated pool
        pool.serialize(&mut *pool_data)?;
        
        Ok(())
    }
    
    /// Calculate volume-weighted crossing price as per CLAUDE.md
    fn calculate_crossing_price(
        buy_volume: u64,
        sell_volume: u64,
        buy_value: U128F128,
        sell_value: U128F128,
        price_feed: &PriceFeed,
    ) -> Result<U64F64, ProgramError> {
        // If no limits, use mid-market price
        if buy_value.is_zero() && sell_value.is_zero() {
            return Ok(price_feed.mid_price());
        }
        
        // Calculate VWAP for each side as per CLAUDE.md
        let buy_vwap = if buy_volume > 0 && !buy_value.is_zero() {
            // VWAP = total value / total volume
            buy_value.saturating_div(U128F128::from_num(buy_volume as u128))
        } else {
            // No buy orders with limits, use best ask
            U128F128::from_num(price_feed.best_ask.to_num() as u128)
        };
        
        let sell_vwap = if sell_volume > 0 && !sell_value.is_zero() {
            // VWAP = total value / total volume  
            sell_value.saturating_div(U128F128::from_num(sell_volume as u128))
        } else {
            // No sell orders with limits, use best bid
            U128F128::from_num(price_feed.best_bid.to_num() as u128)
        };
        
        // Crossing price is volume-weighted average of both VWAPs
        let total_volume = buy_volume.saturating_add(sell_volume);
        if total_volume == 0 {
            return Ok(price_feed.mid_price());
        }
        
        let buy_weight = U128F128::from_num(buy_volume as u128);
        let sell_weight = U128F128::from_num(sell_volume as u128);
        
        // Weighted price = (buy_vwap * buy_volume + sell_vwap * sell_volume) / total_volume
        let weighted_price = buy_vwap.saturating_mul(buy_weight)
            .saturating_add(sell_vwap.saturating_mul(sell_weight))
            .saturating_div(U128F128::from_num(total_volume as u128));
        
        // Convert back to U64F64
        Ok(U64F64::from_num(weighted_price.to_num() as u64))
    }
    
    /// Get obfuscated size bucket for privacy
    fn get_size_bucket(size: u64) -> u8 {
        match size {
            0..=1000 => 0,           // Small
            1001..=10000 => 1,       // Medium
            10001..=100000 => 2,     // Large
            _ => 3,                  // Whale
        }
    }
}

// Dark pool constants
impl DarkPool {
    pub const SIZE: usize = 32 + // pool_id
        32 + // market_id
        1 + // status
        8 + // min_size
        8 + // settlement_frequency
        8 + // last_settlement
        8 + // buy_volume
        8 + // sell_volume
        32 + // buy_value (U128F128)
        32 + // sell_value
        2 + // participants
        1 + 16 + // last_match_price Option<U64F64>
        8; // total_matched
}