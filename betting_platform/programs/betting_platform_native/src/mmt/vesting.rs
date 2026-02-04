//! MMT Token Vesting Module
//! 
//! Manages the vesting schedule for the 90M reserved MMT tokens
//! as specified in the protocol requirements

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    mmt::constants::{RESERVED_ALLOCATION, MMT_DECIMALS},
};

/// Vesting schedule types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum VestingScheduleType {
    /// Team allocation - 4 year vesting with 1 year cliff
    Team,
    /// Advisors - 2 year vesting with 6 month cliff  
    Advisors,
    /// Strategic partners - 3 year vesting with 6 month cliff
    Strategic,
    /// Ecosystem development - 5 year linear vesting
    Ecosystem,
    /// Reserve fund - 10 year vesting, unlocks after year 3
    Reserve,
}

/// Vesting schedule definition
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VestingSchedule {
    /// Schedule discriminator
    pub discriminator: [u8; 8],
    /// Schedule type
    pub schedule_type: VestingScheduleType,
    /// Beneficiary pubkey
    pub beneficiary: Pubkey,
    /// Total allocation
    pub total_amount: u64,
    /// Start timestamp
    pub start_timestamp: i64,
    /// Cliff duration in seconds
    pub cliff_duration: u64,
    /// Total vesting duration in seconds
    pub vesting_duration: u64,
    /// Amount already claimed
    pub claimed_amount: u64,
    /// Last claim timestamp
    pub last_claim_timestamp: i64,
    /// Is schedule active
    pub is_active: bool,
    /// Is revocable by admin
    pub is_revocable: bool,
}

impl VestingSchedule {
    pub const LEN: usize = 8 + // discriminator
        1 + // schedule_type
        32 + // beneficiary
        8 + // total_amount
        8 + // start_timestamp
        8 + // cliff_duration
        8 + // vesting_duration
        8 + // claimed_amount
        8 + // last_claim_timestamp
        1 + // is_active
        1; // is_revocable
        
    pub const DISCRIMINATOR: [u8; 8] = [86, 69, 83, 84, 73, 78, 71, 49]; // "VESTING1"
    
    /// Calculate vested amount at current time
    pub fn calculate_vested_amount(&self, current_timestamp: i64) -> Result<u64, ProgramError> {
        if !self.is_active {
            return Ok(0);
        }
        
        let elapsed = current_timestamp.saturating_sub(self.start_timestamp);
        
        // Check if still in cliff period
        if elapsed < self.cliff_duration as i64 {
            return Ok(0);
        }
        
        // Check if fully vested
        if elapsed >= self.vesting_duration as i64 {
            return Ok(self.total_amount);
        }
        
        // Calculate linear vesting
        let vested = (self.total_amount as u128)
            .checked_mul(elapsed as u128)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(self.vesting_duration as u128)
            .ok_or(BettingPlatformError::DivisionByZero)?;
            
        Ok(vested as u64)
    }
    
    /// Calculate claimable amount
    pub fn calculate_claimable(&self, current_timestamp: i64) -> Result<u64, ProgramError> {
        let vested = self.calculate_vested_amount(current_timestamp)?;
        Ok(vested.saturating_sub(self.claimed_amount))
    }
}

impl Sealed for VestingSchedule {}

impl IsInitialized for VestingSchedule {
    fn is_initialized(&self) -> bool {
        self.discriminator == Self::DISCRIMINATOR
    }
}

impl Pack for VestingSchedule {
    const LEN: usize = Self::LEN;
    
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let schedule = VestingSchedule::deserialize(&mut &src[..])
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(schedule)
    }
    
    fn pack_into_slice(&self, dst: &mut [u8]) {
        self.serialize(&mut &mut dst[..]).unwrap();
    }
}

/// Vesting allocations for 90M reserved tokens
pub struct VestingAllocations;

impl VestingAllocations {
    /// Team allocation: 20M MMT (22.2% of reserved)
    pub const TEAM_ALLOCATION: u64 = 20_000_000 * 10u64.pow(MMT_DECIMALS as u32);
    
    /// Advisors allocation: 5M MMT (5.6% of reserved)
    pub const ADVISORS_ALLOCATION: u64 = 5_000_000 * 10u64.pow(MMT_DECIMALS as u32);
    
    /// Strategic partners: 15M MMT (16.7% of reserved)
    pub const STRATEGIC_ALLOCATION: u64 = 15_000_000 * 10u64.pow(MMT_DECIMALS as u32);
    
    /// Ecosystem development: 30M MMT (33.3% of reserved)
    pub const ECOSYSTEM_ALLOCATION: u64 = 30_000_000 * 10u64.pow(MMT_DECIMALS as u32);
    
    /// Reserve fund: 20M MMT (22.2% of reserved)
    pub const RESERVE_ALLOCATION: u64 = 20_000_000 * 10u64.pow(MMT_DECIMALS as u32);
    
    /// Verify total equals 90M
    pub fn verify_total() -> bool {
        let total = Self::TEAM_ALLOCATION +
            Self::ADVISORS_ALLOCATION +
            Self::STRATEGIC_ALLOCATION +
            Self::ECOSYSTEM_ALLOCATION +
            Self::RESERVE_ALLOCATION;
            
        total == RESERVED_ALLOCATION
    }
    
    /// Get vesting parameters for schedule type
    pub fn get_vesting_params(schedule_type: VestingScheduleType) -> (u64, u64) {
        match schedule_type {
            VestingScheduleType::Team => (
                365 * 24 * 60 * 60, // 1 year cliff
                4 * 365 * 24 * 60 * 60, // 4 year total
            ),
            VestingScheduleType::Advisors => (
                180 * 24 * 60 * 60, // 6 month cliff
                2 * 365 * 24 * 60 * 60, // 2 year total
            ),
            VestingScheduleType::Strategic => (
                180 * 24 * 60 * 60, // 6 month cliff
                3 * 365 * 24 * 60 * 60, // 3 year total
            ),
            VestingScheduleType::Ecosystem => (
                0, // No cliff
                5 * 365 * 24 * 60 * 60, // 5 year total
            ),
            VestingScheduleType::Reserve => (
                3 * 365 * 24 * 60 * 60, // 3 year cliff
                10 * 365 * 24 * 60 * 60, // 10 year total
            ),
        }
    }
}

/// Process vesting schedule creation
pub fn process_create_vesting_schedule(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    schedule_type: VestingScheduleType,
    beneficiary: Pubkey,
    allocation: u64,
) -> ProgramResult {
    msg!("Creating vesting schedule: {:?}", schedule_type);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let vesting_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify allocation amount
    let expected_allocation = match schedule_type {
        VestingScheduleType::Team => VestingAllocations::TEAM_ALLOCATION,
        VestingScheduleType::Advisors => VestingAllocations::ADVISORS_ALLOCATION,
        VestingScheduleType::Strategic => VestingAllocations::STRATEGIC_ALLOCATION,
        VestingScheduleType::Ecosystem => VestingAllocations::ECOSYSTEM_ALLOCATION,
        VestingScheduleType::Reserve => VestingAllocations::RESERVE_ALLOCATION,
    };
    
    if allocation > expected_allocation {
        msg!("Allocation {} exceeds maximum {} for {:?}", 
            allocation, expected_allocation, schedule_type);
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Get vesting parameters
    let (cliff_duration, vesting_duration) = VestingAllocations::get_vesting_params(schedule_type);
    
    // Get current time
    let clock = Clock::get()?;
    
    // Create vesting schedule
    let schedule = VestingSchedule {
        discriminator: VestingSchedule::DISCRIMINATOR,
        schedule_type,
        beneficiary,
        total_amount: allocation,
        start_timestamp: clock.unix_timestamp,
        cliff_duration,
        vesting_duration,
        claimed_amount: 0,
        last_claim_timestamp: 0,
        is_active: true,
        is_revocable: schedule_type != VestingScheduleType::Team, // Team allocations are not revocable
    };
    
    // Pack into account
    schedule.pack_into_slice(&mut vesting_account.data.borrow_mut());
    
    msg!("Created vesting schedule for {} MMT over {} seconds", 
        allocation / 10u64.pow(MMT_DECIMALS as u32),
        vesting_duration);
    
    Ok(())
}

/// Process vesting claim
pub fn process_claim_vested(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Processing vesting claim");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let beneficiary = next_account_info(account_info_iter)?;
    let vesting_account = next_account_info(account_info_iter)?;
    let vesting_vault = next_account_info(account_info_iter)?;
    let beneficiary_token_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Verify beneficiary is signer
    if !beneficiary.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load vesting schedule
    let mut schedule = VestingSchedule::unpack(&vesting_account.data.borrow())?;
    
    // Verify beneficiary
    if schedule.beneficiary != *beneficiary.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if active
    if !schedule.is_active {
        return Err(BettingPlatformError::VestingInactive.into());
    }
    
    // Calculate claimable amount
    let clock = Clock::get()?;
    let claimable = schedule.calculate_claimable(clock.unix_timestamp)?;
    
    if claimable == 0 {
        msg!("No tokens available to claim");
        return Err(BettingPlatformError::NothingToClaim.into());
    }
    
    // Transfer tokens from vault to beneficiary
    let transfer_instruction = spl_token::instruction::transfer(
        token_program.key,
        vesting_vault.key,
        beneficiary_token_account.key,
        vesting_vault.key, // PDA is authority
        &[],
        claimable,
    )?;
    
    msg!("Claiming {} vested tokens", claimable / 10u64.pow(MMT_DECIMALS as u32));
    
    solana_program::program::invoke_signed(
        &transfer_instruction,
        &[
            vesting_vault.clone(),
            beneficiary_token_account.clone(),
            token_program.clone(),
        ],
        &[&[b"vesting_vault", &[schedule.schedule_type as u8]]],
    )?;
    
    // Update schedule
    schedule.claimed_amount = schedule.claimed_amount.saturating_add(claimable);
    schedule.last_claim_timestamp = clock.unix_timestamp;
    
    // Pack updated schedule
    schedule.pack_into_slice(&mut vesting_account.data.borrow_mut());
    
    msg!("Successfully claimed {} MMT", claimable / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vesting_allocations() {
        assert!(VestingAllocations::verify_total());
        
        let total = VestingAllocations::TEAM_ALLOCATION +
            VestingAllocations::ADVISORS_ALLOCATION +
            VestingAllocations::STRATEGIC_ALLOCATION +
            VestingAllocations::ECOSYSTEM_ALLOCATION +
            VestingAllocations::RESERVE_ALLOCATION;
            
        assert_eq!(total, RESERVED_ALLOCATION);
    }
    
    #[test]
    fn test_vesting_calculation() {
        let schedule = VestingSchedule {
            discriminator: VestingSchedule::DISCRIMINATOR,
            schedule_type: VestingScheduleType::Team,
            beneficiary: Pubkey::default(),
            total_amount: 1_000_000,
            start_timestamp: 0,
            cliff_duration: 365 * 24 * 60 * 60, // 1 year
            vesting_duration: 4 * 365 * 24 * 60 * 60, // 4 years
            claimed_amount: 0,
            last_claim_timestamp: 0,
            is_active: true,
            is_revocable: false,
        };
        
        // Before cliff - nothing vested
        assert_eq!(schedule.calculate_vested_amount(180 * 24 * 60 * 60).unwrap(), 0);
        
        // After cliff - 25% vested
        assert_eq!(schedule.calculate_vested_amount(365 * 24 * 60 * 60).unwrap(), 250_000);
        
        // After 2 years - 50% vested
        assert_eq!(schedule.calculate_vested_amount(2 * 365 * 24 * 60 * 60).unwrap(), 500_000);
        
        // Fully vested
        assert_eq!(schedule.calculate_vested_amount(4 * 365 * 24 * 60 * 60).unwrap(), 1_000_000);
    }
}