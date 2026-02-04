//! Vampire Attack Protection Module
//!
//! Implements protection mechanisms against vampire attacks where
//! malicious actors try to drain the vault during bootstrap phase.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
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
    events::{emit_event, EventType},
    integration::{
        bootstrap_vault_initialization::BootstrapVaultState,
        minimum_viable_vault::VaultViabilityTracker,
    },
};

/// Vampire attack protection constants
pub const VAMPIRE_ATTACK_COVERAGE_THRESHOLD: u64 = 5000; // 0.5 coverage in basis points
pub const SUSPICIOUS_WITHDRAWAL_THRESHOLD: u64 = 2000; // 20% of vault in single withdrawal
pub const RAPID_WITHDRAWAL_WINDOW_SLOTS: u64 = 150; // ~60 seconds
pub const MAX_WITHDRAWALS_PER_WINDOW: u8 = 3; // Max withdrawals in window
pub const RECOVERY_COOLDOWN_SLOTS: u64 = 3000; // ~20 minutes cooldown after attack

/// Vampire attack detector state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VampireAttackDetector {
    /// Is protection currently active
    pub protection_active: bool,
    /// Coverage ratio at last check
    pub last_coverage_ratio: u64,
    /// Number of rapid withdrawals detected
    pub rapid_withdrawal_count: u8,
    /// Slot of first withdrawal in current window
    pub withdrawal_window_start: u64,
    /// Total withdrawn in current window
    pub window_withdrawal_amount: u64,
    /// Attack detected at slot
    pub attack_detected_slot: Option<u64>,
    /// Recovery allowed after slot
    pub recovery_allowed_slot: Option<u64>,
    /// Historical attack count
    pub total_attacks_prevented: u32,
    /// Suspicious addresses
    pub suspicious_addresses: Vec<Pubkey>,
}

impl VampireAttackDetector {
    pub const SIZE: usize = 1 +     // protection_active
        8 +     // last_coverage_ratio
        1 +     // rapid_withdrawal_count
        8 +     // withdrawal_window_start
        8 +     // window_withdrawal_amount
        9 +     // attack_detected_slot (Option)
        9 +     // recovery_allowed_slot (Option)
        4 +     // total_attacks_prevented
        4 +     // suspicious_addresses length
        32 * 10 + // Up to 10 suspicious addresses
        64;     // Padding

    pub fn new() -> Self {
        Self {
            protection_active: true,
            last_coverage_ratio: 10000, // Start at 1.0
            rapid_withdrawal_count: 0,
            withdrawal_window_start: 0,
            window_withdrawal_amount: 0,
            attack_detected_slot: None,
            recovery_allowed_slot: None,
            total_attacks_prevented: 0,
            suspicious_addresses: Vec::new(),
        }
    }
}

/// Check for vampire attack before processing withdrawal
pub fn check_vampire_attack_withdrawal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    withdrawal_amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let withdrawer = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let detector_account = next_account_info(account_info_iter)?;
    let viability_tracker_account = next_account_info(account_info_iter)?;
    
    let clock = Clock::get()?;
    
    // Load states
    let vault = BootstrapVaultState::deserialize(&mut &vault_account.data.borrow()[..])?;
    let mut detector = VampireAttackDetector::deserialize(&mut &detector_account.data.borrow()[..])?;
    let tracker = VaultViabilityTracker::deserialize(&mut &viability_tracker_account.data.borrow()[..])?;
    
    // Skip checks if not in bootstrap phase
    if !vault.is_bootstrap_phase || vault.bootstrap_complete {
        return Ok(());
    }
    
    // Check if still in recovery cooldown
    if let Some(recovery_slot) = detector.recovery_allowed_slot {
        if clock.slot < recovery_slot {
            msg!("Vampire attack recovery cooldown active");
            return Err(BettingPlatformError::VampireAttackCooldown.into());
        }
    }
    
    // Calculate post-withdrawal coverage
    let post_withdrawal_balance = vault.total_deposits
        .checked_sub(withdrawal_amount)
        .ok_or(BettingPlatformError::InsufficientBalance)?;
    
    let post_withdrawal_coverage = if vault.minimum_viable_size > 0 {
        (post_withdrawal_balance * 10000) / vault.minimum_viable_size
    } else {
        10000 // 1.0 if no minimum set
    };
    
    // Check 1: Coverage ratio threshold
    if post_withdrawal_coverage < VAMPIRE_ATTACK_COVERAGE_THRESHOLD {
        handle_vampire_attack(
            &mut detector,
            withdrawer.key,
            withdrawal_amount,
            VampireAttackType::CoverageDrain,
            clock.slot,
        )?;
        
        detector.serialize(&mut &mut detector_account.data.borrow_mut()[..])?;
        return Err(BettingPlatformError::VampireAttackDetected.into());
    }
    
    // Check 2: Large single withdrawal
    let withdrawal_percentage = (withdrawal_amount * 10000) / vault.total_deposits;
    if withdrawal_percentage > SUSPICIOUS_WITHDRAWAL_THRESHOLD {
        handle_vampire_attack(
            &mut detector,
            withdrawer.key,
            withdrawal_amount,
            VampireAttackType::LargeWithdrawal,
            clock.slot,
        )?;
        
        detector.serialize(&mut &mut detector_account.data.borrow_mut()[..])?;
        return Err(BettingPlatformError::SuspiciousWithdrawal.into());
    }
    
    // Check 3: Rapid withdrawals
    if clock.slot > detector.withdrawal_window_start + RAPID_WITHDRAWAL_WINDOW_SLOTS {
        // Reset window
        detector.withdrawal_window_start = clock.slot;
        detector.rapid_withdrawal_count = 0;
        detector.window_withdrawal_amount = 0;
    }
    
    detector.rapid_withdrawal_count += 1;
    detector.window_withdrawal_amount = detector.window_withdrawal_amount
        .checked_add(withdrawal_amount)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    if detector.rapid_withdrawal_count > MAX_WITHDRAWALS_PER_WINDOW {
        handle_vampire_attack(
            &mut detector,
            withdrawer.key,
            withdrawal_amount,
            VampireAttackType::RapidWithdrawals,
            clock.slot,
        )?;
        
        detector.serialize(&mut &mut detector_account.data.borrow_mut()[..])?;
        return Err(BettingPlatformError::RapidWithdrawalsDetected.into());
    }
    
    // Check 4: Suspicious address
    if detector.suspicious_addresses.contains(withdrawer.key) {
        msg!("Withdrawal blocked: Address flagged as suspicious");
        return Err(BettingPlatformError::SuspiciousAddress.into());
    }
    
    // Update detector state
    detector.last_coverage_ratio = post_withdrawal_coverage;
    detector.serialize(&mut &mut detector_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Handle detected vampire attack
fn handle_vampire_attack(
    detector: &mut VampireAttackDetector,
    attacker: &Pubkey,
    amount: u64,
    attack_type: VampireAttackType,
    current_slot: u64,
) -> ProgramResult {
    detector.attack_detected_slot = Some(current_slot);
    detector.recovery_allowed_slot = Some(current_slot + RECOVERY_COOLDOWN_SLOTS);
    detector.total_attacks_prevented += 1;
    
    // Add attacker to suspicious list if not already there
    if !detector.suspicious_addresses.contains(attacker) && 
       detector.suspicious_addresses.len() < 10 {
        detector.suspicious_addresses.push(*attacker);
    }
    
    msg!("🚨 VAMPIRE ATTACK DETECTED!");
    msg!("  Type: {:?}", attack_type);
    msg!("  Attacker: {}", attacker);
    msg!("  Amount: ${}", amount / 1_000_000);
    msg!("  Recovery allowed after slot: {}", current_slot + RECOVERY_COOLDOWN_SLOTS);
    
    emit_event(EventType::VampireAttackDetected, &VampireAttackDetectedEvent {
        attacker: *attacker,
        attack_type: attack_type as u8,
        amount,
        coverage_ratio: detector.last_coverage_ratio,
        slot: current_slot,
    });
    
    Ok(())
}

/// Initialize vampire attack detector
pub fn initialize_vampire_detector(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let detector_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let detector = VampireAttackDetector::new();
    detector.serialize(&mut &mut detector_account.data.borrow_mut()[..])?;
    
    msg!("Vampire attack detector initialized");
    msg!("Coverage threshold: {}%", VAMPIRE_ATTACK_COVERAGE_THRESHOLD / 100);
    msg!("Suspicious withdrawal: {}%", SUSPICIOUS_WITHDRAWAL_THRESHOLD / 100);
    
    Ok(())
}

/// Admin function to reset vampire attack protection
pub fn reset_vampire_protection(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let detector_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut detector = VampireAttackDetector::deserialize(&mut &detector_account.data.borrow()[..])?;
    
    // Reset protection state
    detector.attack_detected_slot = None;
    detector.recovery_allowed_slot = None;
    detector.rapid_withdrawal_count = 0;
    detector.window_withdrawal_amount = 0;
    
    detector.serialize(&mut &mut detector_account.data.borrow_mut()[..])?;
    
    msg!("Vampire attack protection reset by admin");
    
    emit_event(EventType::VampireProtectionReset, &VampireProtectionResetEvent {
        admin: *authority.key,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}

/// Check if address is suspicious
pub fn is_address_suspicious(
    detector_account: &AccountInfo,
    address: &Pubkey,
) -> Result<bool, ProgramError> {
    let detector = VampireAttackDetector::deserialize(&mut &detector_account.data.borrow()[..])?;
    Ok(detector.suspicious_addresses.contains(address))
}

/// Types of vampire attacks
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum VampireAttackType {
    CoverageDrain = 0,
    LargeWithdrawal = 1,
    RapidWithdrawals = 2,
    CoordinatedAttack = 3,
}

// Event definitions
#[derive(BorshSerialize, BorshDeserialize)]
pub struct VampireAttackDetectedEvent {
    pub attacker: Pubkey,
    pub attack_type: u8,
    pub amount: u64,
    pub coverage_ratio: u64,
    pub slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct VampireProtectionResetEvent {
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_coverage_calculation() {
        // Test coverage below threshold
        let post_balance = 4_000_000_000; // $4k
        let minimum_viable = 10_000_000_000; // $10k
        let coverage = (post_balance * 10000) / minimum_viable;
        assert_eq!(coverage, 4000); // 0.4 coverage
        assert!(coverage < VAMPIRE_ATTACK_COVERAGE_THRESHOLD);
        
        // Test coverage at threshold
        let post_balance = 5_000_000_000; // $5k
        let coverage = (post_balance * 10000) / minimum_viable;
        assert_eq!(coverage, 5000); // 0.5 coverage
        assert_eq!(coverage, VAMPIRE_ATTACK_COVERAGE_THRESHOLD);
    }
    
    #[test]
    fn test_withdrawal_percentage() {
        let vault_balance = 10_000_000_000; // $10k
        let withdrawal = 2_500_000_000; // $2.5k
        let percentage = (withdrawal * 10000) / vault_balance;
        assert_eq!(percentage, 2500); // 25%
        assert!(percentage > SUSPICIOUS_WITHDRAWAL_THRESHOLD);
    }
    
    #[test]
    fn test_detector_initialization() {
        let detector = VampireAttackDetector::new();
        assert!(detector.protection_active);
        assert_eq!(detector.last_coverage_ratio, 10000);
        assert_eq!(detector.rapid_withdrawal_count, 0);
        assert!(detector.attack_detected_slot.is_none());
        assert!(detector.suspicious_addresses.is_empty());
    }
}