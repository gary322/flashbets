//! CDP Vault Management
//!
//! Manages collateral pools and vault operations

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
    constants::*,
};

use super::state::CollateralType;

/// Vault discriminator
pub const VAULT_DISCRIMINATOR: [u8; 8] = [86, 65, 85, 76, 84, 67, 68, 80]; // "VAULTCDP"

/// Vault state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum VaultState {
    Active,
    Paused,
    EmergencyShutdown,
    Migrating,
}

/// CDP Vault - Main collateral pool
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CDPVault {
    /// Discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Vault ID
    pub vault_id: u128,
    
    /// Vault state
    pub state: VaultState,
    
    /// Total deposited collateral
    pub total_collateral: u128,
    
    /// Total borrowed from vault
    pub total_borrowed: u128,
    
    /// Available liquidity
    pub available_liquidity: u128,
    
    /// Reserve factor (percentage kept as reserve)
    pub reserve_factor: f64,
    
    /// Vault reserves
    pub reserves: u128,
    
    /// Utilization rate
    pub utilization_rate: f64,
    
    /// Supply APY
    pub supply_apy: f64,
    
    /// Borrow APY
    pub borrow_apy: f64,
    
    /// Total shares issued
    pub total_shares: u128,
    
    /// Share price
    pub share_price: f64,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Authority
    pub authority: Pubkey,
    
    /// Stats
    pub stats: VaultStats,
}

/// Vault statistics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VaultStats {
    /// Total deposits
    pub total_deposits: u128,
    
    /// Total withdrawals
    pub total_withdrawals: u128,
    
    /// Total borrows
    pub total_borrows: u128,
    
    /// Total repayments
    pub total_repayments: u128,
    
    /// Total interest earned
    pub total_interest_earned: u128,
    
    /// Total fees collected
    pub total_fees_collected: u128,
    
    /// Number of depositors
    pub depositor_count: u64,
    
    /// Number of borrowers
    pub borrower_count: u64,
}

impl VaultStats {
    pub fn new() -> Self {
        Self {
            total_deposits: 0,
            total_withdrawals: 0,
            total_borrows: 0,
            total_repayments: 0,
            total_interest_earned: 0,
            total_fees_collected: 0,
            depositor_count: 0,
            borrower_count: 0,
        }
    }
}

impl CDPVault {
    pub fn new(vault_id: u128, authority: Pubkey) -> Self {
        Self {
            discriminator: VAULT_DISCRIMINATOR,
            vault_id,
            state: VaultState::Active,
            total_collateral: 0,
            total_borrowed: 0,
            available_liquidity: 0,
            reserve_factor: 0.1, // 10% reserves
            reserves: 0,
            utilization_rate: 0.0,
            supply_apy: 0.02, // 2% base
            borrow_apy: 0.05, // 5% base
            total_shares: 0,
            share_price: 1.0,
            last_update: 0,
            created_at: 0,
            authority,
            stats: VaultStats::new(),
        }
    }
    
    /// Validate vault state
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != VAULT_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        match self.state {
            VaultState::EmergencyShutdown => {
                msg!("Vault is in emergency shutdown");
                return Err(BettingPlatformError::EmergencyPause.into());
            }
            VaultState::Paused => {
                msg!("Vault is paused");
                return Err(BettingPlatformError::VaultPaused.into());
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Deposit collateral
    pub fn deposit(&mut self, amount: u128) -> Result<u128, ProgramError> {
        self.validate()?;
        
        // Calculate shares to mint
        let shares = if self.total_shares == 0 {
            amount // First deposit, 1:1 ratio
        } else {
            ((amount as f64) / self.share_price) as u128
        };
        
        // Update vault state
        self.total_collateral = self.total_collateral
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.available_liquidity = self.available_liquidity
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.total_shares = self.total_shares
            .checked_add(shares)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update stats
        self.stats.total_deposits = self.stats.total_deposits
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.update_metrics()?;
        
        msg!("Deposited {} collateral for {} shares", amount, shares);
        
        Ok(shares)
    }
    
    /// Withdraw collateral
    pub fn withdraw(&mut self, shares: u128) -> Result<u128, ProgramError> {
        self.validate()?;
        
        if shares > self.total_shares {
            return Err(BettingPlatformError::InsufficientShares.into());
        }
        
        // Calculate collateral to return
        let collateral = ((shares as f64) * self.share_price) as u128;
        
        if collateral > self.available_liquidity {
            return Err(BettingPlatformError::InsufficientLiquidity.into());
        }
        
        // Update vault state
        self.total_collateral = self.total_collateral
            .checked_sub(collateral)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.available_liquidity = self.available_liquidity
            .checked_sub(collateral)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.total_shares = self.total_shares
            .checked_sub(shares)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update stats
        self.stats.total_withdrawals = self.stats.total_withdrawals
            .checked_add(collateral)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.update_metrics()?;
        
        msg!("Withdrew {} collateral for {} shares", collateral, shares);
        
        Ok(collateral)
    }
    
    /// Borrow from vault
    pub fn borrow(&mut self, amount: u128) -> Result<(), ProgramError> {
        self.validate()?;
        
        if amount > self.available_liquidity {
            return Err(BettingPlatformError::InsufficientLiquidity.into());
        }
        
        // Update state
        self.total_borrowed = self.total_borrowed
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.available_liquidity = self.available_liquidity
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update stats
        self.stats.total_borrows = self.stats.total_borrows
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.update_metrics()?;
        
        Ok(())
    }
    
    /// Repay to vault
    pub fn repay(&mut self, amount: u128, interest: u128) -> Result<(), ProgramError> {
        // Update state
        self.total_borrowed = self.total_borrowed
            .checked_sub(amount.min(self.total_borrowed))
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        let total_repaid = amount.checked_add(interest)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.available_liquidity = self.available_liquidity
            .checked_add(total_repaid)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Add interest to reserves
        let reserve_amount = ((interest as f64) * self.reserve_factor) as u128;
        self.reserves = self.reserves
            .checked_add(reserve_amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update stats
        self.stats.total_repayments = self.stats.total_repayments
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.stats.total_interest_earned = self.stats.total_interest_earned
            .checked_add(interest)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.update_metrics()?;
        
        Ok(())
    }
    
    /// Update vault metrics
    pub fn update_metrics(&mut self) -> Result<(), ProgramError> {
        // Calculate utilization
        if self.total_collateral > 0 {
            self.utilization_rate = (self.total_borrowed as f64) / (self.total_collateral as f64);
        } else {
            self.utilization_rate = 0.0;
        }
        
        // Update APYs based on utilization
        self.update_interest_rates();
        
        // Update share price
        if self.total_shares > 0 {
            self.share_price = (self.total_collateral as f64) / (self.total_shares as f64);
        } else {
            self.share_price = 1.0;
        }
        
        self.last_update = Clock::get()?.unix_timestamp;
        
        Ok(())
    }
    
    /// Update interest rates based on utilization
    fn update_interest_rates(&mut self) {
        // Kink model for interest rates
        let kink = 0.8;
        let base_borrow = 0.02;
        let slope1 = 0.1;
        let slope2 = 0.5;
        
        if self.utilization_rate <= kink {
            self.borrow_apy = base_borrow + slope1 * self.utilization_rate;
        } else {
            self.borrow_apy = base_borrow + slope1 * kink + 
                              slope2 * (self.utilization_rate - kink);
        }
        
        // Supply APY = Borrow APY * Utilization * (1 - Reserve Factor)
        self.supply_apy = self.borrow_apy * self.utilization_rate * (1.0 - self.reserve_factor);
    }
    
    /// Get vault health score (0-100)
    pub fn get_health_score(&self) -> u8 {
        let mut score = 100u8;
        
        // Deduct for high utilization
        if self.utilization_rate > 0.9 {
            score = score.saturating_sub(30);
        } else if self.utilization_rate > 0.8 {
            score = score.saturating_sub(10);
        }
        
        // Deduct for low liquidity
        let liquidity_ratio = if self.total_collateral > 0 {
            (self.available_liquidity as f64) / (self.total_collateral as f64)
        } else {
            1.0
        };
        
        if liquidity_ratio < 0.1 {
            score = score.saturating_sub(20);
        } else if liquidity_ratio < 0.2 {
            score = score.saturating_sub(10);
        }
        
        score
    }
}

/// Collateral pool for specific asset type
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CollateralPool {
    /// Pool ID
    pub pool_id: u128,
    
    /// Collateral type
    pub collateral_type: CollateralType,
    
    /// Collateral mint
    pub collateral_mint: Pubkey,
    
    /// Total deposited
    pub total_deposited: u128,
    
    /// Total borrowed
    pub total_borrowed: u128,
    
    /// Interest rate model
    pub interest_model: InterestModel,
    
    /// Pool caps
    pub deposit_cap: u128,
    
    /// Borrow cap
    pub borrow_cap: u128,
    
    /// Is active
    pub is_active: bool,
}

/// Interest rate model
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct InterestModel {
    /// Base rate
    pub base_rate: f64,
    
    /// Utilization kink point
    pub kink: f64,
    
    /// Slope before kink
    pub slope1: f64,
    
    /// Slope after kink
    pub slope2: f64,
    
    /// Max rate
    pub max_rate: f64,
}

impl InterestModel {
    pub fn calculate_rate(&self, utilization: f64) -> f64 {
        let rate = if utilization <= self.kink {
            self.base_rate + self.slope1 * utilization
        } else {
            self.base_rate + self.slope1 * self.kink + 
            self.slope2 * (utilization - self.kink)
        };
        
        rate.min(self.max_rate)
    }
}

/// Calculate vault health
pub fn calculate_vault_health(vault: &CDPVault) -> u8 {
    vault.get_health_score()
}

/// Execute vault deposit
pub fn execute_vault_deposit(
    vault: &mut CDPVault,
    depositor: &Pubkey,
    amount: u128,
) -> Result<u128, ProgramError> {
    let shares = vault.deposit(amount)?;
    
    // Would update depositor's share balance here
    vault.stats.depositor_count += 1;
    
    msg!("Depositor {} deposited {} for {} shares", 
         depositor, amount, shares);
    
    Ok(shares)
}

/// Execute vault withdrawal
pub fn execute_vault_withdraw(
    vault: &mut CDPVault,
    withdrawer: &Pubkey,
    shares: u128,
) -> Result<u128, ProgramError> {
    let collateral = vault.withdraw(shares)?;
    
    msg!("Withdrawer {} withdrew {} collateral for {} shares", 
         withdrawer, collateral, shares);
    
    Ok(collateral)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vault_deposit_withdraw() {
        let mut vault = CDPVault::new(1, Pubkey::default());
        
        // First deposit
        let shares1 = vault.deposit(1000).unwrap();
        assert_eq!(shares1, 1000); // 1:1 initially
        assert_eq!(vault.total_collateral, 1000);
        assert_eq!(vault.available_liquidity, 1000);
        
        // Second deposit
        let shares2 = vault.deposit(500).unwrap();
        assert_eq!(vault.total_collateral, 1500);
        
        // Withdraw
        let collateral = vault.withdraw(shares1).unwrap();
        assert_eq!(collateral, 1000);
        assert_eq!(vault.total_collateral, 500);
    }
    
    #[test]
    fn test_vault_borrowing() {
        let mut vault = CDPVault::new(1, Pubkey::default());
        vault.deposit(10000).unwrap();
        
        // Borrow
        assert!(vault.borrow(5000).is_ok());
        assert_eq!(vault.total_borrowed, 5000);
        assert_eq!(vault.available_liquidity, 5000);
        assert_eq!(vault.utilization_rate, 0.5);
        
        // Repay with interest
        assert!(vault.repay(5000, 100).is_ok());
        assert_eq!(vault.total_borrowed, 0);
        assert_eq!(vault.available_liquidity, 10100);
        assert_eq!(vault.stats.total_interest_earned, 100);
    }
    
    #[test]
    fn test_interest_model() {
        let model = InterestModel {
            base_rate: 0.02,
            kink: 0.8,
            slope1: 0.1,
            slope2: 0.5,
            max_rate: 0.5,
        };
        
        // Low utilization
        assert!(model.calculate_rate(0.3) < 0.1);
        
        // At kink
        let rate_at_kink = model.calculate_rate(0.8);
        assert!(rate_at_kink > 0.1 && rate_at_kink < 0.2);
        
        // Above kink
        assert!(model.calculate_rate(0.9) > 0.2);
    }
}