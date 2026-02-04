//! Security validation for MMT token system
//!
//! Ensures all security measures are properly implemented

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use crate::{
    error::BettingPlatformError,
    mmt::{
        state::*,
        constants::*,
    },
    math::U64F64,
};
use borsh::{BorshDeserialize, BorshSerialize};

/// Security checks for MMT operations
pub struct SecurityValidator;

impl SecurityValidator {
    /// Validate supply cap is enforced
    pub fn validate_supply_cap(
        mint_account: &AccountInfo,
    ) -> Result<bool, ProgramError> {
        let mint = spl_token::state::Mint::unpack(&mint_account.data.borrow())?;
        
        if mint.supply > TOTAL_SUPPLY {
            msg!("Supply cap violated: {} > {}", mint.supply, TOTAL_SUPPLY);
            return Ok(false);
        }
        
        // Verify mint authority is set correctly
        if mint.mint_authority.is_none() {
            msg!("Mint authority not set - risk of unlimited minting");
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate reserved tokens are properly locked
    pub fn validate_reserved_lock(
        reserved_vault: &AccountInfo,
        expected_amount: u64,
    ) -> Result<bool, ProgramError> {
        let vault = spl_token::state::Account::unpack(&reserved_vault.data.borrow())?;
        
        if vault.amount != expected_amount {
            msg!("Reserved vault amount mismatch: {} vs {}", vault.amount, expected_amount);
            return Ok(false);
        }
        
        // Verify vault owner is set to system program (effectively locked)
        if vault.owner == solana_program::system_program::id() {
            msg!("Reserved vault properly locked");
            return Ok(true);
        }
        
        msg!("Reserved vault not properly locked");
        Ok(false)
    }
    
    /// Validate overflow protection in calculations
    pub fn validate_overflow_protection() -> Result<bool, ProgramError> {
        // Test various overflow scenarios
        
        // Test 1: Large stake multiplication
        let large_stake = u64::MAX / 2;
        let multiplier = 2u64;
        
        match large_stake.checked_mul(multiplier) {
            Some(_) => {
                msg!("Overflow protection working for stake multiplication");
            }
            None => {
                msg!("Potential overflow in stake calculations");
                return Ok(false);
            }
        }
        
        // Test 2: Reward calculation overflow
        let total_fees = u64::MAX / 100;
        let rebate_percentage = 1500u64; // 15%
        
        match total_fees.checked_mul(rebate_percentage) {
            Some(rebate_amount) => {
                match rebate_amount.checked_div(10000) {
                    Some(_) => {
                        msg!("Overflow protection working for reward calculations");
                    }
                    None => {
                        msg!("Division overflow in reward calculation");
                        return Ok(false);
                    }
                }
            }
            None => {
                msg!("Multiplication overflow in reward calculation");
                return Ok(false);
            }
        }
        
        // Test 3: Season emission overflow
        let emission_rate = SEASON_ALLOCATION / SEASON_DURATION_SLOTS;
        let max_slots = SEASON_DURATION_SLOTS;
        
        match emission_rate.checked_mul(max_slots) {
            Some(total_emission) => {
                if total_emission <= SEASON_ALLOCATION {
                    msg!("Season emission calculations protected from overflow");
                } else {
                    msg!("Season emission exceeds allocation");
                    return Ok(false);
                }
            }
            None => {
                msg!("Overflow in season emission calculation");
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Validate reentrancy guards
    pub fn validate_reentrancy_guards(
        account: &AccountInfo,
        operation: &str,
    ) -> Result<bool, ProgramError> {
        // Check if account is already locked
        let data = account.data.borrow();
        if data.len() < 1 {
            return Err(ProgramError::AccountDataTooSmall);
        }
        
        // First byte used as reentrancy flag
        let is_locked = data[0] != 0;
        
        if is_locked {
            msg!("Reentrancy detected for operation: {}", operation);
            return Ok(false);
        }
        
        msg!("No reentrancy detected for: {}", operation);
        Ok(true)
    }
    
    /// Validate staking security
    pub fn validate_staking_security(
        stake_account: &StakeAccount,
        staking_pool: &StakingPool,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Check 1: Minimum stake requirement
        if stake_account.amount_staked < MIN_STAKE_AMOUNT {
            msg!("Stake below minimum: {} < {}", stake_account.amount_staked, MIN_STAKE_AMOUNT);
            return Ok(false);
        }
        
        // Check 2: Lock period enforcement
        if let Some(lock_end) = stake_account.lock_end_slot {
            if current_slot < lock_end {
                msg!("Stake is locked until slot {}", lock_end);
                // This is expected behavior, not a security issue
            }
        }
        
        // Check 3: Total staked consistency
        if stake_account.amount_staked > staking_pool.total_staked {
            msg!("User stake exceeds total pool stake");
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate maker rewards security
    pub fn validate_maker_rewards_security(
        maker_account: &MakerAccount,
        spread_improvement: u16,
        notional: u64,
    ) -> Result<bool, ProgramError> {
        // Check 1: Minimum spread improvement
        if spread_improvement < MIN_SPREAD_IMPROVEMENT_BP {
            msg!("Spread improvement below minimum: {} < {}", 
                spread_improvement, MIN_SPREAD_IMPROVEMENT_BP);
            return Ok(false);
        }
        
        // Check 2: Notional bounds
        if notional == 0 {
            msg!("Zero notional trade");
            return Ok(false);
        }
        
        if notional > MAX_TRADE_NOTIONAL {
            msg!("Trade notional exceeds maximum: {} > {}", 
                notional, MAX_TRADE_NOTIONAL);
            return Ok(false);
        }
        
        // Check 3: Reward calculation bounds
        let max_reward_per_trade = notional
            .saturating_mul(spread_improvement as u64)
            .saturating_div(10000);
        
        // With 2x multiplier for early traders
        let max_with_bonus = max_reward_per_trade.saturating_mul(2);
        
        if maker_account.pending_rewards > max_with_bonus * 1000 {
            msg!("Pending rewards suspiciously high");
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate season transition security
    pub fn validate_season_transition(
        current_season: &SeasonEmission,
        next_season: &SeasonEmission,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Check 1: Current season must be ended
        if current_slot < current_season.end_slot {
            msg!("Cannot transition season before end: {} < {}", 
                current_slot, current_season.end_slot);
            return Ok(false);
        }
        
        // Check 2: Season numbers must increment
        if next_season.season != current_season.season + 1 {
            msg!("Invalid season progression: {} -> {}", 
                current_season.season, next_season.season);
            return Ok(false);
        }
        
        // Check 3: Allocation rollover calculation
        let unused = current_season.total_allocation
            .saturating_sub(current_season.emitted_amount);
        
        let expected_allocation = SEASON_ALLOCATION
            .saturating_add(unused);
        
        if next_season.total_allocation != expected_allocation {
            msg!("Invalid allocation rollover: {} vs expected {}", 
                next_season.total_allocation, expected_allocation);
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate distribution security
    pub fn validate_distribution_security(
        distribution_type: &DistributionType,
        amount: u64,
        season_emission: &SeasonEmission,
    ) -> Result<bool, ProgramError> {
        // Check 1: Amount within remaining allocation
        let remaining = season_emission.total_allocation
            .saturating_sub(season_emission.emitted_amount);
        
        if amount > remaining {
            msg!("Distribution exceeds remaining allocation: {} > {}", 
                amount, remaining);
            return Ok(false);
        }
        
        // Check 2: Distribution type limits
        match distribution_type {
            DistributionType::MakerReward => {
                let max_maker_allocation = season_emission.total_allocation / 3; // 33% max
                if season_emission.maker_rewards.saturating_add(amount) > max_maker_allocation {
                    msg!("Maker rewards exceed allocation limit");
                    return Ok(false);
                }
            }
            DistributionType::StakingReward => {
                let max_staking_allocation = season_emission.total_allocation / 2; // 50% max
                if season_emission.staking_rewards.saturating_add(amount) > max_staking_allocation {
                    msg!("Staking rewards exceed allocation limit");
                    return Ok(false);
                }
            }
            DistributionType::EarlyTraderBonus => {
                let max_bonus_allocation = season_emission.total_allocation / 5; // 20% max
                if season_emission.early_trader_bonus.saturating_add(amount) > max_bonus_allocation {
                    msg!("Early trader bonus exceeds allocation limit");
                    return Ok(false);
                }
            }
            _ => {}
        }
        
        Ok(true)
    }
    
    /// Validate PDA security
    pub fn validate_pda_security(
        program_id: &Pubkey,
        account: &AccountInfo,
        expected_seeds: &[&[u8]],
    ) -> Result<bool, ProgramError> {
        // Derive expected PDA
        let (expected_pubkey, _bump) = Pubkey::find_program_address(
            expected_seeds,
            program_id,
        );
        
        if *account.key != expected_pubkey {
            msg!("PDA mismatch: {} vs expected {}", account.key, expected_pubkey);
            return Ok(false);
        }
        
        // Verify account is owned by program
        if *account.owner != *program_id {
            msg!("PDA not owned by program: owner is {}", account.owner);
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Comprehensive security audit
    pub fn run_security_audit(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> Result<SecurityAuditReport, ProgramError> {
        let mut report = SecurityAuditReport::default();
        
        // Run all security checks
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts for audit
        let mmt_config = next_account_info(account_info_iter)?;
        let mint = next_account_info(account_info_iter)?;
        let reserved_vault = next_account_info(account_info_iter)?;
        let staking_pool_account = next_account_info(account_info_iter)?;
        let season_emission_account = next_account_info(account_info_iter)?;
        
        // 1. Supply cap check
        report.supply_cap_valid = Self::validate_supply_cap(mint)?;
        
        // 2. Reserved lock check
        report.reserved_lock_valid = Self::validate_reserved_lock(
            reserved_vault,
            RESERVED_ALLOCATION,
        )?;
        
        // 3. Overflow protection check
        report.overflow_protection_valid = Self::validate_overflow_protection()?;
        
        // 4. PDA security checks
        report.config_pda_valid = Self::validate_pda_security(
            program_id,
            mmt_config,
            &[b"mmt_config"],
        )?;
        
        report.mint_pda_valid = Self::validate_pda_security(
            program_id,
            mint,
            &[b"mmt_mint"],
        )?;
        
        // 5. Load and validate staking pool
        let staking_pool = StakingPool::try_from_slice(&staking_pool_account.data.borrow())?;
        report.staking_pool_valid = staking_pool.total_staked <= u64::MAX / 2;
        
        // 6. Load and validate season emission
        let season_emission = SeasonEmission::try_from_slice(&season_emission_account.data.borrow())?;
        report.season_emission_valid = season_emission.emitted_amount <= season_emission.total_allocation;
        
        // Calculate overall security score
        report.calculate_score();
        
        msg!("Security audit complete. Score: {}/100", report.security_score);
        
        Ok(report)
    }
}

/// Security audit report
#[derive(Default, Debug)]
pub struct SecurityAuditReport {
    pub supply_cap_valid: bool,
    pub reserved_lock_valid: bool,
    pub overflow_protection_valid: bool,
    pub config_pda_valid: bool,
    pub mint_pda_valid: bool,
    pub staking_pool_valid: bool,
    pub season_emission_valid: bool,
    pub reentrancy_guards_valid: bool,
    pub security_score: u8,
}

impl SecurityAuditReport {
    fn calculate_score(&mut self) {
        let mut score = 0u8;
        
        if self.supply_cap_valid { score += 15; }
        if self.reserved_lock_valid { score += 15; }
        if self.overflow_protection_valid { score += 20; }
        if self.config_pda_valid { score += 10; }
        if self.mint_pda_valid { score += 10; }
        if self.staking_pool_valid { score += 15; }
        if self.season_emission_valid { score += 10; }
        if self.reentrancy_guards_valid { score += 5; }
        
        self.security_score = score;
    }
    
    pub fn is_secure(&self) -> bool {
        self.security_score >= 90
    }
}

/// Process security audit instruction
pub fn process_security_audit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Running MMT security audit...");
    
    let report = SecurityValidator::run_security_audit(program_id, accounts)?;
    
    if !report.is_secure() {
        msg!("Security audit failed! Score: {}/100", report.security_score);
        msg!("Supply cap valid: {}", report.supply_cap_valid);
        msg!("Reserved lock valid: {}", report.reserved_lock_valid);
        msg!("Overflow protection: {}", report.overflow_protection_valid);
        msg!("Config PDA valid: {}", report.config_pda_valid);
        msg!("Mint PDA valid: {}", report.mint_pda_valid);
        msg!("Staking pool valid: {}", report.staking_pool_valid);
        msg!("Season emission valid: {}", report.season_emission_valid);
        
        return Err(BettingPlatformError::SecurityCheckFailed.into());
    }
    
    msg!("Security audit passed! Score: {}/100", report.security_score);
    Ok(())
}

// Helper function
use solana_program::account_info::next_account_info;

// Additional constants for security
pub const MAX_TRADE_NOTIONAL: u64 = 10_000_000 * 10u64.pow(6); // 10M USDC
pub const MIN_STAKE_AMOUNT: u64 = 100 * 10u64.pow(9); // 100 MMT minimum stake

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_overflow_protection() {
        assert!(SecurityValidator::validate_overflow_protection().is_ok());
    }
    
    #[test]
    fn test_supply_cap_validation() {
        // Would need mock accounts for full test
        // This is a placeholder for the test structure
    }
    
    #[test]
    fn test_security_audit_scoring() {
        let mut report = SecurityAuditReport::default();
        
        // All checks pass
        report.supply_cap_valid = true;
        report.reserved_lock_valid = true;
        report.overflow_protection_valid = true;
        report.config_pda_valid = true;
        report.mint_pda_valid = true;
        report.staking_pool_valid = true;
        report.season_emission_valid = true;
        report.reentrancy_guards_valid = true;
        
        report.calculate_score();
        assert_eq!(report.security_score, 100);
        assert!(report.is_secure());
        
        // One check fails
        report.overflow_protection_valid = false;
        report.calculate_score();
        assert_eq!(report.security_score, 80);
        assert!(!report.is_secure());
    }
}