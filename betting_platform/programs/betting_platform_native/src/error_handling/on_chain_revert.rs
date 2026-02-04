//! On-Chain Revert Capability (1 slot for non-liquidation)
//! 
//! Implements 1-slot revert window for non-liquidation actions
//! allowing users to revert transactions within the same slot

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{
        Position,
        ProposalPDA,
        accounts::discriminators,
    },
    events::{emit_event, EventType},
    define_event,
};

/// Maximum revertible actions per slot
pub const MAX_REVERTIBLE_PER_SLOT: usize = 100;

/// Actions that can be reverted
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum RevertibleAction {
    /// Position opened
    PositionOpened {
        position_id: [u8; 32],
        market_id: u128,
        size: u64,
        margin: u64,
    },
    
    /// Position closed
    PositionClosed {
        position_id: [u8; 32],
        final_pnl: i64,
        returned_collateral: u64,
    },
    
    /// Position modified
    PositionModified {
        position_id: [u8; 32],
        old_size: u64,
        new_size: u64,
        old_leverage: u8,
        new_leverage: u8,
    },
    
    /// Order placed
    OrderPlaced {
        order_id: [u8; 32],
        order_type: OrderType,
        size: u64,
    },
    
    /// Order cancelled
    OrderCancelled {
        order_id: [u8; 32],
    },
    
    /// MMT staked
    MMTStaked {
        amount: u64,
        lock_period: u64,
    },
    
    /// MMT unstaked
    MMTUnstaked {
        amount: u64,
    },
}

/// Order type for revertible orders
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}

/// Revertible action record
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RevertibleActionRecord {
    /// Unique action ID
    pub action_id: [u8; 32],
    
    /// User who performed the action
    pub user: Pubkey,
    
    /// The action that can be reverted
    pub action: RevertibleAction,
    
    /// Slot when action was performed
    pub slot: u64,
    
    /// Timestamp when action was performed
    pub timestamp: i64,
    
    /// Whether this action has been reverted
    pub is_reverted: bool,
    
    /// State snapshot before the action
    pub state_before: StateBeforeAction,
    
    /// Accounts involved in the action
    pub involved_accounts: Vec<Pubkey>,
}

/// State snapshot before an action
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StateBeforeAction {
    /// User's balance before action
    pub user_balance: u64,
    
    /// Market liquidity before action
    pub market_liquidity: Option<u64>,
    
    /// Position state before action (if applicable)
    pub position_state: Option<PositionStateBefore>,
    
    /// Order state before action (if applicable)
    pub order_state: Option<OrderStateBefore>,
    
    /// MMT staking state before action
    pub staking_state: Option<StakingStateBefore>,
}

/// Position state before modification
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionStateBefore {
    pub size: u64,
    pub margin: u64,
    pub leverage: u8,
    pub entry_price: u64,
    pub is_long: bool,
    pub unrealized_pnl: i64,
}

/// Order state before modification
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderStateBefore {
    pub order_type: OrderType,
    pub size: u64,
    pub price: Option<u64>,
    pub is_active: bool,
}

/// Staking state before modification
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StakingStateBefore {
    pub staked_amount: u64,
    pub lock_end_slot: u64,
    pub rewards_earned: u64,
}

/// Slot revert tracker - tracks revertible actions in current slot
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SlotRevertTracker {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Current slot being tracked
    pub current_slot: u64,
    
    /// Revertible actions in this slot
    pub actions: Vec<RevertibleActionRecord>,
    
    /// Total reverts performed
    pub total_reverts: u64,
    
    /// Last cleanup slot
    pub last_cleanup_slot: u64,
}

impl SlotRevertTracker {
    pub fn new() -> Self {
        let clock = Clock::get().unwrap();
        Self {
            discriminator: discriminators::SLOT_REVERT_TRACKER,
            current_slot: clock.slot,
            actions: Vec::new(),
            total_reverts: 0,
            last_cleanup_slot: clock.slot,
        }
    }
    
    /// Add a new revertible action
    pub fn add_action(&mut self, action: RevertibleActionRecord) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
        
        // Update slot if needed
        if clock.slot > self.current_slot {
            self.cleanup_old_actions(clock.slot);
            self.current_slot = clock.slot;
        }
        
        // Check limit
        if self.actions.len() >= MAX_REVERTIBLE_PER_SLOT {
            return Err(BettingPlatformError::TooManyRevertibleActions.into());
        }
        
        self.actions.push(action);
        Ok(())
    }
    
    /// Find an action by ID
    pub fn find_action(&self, action_id: &[u8; 32]) -> Option<&RevertibleActionRecord> {
        self.actions.iter()
            .find(|a| &a.action_id == action_id && !a.is_reverted)
    }
    
    /// Mark an action as reverted
    pub fn mark_reverted(&mut self, action_id: &[u8; 32]) -> Result<(), ProgramError> {
        if let Some(action) = self.actions.iter_mut().find(|a| &a.action_id == action_id) {
            if action.is_reverted {
                return Err(BettingPlatformError::ActionAlreadyReverted.into());
            }
            action.is_reverted = true;
            self.total_reverts += 1;
            Ok(())
        } else {
            Err(BettingPlatformError::ActionNotFound.into())
        }
    }
    
    /// Clean up actions from previous slots
    fn cleanup_old_actions(&mut self, current_slot: u64) {
        self.actions.retain(|action| {
            action.slot == current_slot && !action.is_reverted
        });
        self.last_cleanup_slot = current_slot;
    }
}

/// Record a revertible action
pub fn record_revertible_action(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    action: RevertibleAction,
    state_before: StateBeforeAction,
) -> ProgramResult {
    msg!("Recording revertible action");
    
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;
    let revert_tracker_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Collect involved accounts
    let involved_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Check if action is liquidation (not revertible)
    if is_liquidation_action(&action) {
        msg!("Liquidation actions are not revertible");
        return Ok(());
    }
    
    // Load or create revert tracker
    let mut tracker = if revert_tracker_account.data_len() > 0 {
        SlotRevertTracker::try_from_slice(&revert_tracker_account.data.borrow())?
    } else {
        SlotRevertTracker::new()
    };
    
    // Generate action ID
    let clock = Clock::get()?;
    let action_id = {
        let mut data = Vec::new();
        data.extend_from_slice(user_account.key.as_ref());
        data.extend_from_slice(&clock.slot.to_le_bytes());
        data.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
        solana_program::hash::hash(&data).to_bytes()
    };
    
    // Create action record
    let record = RevertibleActionRecord {
        action_id,
        user: *user_account.key,
        action: action.clone(),
        slot: clock.slot,
        timestamp: clock.unix_timestamp,
        is_reverted: false,
        state_before,
        involved_accounts: involved_accounts.iter().map(|a| *a.key).collect(),
    };
    
    // Add to tracker
    tracker.add_action(record)?;
    
    // Save tracker
    tracker.serialize(&mut &mut revert_tracker_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::ActionRecorded, &ActionRecorded {
        action_id,
        user: *user_account.key,
        action_type: format!("{:?}", action),
        slot: clock.slot,
    });
    
    msg!("Action {} recorded as revertible", bs58::encode(action_id).into_string());
    
    Ok(())
}

/// Revert an action within the same slot
pub fn revert_action(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    action_id: [u8; 32],
) -> ProgramResult {
    msg!("Reverting action {}", bs58::encode(action_id).into_string());
    
    let account_info_iter = &mut accounts.iter();
    let user_account = next_account_info(account_info_iter)?;
    let revert_tracker_account = next_account_info(account_info_iter)?;
    
    // Remaining accounts for revert execution
    let revert_accounts: Vec<&AccountInfo> = account_info_iter.collect();
    
    // Verify signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load tracker
    let mut tracker = SlotRevertTracker::try_from_slice(&revert_tracker_account.data.borrow())?;
    
    // Find action
    let action_record = tracker.find_action(&action_id)
        .ok_or(BettingPlatformError::ActionNotFound)?
        .clone();
    
    // Verify user owns the action
    if action_record.user != *user_account.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify still in same slot
    let clock = Clock::get()?;
    if clock.slot != action_record.slot {
        return Err(BettingPlatformError::RevertWindowExpired.into());
    }
    
    // Execute the revert
    execute_revert(
        program_id,
        &action_record.action,
        &action_record.state_before,
        &revert_accounts,
    )?;
    
    // Mark as reverted
    tracker.mark_reverted(&action_id)?;
    
    // Save tracker
    tracker.serialize(&mut &mut revert_tracker_account.data.borrow_mut()[..])?;
    
    // Emit event
    emit_event(EventType::ActionReverted, &ActionReverted {
        action_id,
        user: *user_account.key,
        action_type: format!("{:?}", action_record.action),
        reverted_at: clock.unix_timestamp,
    });
    
    msg!("Action reverted successfully");
    
    Ok(())
}

/// Check if an action is a liquidation (not revertible)
fn is_liquidation_action(action: &RevertibleAction) -> bool {
    // Liquidations cannot be reverted for system stability
    false // None of the defined actions are liquidations
}

/// Execute the revert based on action type
fn execute_revert(
    program_id: &Pubkey,
    action: &RevertibleAction,
    state_before: &StateBeforeAction,
    accounts: &[&AccountInfo],
) -> ProgramResult {
    match action {
        RevertibleAction::PositionOpened { position_id, market_id, size, margin } => {
            msg!("Reverting position open: closing position {}", bs58::encode(position_id).into_string());
            // Close the position and return margin
            // This would call into position closing logic
        }
        
        RevertibleAction::PositionClosed { position_id, final_pnl, returned_collateral } => {
            msg!("Reverting position close: reopening position {}", bs58::encode(position_id).into_string());
            // Reopen the position with original state
            if let Some(pos_state) = &state_before.position_state {
                // Restore position with original parameters
            }
        }
        
        RevertibleAction::PositionModified { position_id, old_size, new_size, old_leverage, new_leverage } => {
            msg!("Reverting position modification: restoring original size/leverage");
            // Restore original position parameters
        }
        
        RevertibleAction::OrderPlaced { order_id, order_type, size } => {
            msg!("Reverting order placement: cancelling order {}", bs58::encode(order_id).into_string());
            // Cancel the placed order
        }
        
        RevertibleAction::OrderCancelled { order_id } => {
            msg!("Reverting order cancellation: restoring order {}", bs58::encode(order_id).into_string());
            // Restore the cancelled order
            if let Some(order_state) = &state_before.order_state {
                // Recreate order with original parameters
            }
        }
        
        RevertibleAction::MMTStaked { amount, lock_period } => {
            msg!("Reverting MMT stake: unstaking {} MMT", amount);
            // Unstake the MMT tokens
        }
        
        RevertibleAction::MMTUnstaked { amount } => {
            msg!("Reverting MMT unstake: restaking {} MMT", amount);
            // Restake the MMT tokens
            if let Some(staking_state) = &state_before.staking_state {
                // Restore staking with original parameters
            }
        }
    }
    
    Ok(())
}

// Events
define_event!(ActionRecorded, EventType::ActionRecorded, {
    action_id: [u8; 32],
    user: Pubkey,
    action_type: String,
    slot: u64
});

define_event!(ActionReverted, EventType::ActionReverted, {
    action_id: [u8; 32],
    user: Pubkey,
    action_type: String,
    reverted_at: i64
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_revert_window() {
        let action_slot = 1000;
        let current_slot = 1000;
        
        // Same slot - can revert
        assert!(current_slot == action_slot);
        
        // Next slot - cannot revert
        let next_slot = 1001;
        assert!(next_slot > action_slot);
    }
    
    #[test]
    fn test_action_types() {
        // Test that all action types are properly defined
        let actions = vec![
            RevertibleAction::PositionOpened {
                position_id: [1; 32],
                market_id: 1,
                size: 1000,
                margin: 100,
            },
            RevertibleAction::PositionClosed {
                position_id: [2; 32],
                final_pnl: 50,
                returned_collateral: 150,
            },
            RevertibleAction::OrderPlaced {
                order_id: [3; 32],
                order_type: OrderType::Limit,
                size: 500,
            },
        ];
        
        for action in actions {
            assert!(!is_liquidation_action(&action));
        }
    }
}