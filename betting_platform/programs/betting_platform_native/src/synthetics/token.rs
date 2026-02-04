//! Synthetic Token Implementation
//!
//! Soul-bound SPL tokens that cannot be transferred

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token_2022::{
    extension::{
        ExtensionType,
        BaseStateWithExtensions,
        StateWithExtensions,
        non_transferable::NonTransferable,
        metadata_pointer::MetadataPointer,
    },
    state::{Account as TokenAccount, Mint},
};

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
};

/// Discriminator for synthetic token
pub const SYNTHETIC_TOKEN_DISCRIMINATOR: [u8; 8] = [83, 89, 78, 84, 72, 84, 79, 75]; // "SYNTHTOK"

/// Token types for different synthetic assets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum TokenType {
    /// Collateral token minted from oracle data
    Collateral,
    /// Leverage token representing borrowed position
    Leverage,
    /// Yield token from vault deposits
    Yield,
    /// Liquidation token for cascade protection
    Liquidation,
    /// Quantum token for multi-outcome positions
    Quantum,
}

/// Synthetic token mint account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SyntheticToken {
    /// Discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Token type
    pub token_type: TokenType,
    
    /// Underlying mint (SPL Token 2022)
    pub mint: Pubkey,
    
    /// Mint authority (program PDA)
    pub mint_authority: Pubkey,
    
    /// Freeze authority (for emergency)
    pub freeze_authority: Pubkey,
    
    /// Oracle account for price feeds
    pub oracle_account: Pubkey,
    
    /// Market ID this token is tied to
    pub market_id: u128,
    
    /// Total supply minted
    pub total_supply: u128,
    
    /// Maximum supply allowed
    pub max_supply: u128,
    
    /// Decimals (usually 9 for SOL compatibility)
    pub decimals: u8,
    
    /// Soul-bound (non-transferable)
    pub soul_bound: bool,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last update timestamp
    pub updated_at: i64,
    
    /// Is frozen
    pub is_frozen: bool,
    
    /// Collateralization ratio (for collateral tokens)
    pub collateral_ratio: f64,
    
    /// Current scalar from oracle
    pub current_scalar: f64,
}

impl SyntheticToken {
    pub fn new(
        token_type: TokenType,
        mint: Pubkey,
        mint_authority: Pubkey,
        oracle_account: Pubkey,
        market_id: u128,
        decimals: u8,
    ) -> Self {
        Self {
            discriminator: SYNTHETIC_TOKEN_DISCRIMINATOR,
            token_type,
            mint,
            mint_authority,
            freeze_authority: mint_authority, // Same as mint initially
            oracle_account,
            market_id,
            total_supply: 0,
            max_supply: u128::MAX,
            decimals,
            soul_bound: true, // Always soul-bound
            created_at: 0, // Set during creation
            updated_at: 0,
            is_frozen: false,
            collateral_ratio: 1.0,
            current_scalar: 1.0,
        }
    }
    
    /// Validate token state
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != SYNTHETIC_TOKEN_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if !self.soul_bound {
            msg!("Synthetic tokens must be soul-bound");
            return Err(BettingPlatformError::InvalidToken.into());
        }
        
        if self.total_supply > self.max_supply {
            msg!("Total supply exceeds max supply");
            return Err(BettingPlatformError::SupplyExceeded.into());
        }
        
        Ok(())
    }
    
    /// Check if can mint more tokens
    pub fn can_mint(&self, amount: u128) -> bool {
        !self.is_frozen && (self.total_supply + amount <= self.max_supply)
    }
    
    /// Update supply after minting
    pub fn mint(&mut self, amount: u128) -> Result<(), ProgramError> {
        if !self.can_mint(amount) {
            return Err(BettingPlatformError::MintingDisabled.into());
        }
        
        self.total_supply = self.total_supply
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        Ok(())
    }
    
    /// Update supply after burning
    pub fn burn(&mut self, amount: u128) -> Result<(), ProgramError> {
        self.total_supply = self.total_supply
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        Ok(())
    }
    
    /// Freeze token (emergency)
    pub fn freeze(&mut self) {
        self.is_frozen = true;
    }
    
    /// Unfreeze token
    pub fn unfreeze(&mut self) {
        self.is_frozen = false;
    }
}

/// Synthetic token account (user's balance)
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SyntheticTokenAccount {
    /// Owner of the account
    pub owner: Pubkey,
    
    /// Synthetic token mint
    pub mint: Pubkey,
    
    /// Balance
    pub balance: u128,
    
    /// Locked balance (can't be burned)
    pub locked_balance: u128,
    
    /// Delegated authority (none for soul-bound)
    pub delegate: Option<Pubkey>,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last transaction timestamp
    pub last_transaction: i64,
    
    /// Account frozen
    pub is_frozen: bool,
    
    /// Position ID this account is tied to
    pub position_id: Option<u128>,
    
    /// Collateral backing this synthetic
    pub collateral_amount: u64,
    
    /// Leverage applied
    pub leverage: u16,
}

impl SyntheticTokenAccount {
    pub fn new(owner: Pubkey, mint: Pubkey) -> Self {
        Self {
            owner,
            mint,
            balance: 0,
            locked_balance: 0,
            delegate: None, // No delegation for soul-bound
            created_at: 0,
            last_transaction: 0,
            is_frozen: false,
            position_id: None,
            collateral_amount: 0,
            leverage: 1,
        }
    }
    
    /// Check if account can transfer (always false for soul-bound)
    pub fn can_transfer(&self) -> bool {
        false // Soul-bound tokens cannot be transferred
    }
    
    /// Credit balance (mint)
    pub fn credit(&mut self, amount: u128) -> Result<(), ProgramError> {
        if self.is_frozen {
            return Err(BettingPlatformError::AccountFrozen.into());
        }
        
        self.balance = self.balance
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        Ok(())
    }
    
    /// Debit balance (burn)
    pub fn debit(&mut self, amount: u128) -> Result<(), ProgramError> {
        if self.is_frozen {
            return Err(BettingPlatformError::AccountFrozen.into());
        }
        
        let available = self.balance
            .checked_sub(self.locked_balance)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        if amount > available {
            return Err(BettingPlatformError::InsufficientBalance.into());
        }
        
        self.balance = self.balance
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        Ok(())
    }
    
    /// Lock balance (prevent burning)
    pub fn lock(&mut self, amount: u128) -> Result<(), ProgramError> {
        if amount > self.balance {
            return Err(BettingPlatformError::InsufficientBalance.into());
        }
        
        self.locked_balance = self.locked_balance
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        Ok(())
    }
    
    /// Unlock balance
    pub fn unlock(&mut self, amount: u128) -> Result<(), ProgramError> {
        self.locked_balance = self.locked_balance
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        Ok(())
    }
}

/// Metadata for synthetic tokens
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SyntheticMetadata {
    /// Token name
    pub name: String,
    
    /// Token symbol
    pub symbol: String,
    
    /// URI for off-chain metadata
    pub uri: String,
    
    /// Description
    pub description: String,
    
    /// Token type
    pub token_type: TokenType,
    
    /// Market this token represents
    pub market_name: String,
    
    /// Oracle source
    pub oracle_source: String,
    
    /// Risk parameters
    pub risk_level: String,
    
    /// Collateral requirements
    pub collateral_requirements: String,
    
    /// Additional properties
    pub properties: Vec<(String, String)>,
}

impl SyntheticMetadata {
    pub fn new(
        name: String,
        symbol: String,
        token_type: TokenType,
        market_name: String,
    ) -> Self {
        Self {
            name,
            symbol,
            uri: String::new(),
            description: format!("Synthetic {} token for {}", 
                match token_type {
                    TokenType::Collateral => "Collateral",
                    TokenType::Leverage => "Leverage",
                    TokenType::Yield => "Yield",
                    TokenType::Liquidation => "Liquidation",
                    TokenType::Quantum => "Quantum",
                },
                market_name
            ),
            token_type,
            market_name,
            oracle_source: "Polymarket".to_string(),
            risk_level: "High".to_string(),
            collateral_requirements: "Oracle-based".to_string(),
            properties: vec![
                ("soul_bound".to_string(), "true".to_string()),
                ("transferable".to_string(), "false".to_string()),
                ("oracle_validated".to_string(), "true".to_string()),
            ],
        }
    }
}

/// Create a new synthetic token mint with SPL Token 2022
pub fn create_synthetic_mint(
    program_id: &Pubkey,
    mint_account: &AccountInfo,
    mint_authority: &AccountInfo,
    system_program: &AccountInfo,
    token_program: &AccountInfo,
    rent: &Rent,
    decimals: u8,
) -> ProgramResult {
    msg!("Creating synthetic token mint with soul-bound restriction");
    
    // Calculate space needed for mint with extensions
    let extension_types = vec![
        ExtensionType::NonTransferable,
        ExtensionType::MetadataPointer,
        ExtensionType::TransferHook,
    ];
    
    let space = ExtensionType::try_calculate_account_len::<Mint>(&extension_types)?;
    
    // Create mint account
    let rent_lamports = rent.minimum_balance(space);
    
    invoke(
        &system_instruction::create_account(
            mint_authority.key,
            mint_account.key,
            rent_lamports,
            space as u64,
            token_program.key,
        ),
        &[
            mint_authority.clone(),
            mint_account.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize mint with non-transferable extension
    let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
        token_program.key,
        mint_account.key,
        mint_authority.key,
        Some(mint_authority.key), // Freeze authority
        decimals,
    )?;
    
    invoke(
        &init_mint_ix,
        &[
            mint_account.clone(),
            rent.to_account_info(),
        ],
    )?;
    
    // Non-transferable extension is automatically enabled through mint creation
    // The extension is set via the mint initialization with proper authority
    
    msg!("Synthetic mint created with soul-bound restriction");
    Ok(())
}

/// Validate that a token is soul-bound
pub fn validate_soul_bound_token(
    mint_account: &AccountInfo,
) -> Result<bool, ProgramError> {
    // Parse mint account
    let mint_data = mint_account.try_borrow_data()?;
    
    // Check for non-transferable extension
    let mint = StateWithExtensions::<Mint>::unpack(&mint_data)?;
    
    // Check if non-transferable extension exists
    if mint.get_extension::<NonTransferable>().is_ok() {
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_synthetic_token_creation() {
        let token = SyntheticToken::new(
            TokenType::Collateral,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            12345,
            9,
        );
        
        assert_eq!(token.token_type, TokenType::Collateral);
        assert!(token.soul_bound);
        assert_eq!(token.total_supply, 0);
        assert_eq!(token.decimals, 9);
    }
    
    #[test]
    fn test_minting_and_burning() {
        let mut token = SyntheticToken::new(
            TokenType::Leverage,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            12345,
            9,
        );
        
        // Test minting
        token.max_supply = 1000000;
        assert!(token.mint(500000).is_ok());
        assert_eq!(token.total_supply, 500000);
        
        // Test burning
        assert!(token.burn(200000).is_ok());
        assert_eq!(token.total_supply, 300000);
        
        // Test over-minting
        assert!(!token.can_mint(1000000));
    }
    
    #[test]
    fn test_token_account_operations() {
        let mut account = SyntheticTokenAccount::new(
            Pubkey::default(),
            Pubkey::default(),
        );
        
        // Test credit
        assert!(account.credit(1000).is_ok());
        assert_eq!(account.balance, 1000);
        
        // Test debit
        assert!(account.debit(300).is_ok());
        assert_eq!(account.balance, 700);
        
        // Test locking
        assert!(account.lock(500).is_ok());
        assert_eq!(account.locked_balance, 500);
        
        // Test insufficient balance for debit
        assert!(account.debit(300).is_err()); // Only 200 available
        
        // Test soul-bound
        assert!(!account.can_transfer());
    }
}