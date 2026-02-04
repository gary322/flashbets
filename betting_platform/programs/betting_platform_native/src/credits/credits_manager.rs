//! Credits Manager Implementation
//!
//! Handles deposit-to-credits conversion and tracking across proposals

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    state::{VersePDA, UserMap},
    account_validation::DISCRIMINATOR_SIZE,
};

/// Discriminator for user credits account
pub const USER_CREDITS_DISCRIMINATOR: [u8; 8] = [99, 114, 101, 100, 105, 116, 115, 0];

/// User credits account - tracks deposits as credits across proposals
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct UserCredits {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User public key
    pub user: Pubkey,
    
    /// Verse ID these credits belong to
    pub verse_id: u128,
    
    /// Total deposit amount (equals total credits)
    pub total_deposit: u64,
    
    /// Available credits (not locked in positions)
    pub available_credits: u64,
    
    /// Credits locked in positions
    pub locked_credits: u64,
    
    /// Number of active positions using these credits
    pub active_positions: u32,
    
    /// Timestamp of last update
    pub last_update: i64,
    
    /// Is eligible for refund
    pub refund_eligible: bool,
    
    /// PDA bump seed
    pub bump: u8,
}

impl UserCredits {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 16 + 8 + 8 + 8 + 4 + 8 + 1 + 1;
    
    /// Create new user credits from deposit
    pub fn new(user: Pubkey, verse_id: u128, deposit: u64, bump: u8) -> Self {
        let clock = Clock::get().unwrap_or_default();
        
        Self {
            discriminator: USER_CREDITS_DISCRIMINATOR,
            user,
            verse_id,
            total_deposit: deposit,
            available_credits: deposit, // Credits = deposit (1:1)
            locked_credits: 0,
            active_positions: 0,
            last_update: clock.unix_timestamp,
            refund_eligible: false,
            bump,
        }
    }
    
    /// Validate account data
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != USER_CREDITS_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Total deposit must equal available + locked
        if self.total_deposit != self.available_credits + self.locked_credits {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
    
    /// Lock credits for a new position
    pub fn lock_credits(&mut self, amount: u64) -> Result<(), ProgramError> {
        if amount > self.available_credits {
            return Err(BettingPlatformError::InsufficientBalance.into());
        }
        
        self.available_credits = self.available_credits.saturating_sub(amount);
        self.locked_credits = self.locked_credits.saturating_add(amount);
        self.active_positions = self.active_positions.saturating_add(1);
        
        let clock = Clock::get()?;
        self.last_update = clock.unix_timestamp;
        
        Ok(())
    }
    
    /// Release credits when position closes
    pub fn release_credits(&mut self, amount: u64) -> Result<(), ProgramError> {
        if amount > self.locked_credits {
            return Err(ProgramError::InvalidAccountData);
        }
        
        self.locked_credits = self.locked_credits.saturating_sub(amount);
        self.available_credits = self.available_credits.saturating_add(amount);
        self.active_positions = self.active_positions.saturating_sub(1);
        
        let clock = Clock::get()?;
        self.last_update = clock.unix_timestamp;
        
        Ok(())
    }
    
    /// Mark as eligible for refund
    pub fn mark_refund_eligible(&mut self) {
        self.refund_eligible = true;
    }
    
    /// Process refund - returns amount to refund
    pub fn process_refund(&mut self) -> Result<u64, ProgramError> {
        if !self.refund_eligible {
            return Err(BettingPlatformError::NotEligibleForRefund.into());
        }
        
        if self.active_positions > 0 {
            return Err(BettingPlatformError::ActivePositionsExist.into());
        }
        
        // Return all available credits
        let refund_amount = self.available_credits;
        self.available_credits = 0;
        self.total_deposit = self.locked_credits; // Only locked credits remain
        
        Ok(refund_amount)
    }
}

/// Credits manager for handling deposit conversions
pub struct CreditsManager;

impl CreditsManager {
    /// Convert deposit to credits for a verse
    pub fn deposit_to_credits(
        user: &Pubkey,
        verse: &VersePDA,
        deposit_amount: u64,
    ) -> Result<CreditsConversion, ProgramError> {
        // Validate verse is active
        if verse.status != crate::state::VerseStatus::Active {
            return Err(BettingPlatformError::VerseNotActive.into());
        }
        
        // Credits = deposit (1:1 conversion as per spec)
        let credits = deposit_amount;
        
        // Calculate efficiency metrics
        let num_proposals = verse.child_count.max(1) as u64;
        let max_positions = num_proposals.min(32); // Cap at 32 for safety
        
        Ok(CreditsConversion {
            deposit: deposit_amount,
            credits,
            verse_id: verse.verse_id,
            max_positions: max_positions as u8,
            conversion_rate: 1, // 1:1 conversion
        })
    }
    
    /// Check if user can open position with credits
    pub fn can_open_position(
        user_credits: &UserCredits,
        position_size: u64,
        leverage: u8,
    ) -> Result<bool, ProgramError> {
        // Calculate required credits (margin for position)
        let required_credits = position_size / leverage as u64;
        
        if required_credits > user_credits.available_credits {
            return Ok(false);
        }
        
        // Check if user has too many positions
        if user_credits.active_positions >= 32 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Calculate refund amount at settle_slot
    pub fn calculate_refund(
        user_credits: &UserCredits,
        verse: &VersePDA,
        current_slot: u64,
        settle_slot: u64,
    ) -> Result<u64, ProgramError> {
        // Only refund at or after settle_slot
        if current_slot < settle_slot {
            return Ok(0);
        }
        
        // Only refund if verse is resolved or user has no positions
        if verse.status != crate::state::VerseStatus::Resolved && user_credits.active_positions > 0 {
            return Ok(0);
        }
        
        // Refund all available credits
        Ok(user_credits.available_credits)
    }
}

/// Result of deposit to credits conversion
#[derive(Debug)]
pub struct CreditsConversion {
    /// Original deposit amount
    pub deposit: u64,
    /// Credits received
    pub credits: u64,
    /// Verse ID
    pub verse_id: u128,
    /// Maximum positions allowed
    pub max_positions: u8,
    /// Conversion rate (always 1 for 1:1)
    pub conversion_rate: u64,
}

/// Derive PDA for user credits account
pub fn derive_user_credits_pda(
    program_id: &Pubkey,
    user: &Pubkey,
    verse_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"user_credits",
            user.as_ref(),
            &verse_id.to_le_bytes(),
        ],
        program_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_credits_creation() {
        let user = Pubkey::new_unique();
        let credits = UserCredits::new(user, 1, 1000, 255);
        
        assert_eq!(credits.total_deposit, 1000);
        assert_eq!(credits.available_credits, 1000);
        assert_eq!(credits.locked_credits, 0);
        assert_eq!(credits.active_positions, 0);
    }
    
    #[test]
    fn test_credit_locking() {
        let user = Pubkey::new_unique();
        let mut credits = UserCredits::new(user, 1, 1000, 255);
        
        // Lock 400 credits
        assert!(credits.lock_credits(400).is_ok());
        assert_eq!(credits.available_credits, 600);
        assert_eq!(credits.locked_credits, 400);
        assert_eq!(credits.active_positions, 1);
        
        // Try to lock more than available
        assert!(credits.lock_credits(700).is_err());
        
        // Lock remaining
        assert!(credits.lock_credits(600).is_ok());
        assert_eq!(credits.available_credits, 0);
        assert_eq!(credits.locked_credits, 1000);
        assert_eq!(credits.active_positions, 2);
    }
    
    #[test]
    fn test_credit_release() {
        let user = Pubkey::new_unique();
        let mut credits = UserCredits::new(user, 1, 1000, 255);
        
        // Lock and release credits
        credits.lock_credits(400).unwrap();
        assert!(credits.release_credits(200).is_ok());
        
        assert_eq!(credits.available_credits, 800);
        assert_eq!(credits.locked_credits, 200);
        assert_eq!(credits.active_positions, 0);
    }
}