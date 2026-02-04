//! Mint Authority Management for Synthetic Tokens
//!
//! Controls who can mint/burn synthetic tokens based on oracle data

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
use spl_token_2022::{
    instruction::{mint_to, burn},
    state::Mint,
};

use crate::{
    error::BettingPlatformError,
    oracle::{OraclePDA, FallbackHandler, MAX_PROB_LATENCY_SLOTS},
    state::FusedMigrationFlags,
    constants::*,
};

/// Mint authority configuration
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MintAuthority {
    /// Program PDA that controls minting
    pub authority_pda: Pubkey,
    
    /// Bump seed for PDA
    pub bump: u8,
    
    /// Oracle account for price validation
    pub oracle_account: Pubkey,
    
    /// Market ID
    pub market_id: u128,
    
    /// Minting enabled
    pub minting_enabled: bool,
    
    /// Burning enabled
    pub burning_enabled: bool,
    
    /// Emergency pause
    pub emergency_paused: bool,
    
    /// Total minted across all users
    pub total_minted: u128,
    
    /// Total burned across all users
    pub total_burned: u128,
    
    /// Last mint timestamp
    pub last_mint_slot: u64,
    
    /// Last burn timestamp
    pub last_burn_slot: u64,
    
    /// Mint limits configuration
    pub mint_limits: MintLimits,
    
    /// Required collateral ratio
    pub collateral_ratio: f64,
    
    /// Liquidation threshold
    pub liquidation_threshold: f64,
}

/// Mint limits and constraints
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MintLimits {
    /// Maximum mint per transaction
    pub max_mint_per_tx: u128,
    
    /// Maximum mint per user
    pub max_mint_per_user: u128,
    
    /// Maximum total supply
    pub max_total_supply: u128,
    
    /// Minimum collateral required
    pub min_collateral: u64,
    
    /// Maximum leverage allowed
    pub max_leverage: u16,
    
    /// Cooldown between mints (slots)
    pub mint_cooldown: u64,
    
    /// Daily mint limit
    pub daily_mint_limit: u128,
    
    /// Current daily minted
    pub daily_minted: u128,
    
    /// Last daily reset slot
    pub last_daily_reset: u64,
}

impl MintLimits {
    pub fn new() -> Self {
        Self {
            max_mint_per_tx: 1_000_000 * 10u128.pow(9), // 1M tokens
            max_mint_per_user: 10_000_000 * 10u128.pow(9), // 10M tokens
            max_total_supply: 1_000_000_000 * 10u128.pow(9), // 1B tokens
            min_collateral: 100 * 10u64.pow(6), // 100 USDC
            max_leverage: 1000, // 1000x max
            mint_cooldown: 10, // 10 slots (~4 seconds)
            daily_mint_limit: 100_000_000 * 10u128.pow(9), // 100M daily
            daily_minted: 0,
            last_daily_reset: 0,
        }
    }
    
    /// Check if daily limit needs reset
    pub fn check_daily_reset(&mut self, current_slot: u64) {
        let slots_per_day = 216_000; // ~24 hours
        if current_slot >= self.last_daily_reset + slots_per_day {
            self.daily_minted = 0;
            self.last_daily_reset = current_slot;
        }
    }
    
    /// Check if can mint amount
    pub fn can_mint(&mut self, amount: u128, current_slot: u64) -> Result<(), ProgramError> {
        self.check_daily_reset(current_slot);
        
        if amount > self.max_mint_per_tx {
            msg!("Amount exceeds max per transaction");
            return Err(BettingPlatformError::ExceedsMintLimit.into());
        }
        
        if self.daily_minted + amount > self.daily_mint_limit {
            msg!("Would exceed daily mint limit");
            return Err(BettingPlatformError::DailyLimitExceeded.into());
        }
        
        Ok(())
    }
    
    /// Record minted amount
    pub fn record_mint(&mut self, amount: u128) {
        self.daily_minted += amount;
    }
}

/// Mint configuration for different token types
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MintConfig {
    /// Token type being minted
    pub token_type: super::token::TokenType,
    
    /// Base collateral ratio
    pub base_collateral_ratio: f64,
    
    /// Risk adjustment factor
    pub risk_adjustment: f64,
    
    /// Oracle scalar multiplier
    pub scalar_multiplier: f64,
    
    /// Liquidation penalty
    pub liquidation_penalty: f64,
    
    /// Mint fee (basis points)
    pub mint_fee_bps: u32,
    
    /// Burn fee (basis points)
    pub burn_fee_bps: u32,
    
    /// Protocol fee recipient
    pub protocol_fee_recipient: Pubkey,
}

impl MintConfig {
    pub fn new(token_type: super::token::TokenType) -> Self {
        let (collateral_ratio, risk_adj, liquidation_penalty) = match token_type {
            super::token::TokenType::Collateral => (1.5, 1.0, 0.1),
            super::token::TokenType::Leverage => (2.0, 1.5, 0.15),
            super::token::TokenType::Yield => (1.2, 0.8, 0.05),
            super::token::TokenType::Liquidation => (1.0, 2.0, 0.0),
            super::token::TokenType::Quantum => (1.8, 1.2, 0.12),
        };
        
        Self {
            token_type,
            base_collateral_ratio: collateral_ratio,
            risk_adjustment: risk_adj,
            scalar_multiplier: 1.0,
            liquidation_penalty,
            mint_fee_bps: 30, // 0.3%
            burn_fee_bps: 10, // 0.1%
            protocol_fee_recipient: Pubkey::default(),
        }
    }
    
    /// Calculate required collateral
    pub fn calculate_required_collateral(
        &self,
        mint_amount: u128,
        oracle_scalar: f64,
    ) -> u128 {
        let base_collateral = mint_amount as f64 * self.base_collateral_ratio;
        let risk_adjusted = base_collateral * self.risk_adjustment;
        let scalar_adjusted = risk_adjusted / oracle_scalar.max(1.0);
        
        scalar_adjusted as u128
    }
    
    /// Calculate mint fee
    pub fn calculate_mint_fee(&self, amount: u128) -> u128 {
        (amount * self.mint_fee_bps as u128) / 10000
    }
    
    /// Calculate burn fee
    pub fn calculate_burn_fee(&self, amount: u128) -> u128 {
        (amount * self.burn_fee_bps as u128) / 10000
    }
}

impl MintAuthority {
    pub fn new(
        authority_pda: Pubkey,
        bump: u8,
        oracle_account: Pubkey,
        market_id: u128,
    ) -> Self {
        Self {
            authority_pda,
            bump,
            oracle_account,
            market_id,
            minting_enabled: true,
            burning_enabled: true,
            emergency_paused: false,
            total_minted: 0,
            total_burned: 0,
            last_mint_slot: 0,
            last_burn_slot: 0,
            mint_limits: MintLimits::new(),
            collateral_ratio: 1.5,
            liquidation_threshold: 1.2,
        }
    }
    
    /// Validate mint authority
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.emergency_paused {
            msg!("Minting is emergency paused");
            return Err(BettingPlatformError::EmergencyPause.into());
        }
        
        if !self.minting_enabled {
            msg!("Minting is disabled");
            return Err(BettingPlatformError::MintingDisabled.into());
        }
        
        Ok(())
    }
    
    /// Check if can mint based on oracle
    pub fn can_mint_with_oracle(
        &mut self,
        amount: u128,
        oracle_pda: &OraclePDA,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Basic validation
        self.validate()?;
        
        // Check mint limits
        self.mint_limits.can_mint(amount, current_slot)?;
        
        // Check cooldown
        if current_slot < self.last_mint_slot + self.mint_limits.mint_cooldown {
            msg!("Mint cooldown not met");
            return Err(BettingPlatformError::CooldownActive.into());
        }
        
        // Validate oracle data freshness
        if current_slot > oracle_pda.last_update_slot + MAX_PROB_LATENCY_SLOTS {
            msg!("Oracle data is stale");
            return Err(BettingPlatformError::StaleOracle.into());
        }
        
        // Check if oracle is halted
        if oracle_pda.is_halted {
            msg!("Oracle is halted");
            return Err(BettingPlatformError::OracleHalted.into());
        }
        
        Ok(())
    }
    
    /// Execute mint operation
    pub fn execute_mint(
        &mut self,
        amount: u128,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        self.total_minted = self.total_minted
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.mint_limits.record_mint(amount);
        self.last_mint_slot = current_slot;
        
        Ok(())
    }
    
    /// Execute burn operation
    pub fn execute_burn(
        &mut self,
        amount: u128,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        if !self.burning_enabled {
            return Err(BettingPlatformError::BurningDisabled.into());
        }
        
        self.total_burned = self.total_burned
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.last_burn_slot = current_slot;
        
        Ok(())
    }
    
    /// Pause minting (emergency)
    pub fn pause(&mut self) {
        self.emergency_paused = true;
        self.minting_enabled = false;
    }
    
    /// Resume minting
    pub fn resume(&mut self) {
        self.emergency_paused = false;
        self.minting_enabled = true;
    }
}

/// Validate mint authority for operation
pub fn validate_mint_authority(
    authority_account: &AccountInfo,
    expected_authority: &Pubkey,
    signer_account: &AccountInfo,
) -> Result<(), ProgramError> {
    // Check if authority account is correct
    if authority_account.key != expected_authority {
        msg!("Invalid mint authority account");
        return Err(BettingPlatformError::InvalidAuthority.into());
    }
    
    // Check if signer is authorized
    if !signer_account.is_signer {
        msg!("Signer required for mint operation");
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    Ok(())
}

/// Calculate mint amount based on oracle data
pub fn calculate_mint_amount(
    collateral: u64,
    oracle_scalar: f64,
    leverage: u16,
    config: &MintConfig,
) -> Result<u128, ProgramError> {
    // Base mint amount
    let base_amount = collateral as u128 * leverage as u128;
    
    // Apply oracle scalar
    let scaled_amount = (base_amount as f64 * oracle_scalar) as u128;
    
    // Apply risk adjustment
    let risk_adjusted = (scaled_amount as f64 * config.risk_adjustment) as u128;
    
    // Apply fees
    let fee = config.calculate_mint_fee(risk_adjusted);
    let final_amount = risk_adjusted
        .checked_sub(fee)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    
    Ok(final_amount)
}

/// Mint synthetic tokens
pub fn mint_synthetic_tokens(
    program_id: &Pubkey,
    mint: &AccountInfo,
    destination: &AccountInfo,
    mint_authority: &AccountInfo,
    token_program: &AccountInfo,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    msg!("Minting {} synthetic tokens", amount);
    
    let mint_ix = mint_to(
        token_program.key,
        mint.key,
        destination.key,
        mint_authority.key,
        &[],
        amount,
    )?;
    
    invoke_signed(
        &mint_ix,
        &[
            mint.clone(),
            destination.clone(),
            mint_authority.clone(),
        ],
        &[signer_seeds],
    )?;
    
    msg!("Successfully minted {} tokens", amount);
    Ok(())
}

/// Burn synthetic tokens
pub fn burn_synthetic_tokens(
    program_id: &Pubkey,
    source: &AccountInfo,
    mint: &AccountInfo,
    authority: &AccountInfo,
    token_program: &AccountInfo,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    msg!("Burning {} synthetic tokens", amount);
    
    let burn_ix = burn(
        token_program.key,
        source.key,
        mint.key,
        authority.key,
        &[],
        amount,
    )?;
    
    invoke_signed(
        &burn_ix,
        &[
            source.clone(),
            mint.clone(),
            authority.clone(),
        ],
        &[signer_seeds],
    )?;
    
    msg!("Successfully burned {} tokens", amount);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mint_limits() {
        let mut limits = MintLimits::new();
        
        // Test daily reset
        limits.daily_minted = 1000;
        limits.last_daily_reset = 0;
        limits.check_daily_reset(216_001);
        assert_eq!(limits.daily_minted, 0);
        
        // Test can mint
        assert!(limits.can_mint(1000, 216_001).is_ok());
        
        // Test exceeds limit
        let large_amount = limits.max_mint_per_tx + 1;
        assert!(limits.can_mint(large_amount, 216_001).is_err());
    }
    
    #[test]
    fn test_mint_config() {
        let config = MintConfig::new(super::super::token::TokenType::Leverage);
        
        // Test collateral calculation
        let collateral = config.calculate_required_collateral(
            1000 * 10u128.pow(9),
            2.0, // oracle scalar
        );
        assert!(collateral > 0);
        
        // Test fee calculation
        let fee = config.calculate_mint_fee(10000);
        assert_eq!(fee, 30); // 0.3% of 10000
    }
    
    #[test]
    fn test_mint_authority() {
        let mut authority = MintAuthority::new(
            Pubkey::default(),
            255,
            Pubkey::default(),
            12345,
        );
        
        // Test validation
        assert!(authority.validate().is_ok());
        
        // Test pause
        authority.pause();
        assert!(authority.validate().is_err());
        
        // Test resume
        authority.resume();
        assert!(authority.validate().is_ok());
        
        // Test mint execution
        assert!(authority.execute_mint(1000, 100).is_ok());
        assert_eq!(authority.total_minted, 1000);
        assert_eq!(authority.last_mint_slot, 100);
    }
}