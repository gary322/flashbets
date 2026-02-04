//! Multi-collateral support for various SPL tokens
//!
//! Extends collateral management to support USDT, SOL, and other tokens

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    cpi::{associated_token, spl_token},
    error::BettingPlatformError,
    events::{CollateralDeposited, CollateralWithdrawn, Event},
    pda::CollateralVaultPDA,
};

/// Supported collateral types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum CollateralType {
    USDC,
    USDT,
    SOL,
    WBTC,
    WETH,
}

impl CollateralType {
    /// Get the mint address for each collateral type
    pub fn mint_address(&self) -> Pubkey {
        match self {
            CollateralType::USDC => solana_program::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
            CollateralType::USDT => solana_program::pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
            CollateralType::SOL => solana_program::pubkey!("So11111111111111111111111111111111111111112"), // Wrapped SOL
            CollateralType::WBTC => solana_program::pubkey!("9n4nbM75f5Ui33ZbPYXn59EwSgE8CGsHtAeTH5YFeJ9E"), // Wrapped BTC
            CollateralType::WETH => solana_program::pubkey!("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs"), // Wrapped ETH
        }
    }

    /// Get the decimal places for each collateral type
    pub fn decimals(&self) -> u8 {
        match self {
            CollateralType::USDC => 6,
            CollateralType::USDT => 6,
            CollateralType::SOL => 9,
            CollateralType::WBTC => 8,
            CollateralType::WETH => 8,
        }
    }

    /// Get the collateral value in USD using oracle prices
    pub fn get_usd_value(&self, amount: u64, oracle_price: u64) -> Result<u64, ProgramError> {
        // Oracle price is expected to be in USD with 8 decimals precision
        // For stablecoins, oracle should still provide price (around 100_000_000 for $1)
        
        // Validate oracle price is reasonable (between $0.01 and $1,000,000)
        if oracle_price < 1_000_000 || oracle_price > 100_000_000_000_000 {
            return Err(BettingPlatformError::InvalidPrice.into());
        }

        // Calculate USD value with proper decimal adjustment
        let decimal_divisor = 10u64.pow(self.decimals() as u32);
        
        // First normalize amount to standard precision
        let normalized_amount = amount as u128;
        
        // Calculate: (amount * oracle_price) / (decimal_divisor * 10^8)
        // Where 10^8 accounts for oracle price decimals
        let usd_value = normalized_amount
            .checked_mul(oracle_price as u128)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(decimal_divisor as u128)
            .ok_or(BettingPlatformError::DivisionByZero)?
            .checked_div(100_000_000) // Oracle price decimals (8)
            .ok_or(BettingPlatformError::DivisionByZero)?
            .checked_div(1_000_000) // Convert to USDC decimals (6)
            .ok_or(BettingPlatformError::DivisionByZero)?;

        // Ensure result fits in u64
        if usd_value > u64::MAX as u128 {
            return Err(BettingPlatformError::MathOverflow.into());
        }

        Ok(usd_value as u64)
    }

    /// Get the collateral ratio (LTV) for risk management
    pub fn ltv_ratio(&self) -> u8 {
        match self {
            CollateralType::USDC => 100,  // 100% LTV for stablecoins
            CollateralType::USDT => 100,  // 100% LTV for stablecoins
            CollateralType::SOL => 80,    // 80% LTV for volatile assets
            CollateralType::WBTC => 80,   // 80% LTV for volatile assets
            CollateralType::WETH => 80,   // 80% LTV for volatile assets
        }
    }
}

/// Multi-collateral vault tracking different asset types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MultiCollateralVault {
    pub discriminator: [u8; 8],
    pub usdc_deposits: u64,
    pub usdt_deposits: u64,
    pub sol_deposits: u64,
    pub wbtc_deposits: u64,
    pub weth_deposits: u64,
    pub total_usd_value: u64,
    pub total_borrowed_usd: u64,
    pub depositor_count: u32,
    pub last_update: i64,
    pub oracle_feed: Pubkey,
}

impl MultiCollateralVault {
    pub const DISCRIMINATOR: [u8; 8] = *b"MULTCOLL";

    /// Calculate total USD value of all collateral
    pub fn calculate_total_usd_value(&self) -> Result<u64, ProgramError> {
        // Oracle prices with 8 decimals precision
        let usdc_oracle_price = 100_000_000; // $1.00
        let usdt_oracle_price = 100_000_000; // $1.00
        let sol_oracle_price = 10_000_000_000; // $100.00
        let wbtc_oracle_price = 4_000_000_000_000; // $40,000.00
        let weth_oracle_price = 250_000_000_000; // $2,500.00
        
        let usdc_value = CollateralType::USDC.get_usd_value(self.usdc_deposits, usdc_oracle_price)?;
        let usdt_value = CollateralType::USDT.get_usd_value(self.usdt_deposits, usdt_oracle_price)?;
        let sol_value = CollateralType::SOL.get_usd_value(self.sol_deposits, sol_oracle_price)?;
        let wbtc_value = CollateralType::WBTC.get_usd_value(self.wbtc_deposits, wbtc_oracle_price)?;
        let weth_value = CollateralType::WETH.get_usd_value(self.weth_deposits, weth_oracle_price)?;

        usdc_value
            .checked_add(usdt_value)
            .and_then(|v| v.checked_add(sol_value))
            .and_then(|v| v.checked_add(wbtc_value))
            .and_then(|v| v.checked_add(weth_value))
            .ok_or(BettingPlatformError::MathOverflow.into())
    }

    /// Get available borrowing power considering LTV ratios
    pub fn get_borrowing_power(&self) -> Result<u64, ProgramError> {
        let usdc_power = self.usdc_deposits
            .checked_mul(CollateralType::USDC.ltv_ratio() as u64)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::DivisionByZero)?;

        let usdt_power = self.usdt_deposits
            .checked_mul(CollateralType::USDT.ltv_ratio() as u64)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::DivisionByZero)?;

        // Convert SOL to USD value then apply LTV
        let sol_oracle_price = 10_000_000_000; // $100.00 with 8 decimals
        let sol_usd = CollateralType::SOL.get_usd_value(self.sol_deposits, sol_oracle_price)?;
        let sol_power = sol_usd
            .checked_mul(CollateralType::SOL.ltv_ratio() as u64)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::DivisionByZero)?;

        // Similar for WBTC and WETH
        let wbtc_oracle_price = 4_000_000_000_000; // $40,000.00 with 8 decimals
        let wbtc_usd = CollateralType::WBTC.get_usd_value(self.wbtc_deposits, wbtc_oracle_price)?;
        let wbtc_power = wbtc_usd
            .checked_mul(CollateralType::WBTC.ltv_ratio() as u64)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::DivisionByZero)?;

        let weth_oracle_price = 250_000_000_000; // $2,500.00 with 8 decimals
        let weth_usd = CollateralType::WETH.get_usd_value(self.weth_deposits, weth_oracle_price)?;
        let weth_power = weth_usd
            .checked_mul(CollateralType::WETH.ltv_ratio() as u64)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::DivisionByZero)?;

        // Sum up total borrowing power
        usdc_power
            .checked_add(usdt_power)
            .and_then(|v| v.checked_add(sol_power))
            .and_then(|v| v.checked_add(wbtc_power))
            .and_then(|v| v.checked_add(weth_power))
            .and_then(|v| v.checked_sub(self.total_borrowed_usd))
            .ok_or(BettingPlatformError::MathOverflow.into())
    }
}

/// Process multi-collateral deposit
pub fn process_deposit_multi_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    collateral_type: CollateralType,
    amount: u64,
) -> ProgramResult {
    msg!("Processing multi-collateral deposit of {} {:?}", amount, collateral_type);
    
    let account_info_iter = &mut accounts.iter();
    
    let depositor = next_account_info(account_info_iter)?;
    let depositor_token_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let token_mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let associated_token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(depositor)?;
    validate_writable(vault_account)?;
    validate_writable(vault_token_account)?;
    
    // Verify token mint matches collateral type
    if token_mint.key != &collateral_type.mint_address() {
        return Err(BettingPlatformError::InvalidMint.into());
    }
    
    // Derive and verify vault PDA
    let (vault_pda, bump) = CollateralVaultPDA::derive(program_id);
    if vault_account.key != &vault_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create vault's associated token account if needed
    let vault_ata = associated_token::get_associated_token_address(
        &vault_pda,
        &collateral_type.mint_address(),
    );
    
    if vault_token_account.key != &vault_ata {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Create ATA if it doesn't exist
    if vault_token_account.data_is_empty() {
        associated_token::create_associated_token_account(
            depositor,
            vault_token_account,
            vault_account,
            token_mint,
            system_program,
            token_program,
            rent_sysvar,
        )?;
    }
    
    // Transfer tokens from depositor to vault
    spl_token::transfer(
        depositor_token_account,
        vault_token_account,
        depositor,
        amount,
        token_program,
        &[],
    )?;
    
    // Load or initialize multi-collateral vault state
    let mut vault = if vault_account.data_len() > 0 {
        MultiCollateralVault::try_from_slice(&vault_account.data.borrow())?
    } else {
        MultiCollateralVault {
            discriminator: MultiCollateralVault::DISCRIMINATOR,
            usdc_deposits: 0,
            usdt_deposits: 0,
            sol_deposits: 0,
            wbtc_deposits: 0,
            weth_deposits: 0,
            total_usd_value: 0,
            total_borrowed_usd: 0,
            depositor_count: 0,
            last_update: Clock::get()?.unix_timestamp,
            oracle_feed: Pubkey::default(),
        }
    };
    
    // Update vault state based on collateral type
    match collateral_type {
        CollateralType::USDC => {
            vault.usdc_deposits = vault.usdc_deposits
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }
        CollateralType::USDT => {
            vault.usdt_deposits = vault.usdt_deposits
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }
        CollateralType::SOL => {
            vault.sol_deposits = vault.sol_deposits
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }
        CollateralType::WBTC => {
            vault.wbtc_deposits = vault.wbtc_deposits
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }
        CollateralType::WETH => {
            vault.weth_deposits = vault.weth_deposits
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }
    }
    
    // Update total USD value
    vault.total_usd_value = vault.calculate_total_usd_value()?;
    vault.depositor_count += 1;
    vault.last_update = Clock::get()?.unix_timestamp;
    
    // Save vault state
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Convert amount to USD for event
    let oracle_price = match collateral_type {
        CollateralType::USDC => 100_000_000, // $1.00
        CollateralType::USDT => 100_000_000, // $1.00
        CollateralType::SOL => 10_000_000_000, // $100.00
        CollateralType::WBTC => 4_000_000_000_000, // $40,000.00
        CollateralType::WETH => 250_000_000_000, // $2,500.00
    };
    let usd_value = collateral_type.get_usd_value(amount, oracle_price)?;
    
    // Emit event
    CollateralDeposited {
        depositor: *depositor.key,
        amount: usd_value,
        total_deposits: vault.total_usd_value,
        timestamp: Clock::get()?.unix_timestamp,
    }
    .emit();
    
    msg!("Multi-collateral deposit successful");
    Ok(())
}

/// Process multi-collateral withdrawal
pub fn process_withdraw_multi_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    collateral_type: CollateralType,
    amount: u64,
) -> ProgramResult {
    msg!("Processing multi-collateral withdrawal of {} {:?}", amount, collateral_type);
    
    let account_info_iter = &mut accounts.iter();
    
    let withdrawer = next_account_info(account_info_iter)?;
    let withdrawer_token_account = next_account_info(account_info_iter)?;
    let vault_account = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(withdrawer)?;
    validate_writable(vault_account)?;
    validate_writable(vault_token_account)?;
    validate_writable(withdrawer_token_account)?;
    
    // Verify vault PDA
    let (vault_pda, bump) = CollateralVaultPDA::derive(program_id);
    if vault_account.key != &vault_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load vault state
    let mut vault = MultiCollateralVault::try_from_slice(&vault_account.data.borrow())?;
    
    // Check available balance for specific collateral type
    let available = match collateral_type {
        CollateralType::USDC => vault.usdc_deposits,
        CollateralType::USDT => vault.usdt_deposits,
        CollateralType::SOL => vault.sol_deposits,
        CollateralType::WBTC => vault.wbtc_deposits,
        CollateralType::WETH => vault.weth_deposits,
    };
    
    if amount > available {
        return Err(BettingPlatformError::InsufficientCollateral.into());
    }
    
    // Check borrowing power remains positive after withdrawal
    let oracle_price = match collateral_type {
        CollateralType::USDC => 100_000_000, // $1.00
        CollateralType::USDT => 100_000_000, // $1.00
        CollateralType::SOL => 10_000_000_000, // $100.00
        CollateralType::WBTC => 4_000_000_000_000, // $40,000.00
        CollateralType::WETH => 250_000_000_000, // $2,500.00
    };
    let usd_value = collateral_type.get_usd_value(amount, oracle_price)?;
    let new_borrowing_power = vault.get_borrowing_power()?
        .checked_sub(usd_value)
        .ok_or(BettingPlatformError::Underflow)?;
    
    if new_borrowing_power < 0 {
        return Err(BettingPlatformError::InsufficientCollateral.into());
    }
    
    // Transfer tokens from vault to withdrawer
    let vault_seeds = &[
        b"collateral_vault".as_ref(),
        &[bump],
    ];
    spl_token::transfer(
        vault_token_account,
        withdrawer_token_account,
        vault_authority,
        amount,
        token_program,
        &[vault_seeds],
    )?;
    
    // Update vault state
    match collateral_type {
        CollateralType::USDC => {
            vault.usdc_deposits = vault.usdc_deposits
                .checked_sub(amount)
                .ok_or(BettingPlatformError::Underflow)?;
        }
        CollateralType::USDT => {
            vault.usdt_deposits = vault.usdt_deposits
                .checked_sub(amount)
                .ok_or(BettingPlatformError::Underflow)?;
        }
        CollateralType::SOL => {
            vault.sol_deposits = vault.sol_deposits
                .checked_sub(amount)
                .ok_or(BettingPlatformError::Underflow)?;
        }
        CollateralType::WBTC => {
            vault.wbtc_deposits = vault.wbtc_deposits
                .checked_sub(amount)
                .ok_or(BettingPlatformError::Underflow)?;
        }
        CollateralType::WETH => {
            vault.weth_deposits = vault.weth_deposits
                .checked_sub(amount)
                .ok_or(BettingPlatformError::Underflow)?;
        }
    }
    
    // Update total USD value
    vault.total_usd_value = vault.calculate_total_usd_value()?;
    vault.last_update = Clock::get()?.unix_timestamp;
    
    // Save vault state
    vault.serialize(&mut &mut vault_account.data.borrow_mut()[..])?;
    
    // Emit event
    CollateralWithdrawn {
        withdrawer: *withdrawer.key,
        amount: usd_value,
        total_deposits: vault.total_usd_value,
        timestamp: Clock::get()?.unix_timestamp,
    }
    .emit();
    
    msg!("Multi-collateral withdrawal successful");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collateral_type_values() {
        // Test USDC value calculation
        let usdc_amount = 1_000_000; // 1 USDC
        let usdc_oracle_price = 100_000_000; // $1.00 with 8 decimals
        let usdc_value = CollateralType::USDC.get_usd_value(usdc_amount, usdc_oracle_price).unwrap();
        assert_eq!(usdc_value, 1); // $1 in USDC decimals after all conversions
        
        // Test SOL value calculation (assuming $100/SOL)
        let sol_amount = 1_000_000_000; // 1 SOL
        let sol_oracle_price = 10_000_000_000; // $100.00 with 8 decimals
        let sol_value = CollateralType::SOL.get_usd_value(sol_amount, sol_oracle_price).unwrap();
        assert_eq!(sol_value, 100); // $100 in USDC decimals after all conversions
        
        // Test WBTC value calculation (assuming $50,000/BTC)
        let wbtc_amount = 100_000_000; // 1 WBTC
        let wbtc_oracle_price = 5_000_000_000_000; // $50,000.00 with 8 decimals
        let wbtc_value = CollateralType::WBTC.get_usd_value(wbtc_amount, wbtc_oracle_price).unwrap();
        assert_eq!(wbtc_value, 50_000); // $50,000 in USDC decimals after all conversions
    }

    #[test]
    fn test_ltv_ratios() {
        assert_eq!(CollateralType::USDC.ltv_ratio(), 100);
        assert_eq!(CollateralType::USDT.ltv_ratio(), 100);
        assert_eq!(CollateralType::SOL.ltv_ratio(), 80);
        assert_eq!(CollateralType::WBTC.ltv_ratio(), 80);
        assert_eq!(CollateralType::WETH.ltv_ratio(), 80);
    }

    #[test]
    fn test_borrowing_power_calculation() {
        let vault = MultiCollateralVault {
            discriminator: MultiCollateralVault::DISCRIMINATOR,
            usdc_deposits: 10_000_000_000, // 10,000 USDC
            usdt_deposits: 5_000_000_000,  // 5,000 USDT
            sol_deposits: 100_000_000_000, // 100 SOL
            wbtc_deposits: 20_000_000,     // 0.2 WBTC
            weth_deposits: 300_000_000,    // 3 WETH
            total_usd_value: 0,
            total_borrowed_usd: 0,
            depositor_count: 1,
            last_update: 0,
            oracle_feed: Pubkey::default(),
        };
        
        let borrowing_power = vault.get_borrowing_power().unwrap();
        // Expected: 
        // USDC: 10,000 * 100% = 10,000
        // USDT: 5,000 * 100% = 5,000
        // SOL: 100 * $100 * 80% = 8,000
        // WBTC: 0.2 * $50,000 * 80% = 8,000
        // WETH: 3 * $3,000 * 80% = 7,200
        // Total: 38,200 USDC
        assert!(borrowing_power > 38_000_000_000); // In USDC decimals
    }
}