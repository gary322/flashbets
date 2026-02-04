//! Critical Exploit Halt Mechanism for Migration
//!
//! Implements emergency halt functionality for old program when critical exploits are detected

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
    security::emergency_pause::{PauseLevel, OperationCategory},
    events::{EventType, emit_event, EmergencyHaltEvent},
};

/// Critical exploit types that trigger halts
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ExploitType {
    /// Integer overflow/underflow exploit
    IntegerOverflow,
    /// Reentrancy attack detected
    Reentrancy,
    /// Flash loan attack detected
    FlashLoan,
    /// Oracle manipulation detected
    OracleManipulation,
    /// Unauthorized access attempt
    UnauthorizedAccess,
    /// Abnormal liquidation cascade
    LiquidationCascade,
    /// State corruption detected
    StateCorruption,
    /// Unknown critical exploit
    Unknown,
}

/// Exploit detection result
#[derive(Debug, Clone)]
pub struct ExploitDetection {
    pub exploit_type: ExploitType,
    pub severity: ExploitSeverity,
    pub affected_accounts: Vec<Pubkey>,
    pub estimated_damage: u64,
    pub detection_confidence: u8, // 0-100%
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ExploitSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Migration halt state for old program
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationHaltState {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Old program ID being halted
    pub old_program_id: Pubkey,
    
    /// New program ID (migration target)
    pub new_program_id: Pubkey,
    
    /// Halt authority (security council)
    pub halt_authority: Pubkey,
    
    /// Is halted
    pub is_halted: bool,
    
    /// Halt reason
    pub halt_reason: HaltReason,
    
    /// Halt timestamp
    pub halt_timestamp: i64,
    
    /// Halt slot
    pub halt_slot: u64,
    
    /// Allow position closes only
    pub allow_closes_only: bool,
    
    /// Emergency withdrawal enabled
    pub emergency_withdrawal_enabled: bool,
    
    /// Exploit details
    pub exploit_info: Option<ExploitInfo>,
    
    /// Total halts triggered
    pub total_halts: u32,
    
    /// Auto-resume slot (0 = manual only)
    pub auto_resume_slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ExploitInfo {
    pub exploit_type: ExploitType,
    pub severity: ExploitSeverity,
    pub detection_slot: u64,
    pub affected_users: u32,
    pub estimated_loss: u64,
    pub patched: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum HaltReason {
    /// Critical exploit detected
    CriticalExploit,
    /// Security audit finding
    SecurityAudit,
    /// Regulatory requirement
    Regulatory,
    /// Technical maintenance
    Maintenance,
    /// User protection
    UserProtection,
}

impl MigrationHaltState {
    pub const SIZE: usize = 8 + // discriminator
        32 + // old_program_id
        32 + // new_program_id
        32 + // halt_authority
        1 + // is_halted
        1 + 128 + // halt_reason (enum + padding)
        8 + // halt_timestamp
        8 + // halt_slot
        1 + // allow_closes_only
        1 + // emergency_withdrawal_enabled
        1 + 256 + // exploit_info (Option + struct)
        4 + // total_halts
        8; // auto_resume_slot
        
    /// Create new halt state
    pub fn new(
        old_program_id: Pubkey,
        new_program_id: Pubkey,
        halt_authority: Pubkey,
    ) -> Self {
        Self {
            discriminator: MIGRATION_HALT_DISCRIMINATOR,
            old_program_id,
            new_program_id,
            halt_authority,
            is_halted: false,
            halt_reason: HaltReason::CriticalExploit,
            halt_timestamp: 0,
            halt_slot: 0,
            allow_closes_only: false,
            emergency_withdrawal_enabled: false,
            exploit_info: None,
            total_halts: 0,
            auto_resume_slot: 0,
        }
    }
    
    /// Trigger emergency halt
    pub fn trigger_halt(
        &mut self,
        reason: HaltReason,
        exploit_detection: Option<ExploitDetection>,
    ) -> ProgramResult {
        if self.is_halted {
            return Err(BettingPlatformError::AlreadyHalted.into());
        }
        
        let clock = Clock::get()?;
        
        self.is_halted = true;
        self.halt_reason = reason;
        self.halt_timestamp = clock.unix_timestamp;
        self.halt_slot = clock.slot;
        self.total_halts += 1;
        
        // Configure halt based on reason
        match reason {
            HaltReason::CriticalExploit => {
                self.allow_closes_only = true;
                self.emergency_withdrawal_enabled = true;
                
                if let Some(detection) = exploit_detection {
                    self.exploit_info = Some(ExploitInfo {
                        exploit_type: detection.exploit_type,
                        severity: detection.severity,
                        detection_slot: clock.slot,
                        affected_users: detection.affected_accounts.len() as u32,
                        estimated_loss: detection.estimated_damage,
                        patched: false,
                    });
                }
            }
            HaltReason::SecurityAudit => {
                self.allow_closes_only = true;
                self.emergency_withdrawal_enabled = false;
            }
            HaltReason::UserProtection => {
                self.allow_closes_only = true;
                self.emergency_withdrawal_enabled = true;
            }
            _ => {
                self.allow_closes_only = false;
                self.emergency_withdrawal_enabled = false;
            }
        }
        
        msg!("EMERGENCY HALT TRIGGERED: {:?}", reason);
        
        // Emit halt event
        emit_event(EventType::EmergencyHaltEvent, &EmergencyHaltEvent {
            slot: clock.slot,
            reason: format!("{:?}", reason),
        });
        
        Ok(())
    }
    
    /// Check if operation is allowed during halt
    pub fn is_operation_allowed(&self, operation: OperationCategory) -> bool {
        if !self.is_halted {
            return true;
        }
        
        match operation {
            // Always allow read operations
            OperationCategory::View => true,
            
            // Allow emergency operations if enabled
            OperationCategory::Emergency => self.emergency_withdrawal_enabled,
            
            // Allow closes if configured
            OperationCategory::Trading => self.allow_closes_only,
            
            // Block everything else during halt
            _ => false,
        }
    }
    
    /// Resume from halt
    pub fn resume_operations(&mut self, authority: &Pubkey) -> ProgramResult {
        if !self.is_halted {
            return Err(BettingPlatformError::NotHalted.into());
        }
        
        if authority != &self.halt_authority {
            return Err(BettingPlatformError::UnauthorizedHaltOperation.into());
        }
        
        let clock = Clock::get()?;
        let halt_duration = clock.slot.saturating_sub(self.halt_slot);
        
        self.is_halted = false;
        self.allow_closes_only = false;
        self.emergency_withdrawal_enabled = false;
        
        msg!("Operations resumed after {} slots", halt_duration);
        
        Ok(())
    }
    
    /// Check if auto-resume should trigger
    pub fn check_auto_resume(&mut self) -> ProgramResult {
        if !self.is_halted || self.auto_resume_slot == 0 {
            return Ok(());
        }
        
        let current_slot = Clock::get()?.slot;
        
        if current_slot >= self.auto_resume_slot {
            self.is_halted = false;
            self.allow_closes_only = false;
            self.emergency_withdrawal_enabled = false;
            
            msg!("Auto-resumed at slot {}", current_slot);
        }
        
        Ok(())
    }
}

/// Exploit detection engine
pub struct ExploitDetector;

impl ExploitDetector {
    /// Detect potential exploits
    pub fn detect_exploit(
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> Option<ExploitDetection> {
        // Check for integer overflow patterns
        if Self::detect_integer_overflow(instruction_data) {
            return Some(ExploitDetection {
                exploit_type: ExploitType::IntegerOverflow,
                severity: ExploitSeverity::Critical,
                affected_accounts: vec![],
                estimated_damage: 0,
                detection_confidence: 90,
            });
        }
        
        // Check for reentrancy patterns
        if Self::detect_reentrancy(accounts) {
            return Some(ExploitDetection {
                exploit_type: ExploitType::Reentrancy,
                severity: ExploitSeverity::High,
                affected_accounts: accounts.iter().map(|a| *a.key).collect(),
                estimated_damage: 0,
                detection_confidence: 85,
            });
        }
        
        // Check for flash loan patterns
        if Self::detect_flash_loan(accounts, instruction_data) {
            return Some(ExploitDetection {
                exploit_type: ExploitType::FlashLoan,
                severity: ExploitSeverity::High,
                affected_accounts: vec![],
                estimated_damage: 0,
                detection_confidence: 80,
            });
        }
        
        None
    }
    
    /// Detect integer overflow patterns
    fn detect_integer_overflow(instruction_data: &[u8]) -> bool {
        // Check for suspicious large values
        if instruction_data.len() >= 8 {
            let value = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap_or_default());
            if value > u64::MAX / 2 {
                return true;
            }
        }
        false
    }
    
    /// Detect reentrancy patterns
    fn detect_reentrancy(accounts: &[AccountInfo]) -> bool {
        // Check for circular dependencies or multiple mutable borrows
        for (i, account) in accounts.iter().enumerate() {
            if account.is_writable {
                for (j, other) in accounts.iter().enumerate() {
                    if i != j && account.key == other.key && other.is_writable {
                        return true;
                    }
                }
            }
        }
        false
    }
    
    /// Detect flash loan patterns
    fn detect_flash_loan(_accounts: &[AccountInfo], instruction_data: &[u8]) -> bool {
        // Check for borrow and repay in same transaction
        // Simplified check - in production would analyze full transaction
        instruction_data.len() > 100 // Suspiciously large instruction
    }
}

// Use discriminator constant
pub const MIGRATION_HALT_DISCRIMINATOR: [u8; 8] = [77, 73, 71, 72, 65, 76, 84, 83]; // "MIGHALTS"

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_halt_trigger() {
        let mut halt_state = MigrationHaltState::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        
        assert!(!halt_state.is_halted);
        
        halt_state.trigger_halt(
            HaltReason::CriticalExploit,
            Some(ExploitDetection {
                exploit_type: ExploitType::IntegerOverflow,
                severity: ExploitSeverity::Critical,
                affected_accounts: vec![],
                estimated_damage: 1_000_000,
                detection_confidence: 95,
            })
        ).unwrap();
        
        assert!(halt_state.is_halted);
        assert!(halt_state.allow_closes_only);
        assert!(halt_state.emergency_withdrawal_enabled);
    }
    
    #[test]
    fn test_operation_permissions() {
        let mut halt_state = MigrationHaltState::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        
        // Before halt - all allowed
        assert!(halt_state.is_operation_allowed(OperationCategory::Trading));
        
        // After halt - restricted
        halt_state.trigger_halt(HaltReason::CriticalExploit, None).unwrap();
        assert!(halt_state.is_operation_allowed(OperationCategory::View));
        assert!(halt_state.is_operation_allowed(OperationCategory::Emergency));
        assert!(!halt_state.is_operation_allowed(OperationCategory::Deposit));
    }
}