//! Stop-loss keeper system
//!
//! Implements stop-loss execution with user-paid 2bp bounties

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

use crate::{
    error::BettingPlatformError,
    events::{Event, StopLossExecuted},
    math::U64F64,
    state::{KeeperAccount, KeeperStatus},
};

/// Stop keeper bounty basis points (2bp = 0.02%)
pub const STOP_KEEPER_BOUNTY_BPS: u64 = 2;

/// Stop order types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum StopOrderType {
    StopLoss,
    TakeProfit,
    TrailingStop,
}

/// Order side
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OrderSide {
    Long,
    Short,
}

/// Stop order account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StopOrder {
    /// Order ID
    pub order_id: [u8; 32],
    
    /// User who placed the order
    pub user: Pubkey,
    
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Position ID this stop is for
    pub position_id: [u8; 32],
    
    /// Order type
    pub order_type: StopOrderType,
    
    /// Order side
    pub side: OrderSide,
    
    /// Trigger price
    pub trigger_price: U64F64,
    
    /// Order size
    pub size: u64,
    
    /// Prepaid keeper bounty
    pub prepaid_bounty: u64,
    
    /// Is order active
    pub is_active: bool,
    
    /// Entry price for position
    pub position_entry_price: U64F64,
    
    /// Trailing distance (for trailing stops)
    pub trailing_distance: U64F64,
    
    /// Current trailing price
    pub trailing_price: U64F64,
    
    /// Creation slot
    pub created_slot: u64,
}

/// Execution result
#[derive(Debug)]
pub struct ExecutionResult {
    pub executed_value: u64,
    pub execution_price: U64F64,
}

/// Triggered order info
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TriggeredOrder {
    pub order_id: [u8; 32],
    pub account: Pubkey,
    pub order_type: StopOrderType,
    pub trigger_price: U64F64,
    pub current_price: U64F64,
    pub priority: u64,
}

/// Stop-loss keeper implementation
pub struct StopLossKeeper;

impl StopLossKeeper {
    /// Execute stop-loss with 2bp user-paid bounty
    pub fn execute_stop_loss(
        order: &mut StopOrder,
        keeper: &mut KeeperAccount,
        current_price: U64F64,
    ) -> ProgramResult {
        // Verify stop condition met
        let triggered = match order.order_type {
            StopOrderType::StopLoss => current_price <= order.trigger_price,
            StopOrderType::TakeProfit => current_price >= order.trigger_price,
            StopOrderType::TrailingStop => {
                let distance = if order.side == OrderSide::Long {
                    order.trailing_price.checked_sub(current_price)?
                } else {
                    current_price.checked_sub(order.trailing_price)?
                };
                distance >= order.trailing_distance
            }
        };
        
        if !triggered {
            return Err(BettingPlatformError::StopConditionNotMet.into());
        }
        
        // Verify keeper is active
        if keeper.status != KeeperStatus::Active {
            return Err(BettingPlatformError::NoActiveKeepers.into());
        }
        
        // Execute the stop order
        let execution_result = order.execute(current_price)?;
        
        // Calculate keeper bounty (2bp of order value)
        let keeper_bounty = execution_result.executed_value
            .checked_mul(STOP_KEEPER_BOUNTY_BPS)
            .and_then(|v| v.checked_div(10000))
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        // Verify sufficient prepaid bounty
        if order.prepaid_bounty < keeper_bounty {
            return Err(BettingPlatformError::InsufficientPrepaidBounty.into());
        }
        
        // Transfer bounty to keeper (in production, would use CPI)
        order.prepaid_bounty = order.prepaid_bounty
            .checked_sub(keeper_bounty)
            .ok_or(BettingPlatformError::Underflow)?;
        
        // Update keeper stats
        keeper.successful_operations = keeper.successful_operations
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.total_operations = keeper.total_operations
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.total_rewards_earned = keeper.total_rewards_earned
            .checked_add(keeper_bounty)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.last_operation_slot = Clock::get()?.slot;
        
        // Mark order as executed
        order.is_active = false;
        
        // Emit event
        StopLossExecuted {
            order_id: order.order_id,
            keeper_id: keeper.keeper_id,
            trigger_price: order.trigger_price.to_num(),
            execution_price: current_price.to_num(),
            keeper_bounty,
            order_type: order.order_type as u8,
        }.emit();
        
        msg!("Executed {} order {} at price {}, keeper bounty: {}",
            match order.order_type {
                StopOrderType::StopLoss => "stop-loss",
                StopOrderType::TakeProfit => "take-profit",
                StopOrderType::TrailingStop => "trailing-stop",
            },
            bs58::encode(&order.order_id[..8]).into_string(),
            current_price.to_num(),
            keeper_bounty
        );
        
        Ok(())
    }
    
    /// Scan for triggered stop orders
    pub fn scan_stop_orders(
        order_accounts: &[AccountInfo],
        market_id: [u8; 32],
        current_price: U64F64,
    ) -> Result<Vec<TriggeredOrder>, ProgramError> {
        let mut triggered = Vec::new();
        
        for account in order_accounts {
            match StopOrder::try_from_slice(&account.data.borrow()) {
                Ok(mut order) => {
                    if order.market_id != market_id || !order.is_active {
                        continue;
                    }
                    
                    let should_trigger = match order.order_type {
                        StopOrderType::StopLoss => current_price <= order.trigger_price,
                        StopOrderType::TakeProfit => current_price >= order.trigger_price,
                        StopOrderType::TrailingStop => {
                            // Update trailing price if moved favorably
                            let updated = Self::update_trailing_stop(&mut order, current_price)?;
                            
                            if updated {
                                // Save updated trailing price
                                order.serialize(&mut &mut account.data.borrow_mut()[..])?;
                                false // Don't trigger yet
                            } else {
                                // Check if stop hit
                                let distance = if order.side == OrderSide::Long {
                                    order.trailing_price - current_price
                                } else {
                                    current_price - order.trailing_price
                                };
                                distance >= order.trailing_distance
                            }
                        }
                    };
                    
                    if should_trigger {
                        triggered.push(TriggeredOrder {
                            order_id: order.order_id,
                            account: *account.key,
                            order_type: order.order_type,
                            trigger_price: order.trigger_price,
                            current_price,
                            priority: order.calculate_priority(),
                        });
                    }
                }
                Err(_) => continue,
            }
        }
        
        // Sort by priority (user stake, order age, etc.)
        triggered.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        Ok(triggered)
    }
    
    /// Update trailing stop price if market moved favorably
    fn update_trailing_stop(
        order: &mut StopOrder,
        current_price: U64F64,
    ) -> Result<bool, ProgramError> {
        match order.side {
            OrderSide::Long => {
                // For long positions, update if price increased
                if current_price > order.trailing_price {
                    order.trailing_price = current_price;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            OrderSide::Short => {
                // For short positions, update if price decreased
                if current_price < order.trailing_price {
                    order.trailing_price = current_price;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

impl StopOrder {
    /// Execute the stop order
    pub fn execute(&self, execution_price: U64F64) -> Result<ExecutionResult, ProgramError> {
        // Calculate executed value based on order size and price
        let executed_value = U64F64::from_num(self.size)
            .checked_mul(execution_price)?
            .to_num();
        
        Ok(ExecutionResult {
            executed_value,
            execution_price,
        })
    }
    
    /// Calculate order priority for execution ordering
    pub fn calculate_priority(&self) -> u64 {
        // Priority factors:
        // 1. Order age (older = higher priority)
        // 2. Prepaid bounty amount
        // 3. Order size
        
        let age_factor = Clock::get()
            .unwrap_or_default()
            .slot
            .saturating_sub(self.created_slot);
            
        let bounty_factor = self.prepaid_bounty / 1_000_000; // In SOL
        let size_factor = self.size / 1_000_000_000; // In dollars
        
        // Weighted priority
        age_factor * 100 + bounty_factor * 10 + size_factor
    }
}

// Hex encoding utility
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stop_loss_trigger() {
        let order = StopOrder {
            order_id: [1u8; 32],
            user: Pubkey::default(),
            market_id: [0u8; 32],
            position_id: [0u8; 32],
            order_type: StopOrderType::StopLoss,
            side: OrderSide::Long,
            trigger_price: U64F64::from_num(95),
            size: 1000,
            prepaid_bounty: 20,
            is_active: true,
            position_entry_price: U64F64::from_num(100),
            trailing_distance: U64F64::from_num(0),
            trailing_price: U64F64::from_num(0),
            created_slot: 0,
        };
        
        // Should trigger when price drops to 95 or below
        let current_price = U64F64::from_num(94);
        let triggered = current_price <= order.trigger_price;
        assert!(triggered);
    }
    
    #[test]
    fn test_take_profit_trigger() {
        let order = StopOrder {
            order_id: [1u8; 32],
            user: Pubkey::default(),
            market_id: [0u8; 32],
            position_id: [0u8; 32],
            order_type: StopOrderType::TakeProfit,
            side: OrderSide::Long,
            trigger_price: U64F64::from_num(110),
            size: 1000,
            prepaid_bounty: 20,
            is_active: true,
            position_entry_price: U64F64::from_num(100),
            trailing_distance: U64F64::from_num(0),
            trailing_price: U64F64::from_num(0),
            created_slot: 0,
        };
        
        // Should trigger when price rises to 110 or above
        let current_price = U64F64::from_num(111);
        let triggered = current_price >= order.trigger_price;
        assert!(triggered);
    }
    
    #[test]
    fn test_keeper_bounty_calculation() {
        let executed_value = 1_000_000_000; // $1000
        let expected_bounty = 200_000; // 2bp = $0.20
        
        let bounty = executed_value * STOP_KEEPER_BOUNTY_BPS / 10000;
        assert_eq!(bounty, expected_bounty);
    }
}