//! Enhanced Bootstrap Phase Implementation
//! 
//! Implements the complete bootstrap phase with:
//! - $0 vault initialization
//! - MMT rewards (20% of first season for early LPs)
//! - Coverage formula: coverage = vault / (0.5 * OI)
//! - $10k minimum viable vault
//! - Vampire attack protection (halt if coverage < 0.5)

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
    math::fixed_point::U64F64,
};

/// Bootstrap constants
pub const MINIMUM_VIABLE_VAULT: u64 = 100_000_000_000; // $100k minimum
pub const BOOTSTRAP_MMT_ALLOCATION: u64 = 2_000_000_000_000; // 2M MMT (20% of 10M season)
pub const COVERAGE_HALT_THRESHOLD: u64 = 5000; // 0.5 coverage (50%)
pub const VAMPIRE_ATTACK_WITHDRAWAL_LIMIT: u64 = 2000; // 20% max withdrawal
pub const BOOTSTRAP_EPOCH: u8 = 1;

/// Enhanced bootstrap coordinator
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default)]
pub struct EnhancedBootstrapCoordinator {
    pub authority: Pubkey,
    pub vault_balance: u64,
    pub total_open_interest: u64,
    pub coverage_ratio: u64, // In basis points (10000 = 1.0)
    pub bootstrap_start_slot: u64,
    pub bootstrap_complete: bool,
    pub is_halted: bool,
    pub halt_reason: BootstrapHaltReason,
    
    // MMT distribution
    pub mmt_pool_remaining: u64,
    pub mmt_distributed: u64,
    pub early_lp_addresses: Vec<Pubkey>,
    pub lp_shares: Vec<u64>, // Proportional shares for MMT
    
    // Protection metrics
    pub last_coverage_check: u64,
    pub consecutive_low_coverage: u8,
    pub withdrawal_count_current_window: u8,
    pub withdrawal_window_start: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum BootstrapHaltReason {
    None,
    LowCoverage,
    VampireAttack,
    ManualHalt,
}

impl Default for BootstrapHaltReason {
    fn default() -> Self {
        BootstrapHaltReason::None
    }
}

impl EnhancedBootstrapCoordinator {
    pub const SIZE: usize = 32 + // authority
        8 +   // vault_balance
        8 +   // total_open_interest
        8 +   // coverage_ratio
        8 +   // bootstrap_start_slot
        1 +   // bootstrap_complete
        1 +   // is_halted
        1 +   // halt_reason
        8 +   // mmt_pool_remaining
        8 +   // mmt_distributed
        4 + (32 * 100) + // early_lp_addresses (max 100)
        4 + (8 * 100) +  // lp_shares (max 100)
        8 +   // last_coverage_check
        1 +   // consecutive_low_coverage
        1 +   // withdrawal_count_current_window
        8;    // withdrawal_window_start

    /// Initialize bootstrap with $0 vault
    pub fn initialize(&mut self, authority: &Pubkey, current_slot: u64) -> ProgramResult {
        self.authority = *authority;
        self.vault = 0; // Start with $0
        self.total_oi = 0;
        self.coverage_ratio = 0; // 0 coverage at start
        self.bootstrap_start_slot = current_slot;
        self.bootstrap_complete = false;
        self.is_halted = false;
        self.halt_reason = BootstrapHaltReason::None;
        self.mmt_pool_remaining = BOOTSTRAP_MMT_ALLOCATION;
        self.mmt_distributed = 0;
        self.early_lp_addresses = Vec::new();
        self.lp_shares = Vec::new();
        self.last_coverage_check = current_slot;
        self.consecutive_low_coverage = 0;
        self.withdrawal_count_current_window = 0;
        self.withdrawal_window_start = current_slot;

        msg!("Bootstrap initialized with $0 vault, 2M MMT allocation ready");
        Ok(())
    }

    /// Process deposit and distribute MMT rewards immediately
    pub fn process_deposit(
        &mut self,
        depositor: &Pubkey,
        amount: u64,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        // Update vault balance
        self.vault = self.vault
            .checked_add(amount)
            .ok_or(BettingPlatformError::MathOverflow)?;

        // Calculate MMT reward based on deposit proportion
        let mmt_reward = self.calculate_mmt_reward(amount)?;

        // Track early LP
        if !self.early_lp_addresses.contains(depositor) {
            self.early_lp_addresses.push(*depositor);
            self.lp_shares.push(amount);
        } else {
            // Update existing LP share
            if let Some(pos) = self.early_lp_addresses.iter().position(|&x| x == *depositor) {
                self.lp_shares[pos] = self.lp_shares[pos]
                    .checked_add(amount)
                    .ok_or(BettingPlatformError::MathOverflow)?;
            }
        }

        // Distribute MMT immediately
        self.mmt_distributed = self.mmt_distributed
            .checked_add(mmt_reward)
            .ok_or(BettingPlatformError::MathOverflow)?;
        self.mmt_pool_remaining = self.mmt_pool_remaining
            .checked_sub(mmt_reward)
            .ok_or(BettingPlatformError::InsufficientFunds)?;

        // Update coverage ratio
        self.update_coverage_ratio()?;

        // Check if we've reached minimum viable vault
        if !self.bootstrap_complete && self.vault >= MINIMUM_VIABLE_VAULT {
            self.bootstrap_complete = true;
            msg!("Bootstrap complete! Minimum viable vault of $100k reached");
        }

        msg!("Deposit processed: {} USDC, {} MMT rewarded", amount, mmt_reward);
        Ok(mmt_reward)
    }

    /// Calculate MMT reward for deposit
    fn calculate_mmt_reward(&self, deposit_amount: u64) -> Result<u64, ProgramError> {
        if self.mmt_pool_remaining == 0 {
            return Ok(0);
        }

        // Early deposits get proportionally more MMT
        // Formula: reward = (deposit / 100k) * remaining_pool * early_bonus
        let deposit_ratio = U64F64::from_num(deposit_amount) / U64F64::from_num(MINIMUM_VIABLE_VAULT);
        let pool_ratio = deposit_ratio * U64F64::from_fraction(1, 10).unwrap(); // 10% of remaining per $100k
        
        // Early bonus: 2x for first $1k, 1.5x for next $4k, 1x after
        let early_bonus = if self.vault < 1_000_000_000 {
            U64F64::from_num(2)
        } else if self.vault < 5_000_000_000 {
            U64F64::from_fraction(3, 2).unwrap()
        } else {
            U64F64::from_num(1)
        };

        let reward = U64F64::from_num(self.mmt_pool_remaining) * pool_ratio * early_bonus;
        let reward_u64 = reward.to_num();

        // Cap at remaining pool
        Ok(reward_u64.min(self.mmt_pool_remaining))
    }

    /// Update coverage ratio: coverage = vault / (0.5 * OI)
    pub fn update_coverage_ratio(&mut self) -> ProgramResult {
        if self.total_oi == 0 {
            self.coverage_ratio = 0;
            return Ok(());
        }

        // coverage = vault / (0.5 * OI)
        // Multiply by 10000 for basis points
        let numerator = self.vault
            .checked_mul(10000)
            .ok_or(BettingPlatformError::MathOverflow)?;
        let denominator = self.total_oi / 2; // 0.5 * OI

        self.coverage_ratio = numerator
            .checked_div(denominator)
            .unwrap_or(0);

        Ok(())
    }

    /// Calculate maximum leverage based on coverage
    pub fn calculate_max_leverage(&self) -> u64 {
        if self.vault < 1_000_000_000 { // Less than $1k
            return 0; // No leverage
        }

        // Linear scaling from 1x to 10x based on vault size
        let leverage = (self.vault / 1_000_000_000).min(10);
        
        // Further limit by coverage if OI exists
        if self.coverage_ratio > 0 && self.coverage_ratio < 10000 {
            let coverage_limited = (self.coverage_ratio * leverage) / 10000;
            leverage.min(coverage_limited)
        } else {
            leverage
        }
    }

    /// Check for vampire attack conditions
    pub fn check_vampire_attack(
        &mut self,
        withdrawal_amount: u64,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Reset withdrawal window if needed (60 seconds = 150 slots)
        if current_slot > self.withdrawal_window_start + 150 {
            self.withdrawal_count_current_window = 0;
            self.withdrawal_window_start = current_slot;
        }

        // Check 1: Would withdrawal drop coverage below 0.5?
        let new_vault_balance = self.vault
            .checked_sub(withdrawal_amount)
            .ok_or(BettingPlatformError::InsufficientFunds)?;

        if self.total_oi > 0 {
            let new_coverage = (new_vault_balance * 10000) / (self.total_oi / 2);
            if new_coverage < COVERAGE_HALT_THRESHOLD {
                self.is_halted = true;
                self.halt_reason = BootstrapHaltReason::LowCoverage;
                msg!("Vampire attack detected: coverage would drop below 0.5");
                return Ok(true);
            }
        }

        // Check 2: Is withdrawal > 20% of vault?
        let withdrawal_percentage = (withdrawal_amount * 10000) / self.vault;
        if withdrawal_percentage > VAMPIRE_ATTACK_WITHDRAWAL_LIMIT {
            msg!("Large withdrawal detected: {}% of vault", withdrawal_percentage / 100);
            return Ok(true);
        }

        // Check 3: Too many withdrawals in window?
        self.withdrawal_count_current_window += 1;
        if self.withdrawal_count_current_window > 3 {
            self.is_halted = true;
            self.halt_reason = BootstrapHaltReason::VampireAttack;
            msg!("Rapid withdrawals detected: vampire attack protection triggered");
            return Ok(true);
        }

        Ok(false)
    }

    /// Process withdrawal with vampire attack checks
    pub fn process_withdrawal(
        &mut self,
        amount: u64,
        current_slot: u64,
    ) -> ProgramResult {
        // Check for vampire attack
        if self.check_vampire_attack(amount, current_slot)? {
            return Err(BettingPlatformError::VampireAttackDetected.into());
        }

        // Process withdrawal
        self.vault = self.vault
            .checked_sub(amount)
            .ok_or(BettingPlatformError::InsufficientFunds)?;

        // Update coverage
        self.update_coverage_ratio()?;

        // Check if coverage dropped dangerously low
        if self.coverage_ratio < COVERAGE_HALT_THRESHOLD && self.total_oi > 0 {
            self.consecutive_low_coverage += 1;
            if self.consecutive_low_coverage >= 3 {
                self.is_halted = true;
                self.halt_reason = BootstrapHaltReason::LowCoverage;
                msg!("System halted: coverage below 0.5 for 3 consecutive checks");
            }
        } else {
            self.consecutive_low_coverage = 0;
        }

        Ok(())
    }

    /// Get bootstrap phase status for UI
    pub fn get_status(&self) -> BootstrapStatus {
        BootstrapStatus {
            vault_balance: self.vault,
            coverage_ratio: self.coverage_ratio,
            is_complete: self.bootstrap_complete,
            is_halted: self.is_halted,
            progress_percentage: (self.vault * 100) / MINIMUM_VIABLE_VAULT,
            max_leverage: self.calculate_max_leverage(),
            mmt_remaining: self.mmt_pool_remaining,
            early_lp_count: self.early_lp_addresses.len() as u32,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct BootstrapStatus {
    pub vault_balance: u64,
    pub coverage_ratio: u64,
    pub is_complete: bool,
    pub is_halted: bool,
    pub progress_percentage: u64,
    pub max_leverage: u64,
    pub mmt_remaining: u64,
    pub early_lp_count: u32,
}

/// Money-making calculations for bootstrap phase
impl EnhancedBootstrapCoordinator {
    /// Calculate APY for early LPs
    pub fn calculate_early_lp_apy(&self) -> f64 {
        // Base APY from fees: 12%
        let base_apy = 0.12;
        
        // MMT bonus (assuming MMT appreciates 2x)
        let mmt_bonus = if self.vault > 0 {
            (self.mmt_distributed as f64 * 2.0) / self.vault as f64
        } else {
            0.0
        };

        base_apy + mmt_bonus
    }

    /// Get optimal deposit timing
    pub fn get_deposit_timing_edge(&self) -> &'static str {
        match self.vault {
            0..=999_999_999 => "Maximum edge: 2x MMT multiplier + first mover advantage",
            1_000_000_000..=4_999_999_999 => "High edge: 1.5x MMT multiplier",
            5_000_000_000..=9_999_999_999 => "Good edge: Standard MMT + leverage unlock soon",
            _ => "Normal returns: Bootstrap complete, standard APY",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_vault_initialization() {
        let mut bootstrap = EnhancedBootstrapCoordinator::default();
        bootstrap.initialize(&Pubkey::new_unique(), 1000).unwrap();

        assert_eq!(bootstrap.vault, 0);
        assert_eq!(bootstrap.coverage_ratio, 0);
        assert_eq!(bootstrap.mmt_pool_remaining, BOOTSTRAP_MMT_ALLOCATION);
        assert!(!bootstrap.bootstrap_complete);
    }

    #[test]
    fn test_coverage_calculation() {
        let mut bootstrap = EnhancedBootstrapCoordinator::default();
        bootstrap.vault = 5_000_000_000; // $5k
        bootstrap.total_oi = 10_000_000_000; // $10k OI

        bootstrap.update_coverage_ratio().unwrap();
        
        // coverage = 5k / (0.5 * 10k) = 5k / 5k = 1.0 = 10000 bps
        assert_eq!(bootstrap.coverage_ratio, 10000);
    }

    #[test]
    fn test_vampire_attack_detection() {
        let mut bootstrap = EnhancedBootstrapCoordinator::default();
        bootstrap.vault = 10_000_000_000; // $10k
        bootstrap.total_oi = 15_000_000_000; // $15k OI
        bootstrap.update_coverage_ratio().unwrap();

        // Try to withdraw $6k (would drop coverage below 0.5)
        let is_attack = bootstrap.check_vampire_attack(6_000_000_000, 1000).unwrap();
        assert!(is_attack);
        assert!(bootstrap.is_halted);
        assert_eq!(bootstrap.halt_reason, BootstrapHaltReason::LowCoverage);
    }

    #[test]
    fn test_mmt_distribution() {
        let mut bootstrap = EnhancedBootstrapCoordinator::default();
        bootstrap.initialize(&Pubkey::new_unique(), 1000).unwrap();

        let depositor = Pubkey::new_unique();
        let mmt_reward = bootstrap.process_deposit(&depositor, 1_000_000_000, 1000).unwrap();

        assert!(mmt_reward > 0);
        assert_eq!(bootstrap.mmt_distributed, mmt_reward);
        assert_eq!(bootstrap.early_lp_addresses.len(), 1);
    }
}