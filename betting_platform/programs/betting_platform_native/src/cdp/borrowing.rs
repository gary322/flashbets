//! CDP Borrowing Mechanism
//!
//! Handles borrowing of synthetic tokens against collateral

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    oracle::{OraclePDA, FallbackHandler},
    synthetics::{MintAuthority, MintConfig, TokenType},
    constants::*,
};

use super::state::{CDPAccount, CollateralType, CDPStatus};

/// Borrow request parameters
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BorrowRequest {
    /// CDP ID
    pub cdp_id: u128,
    
    /// Amount to borrow (in synthetic tokens)
    pub borrow_amount: u128,
    
    /// Desired leverage
    pub leverage: u16,
    
    /// Max interest rate accepted
    pub max_interest_rate: f64,
    
    /// Collateral to add (if any)
    pub additional_collateral: u128,
    
    /// Use oracle scalar
    pub use_oracle_scalar: bool,
    
    /// Slippage tolerance
    pub slippage_tolerance: f64,
}

/// Borrow limits configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BorrowLimits {
    /// Minimum borrow amount
    pub min_borrow: u128,
    
    /// Maximum borrow per CDP
    pub max_borrow_per_cdp: u128,
    
    /// Maximum total system debt
    pub max_total_debt: u128,
    
    /// Maximum leverage allowed
    pub max_leverage: u16,
    
    /// Minimum collateral ratio
    pub min_collateral_ratio: f64,
    
    /// Borrow fee (basis points)
    pub borrow_fee_bps: u32,
    
    /// Daily borrow limit per user
    pub daily_limit_per_user: u128,
    
    /// Current daily borrowed
    pub daily_borrowed: u128,
    
    /// Last daily reset
    pub last_daily_reset: i64,
}

impl BorrowLimits {
    pub fn new() -> Self {
        Self {
            min_borrow: 100 * 10u128.pow(6), // 100 USDC min
            max_borrow_per_cdp: 10_000_000 * 10u128.pow(6), // 10M max
            max_total_debt: 1_000_000_000 * 10u128.pow(6), // 1B total
            max_leverage: MAX_FUSED_LEVERAGE as u16,
            min_collateral_ratio: 1.5,
            borrow_fee_bps: 50, // 0.5%
            daily_limit_per_user: 1_000_000 * 10u128.pow(6), // 1M daily
            daily_borrowed: 0,
            last_daily_reset: 0,
        }
    }
    
    /// Check and reset daily limit
    pub fn check_daily_reset(&mut self, current_time: i64) {
        let seconds_per_day = 86400;
        if current_time >= self.last_daily_reset + seconds_per_day {
            self.daily_borrowed = 0;
            self.last_daily_reset = current_time;
        }
    }
    
    /// Validate borrow request
    pub fn validate_borrow(
        &mut self,
        amount: u128,
        current_time: i64,
    ) -> Result<(), ProgramError> {
        self.check_daily_reset(current_time);
        
        if amount < self.min_borrow {
            msg!("Borrow amount below minimum");
            return Err(BettingPlatformError::BelowMinimum.into());
        }
        
        if amount > self.max_borrow_per_cdp {
            msg!("Borrow amount exceeds CDP limit");
            return Err(BettingPlatformError::ExceedsBorrowLimit.into());
        }
        
        if self.daily_borrowed + amount > self.daily_limit_per_user {
            msg!("Would exceed daily borrow limit");
            return Err(BettingPlatformError::DailyLimitExceeded.into());
        }
        
        Ok(())
    }
    
    /// Record borrow
    pub fn record_borrow(&mut self, amount: u128) {
        self.daily_borrowed += amount;
    }
}

/// Borrow position tracking
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BorrowPosition {
    /// CDP ID
    pub cdp_id: u128,
    
    /// Borrow ID
    pub borrow_id: u128,
    
    /// Owner
    pub owner: Pubkey,
    
    /// Amount borrowed
    pub amount_borrowed: u128,
    
    /// Collateral locked
    pub collateral_locked: u128,
    
    /// Leverage used
    pub leverage: u16,
    
    /// Interest rate
    pub interest_rate: f64,
    
    /// Oracle scalar at borrow
    pub oracle_scalar: f64,
    
    /// Borrow timestamp
    pub borrowed_at: i64,
    
    /// Last interest accrual
    pub last_interest_update: i64,
    
    /// Total interest accrued
    pub interest_accrued: u128,
    
    /// Is active
    pub is_active: bool,
    
    /// Liquidation price
    pub liquidation_price: f64,
}

impl BorrowPosition {
    pub fn new(
        cdp_id: u128,
        borrow_id: u128,
        owner: Pubkey,
        amount: u128,
        collateral: u128,
        leverage: u16,
        interest_rate: f64,
        oracle_scalar: f64,
    ) -> Self {
        Self {
            cdp_id,
            borrow_id,
            owner,
            amount_borrowed: amount,
            collateral_locked: collateral,
            leverage,
            interest_rate,
            oracle_scalar,
            borrowed_at: 0,
            last_interest_update: 0,
            interest_accrued: 0,
            is_active: true,
            liquidation_price: 0.0,
        }
    }
    
    /// Calculate interest accrued
    pub fn calculate_interest(&mut self, current_time: i64) -> u128 {
        if !self.is_active || self.last_interest_update == 0 {
            return 0;
        }
        
        let time_elapsed = current_time.saturating_sub(self.last_interest_update) as f64;
        let seconds_per_year = 365.25 * 24.0 * 60.0 * 60.0;
        
        // Compound interest
        let rate_per_second = self.interest_rate / seconds_per_year;
        let compound_factor = (1.0 + rate_per_second).powf(time_elapsed);
        let new_total = (self.amount_borrowed as f64) * compound_factor;
        let interest = (new_total - self.amount_borrowed as f64) as u128;
        
        self.interest_accrued += interest;
        self.last_interest_update = current_time;
        
        interest
    }
    
    /// Get total owed
    pub fn get_total_owed(&self) -> u128 {
        self.amount_borrowed + self.interest_accrued
    }
}

/// Calculate maximum borrow capacity
pub fn calculate_borrow_capacity(
    collateral_amount: u128,
    collateral_type: &CollateralType,
    oracle_price: f64,
    oracle_scalar: f64,
) -> u128 {
    // Get max LTV for collateral type
    let max_ltv = collateral_type.get_max_ltv();
    
    // Calculate collateral value in USD
    let collateral_value_usd = (collateral_amount as f64) * oracle_price;
    
    // Apply oracle scalar for fused leverage
    let scaled_value = collateral_value_usd * oracle_scalar;
    
    // Calculate max borrow
    let max_borrow = scaled_value * max_ltv;
    
    max_borrow as u128
}

/// Calculate maximum borrow with leverage
pub fn calculate_max_borrow(
    collateral_amount: u128,
    leverage: u16,
    collateral_type: &CollateralType,
    oracle_price: f64,
    use_oracle_scalar: bool,
    oracle_scalar: f64,
) -> Result<u128, ProgramError> {
    // Base borrow = collateral * leverage
    let base_borrow = (collateral_amount as u128)
        .checked_mul(leverage as u128)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    
    // Apply oracle scalar if enabled
    let scaled_borrow = if use_oracle_scalar {
        ((base_borrow as f64) * oracle_scalar) as u128
    } else {
        base_borrow
    };
    
    // Check against max LTV
    let max_capacity = calculate_borrow_capacity(
        collateral_amount,
        collateral_type,
        oracle_price,
        if use_oracle_scalar { oracle_scalar } else { 1.0 },
    );
    
    // Return minimum of scaled borrow and max capacity
    Ok(scaled_borrow.min(max_capacity))
}

/// Execute borrow operation
pub fn execute_borrow(
    program_id: &Pubkey,
    cdp_account: &mut CDPAccount,
    borrow_request: &BorrowRequest,
    oracle_pda: &OraclePDA,
    mint_authority: &mut MintAuthority,
    current_time: i64,
) -> Result<u128, ProgramError> {
    // Validate CDP
    cdp_account.validate()?;
    
    // Add additional collateral if provided
    if borrow_request.additional_collateral > 0 {
        cdp_account.deposit_collateral(borrow_request.additional_collateral)?;
    }
    
    // Get oracle price and scalar
    let oracle_price = oracle_pda.current_prob;
    let oracle_scalar = if borrow_request.use_oracle_scalar {
        oracle_pda.calculate_scalar()
    } else {
        1.0
    };
    
    // Calculate max borrow
    let max_borrow = calculate_max_borrow(
        cdp_account.collateral_amount,
        borrow_request.leverage,
        &cdp_account.collateral_type,
        oracle_price,
        borrow_request.use_oracle_scalar,
        oracle_scalar,
    )?;
    
    // Check requested amount
    if borrow_request.borrow_amount > max_borrow {
        msg!("Requested borrow exceeds maximum: {} > {}", 
             borrow_request.borrow_amount, max_borrow);
        return Err(BettingPlatformError::ExceedsBorrowLimit.into());
    }
    
    // Check interest rate
    let interest_rate = calculate_interest_rate(
        cdp_account.debt_amount,
        borrow_request.borrow_amount,
        cdp_account.collateral_amount,
    );
    
    if interest_rate > borrow_request.max_interest_rate {
        msg!("Interest rate {} exceeds maximum {}", 
             interest_rate, borrow_request.max_interest_rate);
        return Err(BettingPlatformError::InterestRateTooHigh.into());
    }
    
    // Update CDP
    cdp_account.borrow(borrow_request.borrow_amount, oracle_price)?;
    cdp_account.interest_rate = interest_rate;
    cdp_account.last_action = current_time;
    
    // Check if can mint
    mint_authority.can_mint_with_oracle(
        borrow_request.borrow_amount,
        oracle_pda,
        Clock::get()?.slot,
    )?;
    
    // Execute mint
    mint_authority.execute_mint(
        borrow_request.borrow_amount,
        Clock::get()?.slot,
    )?;
    
    msg!("Borrowed {} synthetic tokens at {}x leverage", 
         borrow_request.borrow_amount, borrow_request.leverage);
    
    Ok(borrow_request.borrow_amount)
}

/// Execute repay operation
pub fn execute_repay(
    program_id: &Pubkey,
    cdp_account: &mut CDPAccount,
    repay_amount: u128,
    mint_authority: &mut MintAuthority,
    current_time: i64,
) -> Result<u128, ProgramError> {
    // Calculate interest owed
    let interest_owed = calculate_accrued_interest(
        cdp_account.debt_amount,
        cdp_account.interest_rate,
        cdp_account.last_interest_update,
        current_time,
    );
    
    // Total amount to repay
    let total_owed = cdp_account.debt_amount + interest_owed;
    
    // Determine actual repay amount
    let actual_repay = repay_amount.min(total_owed);
    
    // First apply to interest, then principal
    let mut remaining = actual_repay;
    
    if interest_owed > 0 {
        let interest_payment = remaining.min(interest_owed);
        cdp_account.accrued_interest = cdp_account.accrued_interest
            .saturating_sub(interest_payment);
        remaining = remaining.saturating_sub(interest_payment);
    }
    
    // Apply remaining to principal
    if remaining > 0 {
        cdp_account.repay(remaining)?;
    }
    
    // Update last action
    cdp_account.last_action = current_time;
    cdp_account.last_interest_update = current_time;
    
    // Execute burn of synthetic tokens
    mint_authority.execute_burn(actual_repay, Clock::get()?.slot)?;
    
    msg!("Repaid {} tokens (interest: {}, principal: {})", 
         actual_repay, 
         actual_repay.saturating_sub(remaining),
         remaining);
    
    Ok(actual_repay)
}

/// Calculate interest rate based on utilization
pub fn calculate_interest_rate(
    current_debt: u128,
    new_borrow: u128,
    collateral: u128,
) -> f64 {
    if collateral == 0 {
        return 0.2; // 20% max rate
    }
    
    // Calculate utilization ratio
    let total_debt = current_debt + new_borrow;
    let utilization = (total_debt as f64) / (collateral as f64);
    
    // Interest rate model (kink at 80% utilization)
    let base_rate = 0.02; // 2% base
    let kink = 0.8;
    let slope1 = 0.1; // 10% slope before kink
    let slope2 = 0.5; // 50% slope after kink
    
    if utilization <= kink {
        base_rate + slope1 * utilization
    } else {
        base_rate + slope1 * kink + slope2 * (utilization - kink)
    }
}

/// Calculate accrued interest
pub fn calculate_accrued_interest(
    principal: u128,
    rate: f64,
    last_update: i64,
    current_time: i64,
) -> u128 {
    if last_update == 0 || last_update >= current_time {
        return 0;
    }
    
    let time_elapsed = (current_time - last_update) as f64;
    let seconds_per_year = 365.25 * 24.0 * 60.0 * 60.0;
    
    // Simple interest for short periods
    let interest = (principal as f64) * rate * (time_elapsed / seconds_per_year);
    
    interest as u128
}

/// Create a new borrow position
pub fn create_borrow_position(
    cdp_id: u128,
    owner: Pubkey,
    borrow_request: &BorrowRequest,
    oracle_scalar: f64,
    interest_rate: f64,
) -> BorrowPosition {
    let mut position = BorrowPosition::new(
        cdp_id,
        cdp_id * 1000 + 1, // Simple borrow ID generation
        owner,
        borrow_request.borrow_amount,
        borrow_request.additional_collateral,
        borrow_request.leverage,
        interest_rate,
        oracle_scalar,
    );
    
    position.borrowed_at = Clock::get().unwrap().unix_timestamp;
    position.last_interest_update = position.borrowed_at;
    
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_borrow_capacity() {
        let collateral = 1000;
        let oracle_price = 1.0;
        let oracle_scalar = 2.0;
        
        let capacity = calculate_borrow_capacity(
            collateral,
            &CollateralType::USDC,
            oracle_price,
            oracle_scalar,
        );
        
        // USDC max LTV = 0.67, so capacity = 1000 * 1.0 * 2.0 * 0.67 = 1340
        assert_eq!(capacity, 1340);
    }
    
    #[test]
    fn test_interest_rate_calculation() {
        // Low utilization
        let rate = calculate_interest_rate(200, 100, 1000);
        assert!(rate < 0.05); // Should be low
        
        // High utilization
        let rate = calculate_interest_rate(800, 100, 1000);
        assert!(rate > 0.1); // Should be higher
        
        // Over-utilized
        let rate = calculate_interest_rate(900, 200, 1000);
        assert!(rate > 0.15); // Should be very high
    }
    
    #[test]
    fn test_borrow_limits() {
        let mut limits = BorrowLimits::new();
        
        // Test validation
        assert!(limits.validate_borrow(1000 * 10u128.pow(6), 0).is_ok());
        assert!(limits.validate_borrow(50 * 10u128.pow(6), 0).is_err()); // Below min
        
        // Test daily limit
        limits.record_borrow(500_000 * 10u128.pow(6));
        assert!(limits.validate_borrow(600_000 * 10u128.pow(6), 0).is_err()); // Exceeds daily
    }
}