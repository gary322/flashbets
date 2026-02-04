//! Iceberg order implementation
//!
//! Splits large orders into smaller chunks to avoid market impact

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
use crate::trading::advanced_orders::{
    AdvancedOrder, OrderType, OrderStatus, PolymarketOrder, PolymarketOrderType,
};

pub const DEFAULT_DISPLAY_PERCENT: u64 = 1000; // 10% as per CLAUDE.md
pub const MAX_RANDOMIZATION: u64 = 1000;       // 10% max randomization

pub struct IcebergEngine;

impl IcebergEngine {
    /// Calculate next slice size with randomization
    pub fn calculate_next_slice(
        total_remaining: u64,
        display_size: u64,
        randomization: u8,
        seed: &[u8; 32],
    ) -> Result<u64, ProgramError> {
        if randomization > 10 {
            return Err(BettingPlatformError::InvalidRandomization.into());
        }
        
        let base_slice = display_size;
        
        if randomization == 0 {
            return Ok(base_slice.min(total_remaining));
        }
        
        // Use seed for deterministic randomization as per CLAUDE.md
        let hash = solana_program::keccak::hash(seed);
        let random_bytes = hash.to_bytes();
        let random_value = u64::from_le_bytes(
            random_bytes[0..8].try_into().unwrap()
        );
        
        // Apply randomization: ±randomization% (0-10% as per CLAUDE.md)
        let randomization_basis_points = (randomization as u64) * 100; // Convert to basis points
        let random_factor = random_value % (randomization_basis_points * 2);
        
        // Calculate adjustment amount
        let adjustment = base_slice
            .saturating_mul(random_factor)
            .saturating_div(10000 * 2); // Divide by 20000 to get proper percentage
        
        // Apply adjustment with 50% chance of increase or decrease
        let final_slice = if (random_value / 1000) % 2 == 0 {
            base_slice.saturating_add(adjustment)
        } else {
            base_slice.saturating_sub(adjustment)
        };
        
        Ok(final_slice.min(total_remaining))
    }
    
    /// Execute iceberg slice
    pub fn execute_slice(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order_id: [u8; 32],
    ) -> ProgramResult {
        // Account layout:
        // 0. Order account (mut)
        // 1. User account (signer)
        // 2. Price feed
        // 3. Keeper signer
        // 4. Polymarket interface
        // 5. System program
        // 6. Clock sysvar
        
        if accounts.len() < 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let order_account = &accounts[0];
        let user_account = &accounts[1];
        let price_feed_account = &accounts[2];
        let keeper_account = &accounts[3];
        let polymarket_interface = &accounts[4];
        let _system_program = &accounts[5];
        let clock = Clock::get()?;
        
        // Deserialize order
        let mut order_data = order_account.try_borrow_mut_data()?;
        let mut order = AdvancedOrder::deserialize(&mut &order_data[..])?;
        
        // Verify iceberg order
        match &order.order_type {
            OrderType::Iceberg { display_size, total_size, randomization } => {
                // Calculate slice size using deterministic seed as per CLAUDE.md
                let seed_components = [
                    &order_id[..],
                    &clock.slot.to_le_bytes()[..],
                ];
                let seed_data = seed_components.concat();
                let seed = solana_program::keccak::hash(&seed_data).to_bytes();
                
                let slice_size = Self::calculate_next_slice(
                    order.remaining_amount,
                    *display_size,
                    *randomization,
                    &seed,
                )?;
                
                // Route slice to Polymarket
                let polymarket_order = PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Market, // Or limit based on config
                    time_priority: false,
                    dark_pool: false,
                };
                
                // Serialize polymarket order for CPI
                let polymarket_order_data = borsh::to_vec(&polymarket_order)
                    .map_err(|_| ProgramError::InvalidAccountData)?;
                
                // Execute via keeper (simulated - in production would use actual CPI)
                msg!("Routing iceberg slice to Polymarket: {} units", slice_size);
                
                // Update order state
                order.filled_amount = order.filled_amount
                    .saturating_add(slice_size);
                order.remaining_amount = order.remaining_amount
                    .saturating_sub(slice_size);
                order.executions_count += 1;
                order.last_execution_slot = clock.slot;
                
                if order.remaining_amount == 0 {
                    order.status = OrderStatus::Filled;
                    
                    // Emit completion event
                    msg!(
                        "IcebergCompleted - order_id: {:?}, total_filled: {}, average_price: {}, slices: {}",
                        order_id,
                        order.filled_amount,
                        order.average_price,
                        order.executions_count
                    );
                }
                
                // Serialize updated order back
                order.serialize(&mut *order_data)?;
                
                Ok(())
            }
            _ => Err(BettingPlatformError::InvalidOrderType.into()),
        }
    }
}

// Context structs for type safety
pub struct ExecuteIcebergSliceContext<'a, 'info> {
    pub order: &'a AccountInfo<'info>,
    pub user: &'a AccountInfo<'info>,
    pub price_feed: &'a AccountInfo<'info>,
    pub keeper: &'a AccountInfo<'info>,
    pub polymarket_interface: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}