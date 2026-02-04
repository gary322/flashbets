//! Reentrancy Guard
//!
//! Production-grade protection against reentrancy attacks

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::discriminators,
};

/// Reentrancy guard states
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReentrancyState {
    /// Not entered - ready for new operation
    NotEntered = 0,
    /// Entered - operation in progress
    Entered = 1,
    /// Locked - permanent lock (emergency)
    Locked = 2,
}

/// Reentrancy guard account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ReentrancyGuard {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Current state
    pub state: ReentrancyState,
    /// Last operation slot
    pub last_slot: u64,
    /// Operation counter
    pub operation_count: u64,
    /// Emergency lock authority
    pub lock_authority: Pubkey,
}

impl ReentrancyGuard {
    pub const SIZE: usize = 8 + 1 + 8 + 8 + 32; // 57 bytes

    /// Initialize new reentrancy guard
    pub fn new(lock_authority: Pubkey) -> Self {
        Self {
            discriminator: discriminators::REENTRANCY_GUARD,
            state: ReentrancyState::NotEntered,
            last_slot: 0,
            operation_count: 0,
            lock_authority,
        }
    }

    /// Enter guarded section
    pub fn enter(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        match self.state {
            ReentrancyState::NotEntered => {
                self.state = ReentrancyState::Entered;
                self.last_slot = current_slot;
                self.operation_count += 1;
                Ok(())
            }
            ReentrancyState::Entered => {
                msg!("Reentrancy detected at slot {}", current_slot);
                Err(BettingPlatformError::ReentrancyDetected.into())
            }
            ReentrancyState::Locked => {
                msg!("Guard is locked");
                Err(BettingPlatformError::GuardLocked.into())
            }
        }
    }

    /// Exit guarded section
    pub fn exit(&mut self) -> Result<(), ProgramError> {
        match self.state {
            ReentrancyState::Entered => {
                self.state = ReentrancyState::NotEntered;
                Ok(())
            }
            _ => {
                msg!("Invalid exit state: {:?}", self.state);
                Err(BettingPlatformError::InvalidGuardState.into())
            }
        }
    }

    /// Emergency lock
    pub fn emergency_lock(&mut self, authority: &Pubkey) -> Result<(), ProgramError> {
        if *authority != self.lock_authority {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        self.state = ReentrancyState::Locked;
        msg!("Reentrancy guard locked by authority");
        Ok(())
    }

    /// Check if currently guarded
    pub fn is_entered(&self) -> bool {
        self.state == ReentrancyState::Entered
    }

    /// Validate guard account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::REENTRANCY_GUARD {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Reentrancy guard context for automatic cleanup
pub struct ReentrancyContext<'a> {
    guard_account: &'a AccountInfo<'a>,
    guard: ReentrancyGuard,
}

impl<'a> ReentrancyContext<'a> {
    /// Create new context and enter guard
    pub fn new(
        guard_account: &'a AccountInfo<'a>,
        current_slot: u64,
    ) -> Result<Self, ProgramError> {
        let mut guard = ReentrancyGuard::try_from_slice(&guard_account.data.borrow())?;
        guard.validate()?;
        guard.enter(current_slot)?;
        
        // Save state
        guard.serialize(&mut &mut guard_account.data.borrow_mut()[..])?;
        
        Ok(Self {
            guard_account,
            guard,
        })
    }

    /// Exit guard (called automatically on drop)
    pub fn exit(mut self) -> Result<(), ProgramError> {
        self.guard.exit()?;
        self.guard.serialize(&mut &mut self.guard_account.data.borrow_mut()[..])?;
        Ok(())
    }
}

impl<'a> Drop for ReentrancyContext<'a> {
    fn drop(&mut self) {
        // Attempt to exit on drop
        if self.guard.is_entered() {
            let _ = self.guard.exit();
            let _ = self.guard.serialize(&mut &mut self.guard_account.data.borrow_mut()[..]);
        }
    }
}

/// Initialize reentrancy guard account
pub fn initialize_reentrancy_guard<'a>(
    guard_account: &AccountInfo<'a>,
    lock_authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &Rent,
) -> ProgramResult {
    // Verify account is uninitialized
    if !guard_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // Allocate space
    let space = ReentrancyGuard::SIZE;
    let rent_lamports = rent.minimum_balance(space);

    // Create account
    solana_program::program::invoke(
        &solana_program::system_instruction::create_account(
            payer.key,
            guard_account.key,
            rent_lamports,
            space as u64,
            &crate::ID,
        ),
        &[payer.clone(), guard_account.clone(), system_program.clone()],
    )?;

    // Initialize guard
    let guard = ReentrancyGuard::new(*lock_authority.key);
    guard.serialize(&mut &mut guard_account.data.borrow_mut()[..])?;

    msg!("Reentrancy guard initialized");
    Ok(())
}

/// Macro for using reentrancy guard
#[macro_export]
macro_rules! with_reentrancy_guard {
    ($guard_account:expr, $slot:expr, $body:block) => {{
        let _guard = $crate::security::ReentrancyContext::new($guard_account, $slot)?;
        let result = $body;
        _guard.exit()?;
        result
    }};
}

/// Check for potential reentrancy patterns
pub fn check_reentrancy_pattern(
    instructions: &[solana_program::instruction::Instruction],
    current_program_id: &Pubkey,
) -> Result<(), ProgramError> {
    let mut call_depth = 0;
    let max_depth = 3;
    
    for instruction in instructions {
        if instruction.program_id == *current_program_id {
            call_depth += 1;
            if call_depth > max_depth {
                msg!("Potential reentrancy pattern detected: depth {}", call_depth);
                return Err(BettingPlatformError::ReentrancyDetected.into());
            }
        }
    }
    
    Ok(())
}

/// Guard for cross-program invocations
pub struct CrossProgramGuard {
    allowed_programs: Vec<Pubkey>,
    max_depth: u8,
    current_depth: u8,
}

impl CrossProgramGuard {
    pub fn new(allowed_programs: Vec<Pubkey>) -> Self {
        Self {
            allowed_programs,
            max_depth: 2,
            current_depth: 0,
        }
    }

    /// Check if program is allowed
    pub fn is_allowed(&self, program_id: &Pubkey) -> bool {
        self.allowed_programs.contains(program_id) ||
        *program_id == solana_program::system_program::ID ||
        *program_id == spl_token::ID
    }

    /// Enter cross-program call
    pub fn enter_cpi(&mut self, target_program: &Pubkey) -> Result<(), ProgramError> {
        if !self.is_allowed(target_program) {
            msg!("Unauthorized CPI to {}", target_program);
            return Err(BettingPlatformError::UnauthorizedCPI.into());
        }

        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            msg!("CPI depth exceeded: {}", self.current_depth);
            return Err(BettingPlatformError::CPIDepthExceeded.into());
        }

        Ok(())
    }

    /// Exit cross-program call
    pub fn exit_cpi(&mut self) {
        self.current_depth = self.current_depth.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reentrancy_guard_flow() {
        let mut guard = ReentrancyGuard::new(Pubkey::new_unique());
        
        // Initial state
        assert_eq!(guard.state, ReentrancyState::NotEntered);
        
        // Enter guard
        assert!(guard.enter(100).is_ok());
        assert_eq!(guard.state, ReentrancyState::Entered);
        
        // Try to re-enter (should fail)
        assert!(guard.enter(101).is_err());
        
        // Exit guard
        assert!(guard.exit().is_ok());
        assert_eq!(guard.state, ReentrancyState::NotEntered);
        
        // Can enter again
        assert!(guard.enter(102).is_ok());
    }

    #[test]
    fn test_emergency_lock() {
        let authority = Pubkey::new_unique();
        let mut guard = ReentrancyGuard::new(authority);
        
        // Lock with correct authority
        assert!(guard.emergency_lock(&authority).is_ok());
        assert_eq!(guard.state, ReentrancyState::Locked);
        
        // Cannot enter when locked
        assert!(guard.enter(100).is_err());
    }

    #[test]
    fn test_cross_program_guard() {
        let allowed = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        let mut guard = CrossProgramGuard::new(allowed.clone());
        
        // Allowed program
        assert!(guard.enter_cpi(&allowed[0]).is_ok());
        assert_eq!(guard.current_depth, 1);
        
        // System program is always allowed
        assert!(guard.enter_cpi(&solana_program::system_program::ID).is_ok());
        assert_eq!(guard.current_depth, 2);
        
        // Depth limit
        assert!(guard.enter_cpi(&allowed[1]).is_err());
        
        // Exit reduces depth
        guard.exit_cpi();
        assert_eq!(guard.current_depth, 1);
    }
}