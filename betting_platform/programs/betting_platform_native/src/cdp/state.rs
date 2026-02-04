//! CDP State Management
//!
//! Core state structures for Collateralized Debt Positions

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
    constants::*,
};

/// CDP account seed
pub const CDP_ACCOUNT_SEED: &[u8] = b"cdp_account";
pub const CDP_VAULT_SEED: &[u8] = b"cdp_vault";

/// CDP account discriminator
pub const CDP_DISCRIMINATOR: [u8; 8] = [67, 68, 80, 65, 67, 67, 78, 84]; // "CDPACCNT"

/// CDP Status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum CDPStatus {
    Active,
    UnderCollateralized,
    Liquidating,
    Liquidated,
    Closed,
    Frozen,
}

/// Collateral types accepted
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum CollateralType {
    USDC,
    SOL,
    BTC,
    ETH,
    SyntheticToken,
}

impl CollateralType {
    /// Get collateral ratio for type
    pub fn get_collateral_ratio(&self) -> f64 {
        match self {
            CollateralType::USDC => 1.5,
            CollateralType::SOL => 2.0,
            CollateralType::BTC => 1.8,
            CollateralType::ETH => 1.8,
            CollateralType::SyntheticToken => 2.5,
        }
    }
    
    /// Get liquidation ratio
    pub fn get_liquidation_ratio(&self) -> f64 {
        match self {
            CollateralType::USDC => 1.2,
            CollateralType::SOL => 1.5,
            CollateralType::BTC => 1.4,
            CollateralType::ETH => 1.4,
            CollateralType::SyntheticToken => 1.8,
        }
    }
    
    /// Get max LTV (Loan-to-Value)
    pub fn get_max_ltv(&self) -> f64 {
        match self {
            CollateralType::USDC => 0.67, // 1/1.5
            CollateralType::SOL => 0.50,  // 1/2.0
            CollateralType::BTC => 0.56,  // 1/1.8
            CollateralType::ETH => 0.56,  // 1/1.8
            CollateralType::SyntheticToken => 0.40, // 1/2.5
        }
    }
}

/// CDP Account - Main state for a user's CDP
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CDPAccount {
    /// Discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Owner of the CDP
    pub owner: Pubkey,
    
    /// CDP ID (unique)
    pub cdp_id: u128,
    
    /// Market ID this CDP is for
    pub market_id: u128,
    
    /// Status of the CDP
    pub status: CDPStatus,
    
    /// Collateral deposited
    pub collateral_amount: u128,
    
    /// Collateral type
    pub collateral_type: CollateralType,
    
    /// Collateral mint
    pub collateral_mint: Pubkey,
    
    /// Debt amount (synthetic tokens borrowed)
    pub debt_amount: u128,
    
    /// Synthetic token mint
    pub synthetic_mint: Pubkey,
    
    /// Oracle account for price feeds
    pub oracle_account: Pubkey,
    
    /// Last oracle price
    pub last_oracle_price: f64,
    
    /// Collateralization ratio
    pub collateral_ratio: f64,
    
    /// Interest rate (annual)
    pub interest_rate: f64,
    
    /// Accrued interest
    pub accrued_interest: u128,
    
    /// Last interest update
    pub last_interest_update: i64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last action timestamp
    pub last_action: i64,
    
    /// Liquidation price
    pub liquidation_price: f64,
    
    /// Health factor (1.0 = at liquidation threshold)
    pub health_factor: f64,
    
    /// Is emergency stopped
    pub emergency_stopped: bool,
    
    /// Leverage multiplier
    pub leverage: u16,
    
    /// Max leverage allowed
    pub max_leverage: u16,
    
    /// Fixed collateral cap (coll_cap = 2.0)
    pub coll_cap: f64,
}

impl CDPAccount {
    pub fn new(
        owner: Pubkey,
        cdp_id: u128,
        market_id: u128,
        collateral_type: CollateralType,
        collateral_mint: Pubkey,
        synthetic_mint: Pubkey,
        oracle_account: Pubkey,
    ) -> Self {
        Self {
            discriminator: CDP_DISCRIMINATOR,
            owner,
            cdp_id,
            market_id,
            status: CDPStatus::Active,
            collateral_amount: 0,
            collateral_type,
            collateral_mint,
            debt_amount: 0,
            synthetic_mint,
            oracle_account,
            last_oracle_price: 0.0,
            collateral_ratio: 0.0,
            interest_rate: 0.05, // 5% default
            accrued_interest: 0,
            last_interest_update: 0,
            created_at: 0,
            last_action: 0,
            liquidation_price: 0.0,
            health_factor: f64::MAX,
            emergency_stopped: false,
            leverage: 1,
            max_leverage: MAX_FUSED_LEVERAGE as u16,
            coll_cap: 2.0, // Fixed as per requirements
        }
    }
    
    /// Validate CDP state
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != CDP_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.emergency_stopped {
            msg!("CDP is emergency stopped");
            return Err(BettingPlatformError::EmergencyPause.into());
        }
        
        match self.status {
            CDPStatus::Liquidated => {
                msg!("CDP is liquidated");
                return Err(BettingPlatformError::PositionLiquidated.into());
            }
            CDPStatus::Closed => {
                msg!("CDP is closed");
                return Err(BettingPlatformError::PositionClosed.into());
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Calculate health factor
    pub fn calculate_health_factor(&mut self, oracle_price: f64) -> f64 {
        if self.debt_amount == 0 {
            self.health_factor = f64::MAX;
            return self.health_factor;
        }
        
        // Collateral value in USD
        let collateral_value = (self.collateral_amount as f64) * oracle_price;
        
        // Get liquidation ratio for collateral type
        let liquidation_ratio = self.collateral_type.get_liquidation_ratio();
        
        // Health = (Collateral * Oracle Price) / (Debt * Liquidation Ratio)
        self.health_factor = collateral_value / (self.debt_amount as f64 * liquidation_ratio);
        
        // Update liquidation price
        self.liquidation_price = (self.debt_amount as f64 * liquidation_ratio) / self.collateral_amount as f64;
        
        self.health_factor
    }
    
    /// Check if CDP can borrow
    pub fn can_borrow(&self, borrow_amount: u128, oracle_price: f64) -> Result<(), ProgramError> {
        self.validate()?;
        
        if self.collateral_amount == 0 {
            msg!("No collateral deposited");
            return Err(BettingPlatformError::InsufficientCollateral.into());
        }
        
        // Calculate max borrow based on collateral
        let collateral_value = (self.collateral_amount as f64) * oracle_price;
        let max_ltv = self.collateral_type.get_max_ltv();
        let max_borrow = (collateral_value * max_ltv) as u128;
        
        // Check if new debt would exceed max
        let new_debt = self.debt_amount
            .checked_add(borrow_amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        if new_debt > max_borrow {
            msg!("Borrow would exceed max LTV");
            return Err(BettingPlatformError::ExceedsMaxLTV.into());
        }
        
        // Check leverage limits
        let effective_leverage = (new_debt / self.collateral_amount) as u16;
        if effective_leverage > self.max_leverage {
            msg!("Would exceed max leverage");
            return Err(BettingPlatformError::ExceedsMaxLeverage.into());
        }
        
        Ok(())
    }
    
    /// Deposit collateral
    pub fn deposit_collateral(&mut self, amount: u128) -> Result<(), ProgramError> {
        self.validate()?;
        
        self.collateral_amount = self.collateral_amount
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Recalculate ratios
        if self.debt_amount > 0 {
            self.collateral_ratio = self.collateral_amount as f64 / self.debt_amount as f64;
        }
        
        Ok(())
    }
    
    /// Withdraw collateral
    pub fn withdraw_collateral(
        &mut self, 
        amount: u128,
        oracle_price: f64,
    ) -> Result<(), ProgramError> {
        self.validate()?;
        
        if amount > self.collateral_amount {
            return Err(BettingPlatformError::InsufficientCollateral.into());
        }
        
        let new_collateral = self.collateral_amount
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Check if withdrawal maintains minimum collateral ratio
        if self.debt_amount > 0 {
            let new_collateral_value = (new_collateral as f64) * oracle_price;
            let required_ratio = self.collateral_type.get_collateral_ratio();
            let required_collateral = (self.debt_amount as f64) * required_ratio;
            
            if new_collateral_value < required_collateral {
                msg!("Withdrawal would make CDP undercollateralized");
                return Err(BettingPlatformError::WouldBeUndercollateralized.into());
            }
        }
        
        self.collateral_amount = new_collateral;
        
        // Recalculate ratios
        if self.debt_amount > 0 {
            self.collateral_ratio = self.collateral_amount as f64 / self.debt_amount as f64;
        }
        
        Ok(())
    }
    
    /// Borrow synthetic tokens
    pub fn borrow(&mut self, amount: u128, oracle_price: f64) -> Result<(), ProgramError> {
        self.can_borrow(amount, oracle_price)?;
        
        self.debt_amount = self.debt_amount
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update leverage
        self.leverage = ((self.debt_amount / self.collateral_amount.max(1)) as u16).max(1);
        
        // Recalculate ratios
        self.collateral_ratio = self.collateral_amount as f64 / self.debt_amount as f64;
        self.calculate_health_factor(oracle_price);
        
        Ok(())
    }
    
    /// Repay debt
    pub fn repay(&mut self, amount: u128) -> Result<(), ProgramError> {
        if amount > self.debt_amount {
            msg!("Repay amount exceeds debt");
            return Err(BettingPlatformError::ExceedsDebt.into());
        }
        
        self.debt_amount = self.debt_amount
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update leverage
        if self.collateral_amount > 0 {
            self.leverage = ((self.debt_amount / self.collateral_amount) as u16).max(1);
        }
        
        // Recalculate ratios
        if self.debt_amount > 0 {
            self.collateral_ratio = self.collateral_amount as f64 / self.debt_amount as f64;
        } else {
            self.collateral_ratio = f64::MAX;
            self.health_factor = f64::MAX;
        }
        
        // Check if fully repaid
        if self.debt_amount == 0 && self.accrued_interest == 0 {
            self.status = CDPStatus::Closed;
        }
        
        Ok(())
    }
    
    /// Check if should liquidate
    pub fn should_liquidate(&self) -> bool {
        self.health_factor < 1.0 || self.status == CDPStatus::UnderCollateralized
    }
    
    /// Execute liquidation
    pub fn liquidate(&mut self) -> Result<(), ProgramError> {
        if !self.should_liquidate() {
            msg!("CDP not eligible for liquidation");
            return Err(BettingPlatformError::NotLiquidatable.into());
        }
        
        self.status = CDPStatus::Liquidating;
        
        Ok(())
    }
    
    /// Complete liquidation
    pub fn complete_liquidation(&mut self) {
        self.status = CDPStatus::Liquidated;
        self.debt_amount = 0;
        self.collateral_amount = 0;
        self.health_factor = 0.0;
    }
}

/// CDP Global State
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CDPState {
    /// Total CDPs created
    pub total_cdps: u64,
    
    /// Active CDPs
    pub active_cdps: u64,
    
    /// Total collateral locked (USD value)
    pub total_collateral_usd: u128,
    
    /// Total debt issued
    pub total_debt: u128,
    
    /// Total liquidations
    pub total_liquidations: u64,
    
    /// Global collateral ratio
    pub global_collateral_ratio: f64,
    
    /// System health (0-100)
    pub system_health: u8,
    
    /// Emergency shutdown active
    pub emergency_shutdown: bool,
    
    /// Last update slot
    pub last_update_slot: u64,
}

impl CDPState {
    pub fn new() -> Self {
        Self {
            total_cdps: 0,
            active_cdps: 0,
            total_collateral_usd: 0,
            total_debt: 0,
            total_liquidations: 0,
            global_collateral_ratio: 0.0,
            system_health: 100,
            emergency_shutdown: false,
            last_update_slot: 0,
        }
    }
    
    /// Update global state
    pub fn update(&mut self) {
        if self.total_debt > 0 {
            self.global_collateral_ratio = self.total_collateral_usd as f64 / self.total_debt as f64;
            
            // Calculate system health
            let target_ratio = 1.5;
            if self.global_collateral_ratio >= target_ratio {
                self.system_health = 100;
            } else if self.global_collateral_ratio >= 1.2 {
                self.system_health = ((self.global_collateral_ratio - 1.2) / (target_ratio - 1.2) * 100.0) as u8;
            } else {
                self.system_health = 0;
            }
        } else {
            self.global_collateral_ratio = f64::MAX;
            self.system_health = 100;
        }
    }
    
    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        !self.emergency_shutdown && self.system_health > 20
    }
}

/// Debt position for tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DebtPosition {
    /// CDP ID
    pub cdp_id: u128,
    
    /// Owner
    pub owner: Pubkey,
    
    /// Principal borrowed
    pub principal: u128,
    
    /// Interest accrued
    pub interest_accrued: u128,
    
    /// Total owed
    pub total_owed: u128,
    
    /// Interest rate
    pub interest_rate: f64,
    
    /// Last update
    pub last_update: i64,
    
    /// Is active
    pub is_active: bool,
}

impl DebtPosition {
    pub fn new(cdp_id: u128, owner: Pubkey, principal: u128, interest_rate: f64) -> Self {
        Self {
            cdp_id,
            owner,
            principal,
            interest_accrued: 0,
            total_owed: principal,
            interest_rate,
            last_update: 0,
            is_active: true,
        }
    }
    
    /// Update interest
    pub fn update_interest(&mut self, current_time: i64) -> Result<(), ProgramError> {
        if !self.is_active || self.last_update == 0 {
            return Ok(());
        }
        
        let time_elapsed = current_time.saturating_sub(self.last_update) as f64;
        let seconds_per_year = 365.25 * 24.0 * 60.0 * 60.0;
        
        // Simple interest calculation
        let interest = (self.principal as f64) * self.interest_rate * (time_elapsed / seconds_per_year);
        
        self.interest_accrued = self.interest_accrued
            .checked_add(interest as u128)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.total_owed = self.principal
            .checked_add(self.interest_accrued)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.last_update = current_time;
        
        Ok(())
    }
}

/// Derive CDP account PDA
pub fn derive_cdp_account_pda(
    program_id: &Pubkey,
    owner: &Pubkey,
    cdp_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            CDP_ACCOUNT_SEED,
            owner.as_ref(),
            &cdp_id.to_le_bytes(),
        ],
        program_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cdp_creation() {
        let cdp = CDPAccount::new(
            Pubkey::default(),
            1,
            12345,
            CollateralType::USDC,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        
        assert_eq!(cdp.status, CDPStatus::Active);
        assert_eq!(cdp.collateral_amount, 0);
        assert_eq!(cdp.debt_amount, 0);
        assert_eq!(cdp.coll_cap, 2.0);
    }
    
    #[test]
    fn test_health_factor() {
        let mut cdp = CDPAccount::new(
            Pubkey::default(),
            1,
            12345,
            CollateralType::USDC,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        
        cdp.collateral_amount = 1500;
        cdp.debt_amount = 1000;
        
        let health = cdp.calculate_health_factor(1.0);
        assert!(health > 1.0); // Should be healthy
        
        // Test under-collateralized
        cdp.debt_amount = 2000;
        let health = cdp.calculate_health_factor(1.0);
        assert!(health < 1.0); // Should be unhealthy
    }
    
    #[test]
    fn test_borrow_limits() {
        let mut cdp = CDPAccount::new(
            Pubkey::default(),
            1,
            12345,
            CollateralType::USDC,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        
        cdp.collateral_amount = 1000;
        
        // USDC has max LTV of 0.67
        assert!(cdp.can_borrow(600, 1.0).is_ok());
        assert!(cdp.can_borrow(700, 1.0).is_err()); // Would exceed max LTV
    }
}