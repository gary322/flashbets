// Phase 20: Bootstrap Coordination
// Manages the bootstrap phase from $0 to $10k vault for viable leverage

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
    events::{emit_event, EventType, BootstrapStartedEvent, BootstrapDepositEvent, 
             BootstrapCompleteDetailedEvent, MilestoneReachedEvent, ReferralRewardEvent},
    mmt::{
        state::DistributionType,
        constants::*,
    },
};

use crate::math::fixed_point::U64F64;

/// Bootstrap phase constants
// Re-export constants for external use
pub use crate::constants::{BOOTSTRAP_TARGET_VAULT, BOOTSTRAP_FEE_BPS, BOOTSTRAP_MMT_MULTIPLIER};
pub const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000; // $1 minimum
pub const BOOTSTRAP_MMT_EMISSION_RATE: u64 = 10_000_000_000_000; // 10M MMT per season
pub const BOOTSTRAP_IMMEDIATE_REWARD_BPS: u16 = 10000; // 100% immediate for first providers
pub const VAMPIRE_ATTACK_HALT_COVERAGE: u64 = 5000; // 0.5 coverage threshold (basis points)
pub const BOOTSTRAP_MILESTONES: [u64; 5] = [
    1_000_000_000,   // $1k
    2_500_000_000,   // $2.5k
    5_000_000_000,   // $5k
    7_500_000_000,   // $7.5k
    10_000_000_000,  // $10k
];

/// Bootstrap coordinator state
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct BootstrapCoordinator {
    pub vault_balance: u64,
    pub total_deposits: u64,
    pub unique_depositors: u32,
    pub current_milestone: u8,
    pub bootstrap_start_slot: u64,
    pub bootstrap_complete: bool,
    pub coverage_ratio: u64,  // Store as u64, convert to/from U64F64 when needed
    pub max_leverage_available: u64,
    pub total_mmt_distributed: u64,
    pub early_depositor_bonus_active: bool,
    pub incentive_pool: u64,
    pub halted: bool,
    pub total_incentive_pool: u64,
    pub is_active: bool,
    pub current_vault_balance: u64,
}

impl BootstrapCoordinator {
    pub const SIZE: usize = 8 + // vault_balance
        8 + // total_deposits
        4 + // unique_depositors
        1 + // current_milestone
        8 + // bootstrap_start_slot
        1 + // bootstrap_complete
        8 + // coverage_ratio
        8 + // max_leverage_available
        8 + // total_mmt_distributed
        1 + // early_depositor_bonus_active
        8 + // incentive_pool
        1 + // halted
        8 + // total_incentive_pool
        1 + // is_active
        8; // current_vault_balance

    /// Initialize bootstrap coordinator
    pub fn initialize(&mut self, current_slot: u64) -> ProgramResult {
        self.vault_balance_balance = 0;
        self.total_deposits = 0;
        self.unique_depositors = 0;
        self.current_milestone = 0;
        self.bootstrap_start_slot = current_slot;
        self.bootstrap_complete = false;
        self.coverage_ratio = 0;
        self.max_leverage_available = 0;
        self.total_mmt_distributed = 0;
        self.early_depositor_bonus_active = true;
        // 10M MMT per season emission allocated to early liquidity providers
        self.incentive_pool = BOOTSTRAP_MMT_EMISSION_RATE;
        self.halted = false;
        self.total_incentive_pool = BOOTSTRAP_MMT_EMISSION_RATE;
        self.is_active = true;
        self.current_vault_balance = 0;

        msg!("Bootstrap coordinator initialized");
        emit_event(EventType::BootstrapStarted, &BootstrapStartedEvent {
            target_vault: BOOTSTRAP_TARGET_VAULT,
            incentive_pool: self.incentive_pool,
        });

        Ok(())
    }

    /// Process a bootstrap deposit
    pub fn process_deposit(
        &mut self,
        depositor: &Pubkey,
        amount: u64,
        is_new_depositor: bool,
    ) -> Result<BootstrapDepositResult, ProgramError> {
        if self.bootstrap_complete {
            return Err(BettingPlatformError::BootstrapAlreadyComplete.into());
        }

        if amount < MIN_DEPOSIT_AMOUNT {
            return Err(BettingPlatformError::DepositTooSmall.into());
        }

        // Update balances
        self.vault_balance = self.vault_balance
            .checked_add(amount)
            .ok_or(BettingPlatformError::MathOverflow)?;
        
        self.total_deposits = self.total_deposits
            .checked_add(amount)
            .ok_or(BettingPlatformError::MathOverflow)?;

        if is_new_depositor {
            self.unique_depositors += 1;
        }

        // Calculate MMT rewards
        let base_mmt = self.calculate_base_mmt_reward(amount)?;
        let bonus_mmt = self.calculate_bonus_mmt(amount, is_new_depositor)?;
        let total_mmt = base_mmt + bonus_mmt;

        self.total_mmt_distributed = self.total_mmt_distributed
            .checked_add(total_mmt)
            .ok_or(BettingPlatformError::MathOverflow)?;

        // Check milestone progress
        let milestone_reached = self.check_milestone_progress()?;

        // Update coverage and leverage
        self.update_coverage_and_leverage()?;

        // Check if bootstrap complete
        if self.vault_balance >= BOOTSTRAP_TARGET_VAULT {
            self.complete_bootstrap()?;
        }

        emit_event(EventType::BootstrapDeposit, &BootstrapDepositEvent {
            depositor: *depositor,
            amount,
            vault_balance: self.vault_balance,
            mmt_earned: total_mmt,
        });

        Ok(BootstrapDepositResult {
            mmt_earned: total_mmt,
            new_milestone_reached: milestone_reached,
            current_progress_percent: (self.vault_balance * 100) / BOOTSTRAP_TARGET_VAULT,
            leverage_now_available: self.max_leverage_available > 0,
        })
    }

    /// Calculate base MMT reward (2x during bootstrap)
    fn calculate_base_mmt_reward(&self, deposit_amount: u64) -> Result<u64, ProgramError> {
        // Base rate: 1 MMT per $1 deposited, 2x during bootstrap
        // Immediate distribution from seasonal emission (10M MMT/season)
        let deposit_in_dollars = deposit_amount / 1_000_000; // Convert to dollars
        let base_reward = deposit_in_dollars * BOOTSTRAP_MMT_MULTIPLIER * 1_000_000; // MMT has 6 decimals
        
        // Scale reward based on remaining incentive pool
        let scaled_reward = if self.incentive_pool > 0 {
            let reward = base_reward.min(self.incentive_pool);
            reward
        } else {
            0
        };
        
        Ok(scaled_reward)
    }

    /// Calculate bonus MMT for early depositors
    fn calculate_bonus_mmt(
        &self,
        deposit_amount: u64,
        is_new_depositor: bool,
    ) -> Result<u64, ProgramError> {
        let mut bonus = 0u64;

        // Early depositor bonus (first 100 depositors)
        if self.early_depositor_bonus_active && self.unique_depositors <= 100 {
            bonus += deposit_amount / 10_000_000; // 0.01 MMT per $1
        }

        // New depositor bonus
        if is_new_depositor {
            bonus += 1000; // 1000 MMT bonus for new users
        }

        // Milestone bonus
        let milestone_multiplier = match self.current_milestone {
            0 => 150, // 1.5x before first milestone
            1 => 140, // 1.4x
            2 => 130, // 1.3x
            3 => 120, // 1.2x
            4 => 110, // 1.1x
            _ => 100, // 1x
        };

        bonus = (bonus * milestone_multiplier) / 100;

        Ok(bonus)
    }

    /// Check and update milestone progress
    fn check_milestone_progress(&mut self) -> Result<bool, ProgramError> {
        let current = self.current_milestone as usize;
        
        if current < BOOTSTRAP_MILESTONES.len() {
            if self.vault_balance >= BOOTSTRAP_MILESTONES[current] {
                self.current_milestone += 1;
                
                msg!("Bootstrap milestone {} reached! Vault: ${}", 
                    self.current_milestone,
                    self.vault_balance / 1_000_000);

                emit_event(EventType::BootstrapProgress, &MilestoneReachedEvent {
                    milestone: self.current_milestone,
                    vault_balance: self.vault_balance,
                });

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Update coverage ratio and available leverage
    fn update_coverage_and_leverage(&mut self) -> Result<(), ProgramError> {
        // Simplified coverage calculation for bootstrap
        // coverage = vault / (assumed_tail_loss * assumed_oi)
        // During bootstrap, we assume minimal OI
        
        if self.vault_balance >= 1_000_000_000 { // $1k minimum for any leverage
            // Linear scaling: $1k = 1x, $10k = 10x
            self.max_leverage_available = (self.vault_balance / 1_000_000_000).min(10);
            
            // Coverage ratio in fixed point (10000 = 1.0)
            self.coverage_ratio = (self.vault_balance * 10000) / 1_000_000_000;
            
            // Check for vampire attack protection
            if self.coverage_ratio < VAMPIRE_ATTACK_HALT_COVERAGE {
                msg!("WARNING: Coverage ratio {} below threshold {}, halting operations", 
                    self.coverage_ratio, VAMPIRE_ATTACK_HALT_COVERAGE);
                return Err(BettingPlatformError::CoverageRatioBelowMinimum.into());
            }
        }

        Ok(())
    }

    /// Complete the bootstrap phase
    fn complete_bootstrap(&mut self) -> Result<(), ProgramError> {
        self.bootstrap_complete = true;
        self.early_depositor_bonus_active = false;
        self.max_leverage_available = 10; // Full 10x leverage available

        let bootstrap_duration = Clock::get()?.slot - self.bootstrap_start_slot;

        msg!("Bootstrap complete! Duration: {} slots", bootstrap_duration);
        
        emit_event(EventType::BootstrapComplete, &BootstrapCompleteDetailedEvent {
            final_vault: self.vault_balance,
            total_depositors: self.unique_depositors,
            duration_slots: bootstrap_duration,
            mmt_distributed: self.total_mmt_distributed,
        });

        Ok(())
    }

    /// Get bootstrap progress info
    pub fn get_progress(&self) -> BootstrapProgress {
        BootstrapProgress {
            current_vault: self.vault_balance,
            target_vault: BOOTSTRAP_TARGET_VAULT,
            progress_percent: (self.vault_balance * 100) / BOOTSTRAP_TARGET_VAULT,
            current_milestone: self.current_milestone,
            next_milestone: if (self.current_milestone as usize) < BOOTSTRAP_MILESTONES.len() {
                Some(BOOTSTRAP_MILESTONES[self.current_milestone as usize])
            } else {
                None
            },
            depositors_count: self.unique_depositors,
            average_deposit: if self.unique_depositors > 0 {
                self.total_deposits / self.unique_depositors as u64
            } else {
                0
            },
            mmt_remaining: self.incentive_pool.saturating_sub(self.total_mmt_distributed),
            leverage_available: self.max_leverage_available,
        }
    }

    /// Calculate fee discount during bootstrap
    pub fn get_bootstrap_fee_discount(&self) -> u16 {
        if !self.bootstrap_complete {
            BOOTSTRAP_FEE_BPS // 0.28% during bootstrap vs 0.3% normal
        } else {
            30 // Normal 0.3% fee
        }
    }
}

/// Result of a bootstrap deposit
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BootstrapDepositResult {
    pub mmt_earned: u64,
    pub new_milestone_reached: bool,
    pub current_progress_percent: u64,
    pub leverage_now_available: bool,
}

/// Bootstrap progress information
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BootstrapProgress {
    pub current_vault: u64,
    pub target_vault: u64,
    pub progress_percent: u64,
    pub current_milestone: u8,
    pub next_milestone: Option<u64>,
    pub depositors_count: u32,
    pub average_deposit: u64,
    pub mmt_remaining: u64,
    pub leverage_available: u64,
}

/// Bootstrap incentive tracker
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct BootstrapIncentives {
    pub early_bird_rewards: Vec<EarlyBirdReward>,
    pub milestone_bonuses: Vec<MilestoneBonus>,
    pub referral_rewards: Vec<ReferralReward>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EarlyBirdReward {
    pub depositor: Pubkey,
    pub deposit_order: u32,
    pub bonus_mmt: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MilestoneBonus {
    pub milestone: u8,
    pub participants: Vec<Pubkey>,
    pub bonus_pool: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ReferralReward {
    pub referrer: Pubkey,
    pub referred: Pubkey,
    pub reward_mmt: u64,
}

/// Process bootstrap instructions
pub fn process_bootstrap_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_bootstrap(program_id, accounts),
        1 => process_bootstrap_deposit(program_id, accounts, &instruction_data[1..]),
        2 => process_claim_milestone_bonus(program_id, accounts),
        3 => process_referral_deposit(program_id, accounts, &instruction_data[1..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_bootstrap(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let bootstrap_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut bootstrap = BootstrapCoordinator::try_from_slice(&bootstrap_account.data.borrow())?;
    let clock = Clock::get()?;

    bootstrap.initialize(clock.slot)?;

    bootstrap.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_bootstrap_deposit(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let bootstrap_account = next_account_info(account_iter)?;
    let depositor_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;

    if !depositor_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse deposit amount
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let is_new_depositor = data[8] != 0;

    let mut bootstrap = BootstrapCoordinator::try_from_slice(&bootstrap_account.data.borrow())?;

    // Process the deposit
    let result = bootstrap.process_deposit(
        depositor_account.key,
        amount,
        is_new_depositor,
    )?;

    // In production, would handle actual token transfers and MMT minting here

    bootstrap.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;

    msg!("Bootstrap deposit processed: {} MMT earned", result.mmt_earned);

    Ok(())
}

fn process_claim_milestone_bonus(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let bootstrap_account = next_account_info(account_iter)?;
    let claimer_account = next_account_info(account_iter)?;
    let mmt_mint_account = next_account_info(account_iter)?;

    if !claimer_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let bootstrap = BootstrapCoordinator::try_from_slice(&bootstrap_account.data.borrow())?;

    // In production, would verify eligibility and distribute milestone bonuses

    msg!("Milestone bonus claimed");

    Ok(())
}

fn process_referral_deposit(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let bootstrap_account = next_account_info(account_iter)?;
    let depositor_account = next_account_info(account_iter)?;
    let referrer_account = next_account_info(account_iter)?;

    if !depositor_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse deposit amount
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());

    let mut bootstrap = BootstrapCoordinator::try_from_slice(&bootstrap_account.data.borrow())?;

    // Process deposit with referral bonus
    let result = bootstrap.process_deposit(
        depositor_account.key,
        amount,
        true, // New depositor via referral
    )?;

    // Calculate referral reward (10% of depositor's MMT)
    let referral_reward = result.mmt_earned / 10;

    emit_event(EventType::BootstrapProgress, &ReferralRewardEvent {
        referrer: *referrer_account.key,
        referred: *depositor_account.key,
        reward: referral_reward,
    });

    bootstrap.serialize(&mut &mut bootstrap_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;

/// Bootstrap participant record
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct BootstrapParticipant {
    pub pubkey: Pubkey,
    pub deposited_amount: u64,
    pub mmt_earned: u64,
    pub join_slot: u64,
    pub referrer: Option<Pubkey>,
    pub tier: u8, // Bootstrap tier (1-5 based on milestone)
    pub expected_mmt_rewards: u64,
    pub total_deposited: u64,
}

/// Bootstrap state enum
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq)]
pub enum BootstrapState {
    NotStarted,
    Active,
    Completed,
    Failed,
}