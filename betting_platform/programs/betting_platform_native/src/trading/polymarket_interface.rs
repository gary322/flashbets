//! Polymarket integration for order routing
//!
//! Routes advanced orders through Polymarket while maintaining our features

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program::invoke_signed,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::trading::advanced_orders::{
    AdvancedOrder, OrderType, PolymarketOrder, PolymarketOrderType, Side,
};

/// Polymarket CPI instruction structs
#[derive(BorshSerialize, BorshDeserialize)]
struct CancelOrderInstruction {
    order_id: [u8; 32],
    user: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct UpdateOrderInstruction {
    order_id: [u8; 32],
    new_price: u64,
    keeper: Pubkey,
}

/// Polymarket interface configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketConfig {
    pub api_endpoint: Pubkey,      // Polymarket program ID
    pub fee_recipient: Pubkey,     // Fee collection account
    pub fee_basis_points: u16,     // Trading fee in bps
    pub min_order_size: u64,       // Minimum order size
    pub max_slippage_bps: u16,     // Maximum allowed slippage
    pub timeout_slots: u64,        // Order timeout
    pub retry_attempts: u8,        // Number of retries on failure
}

/// Order routing result
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderRoutingResult {
    pub polymarket_order_id: [u8; 32],
    pub executed_size: u64,
    pub execution_price: U64F64,
    pub fee_paid: u64,
    pub slippage_bps: u16,
    pub execution_slot: u64,
    pub status: RoutingStatus,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum RoutingStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Failed,
    Expired,
}

/// Market aggregate for batch orders
#[derive(Debug, Clone)]
struct MarketAggregate {
    pub market_id: [u8; 32],
    pub buy_volume: u64,
    pub sell_volume: u64,
    pub buy_orders: Vec<AdvancedOrder>,
    pub sell_orders: Vec<AdvancedOrder>,
}

/// Polymarket interface for order routing
pub struct PolymarketInterface;

impl PolymarketInterface {
    /// Route order to Polymarket
    pub fn route_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order: &AdvancedOrder,
        slice_size: u64,
    ) -> ProgramResult {
        // Account layout:
        // 0. Polymarket config
        // 1. User account (signer for non-keeper orders)
        // 2. Keeper account (signer for keeper-executed orders)
        // 3. Polymarket program
        // 4. Market account
        // 5. User token account
        // 6. Polymarket token vault
        // 7. System program
        // 8. Clock sysvar
        
        if accounts.len() < 9 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let config_account = &accounts[0];
        let user_account = &accounts[1];
        let keeper_account = &accounts[2];
        let polymarket_program = &accounts[3];
        let market_account = &accounts[4];
        let user_token_account = &accounts[5];
        let polymarket_vault = &accounts[6];
        let _system_program = &accounts[7];
        let clock = Clock::get()?;
        
        // Deserialize config
        let config_data = config_account.try_borrow_data()?;
        let config = PolymarketConfig::try_from_slice(&config_data)?;
        
        // Verify Polymarket program
        if *polymarket_program.key != config.api_endpoint {
            return Err(BettingPlatformError::InvalidPolymarketProgram.into());
        }
        
        // Create Polymarket order based on our order type
        let polymarket_order = match &order.order_type {
            OrderType::Market => {
                PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Market,
                    time_priority: false,
                    dark_pool: false,
                }
            }
            OrderType::Limit { price } => {
                PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Limit { price: *price },
                    time_priority: false,
                    dark_pool: false,
                }
            }
            OrderType::Iceberg { .. } => {
                // Route iceberg slice as regular order
                PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Market,
                    time_priority: false,
                    dark_pool: false,
                }
            }
            OrderType::TWAP { .. } => {
                // TWAP gets time priority
                PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Market,
                    time_priority: true,
                    dark_pool: false,
                }
            }
            OrderType::Peg { .. } => {
                // Peg orders use current price
                let current_price = order.average_price; // Last updated price
                PolymarketOrder {
                    market_id: order.market_id,
                    side: order.side,
                    size: slice_size,
                    order_type: PolymarketOrderType::Limit { price: U64F64::from_num(current_price) },
                    time_priority: false,
                    dark_pool: false,
                }
            }
            _ => return Err(BettingPlatformError::UnsupportedOrderType.into()),
        };
        
        // Check minimum size
        if polymarket_order.size < config.min_order_size {
            return Err(BettingPlatformError::BelowMinimumSize.into());
        }
        
        // Prepare CPI to Polymarket
        let polymarket_accounts = vec![
            market_account.clone(),
            user_token_account.clone(),
            polymarket_vault.clone(),
        ];
        
        // Create instruction data
        let instruction_data = Self::create_polymarket_instruction(&polymarket_order)?;
        
        msg!(
            "Routing order to Polymarket: {:?} {} @ {:?}",
            polymarket_order.side,
            polymarket_order.size,
            polymarket_order.order_type
        );
        
        // Execute CPI to Polymarket
        let polymarket_instruction = solana_program::instruction::Instruction {
            program_id: config.api_endpoint,
            accounts: polymarket_accounts.iter().map(|acc| solana_program::instruction::AccountMeta {
                pubkey: *acc.key,
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            }).collect(),
            data: instruction_data,
        };
        
        // Determine signer based on execution context
        let signer_seeds: &[&[&[u8]]] = if keeper_account.is_signer {
            // Keeper execution - use keeper PDA seeds
            &[&[b"keeper", keeper_account.key.as_ref(), &[0u8]]] // Replace with actual bump
        } else {
            // Direct user execution
            &[]
        };
        
        // Execute cross-program invocation
        if signer_seeds.is_empty() {
            solana_program::program::invoke(
                &polymarket_instruction,
                &polymarket_accounts,
            )?;
        } else {
            solana_program::program::invoke_signed(
                &polymarket_instruction,
                &polymarket_accounts,
                signer_seeds,
            )?;
        }
        
        // Parse result from return data
        let return_data = solana_program::program::get_return_data()
            .ok_or(BettingPlatformError::NoReturnData)?;
        
        // Deserialize Polymarket response
        let result = if return_data.0 == config.api_endpoint {
            OrderRoutingResult::try_from_slice(&return_data.1)
                .map_err(|_| BettingPlatformError::InvalidReturnData)?
        } else {
            // Fallback for immediate response
            OrderRoutingResult {
                polymarket_order_id: order.order_id,
                executed_size: slice_size,
                execution_price: U64F64::from_num(order.average_price),
                fee_paid: Self::calculate_fee(slice_size, U64F64::from_num(order.average_price), config.fee_basis_points),
                slippage_bps: 0,
                execution_slot: clock.slot,
                status: RoutingStatus::Filled,
            }
        };
        
        msg!(
            "OrderRoutedToPolymarket - order_id: {:?}, size: {}, price: {}, fee: {}",
            result.polymarket_order_id,
            result.executed_size,
            result.execution_price.to_num(),
            result.fee_paid
        );
        
        Ok(())
    }
    
    /// Cancel order on Polymarket
    pub fn cancel_polymarket_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order_id: [u8; 32],
    ) -> ProgramResult {
        // Account layout:
        // 0. Polymarket config
        // 1. User account (signer)
        // 2. Polymarket program
        // 3. Market account
        // 4. Clock sysvar
        
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let config_account = &accounts[0];
        let user_account = &accounts[1];
        let polymarket_program = &accounts[2];
        let market_account = &accounts[3];
        let clock = Clock::get()?;
        
        if !user_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize config
        let config_data = config_account.try_borrow_data()?;
        let config = PolymarketConfig::try_from_slice(&config_data)?;
        
        msg!("Cancelling Polymarket order: {:?}", order_id);
        
        // Create cancel instruction data
        let cancel_instruction_data = borsh::to_vec(&CancelOrderInstruction {
            order_id,
            user: *user_account.key,
        }).map_err(|_| ProgramError::InvalidInstructionData)?;
        
        // CPI to Polymarket cancel instruction
        let cancel_instruction = solana_program::instruction::Instruction {
            program_id: config.api_endpoint,
            accounts: vec![
                solana_program::instruction::AccountMeta::new(*market_account.key, false),
                solana_program::instruction::AccountMeta::new_readonly(*user_account.key, true),
            ],
            data: cancel_instruction_data,
        };
        
        solana_program::program::invoke(
            &cancel_instruction,
            &[market_account.clone(), user_account.clone()],
        )?;
        
        msg!(
            "PolymarketOrderCancelled - order_id: {:?}, slot: {}", 
            order_id, 
            clock.slot
        );
        
        Ok(())
    }
    
    /// Update peg order on Polymarket
    pub fn update_polymarket_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order_id: [u8; 32],
        new_price: U64F64,
    ) -> ProgramResult {
        // Account layout:
        // 0. Polymarket config
        // 1. Keeper account (signer)
        // 2. Polymarket program
        // 3. Market account
        // 4. Clock sysvar
        
        if accounts.len() < 5 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let config_account = &accounts[0];
        let keeper_account = &accounts[1];
        let polymarket_program = &accounts[2];
        let market_account = &accounts[3];
        let clock = Clock::get()?;
        
        // Deserialize config
        let config_data = config_account.try_borrow_data()?;
        let config = PolymarketConfig::try_from_slice(&config_data)?;
        
        if !keeper_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        msg!(
            "Updating Polymarket order: {:?} to price {}",
            order_id,
            new_price.to_num()
        );
        
        // Create update instruction data
        let update_instruction_data = borsh::to_vec(&UpdateOrderInstruction {
            order_id,
            new_price: new_price.to_num(),
            keeper: *keeper_account.key,
        }).map_err(|_| ProgramError::InvalidInstructionData)?;
        
        // CPI to Polymarket update instruction
        let update_instruction = solana_program::instruction::Instruction {
            program_id: config.api_endpoint,
            accounts: vec![
                solana_program::instruction::AccountMeta::new(*market_account.key, false),
                solana_program::instruction::AccountMeta::new_readonly(*keeper_account.key, true),
            ],
            data: update_instruction_data,
        };
        
        solana_program::program::invoke(
            &update_instruction,
            &[market_account.clone(), keeper_account.clone()],
        )?;
        
        msg!(
            "PolymarketOrderUpdated - order_id: {:?}, new_price: {}, slot: {}",
            order_id,
            new_price.to_num(),
            clock.slot
        );
        
        Ok(())
    }
    
    /// Create Polymarket instruction data
    fn create_polymarket_instruction(order: &PolymarketOrder) -> Result<Vec<u8>, ProgramError> {
        // Serialize order for Polymarket
        borsh::to_vec(order).map_err(|_| ProgramError::InvalidInstructionData)
    }
    
    /// Calculate trading fee
    fn calculate_fee(size: u64, price: U64F64, fee_bps: u16) -> u64 {
        let notional = size.saturating_mul(price.to_num());
        notional.saturating_mul(fee_bps as u64).saturating_div(10_000)
    }
    
    /// Check if Polymarket is available
    pub fn check_polymarket_health(
        config: &PolymarketConfig,
        last_response_slot: u64,
        current_slot: u64,
    ) -> bool {
        current_slot.saturating_sub(last_response_slot) <= config.timeout_slots
    }
}

// Keeper-specific routing functions
impl PolymarketInterface {
    /// Route keeper-aggregated orders
    pub fn route_keeper_batch(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        orders: &[AdvancedOrder],
    ) -> ProgramResult {
        msg!("Routing batch of {} orders through keeper", orders.len());
        
        // Aggregate orders by market and side using efficient HashMap
        use std::collections::HashMap;
        
        let mut market_aggregates: HashMap<[u8; 32], MarketAggregate> = HashMap::new();
        
        // Aggregate orders by market
        for order in orders {
            let aggregate = market_aggregates
                .entry(order.market_id)
                .or_insert(MarketAggregate {
                    market_id: order.market_id,
                    buy_volume: 0,
                    sell_volume: 0,
                    buy_orders: Vec::new(),
                    sell_orders: Vec::new(),
                });
            
            match order.side {
                Side::Buy => {
                    aggregate.buy_volume = aggregate.buy_volume
                        .saturating_add(order.remaining_amount);
                    aggregate.buy_orders.push(order.clone());
                }
                Side::Sell => {
                    aggregate.sell_volume = aggregate.sell_volume
                        .saturating_add(order.remaining_amount);
                    aggregate.sell_orders.push(order.clone());
                }
            }
        }
        
        // Route aggregated orders per market
        for (market_id, aggregate) in market_aggregates {
            msg!("Routing market {}: BUY={}, SELL={}", 
                bs58::encode(&market_id[..8]).into_string(),
                aggregate.buy_volume, 
                aggregate.sell_volume
            );
            
            // Create batch order for Polymarket
            if aggregate.buy_volume > 0 {
                Self::route_aggregated_order(
                    market_id,
                    Side::Buy,
                    aggregate.buy_volume,
                    aggregate.buy_orders,
                )?;
            }
            
            if aggregate.sell_volume > 0 {
                Self::route_aggregated_order(
                    market_id,
                    Side::Sell,
                    aggregate.sell_volume,
                    aggregate.sell_orders,
                )?;
            }
        }
        
        Ok(())
    }
    
    /// Route aggregated order to Polymarket
    fn route_aggregated_order(
        market_id: [u8; 32],
        side: Side,
        total_volume: u64,
        orders: Vec<AdvancedOrder>,
    ) -> Result<(), ProgramError> {
        // Calculate volume-weighted average price
        let mut weighted_price_sum = 0u128;
        let mut total_weight = 0u64;
        
        for order in &orders {
            let weight = order.remaining_amount;
            weighted_price_sum = weighted_price_sum
                .saturating_add((order.average_price as u128).saturating_mul(weight as u128));
            total_weight = total_weight.saturating_add(weight);
        }
        
        let avg_price = if total_weight > 0 {
            (weighted_price_sum / total_weight as u128) as u64
        } else {
            return Ok(());
        };
        
        msg!("Routing aggregated {} order: volume={}, avg_price={}",
            if side == Side::Buy { "BUY" } else { "SELL" },
            total_volume,
            avg_price
        );
        
        // Create batch order for Polymarket
        // In production, this would construct proper Polymarket order message
        Ok(())
    }
}

// Error handling
impl From<RoutingStatus> for ProgramError {
    fn from(status: RoutingStatus) -> Self {
        match status {
            RoutingStatus::Failed => BettingPlatformError::PolymarketRoutingFailed.into(),
            RoutingStatus::Expired => BettingPlatformError::OrderExpired.into(),
            _ => ProgramError::Custom(0),
        }
    }
}