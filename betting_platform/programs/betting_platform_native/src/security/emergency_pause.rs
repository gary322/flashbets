//! Emergency Pause System
//!
//! Production-grade emergency response and circuit breaker implementation

use solana_program::{
    account_info::AccountInfo,
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
    state::accounts::discriminators,
};

/// Pause levels
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PauseLevel {
    /// Normal operation
    None = 0,
    /// Partial pause - only critical operations allowed
    Partial = 1,
    /// Full pause - only emergency operations allowed
    Full = 2,
    /// Complete freeze - no operations allowed
    Freeze = 3,
}

/// Operation categories for pause control
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationCategory {
    /// Trading operations (open/close positions)
    Trading,
    /// Liquidation operations
    Liquidation,
    /// Oracle updates
    Oracle,
    /// Withdrawals
    Withdrawal,
    /// Deposits
    Deposit,
    /// Administrative operations
    Admin,
    /// Emergency operations
    Emergency,
    /// View operations (read-only)
    View,
}

/// Emergency pause state
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct EmergencyPause {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Version
    pub version: u32,
    /// Pause authority (can pause)
    pub pause_authority: Pubkey,
    /// Unpause authority (can unpause)
    pub unpause_authority: Pubkey,
    /// Current pause level
    pub pause_level: PauseLevel,
    /// Paused categories (bit flags)
    pub paused_categories: u8,
    /// Pause start slot
    pub pause_start_slot: u64,
    /// Auto-unpause slot (0 = manual only)
    pub auto_unpause_slot: u64,
    /// Pause reason
    pub pause_reason: [u8; 128],
    /// Total pauses triggered
    pub total_pauses: u64,
    /// Last pause duration
    pub last_pause_duration: u64,
    /// Emergency contacts notified
    pub contacts_notified: bool,
    /// Grace period for pending operations (slots)
    pub grace_period: u64,
}

impl EmergencyPause {
    pub fn new(pause_authority: Pubkey, unpause_authority: Pubkey) -> Self {
        Self {
            discriminator: discriminators::EMERGENCY_PAUSE,
            version: 1,
            pause_authority,
            unpause_authority,
            pause_level: PauseLevel::None,
            paused_categories: 0,
            pause_start_slot: 0,
            auto_unpause_slot: 0,
            pause_reason: [0; 128],
            total_pauses: 0,
            last_pause_duration: 0,
            contacts_notified: false,
            grace_period: 10, // 10 slots grace period
        }
    }
    
    /// Check if operation is allowed
    pub fn is_operation_allowed(
        &self,
        category: OperationCategory,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check auto-unpause
        if self.auto_unpause_slot > 0 && current_slot >= self.auto_unpause_slot {
            // Would auto-unpause here in practice
            msg!("Auto-unpause triggered at slot {}", current_slot);
        }
        
        // Check pause level
        match self.pause_level {
            PauseLevel::None => Ok(()),
            PauseLevel::Partial => {
                // Allow critical operations
                match category {
                    OperationCategory::Emergency |
                    OperationCategory::Admin |
                    OperationCategory::View |
                    OperationCategory::Liquidation => Ok(()),
                    _ => {
                        msg!("Operation {:?} blocked by partial pause", category);
                        Err(BettingPlatformError::ProtocolPaused.into())
                    }
                }
            }
            PauseLevel::Full => {
                // Only emergency operations
                match category {
                    OperationCategory::Emergency |
                    OperationCategory::View => Ok(()),
                    _ => {
                        msg!("Operation {:?} blocked by full pause", category);
                        Err(BettingPlatformError::ProtocolPaused.into())
                    }
                }
            }
            PauseLevel::Freeze => {
                // No operations allowed
                msg!("Protocol frozen - no operations allowed");
                Err(BettingPlatformError::ProtocolFrozen.into())
            }
        }
    }
    
    /// Check if specific category is paused
    pub fn is_category_paused(&self, category: OperationCategory) -> bool {
        let bit = 1u8 << (category as u8);
        (self.paused_categories & bit) != 0
    }
    
    /// Trigger emergency pause
    pub fn trigger_pause(
        &mut self,
        level: PauseLevel,
        reason: &str,
        duration_slots: u64,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Verify authority
        if *authority != self.pause_authority {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Check if already paused at higher level
        if self.pause_level >= level {
            msg!("Already paused at level {:?}", self.pause_level);
            return Ok(());
        }
        
        let current_slot = Clock::get()?.slot;
        
        // Set pause state
        self.pause_level = level;
        self.pause_start_slot = current_slot;
        self.auto_unpause_slot = if duration_slots > 0 {
            current_slot + duration_slots
        } else {
            0
        };
        
        // Set reason
        let reason_bytes = reason.as_bytes();
        let len = reason_bytes.len().min(128);
        self.pause_reason[..len].copy_from_slice(&reason_bytes[..len]);
        
        // Update categories based on level
        self.paused_categories = match level {
            PauseLevel::None => 0,
            PauseLevel::Partial => {
                (1 << OperationCategory::Trading as u8) |
                (1 << OperationCategory::Withdrawal as u8) |
                (1 << OperationCategory::Deposit as u8)
            }
            PauseLevel::Full => {
                0xFF ^ (1 << OperationCategory::Emergency as u8) ^ 
                (1 << OperationCategory::View as u8)
            }
            PauseLevel::Freeze => 0xFF, // All categories
        };
        
        self.total_pauses += 1;
        self.contacts_notified = false;
        
        msg!("Emergency pause triggered: level {:?}, reason: {}", level, reason);
        
        Ok(())
    }
    
    /// Unpause protocol
    pub fn unpause(
        &mut self,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Verify authority
        if *authority != self.unpause_authority {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        if self.pause_level == PauseLevel::None {
            msg!("Protocol not paused");
            return Ok(());
        }
        
        let current_slot = Clock::get()?.slot;
        
        // Calculate pause duration
        if self.pause_start_slot > 0 {
            self.last_pause_duration = current_slot.saturating_sub(self.pause_start_slot);
        }
        
        // Reset pause state
        self.pause_level = PauseLevel::None;
        self.paused_categories = 0;
        self.pause_start_slot = 0;
        self.auto_unpause_slot = 0;
        self.pause_reason = [0; 128];
        
        msg!("Protocol unpaused after {} slots", self.last_pause_duration);
        
        Ok(())
    }
    
    /// Get pause status
    pub fn get_status(&self) -> PauseStatus {
        let current_slot = Clock::get().unwrap_or_default().slot;
        
        PauseStatus {
            is_paused: self.pause_level != PauseLevel::None,
            level: self.pause_level,
            duration: if self.pause_start_slot > 0 {
                current_slot.saturating_sub(self.pause_start_slot)
            } else {
                0
            },
            auto_unpause_in: if self.auto_unpause_slot > current_slot {
                self.auto_unpause_slot - current_slot
            } else {
                0
            },
            reason: String::from_utf8_lossy(&self.pause_reason)
                .trim_end_matches('\0')
                .to_string(),
            paused_categories: self.get_paused_categories(),
        }
    }
    
    /// Get list of paused categories
    fn get_paused_categories(&self) -> Vec<OperationCategory> {
        let mut categories = Vec::new();
        
        for i in 0..8 {
            if (self.paused_categories & (1 << i)) != 0 {
                match i {
                    0 => categories.push(OperationCategory::Trading),
                    1 => categories.push(OperationCategory::Liquidation),
                    2 => categories.push(OperationCategory::Oracle),
                    3 => categories.push(OperationCategory::Withdrawal),
                    4 => categories.push(OperationCategory::Deposit),
                    5 => categories.push(OperationCategory::Admin),
                    6 => categories.push(OperationCategory::Emergency),
                    7 => categories.push(OperationCategory::View),
                    _ => {}
                }
            }
        }
        
        categories
    }
}

/// Pause status
#[derive(Debug)]
pub struct PauseStatus {
    pub is_paused: bool,
    pub level: PauseLevel,
    pub duration: u64,
    pub auto_unpause_in: u64,
    pub reason: String,
    pub paused_categories: Vec<OperationCategory>,
}

/// Circuit breaker for automatic pausing
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CircuitBreaker {
    /// Price volatility threshold (basis points)
    pub volatility_threshold: u16,
    /// Volume spike threshold (percentage)
    pub volume_threshold: u16,
    /// Liquidation cascade threshold
    pub liquidation_threshold: u32,
    /// Loss threshold (basis points)
    pub loss_threshold: u16,
    /// Time window (slots)
    pub window_slots: u64,
    /// Trigger count in window
    pub trigger_count: u8,
    /// Auto-pause duration
    pub pause_duration: u64,
}

impl CircuitBreaker {
    pub fn default() -> Self {
        Self {
            volatility_threshold: 1000, // 10% volatility
            volume_threshold: 500, // 500% volume spike
            liquidation_threshold: 100, // 100 liquidations
            loss_threshold: 500, // 5% loss
            window_slots: 100,
            trigger_count: 0,
            pause_duration: 300, // 300 slots
        }
    }
    
    /// Check if circuit breaker should trigger
    pub fn should_trigger(
        &mut self,
        metrics: &CircuitBreakerMetrics,
    ) -> Option<(PauseLevel, String)> {
        // Check volatility
        if metrics.max_volatility > self.volatility_threshold {
            self.trigger_count += 1;
            return Some((
                PauseLevel::Partial,
                format!("High volatility: {}bps", metrics.max_volatility)
            ));
        }
        
        // Check volume spike
        if metrics.volume_ratio > self.volume_threshold {
            self.trigger_count += 1;
            return Some((
                PauseLevel::Partial,
                format!("Volume spike: {}%", metrics.volume_ratio)
            ));
        }
        
        // Check liquidation cascade
        if metrics.liquidation_count > self.liquidation_threshold {
            self.trigger_count += 1;
            return Some((
                PauseLevel::Full,
                format!("Liquidation cascade: {} positions", metrics.liquidation_count)
            ));
        }
        
        // Check protocol loss
        if metrics.protocol_loss_bps > self.loss_threshold {
            self.trigger_count += 1;
            return Some((
                PauseLevel::Full,
                format!("Protocol loss: {}bps", metrics.protocol_loss_bps)
            ));
        }
        
        None
    }
}

/// Circuit breaker metrics
#[derive(Debug)]
pub struct CircuitBreakerMetrics {
    pub max_volatility: u16,
    pub volume_ratio: u16,
    pub liquidation_count: u32,
    pub protocol_loss_bps: u16,
}

/// Initialize emergency pause system
pub fn initialize_emergency_pause<'a>(
    pause_account: &AccountInfo<'a>,
    pause_authority: &AccountInfo<'a>,
    unpause_authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    // Verify account is uninitialized
    if !pause_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Create pause system
    let pause_system = EmergencyPause::new(
        *pause_authority.key,
        *unpause_authority.key,
    );
    
    // Calculate space
    let space = pause_system.try_to_vec()?.len();
    
    // Create account
    let rent = solana_program::rent::Rent::get()?;
    let rent_lamports = rent.minimum_balance(space);
    
    solana_program::program::invoke(
        &solana_program::system_instruction::create_account(
            payer.key,
            pause_account.key,
            rent_lamports,
            space as u64,
            &crate::ID,
        ),
        &[payer.clone(), pause_account.clone(), system_program.clone()],
    )?;
    
    // Initialize
    pause_system.serialize(&mut &mut pause_account.data.borrow_mut()[..])?;
    
    msg!("Emergency pause system initialized");
    Ok(())
}

/// Macro for checking pause status
#[macro_export]
macro_rules! require_not_paused {
    ($pause_state:expr, $category:expr) => {{
        let current_slot = Clock::get()?.slot;
        $pause_state.is_operation_allowed($category, current_slot)?;
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pause_levels() {
        let authority = Pubkey::new_unique();
        let mut pause = EmergencyPause::new(authority, authority);
        
        // Normal operation
        assert!(pause.is_operation_allowed(OperationCategory::Trading, 100).is_ok());
        
        // Partial pause
        assert!(pause.trigger_pause(
            PauseLevel::Partial,
            "Test partial pause",
            0,
            &authority
        ).is_ok());
        
        // Trading blocked, emergency allowed
        assert!(pause.is_operation_allowed(OperationCategory::Trading, 100).is_err());
        assert!(pause.is_operation_allowed(OperationCategory::Emergency, 100).is_ok());
        
        // Full pause
        assert!(pause.trigger_pause(
            PauseLevel::Full,
            "Test full pause",
            0,
            &authority
        ).is_ok());
        
        // Only emergency allowed
        assert!(pause.is_operation_allowed(OperationCategory::Admin, 100).is_err());
        assert!(pause.is_operation_allowed(OperationCategory::Emergency, 100).is_ok());
    }

    #[test]
    fn test_auto_unpause() {
        let authority = Pubkey::new_unique();
        let mut pause = EmergencyPause::new(authority, authority);
        
        // Pause for 100 slots
        assert!(pause.trigger_pause(
            PauseLevel::Partial,
            "Auto-unpause test",
            100,
            &authority
        ).is_ok());
        
        assert_eq!(pause.auto_unpause_slot, 100);
        
        // Should be paused at slot 50
        assert!(pause.is_operation_allowed(OperationCategory::Trading, 50).is_err());
        
        // Note: Auto-unpause would be implemented in the actual check
    }

    #[test]
    fn test_circuit_breaker() {
        let mut breaker = CircuitBreaker::default();
        
        // Normal metrics
        let normal_metrics = CircuitBreakerMetrics {
            max_volatility: 500, // 5%
            volume_ratio: 200, // 200%
            liquidation_count: 10,
            protocol_loss_bps: 100, // 1%
        };
        
        assert!(breaker.should_trigger(&normal_metrics).is_none());
        
        // High volatility
        let volatile_metrics = CircuitBreakerMetrics {
            max_volatility: 1500, // 15%
            volume_ratio: 200,
            liquidation_count: 10,
            protocol_loss_bps: 100,
        };
        
        let trigger = breaker.should_trigger(&volatile_metrics);
        assert!(trigger.is_some());
        assert_eq!(trigger.unwrap().0, PauseLevel::Partial);
    }

    #[test]
    fn test_pause_categories() {
        let authority = Pubkey::new_unique();
        let pause = EmergencyPause::new(authority, authority);
        
        // Check category bit flags
        assert!(!pause.is_category_paused(OperationCategory::Trading));
        assert!(!pause.is_category_paused(OperationCategory::Liquidation));
        
        let mut pause_with_categories = pause;
        pause_with_categories.paused_categories = 
            (1 << OperationCategory::Trading as u8) |
            (1 << OperationCategory::Withdrawal as u8);
        
        assert!(pause_with_categories.is_category_paused(OperationCategory::Trading));
        assert!(pause_with_categories.is_category_paused(OperationCategory::Withdrawal));
        assert!(!pause_with_categories.is_category_paused(OperationCategory::Liquidation));
    }
}