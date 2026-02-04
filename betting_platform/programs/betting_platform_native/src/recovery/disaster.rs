//! Disaster recovery implementation
//!
//! Handles system failures and emergency procedures

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
use crate::math::U64F64;
use crate::monitoring::health::{CircuitBreakerReason, SystemStatus};

/// Disaster recovery state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DisasterRecoveryState {
    pub current_mode: RecoveryMode,
    pub last_checkpoint_slot: u64,
    pub recovery_initiated_slot: Option<u64>,
    pub recovery_completed_slot: Option<u64>,
    
    // Recovery tracking
    pub positions_to_recover: u64,
    pub positions_recovered: u64,
    pub orders_to_recover: u64,
    pub orders_recovered: u64,
    
    // Polymarket sync
    pub polymarket_last_sync: u64,
    pub polymarket_out_of_sync: bool,
    pub polymarket_outage_start: Option<u64>,
    
    // Emergency actions taken
    pub emergency_actions: Vec<EmergencyAction>,
    
    // Recovery authority
    pub recovery_authority: Pubkey,
    pub emergency_contacts: Vec<Pubkey>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum RecoveryMode {
    Normal,              // System operating normally
    PartialDegradation,  // Some services degraded
    FullRecovery,        // Full recovery in progress
    Emergency,           // Emergency mode - minimal operations
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct EmergencyAction {
    pub action_type: EmergencyActionType,
    pub triggered_slot: u64,
    pub triggered_by: Pubkey,
    pub reason: String,
    pub affected_accounts: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum EmergencyActionType {
    HaltTrading,
    HaltLiquidations,
    PausePolymarketSync,
    ForceSettlement,
    EmergencyWithdrawals,
    RestoreFromCheckpoint,
}

/// Recovery manager
pub struct RecoveryManager;

impl RecoveryManager {
    /// Initiate disaster recovery
    pub fn initiate_recovery(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        reason: CircuitBreakerReason,
    ) -> ProgramResult {
        // Account layout:
        // 0. Recovery state account (mut)
        // 1. System health account (mut)
        // 2. Authority (signer)
        // 3. Clock sysvar
        
        if accounts.len() < 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let recovery_account = &accounts[0];
        let health_account = &accounts[1];
        let authority = &accounts[2];
        let clock = Clock::get()?;
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize states
        let mut recovery_data = recovery_account.try_borrow_mut_data()?;
        let mut recovery_state = DisasterRecoveryState::try_from_slice(&recovery_data)?;
        
        // Verify authority
        if *authority.key != recovery_state.recovery_authority {
            return Err(BettingPlatformError::UnauthorizedRecoveryAction.into());
        }
        
        // Determine recovery mode based on reason
        let recovery_mode = match reason {
            CircuitBreakerReason::PolymarketOutage => RecoveryMode::PartialDegradation,
            CircuitBreakerReason::SolanaOutage => RecoveryMode::Emergency,
            CircuitBreakerReason::LowCoverage => RecoveryMode::FullRecovery,
            _ => RecoveryMode::PartialDegradation,
        };
        
        recovery_state.current_mode = recovery_mode;
        recovery_state.recovery_initiated_slot = Some(clock.slot);
        
        // Log emergency action
        let action = EmergencyAction {
            action_type: EmergencyActionType::HaltTrading,
            triggered_slot: clock.slot,
            triggered_by: *authority.key,
            reason: format!("{:?}", reason),
            affected_accounts: 0, // Will be updated
        };
        
        recovery_state.emergency_actions.push(action);
        
        msg!(
            "DisasterRecoveryInitiated - mode: {:?}, reason: {:?}, slot: {}",
            recovery_mode,
            reason,
            clock.slot
        );
        
        // Serialize updated state
        recovery_state.serialize(&mut *recovery_data)?;
        
        Ok(())
    }
    
    /// Handle Polymarket outage
    pub fn handle_polymarket_outage(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        // Account layout:
        // 0. Recovery state account (mut)
        // 1. System health account (mut)
        // 2. Keeper (signer)
        // 3. Clock sysvar
        
        if accounts.len() < 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let recovery_account = &accounts[0];
        let _health_account = &accounts[1];
        let keeper = &accounts[2];
        let clock = Clock::get()?;
        
        if !keeper.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize recovery state
        let mut recovery_data = recovery_account.try_borrow_mut_data()?;
        let mut recovery_state = DisasterRecoveryState::try_from_slice(&recovery_data)?;
        
        // Check if outage is already being handled
        if recovery_state.polymarket_outage_start.is_some() {
            let duration = clock.slot - recovery_state.polymarket_outage_start.unwrap();
            
            // CLAUDE.md: If outage > 5 minutes (750 slots @ 400ms), halt new orders
            const POLYMARKET_OUTAGE_THRESHOLD: u64 = 750; // 5 minutes
            
            if duration > POLYMARKET_OUTAGE_THRESHOLD {
                msg!("CRITICAL: Polymarket outage exceeds 5 minutes ({} slots), halting ALL new orders", duration);
                
                // First action: Pause Polymarket sync
                let pause_action = EmergencyAction {
                    action_type: EmergencyActionType::PausePolymarketSync,
                    triggered_slot: clock.slot,
                    triggered_by: *keeper.key,
                    reason: format!("Polymarket outage > 5 min ({}s)", duration * 400 / 1000),
                    affected_accounts: 0,
                };
                recovery_state.emergency_actions.push(pause_action);
                
                // Second action: Halt all trading
                let halt_action = EmergencyAction {
                    action_type: EmergencyActionType::HaltTrading,
                    triggered_slot: clock.slot,
                    triggered_by: *keeper.key,
                    reason: "CLAUDE.md: Halt new orders after 5 min Polymarket outage".to_string(),
                    affected_accounts: 0,
                };
                recovery_state.emergency_actions.push(halt_action);
                
                recovery_state.current_mode = RecoveryMode::PartialDegradation;
                
                msg!(
                    "PolymarketOutageHaltTriggered - duration_slots: {}, duration_seconds: {}, action: halt_new_orders",
                    duration,
                    duration * 400 / 1000
                );
            }
        } else {
            // First detection of outage
            recovery_state.polymarket_outage_start = Some(clock.slot);
            recovery_state.polymarket_out_of_sync = true;
            
            msg!(
                "PolymarketOutageDetected - slot: {}", 
                clock.slot
            );
        }
        
        // Serialize updated state
        recovery_state.serialize(&mut *recovery_data)?;
        
        Ok(())
    }
    
    /// Restore from checkpoint
    pub fn restore_from_checkpoint(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        checkpoint_slot: u64,
    ) -> ProgramResult {
        // Account layout:
        // 0. Recovery state account (mut)
        // 1. Checkpoint account
        // 2. Authority (signer)
        // 3. Clock sysvar
        
        if accounts.len() < 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let recovery_account = &accounts[0];
        let checkpoint_account = &accounts[1];
        let authority = &accounts[2];
        let clock = Clock::get()?;
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize states
        let mut recovery_data = recovery_account.try_borrow_mut_data()?;
        let mut recovery_state = DisasterRecoveryState::try_from_slice(&recovery_data)?;
        
        // Verify authority
        if *authority.key != recovery_state.recovery_authority {
            return Err(BettingPlatformError::UnauthorizedRecoveryAction.into());
        }
        
        // Verify checkpoint exists and is valid
        if checkpoint_slot > recovery_state.last_checkpoint_slot {
            return Err(BettingPlatformError::InvalidCheckpoint.into());
        }
        
        msg!("Initiating restore from checkpoint at slot {}", checkpoint_slot);
        
        recovery_state.current_mode = RecoveryMode::FullRecovery;
        recovery_state.recovery_initiated_slot = Some(clock.slot);
        
        let action = EmergencyAction {
            action_type: EmergencyActionType::RestoreFromCheckpoint,
            triggered_slot: clock.slot,
            triggered_by: *authority.key,
            reason: format!("Restore from checkpoint {}", checkpoint_slot),
            affected_accounts: recovery_state.positions_to_recover + recovery_state.orders_to_recover,
        };
        
        recovery_state.emergency_actions.push(action);
        
        msg!(
            "CheckpointRestoreInitiated - checkpoint_slot: {}, positions: {}, orders: {}",
            checkpoint_slot,
            recovery_state.positions_to_recover,
            recovery_state.orders_to_recover
        );
        
        // Serialize updated state
        recovery_state.serialize(&mut *recovery_data)?;
        
        Ok(())
    }
    
    /// Complete recovery process
    pub fn complete_recovery(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        // Account layout:
        // 0. Recovery state account (mut)
        // 1. System health account (mut)
        // 2. Authority (signer)
        // 3. Clock sysvar
        
        if accounts.len() < 4 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let recovery_account = &accounts[0];
        let _health_account = &accounts[1];
        let authority = &accounts[2];
        let clock = Clock::get()?;
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize recovery state
        let mut recovery_data = recovery_account.try_borrow_mut_data()?;
        let mut recovery_state = DisasterRecoveryState::try_from_slice(&recovery_data)?;
        
        // Verify all items recovered
        if recovery_state.positions_recovered < recovery_state.positions_to_recover ||
           recovery_state.orders_recovered < recovery_state.orders_to_recover {
            return Err(BettingPlatformError::RecoveryIncomplete.into());
        }
        
        // Mark recovery complete
        recovery_state.current_mode = RecoveryMode::Normal;
        recovery_state.recovery_completed_slot = Some(clock.slot);
        recovery_state.polymarket_out_of_sync = false;
        recovery_state.polymarket_outage_start = None;
        
        let duration = clock.slot - recovery_state.recovery_initiated_slot.unwrap_or(clock.slot);
        
        msg!(
            "RecoveryCompleted - duration_slots: {}, positions_recovered: {}, orders_recovered: {}",
            duration,
            recovery_state.positions_recovered,
            recovery_state.orders_recovered
        );
        
        // Reset counters
        recovery_state.positions_to_recover = 0;
        recovery_state.positions_recovered = 0;
        recovery_state.orders_to_recover = 0;
        recovery_state.orders_recovered = 0;
        
        // Serialize updated state
        recovery_state.serialize(&mut *recovery_data)?;
        
        Ok(())
    }
    
    /// Check if operation is allowed during recovery
    pub fn check_operation_allowed(
        recovery_state: &DisasterRecoveryState,
        operation: &str,
    ) -> bool {
        // Check for active halt trading action
        let trading_halted = recovery_state.emergency_actions.iter()
            .any(|action| action.action_type == EmergencyActionType::HaltTrading);
        
        if trading_halted && matches!(operation, "open_position" | "place_order" | "increase_position") {
            msg!("Operation {} blocked: Trading halted due to emergency action", operation);
            return false;
        }
        
        match recovery_state.current_mode {
            RecoveryMode::Normal => true,
            RecoveryMode::PartialDegradation => {
                // Allow reads and closes, block new positions
                matches!(operation, "read" | "close_position" | "withdraw")
            }
            RecoveryMode::FullRecovery => {
                // Only allow emergency operations
                matches!(operation, "emergency_withdraw" | "read")
            }
            RecoveryMode::Emergency => {
                // Block all operations except emergency
                matches!(operation, "emergency_withdraw")
            }
        }
    }
    
    /// Check Polymarket health status (CLAUDE.md: 5 min outage detection)
    pub fn check_polymarket_health(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        last_successful_sync: u64,
    ) -> ProgramResult {
        // Account layout:
        // 0. Recovery state account (mut)
        // 1. Alert configuration account (mut)
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let recovery_account = &accounts[0];
        let _alert_account = &accounts[1];
        let clock = Clock::get()?;
        
        let mut recovery_data = recovery_account.try_borrow_mut_data()?;
        let mut recovery_state = DisasterRecoveryState::try_from_slice(&recovery_data)?;
        
        // Calculate time since last sync
        let slots_since_sync = clock.slot.saturating_sub(last_successful_sync);
        
        // CLAUDE.md: 750 slots = 5 minutes @ 400ms per slot
        const POLYMARKET_SYNC_WARNING: u64 = 450;  // 3 minutes warning
        const POLYMARKET_SYNC_CRITICAL: u64 = 750; // 5 minutes critical
        
        if slots_since_sync > POLYMARKET_SYNC_CRITICAL {
            // Trigger outage handling if not already handled
            if recovery_state.polymarket_outage_start.is_none() {
                recovery_state.polymarket_outage_start = Some(clock.slot.saturating_sub(slots_since_sync));
                recovery_state.polymarket_out_of_sync = true;
                msg!("Polymarket sync critical: {} slots since last sync", slots_since_sync);
            }
        } else if slots_since_sync > POLYMARKET_SYNC_WARNING {
            msg!("Polymarket sync warning: {} slots since last sync", slots_since_sync);
        } else {
            // Clear outage if resolved
            if recovery_state.polymarket_outage_start.is_some() {
                recovery_state.polymarket_outage_start = None;
                recovery_state.polymarket_out_of_sync = false;
                
                msg!(
                    "PolymarketOutageResolved - sync_restored_at_slot: {}", 
                    clock.slot
                );
            }
        }
        
        recovery_state.polymarket_last_sync = last_successful_sync;
        recovery_state.serialize(&mut *recovery_data)?;
        
        Ok(())
    }
}

// Recovery state helpers
impl DisasterRecoveryState {
    pub const SIZE: usize = 1 + // current_mode
        8 + // last_checkpoint_slot
        1 + 8 + // recovery_initiated_slot Option
        1 + 8 + // recovery_completed_slot Option
        8 + // positions_to_recover
        8 + // positions_recovered
        8 + // orders_to_recover
        8 + // orders_recovered
        8 + // polymarket_last_sync
        1 + // polymarket_out_of_sync
        1 + 8 + // polymarket_outage_start Option
        4 + (10 * EmergencyAction::SIZE) + // emergency_actions Vec (max 10)
        32 + // recovery_authority
        4 + (5 * 32); // emergency_contacts Vec<Pubkey> (max 5)
    
    /// Get recovery progress percentage
    pub fn get_recovery_progress(&self) -> u8 {
        let total = self.positions_to_recover + self.orders_to_recover;
        if total == 0 {
            return 100;
        }
        
        let recovered = self.positions_recovered + self.orders_recovered;
        ((recovered * 100) / total) as u8
    }
    
    /// Check if recovery is needed
    pub fn needs_recovery(&self) -> bool {
        self.current_mode != RecoveryMode::Normal ||
        self.polymarket_out_of_sync ||
        self.positions_to_recover > self.positions_recovered ||
        self.orders_to_recover > self.orders_recovered
    }
}

impl EmergencyAction {
    pub const SIZE: usize = 1 + // action_type
        8 + // triggered_slot
        32 + // triggered_by
        4 + 128 + // reason String (max 128 chars)
        8; // affected_accounts
}